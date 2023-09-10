use cosmwasm_schema::cw_serde;
use cosmwasm_schema::QueryResponses;
use cosmwasm_std::{Addr, Decimal, Timestamp, Uint128};
use pyth_sdk_cw::Price;
use pyth_sdk_cw::PriceIdentifier;

pub const FEE_PRECISION: u128 = 100u128;

#[cw_serde]
pub enum Direction {
    Bull,
    Bear,
}

impl ToString for Direction {
    fn to_string(&self) -> String {
        match self {
            Direction::Bull => "bull",
            Direction::Bear => "bear",
        }
        .to_string()
    }
}

#[cw_serde]
/**
 * Parameters which are mutable by a governance vote
 */
pub struct Config {
    /* After a round ends this is the duration of the next */
    pub next_round_seconds: Uint128,
    pub minimum_bet: Uint128,
    pub gaming_fee: Uint128,
    //The token we are placing the bet with
    pub token_denom: String,
    //Address of the users contract where this contract will add the XP.
    pub users_contract: Addr,
    //Rewards for Users
    pub exp_per_denom_bet: u64,
    pub exp_per_denom_won: u64,
    pub dev_wallet_list: Vec<WalletInfo>,
}

#[cw_serde]
pub struct NextRound {
    pub id: Uint128,
    pub bid_time: Timestamp,
    pub open_time: Timestamp,
    pub close_time: Timestamp,
    pub bull_amount: Uint128,
    pub bear_amount: Uint128,
    pub denom: String,
}

#[cw_serde]
pub struct LiveRound {
    pub id: Uint128,
    pub bid_time: Timestamp,
    pub open_time: Timestamp,
    pub close_time: Timestamp,
    pub open_price: Price,
    pub bull_amount: Uint128,
    pub bear_amount: Uint128,
    pub denom: String,
}

#[cw_serde]
pub struct FinishedRound {
    pub id: Uint128,
    pub bid_time: Timestamp,
    pub open_time: Timestamp,
    pub close_time: Timestamp,
    pub open_price: Price,
    pub close_price: Price,
    pub winner: Option<Direction>,
    pub bull_amount: Uint128,
    pub bear_amount: Uint128,
    pub denom: String,
}

pub mod msg {
    use super::*;

    #[cw_serde]
    pub struct MigrateMsg {}

    #[cw_serde]
    pub struct InstantiateMsg {
        /* Mutable params */
        pub config: Config,
        pub oracle_addr: Option<Addr>,
        //What are we betting against
        pub bet_token_denoms: Vec<String>,
        pub identifier: Vec<IdentifierBet>,
    }

    #[cw_serde]
    pub struct IdentifierBet {
        pub denom: String,
        pub identifier: PriceIdentifier,
    }

    #[cw_serde]
    pub enum ExecuteMsg {
        /**
         * Update part of or all of the mutable config params
         */
        UpdateConfig {
            config: Config,
        },
        /**
         * Price go up
         */
        BetBull {
            /* In case the TX is delayed */
            round_id: Uint128,
            amount: Uint128,
        },
        /**
         * Price go down
         */
        BetBear {
            /* In case the TX is delayed */
            round_id: Uint128,
            amount: Uint128,
        },
        /**
         * Permissionless msg to close the current round and open the next
         * NOTE It is permissionless because we can check timestamps :)
         */
        CloseRound {},
        /**
         * Settle winnings for an account
         */
        CollectWinnings {},
        CollectionWinningRound {
            round_id: Uint128,
        },
        Halt {},
        Resume {},
        AddAdmin {
            new_admin: Addr,
        },
        RemoveAdmin {
            old_admin: Addr,
        },
        ModifyDevWallet {
            new_dev_wallets: Vec<WalletInfo>,
        },
        ModifyOracleAddress {
            address: Addr,
        },
        AddIdentifier {
            identifier: IdentifierBet,
        },
        ModifyBetArray {
            denoms: Vec<String>,
        },
    }

