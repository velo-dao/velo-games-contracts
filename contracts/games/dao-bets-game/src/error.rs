use cw_ownable::OwnershipError;
use cw_utils::PaymentError;
use thiserror::Error;

use cosmwasm_std::{OverflowError, StdError};

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error(transparent)]
    Ownership(#[from] OwnershipError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("The sum of wallet ratio is not equal to 1")]
    WrongRatio {},

    #[error("Need to bet more than minimum bet amount")]
    BetUnderMinBetAmount {},

    #[error("No bet found with that id")]
    BetNotFound {},

    #[error("Bet already finished")]
    BetAlreadyFinished {},

    #[error("Can't bet on multiple options")]
    CantIncreaseBetOnDifferentOption {},

    #[error("This option does not exist")]
    InvalidOption {},

    #[error("There's nothing to claim")]
    NothingToClaim {},
}
