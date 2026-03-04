# MOCHI Development Notes

## Regression Checks

Run the following commands from the workspace root before submitting changes to MOCHI components:

```sh
cargo check -p mochi-core -p mochi-ui-egui -p mochi-integration
cargo test -p mochi-integration
```

The `mochi-integration` crate provides lightweight Torii mocks and supervisor smoke tests so we can validate local workflows without compiling the full Iroha binary set.
