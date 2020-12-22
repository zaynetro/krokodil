use std::collections::HashMap;
use std::env;
use std::sync::Arc;

use log::info;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;
use warp::ws::Message;
use warp::Filter;

mod errors;

mod games;

use games::Games;

pub type App = Arc<Mutex<AppState>>;

type PlayerConn = mpsc::UnboundedSender<Result<Message, warp::Error>>;

pub struct AppState {
    games: Games,
    // All active websocket connections
    connections: HashMap<Uuid, PlayerConn>,
}

#[tokio::main]
async fn main() {
    if env::var_os("RUST_LOG").is_none() {
        // Set `RUST_LOG=backend=debug` to see debug logs,
        // this only shows access logs.
        env::set_var("RUST_LOG", "krokodil=info");
    }

    pretty_env_logger::init();

    let (host, port) = match env::var("PORT") {
        Ok(port) => ([0, 0, 0, 0], port.parse().expect("PORT must be a number")),
        Err(_) => ([127, 0, 0, 1], 3030),
    };

    // TODO: we need a thread that will remove players and games after a period of inactivity
    let app = Arc::new(Mutex::new(AppState {
        games: Games::new(),
        connections: HashMap::new(),
    }));

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

mod filters {
    use std::convert::Infallible;

    use serde::Deserialize;
    use uuid::Uuid;
    use warp::http::header;
    use warp::{filters::reply, Filter};

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
                "default-src 'self'",
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
                ws.on_upgrade(move |websocket| {
                    handlers::sync(websocket, app, query.game_id, query.player_id)
                })
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

    #[derive(Debug, Deserialize)]
    pub struct SyncQuery {
        pub game_id: String,
        pub player_id: Option<Uuid>,
    }
}

mod handlers {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use futures::{FutureExt, SinkExt, StreamExt};
    use log::{error, info};
    use serde::{Deserialize, Serialize};
    use tokio::sync::mpsc;
    use uuid::Uuid;
    use warp::http::{StatusCode, Uri};
    use warp::ws::Message;

    use super::{App, PlayerConn};
    use crate::games::Player;
    use crate::games::{DrawingSegment, Game};

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

    pub async fn sync(
        websocket: warp::filters::ws::WebSocket,
        app: App,
        game_id: String,
        maybe_player_id: Option<Uuid>,
    ) {
        let player_id = maybe_player_id.unwrap_or(Uuid::new_v4());
        if maybe_player_id.is_some() {
            info!("Existing player {} in game {}", player_id, game_id);
        } else {
            info!("New player {} in game {}", player_id, game_id);
        }

        // Split the socket into a sender and receive of messages.
        let (mut ws_tx, mut ws_rx) = websocket.split();

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
            tx,
            player_id,
            new_player: maybe_player_id.is_none(),
            game_id,
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
        tx: PlayerConn,
        player_id: Uuid,
        new_player: bool,
        game_id: String,
    }

    impl PlayerConnLifecycle {
        /// Add our player to the game and to the known connections. Then send game info
        async fn init(&mut self) {
            let mut app = self.app.lock().await;
            // TODO: a single player might have multiple connections
            app.connections.insert(self.player_id, self.tx.clone());

            let (game, player) = app.games.add_player(&self.game_id, self.player_id.clone());

            if self.new_player {
                // Send this player ids only if it was new
                self.tx
                    .send(message(OutgoingEvent {
                        from_event_id: None,
                        body: OutgoingEventBody::YouAre { player },
                    }))
                    .expect("Send player info");
            }

            // Send game info
            self.tx
                .send(message(OutgoingEvent {
                    from_event_id: None,
                    body: OutgoingEventBody::Game(game.clone()),
                }))
                .expect("Send game");

            // Send current drawing
            game.iter_drawing(|segment| {
                self.tx
                    .send(message(OutgoingEvent {
                        from_event_id: None,
                        body: OutgoingEventBody::AddDrawingSegment(segment.clone()),
                    }))
                    .expect("Send segment");
            });

            // TODO: notify other players about new player

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
                    self.tx
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

                IncomingEventBody::SubmitWord { word } => {
                    let game = {
                        let mut app = self.app.lock().await;
                        let game = app.games.find_mut(&self.game_id).expect("Game");
                        if !game.submit_word(&self.player_id, word) {
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
                            let _ = self.tx.send(message(OutgoingEvent {
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
            }
        }

        async fn disconnected(&mut self) {
            // Remove player connection
            self.app.lock().await.connections.remove(&self.player_id);

            log::debug!("Player {} disconnected", self.player_id);
            // TODO: schedule player to be removed from the game
        }

        async fn notify_all(&self, event: OutgoingEvent) {
            let app = self.app.lock().await;
            let game = app.games.find(&self.game_id).expect("Game");

            for player in &game.players {
                if let Some(tx) = app.connections.get(&player.id) {
                    let _ = tx.send(message(event.clone()));
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

                if let Some(tx) = app.connections.get(&player.id) {
                    let _ = tx.send(message(event.clone()));
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
        },
        GuessWord {
            word: String,
        },
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
        ClearDrawing {},
        Pong,
    }
}
