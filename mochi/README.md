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

On a clean launch, Mochi now opens a first-run wizard instead of dropping you
into the raw ops view. The default home is the Dashboard, which surfaces:

- prefunded dev accounts and explorer balances;
- recent blocks and one-click composer actions;
- copyable local shell exports for app bootstrap;
- generated bootstrap files in `.env.local` and `.mochi/generated/*`; and
- a Chaos Lab tab for quick peer bounce / partition / slowdown drills against
  the current supervised sandbox.

The Network page still exposes the lower-level launch recipe, app env snippet,
and `/status` curl probe so the same setup can move between GUI and shell
without reconstructing the config by hand.
