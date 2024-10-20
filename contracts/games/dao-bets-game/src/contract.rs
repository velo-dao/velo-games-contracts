use cosmwasm_std::{
    coins, entry_point, to_json_binary, Addr, BankMsg, Binary, Decimal, Deps, DepsMut, Env,
    MessageInfo, Order, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw_ownable::{assert_owner, get_ownership, initialize_owner};
use cw_storage_plus::Bound;
use cw_utils::must_pay;
use dao_bets::dao_bets::{Bet, BetInfo, BetOption, ClaimInfo, Config};
use general::users::ExecuteMsg::AddExperienceAndElo;

use crate::{
    error::ContractError,
    msg::{
        ExecuteMsg, InstantiateMsg, MigrateMsg, MyBetsResponse, PendingRewardRoundsResponse,
        QueryMsg,
    },
    state::{
        bet_info_key, bet_info_storage, claim_info_key, claim_info_storage, CONFIG, FINISHED_BETS,
        NEXT_BET_ID, TOTALS_SPENT, UNFINISHED_BETS,
    },
};

// Pagination info for queries
const MAX_PAGE_LIMIT: u32 = 250;

const FEE_PRECISION: u128 = 100;

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    initialize_owner(
        deps.storage,
        deps.api,
        Some(
            deps.api
                .addr_validate(msg.owner.unwrap_or(info.sender).as_str())?
                .as_str(),
        ),
    )?;

    if !msg.config.fee_receiver_wallet_list.is_empty() {
        let mut total_ratio = Decimal::zero();
        for dev_wallet in msg.config.fee_receiver_wallet_list.clone() {
            total_ratio += dev_wallet.ratio;
        }

        if total_ratio != Decimal::one() {
            return Err(ContractError::WrongRatio {});
        }
    }

    CONFIG.save(deps.storage, &msg.config)?;
    NEXT_BET_ID.save(deps.storage, &1)?;

    Ok(Response::new().add_attribute("method", "instantiate_dao_bets"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateOwnership(action) => update_ownership(deps, env, info, action),
        ExecuteMsg::UpdateConfig { config } => update_config(deps, info, config),
        ExecuteMsg::BetOn { bet_id, option } => bet_on(deps, env, info, bet_id, option),
        ExecuteMsg::CollectWinnings {} => collect_winnings(deps, info),
        ExecuteMsg::CollectionWinningBet { bet_id } => collect_winnings_bet(deps, info, bet_id),
        ExecuteMsg::CreateBet {
            topic,
            description,
            img_url,
            end_bet_timestamp,
            expected_result_timestamp,
            options,
        } => create_bet(
            deps,
            info,
            topic,
            description,
            img_url,
            end_bet_timestamp,
            expected_result_timestamp,
            options,
        ),
        ExecuteMsg::ModifyBet {
            bet_id,
            topic,
            description,
            end_bet_timestamp,
            expected_result_timestamp,
            img_url,
        } => modify_bet(
            deps,
            info,
            bet_id,
            topic,
            description,
            end_bet_timestamp,
            expected_result_timestamp,
            img_url,
        ),
        ExecuteMsg::CompleteBet {
            bet_id,
            result_option,
        } => complete_bet(deps, info, bet_id, result_option),
        ExecuteMsg::CancelBet { bet_id } => cancel_bet(deps, info, bet_id),
    }
}

fn update_ownership(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    action: cw_ownable::Action,
) -> Result<Response, ContractError> {
    let ownership = cw_ownable::update_ownership(deps, &env.block, &info.sender, action)?;
    Ok(Response::new().add_attributes(ownership.into_attributes()))
}

fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    config: Config,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;

    let mut total_ratio = Decimal::zero();
    for fee_receiver_wallet in &config.fee_receiver_wallet_list {
        total_ratio += fee_receiver_wallet.ratio;
    }

    if total_ratio != Decimal::one() {
        return Err(ContractError::WrongRatio {});
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new())
}

