# DAO Bets

This is a contract that will run the DAO based bets.

The contract will allow a DAO to create a bet with multiple options and each user can bet on one of the options. The contract will keep track of the bets. A bet will be created with a specific open time and close time for bets to be submitted. After the close time, the DAO can submit the result of the bet at any point in time and the winners can claim their corresponding share of the prize.

We will run a service in a server that will periodically close rounds using the admin wallet to keep the game ongoing indefinitely.
