use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};
use general::users::Config as UsersConfig;
use prediction::prediction_game::{msg::IdentifierBet, WalletInfo};

use crate::state::Config;

#[cw_serde]
pub struct InstantiateMsg {
    pub users_code_id: u64,
    pub users_config: UsersConfig,
    pub games_code_id: u64,
    pub dev_wallet_list: Vec<WalletInfo>,
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    CreateGame {
        next_round_seconds: Uint128,
        minimum_bet: Uint128,
        gaming_fee: Uint128,
        token_denom: String,
        exp_per_denom_bet: u64,
        exp_per_denom_won: u64,
        oracle_addr: Option<Addr>,
        bet_token_denoms: Vec<String>,
        identifiers: Vec<IdentifierBet>,
        label: String,
    },
    ModifyDevWallets {
        wallets: Vec<WalletInfo>,
        update_all_games: bool,
    },
    UpdateCodeIds {
        users_code_id: u64,
        games_code_id: u64,
    },

    UpdateUsersContract {
        address: Addr,
        update_all_games: bool,
        add_all_games_to_users_contract: bool,
    },

    UpdateOracleForAllGames {
        oracle_addr: Addr,
    },

    HaltAllGames {},
    ResumeAllGames {},

    ManuallyAddGame {
        address: Addr,
        add_to_users_contract: bool,
    },

    ManuallyRemoveGame {
        address: Addr,
        remove_from_users_contract: bool,
    },
}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    Config {},
    #[returns(Vec<Addr>)]
    Games {
        start_after: Option<Addr>,
        limit: Option<u32>,
    },
    #[returns(Vec<GameInfo>)]
    GamesInfo {
        start_after: Option<Addr>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub struct GameInfo {
    pub address: Addr,
    pub next_round_seconds: Uint128,
}

#[cw_serde]
pub struct MigrateMsg {}
