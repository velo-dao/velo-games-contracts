use std::collections::HashMap;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Uint128};

#[cw_serde]
pub struct Config {
    pub minimum_bet: Uint128,
    pub gaming_fee: Uint128,
    // The token we are placing the bet with
    pub token_denom: String,
    // Address of the users contract where this contract will add the XP.
    pub users_contract: Addr,
    // Rewards for Users
    pub exp_per_denom_bet: u64,
    pub exp_per_denom_won: u64,
    pub fee_receiver_wallet_list: Vec<WalletInfo>,
}

#[cw_serde]
pub struct WalletInfo {
    pub address: Addr,
    pub ratio: Decimal,
}

/// Primary key for betinfo: (round_id, player)
pub type BetInfoKey = (u128, Addr);
/// Primary key for claiminfo: (round_id, player)
pub type ClaimInfoKey = (u128, Addr);

#[cw_serde]
pub struct BetInfo {
    pub player: Addr,
    pub bet_id: Uint128,
    pub amount: Uint128,
    pub option: String,
}

#[cw_serde]
pub struct ClaimInfo {
    pub player: Addr,
    pub bet_id: Uint128,
    pub claimed_amount: Uint128,
}

#[cw_serde]
pub struct Bet {
    pub bet_id: u128,
    pub topic: String,
    pub description: String,
    pub rules: Option<String>,
    pub img_url: Option<String>,
    pub end_bet_timestamp: u64,
    pub expected_result_timestamp: Option<u64>,
    pub options: Vec<BetOption>,
    pub current_bet_amounts: HashMap<String, Uint128>,
    pub result_option: Option<String>,
    pub cancelled: bool,
    pub num_players: u64,
}

#[cw_serde]
pub struct BetOption {
    pub title: String,
    pub img_url: Option<String>,
}
