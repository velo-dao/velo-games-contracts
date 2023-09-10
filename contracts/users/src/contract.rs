use cosmwasm_std::{entry_point, to_binary, DepsMut, Empty, Env, MessageInfo, Order, Response};
use cosmwasm_std::{Addr, Binary, Deps, StdResult};
use cw2::set_contract_version;
use cw_ownable::{assert_owner, initialize_owner};
use general::users::{Config, Elo, ExecuteMsg, InstantiateMsg, QueryMsg, User};

use crate::error::ContractError;
use crate::state::{ADDRESS_TO_USER, CONFIG, GAME_CONTRACTS, NUM_USERS};

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    initialize_owner(
        deps.storage,
        deps.api,
        Some(&info.sender.clone().into_string()),
    )?;

    NUM_USERS.save(deps.storage, &0)?;
    CONFIG.save(deps.storage, &msg.config)?;

    Ok(Response::new()
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
        ExecuteMsg::UpdateConfig { config } => update_config(deps, info, config),
        ExecuteMsg::AddGame { address } => add_game(deps, info, address),
        ExecuteMsg::RemoveGame { address } => remove_game(deps, info, address),
        ExecuteMsg::ModifyUser { user } => modify_user(deps, info, user),
        ExecuteMsg::ResetElo { elo_substraction } => reset_elo(deps, info, elo_substraction),
        ExecuteMsg::AddExperienceAndElo {
            user,
            experience,
            elo,
        } => add_experience_and_elo(deps, info, user, experience, elo),
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

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("exp_per_level", config.initial_exp_per_level.to_string())
        .add_attribute(
            "increase_exp_per_level",
            config.exp_increase_per_level.to_string(),
        ))
}

fn add_game(deps: DepsMut, info: MessageInfo, game: Addr) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;

    GAME_CONTRACTS.save(
        deps.storage,
        deps.api.addr_validate(game.as_ref())?,
        &Empty {},
    )?;

    Ok(Response::new()
        .add_attribute("action", "add_game_contract")
        .add_attribute("address", game))
}

fn remove_game(deps: DepsMut, info: MessageInfo, game: Addr) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;

    GAME_CONTRACTS.remove(deps.storage, game.clone());

    Ok(Response::new()
        .add_attribute("action", "remove_game_contract")
        .add_attribute("address", game))
}

fn modify_user(
    deps: DepsMut,
    info: MessageInfo,
    mut user: User,
) -> Result<Response, ContractError> {
    if user.experience.is_some() || user.elo.is_some() {
        return Err(ContractError::CantModifyExpOrElo {});
    }

    let current_user = ADDRESS_TO_USER.may_load(deps.storage, info.sender.clone())?;

    if let Some(current) = current_user {
        user.elo = current.elo;
        user.experience = current.experience
    } else {
        NUM_USERS.update(deps.storage, |n| -> Result<_, ContractError> { Ok(n + 1) })?;
    }

    ADDRESS_TO_USER.save(deps.storage, info.sender.clone(), &user)?;

    Ok(Response::new()
        .add_attribute("action", "modify_user")
        .add_attribute("user", info.sender))
}

fn reset_elo(
    deps: DepsMut,
    info: MessageInfo,
    elo_substraction: Option<u64>,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;

    let all_addresses: Vec<Addr> = ADDRESS_TO_USER
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|v| v.ok())
        .map(|(k, _)| k)
        .collect();

    for each_addr in all_addresses {
        let mut user = ADDRESS_TO_USER.load(deps.storage, each_addr.clone())?;
        let mut elo = user.elo.unwrap_or(0);
        if let Some(substraction) = elo_substraction {
            elo = elo.saturating_sub(substraction);
        }
        user.elo = Some(elo);
        ADDRESS_TO_USER.save(deps.storage, each_addr, &user)?;
    }

    Ok(Response::new().add_attribute("action", "reset_elo"))
}

fn add_experience_and_elo(
    deps: DepsMut,
    info: MessageInfo,
    user: Addr,
    experience: u64,
    elo: Option<Elo>,
) -> Result<Response, ContractError> {
    if GAME_CONTRACTS.has(deps.storage, info.sender.clone()) {
        return Err(ContractError::AddressNotAllowedToModifyExpOrElo {
            address: info.sender.to_string(),
        });
    }

    let mut updated_user;
    let current_user = ADDRESS_TO_USER.may_load(deps.storage, info.sender.clone())?;

    if let Some(current) = current_user {
        updated_user = current.clone();
        updated_user.experience = Some(current.experience.unwrap_or_default() + experience);
        if let Some(elo) = elo.clone() {
            let mut current_elo = updated_user.elo.unwrap_or_default();
            if elo.add {
                current_elo += elo.amount;
            } else {
                current_elo = current_elo.saturating_sub(elo.amount);
            }
            updated_user.elo = Some(current_elo);
        }
    } else {
        updated_user = User {
            username: None,
            description: None,
            country: None,
            image_url: None,
            first_name: None,
            last_name: None,
            email: None,
            phone: None,
            website: None,
            socials: None,
            experience: Some(experience),
            elo: elo.as_ref().map(|e| if e.add { e.amount } else { 0 }),
        };
        NUM_USERS.update(deps.storage, |n| -> Result<_, ContractError> { Ok(n + 1) })?;
    }

    ADDRESS_TO_USER.save(deps.storage, info.sender.clone(), &updated_user)?;

    Ok(Response::new()
        .add_attribute("action", "modify_experience_and_elo")
        .add_attribute("game_contract", info.sender)
        .add_attribute("user", user)
        .add_attribute("experience_addition", experience.to_string())
        .add_attribute("elo_modification", elo.is_some().to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::User { address } => to_binary(&query_user(deps, address)?),
        QueryMsg::TotalUsers {} => to_binary(&query_total_users(deps)?),
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::GameRegistered { game_address } => {
            to_binary(&query_game_registered(deps, game_address)?)
        }
    }
}

fn query_user(deps: Deps, address: Addr) -> StdResult<User> {
    let user = ADDRESS_TO_USER.load(deps.storage, address)?;
    Ok(user)
}

fn query_total_users(deps: Deps) -> StdResult<u128> {
    let n_users = NUM_USERS.load(deps.storage)?;
    Ok(n_users)
}

fn query_config(deps: Deps) -> StdResult<Config> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config)
}

fn query_game_registered(deps: Deps, game_address: Addr) -> StdResult<bool> {
    Ok(GAME_CONTRACTS.has(deps.storage, game_address))
}
