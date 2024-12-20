use cosmwasm_std::{Addr, Empty};
use cw_storage_plus::{Item, Map};
use general::users::{Config, EventInfo, User};

/// Top level storage key. Values must not conflict.
/// Each key is only one byte long to ensure we use the smallest possible storage keys.
#[repr(u8)]
pub enum TopKey {
    NumUsers = b'a',
    Config = b'b',
    AddressToUser = b'c',
    UsernameToUser = b'd',
    // Contracts allowed to modify users info
    GameContracts = b'e',
    Admins = b'f',
    OngoingEvents = b'g',
    FinishedEvents = b'h',
    ParticipatedEvents = b'i',
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

pub const NUM_USERS: Item<u128> = Item::new(TopKey::NumUsers.as_str());
pub const ADDRESS_TO_USER: Map<Addr, User> = Map::new(TopKey::AddressToUser.as_str());
pub const USERNAME_TO_USER: Map<String, User> = Map::new(TopKey::UsernameToUser.as_str());
pub const GAME_CONTRACTS: Map<Addr, Empty> = Map::new(TopKey::GameContracts.as_str());
pub const CONFIG: Item<Config> = Item::new(TopKey::Config.as_str());
pub const ADMINS: Item<Vec<Addr>> = Item::new(TopKey::Admins.as_str());
pub const ONGOING_EVENTS: Map<String, EventInfo> = Map::new(TopKey::OngoingEvents.as_str());
pub const FINISHED_EVENTS: Map<String, EventInfo> = Map::new(TopKey::FinishedEvents.as_str());
pub const PARTICIPATED_EVENTS: Map<(Addr, String), Empty> =
    Map::new(TopKey::ParticipatedEvents.as_str());
