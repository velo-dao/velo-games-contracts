use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, Map, MultiIndex};
use prediction::prediction_game::{BetInfo, BetInfoKey, ClaimInfo, ClaimInfoKey};
use prediction::prediction_game::{Config, FinishedRound, LiveRound, NextRound};
use pyth_sdk_cw::PriceIdentifier;

/// Top level storage key. Values must not conflict.
/// Each key is only one byte long to ensure we use the smallest possible storage keys.
#[repr(u8)]
pub enum TopKey {
    IsHalted = b'I',
    Config = b'C',
    NextRoundId = b'n',
    NextRound = b'N',
    LiveRound = b'L',
    Rounds = b'r',
    Admins = b'a',
    TotalsSpent = b't',
    Oracle = b'o',
    PriceIdentifiers = b'p',
    RoundDenoms = b'd',
}

impl TopKey {
    const fn as_str(&self) -> &str {
        let array_ref = unsafe { std::mem::transmute::<_, &[u8; 1]>(self) };
        match core::str::from_utf8(array_ref) {
            Ok(a) => a,
            Err(_) => panic!("Non-utf8 enum value found. Use a-z, A-Z and 0-9"),
        }
    }
}

pub const IS_HALTED: Item<bool> = Item::new(TopKey::IsHalted.as_str());
pub const CONFIG: Item<Config> = Item::new(TopKey::Config.as_str());
pub const NEXT_ROUND_ID: Item<u128> = Item::new(TopKey::NextRoundId.as_str());
/* The round that's open for betting */
pub const NEXT_ROUND: Item<NextRound> = Item::new(TopKey::NextRound.as_str());
/* The live round; not accepting bets */
pub const LIVE_ROUND: Item<LiveRound> = Item::new(TopKey::LiveRound.as_str());

pub const ROUNDS: Map<u128, FinishedRound> = Map::new(TopKey::Rounds.as_str());

pub const ADMINS: Item<Vec<Addr>> = Item::new(TopKey::Admins.as_str());

pub const TOTALS_SPENT: Map<Addr, Uint128> = Map::new(TopKey::TotalsSpent.as_str());

pub const ORACLE: Item<Addr> = Item::new(TopKey::Oracle.as_str());

pub const PRICE_IDENTIFIERS: Map<String, PriceIdentifier> =
    Map::new(TopKey::PriceIdentifiers.as_str());

pub const ROUND_DENOMS: Item<Vec<String>> = Item::new(TopKey::RoundDenoms.as_str());

/// Convenience bid key constructor
pub fn bet_info_key(round_id: u128, player: &Addr) -> BetInfoKey {
    (round_id, player.clone())
}

/// Defines incides for accessing bids
pub struct BetInfoIndexes<'a> {
    pub player: MultiIndex<'a, Addr, BetInfo, BetInfoKey>,
    pub round_id: MultiIndex<'a, u128, BetInfo, BetInfoKey>,
}

impl<'a> IndexList<BetInfo> for BetInfoIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<BetInfo>> + '_> {
        let v: Vec<&dyn Index<BetInfo>> = vec![&self.player, &self.round_id];
        Box::new(v.into_iter())
    }
}

pub fn bet_info_storage<'a>() -> IndexedMap<'a, BetInfoKey, BetInfo, BetInfoIndexes<'a>> {
    let indexes = BetInfoIndexes {
        player: MultiIndex::new(
            |_pk: &[u8], d: &BetInfo| d.player.clone(),
            "bet_info",
            "bet_info_collection",
        ),
        round_id: MultiIndex::new(
            |_pk: &[u8], d: &BetInfo| d.round_id.u128(),
            "bet_info",
            "round_id",
        ),
    };
    IndexedMap::new("bet_info", indexes)
}
/// Convenience bid key constructor
pub fn claim_info_key(round_id: u128, player: &Addr) -> ClaimInfoKey {
    (round_id, player.clone())
}

/// Defines incides for accessing bids
pub struct ClaimInfoIndexes<'a> {
    pub player: MultiIndex<'a, Addr, ClaimInfo, ClaimInfoKey>,
    pub round_id: MultiIndex<'a, u128, ClaimInfo, ClaimInfoKey>,
}

impl<'a> IndexList<ClaimInfo> for ClaimInfoIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<ClaimInfo>> + '_> {
        let v: Vec<&dyn Index<ClaimInfo>> = vec![&self.player, &self.round_id];
        Box::new(v.into_iter())
    }
}

pub fn claim_info_storage<'a>() -> IndexedMap<'a, ClaimInfoKey, ClaimInfo, ClaimInfoIndexes<'a>> {
    let indexes = ClaimInfoIndexes {
        player: MultiIndex::new(
            |_pk: &[u8], d: &ClaimInfo| d.player.clone(),
            "claim_info",
            "claim_info_collection",
        ),
        round_id: MultiIndex::new(
            |_pk: &[u8], d: &ClaimInfo| d.round_id.u128(),
            "claim_info",
            "claim_round_id",
        ),
    };
    IndexedMap::new("claim_info", indexes)
}
