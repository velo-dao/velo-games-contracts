use std::cmp::Ordering;
use std::vec;

use crate::error::ContractError;
use crate::state::{
    bet_info_key, bet_info_storage, claim_info_key, claim_info_storage, ADMINS, CONFIG, IS_HALTED,
    LIVE_ROUND, NEXT_ROUND, NEXT_ROUND_ID, PRICE_TICKERS, ROUNDS, ROUND_DENOMS, TOTALS_SPENT,
};
use chrono::DateTime;

use cw_utils::one_coin;
use neutron_sdk::bindings::oracle::query::{GetPriceResponse, OracleQuery};
use neutron_sdk::bindings::oracle::types::CurrencyPair;
use neutron_sdk::bindings::query::NeutronQuery;
use prediction::prediction_game::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use prediction::prediction_game::{
    AdminsResponse, BetInfo, ClaimInfo, ClaimInfoResponse, ConfigResponse, MyGameResponse,
    PendingRefundableAmountResponse, PendingRefundableAmountRoundsResponse, PendingRewardResponse,
    PendingRewardRoundsResponse, RoundDenomsResponse, RoundUsersResponse, TickersResponse,
    TotalSpentResponse, WalletInfo,
};
use prediction::prediction_game::{Config, Direction};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_json_binary, Addr, BankMsg, Binary, Decimal, Deps, DepsMut, Env, Event, Int128,
    MessageInfo, Order, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw_storage_plus::Bound;
use general::users::ExecuteMsg::AddExperienceAndElo;
use prediction::prediction_game::{FinishedRound, LiveRound, NextRound};
use prediction::prediction_game::{MyCurrentPositionResponse, StatusResponse};

const FEE_PRECISION: u128 = 100;

// Pagination info for queries
const MAX_PAGE_LIMIT: u32 = 250;

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const USD_TICKER: &str = "USD";
const PRICE_DECIMALS: u64 = 18;
const MAX_OLD_PRICE_TIME: u64 = 10;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    if !msg.config.dev_wallet_list.is_empty() {
        let mut total_ratio = Decimal::zero();
        for dev_wallet in msg.config.dev_wallet_list.clone() {
            total_ratio += dev_wallet.ratio;
        }

        if total_ratio != Decimal::one() {
            return Err(ContractError::WrongRatio {});
        }
    }

    CONFIG.save(deps.storage, &msg.config)?;
    NEXT_ROUND_ID.save(deps.storage, &0u128)?;
    IS_HALTED.save(deps.storage, &false)?;
    let mut admins = vec![info.sender];
    if let Some(admins_list) = msg.extra_admins {
        for admin in admins_list {
            deps.api.addr_validate(admin.as_str())?;
            admins.push(admin);
        }
    }

    ADMINS.save(deps.storage, &admins)?;

    if msg.denom_tickers.is_empty() {
        return Err(ContractError::DenomsEmpty {});
    }

    for each_denom_ticker in msg.denom_tickers.iter() {
        PRICE_TICKERS.save(
            deps.storage,
            each_denom_ticker.denom.clone(),
            &each_denom_ticker.ticker.clone(),
        )?;
    }

    let bet_token_denoms = msg.denom_tickers.iter().map(|x| x.denom.clone()).collect();

    ROUND_DENOMS.save(deps.storage, &bet_token_denoms)?;

    Ok(Response::new().add_attribute("velo_method", "instantiate_prediction_game"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, MigrateMsg {}: MigrateMsg) -> StdResult<Response> {
    let version = cw2::get_contract_version(deps.storage)?;
    if version.contract != CONTRACT_NAME {
        return Err(StdError::generic_err("Can only upgrade from same type"));
    }
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut<NeutronQuery>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig { config } => {
            execute_update_config(deps.into_empty(), info, config)
        }
        ExecuteMsg::BetBear { round_id, amount } => execute_bet(
            deps.into_empty(),
            info,
            env,
            round_id,
            Direction::Bear,
            amount,
        ),
        ExecuteMsg::BetBull { round_id, amount } => execute_bet(
            deps.into_empty(),
            info,
            env,
            round_id,
            Direction::Bull,
            amount,
        ),
        ExecuteMsg::CloseRound {} => execute_close_round(deps, env),
        ExecuteMsg::CollectWinnings {} => execute_collect_winnings(deps.into_empty(), info),
        ExecuteMsg::CollectionWinningRound { round_id } => {
            execute_collect_winning_round(deps.into_empty(), info, round_id)
        }
        ExecuteMsg::Halt {} => execute_update_halt(deps.into_empty(), info, true),
        ExecuteMsg::Resume {} => execute_update_halt(deps.into_empty(), info, false),
        ExecuteMsg::AddAdmin { new_admin } => execute_add_admin(deps.into_empty(), info, new_admin),
        ExecuteMsg::RemoveAdmin { old_admin } => {
            execute_remove_admin(deps.into_empty(), info, old_admin)
        }
        ExecuteMsg::ModifyDevWallet { new_dev_wallets } => {
            execute_modify_dev_wallets(deps.into_empty(), info, new_dev_wallets)
        }
        ExecuteMsg::AddTicker { denom, ticker } => {
            execute_add_ticker(deps.into_empty(), info, denom, ticker)
        }
        ExecuteMsg::ModifyBetArray { denoms } => {
            execute_modify_bet_array(deps.into_empty(), info, denoms)
        }
    }
}

