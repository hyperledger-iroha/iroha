# Kagami (Teacher and Exemplar and/or Looking glass)

Kagami is a tool used to generate and validate automatically generated data files that are shipped with Iroha.

## Build

From anywhere in the repository, run:

```bash
cargo build --bin kagami
```

This will place `kagami` inside the `target/debug/` directory (from the root of the repository).

## Usage

### Subcommands

| Command                                             | Description                                                                                                                        |
|-----------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------|
| `crypto`                         | Generate cryptographic key pairs using the given algorithm and either private key or seed                                                                                                      |
| `schema` | Generate the schema used for code generation in Iroha SDKs                                                                                           |
| [`genesis`](src/genesis/README.md) | Commands related to genesis                                                                                            |
| [`codec`](src/codec/README.md)                  | Commands related to codec |
| `help`                                              | Print the help message for the tool or a subcommand   

Run:

```bash
kagami --help
```
