# Iroha CLI Client

Iroha Client CLI is a "thin" wrapper around functionality exposed in the `iroha` crate. Specifically, it should be used as a reference for using `iroha`'s features, and not as a production-ready client. As such, the CLI client is not guaranteed to support all features supported by the client library. Check [Iroha 2 documentation](https://docs.iroha.tech/get-started/operate-iroha-2-via-cli.html) for a detailed tutorial on working with Iroha Client CLI.

## Installation

**Requirements:** a working [Rust toolchain](https://www.rust-lang.org/learn/get-started) (version 1.62.1), installed and configured.

Build Iroha and its binaries:

```bash
cargo build
```

The above command will produce the `iroha` ELF executable file for Linux/BSD, the `iroha` executable for MacOS, and the `iroha.exe` executable for Windows, depending on your platform and configuration.

Alternatively, check out the [documentation](https://docs.iroha.tech/get-started/install-iroha-2.html) for system-wide installation instructions.

## Usage

See [Command-Line Help](CommandLineHelp.md).

Refer to [Iroha Special Instructions](https://docs.iroha.tech/blockchain/instructions.html) for more information about Iroha instructions such as register, mint, grant, and so on.

## Examples

:grey_exclamation: All examples below are Unix-oriented. If you're working on Windows, we would highly encourage you to consider using WSL, as most documentation assumes a POSIX-like shell running on your system. Please be advised that the differences in the syntax may go beyond executing `iroha.exe` instead of `iroha`.

### Create new Domain

To create a domain, you need to specify the entity type first (`domain` in our case) and then the command (`register`) with a list of required parameters. For the `domain` entity, you only need to provide the `id` argument as a string that doesn't contain the `@`, `#` or `$` symbols.

```bash
iroha domain register --id "Soramitsu"
```

### Create new Account

To create an account, specify the entity type (`account`) and the command (`register`). Then define the value of the `id` argument in "signatory@domain" format, where signatory is the account's public key in multihash representation:

```bash
iroha account register --id "ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu"
```

### Mint Asset to Account

To add assets to the account, you must first register an Asset Definition. Specify the `asset` entity and then use the `register` and `mint` commands respectively. Here is an example of adding Assets of the type `Quantity` to the account:

```bash
iroha asset register --id "XOR#Soramitsu" --type Numeric
iroha asset mint --id "XOR##ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu" --quantity 1010
```

With this, you created `XOR#Soramitsu`, an asset of type `Numeric`, and then gave `1010` units of this asset to the account `ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu`.

### Query Account Assets Quantity

You can use Query API to check that your instructions were applied and the _world_ is in the desired state. For example, to know how many units of a particular asset an account has, use `asset get` with the specified account and asset:

```bash
iroha asset get --id "XOR##ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu"
```

This query returns the quantity of `XOR#Soramitsu` asset for the `ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu` account.

You can also filter based on either account, asset or domain id by using the filtering API provided by the Iroha client CLI. Generally, filtering follows the `iroha ENTITY list filter PREDICATE` pattern, where ENTITY is asset, account or domain and PREDICATE is condition used for filtering serialized using JSON5 (check `iroha::data_model::predicate::value::ValuePredicate` type).

Here are some examples of filtering:

```bash
# Filter domains by id
iroha domain list filter '{"Atom": {"Id": {"Atom": {"Equals": "wonderland"}}}}'
# Filter accounts by domain
iroha account list filter '{"Atom": {"Id": {"Domain": {"Atom": {"Equals": "wonderland"}}}}}' 
# Filter asset by domain
iroha asset list filter '{"Or": [{"Atom": {"Id": {"Definition": {"Domain": {"Atom": {"Equals": "wonderland"}}}}}}, {"Atom": {"Id": {"Account": {"Domain": {"Atom": {"Equals": "wonderland"}}}}}}]}'
```

### Execute WASM transaction

Use `--file` to specify a path to the WASM file:

```bash
iroha transaction wasm --file /path/to/file.wasm
```

Or skip `--file` to read WASM from standard input:

```bash
cat /path/to/file.wasm | iroha transaction wasm
```

These subcommands submit the provided wasm binary as an `Executable` to be executed outside a trigger context.

### Execute Multi-instruction Transactions

The reference implementation of the Rust client, `iroha`, is often used for diagnosing problems in other implementations.

To test transactions in the JSON format (used in the genesis block and by other SDKs), pipe the transaction into the client and add the `transaction stdin` subcommand to the arguments:

```bash
cat samples/instructions.json | iroha transaction stdin
```

### Request arbitrary query

```bash
cat samples/query.json | iroha query stdin
```
