use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Empty};
use cw_storage_plus::{Item, Map};
use prediction::prediction_game::WalletInfo;

/// Top level storage key. Values must not conflict.
/// Each key is only one byte long to ensure we use the smallest possible storage keys.
#[repr(u8)]
pub enum TopKey {
    Config = b'a',
    Games = b'b',
    GamesCodeId = b'c',
    DevWallets = b'd',
}

impl TopKey {
    const fn as_str(&self) -> &str {
        let array_ref = unsafe { std::mem::transmute::<&TopKey, &[u8; 1]>(self) };
        match core::str::from_utf8(array_ref) {
            Ok(a) => a,
            Err(_) => panic!("Non-utf8 enum value found. Use a-z, A-Z and 0-9"),
        }
    }
}

#[cw_serde]
pub struct Config {
    pub users_code_id: u64,
    pub users_contract: Addr,
    pub games_code_id: u64,
    pub dev_wallet_list: Vec<WalletInfo>,
}

pub const CONFIG: Item<Config> = Item::new(TopKey::Config.as_str());
pub const GAMES: Map<Addr, Empty> = Map::new(TopKey::Games.as_str());
