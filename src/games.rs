use std::collections::{hash_map::Entry, HashMap, HashSet};

use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug)]
pub struct Games {
    /// Reserved game ids
    pending_ids: HashSet<String>,
    /// Games with joined players
    rooms: HashMap<String, Game>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Game {
    id: String,
    stage: GameStage,
    pub players: Vec<Player>,
    history: Vec<Turn>,
}

impl Game {
    fn new(id: String, player: Player) -> Self {
        Self {
            id,
            stage: GameStage::PlayerChoosing {
                player_id: player.id.clone(),
            },
            players: vec![player],
            history: vec![],
        }
    }

    /// Add a player to the game
    fn add_player(&mut self, player: Player) {
        // TODO: check that player is not there already
        self.players.push(player);
    }

    /// Add drawing segment if we are in drawing stage
    pub fn add_segment(&mut self, segment: DrawingSegment) {
        if let GameStage::PlayerDrawing {
            ref mut drawing, ..
        } = &mut self.stage
        {
            drawing.push(segment);
        }
    }

    /// Remove drawing segment if we are in drawing stage
    pub fn remove_segment(&mut self, segment_id: &str) {
        if let GameStage::PlayerDrawing {
            ref mut drawing, ..
        } = &mut self.stage
        {
            drawing.retain(|s| s.id != segment_id);
        }
    }

    /// Submit a word to draw. Transitions to drawing stage if this player was allowed to do that.
    /// Return true if transitioned.
    pub fn submit_word(&mut self, submitting_player_id: &Uuid, word: String) -> bool {
        match self.stage {
            GameStage::PlayerChoosing { player_id } if submitting_player_id == &player_id => {
                // continue
                self.stage = GameStage::PlayerDrawing {
                    player_id,
                    word: word.trim().to_string(),
                    drawing: vec![],
                };
                true
            }
            _ => {
                // This player cannot submit a word to draw
                false
            }
        }
    }

    /// Guess a word. Transitions to choose a word stage if guess was correct.
    /// Return true if transitioned.
    pub fn guess_word(&mut self, guessing_player_id: &Uuid, guess: &str) -> bool {
        match &self.stage {
            // TODO: case insensitive comparison
            GameStage::PlayerDrawing { word, .. } if word == guess => {
                self.stage = GameStage::PlayerChoosing {
                    player_id: guessing_player_id.clone(),
                };
                true
            }
            _ => {
                // Wrong guess or state
                false
            }
        }
    }

    /// Iterate over drawing segments if there is a drawing
    pub fn iter_drawing(&self, cb: impl Fn(&DrawingSegment)) {
        if let GameStage::PlayerDrawing { drawing, .. } = &self.stage {
            for segment in drawing {
                cb(&segment);
            }
        }
    }
}

// TODO: implement custom Clone to skip cloning drawing
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
enum GameStage {
    /// A player is choosing a word
    #[serde(rename_all = "camelCase")]
    PlayerChoosing { player_id: Uuid },

    /// A player is drawing while others are guessing
    #[serde(rename_all = "camelCase")]
    PlayerDrawing {
        player_id: Uuid,
        #[serde(skip)]
        word: String,
        #[serde(skip)]
        drawing: Vec<DrawingSegment>,
    },
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub id: Uuid,
    pub nickname: String,
}

/// Turn describes historic turn of the game.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Turn {
    word: String,
    player_guessed: Option<Player>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawingSegment {
    id: String,
    stroke: String,
    line_width: i32,
    points: Vec<Point>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Point {
    x: i32,
    y: i32,
}

impl Games {
    pub fn new() -> Self {
        Self {
            pending_ids: HashSet::new(),
            rooms: HashMap::new(),
        }
    }

    fn new_game_id() -> String {
        rand_str(6)
    }

    /// Reserve a game id
    pub fn reserve_id(&mut self) -> String {
        let id = Self::new_game_id();
        // TODO: verify there is no such id already in pending_ids and rooms
        self.pending_ids.insert(id.clone());
        id
    }

    /// Try to find a game by ID
    pub fn find_mut(&mut self, id: &str) -> Option<&mut Game> {
        self.rooms.get_mut(id)
    }

    pub fn find(&self, id: &str) -> Option<&Game> {
        self.rooms.get(id)
    }

    /// Return whether a game or pending game exists
    pub fn exists(&self, game_id: &str) -> bool {
        self.pending_ids.contains(game_id) || self.rooms.contains_key(game_id)
    }

    fn new_player(player_id: Uuid) -> Player {
        Player {
            id: player_id,
            nickname: rand_str(3),
        }
    }

    /// Adds a player to existing game or creates a game
    pub fn add_player(&mut self, game_id: &str, player_id: Uuid) -> (&Game, Player) {
        let player = Self::new_player(player_id);
        let game = self
            .rooms
            .entry(game_id.to_string())
            .and_modify(|game| {
                game.add_player(player.clone());
            })
            .or_insert_with(|| Game::new(game_id.to_string(), player.clone()));
        (game, player)
    }
}

/// Generate a random string
fn rand_str(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(Alphanumeric)
        .take(len)
        .collect()
}
