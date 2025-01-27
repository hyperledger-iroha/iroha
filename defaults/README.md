# Iroha2 Testnet

[peer]: https://hyperledger-iroha.github.io/iroha-2-docs/reference/glossary.html#peer
[account]: https://hyperledger-iroha.github.io/iroha-2-docs/guide/configure/client-configuration.html#user-account
[assets]: https://hyperledger-iroha.github.io/iroha-2-docs/reference/glossary.html#asset
[transactions]: https://hyperledger-iroha.github.io/iroha-2-docs/blockchain/transactions.html#transactions
[query]: https://hyperledger-iroha.github.io/iroha-2-docs/reference/glossary.html#iroha-query

This guide explains how to launch your own [peer] and operate your [account].

## Generate Your Key Pair and Declare the Public Key

To generate a unique key pair, run the utility tool included in the Docker image:

```bash
docker pull hyperledger/iroha:testnet-2.0.0-rc.1
docker run hyperledger/iroha:testnet-2.0.0-rc.1 kagami crypto
```

__Example output:__

```log
Public key (multihash): "ed0120CAA7C95F78150097932C3E1C62B89D73007C5F30D5907DD0FBE7EA09AF6658E2"
Private key (multihash): "8026205F4FD09D9F9C390B9E3B0DB7CFA3E8B8D567707227E549519CC0C170D87447B9"
```

Share your __public key__ with the administrator to register your [peer] and [account]. Ensure that your __private key__ remains confidential.

__Notes:__

* For testnet purposes, the same key pair is used for both the peer and the account. In production environments, using separate key pairs is recommended.
* In production environments, operating your own peer is not necessarily required; you can interact with public endpoints instead.
* Account registration is planned to be automatic in future releases.

## Launch Your Own Peer

### 1. Ensure a Static IP Address

Confirm that your machine or server is assigned a static, publicly accessible IP address. Most cloud providers enable this by default.

### 2. Configure Port Access

Ensure that port `1337` is open in your firewall settings, or configure your cloud provider’s security group to allow inbound traffic.

### 3. Update Docker Compose Configuration

Edit the `docker-compose.volunteer.yml` file as follows:

```yml
# For the attached client
ACCOUNT_PUBLIC_KEY: <your_public_key>
ACCOUNT_PRIVATE_KEY: <your_private_key>
# For the peer
PUBLIC_KEY: <your_public_key>
PRIVATE_KEY: <your_private_key>
P2P_PUBLIC_ADDRESS: <your_advertised_host>:1337
```

### 4. Start the Docker Container

Run the following command to launch your peer:

```bash
docker compose -f docker-compose.volunteer.yml up -d
```

### 5. Check Peer Status

Once your peer is registered, verify its status using one of the following commands:

```bash
curl <your_host>:8080/status
curl <your_host>:8080/peers
```

__Note:__ If the peer list is empty, your peer may not be registered, or there might be network issues.

## Perform [Transactions] via Your Peer

### 1. Send and Inspect a Mock Transaction

To listen for incoming transactions, attach a shell to the running container and run:

```bash
cd /config
iroha events transaction
```

In another shell, send a mock transaction:

```bash
cd /config
iroha transaction ping --msg "This is a mock transaction"
```

__Example output:__

```json
"23EC79207A5573333057A4836533A72ED015AADE4DABC00CA8676120C919DE67"
```

__Note:__ If the account is not found, your account may not be registered.

If the transaction listener is running, you should see a confirmation that the transaction has been approved:

```json
{
  "Pipeline": {
    "Transaction": {
      "hash": "23EC79207A5573333057A4836533A72ED015AADE4DABC00CA8676120C919DE67",
      "block_height": 2,
      "status": "Approved"
    }
  }
}
```

### 2. Query Transaction Details

Retrieve details of a specific transaction using its hash:

```bash
cd /config
iroha transaction get --hash "23EC79207A5573333057A4836533A72ED015AADE4DABC00CA8676120C919DE67"
```

__Example result:__

```json
{
  "block_hash": "377BB64FB105B66B9A518903C1F861E144DC552A1C695BC0E1D3C87DE004B6CD",
  "value": {
    "version": "1",
    "content": {
      ...
      "msg": "This is a mock transaction"
      ...
    }
  },
  "error": null
}
```

### 3. Transfer [Assets]

By default, your account receives an initial airdrop of 100 roses. Verify this with the following query:

```bash
cd /config
iroha asset get --id "rose##<your_public_key>@wonderland"
```

__Example result:__

```json
{
  "id": "rose##ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
  "value": {
    "Numeric": "100"
  }
}
```

To transfer roses to another account, run the following command and verify the updated balance:

```bash
cd /config
iroha asset transfer --id "rose##<your_public_key>@wonderland" --to "<another_public_key>@wonderland" --quantity 0.4
iroha asset get --id "rose##<your_public_key>@wonderland"
```

__Example result:__

```json
{
  "id": "rose##ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
  "value": {
    "Numeric": "99.6"
  }
}
```

For further information, consult the [Command-Line Help](../crates/iroha_cli/CommandLineHelp.md).
