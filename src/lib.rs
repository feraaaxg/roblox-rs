use serde::{Deserialize, Serialize};

pub mod account;
pub mod app_error;

#[derive(Debug, Clone)]
pub struct Badge {
    pub id: u64,
    pub name: String,
    pub typed: String,
    pub display_name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct Game {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub creator_id: u64,
    pub creator_name: String,
    pub creator_type: String,
    pub root_place_id: u64,
    pub place_visits: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GamePass {
    pub game_pass_id: u64,
    pub name: String,
    pub description: String,
    pub is_for_sale: bool,
    pub price: Option<u32>,
    pub creator_id: u64,
    pub creator_name: String,
    pub creator_type: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateServer {
    pub id: u64,
    pub name: Option<String>,
    pub universe_id: u64,
    pub place_id: u64,
    pub active: bool,
    pub subscription_status: String,
    pub max_players: u32,
    pub price: Option<u32>,
    pub link_code: String,
    pub link: String,
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub display_name: String,
}
