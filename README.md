# Games Contracts

Repository that will contain all games

## Contracts

| Name                                   | Description             |
| -------------------------------------- | ----------------------- |
| [`prediction_game`](contracts/prediction_game)   | Prediction contract |
| [`users`](contracts/users)   | Users contract |

### You can compile each contract:

Go to contract directory and run

```
cargo wasm
```

### For a production-ready (compressed) build:

Run the following from the repository root

```
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.14.0
```

The optimized contracts are generated in the artifacts/ directory.
