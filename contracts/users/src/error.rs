use cosmwasm_std::StdError;
use cw_ownable::OwnershipError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error(transparent)]
    Ownership(#[from] OwnershipError),

    #[error("Neither EXP or ELO can be modified by user")]
    CantModifyExpOrElo {},

    #[error("Address {} not allowed to modify EXP or ELO", address)]
    AddressNotAllowedToModifyExpOrElo { address: String },
}
