# Games Contracts

Repository that will contain all games

## Contracts

| Name                                                 | Description           |
| ---------------------------------------------------- | --------------------- |
| [`manager`](contracts/managers/manager)              | Game manager contract |
| [`prediction_game`](contracts/games/prediction-game) | Prediction contract   |
| [`dao-bets`](contracts/games/dao-bets-game)          | DAO governed bets     |
| [`users`](contracts/others/users)                    | Users contract        |

### You can compile each contract:

Go to contract directory and run

```
cargo wasm
```

### For a production-ready (compressed) build:

Run the following from the repository root

```
    docker run --rm -v "$(pwd)":/code \
        --mount type=volume,source="$(basename "$(pwd)")_cache",target=/target \
        --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
        cosmwasm/optimizer:0.16.0
```

The optimized contracts are generated in the artifacts/ directory.