fn bet_on(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bet_id: Uint128,
    option: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let funds_sent = must_pay(&info, &config.token_denom)?;

    if funds_sent < config.minimum_bet {
        return Err(ContractError::BetUnderMinBetAmount {});
    }

    let mut bet = UNFINISHED_BETS
        .load(deps.storage, bet_id.u128())
        .map_err(|_| ContractError::BetNotFound {})?;

    if env.block.time.seconds() > bet.end_bet_timestamp {
        return Err(ContractError::BetAlreadyFinished {});
    }

    let totals = TOTALS_SPENT.may_load(deps.storage, info.clone().sender)?;
    if let Some(totals) = totals {
        TOTALS_SPENT.save(
            deps.storage,
            info.clone().sender,
            &totals.checked_add(funds_sent)?,
        )?;
    } else {
        TOTALS_SPENT.save(deps.storage, info.clone().sender, &funds_sent)?;
    }

    let bet_info_key = bet_info_key(bet_id.u128(), &info.sender.clone());
    let bet_info = bet_info_storage().may_load(deps.storage, bet_info_key.clone())?;

    let mut amount_bet = funds_sent;
    if let Some(bet_info) = bet_info {
        if bet_info.option != option {
            return Err(ContractError::CantIncreaseBetOnDifferentOption {});
        }
        amount_bet += bet_info.amount;
    } else {
        bet.num_players += 1;
    }

    let bet_total_amount = bet
        .current_bet_amounts
        .get_mut(&option)
        .ok_or(ContractError::InvalidOption {})?;

    *bet_total_amount = bet_total_amount.checked_add(funds_sent)?;

    UNFINISHED_BETS.save(deps.storage, bet_id.u128(), &bet)?;

    let experience_message = AddExperienceAndElo {
        user: info.sender.clone(),
        experience: funds_sent.u128() as u64 * config.exp_per_denom_bet,
        elo: None,
    };

    let wasm_message = WasmMsg::Execute {
        contract_addr: config.users_contract.to_string(),
        msg: to_json_binary(&experience_message)?,
        funds: vec![],
    };

    bet_info_storage().save(
        deps.storage,
        bet_info_key.clone(),
        &BetInfo {
            player: info.sender.clone(),
            bet_id,
            amount: amount_bet,
            option: option.clone(),
        },
    )?;

    Ok(Response::new()
        .add_message(wasm_message)
        .add_attribute("action", "bet".to_string())
        .add_attribute("bet_id", bet_id.to_string())
        .add_attribute("option", option)
        .add_attribute("amount", funds_sent)
        .add_attribute("account", info.sender.to_string()))
}

