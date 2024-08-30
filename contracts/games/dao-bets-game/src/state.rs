use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, Map, MultiIndex};
use dao_bets::dao_bets::{Bet, BetInfo, BetInfoKey, ClaimInfo, ClaimInfoKey, Config};

/// Top level storage key. Values must not conflict.
/// Each key is only one byte long to ensure we use the smallest possible storage keys.
#[repr(u8)]
pub enum TopKey {
    Config = b'0',
    NextBetId = b'1',
    UnfinishedBets = b'2',
    FinishedBets = b'3',
    TotalsSpent = b'4',
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

pub const CONFIG: Item<Config> = Item::new(TopKey::Config.as_str());
pub const NEXT_BET_ID: Item<u128> = Item::new(TopKey::NextBetId.as_str());
// Bets where result wasn't submitted by the DAO yet
pub struct BetIndexes<'a> {
    pub topic: MultiIndex<'a, String, Bet, u128>,
}

impl<'a> IndexList<Bet> for BetIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Bet>> + '_> {
        let v: Vec<&dyn Index<Bet>> = vec![&self.topic];
        Box::new(v.into_iter())
    }
}

pub const UNFINISHED_BETS: IndexedMap<u128, Bet, BetIndexes> = IndexedMap::new(
    TopKey::UnfinishedBets.as_str(),
    BetIndexes {
        topic: MultiIndex::new(
            |_pk, bet| bet.topic.clone(),
            TopKey::UnfinishedBets.as_str(),
            "unfinished_bet__topic",
        ),
    },
);

pub const FINISHED_BETS: IndexedMap<u128, Bet, BetIndexes> = IndexedMap::new(
    TopKey::FinishedBets.as_str(),
    BetIndexes {
        topic: MultiIndex::new(
            |_pk, bet| bet.topic.clone(),
            TopKey::FinishedBets.as_str(),
            "finished_bet__topic",
        ),
    },
);

pub const TOTALS_SPENT: Map<Addr, Uint128> = Map::new(TopKey::TotalsSpent.as_str());

/// Defines indexes for accessing bids
pub struct BetInfoIndexes<'a> {
    pub player: MultiIndex<'a, Addr, BetInfo, BetInfoKey>,
    pub bet_id: MultiIndex<'a, u128, BetInfo, BetInfoKey>,
}

impl<'a> IndexList<BetInfo> for BetInfoIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<BetInfo>> + '_> {
        let v: Vec<&dyn Index<BetInfo>> = vec![&self.player, &self.bet_id];
        Box::new(v.into_iter())
    }
}

pub fn bet_info_storage<'a>() -> IndexedMap<BetInfoKey, BetInfo, BetInfoIndexes<'a>> {
    let indexes = BetInfoIndexes {
        player: MultiIndex::new(
            |_pk: &[u8], d: &BetInfo| d.player.clone(),
            "bet_info",
            "bet_info_collection",
        ),
        bet_id: MultiIndex::new(
            |_pk: &[u8], d: &BetInfo| d.bet_id.u128(),
            "bet_info",
            "bet_id",
        ),
    };
    IndexedMap::new("bet_info", indexes)
}

/// Convenience bid key constructor
pub fn bet_info_key(bet_id: u128, player: &Addr) -> BetInfoKey {
    (bet_id, player.clone())
}

/// Convenience bid key constructor
pub fn claim_info_key(bet_id: u128, player: &Addr) -> ClaimInfoKey {
    (bet_id, player.clone())
}

/// Defines incides for accessing bids
pub struct ClaimInfoIndexes<'a> {
    pub player: MultiIndex<'a, Addr, ClaimInfo, ClaimInfoKey>,
    pub bet_id: MultiIndex<'a, u128, ClaimInfo, ClaimInfoKey>,
}

impl<'a> IndexList<ClaimInfo> for ClaimInfoIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<ClaimInfo>> + '_> {
        let v: Vec<&dyn Index<ClaimInfo>> = vec![&self.player, &self.bet_id];
        Box::new(v.into_iter())
    }
}

pub fn claim_info_storage<'a>() -> IndexedMap<ClaimInfoKey, ClaimInfo, ClaimInfoIndexes<'a>> {
    let indexes = ClaimInfoIndexes {
        player: MultiIndex::new(
            |_pk: &[u8], d: &ClaimInfo| d.player.clone(),
            "claim_info",
            "claim_info_collection",
        ),
        bet_id: MultiIndex::new(
            |_pk: &[u8], d: &ClaimInfo| d.bet_id.u128(),
            "claim_info",
            "claim_bet_id",
        ),
    };
    IndexedMap::new("claim_info", indexes)
}
