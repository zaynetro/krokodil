use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::info;
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{mpsc, Mutex},
    time::interval,
};
use uuid::Uuid;
use warp::ws::Message;
use warp::Filter;

mod errors;

mod games;

use games::{CanvasSize, DrawingSegment, Game, Games, Player};

pub type App = Arc<Mutex<AppState>>;

/// A reference to player connection
#[derive(Clone)]
pub struct PlayerConn {
    pub id: usize,
    pub tx: mpsc::UnboundedSender<Result<Message, warp::Error>>,
}

pub struct AppState {
    games: Games,
    /// All active websocket connections. A mapping from player id to connection reference.
    connections: HashMap<Uuid, PlayerConn>,
    /// A mapping from player id to the time when WS connection ended.
    exited_players: HashMap<Uuid, Instant>,
}

const REMOVE_PLAYER_AFTER: Duration = Duration::from_secs(60 * 5);

// TODO: error handling

#[tokio::main]
async fn main() {
    if env::var_os("RUST_LOG").is_none() {
        // Set `RUST_LOG=backend=debug` to see debug logs,
        // this only shows access logs.
        env::set_var("RUST_LOG", "krokodil=debug");
    }

    pretty_env_logger::init();

    let (host, port) = match env::var("PORT") {
        Ok(port) => ([0, 0, 0, 0], port.parse().expect("PORT must be a number")),
        Err(_) => ([127, 0, 0, 1], 3030),
    };

    let app = Arc::new(Mutex::new(AppState {
        games: Games::new(),
        connections: HashMap::new(),
        exited_players: HashMap::new(),
    }));
    tokio::spawn(remove_players_job(app.clone()));

    let routes = filters::index()
        .or(filters::static_files())
        .or(filters::create_game(app.clone()))
        .or(filters::game(app.clone()))
        .or(filters::sync(app.clone()))
        .with(warp::compression::gzip());

    info!("Listening on {:?}:{}", host, port);
    warp::serve(routes.with(warp::log("backend")))
        .run((host, port))
        .await;
}

/// Periodically scan for exited players and remove them from games.
async fn remove_players_job(app: App) {
    loop {
        interval(Duration::from_secs(30)).tick().await;

        let mut remove_players = vec![];

        {
            // Prepare a list of players to remove
            let mut app = app.lock().await;
            let now = Instant::now();
            for (player_id, exited_at) in &mut app.exited_players {
                if now.duration_since(*exited_at) > REMOVE_PLAYER_AFTER {
                    remove_players.push(player_id.clone());
                }
            }
        }

        let mut all_modified_games = HashMap::new();
        {
            // Remove players from the games
            let mut app = app.lock().await;
            for player_id in &remove_players {
                log::debug!("Removing exited player {}", player_id);
                let modified_games = app.games.remove_player(&player_id);
                for game in modified_games {
                    all_modified_games.insert(game.id.clone(), game);
                }
            }
        }

        {
            // Notify all other players in the modified games
            let app = app.lock().await;
            for game in all_modified_games.values() {
                log::debug!(
                    "Notifying {} players in game={} about removed player",
                    game.players.len(),
                    game.id
                );
                for player in &game.players {
                    if let Some(conn) = app.connections.get(&player.id) {
                        let _ = conn.tx.send(message(OutgoingEvent {
                            from_event_id: None,
                            body: OutgoingEventBody::Game(game.clone()),
                        }));
                    }
                }
            }
        }

        // Remove exited players
        let mut app = app.lock().await;
        for player_id in &remove_players {
            app.exited_players.remove(&player_id);
        }
    }
}

mod filters {
    use std::convert::Infallible;

    use warp::http::header;
    use warp::{filters::reply, Filter};

    use crate::SyncQuery;

    use super::{errors, handlers, App};

