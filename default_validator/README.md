# `iroha_default_validator`

Use the [Wasm Builder CLI](../tools/wasm_builder_cli) in order to build it:

```bash
cargo run --bin iroha_wasm_builder_cli -- \
  build ./default_validator --optimize --outfile ./configs/peer/validator.wasm
```