fn execute_collect_winnings(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut winnings = Uint128::zero();
    let mut resp = Response::new();

    let my_game_list = query_my_games_without_limit(deps.as_ref(), info.sender.clone())?;
    let live_round = LIVE_ROUND.load(deps.storage)?;
    let next_round = NEXT_ROUND.load(deps.storage)?;
    let mut amount_commissionable = Uint128::zero();

    for game in my_game_list.my_game_list {
        let round_id = game.round_id;

        if live_round.id == round_id || next_round.id == round_id {
            continue;
        }

        let round = ROUNDS.load(deps.storage, round_id.u128())?;

        let pool_shares = round.bear_amount + round.bull_amount;
        let bet_info_key = bet_info_key(round_id.u128(), &info.sender);

        bet_info_storage().remove(deps.storage, bet_info_key.clone())?;

        let claim_info_key = claim_info_key(round_id.u128(), &info.sender);

        if round.bear_amount == Uint128::zero() || round.bull_amount == Uint128::zero() {
            winnings += game.amount;
            if game.amount > Uint128::zero() {
                claim_info_storage().save(
                    deps.storage,
                    claim_info_key,
                    &ClaimInfo {
                        player: info.sender.clone(),
                        round_id,
                        claimed_amount: game.amount,
                    },
                )?;
            }
        } else {
            let round_winnings = match round.winner {
                Some(Direction::Bull) => {
                    /* Only claimable once */
                    match game.direction {
                        Direction::Bull => {
                            let won_shares = game.amount;
                            pool_shares.multiply_ratio(won_shares, round.bull_amount)
                        }
                        Direction::Bear => Uint128::zero(),
                    }
                }
                Some(Direction::Bear) => {
                    /* Only claimable once */
                    match game.direction {
                        Direction::Bull => Uint128::zero(),
                        Direction::Bear => {
                            let won_shares = game.amount;
                            pool_shares.multiply_ratio(won_shares, round.bear_amount)
                        }
                    }
                }
                None => {
                    /* Only claimable once */
                    game.amount
                }
            };

            /* Count it up */
            winnings += round_winnings;
            amount_commissionable += round_winnings;

            if round_winnings > Uint128::zero() {
                claim_info_storage().save(
                    deps.storage,
                    claim_info_key,
                    &ClaimInfo {
                        player: info.sender.clone(),
                        round_id,
                        claimed_amount: round_winnings,
                    },
                )?;
            }
        }
    }

    if winnings == Uint128::zero() {
        return Err(ContractError::Std(StdError::generic_err(
            "Nothing to claim",
        )));
    }

    let mut dev_fee = Uint128::zero();
    if amount_commissionable != Uint128::zero() {
        dev_fee = compute_gaming_fee(deps.as_ref(), amount_commissionable)?;
        let mut messages_dev_fees = Vec::new();
        for dev_wallet in config.clone().dev_wallet_list {
            let token_transfer_msg = BankMsg::Send {
                to_address: dev_wallet.address.to_string(),
                amount: coins(
                    dev_fee.mul_floor(dev_wallet.ratio).u128(),
                    &config.token_denom,
                ),
            };
            messages_dev_fees.push(token_transfer_msg)
        }

        let experience_message = AddExperienceAndElo {
            user: info.sender.clone(),
            experience: amount_commissionable.u128() as u64 * config.exp_per_denom_won,
            elo: None,
        };

        let wasm_message = WasmMsg::Execute {
            contract_addr: config.users_contract.to_string(),
            msg: to_json_binary(&experience_message)?,
            funds: vec![],
        };

        resp = resp
            .add_messages(messages_dev_fees)
            .add_message(wasm_message)
            .add_attribute("velo_action", "distribute-dev-rewards")
            .add_attribute("velo_amount", dev_fee);
    }

    let amount_winnings = winnings.u128() - dev_fee.u128();
    let msg_send_winnings = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(amount_winnings, &config.token_denom),
    };

    Ok(resp
        .add_message(msg_send_winnings)
        .add_attribute("velo_action", "collect-winnings")
        .add_attribute("velo_claimer", info.sender)
        .add_attribute("velo_amount", amount_winnings.to_string()))
}

