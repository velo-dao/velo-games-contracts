use cosmwasm_std::StdError;
use cw_ownable::OwnershipError;
use thiserror::Error;
use url::ParseError;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error(transparent)]
    Ownership(#[from] OwnershipError),

    #[error("{0}")]
    ParseError(#[from] ParseError),

    #[error("Neither EXP or ELO can be modified by user")]
    CantModifyExpOrElo {},

    #[error("Creation Date is not modifiable")]
    CantModifyCreationDate {},

    #[error("Verified tick not modifiable")]
    CantModifyVerified {},

    #[error("Address not modifiable")]
    CantModifyAddress {},

    #[error("Address {} not allowed to modify EXP or ELO", address)]
    AddressNotAllowedToModifyExpOrElo { address: String },

    #[error("Username cannot be empty")]
    UsernameCannotBeEmpty {},

    #[error(
        "Invalid length for string: {}, length must be between {} and {}",
        text,
        min,
        max
    )]
    InvalidLength { text: String, min: u64, max: u64 },

    #[error("Profanity filter didn't allow {}", text)]
    ProfanityFilter { text: String },

    #[error("Username already exists")]
    UsernameAlreadyExists {},

    #[error("Phone number not valid")]
    InvalidPhoneNumber {},
}
