# Users

This contract is responsible for managing the users information and their XP/ELO. It will be used to add the XP/ELO to the players during all games that are registered to it.

For a game to be able to add XP/ELO for a specific user, it needs to be registered in the users contract using the `add_game` functionality. This will allow the game to send messages to the contract updating users.

In the same way, if a game should not be allowed to keep updating the users, it can be removed from the users contract using the `remove_game` functionality.

The contract is instantiated with the following information, which can be updated by the owner of the contract:

```rust
pub struct Config {
    // How much exp is needed to get from lvl 0 to lvl 1
    pub initial_exp_per_level: u64,
    // Increase needed per level
    // If initial_exp_per_level is 100 and exp_increase_per_level is 10, then the exp needed to get from lvl 1 to lvl 2 is 110
    pub exp_increase_per_level: u64,
}
```

As described in the comments, this information will be used to calculate the XP needed to get from one level to another. The XP needed to get from level 0 to level 1 is `initial_exp_per_level`. The XP needed to get from level 1 to level 2 is `initial_exp_per_level + exp_increase_per_level` and so on.

A user can modify their user information (except ELO, XP, creation date and address) as long as all the information passed is valid (passes all the censorship checks and the username is not already taken).

## Owner actions

Owners can perform special actions to a specific like verifying it (similar to some other social network verification processes), and maybe more in the future.
Owners can also perform a global elo reset (similar to seasons in videogames, where the elo gets reset or substracted a certain amount). XP will never be reset or modified.

## Games actions

If a game is registered in the contract, it means it can modify users ELO/XP. If for some reason, the user has not created a profile because he doesn't want to, we will accumulate all his XP and ELO in a "ghost" profile tied to his address. If the user decides later on to create a profile, all his XP and ELO will be added to his profile.