fn execute_collect_winning_round(
    deps: DepsMut,
    info: MessageInfo,
    round_id: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut winnings = Uint128::zero();
    let mut resp = Response::new();

    let mut my_game_list: Vec<BetInfo> = Vec::new();

    let bet_info_key_round = bet_info_key(round_id.u128(), &info.sender);
    let game = bet_info_storage().may_load(deps.storage, bet_info_key_round)?;
    if let Some(_game) = game {
        my_game_list.push(_game)
    }

    let mut amount_commissionable = Uint128::zero();

    for game in my_game_list {
        let round_id = game.round_id;
        let round = ROUNDS.load(deps.storage, round_id.u128())?;

        let pool_shares = round.bear_amount + round.bull_amount;
        let bet_info_key = bet_info_key(round_id.u128(), &info.sender);

        bet_info_storage().remove(deps.storage, bet_info_key.clone())?;

        let claim_info_key = claim_info_key(round_id.u128(), &info.sender);

        if round.bear_amount == Uint128::zero() || round.bull_amount == Uint128::zero() {
            winnings += game.amount;
            if game.amount > Uint128::zero() {
                claim_info_storage().save(
                    deps.storage,
                    claim_info_key,
                    &ClaimInfo {
                        player: info.sender.clone(),
                        round_id,
                        claimed_amount: winnings,
                    },
                )?;
            }
        } else {
            let round_winnings = match round.winner {
                Some(Direction::Bull) => {
                    /* Only claimable once */
                    match game.direction {
                        Direction::Bull => {
                            let won_shares = game.amount;
                            pool_shares.multiply_ratio(won_shares, round.bull_amount)
                        }
                        Direction::Bear => Uint128::zero(),
                    }
                }
                Some(Direction::Bear) => {
                    /* Only claimable once */
                    match game.direction {
                        Direction::Bull => Uint128::zero(),
                        Direction::Bear => {
                            let won_shares = game.amount;
                            pool_shares.multiply_ratio(won_shares, round.bear_amount)
                        }
                    }
                }
                None => {
                    /* Only claimable once */
                    game.amount
                }
            };

            /* Count it up */
            winnings += round_winnings;
            amount_commissionable += round_winnings;
            if round_winnings > Uint128::zero() {
                claim_info_storage().save(
                    deps.storage,
                    claim_info_key,
                    &ClaimInfo {
                        player: info.sender.clone(),
                        round_id,
                        claimed_amount: winnings,
                    },
                )?;
            }
        }
    }

    if winnings == Uint128::zero() {
        return Err(ContractError::Std(StdError::generic_err(
            "Nothing to claim",
        )));
    }

    let mut dev_fee = Uint128::zero();
    if amount_commissionable != Uint128::zero() {
        dev_fee = compute_gaming_fee(deps.as_ref(), amount_commissionable)?;
        let mut messages_dev_fees = Vec::new();
        for dev_wallet in config.clone().dev_wallet_list {
            let token_transfer_msg = BankMsg::Send {
                to_address: dev_wallet.address.to_string(),
                amount: coins(
                    dev_fee.mul_floor(dev_wallet.ratio).u128(),
                    &config.token_denom,
                ),
            };
            messages_dev_fees.push(token_transfer_msg)
        }

        let experience_message = AddExperienceAndElo {
            user: info.sender.clone(),
            experience: amount_commissionable.u128() as u64 * config.exp_per_denom_won,
            elo: None,
        };

        let wasm_message = WasmMsg::Execute {
            contract_addr: config.users_contract.to_string(),
            msg: to_json_binary(&experience_message)?,
            funds: vec![],
        };

        resp = resp
            .add_messages(messages_dev_fees)
            .add_message(wasm_message)
            .add_attribute("velo_action", "distribute-dev-rewards")
            .add_attribute("velo_amount", dev_fee);
    }

    let amount_winnings = winnings.u128() - dev_fee.u128();
    let msg_send_winnings = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(amount_winnings, &config.token_denom),
    };

    Ok(resp
        .add_message(msg_send_winnings)
        .add_attribute("velo_action", "collect-winnings-round")
        .add_attribute("velo_round_id", round_id)
        .add_attribute("velo_claimer", info.sender)
        .add_attribute("velo_amount", amount_winnings.to_string()))
}

fn execute_bet(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    round_id: Uint128,
    dir: Direction,
    gross: Uint128,
) -> Result<Response, ContractError> {
    assert_not_halted(deps.as_ref())?;

    let mut bet_round = assert_is_current_round(deps.as_ref(), round_id)?;
    let mut resp = Response::new();
    let config = CONFIG.load(deps.storage)?;

    let funds_sent = one_coin(&info)?;

    let totals = TOTALS_SPENT.may_load(deps.storage, info.clone().sender)?;
    if let Some(totals) = totals {
        TOTALS_SPENT.save(
            deps.storage,
            info.clone().sender,
            &totals.checked_add(funds_sent.amount)?,
        )?;
    } else {
        TOTALS_SPENT.save(deps.storage, info.clone().sender, &funds_sent.amount)?;
    }

    if funds_sent.denom != config.token_denom {
        return Err(ContractError::InvalidFunds {});
    }

    if funds_sent.amount != gross {
        return Err(ContractError::NotEnoughFunds {});
    }

    if funds_sent.amount < config.minimum_bet {
        return Err(ContractError::BetUnderMinBetAmount {});
    }

    if env.block.time > bet_round.open_time {
        return Err(ContractError::RoundFinished {
            round_id,
            seconds: env.block.time.seconds() - bet_round.open_time.seconds(),
        });
    }

    let bet_info_key = bet_info_key(round_id.u128(), &info.sender.clone());

    let bet_info = bet_info_storage().may_load(deps.storage, bet_info_key.clone())?;

    let mut amount_bet = gross;
    if let Some(bet_info) = bet_info {
        if bet_info.direction != dir {
            return Err(ContractError::InvalidDirectionBet {});
        }
        amount_bet += bet_info.amount;
    }

    let experience_message = AddExperienceAndElo {
        user: info.sender.clone(),
        experience: funds_sent.amount.u128() as u64 * config.exp_per_denom_bet,
        elo: None,
    };

    let wasm_message = WasmMsg::Execute {
        contract_addr: config.users_contract.to_string(),
        msg: to_json_binary(&experience_message)?,
        funds: vec![],
    };

    match dir {
        Direction::Bull => {
            bet_info_storage().save(
                deps.storage,
                bet_info_key.clone(),
                &BetInfo {
                    player: info.sender.clone(),
                    round_id,
                    amount: amount_bet,
                    direction: Direction::Bull,
                },
            )?;
            bet_round.bull_amount += gross;
            NEXT_ROUND.save(deps.storage, &bet_round)?;
            resp = resp
                .add_attribute("velo_action", "bet".to_string())
                .add_attribute("velo_round", round_id.to_string())
                .add_attribute("velo_direction", "bull".to_string())
                .add_attribute("velo_amount", gross.to_string())
                .add_attribute("velo_round_bull_total", bet_round.bull_amount.to_string())
                .add_attribute("velo_account", info.sender.to_string());
        }
        Direction::Bear => {
            bet_info_storage().save(
                deps.storage,
                bet_info_key.clone(),
                &BetInfo {
                    player: info.sender.clone(),
                    round_id,
                    amount: amount_bet,
                    direction: Direction::Bear,
                },
            )?;
            bet_round.bear_amount += gross;
            NEXT_ROUND.save(deps.storage, &bet_round)?;
            resp = resp
                .add_attribute("velo_action", "bet".to_string())
                .add_attribute("velo_round", round_id.to_string())
                .add_attribute("velo_direction", "bear".to_string())
                .add_attribute("velo_amount", gross.to_string())
                .add_attribute("velo_round_bear_total", bet_round.bear_amount.to_string())
                .add_attribute("velo_account", info.sender.to_string());
        }
    }

    Ok(resp.add_message(wasm_message))
}