    pub fn index() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path::end())
            .and(warp::fs::file("./ui/static/index.html"))
            .with(reply::header(
                header::CONTENT_SECURITY_POLICY,
                "default-src 'self'",
            ))
    }

    pub fn static_files() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
    {
        warp::path("static").and(warp::fs::dir("./ui/dist").or(warp::fs::dir("./ui/static")))
    }

    pub fn create_game(
        app: App,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::post()
            .and(warp::path::end())
            .and(with_app(app.clone()))
            .and_then(handlers::create_game)
    }

    pub fn game(
        app: App,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("game" / String)
            .and(warp::get())
            .and(with_app(app.clone()))
            .and_then(handlers::game)
            .with(reply::header(
                header::CONTENT_SECURITY_POLICY,
                // We allow websocket connections explicitly because ios otherwise will not work
                "default-src 'self' ws: wss:",
            ))
    }

    pub fn sync(
        app: App,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("sync")
            .and(warp::query::<SyncQuery>())
            .and(with_app(app.clone()))
            .and_then(require_game_id)
            .and(warp::ws())
            .map(|(app, query): (App, SyncQuery), ws: warp::ws::Ws| {
                ws.on_upgrade(move |websocket| handlers::sync(websocket, app, query))
            })
    }

    async fn require_game_id(
        query: SyncQuery,
        app: App,
    ) -> Result<(App, SyncQuery), warp::Rejection> {
        let game_present = {
            let app = app.lock().await;
            app.games.exists(&query.game_id)
        };
        if game_present {
            Ok((app, query))
        } else {
            Err(warp::reject::custom(errors::MissingGame))
        }
    }

    fn with_app(app: App) -> impl Filter<Extract = (App,), Error = Infallible> + Clone {
        warp::any().map(move || app.clone())
    }
}

mod handlers {
    use std::{
        collections::hash_map::Entry,
        sync::atomic::{AtomicUsize, Ordering},
        time::Instant,
    };

    use futures::{FutureExt, StreamExt};
    use log::{error, info};
    use tokio::sync::mpsc;
    use uuid::Uuid;
    use warp::http::Uri;
    use warp::ws::Message;

    use super::{App, PlayerConn};
    use crate::{
        message, IncomingEvent, IncomingEventBody, OutgoingEvent, OutgoingEventBody, SyncQuery,
    };

    /// Our global unique conn id counter.
    static NEXT_CONN_ID: AtomicUsize = AtomicUsize::new(1);
    const GAME_HTML: &str = include_str!("../ui/static/game.html");

