use censor::Censor;
use cosmwasm_std::{
    entry_point, to_json_binary, DepsMut, Empty, Env, MessageInfo, Order, Response, Storage,
};
use cosmwasm_std::{Addr, Binary, Deps, StdResult};
use cw2::set_contract_version;
use cw_ownable::{assert_owner, initialize_owner};
use general::users::{Config, Elo, ExecuteMsg, InstantiateMsg, QueryMsg, User};
use url::Url;

use crate::error::ContractError;
use crate::state::{ADDRESS_TO_USER, CONFIG, GAME_CONTRACTS, NUM_USERS, USERNAME_TO_USER};

const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_MAX_LIMIT: u32 = 250;

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
        ExecuteMsg::ModifyUser { user } => modify_user(deps, env, info, user),
        ExecuteMsg::ModifyVerification {
            username,
            is_verified,
        } => modify_verification(deps, info, username, is_verified),
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
    env: Env,
    info: MessageInfo,
    mut user: User,
) -> Result<Response, ContractError> {
    validate_user(deps.storage, &user)?;

    let current_user = ADDRESS_TO_USER.may_load(deps.storage, info.sender.clone())?;

    if let Some(current) = current_user {
        user.elo = current.elo;
        user.experience = current.experience;
        user.creation_date = current.creation_date;
        user.is_verified = current.is_verified
    } else {
        user.creation_date = Some(env.block.time);
        NUM_USERS.update(deps.storage, |n| -> Result<_, ContractError> { Ok(n + 1) })?;
    }

    user.address = Some(info.sender.clone());
    ADDRESS_TO_USER.save(deps.storage, info.sender.clone(), &user)?;
    USERNAME_TO_USER.save(
        deps.storage,
        user.to_owned().username.unwrap().to_lowercase(),
        &user,
    )?;

    Ok(Response::new()
        .add_attribute("action", "modify_user")
        .add_attribute("user", info.sender))
}

fn modify_verification(
    deps: DepsMut,
    info: MessageInfo,
    username: String,
    is_verified: bool,
) -> Result<Response, ContractError> {
    assert_owner(deps.storage, &info.sender)?;

    let mut user = USERNAME_TO_USER.load(deps.storage, username.to_owned())?;
    user.is_verified = Some(is_verified);
    ADDRESS_TO_USER.save(deps.storage, user.to_owned().address.unwrap(), &user)?;
    USERNAME_TO_USER.save(deps.storage, username.to_owned(), &user)?;

    Ok(Response::new()
        .add_attribute("action", "modify_verification")
        .add_attribute("username", username)
        .add_attribute("is_verified", is_verified.to_string()))
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
            address: Some(user.to_owned()),
            username: None,
            display_name: None,
            description: None,
            country: None,
            image_url: None,
            first_name: None,
            last_name: None,
            phone: None,
            website: None,
            socials: None,
            experience: Some(experience),
            elo: elo.as_ref().map(|e| if e.add { e.amount } else { 0 }),
            creation_date: None,
            is_verified: None,
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
        QueryMsg::User { address } => to_json_binary(&query_user(deps, address)?),
        QueryMsg::TotalUsers {} => to_json_binary(&query_total_users(deps)?),
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::GameRegistered { game_address } => {
            to_json_binary(&query_game_registered(deps, game_address)?)
        }
        QueryMsg::Users { offset, limit } => to_json_binary(&query_users(deps, offset, limit)?),
    }
}

fn query_user(deps: Deps, address: Addr) -> StdResult<User> {
    let user = ADDRESS_TO_USER.load(deps.storage, address)?;
    Ok(user)
}