fn execute_close_round(deps: DepsMut<NeutronQuery>, env: Env) -> Result<Response, ContractError> {
    assert_not_halted(deps.as_ref().into_empty())?;
    let now = env.block.time;
    let config = CONFIG.load(deps.storage)?;
    let mut resp: Response = Response::new();

    let maybe_live_round = LIVE_ROUND.may_load(deps.storage)?;
    match &maybe_live_round {
        Some(live_round) => {
            if now >= live_round.close_time {
                let finished_round =
                    compute_round_close(deps.as_ref(), env.block.time.seconds(), live_round)?;
                ROUNDS.save(deps.storage, live_round.id.u128(), &finished_round)?;
                resp = resp
                    .add_attribute("velo_action", "finished-round")
                    .add_attribute("velo_round_id", live_round.id.to_string())
                    .add_attribute("velo_close_price", finished_round.close_price.to_string())
                    .add_attribute(
                        "winner",
                        match finished_round.winner {
                            Some(w) => w.to_string(),
                            None => "everybody".to_string(),
                        },
                    );
                LIVE_ROUND.remove(deps.storage);
            }
        }
        None => {}
    }

    /* Close the bidding round if it is finished
     * NOTE Don't allow two live rounds at the same time - wait for the other to close
     */
    let new_bid_round = |deps: DepsMut, env: Env| -> StdResult<Uint128> {
        let id = Uint128::from(NEXT_ROUND_ID.load(deps.storage)?);
        let open_time = match LIVE_ROUND.may_load(deps.storage)? {
            Some(live_round) => live_round.close_time,
            None => env
                .block
                .time
                .plus_seconds(config.next_round_seconds.u128() as u64),
        };
        let close_time = open_time.plus_seconds(config.next_round_seconds.u128() as u64);

        let denoms = ROUND_DENOMS.load(deps.storage)?;
        let round_number = id.u128() as usize;
        let denom = denoms
            .get(round_number.checked_rem(denoms.len()).unwrap())
            .unwrap();

        NEXT_ROUND.save(
            deps.storage,
            &NextRound {
                bear_amount: Uint128::zero(),
                bull_amount: Uint128::zero(),
                bid_time: env.block.time,
                close_time,
                open_time,
                id,
                denom: denom.to_string(),
            },
        )?;
        NEXT_ROUND_ID.save(deps.storage, &(id.u128() + 1u128))?;
        Ok(id)
    };

    let maybe_open_round = NEXT_ROUND.may_load(deps.storage)?;
    match &maybe_open_round {
        Some(open_round) => {
            if LIVE_ROUND.may_load(deps.storage)?.is_none() && now >= open_round.open_time {
                let live_round = compute_round_open(deps.as_ref(), env.clone(), open_round)?;
                resp = resp
                    .add_attribute("velo_action", "bidding_close")
                    .add_attribute("velo_round_id", live_round.id.to_string())
                    .add_attribute("velo_open_price", live_round.open_price.to_string())
                    .add_attribute("velo_bear_amount", live_round.bear_amount.to_string())
                    .add_attribute("velo_bull_amount", live_round.bull_amount.to_string());
                LIVE_ROUND.save(deps.storage, &live_round)?;
                NEXT_ROUND.remove(deps.storage);
                let new_round_id = new_bid_round(deps.into_empty(), env)?;
                resp = resp
                    .add_attribute("velo_action", "new_round")
                    .add_attribute("velo_round_id", new_round_id);
            }
        }
        None => {
            let new_round_id = new_bid_round(deps.into_empty(), env)?;
            resp = resp
                .add_attribute("velo_action", "new_round")
                .add_attribute("velo_round_id", new_round_id);
        }
    }

    Ok(resp)
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    u_config: Config,
) -> Result<Response, ContractError> {
    assert_is_admin(deps.as_ref(), info)?;

    CONFIG.save(deps.storage, &u_config)?;

    Ok(Response::new().add_attribute("velo_action", "update_config"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::Status {} => to_json_binary(&query_status(deps, env)?),
        QueryMsg::MyCurrentPosition { address } => {
            to_json_binary(&query_my_current_position(deps, address)?)
        }
        QueryMsg::FinishedRound { round_id } => {
            to_json_binary(&query_finished_round(deps, round_id)?)
        }
        QueryMsg::MyGameList {
            player,
            start_after,
            limit,
        } => to_json_binary(&query_my_games(deps, player, start_after, limit)?),
        QueryMsg::MyPendingReward { player } => {
            to_json_binary(&query_my_pending_reward(deps, player)?)
        }
        QueryMsg::MyPendingRewardRounds { player } => {
            to_json_binary(&query_my_pending_reward_rounds(deps, player)?)
        }
        QueryMsg::MyRefundableAmount { player } => {
            to_json_binary(&query_my_refundable_amount(deps, player)?)
        }
        QueryMsg::MyRefundableAmountRounds { player } => {
            to_json_binary(&query_my_refundable_amount_rounds(deps, player)?)
        }
        QueryMsg::GetUsersPerRound {
            round_id,
            start_after,
            limit,
        } => to_json_binary(&query_users_per_round(deps, round_id, start_after, limit)?),
        QueryMsg::MyPendingRewardRound { round_id, player } => {
            to_json_binary(&query_my_pending_reward_round(deps, round_id, player)?)
        }
        QueryMsg::GetClaimInfoPerRound {
            round_id,
            start_after,
            limit,
        } => to_json_binary(&query_claim_info_per_round(
            deps,
            round_id,
            start_after,
            limit,
        )?),
        QueryMsg::GetClaimInfoByUser {
            player,
            start_after,
            limit,
        } => to_json_binary(&query_claim_info_by_user(deps, player, start_after, limit)?),
        QueryMsg::TotalSpent { player } => to_json_binary(&query_total_spent(deps, player)?),
        QueryMsg::GetAdmins {} => to_json_binary(&query_get_admins(deps)?),
        QueryMsg::GetRoundDenoms {} => to_json_binary(&query_get_round_denoms(deps)?),
        QueryMsg::GetTickers {} => to_json_binary(&query_get_tickers(deps)?),
    }
}

fn query_finished_round(deps: Deps, round_id: Uint128) -> StdResult<FinishedRound> {
    let round = ROUNDS.load(deps.storage, round_id.u128())?;
    Ok(round)
}

fn query_my_current_position(deps: Deps, address: String) -> StdResult<MyCurrentPositionResponse> {
    let round_id = NEXT_ROUND_ID.load(deps.storage)?;
    let next_bet_key = (round_id - 1, deps.api.addr_validate(&address)?);

    let next_bet_info = bet_info_storage().may_load(deps.storage, next_bet_key)?;

    let mut next_bull_amount = Uint128::zero();
    let mut next_bear_amount = Uint128::zero();

    if let Some(bet_info) = next_bet_info {
        match bet_info.direction {
            Direction::Bull => {
                next_bull_amount = bet_info.amount;
            }
            Direction::Bear => {
                next_bear_amount = bet_info.amount;
            }
        }
    }

    let mut live_bull_amount: Uint128 = Uint128::zero();
    let mut live_bear_amount: Uint128 = Uint128::zero();
    if round_id > 1 {
        let live_bet_key = (round_id - 2, deps.api.addr_validate(&address)?);
        let live_bet_info = bet_info_storage().may_load(deps.storage, live_bet_key)?;

        if let Some(bet_info) = live_bet_info {
            match bet_info.direction {
                Direction::Bull => {
                    live_bull_amount = bet_info.amount;
                }
                Direction::Bear => {
                    live_bear_amount = bet_info.amount;
                }
            }
        }
    }

    Ok(MyCurrentPositionResponse {
        next_bear_amount,
        next_bull_amount,
        live_bear_amount,
        live_bull_amount,
    })
}

fn query_status(deps: Deps, env: Env) -> StdResult<StatusResponse> {
    let live_round = LIVE_ROUND.may_load(deps.storage)?;
    let bidding_round = NEXT_ROUND.may_load(deps.storage)?;
    let current_time = env.block.time;

    Ok(StatusResponse {
        bidding_round,
        live_round,
        current_time,
    })
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    CONFIG.load(deps.storage)
}

pub fn query_my_games(
    deps: Deps,
    player: Addr,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<MyGameResponse> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT) as usize;

    let start = if let Some(start) = start_after {
        let round_id = start;
        Some(Bound::exclusive(bet_info_key(round_id.u128(), &player)))
    } else {
        None
    };

    let my_game_list = bet_info_storage()
        .idx
        .player
        .prefix(player.clone())
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;
    Ok(MyGameResponse { my_game_list })
}