    pub async fn create_game(app: App) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
        let mut app = app.lock().await;
        let game_id = app.games.reserve_id();
        let url = format!("/game/{}", game_id);
        log::debug!("Created a new game {}", url);
        Ok(Box::new(warp::redirect(
            url.parse::<Uri>().expect("Parse uri"),
        )))
    }

    pub async fn game(game_id: String, app: App) -> Result<Box<dyn warp::Reply>, warp::Rejection> {
        let app = app.lock().await;
        if app.games.exists(&game_id) {
            log::debug!("Game found {}", game_id);
            Ok(Box::new(warp::reply::html(GAME_HTML)))
        } else {
            log::debug!("Unknown game {}", game_id);
            Ok(Box::new(warp::redirect(Uri::from_static("/"))))
        }
    }

    pub async fn sync(websocket: warp::filters::ws::WebSocket, app: App, query: SyncQuery) {
        let player_id = query.player_id.unwrap_or(Uuid::new_v4());
        let conn_id = NEXT_CONN_ID.fetch_add(1, Ordering::Relaxed);
        if query.player_id.is_some() {
            info!(
                "Existing player {} in game {} conn={}",
                player_id, query.game_id, conn_id
            );
        } else {
            info!(
                "New player {} in game {} conn={}",
                player_id, query.game_id, conn_id
            );
        }

        // Split the socket into a sender and receive of messages.
        let (ws_tx, mut ws_rx) = websocket.split();

        // Use an unbounded channel to handle buffering and flushing of messages
        // to the websocket...
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::task::spawn(rx.forward(ws_tx).map(|result| {
            if let Err(e) = result {
                error!("websocket send error: {}", e);
            }
        }));

        let mut player_lifecycle = PlayerConnLifecycle {
            app: app.clone(),
            conn: PlayerConn { id: conn_id, tx },
            player_id,
            player_nickname: query.nickname,
            new_player: query.player_id.is_none(),
            game_id: query.game_id,
        };

        player_lifecycle.init().await;

        // Read player messages
        while let Some(result) = ws_rx.next().await {
            let msg = match result {
                Ok(msg) => msg,
                Err(e) => {
                    error!("websocket error(uid={}): {}", player_id, e);
                    break;
                }
            };

            player_lifecycle.on_message(msg).await;
        }

        // Once stream ends -> connection disconnected
        player_lifecycle.disconnected().await;
    }

    struct PlayerConnLifecycle {
        app: App,
        conn: PlayerConn,
        player_id: Uuid,
        player_nickname: Option<String>,
        new_player: bool,
        game_id: String,
    }

    impl PlayerConnLifecycle {
        /// Add our player to the game and to the known connections. Then send game info
        async fn init(&mut self) {
            let mut app = self.app.lock().await;
            // Replace existing connection if there were. We support running game in a single tab only.
            app.connections.insert(self.player_id, self.conn.clone());
            app.exited_players.remove(&self.player_id);

            let (game, player) = app.games.add_player(
                &self.game_id,
                self.player_id.clone(),
                self.player_nickname.clone(),
            );

            if self.new_player {
                // Send this player ids only if it was new
                self.conn
                    .tx
                    .send(message(OutgoingEvent {
                        from_event_id: None,
                        body: OutgoingEventBody::YouAre { player },
                    }))
                    .expect("Send player info");
            }

            // TODO: Send game info to all players
            self.conn
                .tx
                .send(message(OutgoingEvent {
                    from_event_id: None,
                    body: OutgoingEventBody::Game(game.clone()),
                }))
                .expect("Send game");

            // Send current drawing
            game.iter_drawing(|segment| {
                self.conn
                    .tx
                    .send(message(OutgoingEvent {
                        from_event_id: None,
                        body: OutgoingEventBody::AddDrawingSegment(segment.clone()),
                    }))
                    .expect("Send segment");
            });

            log::debug!("Player {} initialized", self.player_id);
        }

        async fn on_message(&mut self, msg: Message) {
            let event_str = match msg.to_str() {
                Ok(s) => s,
                Err(_) => {
                    // Skip non text messages
                    return;
                }
            };
            log::debug!("Received message {}", event_str);
            let event: IncomingEvent = match serde_json::from_str(event_str) {
                Ok(event) => event,
                Err(err) => {
                    error!("Failed to read WS message: {} (event={})", err, event_str);
                    return;
                }
            };

            match event.body {
                IncomingEventBody::Ping => {
                    self.conn
                        .tx
                        .send(message(OutgoingEvent {
                            from_event_id: None,
                            body: OutgoingEventBody::Pong,
                        }))
                        .expect("Send pong message");
                    log::debug!("Pong sent");
                }

                IncomingEventBody::AddDrawingSegment(segment) => {
                    {
                        // Add segment to the state
                        let mut app = self.app.lock().await;
                        let game = app.games.find_mut(&self.game_id).expect("Game");
                        game.add_segment(segment.clone());
                    }

                    // Let others know
                    self.notify_others(OutgoingEvent {
                        from_event_id: None,
                        body: OutgoingEventBody::AddDrawingSegment(segment),
                    })
                    .await;
                    log::debug!("Added drawing segment to other players notified");
                }

                IncomingEventBody::RemoveDrawingSegment { segment_id } => {
                    {
                        // Remove segment from the state
                        let mut app = self.app.lock().await;
                        let game = app.games.find_mut(&self.game_id).expect("Game");
                        game.remove_segment(&segment_id);
                    }

                    // Let others know
                    self.notify_others(OutgoingEvent {
                        from_event_id: None,
                        body: OutgoingEventBody::RemoveDrawingSegment { segment_id },
                    })
                    .await;
                    log::debug!("Removed drawing segment to other players notified");
                }

                IncomingEventBody::SubmitWord { word, canvas } => {
                    let game = {
                        let mut app = self.app.lock().await;
                        let game = app.games.find_mut(&self.game_id).expect("Game");
                        if !game.submit_word(&self.player_id, word, canvas) {
                            // Return when game wasn't changed
                            return;
                        }
                        game.clone()
                    };

                    // Clear drawing for all
                    self.notify_all(OutgoingEvent {
                        from_event_id: None,
                        body: OutgoingEventBody::ClearDrawing {},
                    })
                    .await;

                    // Notify all players of games changes
                    self.notify_all(OutgoingEvent {
                        from_event_id: None,
                        body: OutgoingEventBody::Game(game),
                    })
                    .await;

                    log::debug!("Player {} submitted a word", self.player_id);
                }

                IncomingEventBody::GuessWord { word } => {
                    let game = {
                        let mut app = self.app.lock().await;
                        let game = app.games.find_mut(&self.game_id).expect("Game");
                        if !game.guess_word(&self.player_id, &word) {
                            // Notify wrong guess
                            let _ = self.conn.tx.send(message(OutgoingEvent {
                                from_event_id: event.event_id,
                                body: OutgoingEventBody::WrongGuess {},
                            }));
                            return;
                        }
                        game.clone()
                    };

                    // Notify all players of games changes
                    self.notify_all(OutgoingEvent {
                        from_event_id: None,
                        body: OutgoingEventBody::Game(game),
                    })
                    .await;
                    log::debug!("Player {} guessed a word", self.player_id);
                }

                IncomingEventBody::AskWordTip {} => {
                    let mut app = self.app.lock().await;
                    let game = app.games.find_mut(&self.game_id).expect("Game");
                    if let Some(tip) = game.ask_word_tip() {
                        let _ = self.conn.tx.send(message(OutgoingEvent {
                            from_event_id: event.event_id,
                            body: OutgoingEventBody::WordTip { tip },
                        }));
                    }

                    log::debug!("Player {} asked a tip", self.player_id);
                }
            }
        }

        async fn disconnected(&mut self) {
            // Remove player connection that is the same as this one
            let mut app = self.app.lock().await;
            if let Entry::Occupied(e) = app.connections.entry(self.player_id) {
                if e.get().id == self.conn.id {
                    log::debug!("Exiting player {} conn={}", self.player_id, self.conn.id);
                    e.remove();
                }
            }
            app.exited_players.insert(self.player_id, Instant::now());

            log::debug!(
                "Player {} disconnected conn={}",
                self.player_id,
                self.conn.id
            );
        }

        async fn notify_all(&self, event: OutgoingEvent) {
            let app = self.app.lock().await;
            let game = app.games.find(&self.game_id).expect("Game");

            for player in &game.players {
                if let Some(conn) = app.connections.get(&player.id) {
                    let _ = conn.tx.send(message(event.clone()));
                }
            }
        }

        async fn notify_others(&self, event: OutgoingEvent) {
            let app = self.app.lock().await;
            let game = app.games.find(&self.game_id).expect("Game");

            for player in &game.players {
                if self.player_id == player.id {
                    // Do not send it to ourselves
                    continue;
                }

                if let Some(conn) = app.connections.get(&player.id) {
                    let _ = conn.tx.send(message(event.clone()));
                }
            }
        }
    }
}

