use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_ownable::{cw_ownable_execute, cw_ownable_query};
use dao_bets::dao_bets::{Bet, BetInfo, BetOption, Config};

#[cw_serde]
pub struct InstantiateMsg {
    pub config: Config,
    pub owner: Option<Addr>,
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    UpdateConfig {
        config: Config,
    },
    BetOn {
        bet_id: Uint128,
        option: String,
    },
    /**
     * Settle winnings for an account
     */
    CollectWinnings {},
    CollectionWinningBet {
        bet_id: Uint128,
    },
    // Owner actions
    CreateBet {
        topic: String,
        description: String,
        img_url: Option<String>,
        end_bet_timestamp: u64,
        expected_result_timestamp: Option<u64>,
        options: Vec<BetOption>,
    },
    ModifyBet {
        bet_id: Uint128,
        topic: Option<String>,
        description: Option<String>,
        end_bet_timestamp: Option<u64>,
        expected_result_timestamp: Option<u64>,
        img_url: Option<String>,
    },
    CompleteBet {
        bet_id: Uint128,
        result_option: String,
    },
    // Owner action to cancel a bet if it can't be settled or something was wrong so all people who bet can get their money back
    CancelBet {
        bet_id: Uint128,
    },
}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    Config {},
    #[returns(Vec<BetInfo>)]
    MyCurrentBets {
        player: Addr,
        start_after: Option<Uint128>,
        limit: Option<u32>,
    },
    #[returns(Bet)]
    UnfinishedBetInfo { bet_id: Uint128 },
    #[returns(Bet)]
    FinishedBetInfo { bet_id: Uint128 },
    #[returns(Vec<Bet>)]
    UnfinishedBets {
        start_after: Option<Uint128>,
        limit: Option<u32>,
    },
    #[returns(Vec<Bet>)]
    FinishedBets {
        start_after: Option<Uint128>,
        limit: Option<u32>,
    },
    #[returns(Vec<Bet>)]
    UnfinishedBetsByTopic {
        topic: String,
        start_after: Option<Uint128>,
        limit: Option<u32>,
    },
    #[returns(Vec<Bet>)]
    FinishedBetsByTopic {
        topic: String,
        start_after: Option<Uint128>,
        limit: Option<u32>,
    },
    #[returns(Uint128)]
    MyPendingReward { player: Addr },
    #[returns(PendingRewardRoundsResponse)]
    MyPendingRewardRounds { player: Addr },
    #[returns(Uint128)]
    MyPendingRewardRound { round_id: Uint128, player: Addr },
    #[returns(Vec<BetInfo>)]
    GetUsersPerRound {
        round_id: Uint128,
        start_after: Option<Addr>,
        limit: Option<u32>,
    },
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
    #[returns(Uint128)]
    TotalSpent { player: Addr },
}

#[cw_serde]
pub struct MyBetsResponse {
    pub my_bets_list: Vec<BetInfo>,
}

#[cw_serde]
pub struct PendingRewardRoundsResponse {
    pub pending_reward_rounds: Vec<(Uint128, Uint128)>,
    pub pending_reward_total: Uint128,
}

#[cw_serde]
pub struct ClaimInfoResponse {
    pub claim_info: Vec<ClaimInfo>,
}

#[cw_serde]
pub struct ClaimInfo {
    pub player: Addr,
    pub round_id: Uint128,
    pub claimed_amount: Uint128,
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct WalletInfo {
    pub address: Addr,
    pub ratio: Decimal,
}