//it is used for backend saving
pub fn query_users_per_round(
    deps: Deps,
    round_id: Uint128,
    start_after: Option<Addr>,
    limit: Option<u32>,
) -> StdResult<RoundUsersResponse> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT) as usize;

    let start = if let Some(start) = start_after {
        let player = start;
        Some(Bound::exclusive(bet_info_key(round_id.u128(), &player)))
    } else {
        None
    };

    let round_users = bet_info_storage()
        .idx
        .round_id
        .prefix(round_id.u128())
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(RoundUsersResponse { round_users })
}

pub fn query_claim_info_per_round(
    deps: Deps,
    round_id: Uint128,
    start_after: Option<Addr>,
    limit: Option<u32>,
) -> StdResult<ClaimInfoResponse> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT) as usize;

    let start = if let Some(start) = start_after {
        let player = start;
        Some(Bound::exclusive(bet_info_key(round_id.u128(), &player)))
    } else {
        None
    };

    let claim_info = claim_info_storage()
        .idx
        .round_id
        .prefix(round_id.u128())
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(ClaimInfoResponse { claim_info })
}

pub fn query_claim_info_by_user(
    deps: Deps,
    player: Addr,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<ClaimInfoResponse> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT) as usize;

    let start = if let Some(start) = start_after {
        let round_id = start;
        Some(Bound::exclusive(bet_info_key(round_id.u128(), &player)))
    } else {
        None
    };

    let claim_info = claim_info_storage()
        .idx
        .player
        .prefix(player)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(ClaimInfoResponse { claim_info })
}

