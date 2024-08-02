use cw_utils::PaymentError;
use thiserror::Error;

use cosmwasm_std::{OverflowError, StdError, Uint128};

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("Wrong denom sent")]
    InvalidFunds {},

    #[error("Not enough funds for the bet sent")]
    NotEnoughFunds {},

    #[error(
        "Round {} stopped accepting bids {} second(s) ago; the next round has not yet begun",
        round_id,
        seconds
    )]
    RoundFinished { round_id: Uint128, seconds: u64 },

    #[error("Need to bet more than minimum bet amount")]
    BetUnderMinBetAmount {},

    #[error("You cannot bet in both directions")]
    InvalidDirectionBet {},

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("The sum of wallet ratio is not equal to 1")]
    WrongRatio {},

    #[error("At least one admin must remain")]
    NeedOneAdmin {},

    #[error("Denoms can not be empty")]
    DenomsEmpty {},

    #[error("Denom {} must be added with its identifier first.", denom)]
    DenomNotRegistered { denom: String },

    #[error("Price is too old, try again")]
    PriceTooOld {},
}