fn collect_winnings(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut winnings = Uint128::zero();
    let mut amount_commissionable = Uint128::zero();

    let my_game_list = query_my_games_without_limit(deps.as_ref(), info.sender.clone())?;

    for game in my_game_list.my_bets_list {
        let bet_id = game.bet_id;

        let finished_bet = match FINISHED_BETS.may_load(deps.storage, bet_id.u128())? {
            Some(finished_bet) => finished_bet,
            None => continue,
        };

        let bet_info_key = bet_info_key(bet_id.u128(), &info.sender);

        bet_info_storage().remove(deps.storage, bet_info_key.clone())?;

        let claim_info_key = claim_info_key(bet_id.u128(), &info.sender);
        let bet_amount = finished_bet.current_bet_amounts.values().sum::<Uint128>();

        // Hasn't won this round and it's not cancelled
        if finished_bet.result_option.unwrap_or_default() != game.option && !finished_bet.cancelled
        {
            continue;
        }

        let round_winnings = if finished_bet.cancelled {
            game.amount
        } else {
            let total_won_shares = finished_bet.current_bet_amounts.get(&game.option).unwrap();
            let user_won_shares = game.amount;
            bet_amount.multiply_ratio(user_won_shares, total_won_shares.u128())
        };

        /* Count it up */
        winnings += round_winnings;
        if !finished_bet.cancelled {
            amount_commissionable += round_winnings;
        }

        claim_info_storage().save(
            deps.storage,
            claim_info_key,
            &ClaimInfo {
                player: info.sender.clone(),
                bet_id,
                claimed_amount: round_winnings,
            },
        )?;
    }

    if winnings == Uint128::zero() {
        return Err(ContractError::Std(StdError::generic_err(
            "Nothing to claim",
        )));
    }

    let mut fee = Uint128::zero();
    let mut messages_fees = Vec::new();
    let mut resp = Response::new();
    if amount_commissionable > Uint128::zero() {
        fee = compute_gaming_fee(deps.as_ref(), winnings)?;
        for fee_wallet in config.fee_receiver_wallet_list {
            let amount = fee.mul_floor(fee_wallet.ratio).u128();
            if amount > 0 {
                let token_transfer_msg = BankMsg::Send {
                    to_address: fee_wallet.address.to_string(),
                    amount: coins(amount, &config.token_denom),
                };
                messages_fees.push(token_transfer_msg)
            }
        }

        let experience_message = AddExperienceAndElo {
            user: info.sender.clone(),
            experience: winnings.u128() as u64 * config.exp_per_denom_won,
            elo: None,
        };

        let wasm_message = WasmMsg::Execute {
            contract_addr: config.users_contract.to_string(),
            msg: to_json_binary(&experience_message)?,
            funds: vec![],
        };
        resp = resp.add_message(wasm_message);
    }

    let amount_winnings = winnings.u128() - fee.u128();
    let msg_send_winnings = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(amount_winnings, &config.token_denom),
    };

    Ok(resp
        .add_messages(messages_fees)
        .add_message(msg_send_winnings)
        .add_attribute("action", "collect-winnings")
        .add_attribute("claimer", info.sender)
        .add_attribute("amount", amount_winnings.to_string()))
}

