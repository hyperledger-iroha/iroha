# Kagami (Teacher and Exemplar and/or Looking glass)

Kagami is a tool used to generate and validate automatically generated data files that are shipped with Iroha.

## Build

From anywhere in the repository, run:

```bash
cargo build --bin kagami
```

This will place `kagami` inside the `target/debug/` directory (from the root of the repository).

## Usage
See [Command-Line Help](CommandLineHelp.md).

## Examples
- [codec](docs/codec.md)
- [kura](docs/kura.md)
- [swarm](docs/swarm.md)
- [wasm](docs/wasm.md)