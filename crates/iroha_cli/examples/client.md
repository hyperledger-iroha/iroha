# Examples for `iroha client`

In this section we will show you how to use Iroha CLI Client to do the following:

  - [Create new Domain](#create-new-domain)
  - [Create new Account](#create-new-account)
  - [Mint Asset to Account](#mint-asset-to-account)
  - [Query Account Assets Quantity](#query-account-assets-quantity)
  - [Execute WASM transaction](#execute-wasm-transaction)
  - [Execute Multi-instruction Transactions](#execute-multi-instruction-transactions)

### Create new Domain

To create a domain, you need to specify the entity type first (`domain` in our case) and then the command (`register`) with a list of required parameters. For the `domain` entity, you only need to provide the `id` argument as a string that doesn't contain the `@` and `#` symbols.

```bash
./iroha client domain register --id="Soramitsu"
```

### Create new Account

To create an account, specify the entity type (`account`) and the command (`register`). Then define the value of the `id` argument in "signatory@domain" format, where signatory is the account's public key in multihash representation:

```bash
./iroha client account register --id="ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu"
```

### Mint Asset to Account

To add assets to the account, you must first register an Asset Definition. Specify the `asset` entity and then use the `register` and `mint` commands respectively. Here is an example of adding Assets of the type `Quantity` to the account:

```bash
./iroha client asset register --id="XOR#Soramitsu" --type=Numeric
./iroha client asset mint --account="ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu" --asset="XOR#Soramitsu" --quantity=1010
```

With this, you created `XOR#Soramitsu`, an asset of type `Numeric`, and then gave `1010` units of this asset to the account `ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu`.

### Query Account Assets Quantity

You can use Query API to check that your instructions were applied and the _world_ is in the desired state. For example, to know how many units of a particular asset an account has, use `asset get` with the specified account and asset:

```bash
./iroha client asset get --account="ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu" --asset="XOR#Soramitsu"
```

This query returns the quantity of `XOR#Soramitsu` asset for the `ed01204A3C5A6B77BBE439969F95F0AA4E01AE31EC45A0D68C131B2C622751FCC5E3B6@Soramitsu` account.

You can also filter based on either account, asset or domain id by using the filtering API provided by the Iroha client CLI. Generally, filtering follows the `./iroha client ENTITY list filter PREDICATE` pattern, where ENTITY is asset, account or domain and PREDICATE is condition used for filtering serialized using JSON5 (check `iroha::data_model::predicate::value::ValuePredicate` type).

Here are some examples of filtering:

```bash
# Filter domains by id
./iroha client domain list filter '{"Identifiable": {"Is": "wonderland"}}'
# Filter accounts by domain
./iroha client account list filter '{"Identifiable": {"EndsWith": "@wonderland"}}'
# Filter asset by domain
./iroha client asset list filter '{"Or": [{"Identifiable": {"Contains": "#wonderland#"}}, {"And": [{"Identifiable": {"Contains": "##"}}, {"Identifiable": {"EndsWith": "@wonderland"}}]}]}'
```

### Execute WASM transaction

Use `--file` to specify a path to the WASM file:

```bash
./iroha client wasm --file=/path/to/file.wasm
```

Or skip `--file` to read WASM from standard input:

```bash
cat /path/to/file.wasm | ./iroha client wasm
```

These subcommands submit the provided wasm binary as an `Executable` to be executed outside a trigger context.

### Execute Multi-instruction Transactions

The reference implementation of the Rust client, `iroha`, is often used for diagnosing problems in other implementations.

To test transactions in the JSON format (used in the genesis block and by other SDKs), pipe the transaction into the client and add the `json` subcommand to the arguments:

```bash
cat /path/to/file.json | ./iroha client json transaction
```

### Request arbitrary query

```bash
echo '{ "FindAllParameters": null }' | ./iroha client --config client.toml json query
```
