# Prediction Game

This is a contract that will run the Prediction games. It's originally based on the contracts made by DeliverDAO adding new functionality.

Each contract will be 1 prediction game, and we will provide the rounds duration, the gaming fee (% sent to dev wallets), the tokens that we are betting against in an array (which will rotate according to the round id), and the token that we will use as bet currency and prize reward.

We will run a service in a server that will periodically close rounds using the admin wallet to keep the game ongoing indefinitely.

## Instantiation

The contract will be instantiated with the following message:

```rust
pub struct InstantiateMsg {
    /* Mutable params */
    pub config: Config,
    // What are we betting against
    pub denom_tickers: Vec<DenomTicker>,
    // Additional admins for the contract
    pub extra_admins: Option<Vec<Addr>>,
}

pub struct DenomTicker {
    pub denom: String,
    pub ticker: String,
}

pub struct Config {
    /* After a round ends this is the duration of the next */
    pub next_round_seconds: Uint128,
    pub minimum_bet: Uint128,
    pub gaming_fee: Uint128,
    //The token we are placing the bet with
    pub token_denom: String,
    //Address of the users contract where this contract will add the XP.
    pub users_contract: Addr,
    //Rewards for Users
    pub exp_per_denom_bet: u64,
    pub exp_per_denom_won: u64,
    pub dev_wallet_list: Vec<WalletInfo>,
}
```

In the Config we provide the round information (duration, minimum bet and gaming fee). The gaming defined is the % of the win that will be sent to the dev wallets and has a 2 decimal precision. This means that if gaming fee is 100, 1% of the win will be sent to the dev wallets.

The users contract is an additional contract that will keep the players information and will be used to add the XP/ELO to the players. 
For each bet the user makes, the contract will add a certain `exp_per_denom_bet` to the user. If the user wins, the contract will add an additional `exp_per_denom_won` for each denom amount won to the user.

The games will rotate according to the `denom_tickers` array in the Instantiation message. The first round will be played with the first token, the second round with the second token, and so on. The `tickers` are used to fetch the prices from Skip Connect (Oracle) module.

## Execution

The contract has a very simple functionality. Users can place bets and the contract will keep track of the bets and the winners. The contract will also keep track of the rounds and will close them after the round duration has passed. Users place bets for the upcoming rounds, not the current one, so only bets that provide the correct round id (the current round id + 1) will be accepted.

There is a service running by the owner of the contract that will close the rounds periodically after each round time has passed. This will automatically start the next round.

Users can do 2 types of bets: `bet_bull` and `bet_bear`. As the name indicates the user is betting that the price of the token will go up or down. The user will provide the amount of tokens they want to bet and the contract will keep track of the bets for each round. A user can bet multiple times per round but can't bet for both bull and bear. So he can only increase his current bet. 

If, for some reason, only bets in one direction have been received, the round has no winners and the users can claim back their bets without any commission applied to it.

Example:

User 1 bets 1000 tokens for bull
User 2 bets 2000 tokens for bull
Nobody bets for bear

Result - Bull wins:

User 1 and User 2 can claim back 1000 and 2000 tokens respectively.
User 1 and User 2 will get 1000 * exp_per_denom_bet and 2000 * exp_per_denom_bet added to their XP.

If there are bets for both directions, the losers will lose their bets and the winners will get the prize. 
The total prize is calculated as the sum of the `(losers bets + winners bets)`.
According to each winner's bet amount, a proportional amount of the total prize (minus gaming fee) can be claimed by them.

Example 1:

User 1 bets 100 tokens for bull.
User 2 bets 200 tokens for bear.
User 3 bets 50 tokens for bear.
Gaming fee is 1000 (10%).

Result - Bear wins:

User 1 loses 100 tokens.
User 2 can claim 350 * (200/250) = 280 tokens, which after applying the 10% commission is 252 tokens.
User 3 can claim 350 * (50/250) = 70 tokens, which after applying the 10% commission is 63 tokens.

User 1 will get 100 * exp_per_denom_bet added to his XP.
User 2 will get 200 * exp_per_denom_bet + 280 * exp_per_denom_won added to his XP.
User 3 will get 50 * exp_per_denom_bet + 70 * exp_per_denom_won added to his XP.

Example 2:

User 1 bets 100 tokens for bull.
User 2 bets 200 tokens for bear.
User 3 bets 50 tokens for bear.
Gaming fee is 1000 (10%).

Result - Bull wins:

User 2 loses 200 tokens.
User 3 loses 50 tokens.
User 1 can claim 350 * (100/100) = 350 tokens, which after applying the 10% commission is 315 tokens.

User 1 will get 100 * exp_per_denom_bet + 350 * exp_per_denom_won added to his XP.
User 2 will get 200 * exp_per_denom_bet added to his XP.
User 3 will get 50 * exp_per_denom_bet added to his XP.

As we can see from the contract functionality, it encourages people to bet for the less popular option, as the prize will be higher. This will make the game more interesting and will make the prize more attractive for the users, encouraging to increase their bet if they see that the prize they can get is higher.

## Owner actions

The owner of the contract can perform multiple actions, such as changing the contract configuration, adding new tokens to bet against, adding new dev wallets, changing the gaming fee, changing the round duration...

Additionally, the owner will be in charge of closing each round after the round duration has passed. This will be done by calling the `close_round` function. This function will close the round and will start the next one. The fees are claimed individually when each user claims their prize(s).

There is also an emergency `Halt` and `Resume` function to stop the contract from accepting bets or the owner from closing rounds. This is useful in case of an emergency. People can still claim their prizes during the `Halt` state.

## User actions

User actions are limited to placing bets and claiming their prizes. The user can place bets for the next round and can claim their prizes after the round has been closed. The user can also claim back their bets if the round had no winners.
