use cosmwasm_std::{
    entry_point, instantiate2_address, to_json_binary, Addr, Binary, Decimal, Deps, DepsMut, Empty,
    Env, MessageInfo, Order, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version};
use cw_ownable::{assert_owner, get_ownership, initialize_owner};
use cw_storage_plus::Bound;
use prediction::prediction_game::{DenomTicker, WalletInfo};

use crate::{
    error::ContractError,
    msg::{ExecuteMsg, GameInfo, InstantiateMsg, MigrateMsg, QueryMsg},
    state::{Config, CONFIG, GAMES},
};

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_MAX_LIMIT: u32 = 1000;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, _msg: Empty) -> Result<Response, ContractError> {
    let games: Vec<Addr> = GAMES
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(Result::ok)
        .map(|(game, _)| game)
        .collect();

    let mut messages = vec![];
    for game in games.iter() {
        messages.push(WasmMsg::Execute {
            contract_addr: game.to_string(),
            msg: to_json_binary(&prediction::prediction_game::msg::ExecuteMsg::CloseRound {})?,
            funds: vec![],
        });
    }

    Ok(Response::new().add_messages(messages))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    initialize_owner(
        deps.storage,
        deps.api,
        Some(&info.sender.clone().into_string()),
    )?;

    let mut total_ratio = Decimal::zero();
    for dev_wallet in msg.dev_wallet_list.iter() {
        total_ratio += dev_wallet.ratio;
    }

    if total_ratio != Decimal::one() {
        return Err(ContractError::WrongRatio {});
    }

    let canonical_creator = deps.api.addr_canonicalize(env.contract.address.as_str())?;
    let code_info_response = deps.querier.query_wasm_code_info(msg.users_code_id)?;
    let salt_str = env.block.height.to_string();
    let salt = salt_str.as_bytes();
    let canonical_address = instantiate2_address(
        code_info_response.checksum.as_slice(),
        &canonical_creator,
        salt,
    )?;
    let address = deps.api.addr_humanize(&canonical_address)?;

    let config = Config {
        users_code_id: msg.users_code_id,
        users_contract: address,
        games_code_id: msg.games_code_id,
        dev_wallet_list: msg.dev_wallet_list,
    };
    CONFIG.save(deps.storage, &config)?;

    let wasm_msg = WasmMsg::Instantiate2 {
        code_id: msg.users_code_id,
        msg: to_json_binary(&general::users::InstantiateMsg {
            config: msg.users_config,
            extra_admins: Some(vec![info.sender.clone()]),
        })?,
        funds: vec![],
        admin: Some(info.sender.to_string()),
        label: "users_contract".to_string(),
        salt: Binary::from(salt),
    };

    Ok(Response::new()
        .add_message(wasm_msg)
        .add_attribute("action", "instantiate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION)
        .add_attribute("owner", info.sender))
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
        ExecuteMsg::CreateGame {
            next_round_seconds,
            minimum_bet,
            gaming_fee,
            token_denom,
            exp_per_denom_bet,
            exp_per_denom_won,
            denom_tickers,
            label,
        } => create_game(
            deps,
            env,
            info,
            next_round_seconds,
            minimum_bet,
            gaming_fee,
            token_denom,
            exp_per_denom_bet,
            exp_per_denom_won,
            denom_tickers,
            label,
        ),
        ExecuteMsg::ModifyDevWallets {
            wallets,
            update_all_games,
        } => modify_dev_wallets(deps, info, wallets, update_all_games),
        ExecuteMsg::UpdateCodeIds {
            users_code_id,
            games_code_id,
        } => update_code_ids(deps, info, users_code_id, games_code_id),
        ExecuteMsg::UpdateUsersContract {
            address,
            update_all_games,
            add_all_games_to_users_contract,
        } => update_users_contract(
            deps,
            info,
            address,
            update_all_games,
            add_all_games_to_users_contract,
        ),
        ExecuteMsg::HaltAllGames {} => halt_all_games(deps, info),
        ExecuteMsg::ResumeAllGames {} => resume_all_games(deps, info),
        ExecuteMsg::ManuallyAddGame {
            address,
            add_to_users_contract,
        } => manually_add_game(deps, info, address, add_to_users_contract),
        ExecuteMsg::ManuallyRemoveGame {
            address,
            remove_from_users_contract,
        } => manually_remove_game(deps, info, address, remove_from_users_contract),
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

#[allow(clippy::too_many_arguments)]
fn create_game(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    next_round_seconds: Uint128,
    minimum_bet: Uint128,
    gaming_fee: Uint128,
    token_denom: String,
    exp_per_denom_bet: u64,
    exp_per_denom_won: u64,
    denom_tickers: Vec<DenomTicker>,
    label: String,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;
    let config = CONFIG.load(deps.storage)?;
    let canonical_creator = deps.api.addr_canonicalize(env.contract.address.as_str())?;
    let code_info_response = deps.querier.query_wasm_code_info(config.games_code_id)?;
    let salt_str = env.block.height.to_string();
    let salt = salt_str.as_bytes();
    let canonical_address = instantiate2_address(
        code_info_response.checksum.as_slice(),
        &canonical_creator,
        salt,
    )?;
    let address = deps.api.addr_humanize(&canonical_address)?;

    GAMES.save(deps.storage, address.clone(), &Empty {})?;

    let create_game_message = WasmMsg::Instantiate2 {
        code_id: config.games_code_id,
        msg: to_json_binary(&prediction::prediction_game::msg::InstantiateMsg {
            config: prediction::prediction_game::Config {
                next_round_seconds,
                minimum_bet,
                gaming_fee,
                token_denom,
                users_contract: config.users_contract.clone(),
                exp_per_denom_bet,
                exp_per_denom_won,
                dev_wallet_list: config.dev_wallet_list,
            },
            denom_tickers,
            extra_admins: Some(vec![info.sender.clone()]),
        })?,
        funds: vec![],
        admin: Some(info.sender.to_string()),
        label,
        salt: Binary::from(salt),
    };

    let add_game_to_users_contract_message = WasmMsg::Execute {
        contract_addr: config.users_contract.to_string(),
        msg: to_json_binary(&general::users::ExecuteMsg::AddGame {
            address: address.clone(),
        })?,
        funds: vec![],
    };

    Ok(Response::new()
        .add_message(create_game_message)
        .add_message(add_game_to_users_contract_message)
        .add_attribute("action", "create_game")
        .add_attribute("contract_address", address))
}

fn modify_dev_wallets(
    deps: DepsMut,
    info: MessageInfo,
    wallets: Vec<WalletInfo>,
    update_all_games: bool,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;
    let mut config = CONFIG.load(deps.storage)?;

    let mut total_ratio = Decimal::zero();
    for dev_wallet in wallets.iter() {
        total_ratio += dev_wallet.ratio;
    }

    if total_ratio != Decimal::one() {
        return Err(ContractError::WrongRatio {});
    }

    config.dev_wallet_list.clone_from(&wallets);
    CONFIG.save(deps.storage, &config)?;

    let mut messages = vec![];
    if update_all_games {
        let games: Vec<Addr> = GAMES
            .range(deps.storage, None, None, Order::Ascending)
            .filter_map(Result::ok)
            .map(|(game, _)| game)
            .collect();

        for game in games {
            messages.push(WasmMsg::Execute {
                contract_addr: game.to_string(),
                msg: to_json_binary(
                    &prediction::prediction_game::msg::ExecuteMsg::ModifyDevWallet {
                        new_dev_wallets: wallets.clone(),
                    },
                )?,
                funds: vec![],
            });
        }
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "modify_dev_wallets")
        .add_attribute("update_all_games", update_all_games.to_string()))
}

fn update_code_ids(
    deps: DepsMut,
    info: MessageInfo,
    users_code_id: u64,
    games_code_id: u64,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;
    let mut config = CONFIG.load(deps.storage)?;
    config.users_code_id = users_code_id;
    config.games_code_id = games_code_id;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_code_ids"))
}

fn update_users_contract(
    deps: DepsMut,
    info: MessageInfo,
    address: Addr,
    update_all_games: bool,
    add_all_games_to_users_contract: bool,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;
    let mut config = CONFIG.load(deps.storage)?;
    config.users_contract = address.clone();
    CONFIG.save(deps.storage, &config)?;

    let mut messages = vec![];
    if update_all_games || add_all_games_to_users_contract {
        let games: Vec<Addr> = GAMES
            .range(deps.storage, None, None, Order::Ascending)
            .filter_map(Result::ok)
            .map(|(game, _)| game)
            .collect();

        if update_all_games {
            for game in games.iter() {
                let mut games_config: prediction::prediction_game::Config =
                    deps.querier.query_wasm_smart(
                        game.clone(),
                        &to_json_binary(&prediction::prediction_game::msg::QueryMsg::Config {})?,
                    )?;

                games_config.users_contract = address.clone();

                messages.push(WasmMsg::Execute {
                    contract_addr: game.to_string(),
                    msg: to_json_binary(
                        &prediction::prediction_game::msg::ExecuteMsg::UpdateConfig {
                            config: games_config,
                        },
                    )?,
                    funds: vec![],
                });
            }
        }

        if add_all_games_to_users_contract {
            messages.push(WasmMsg::Execute {
                contract_addr: address.to_string(),
                msg: to_json_binary(&general::users::ExecuteMsg::AddGames { addresses: games })?,
                funds: vec![],
            });
        }
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "update_users_contract")
        .add_attribute("update_all_games", update_all_games.to_string())
        .add_attribute(
            "add_all_games_to_users_contract",
            add_all_games_to_users_contract.to_string(),
        ))
}

