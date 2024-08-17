# Manager

This contract is a convenience contract add functionality to manage common information for all games and update all games state at the same time:

- It instantiates a users contract during instantiation that will be shared among all games and ,every time a game is created through it, it will save the game address in a list and update the users
contract to include the new game.
- It allows updating the users contract, which also provides an option to update the users contract of all the games and to add all the games in the manager contract to the users contract.
- It allows halting and resuming all games at the same time.
- Provides a way to query all current games games and to query all current games with their round durations.
- It implements a sudo entry point in case we eventually use the cron module to close rounds of all our games.