fn collect_winnings_bet(
    deps: DepsMut,
    info: MessageInfo,
    bet_id: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let finished_bet = match FINISHED_BETS.may_load(deps.storage, bet_id.u128())? {
        Some(finished_bet) => finished_bet,
        None => return Err(ContractError::BetNotFound {}),
    };

    let bet_info_key_round = bet_info_key(bet_id.u128(), &info.sender);
    let game = bet_info_storage()
        .load(deps.storage, bet_info_key_round)
        .map_err(|_| ContractError::NothingToClaim {})?;

    let bet_info_key = bet_info_key(bet_id.u128(), &info.sender);

    bet_info_storage().remove(deps.storage, bet_info_key.clone())?;

    let claim_info_key = claim_info_key(bet_id.u128(), &info.sender);
    let bet_amount = finished_bet.current_bet_amounts.values().sum::<Uint128>();

    // Hasn't won this round and it's not cancelled
    if finished_bet.result_option.unwrap_or_default() != game.option && !finished_bet.cancelled {
        return Err(ContractError::NothingToClaim {});
    }

    let round_winnings = if finished_bet.cancelled {
        game.amount
    } else {
        let total_won_shares = finished_bet.current_bet_amounts.get(&game.option).unwrap();
        let user_won_shares = game.amount;
        bet_amount.multiply_ratio(user_won_shares, total_won_shares.u128())
    };

    claim_info_storage().save(
        deps.storage,
        claim_info_key,
        &ClaimInfo {
            player: info.sender.clone(),
            bet_id,
            claimed_amount: round_winnings,
        },
    )?;

    let mut fee = Uint128::zero();
    let mut messages_fees = Vec::new();
    let mut resp = Response::new();
    if !finished_bet.cancelled {
        fee = compute_gaming_fee(deps.as_ref(), round_winnings)?;
        for fee_wallet in config.fee_receiver_wallet_list {
            let amount = fee.mul_floor(fee_wallet.ratio).u128();
            if amount > 0 {
                let token_transfer_msg = BankMsg::Send {
                    to_address: fee_wallet.address.to_string(),
                    amount: coins(amount, &config.token_denom),
                };
                messages_fees.push(token_transfer_msg)
            }
        }

        let experience_message = AddExperienceAndElo {
            user: info.sender.clone(),
            experience: round_winnings.u128() as u64 * config.exp_per_denom_won,
            elo: None,
        };

        let wasm_message = WasmMsg::Execute {
            contract_addr: config.users_contract.to_string(),
            msg: to_json_binary(&experience_message)?,
            funds: vec![],
        };
        resp = resp.add_message(wasm_message);
    }

    let amount_winnings = round_winnings.u128() - fee.u128();
    let msg_send_winnings = BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(amount_winnings, &config.token_denom),
    };

    Ok(resp
        .add_messages(messages_fees)
        .add_message(msg_send_winnings)
        .add_attribute("action", "collect-winnings")
        .add_attribute("claimer", info.sender)
        .add_attribute("amount", amount_winnings.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn create_bet(
    deps: DepsMut,
    info: MessageInfo,
    topic: String,
    description: String,
    img_url: Option<String>,
    end_bet_timestamp: u64,
    expected_result_timestamp: Option<u64>,
    options: Vec<BetOption>,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;

    let bet_id = NEXT_BET_ID.load(deps.storage)?;
    NEXT_BET_ID.save(deps.storage, &(bet_id + 1))?;

    let bet = Bet {
        bet_id,
        topic,
        description,
        img_url,
        end_bet_timestamp,
        expected_result_timestamp,
        options: options.clone(),
        current_bet_amounts: options
            .iter()
            .map(|option| (option.title.clone(), Uint128::zero()))
            .collect(),
        result_option: None,
        cancelled: false,
        num_players: 0,
    };

    UNFINISHED_BETS.save(deps.storage, bet_id, &bet)?;

    Ok(Response::new()
        .add_attribute("action", "create-bet")
        .add_attribute("bet_id", bet_id.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn modify_bet(
    deps: DepsMut,
    info: MessageInfo,
    bet_id: Uint128,
    topic: Option<String>,
    description: Option<String>,
    end_bet_timestamp: Option<u64>,
    expected_result_timestamp: Option<u64>,
    img_url: Option<String>,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;

    let mut bet = UNFINISHED_BETS
        .load(deps.storage, bet_id.u128())
        .map_err(|_| ContractError::BetNotFound {})?;

    if let Some(topic) = topic {
        bet.topic = topic;
    }

    if let Some(description) = description {
        bet.description = description;
    }

    if let Some(end_bet_timestamp) = end_bet_timestamp {
        bet.end_bet_timestamp = end_bet_timestamp;
    }

    if let Some(expected_result_timestamp) = expected_result_timestamp {
        bet.expected_result_timestamp = Some(expected_result_timestamp);
    }

    if let Some(img_url) = img_url {
        bet.img_url = Some(img_url);
    }

    UNFINISHED_BETS.save(deps.storage, bet_id.u128(), &bet)?;

    Ok(Response::new()
        .add_attribute("action", "modify-bet")
        .add_attribute("bet_id", bet_id))
}

fn complete_bet(
    deps: DepsMut,
    info: MessageInfo,
    bet_id: Uint128,
    result_option: String,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;

    let mut bet = UNFINISHED_BETS
        .load(deps.storage, bet_id.u128())
        .map_err(|_| ContractError::BetNotFound {})?;

    bet.result_option = Some(result_option.clone());

    FINISHED_BETS.save(deps.storage, bet_id.u128(), &bet)?;
    UNFINISHED_BETS.remove(deps.storage, bet_id.u128())?;

    Ok(Response::new()
        .add_attribute("action", "complete-bet")
        .add_attribute("bet_id", bet_id.to_string())
        .add_attribute("result_option", result_option))
}

fn cancel_bet(
    deps: DepsMut,
    info: MessageInfo,
    bet_id: Uint128,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;

    let mut bet = UNFINISHED_BETS
        .load(deps.storage, bet_id.u128())
        .map_err(|_| ContractError::BetNotFound {})?;

    bet.cancelled = true;

    FINISHED_BETS.save(deps.storage, bet_id.u128(), &bet)?;
    UNFINISHED_BETS.remove(deps.storage, bet_id.u128())?;

    Ok(Response::new()
        .add_attribute("action", "cancel-bet")
        .add_attribute("bet_id", bet_id.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Ownership {} => to_json_binary(&get_ownership(deps.storage)?),
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::MyCurrentBets {
            player,
            start_after,
            limit,
        } => to_json_binary(&query_my_current_bets(deps, player, start_after, limit)?),
        QueryMsg::UnfinishedBetInfo { bet_id } => {
            to_json_binary(&query_unfinished_bet(deps, bet_id)?)
        }
        QueryMsg::FinishedBetInfo { bet_id } => to_json_binary(&query_finished_bet(deps, bet_id)?),
        QueryMsg::UnfinishedBets { start_after, limit } => {
            to_json_binary(&query_unfinished_bets(deps, start_after, limit)?)
        }
        QueryMsg::FinishedBets { start_after, limit } => {
            to_json_binary(&query_finished_bets(deps, start_after, limit)?)
        }
        QueryMsg::MyPendingReward { player } => {
            to_json_binary(&query_my_pending_reward(deps, player)?)
        }
        QueryMsg::MyPendingRewardRounds {
            player,
            start_after,
            limit,
        } => to_json_binary(&query_my_pending_reward_rounds(
            deps,
            player,
            start_after,
            limit,
        )?),
        QueryMsg::MyPendingRewardRoundsByTopic {
            player,
            topic,
            start_after,
            limit,
        } => to_json_binary(&query_my_pending_reward_rounds_by_topic(
            deps,
            player,
            topic,
            start_after,
            limit,
        )?),
        QueryMsg::MyPendingRewardRound { round_id, player } => {
            to_json_binary(&query_my_pending_reward_round(deps, round_id, player)?)
        }
        QueryMsg::GetUsersPerRound {
            round_id,
            start_after,
            limit,
        } => to_json_binary(&query_users_per_round(deps, round_id, start_after, limit)?),
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
        QueryMsg::UnfinishedBetsByTopic {
            topic,
            start_after,
            limit,
        } => to_json_binary(&query_unfinished_bets_by_topic(
            deps,
            topic,
            start_after,
            limit,
        )?),
        QueryMsg::FinishedBetsByTopic {
            topic,
            start_after,
            limit,
        } => to_json_binary(&query_finished_bets_by_topic(
            deps,
            topic,
            start_after,
            limit,
        )?),
        QueryMsg::TotalBets {} => to_json_binary(&query_total_bets(deps)?),
    }
}

fn query_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

fn query_my_current_bets(
    deps: Deps,
    player: Addr,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<Vec<BetInfo>> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT);
    let start = start_after.map(|bet_id| Bound::exclusive((bet_id.u128(), player.clone())));

    let my_bets_list = bet_info_storage()
        .idx
        .player
        .prefix(player)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(my_bets_list)
}

fn query_finished_bet(deps: Deps, bet_id: Uint128) -> StdResult<Bet> {
    FINISHED_BETS.load(deps.storage, bet_id.u128())
}

fn query_unfinished_bet(deps: Deps, bet_id: Uint128) -> StdResult<Bet> {
    UNFINISHED_BETS.load(deps.storage, bet_id.u128())
}

fn query_finished_bets(
    deps: Deps,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<Vec<Bet>> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT);
    let start = start_after.map(|bet_id| Bound::exclusive(bet_id.u128()));

    let bets = FINISHED_BETS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(bets)
}

fn query_unfinished_bets(
    deps: Deps,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<Vec<Bet>> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT);
    let start = start_after.map(|bet_id| Bound::exclusive(bet_id.u128()));

    let bets = UNFINISHED_BETS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(bets)
}

fn query_unfinished_bets_by_topic(
    deps: Deps,
    topic: String,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<Vec<Bet>> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT);
    let start = start_after.map(|bet_id| Bound::exclusive(bet_id.u128()));

    let bets = UNFINISHED_BETS
        .idx
        .topic
        .prefix(topic)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(bets)
}

fn query_finished_bets_by_topic(
    deps: Deps,
    topic: String,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<Vec<Bet>> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT);
    let start = start_after.map(|bet_id| Bound::exclusive(bet_id.u128()));

    let bets = FINISHED_BETS
        .idx
        .topic
        .prefix(topic)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(bets)
}

fn query_my_pending_reward(deps: Deps, player: Addr) -> StdResult<Uint128> {
    let my_bets_list = query_my_games_without_limit(deps, player)?;

    let mut pending_reward = Uint128::zero();
    for game in my_bets_list.my_bets_list {
        let bet_id = game.bet_id;

        let finished_bet = match FINISHED_BETS.may_load(deps.storage, bet_id.u128())? {
            Some(finished_bet) => finished_bet,
            None => continue,
        };

        let bet_amount = finished_bet.current_bet_amounts.values().sum::<Uint128>();

        // Hasn't won this round and it's not cancelled
        if finished_bet.result_option.unwrap_or_default() != game.option && !finished_bet.cancelled
        {
            continue;
        }

        let round_winnings = if finished_bet.cancelled {
            game.amount
        } else {
            let total_won_shares = finished_bet.current_bet_amounts.get(&game.option).unwrap();
            let user_won_shares = game.amount;
            bet_amount.multiply_ratio(user_won_shares, total_won_shares.u128())
        };

        pending_reward += round_winnings;
    }

    Ok(pending_reward)
}

fn query_my_pending_reward_rounds(
    deps: Deps,
    player: Addr,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<PendingRewardRoundsResponse> {
    let my_bets_list = query_my_games_with_limit(deps, player, start_after, limit)?;

    let mut pending_reward_rounds = Vec::new();
    let mut pending_reward_total = Uint128::zero();
    for game in my_bets_list.my_bets_list {
        let bet_id = game.bet_id;

        let finished_bet = match FINISHED_BETS.may_load(deps.storage, bet_id.u128())? {
            Some(finished_bet) => finished_bet,
            None => continue,
        };

        let bet_amount = finished_bet.current_bet_amounts.values().sum::<Uint128>();

        // Hasn't won this round and it's not cancelled
        if finished_bet.result_option.unwrap_or_default() != game.option && !finished_bet.cancelled
        {
            continue;
        }

        let round_winnings = if finished_bet.cancelled {
            game.amount
        } else {
            let total_won_shares = finished_bet.current_bet_amounts.get(&game.option).unwrap();
            let user_won_shares = game.amount;
            bet_amount.multiply_ratio(user_won_shares, total_won_shares.u128())
        };

        pending_reward_total += round_winnings;
        pending_reward_rounds.push((bet_id, round_winnings));
    }

    Ok(PendingRewardRoundsResponse {
        pending_reward_rounds,
        pending_reward_total,
    })
}

fn query_my_pending_reward_rounds_by_topic(
    deps: Deps,
    player: Addr,
    topic: String,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<PendingRewardRoundsResponse> {
    let my_bets_list = query_my_games_with_limit(deps, player, start_after, limit)?;

    let mut pending_reward_rounds = Vec::new();
    let mut pending_reward_total = Uint128::zero();
    for game in my_bets_list.my_bets_list {
        let bet_id = game.bet_id;

        let finished_bet = match FINISHED_BETS.may_load(deps.storage, bet_id.u128())? {
            Some(finished_bet) => finished_bet,
            None => continue,
        };

        // Not this topic
        if finished_bet.topic != topic {
            continue;
        }

        let bet_amount = finished_bet.current_bet_amounts.values().sum::<Uint128>();

        // Hasn't won this round and it's not cancelled
        if finished_bet.result_option.unwrap_or_default() != game.option && !finished_bet.cancelled
        {
            continue;
        }

        let round_winnings = if finished_bet.cancelled {
            game.amount
        } else {
            let total_won_shares = finished_bet.current_bet_amounts.get(&game.option).unwrap();
            let user_won_shares = game.amount;
            bet_amount.multiply_ratio(user_won_shares, total_won_shares.u128())
        };

        pending_reward_total += round_winnings;
        pending_reward_rounds.push((bet_id, round_winnings));
    }

    Ok(PendingRewardRoundsResponse {
        pending_reward_rounds,
        pending_reward_total,
    })
}

fn query_my_pending_reward_round(
    deps: Deps,
    round_id: Uint128,
    player: Addr,
) -> StdResult<Uint128> {
    let finished_bet = match FINISHED_BETS.may_load(deps.storage, round_id.u128())? {
        Some(finished_bet) => finished_bet,
        None => return Ok(Uint128::zero()),
    };

    let bet_info_key = bet_info_key(round_id.u128(), &player);

    let game = bet_info_storage().load(deps.storage, bet_info_key)?;

    let bet_amount = finished_bet.current_bet_amounts.values().sum::<Uint128>();

    // Hasn't won this round and it's not cancelled
    if finished_bet.result_option.unwrap_or_default() != game.option && !finished_bet.cancelled {
        return Ok(Uint128::zero());
    }

    let round_winnings = if finished_bet.cancelled {
        game.amount
    } else {
        let total_won_shares = finished_bet.current_bet_amounts.get(&game.option).unwrap();
        let user_won_shares = game.amount;
        bet_amount.multiply_ratio(user_won_shares, total_won_shares.u128())
    };

    Ok(round_winnings)
}

fn query_my_games_without_limit(deps: Deps, player: Addr) -> StdResult<MyBetsResponse> {
    let my_bets_list = bet_info_storage()
        .idx
        .player
        .prefix(player.clone())
        .range(deps.storage, None, None, Order::Ascending)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;
    Ok(MyBetsResponse { my_bets_list })
}

fn query_my_games_with_limit(
    deps: Deps,
    player: Addr,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<MyBetsResponse> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT);
    let start = start_after.map(|bet_id| Bound::exclusive((bet_id.u128(), player.clone())));

    let my_bets_list = bet_info_storage()
        .idx
        .player
        .prefix(player.clone())
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(MyBetsResponse { my_bets_list })
}

fn query_users_per_round(
    deps: Deps,
    round_id: Uint128,
    start_after: Option<Addr>,
    limit: Option<u32>,
) -> StdResult<Vec<BetInfo>> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT);
    let start = start_after.map(|player| Bound::exclusive((round_id.u128(), player)));

    let users_per_round = bet_info_storage()
        .idx
        .bet_id
        .prefix(round_id.u128())
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(users_per_round)
}

