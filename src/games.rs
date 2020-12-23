use std::collections::{HashMap, HashSet};

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
    pub id: String,
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
        let existing = self.players.iter().find(|p| p.id == player.id);
        if existing.is_none() {
            self.players.push(player);
        }
    }

    /// Remove player from the game. If player is currently drawing or choosing the word then pick another player to do that.
    /// Return true if player was present in the game
    fn remove_player(&mut self, remove_player_id: &Uuid) -> bool {
        let pos = self.players.iter().position(|p| &p.id == remove_player_id);

        if pos.is_some() {
            self.players.remove(pos.expect("Player index"));
        }

        // If there is no more players left then we are done
        if self.players.is_empty() {
            return pos.is_some();
        }

        // Pick next player
        match self.stage {
            GameStage::PlayerChoosing { player_id } if &player_id == remove_player_id => {
                self.stage = GameStage::PlayerChoosing {
                    player_id: self.players[0].id.clone(),
                };
            }
            GameStage::PlayerDrawing { player_id, .. } if &player_id == remove_player_id => {
                self.stage = GameStage::PlayerChoosing {
                    player_id: self.players[0].id.clone(),
                };
            }
            _ => {}
        };
        pos.is_some()
    }

    /// Add drawing segment if we are in drawing stage
    pub fn add_segment(&mut self, segment: DrawingSegment) {
        if let GameStage::PlayerDrawing {
            ref mut drawing, ..
        } = &mut self.stage
        {
            drawing.segments.push(segment);
        }
    }

    /// Remove drawing segment if we are in drawing stage
    pub fn remove_segment(&mut self, segment_id: &str) {
        if let GameStage::PlayerDrawing {
            ref mut drawing, ..
        } = &mut self.stage
        {
            drawing.segments.retain(|s| s.id != segment_id);
        }
    }

    /// Submit a word to draw. Transitions to drawing stage if this player was allowed to do that.
    /// Return true if transitioned.
    pub fn submit_word(&mut self, submitting_player_id: &Uuid, word: String, canvas: CanvasSize) -> bool {
        match self.stage {
            GameStage::PlayerChoosing { player_id } if submitting_player_id == &player_id => {
                // continue
                self.stage = GameStage::PlayerDrawing {
                    player_id,
                    word: word.trim().to_string(),
                    drawing: Drawing {
                        canvas,
                        segments: vec![],
                    },
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
            GameStage::PlayerDrawing { word, .. }
                if word.to_lowercase() == guess.to_lowercase() =>
            {
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
            for segment in &drawing.segments {
                cb(segment);
            }
        }
    }
}

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
        drawing: Drawing,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drawing {
    pub canvas: CanvasSize,
    #[serde(skip)]
    pub segments: Vec<DrawingSegment>,
}

// Implement custom Clone to skip cloning segments
impl Clone for Drawing {
    fn clone(&self) -> Self {
        Self {
            canvas: self.canvas.clone(),
            segments: vec![],
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CanvasSize {
    pub width: u32,
    pub height: u32,
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

    /// Reserve a game id
    pub fn reserve_id(&mut self) -> String {
        let mut len = 6;
        let id = loop {
            // Generate unique game ID
            let id = rand_str(len);
            if !self.exists(&id) {
                // Unique
                break id;
            }
            len += 1;
        };

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

    /// Remove player from all games. Return a list of modified games.
    pub fn remove_player(&mut self, player_id: &Uuid) -> Vec<Game> {
        let mut empty_games = vec![];
        let mut modified_games = vec![];

        {
            // Remove players
            for game in self.rooms.values_mut() {
                let modified = game.remove_player(player_id);
                if game.players.is_empty() {
                    empty_games.push(game.id.clone());
                } else if modified {
                    modified_games.push(game.clone());
                }
            }
        }

        // Remove empty rooms
        for game_id in empty_games {
            log::info!("Removing empty game {}", game_id);
            self.rooms.remove(&game_id);
        }

        modified_games
    }
}

/// Generate a random string
fn rand_str(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(Alphanumeric)
        .take(len)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn games_reserve_id() {
        let mut games = Games::new();
        assert!(games.reserve_id().len() > 0);
        assert_eq!(1, games.pending_ids.len(), "pending_ids.len()");
    }

    #[test]
    fn games_lifecycle() {
        let mut games = Games::new();
        let game_id = "test".to_string();
        let player_id = Uuid::new_v4();
        let player_id_2 = Uuid::new_v4();
        let word = "Apple".to_string();
        let canvas = CanvasSize { width: 100, height: 100, };

        {
            // Create a game
            let (game, player) = games.add_player(&game_id, player_id.clone());
            assert_eq!(player_id, player.id, "player id");
            assert_eq!(1, game.players.len(), "players in the game");
            match game.stage {
                GameStage::PlayerChoosing { player_id: p_id } => {
                    assert_eq!(player_id, p_id, "player id in stage");
                }
                _ => {
                    panic!("Expected PlayerChoosing game stage");
                }
            };
        }

        {
            // Add another player
            let (game, _) = games.add_player(&game_id, player_id_2.clone());
            assert_eq!(2, game.players.len(), "players in the game");
        }

        {
            // Submit word as wrong player
            let game = games.find_mut(&game_id);
            assert!(game.is_some());
            let res = game.unwrap().submit_word(&player_id_2, word.clone(), canvas.clone());
            assert_eq!(false, res);
        }

        {
            // Submit word
            let game = games.find_mut(&game_id);
            assert!(game.is_some());
            let game = game.unwrap();
            let res = game.submit_word(&player_id, word.clone(), canvas.clone());
            assert!(res);
            match game.stage {
                GameStage::PlayerDrawing {
                    player_id: p_id, ..
                } => {
                    assert_eq!(player_id, p_id, "player id in stage");
                }
                _ => {
                    panic!("Expected PlayerDrawing game stage");
                }
            };
        }

        {
            // Guess word incorrectly
            let game = games.find_mut(&game_id);
            assert!(game.is_some());
            let res = game.unwrap().guess_word(&player_id_2, "wrong");
            assert_eq!(false, res);
        }

        {
            // Guess word
            let game = games.find_mut(&game_id);
            assert!(game.is_some());
            let game = game.unwrap();
            let res = game.guess_word(&player_id_2, &word);
            assert!(res);
            match game.stage {
                GameStage::PlayerChoosing {
                    player_id: p_id, ..
                } => {
                    assert_eq!(player_id_2, p_id, "player id in stage");
                }
                _ => {
                    panic!("Expected PlayerChoosing game stage");
                }
            };
        }

        {
            // Remove player from the game
            let modified_games = games.remove_player(&player_id_2);
            assert_eq!(1, modified_games.len(), "modified games len");
            let game = &modified_games[0];
            assert_eq!(1, game.players.len(), "modified game players");
            assert_eq!(player_id, game.players[0].id, "remaining player");
            match game.stage {
                GameStage::PlayerChoosing {
                    player_id: p_id, ..
                } => {
                    assert_eq!(player_id, p_id, "player id in stage");
                }
                _ => {
                    panic!("Expected PlayerChoosing game stage");
                }
            };
        }

        {
            // Remove last player
            let modified_games = games.remove_player(&player_id);
            assert_eq!(0, modified_games.len(), "modified games len");
            assert_eq!(0, games.rooms.len(), "no more games");
        }
    }
}