pub fn query_my_pending_reward(deps: Deps, player: Addr) -> StdResult<PendingRewardResponse> {
    let my_game_list = query_my_games_without_limit(deps, player.clone())?;
    let mut winnings = Uint128::zero();

    for game in my_game_list.my_game_list {
        let round_id = game.round_id;
        let round = ROUNDS.may_load(deps.storage, round_id.u128())?;

        if round.is_none() {
            continue;
        }
        let round = round.unwrap();

        let pool_shares = round.bear_amount + round.bull_amount;

        if round.bear_amount == Uint128::zero() || round.bull_amount == Uint128::zero() {
            continue;
        } else {
            let round_winnings = match round.winner {
                Some(Direction::Bull) => {
                    /* Only claimable once */
                    match game.direction {
                        Direction::Bull => {
                            let won_shares = game.amount;
                            pool_shares.multiply_ratio(won_shares, round.bull_amount)
                        }
                        Direction::Bear => Uint128::zero(),
                    }
                }
                Some(Direction::Bear) => {
                    /* Only claimable once */
                    match game.direction {
                        Direction::Bull => Uint128::zero(),
                        Direction::Bear => {
                            let won_shares = game.amount;
                            pool_shares.multiply_ratio(won_shares, round.bear_amount)
                        }
                    }
                }
                None => {
                    /* Only claimable once */
                    game.amount
                }
            };

            /* Count it up */
            winnings += round_winnings;
        }
    }

    Ok(PendingRewardResponse {
        pending_reward: winnings,
    })
}

pub fn query_my_pending_reward_rounds(
    deps: Deps,
    player: Addr,
) -> StdResult<PendingRewardRoundsResponse> {
    let my_game_list = query_my_games_without_limit(deps, player.clone())?;
    let mut winnings = Uint128::zero();
    let mut winnings_per_round: Vec<(Uint128, Uint128)> = vec![];

    for game in my_game_list.my_game_list {
        let round_id = game.round_id;
        let round = ROUNDS.may_load(deps.storage, round_id.u128())?;

        if round.is_none() {
            continue;
        }
        let round = round.unwrap();

        let pool_shares = round.bear_amount + round.bull_amount;

        if round.bear_amount == Uint128::zero() || round.bull_amount == Uint128::zero() {
            continue;
        } else {
            let round_winnings = match round.winner {
                Some(Direction::Bull) => {
                    /* Only claimable once */
                    match game.direction {
                        Direction::Bull => {
                            let won_shares = game.amount;
                            pool_shares.multiply_ratio(won_shares, round.bull_amount)
                        }
                        Direction::Bear => Uint128::zero(),
                    }
                }
                Some(Direction::Bear) => {
                    /* Only claimable once */
                    match game.direction {
                        Direction::Bull => Uint128::zero(),
                        Direction::Bear => {
                            let won_shares = game.amount;
                            pool_shares.multiply_ratio(won_shares, round.bear_amount)
                        }
                    }
                }
                None => {
                    /* Only claimable once */
                    game.amount
                }
            };

            /* Count it up */
            winnings += round_winnings;
            if round_winnings != Uint128::zero() {
                winnings_per_round.push((round_id, round_winnings))
            }
        }
    }

    Ok(PendingRewardRoundsResponse {
        pending_reward_rounds: winnings_per_round,
        pending_reward_total: winnings,
    })
}

pub fn query_my_pending_reward_round(
    deps: Deps,
    round_id: Uint128,
    player: Addr,
) -> StdResult<PendingRewardResponse> {
    let mut winnings = Uint128::zero();
    let mut my_game_list: Vec<BetInfo> = Vec::new();

    let bet_info_key = bet_info_key(round_id.u128(), &player);
    let game = bet_info_storage().may_load(deps.storage, bet_info_key)?;
    if let Some(_game) = game {
        my_game_list.push(_game)
    }

    for game in my_game_list {
        let round_id = game.round_id;
        let round = ROUNDS.may_load(deps.storage, round_id.u128())?;

        if round.is_none() {
            continue;
        }
        let round = round.unwrap();

        let pool_shares = round.bear_amount + round.bull_amount;

        if round.bear_amount == Uint128::zero() || round.bull_amount == Uint128::zero() {
            winnings += game.amount;
        } else {
            let round_winnings = match round.winner {
                Some(Direction::Bull) => {
                    /* Only claimable once */
                    match game.direction {
                        Direction::Bull => {
                            let won_shares = game.amount;
                            pool_shares.multiply_ratio(won_shares, round.bull_amount)
                        }
                        Direction::Bear => Uint128::zero(),
                    }
                }
                Some(Direction::Bear) => {
                    /* Only claimable once */
                    match game.direction {
                        Direction::Bull => Uint128::zero(),
                        Direction::Bear => {
                            let won_shares = game.amount;
                            pool_shares.multiply_ratio(won_shares, round.bear_amount)
                        }
                    }
                }
                None => {
                    /* Only claimable once */
                    game.amount
                }
            };

            /* Count it up */
            winnings += round_winnings;
        }
    }

    Ok(PendingRewardResponse {
        pending_reward: winnings,
    })
}

