use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Timestamp};

#[cw_serde]
pub struct InstantiateMsg {
    pub config: Config,
    pub extra_admins: Option<Vec<Addr>>,
}

#[allow(clippy::large_enum_variant)]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateConfig {
        config: Config,
    },
    AddGame {
        address: Addr,
    },
    AddGames {
        addresses: Vec<Addr>,
    },
    RemoveGame {
        address: Addr,
    },
    ModifyUser {
        user: User,
    },
    ModifyVerification {
        username: String,
        is_verified: bool,
    },
    ResetElo {
        elo_substraction: Option<u64>,
    },
    AddExperienceAndElo {
        user: Addr,
        experience: u64,
        elo: Option<Elo>,
    },
    AddAdmin {
        new_admin: Addr,
    },
    RemoveAdmin {
        old_admin: Addr,
    },
    AddEvent {
        event_name: String,
        start_timestamp: u64,
        end_timestamp: u64,
        games: Option<Vec<Addr>>,
    },
    AddGameToEvent {
        event_name: String,
        game_address: Addr,
    },
}

#[cw_serde]
pub struct User {
    pub address: Option<Addr>,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub country: Option<String>,
    pub image_url: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub website: Option<String>,
    pub socials: Option<Vec<SocialMedia>>,
    pub experience: Option<u64>,
    pub elo: Option<u64>,
    pub creation_date: Option<Timestamp>,
    pub is_verified: Option<bool>,
}

#[cw_serde]
pub struct Config {
    // How much exp is needed to get from lvl 0 to lvl 1
    pub initial_exp_per_level: u64,
    // Increase needed per level
    // If initial_exp_per_level is 100 and exp_increase_per_level is 10, then the exp needed to get from lvl 1 to lvl 2 is 110
    pub exp_increase_per_level: u64,
}

#[cw_serde]
pub struct Elo {
    // Elo to modify
    pub amount: u64,
    // If true we add, if negative we substract
    pub add: bool,
}

#[cw_serde]
pub enum SocialMedia {
    Twitter(String),
    Instagram(String),
    Telegram(String),
    Discord(String),
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(User)]
    UserByAddress { address: Addr },
    #[returns(User)]
    UserByUsername { username: String },
    #[returns(u128)]
    TotalUsers {},
    #[returns(Config)]
    Config {},
    #[returns(bool)]
    GameRegistered { game_address: Addr },
    #[returns(Vec<User>)]
    Users {
        start_after: Option<Addr>,
        limit: Option<u32>,
    },
    #[returns(Vec<Addr>)]
    Admins {},
    #[returns(Vec<EventInfo>)]
    OngoingEvents {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(Vec<EventInfo>)]
    FinishedEvents {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    #[returns(bool)]
    Participated { user: Addr, event_name: String },
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct EventInfo {
    pub name: String,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub games: Option<Vec<Addr>>,
}

#[cw_serde]
pub enum Activity {
    Participated,
}
