# Profiling `iroha_data_model` Build

To locate slow build steps in `iroha_data_model`, run the helper script:

```sh
./scripts/profile_build.sh
```

This runs `cargo build -p iroha_data_model --timings` and writes timing reports to `target/cargo-timings/`.
Open `cargo-timing.html` in a browser and sort tasks by duration to see which crates or build steps take the most time.

Use the timings to focus optimization efforts on the slowest tasks.
