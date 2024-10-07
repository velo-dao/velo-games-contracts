use cosmwasm_std::StdError;
use thiserror::Error;
use url::ParseError;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

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

    #[error("Can only use alphanumeric characters")]
    AlphanumericOnly {},

    #[error("Username already exists")]
    UsernameAlreadyExists {},

    #[error("At least one admin must remain")]
    NeedOneAdmin {},

    #[error("Event already exists")]
    EventAlreadyExists {},

    #[error("Event end time must be after start time")]
    EventEndTimeBeforeStartTime {},

    #[error("Cannot create an event that already finished")]
    EventAlreadyFinished {},
}
