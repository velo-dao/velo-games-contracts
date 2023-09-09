# Prediction Game

This is a contract that will run the Prediction games. It's originally based on the contracts made by DeliverDAO adding new functionality.

Each contract will be 1 prediction game, and we will provide the rounds duration, the gaming fee (% sent to dev wallets), the tokens that we are betting against in an array (which will rotate according to the round id), and the token that we will use as bet currency and prize reward.

We will run a service in a server that will periodically close rounds using the admin wallet to keep the game ongoing indefinitely.