fn query_claim_info_per_round(
    deps: Deps,
    round_id: Uint128,
    start_after: Option<Addr>,
    limit: Option<u32>,
) -> StdResult<Vec<ClaimInfo>> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT);
    let start = start_after.map(|player| Bound::exclusive((round_id.u128(), player)));

    let claim_info_per_round = claim_info_storage()
        .idx
        .bet_id
        .prefix(round_id.u128())
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(claim_info_per_round)
}

fn query_claim_info_by_user(
    deps: Deps,
    player: Addr,
    start_after: Option<Uint128>,
    limit: Option<u32>,
) -> StdResult<Vec<ClaimInfo>> {
    let limit = limit.unwrap_or(MAX_PAGE_LIMIT).min(MAX_PAGE_LIMIT);
    let start = start_after.map(|bet_id| Bound::exclusive((bet_id.u128(), player.clone())));

    let claim_info_by_user = claim_info_storage()
        .idx
        .player
        .prefix(player)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .map(|res| res.map(|item| item.1))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(claim_info_by_user)
}

fn query_total_spent(deps: Deps, player: Addr) -> StdResult<Uint128> {
    Ok(TOTALS_SPENT
        .may_load(deps.storage, player)?
        .unwrap_or_default())
}

fn query_total_bets(deps: Deps) -> StdResult<Uint128> {
    Ok(Uint128::from(NEXT_BET_ID.load(deps.storage)? - 1))
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

// Helpers
fn compute_gaming_fee(deps: Deps, amount: Uint128) -> StdResult<Uint128> {
    let gaming_fee = CONFIG.load(deps.storage)?.gaming_fee;

    gaming_fee
        .checked_multiply_ratio(amount, FEE_PRECISION * 100)
        .map_err(|e| StdError::generic_err(e.to_string()))
}
