use cosmwasm_std::{Instantiate2AddressError, StdError};
use cw_ownable::OwnershipError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error(transparent)]
    Ownership(#[from] OwnershipError),

    #[error(transparent)]
    Instantiate2AddressError(#[from] Instantiate2AddressError),

    #[error("The sum of wallet ratio is not equal to 1")]
    WrongRatio {},
}