    #[cw_serde]
    #[derive(QueryResponses)]
    pub enum QueryMsg {
        #[returns(ConfigResponse)]
        Config {},
        #[returns(StatusResponse)]
        Status {},
        #[returns(MyCurrentPositionResponse)]
        MyCurrentPosition { address: String },
        #[returns(RoundResponse)]
        FinishedRound { round_id: Uint128 },
        #[returns(MyGameResponse)]
        MyGameList {
            player: Addr,
            start_after: Option<Uint128>,
            limit: Option<u32>,
        },
        #[returns(PendingRewardResponse)]
        MyPendingReward { player: Addr },
        #[returns(PendingRewardRoundsResponse)]
        MyPendingRewardRounds { player: Addr },
        #[returns(RoundUsersResponse)]
        GetUsersPerRound {
            round_id: Uint128,
            start_after: Option<Addr>,
            limit: Option<u32>,
        },
        #[returns(PendingRewardResponse)]
        MyPendingRewardRound { round_id: Uint128, player: Addr },
        #[returns(ClaimInfoResponse)]
        GetClaimInfoPerRound {
            round_id: Uint128,
            start_after: Option<Addr>,
            limit: Option<u32>,
        },
        #[returns(ClaimInfoResponse)]
        GetClaimInfoByUser {
            player: Addr,
            start_after: Option<Uint128>,
            limit: Option<u32>,
        },
        #[returns(TotalSpentResponse)]
        TotalSpent { player: Addr },
        #[returns(AdminsResponse)]
        GetAdmins {},
        #[returns(RoundDenomsResponse)]
        GetRoundDenoms {},
        #[returns(IdentifiersResponse)]
        GetIdentifiers {},
    }
}

pub type ConfigResponse = Config;

pub type RoundResponse = FinishedRound;

#[cw_serde]
pub struct StatusResponse {
    pub bidding_round: Option<NextRound>,
    pub live_round: Option<LiveRound>,
    pub current_time: Timestamp,
}

#[cw_serde]
pub struct MyCurrentPositionResponse {
    pub live_bear_amount: Uint128,
    pub live_bull_amount: Uint128,
    pub next_bear_amount: Uint128,
    pub next_bull_amount: Uint128,
}

#[cw_serde]
pub struct MyGameResponse {
    pub my_game_list: Vec<BetInfo>,
}

#[cw_serde]
pub struct RoundUsersResponse {
    pub round_users: Vec<BetInfo>,
}

#[cw_serde]
pub struct ClaimInfoResponse {
    pub claim_info: Vec<ClaimInfo>,
}

#[cw_serde]
pub struct PendingRewardResponse {
    pub pending_reward: Uint128,
}

#[cw_serde]
pub struct PendingRewardRoundsResponse {
    pub pending_reward_rounds: Vec<(Uint128, Uint128)>,
    pub pending_reward_total: Uint128,
}

#[cw_serde]
pub struct AdminsResponse {
    pub admins: Vec<Addr>,
}

#[cw_serde]
pub struct RoundDenomsResponse {
    pub denoms: Vec<String>,
}

#[cw_serde]
pub struct IdentifiersResponse {
    pub identifiers: Vec<PriceIdentifier>,
}

#[cw_serde]
pub struct WalletInfo {
    pub address: Addr,
    pub ratio: Decimal,
}

#[cw_serde]
pub struct TotalSpentResponse {
    pub total_spent: Uint128,
}

#[cw_serde]
pub struct ClaimInfo {
    pub player: Addr,
    pub round_id: Uint128,
    pub claimed_amount: Uint128,
}

/// Primary key for claiminfo: (round_id, player)
pub type ClaimInfoKey = (u128, Addr);

#[cw_serde]
pub struct BetInfo {
    pub player: Addr,
    pub round_id: Uint128,
    pub amount: Uint128,
    pub direction: Direction,
}

/// Primary key for betinfo: (round_id, player)
pub type BetInfoKey = (u128, Addr);