fn query_users(deps: Deps, offset: Option<u64>, limit: Option<u32>) -> StdResult<Vec<User>> {
    let limit = limit.unwrap_or(DEFAULT_MAX_LIMIT).min(DEFAULT_MAX_LIMIT);
    let offset = offset.unwrap_or(0);
    let users: Vec<User> = ADDRESS_TO_USER
        .range(deps.storage, None, None, Order::Ascending)
        .skip(offset as usize)
        .take(limit as usize)
        .filter_map(|v| v.ok())
        .map(|(_, v)| v)
        .collect();

    Ok(users)
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

// Helpers
fn validate_user(storage: &mut dyn Storage, user: &User) -> Result<(), ContractError> {
    if user.experience.is_some() || user.elo.is_some() {
        return Err(ContractError::CantModifyExpOrElo {});
    }

    if user.creation_date.is_some() {
        return Err(ContractError::CantModifyCreationDate {});
    }

    if user.is_verified.is_some() {
        return Err(ContractError::CantModifyVerified {});
    }

    if user.address.is_some() {
        return Err(ContractError::CantModifyAddress {});
    }

    let censor = Censor::Standard - "ass";

    if let Some(username) = user.username.to_owned() {
        if !(3..=16).contains(&(username.len() as u64)) {
            return Err(ContractError::InvalidLength {
                text: username.to_owned(),
                min: 3,
                max: 16,
            });
        }

        if censor.check(username.as_str()) {
            return Err(ContractError::ProfanityFilter {
                text: username.to_owned(),
            });
        }

        if USERNAME_TO_USER.has(storage, username.to_lowercase()) {
            return Err(ContractError::UsernameAlreadyExists {});
        }
    } else {
        return Err(ContractError::UsernameCannotBeEmpty {});
    }

    if let Some(display_name) = user.display_name.to_owned() {
        if !(3..=16).contains(&(display_name.len() as u64)) {
            return Err(ContractError::InvalidLength {
                text: display_name.to_owned(),
                min: 3,
                max: 16,
            });
        }

        if censor.check(display_name.as_str()) {
            return Err(ContractError::ProfanityFilter {
                text: display_name.to_owned(),
            });
        }
    }

    if let Some(description) = user.description.to_owned() {
        if !(0..=255).contains(&(description.len() as u64)) {
            return Err(ContractError::InvalidLength {
                text: description.to_owned(),
                min: 0,
                max: 255,
            });
        }

        if censor.check(description.as_str()) {
            return Err(ContractError::ProfanityFilter {
                text: description.to_owned(),
            });
        }
    }

    if let Some(image_url) = user.image_url.to_owned() {
        Url::parse(image_url.as_str())?;
        if !(0..=255).contains(&(image_url.len() as u64)) {
            return Err(ContractError::InvalidLength {
                text: image_url.to_owned(),
                min: 0,
                max: 255,
            });
        }
    }

    if let Some(website) = user.website.to_owned() {
        Url::parse(website.as_str())?;
        if !(0..=255).contains(&(website.len() as u64)) {
            return Err(ContractError::InvalidLength {
                text: website.to_owned(),
                min: 0,
                max: 255,
            });
        }
    }

    if let Some(first_name) = user.first_name.to_owned() {
        if !(1..=20).contains(&(first_name.len() as u64)) {
            return Err(ContractError::InvalidLength {
                text: first_name.to_owned(),
                min: 1,
                max: 20,
            });
        }

        if censor.check(first_name.as_str()) {
            return Err(ContractError::ProfanityFilter {
                text: first_name.to_owned(),
            });
        }
    }

    if let Some(last_name) = user.last_name.to_owned() {
        if !(1..=20).contains(&(last_name.len() as u64)) {
            return Err(ContractError::InvalidLength {
                text: last_name.to_owned(),
                min: 1,
                max: 20,
            });
        }

        if censor.check(last_name.as_str()) {
            return Err(ContractError::ProfanityFilter {
                text: last_name.to_owned(),
            });
        }
    }

    if let Some(phone) = user.phone.to_owned() {
        if !(5..=20).contains(&(phone.len() as u64)) {
            return Err(ContractError::InvalidLength {
                text: phone.to_owned(),
                min: 5,
                max: 20,
            });
        }

        let first_character = phone.chars().next();
        if first_character.unwrap() != '+' && !first_character.unwrap().is_numeric() {
            return Err(ContractError::InvalidPhoneNumber {});
        }

        if !phone.chars().skip(1).all(|c| c.is_numeric()) {
            return Err(ContractError::InvalidPhoneNumber {});
        }
    }

    Ok(())
}