pub fn query_my_refundable_amount(
    deps: Deps,
    player: Addr,
) -> StdResult<PendingRefundableAmountResponse> {
    let my_game_list = query_my_games_without_limit(deps, player.clone())?;
    let mut refundable_amount = Uint128::zero();

    for game in my_game_list.my_game_list {
        let round_id = game.round_id;
        let round = ROUNDS.may_load(deps.storage, round_id.u128())?;

        if round.is_none() {
            continue;
        }
        let round = round.unwrap();

        if round.bear_amount == Uint128::zero() || round.bull_amount == Uint128::zero() {
            refundable_amount += game.amount;
        }
    }

    Ok(PendingRefundableAmountResponse {
        pending_refundable_amount: refundable_amount,
    })
}

pub fn query_my_refundable_amount_rounds(
    deps: Deps,
    player: Addr,
) -> StdResult<PendingRefundableAmountRoundsResponse> {
    let my_game_list = query_my_games_without_limit(deps, player.clone())?;
    let mut refundable_amount = Uint128::zero();
    let mut refundable_amount_per_rounds: Vec<(Uint128, Uint128)> = vec![];

    for game in my_game_list.my_game_list {
        let round_id = game.round_id;
        let round = ROUNDS.may_load(deps.storage, round_id.u128())?;

        if round.is_none() {
            continue;
        }
        let round = round.unwrap();

        if round.bear_amount == Uint128::zero() || round.bull_amount == Uint128::zero() {
            refundable_amount += game.amount;
            refundable_amount_per_rounds.push((round_id, game.amount))
        }
    }

    Ok(PendingRefundableAmountRoundsResponse {
        pending_refundable_amount_rounds: refundable_amount_per_rounds,
        pending_refundable_amount_total: refundable_amount,
    })
}

pub fn query_my_games_without_limit(deps: Deps, player: Addr) -> StdResult<MyGameResponse> {
    let my_game_list = bet_info_storage()
        .idx
        .player
        .prefix(player.clone())
        .range(deps.storage, None, None, Order::Ascending)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;
    Ok(MyGameResponse { my_game_list })
}

pub fn query_total_spent(deps: Deps, player: Addr) -> StdResult<TotalSpentResponse> {
    let total = TOTALS_SPENT.may_load(deps.storage, player)?;

    Ok(TotalSpentResponse {
        total_spent: total.unwrap_or(Uint128::zero()),
    })
}

pub fn query_get_admins(deps: Deps) -> StdResult<AdminsResponse> {
    let admins = ADMINS.load(deps.storage)?;

    Ok(AdminsResponse { admins })
}

pub fn query_get_round_denoms(deps: Deps) -> StdResult<RoundDenomsResponse> {
    let denoms = ROUND_DENOMS.load(deps.storage)?;

    Ok(RoundDenomsResponse { denoms })
}

pub fn query_get_tickers(deps: Deps) -> StdResult<TickersResponse> {
    let tickers: Vec<String> = PRICE_TICKERS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|v| v.ok())
        .map(|(_, v)| v)
        .collect();

    Ok(TickersResponse { tickers })
}

fn assert_is_current_round(deps: Deps, round_id: Uint128) -> StdResult<NextRound> {
    let open_round = NEXT_ROUND.load(deps.storage)?;

    if round_id != open_round.id {
        return Err(StdError::generic_err(format!(
            "Tried to open at round {} but it's currently round {}",
            round_id, open_round.id
        )));
    }

    Ok(open_round)
}

fn compute_gaming_fee(deps: Deps, gross: Uint128) -> StdResult<Uint128> {
    let staker_fee = CONFIG.load(deps.storage)?.gaming_fee;

    staker_fee
        .checked_multiply_ratio(gross, FEE_PRECISION * 100)
        .map_err(|e| StdError::generic_err(e.to_string()))
}

fn compute_round_open(
    deps: Deps<NeutronQuery>,
    env: Env,
    round: &NextRound,
) -> Result<LiveRound, ContractError> {
    let open_price = get_current_price(deps, env.block.time.seconds(), round.denom.clone())?;
    let config = CONFIG.load(deps.storage)?;

    Ok(LiveRound {
        id: round.id,
        bid_time: round.bid_time,
        open_time: env.block.time,
        close_time: env
            .block
            .time
            .plus_seconds(config.next_round_seconds.u128() as u64),
        open_price,
        bull_amount: round.bull_amount,
        bear_amount: round.bear_amount,
        denom: round.denom.to_string(),
    })
}

fn get_current_price(
    deps: Deps<NeutronQuery>,
    current_timestamp: u64,
    denom: String,
) -> Result<Int128, ContractError> {
    let ticker = PRICE_TICKERS.load(deps.storage, denom)?;
    let oracle_query = OracleQuery::GetPrice {
        currency_pair: CurrencyPair {
            base: ticker,
            quote: USD_TICKER.to_string(),
        },
    };
    let price_response: GetPriceResponse = deps.querier.query(&oracle_query.into())?;
    let dt = DateTime::parse_from_rfc3339(&price_response.price.block_timestamp)
        .expect("Failed to parse timestamp");
    // Convert to a Unix timestamp (seconds since the Unix epoch)
    let unix_timestamp = dt.timestamp();

    assert_price_not_too_old(current_timestamp, unix_timestamp as u64)?;
    let normalized_price = normalize_price(price_response)?;

    Ok(normalized_price)
}