fn halt_all_games(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;
    let games: Vec<Addr> = GAMES
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(Result::ok)
        .map(|(game, _)| game)
        .collect();

    let mut messages = vec![];
    for game in games.iter() {
        messages.push(WasmMsg::Execute {
            contract_addr: game.to_string(),
            msg: to_json_binary(&prediction::prediction_game::msg::ExecuteMsg::Halt {})?,
            funds: vec![],
        });
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "halt_all_games"))
}

fn resume_all_games(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;
    let games: Vec<Addr> = GAMES
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(Result::ok)
        .map(|(game, _)| game)
        .collect();

    let mut messages = vec![];
    for game in games.iter() {
        messages.push(WasmMsg::Execute {
            contract_addr: game.to_string(),
            msg: to_json_binary(&prediction::prediction_game::msg::ExecuteMsg::Resume {})?,
            funds: vec![],
        });
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "resume_all_games"))
}

fn manually_add_game(
    deps: DepsMut,
    info: MessageInfo,
    address: Addr,
    add_to_users_contract: bool,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;
    deps.api.addr_validate(address.as_str())?;
    GAMES.save(deps.storage, address.clone(), &Empty {})?;

    let mut messages = vec![];
    if add_to_users_contract {
        let config = CONFIG.load(deps.storage)?;
        messages.push(WasmMsg::Execute {
            contract_addr: config.users_contract.to_string(),
            msg: to_json_binary(&general::users::ExecuteMsg::AddGame {
                address: address.clone(),
            })?,
            funds: vec![],
        });
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "manually_add_game")
        .add_attribute("contract_address", address))
}

