use serde::{Deserialize, Serialize};

pub mod account;
pub mod app_error;
#[cfg(test)]
mod test;

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
    pub active: bool,
    #[serde(rename = "universeId")]
    pub universe_id: u64,
    #[serde(rename = "placeId")]
    pub place_id: u64,
    pub name: String,
    #[serde(rename = "ownerId")]
    pub owner_id: u64,
    #[serde(rename = "ownerName")]
    pub owner_name: String,
    #[serde(rename = "priceInRobux")]
    pub price_in_robux: u32,
    #[serde(rename = "privateServerId")]
    pub private_server_id: u64,
    #[serde(rename = "expirationDate")]
    pub expiration_date: String,
    #[serde(rename = "willRenew")]
    pub will_renew: bool,
    #[serde(rename = "universeName")]
    pub universe_name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateServerDetails {
    pub id: u64,
    pub name: String,
    pub game: PrivateServerGame,
    #[serde(rename = "joinCode")]
    pub join_code: String,
    pub active: bool,
    pub subscription: Subscription,
    pub permissions: Permissions,
    #[serde(rename = "voiceSettings")]
    pub voice_settings: VoiceSettings,
    pub link: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateServerGame {
    pub id: u64,
    pub name: String,
    #[serde(rename = "rootPlace")]
    pub root_place: RootPlace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootPlace {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub active: bool,
    pub expired: bool,
    #[serde(rename = "expirationDate")]
    pub expiration_date: String,
    pub price: u32,
    #[serde(rename = "canRenew")]
    pub can_renew: bool,
    #[serde(rename = "hasInsufficientFunds")]
    pub has_insufficient_funds: bool,
    #[serde(rename = "hasRecurringProfile")]
    pub has_recurring_profile: bool,
    #[serde(rename = "hasPriceChanged")]
    pub has_price_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permissions {
    #[serde(rename = "clanAllowed")]
    pub clan_allowed: bool,
    #[serde(rename = "enemyClanId")]
    pub enemy_clan_id: Option<u64>,
    #[serde(rename = "friendsAllowed")]
    pub friends_allowed: bool,

    pub users: Vec<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceSettings {
    pub enabled: bool,
}