fn normalize_price(price_response: GetPriceResponse) -> Result<Int128, ContractError> {
    let price = price_response.price.price;
    let normalized_price = match price_response.decimals.cmp(&PRICE_DECIMALS) {
        Ordering::Greater => {
            let divisor = i128::pow(10, (price_response.decimals - PRICE_DECIMALS) as u32);
            price.checked_div(Int128::from(divisor))?
        }
        Ordering::Less => {
            let multiplier = i128::pow(10, (PRICE_DECIMALS - price_response.decimals) as u32);
            price.checked_mul(Int128::from(multiplier))?
        }
        Ordering::Equal => price,
    };
    Ok(normalized_price)
}

fn compute_round_close(
    deps: Deps<NeutronQuery>,
    current_timestamp: u64,
    round: &LiveRound,
) -> Result<FinishedRound, ContractError> {
    let close_price = get_current_price(deps, current_timestamp, round.denom.clone())?;

    let winner = match close_price.cmp(&round.open_price) {
        std::cmp::Ordering::Greater =>
        /* Bulls win */
        {
            Some(Direction::Bull)
        }
        std::cmp::Ordering::Less =>
        /* Bears win */
        {
            Some(Direction::Bear)
        }
        std::cmp::Ordering::Equal =>
        /* Weird case where nobody was right */
        {
            None
        }
    };

    Ok(FinishedRound {
        id: round.id,
        bid_time: round.bid_time,
        open_time: round.open_time,
        close_time: round.close_time,
        open_price: round.open_price,
        bear_amount: round.bear_amount,
        bull_amount: round.bull_amount,
        winner,
        close_price,
        denom: round.denom.to_string(),
    })
}

fn assert_not_halted(deps: Deps) -> StdResult<bool> {
    let is_halted = IS_HALTED.load(deps.storage)?;
    if is_halted {
        return Err(StdError::generic_err("Contract is halted"));
    }
    Ok(true)
}

fn assert_price_not_too_old(
    current_timestamp: u64,
    price_timestamp: u64,
) -> Result<(), ContractError> {
    if price_timestamp < current_timestamp - MAX_OLD_PRICE_TIME {
        return Err(ContractError::PriceTooOld {});
    }

    Ok(())
}

fn execute_update_halt(
    deps: DepsMut,
    info: MessageInfo,
    is_halted: bool,
) -> Result<Response, ContractError> {
    assert_is_admin(deps.as_ref(), info)?;
    IS_HALTED.save(deps.storage, &is_halted)?;
    Ok(Response::new().add_event(Event::new("predictiona").add_attribute("halt_games", "true")))
}

fn assert_is_admin(deps: Deps, info: MessageInfo) -> StdResult<bool> {
    let admins = ADMINS.load(deps.storage)?;
    if !admins.contains(&info.sender) {
        return Err(StdError::generic_err(format!(
            "Only an admin can execute this function. Sender: {}",
            info.sender
        )));
    }

    Ok(true)
}

fn execute_add_admin(
    deps: DepsMut,
    info: MessageInfo,
    new_admin: Addr,
) -> Result<Response, ContractError> {
    assert_is_admin(deps.as_ref(), info)?;
    deps.api.addr_validate(new_admin.as_ref())?;
    let mut admins = ADMINS.load(deps.storage)?;

    admins.push(new_admin.clone());

    ADMINS.save(deps.storage, &admins)?;

    Ok(Response::new().add_attribute("add_admin", new_admin.to_string()))
}

fn execute_remove_admin(
    deps: DepsMut,
    info: MessageInfo,
    old_admin: Addr,
) -> Result<Response, ContractError> {
    assert_is_admin(deps.as_ref(), info)?;
    let mut admins = ADMINS.load(deps.storage)?;
    admins.retain(|admin| admin != old_admin);

    if admins.is_empty() {
        return Err(ContractError::NeedOneAdmin {});
    }

    ADMINS.save(deps.storage, &admins)?;
    Ok(Response::new().add_attribute("remove_admin", old_admin.to_string()))
}

fn execute_modify_dev_wallets(
    deps: DepsMut,
    info: MessageInfo,
    new_wallets: Vec<WalletInfo>,
) -> Result<Response, ContractError> {
    assert_is_admin(deps.as_ref(), info)?;
    let mut total_ratio = Decimal::zero();
    for dev_wallet in new_wallets.clone() {
        total_ratio += dev_wallet.ratio;
    }

    if total_ratio != Decimal::one() {
        return Err(ContractError::WrongRatio {});
    }

    let mut config = CONFIG.load(deps.storage)?;
    config.dev_wallet_list = new_wallets;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("velo_action", "new_dev_wallets"))
}

fn execute_add_ticker(
    deps: DepsMut,
    info: MessageInfo,
    denom: String,
    ticker: String,
) -> Result<Response, ContractError> {
    assert_is_admin(deps.as_ref(), info)?;
    PRICE_TICKERS.save(deps.storage, denom, &ticker)?;

    Ok(Response::new().add_attribute("velo_action", "add_identifier"))
}

fn execute_modify_bet_array(
    deps: DepsMut,
    info: MessageInfo,
    denoms: Vec<String>,
) -> Result<Response, ContractError> {
    assert_is_admin(deps.as_ref(), info)?;

    if denoms.is_empty() {
        return Err(ContractError::DenomsEmpty {});
    }

    for each_denom in denoms.clone() {
        if !PRICE_TICKERS.has(deps.storage, each_denom.clone()) {
            return Err(ContractError::DenomNotRegistered { denom: each_denom });
        }
    }

    ROUND_DENOMS.save(deps.storage, &denoms)?;

    Ok(Response::new().add_attribute("velo_action", "modify_bet_array"))
}