fn manually_remove_game(
    deps: DepsMut,
    info: MessageInfo,
    address: Addr,
    remove_from_users_contract: bool,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;
    GAMES.remove(deps.storage, address.clone());

    let mut messages = vec![];
    if remove_from_users_contract {
        let config = CONFIG.load(deps.storage)?;
        messages.push(WasmMsg::Execute {
            contract_addr: config.users_contract.to_string(),
            msg: to_json_binary(&general::users::ExecuteMsg::RemoveGame {
                address: address.clone(),
            })?,
            funds: vec![],
        });
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "manually_remove_game")
        .add_attribute("contract_address", address))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Ownership {} => to_json_binary(&get_ownership(deps.storage)?),
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::Games { start_after, limit } => {
            to_json_binary(&query_games(deps, start_after, limit))
        }
        QueryMsg::GamesInfo { start_after, limit } => {
            to_json_binary(&query_games_info(deps, start_after, limit)?)
        }
        QueryMsg::GamesInfoWithDuration {
            start_after,
            limit,
            duration,
        } => to_json_binary(&query_games_info_with_duration(
            deps,
            start_after,
            limit,
            duration,
        )?),
    }
}

fn query_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

fn query_games(deps: Deps, start_after: Option<Addr>, limit: Option<u32>) -> Vec<Addr> {
    let limit = limit.unwrap_or(DEFAULT_MAX_LIMIT).min(DEFAULT_MAX_LIMIT);
    let start = start_after.map(Bound::exclusive);
    let games: Vec<Addr> = GAMES
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .filter_map(Result::ok)
        .map(|(game, _)| game)
        .collect();

    games
}

fn query_games_info(
    deps: Deps,
    start_after: Option<Addr>,
    limit: Option<u32>,
) -> StdResult<Vec<GameInfo>> {
    let limit = limit.unwrap_or(DEFAULT_MAX_LIMIT).min(DEFAULT_MAX_LIMIT);
    let start = start_after.map(Bound::exclusive);
    let games: Vec<Addr> = GAMES
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .filter_map(Result::ok)
        .map(|(game, _)| game)
        .collect();

    let mut games_info = vec![];
    for game in games {
        let game_config: prediction::prediction_game::Config = deps.querier.query_wasm_smart(
            game.clone(),
            &to_json_binary(&prediction::prediction_game::msg::QueryMsg::Config {})?,
        )?;

        games_info.push(GameInfo {
            address: game,
            next_round_seconds: game_config.next_round_seconds,
        })
    }

    Ok(games_info)
}

fn query_games_info_with_duration(
    deps: Deps,
    start_after: Option<Addr>,
    limit: Option<u32>,
    duration: Uint128,
) -> StdResult<Vec<GameInfo>> {
    let limit = limit.unwrap_or(DEFAULT_MAX_LIMIT).min(DEFAULT_MAX_LIMIT);
    let start = start_after.map(Bound::exclusive);
    let games: Vec<Addr> = GAMES
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit as usize)
        .filter_map(Result::ok)
        .map(|(game, _)| game)
        .collect();

    let mut games_info = vec![];
    for game in games {
        let game_config: prediction::prediction_game::Config = deps.querier.query_wasm_smart(
            game.clone(),
            &to_json_binary(&prediction::prediction_game::msg::QueryMsg::Config {})?,
        )?;

        if game_config.next_round_seconds != duration {
            continue;
        }

        games_info.push(GameInfo {
            address: game,
            next_round_seconds: game_config.next_round_seconds,
        })
    }

    Ok(games_info)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    let version = get_contract_version(deps.storage)?;
    if version.contract != CONTRACT_NAME {
        return Err(StdError::generic_err("Can only upgrade from same type"));
    }
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}
