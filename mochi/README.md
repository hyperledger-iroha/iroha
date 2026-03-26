# MOCHI Development Notes

## Regression Checks

Run the following commands from the workspace root before submitting changes to MOCHI components:

```sh
cargo check -p mochi-core -p mochi-ui-egui -p mochi-integration
cargo test -p mochi-integration
```

The `mochi-integration` crate provides lightweight Torii mocks and supervisor smoke tests so we can validate local workflows without compiling the full Iroha binary set.

## Fast Local Loop

For the desktop shell itself, the quickest happy path is:

```sh
cargo run -p mochi-ui-egui -- --profile single-peer --build-binaries
```

Use `--profile four-peer-bft` when you want a closer validator/quorum rehearsal.
The Network page now exposes copyable launch recipes, app bootstrap env snippets,
and `/status` curl probes so local-app setup can move from the GUI into a shell
without reconstructing the config by hand.