fn message(response: impl Serialize) -> Result<Message, warp::Error> {
    let text = serde_json::to_string(&response).expect("Serialize WS message");
    Ok(Message::text(&text))
}

/// IncomingEvent represents every possible incoming message
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IncomingEvent {
    pub event_id: Option<String>,
    pub body: IncomingEventBody,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum IncomingEventBody {
    AddDrawingSegment(DrawingSegment),
    #[serde(rename_all = "camelCase")]
    RemoveDrawingSegment {
        segment_id: String,
    },
    SubmitWord {
        word: String,
        canvas: CanvasSize,
    },
    GuessWord {
        word: String,
    },
    AskWordTip {},
    Ping,
}

/// OutgoingEvent represents every possible outgoing message
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OutgoingEvent {
    pub from_event_id: Option<String>,
    pub body: OutgoingEventBody,
}

#[derive(Serialize, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum OutgoingEventBody {
    Game(Game),
    AddDrawingSegment(DrawingSegment),
    #[serde(rename_all = "camelCase")]
    RemoveDrawingSegment {
        segment_id: String,
    },
    #[serde(rename_all = "camelCase")]
    YouAre {
        player: Player,
    },
    WrongGuess {},
    WordTip {
        tip: String,
    },
    ClearDrawing {},
    Pong,
}

#[derive(Debug, Deserialize)]
pub struct SyncQuery {
    pub game_id: String,
    pub player_id: Option<Uuid>,
    pub nickname: Option<String>,
}
