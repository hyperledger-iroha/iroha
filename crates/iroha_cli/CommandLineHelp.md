# Command-Line Help for `iroha`

This document contains the help content for the `iroha` command-line program.

**Command Overview:**

* [`iroha`↴](#iroha)
* [`iroha domain`↴](#iroha-domain)
* [`iroha domain list`↴](#iroha-domain-list)
* [`iroha domain list all`↴](#iroha-domain-list-all)
* [`iroha domain list filter`↴](#iroha-domain-list-filter)
* [`iroha domain get`↴](#iroha-domain-get)
* [`iroha domain register`↴](#iroha-domain-register)
* [`iroha domain unregister`↴](#iroha-domain-unregister)
* [`iroha domain transfer`↴](#iroha-domain-transfer)
* [`iroha domain meta`↴](#iroha-domain-meta)
* [`iroha domain meta get`↴](#iroha-domain-meta-get)
* [`iroha domain meta set`↴](#iroha-domain-meta-set)
* [`iroha domain meta remove`↴](#iroha-domain-meta-remove)
* [`iroha account`↴](#iroha-account)
* [`iroha account role`↴](#iroha-account-role)
* [`iroha account role list`↴](#iroha-account-role-list)
* [`iroha account role grant`↴](#iroha-account-role-grant)
* [`iroha account role revoke`↴](#iroha-account-role-revoke)
* [`iroha account permission`↴](#iroha-account-permission)
* [`iroha account permission list`↴](#iroha-account-permission-list)
* [`iroha account permission grant`↴](#iroha-account-permission-grant)
* [`iroha account permission revoke`↴](#iroha-account-permission-revoke)
* [`iroha account list`↴](#iroha-account-list)
* [`iroha account list all`↴](#iroha-account-list-all)
* [`iroha account list filter`↴](#iroha-account-list-filter)
* [`iroha account get`↴](#iroha-account-get)
* [`iroha account register`↴](#iroha-account-register)
* [`iroha account unregister`↴](#iroha-account-unregister)
* [`iroha account meta`↴](#iroha-account-meta)
* [`iroha account meta get`↴](#iroha-account-meta-get)
* [`iroha account meta set`↴](#iroha-account-meta-set)
* [`iroha account meta remove`↴](#iroha-account-meta-remove)
* [`iroha asset`↴](#iroha-asset)
* [`iroha asset definition`↴](#iroha-asset-definition)
* [`iroha asset definition list`↴](#iroha-asset-definition-list)
* [`iroha asset definition list all`↴](#iroha-asset-definition-list-all)
* [`iroha asset definition list filter`↴](#iroha-asset-definition-list-filter)
* [`iroha asset definition get`↴](#iroha-asset-definition-get)
* [`iroha asset definition register`↴](#iroha-asset-definition-register)
* [`iroha asset definition unregister`↴](#iroha-asset-definition-unregister)
* [`iroha asset definition transfer`↴](#iroha-asset-definition-transfer)
* [`iroha asset definition meta`↴](#iroha-asset-definition-meta)
* [`iroha asset definition meta get`↴](#iroha-asset-definition-meta-get)
* [`iroha asset definition meta set`↴](#iroha-asset-definition-meta-set)
* [`iroha asset definition meta remove`↴](#iroha-asset-definition-meta-remove)
* [`iroha asset get`↴](#iroha-asset-get)
* [`iroha asset list`↴](#iroha-asset-list)
* [`iroha asset list all`↴](#iroha-asset-list-all)
* [`iroha asset list filter`↴](#iroha-asset-list-filter)
* [`iroha asset mint`↴](#iroha-asset-mint)
* [`iroha asset burn`↴](#iroha-asset-burn)
* [`iroha asset transfer`↴](#iroha-asset-transfer)
* [`iroha nft`↴](#iroha-nft)
* [`iroha nft get`↴](#iroha-nft-get)
* [`iroha nft list`↴](#iroha-nft-list)
* [`iroha nft list all`↴](#iroha-nft-list-all)
* [`iroha nft list filter`↴](#iroha-nft-list-filter)
* [`iroha nft register`↴](#iroha-nft-register)
* [`iroha nft unregister`↴](#iroha-nft-unregister)
* [`iroha nft transfer`↴](#iroha-nft-transfer)
* [`iroha nft getkv`↴](#iroha-nft-getkv)
* [`iroha nft setkv`↴](#iroha-nft-setkv)
* [`iroha nft removekv`↴](#iroha-nft-removekv)
* [`iroha peer`↴](#iroha-peer)
* [`iroha peer list`↴](#iroha-peer-list)
* [`iroha peer list all`↴](#iroha-peer-list-all)
* [`iroha peer register`↴](#iroha-peer-register)
* [`iroha peer unregister`↴](#iroha-peer-unregister)
* [`iroha events`↴](#iroha-events)
* [`iroha events state`↴](#iroha-events-state)
* [`iroha events transaction`↴](#iroha-events-transaction)
* [`iroha events block`↴](#iroha-events-block)
* [`iroha events trigger-execute`↴](#iroha-events-trigger-execute)
* [`iroha events trigger-complete`↴](#iroha-events-trigger-complete)
* [`iroha blocks`↴](#iroha-blocks)
* [`iroha multisig`↴](#iroha-multisig)
* [`iroha multisig list`↴](#iroha-multisig-list)
* [`iroha multisig list all`↴](#iroha-multisig-list-all)
* [`iroha multisig register`↴](#iroha-multisig-register)
* [`iroha multisig propose`↴](#iroha-multisig-propose)
* [`iroha multisig approve`↴](#iroha-multisig-approve)
* [`iroha query`↴](#iroha-query)
* [`iroha query stdin`↴](#iroha-query-stdin)
* [`iroha transaction`↴](#iroha-transaction)
* [`iroha transaction get`↴](#iroha-transaction-get)
* [`iroha transaction ping`↴](#iroha-transaction-ping)
* [`iroha transaction wasm`↴](#iroha-transaction-wasm)
* [`iroha transaction stdin`↴](#iroha-transaction-stdin)
* [`iroha role`↴](#iroha-role)
* [`iroha role permission`↴](#iroha-role-permission)
* [`iroha role permission list`↴](#iroha-role-permission-list)
* [`iroha role permission grant`↴](#iroha-role-permission-grant)
* [`iroha role permission revoke`↴](#iroha-role-permission-revoke)
* [`iroha role list`↴](#iroha-role-list)
* [`iroha role list all`↴](#iroha-role-list-all)
* [`iroha role register`↴](#iroha-role-register)
* [`iroha role unregister`↴](#iroha-role-unregister)
* [`iroha parameter`↴](#iroha-parameter)
* [`iroha parameter list`↴](#iroha-parameter-list)
* [`iroha parameter list all`↴](#iroha-parameter-list-all)
* [`iroha parameter set`↴](#iroha-parameter-set)
* [`iroha trigger`↴](#iroha-trigger)
* [`iroha trigger list`↴](#iroha-trigger-list)
* [`iroha trigger list all`↴](#iroha-trigger-list-all)
* [`iroha trigger get`↴](#iroha-trigger-get)
* [`iroha trigger register`↴](#iroha-trigger-register)
* [`iroha trigger unregister`↴](#iroha-trigger-unregister)
* [`iroha trigger mint`↴](#iroha-trigger-mint)
* [`iroha trigger burn`↴](#iroha-trigger-burn)
* [`iroha trigger meta`↴](#iroha-trigger-meta)
* [`iroha trigger meta get`↴](#iroha-trigger-meta-get)
* [`iroha trigger meta set`↴](#iroha-trigger-meta-set)
* [`iroha trigger meta remove`↴](#iroha-trigger-meta-remove)
* [`iroha executor`↴](#iroha-executor)
* [`iroha executor data-model`↴](#iroha-executor-data-model)
* [`iroha executor upgrade`↴](#iroha-executor-upgrade)
* [`iroha markdown-help`↴](#iroha-markdown-help)

## `iroha`

Iroha Client CLI provides a simple way to interact with the Iroha Web API

**Usage:** `iroha [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `domain` — Read and write domains
* `account` — Read and write accounts
* `asset` — Read and write assets
* `nft` — Read and write NFTs
* `peer` — Read and write peers
* `events` — Subscribe to events: state changes, transaction/block/trigger progress
* `blocks` — Subscribe to blocks
* `multisig` — Read and write multi-signature accounts and transactions
* `query` — Read various data
* `transaction` — Read transactions and write various data
* `role` — Read and write roles
* `parameter` — Read and write system parameters
* `trigger` — Read and write triggers
* `executor` — Read and write the executor
* `markdown-help` — Output CLI documentation in Markdown format

###### **Options:**

* `-c`, `--config <PATH>` — Path to the configuration file.

   By default, `iroha` will try to read `client.toml` file, but would proceed if it is not found.
* `-v`, `--verbose` — Print configuration details to stderr
* `-m`, `--metadata <PATH>` — Path to a JSON5 file for attaching transaction metadata (optional)
* `-i`, `--input` — Reads instructions from stdin and appends new ones.

   Example usage:

   `echo "[]" | iroha -io domain register --id "domain" | iroha -i asset definition register --id "asset#domain" -t Numeric`
* `-o`, `--output` — Outputs instructions to stdout without submitting them.

   Example usage:

   `iroha -o domain register --id "domain" | iroha -io asset definition register --id "asset#domain" -t Numeric | iroha transaction stdin`



## `iroha domain`

Read and write domains

**Usage:** `iroha domain <COMMAND>`

###### **Subcommands:**

* `list` — List domains
* `get` — Retrieve details of a specific domain
* `register` — Register a domain
* `unregister` — Unregister a domain
* `transfer` — Transfer ownership of a domain
* `meta` — Read and write metadata



## `iroha domain list`

List domains

**Usage:** `iroha domain list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha domain list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha domain list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs



## `iroha domain list filter`

Filter by a given predicate

**Usage:** `iroha domain list filter <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Filtering condition specified as a JSON5 string



## `iroha domain get`

Retrieve details of a specific domain

**Usage:** `iroha domain get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name



## `iroha domain register`

Register a domain

**Usage:** `iroha domain register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name



## `iroha domain unregister`

Unregister a domain

**Usage:** `iroha domain unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name



## `iroha domain transfer`

Transfer ownership of a domain

**Usage:** `iroha domain transfer --id <ID> --from <FROM> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name
* `-f`, `--from <FROM>` — Source account, in the format "multihash@domain"
* `-t`, `--to <TO>` — Destination account, in the format "multihash@domain"



## `iroha domain meta`

Read and write metadata

**Usage:** `iroha domain meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON5 input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha domain meta get`

Retrieve a value from the key-value store

**Usage:** `iroha domain meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha domain meta set`

Create or update an entry in the key-value store using JSON5 input from stdin

**Usage:** `iroha domain meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha domain meta remove`

Delete an entry from the key-value store

**Usage:** `iroha domain meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha account`

Read and write accounts

**Usage:** `iroha account <COMMAND>`

###### **Subcommands:**

* `role` — Read and write account roles
* `permission` — Read and write account permissions
* `list` — List accounts
* `get` — Retrieve details of a specific account
* `register` — Register an account
* `unregister` — Unregister an account
* `meta` — Read and write metadata



## `iroha account role`

Read and write account roles

**Usage:** `iroha account role <COMMAND>`

###### **Subcommands:**

* `list` — List account role IDs
* `grant` — Grant a role to an account
* `revoke` — Revoke a role from an account



## `iroha account role list`

List account role IDs

**Usage:** `iroha account role list --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in the format "multihash@domain"



## `iroha account role grant`

Grant a role to an account

**Usage:** `iroha account role grant --id <ID> --role <ROLE>`

###### **Options:**

* `-i`, `--id <ID>` — Account in the format "multihash@domain"
* `-r`, `--role <ROLE>` — Role name



## `iroha account role revoke`

Revoke a role from an account

**Usage:** `iroha account role revoke --id <ID> --role <ROLE>`

###### **Options:**

* `-i`, `--id <ID>` — Account in the format "multihash@domain"
* `-r`, `--role <ROLE>` — Role name



## `iroha account permission`

Read and write account permissions

**Usage:** `iroha account permission <COMMAND>`

###### **Subcommands:**

* `list` — List account permissions
* `grant` — Grant an account permission using JSON5 input from stdin
* `revoke` — Revoke an account permission using JSON5 input from stdin



## `iroha account permission list`

List account permissions

**Usage:** `iroha account permission list --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in the format "multihash@domain"



## `iroha account permission grant`

Grant an account permission using JSON5 input from stdin

**Usage:** `iroha account permission grant --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in the format "multihash@domain"



## `iroha account permission revoke`

Revoke an account permission using JSON5 input from stdin

**Usage:** `iroha account permission revoke --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in the format "multihash@domain"



## `iroha account list`

List accounts

**Usage:** `iroha account list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha account list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha account list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs



## `iroha account list filter`

Filter by a given predicate

**Usage:** `iroha account list filter <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Filtering condition specified as a JSON5 string



## `iroha account get`

Retrieve details of a specific account

**Usage:** `iroha account get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in the format "multihash@domain"



## `iroha account register`

Register an account

**Usage:** `iroha account register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in the format "multihash@domain"



## `iroha account unregister`

Unregister an account

**Usage:** `iroha account unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in the format "multihash@domain"



## `iroha account meta`

Read and write metadata

**Usage:** `iroha account meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON5 input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha account meta get`

Retrieve a value from the key-value store

**Usage:** `iroha account meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha account meta set`

Create or update an entry in the key-value store using JSON5 input from stdin

**Usage:** `iroha account meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha account meta remove`

Delete an entry from the key-value store

**Usage:** `iroha account meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha asset`

Read and write assets

**Usage:** `iroha asset <COMMAND>`

###### **Subcommands:**

* `definition` — Read and write asset definitions
* `get` — Retrieve details of a specific asset
* `list` — List assets
* `mint` — Increase the quantity of an asset
* `burn` — Decrease the quantity of an asset
* `transfer` — Transfer an asset between accounts



## `iroha asset definition`

Read and write asset definitions

**Usage:** `iroha asset definition <COMMAND>`

###### **Subcommands:**

* `list` — List asset definitions
* `get` — Retrieve details of a specific asset definition
* `register` — Register an asset definition
* `unregister` — Unregister an asset definition
* `transfer` — Transfer ownership of an asset definition
* `meta` — Read and write metadata



## `iroha asset definition list`

List asset definitions

**Usage:** `iroha asset definition list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha asset definition list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha asset definition list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs



## `iroha asset definition list filter`

Filter by a given predicate

**Usage:** `iroha asset definition list filter <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Filtering condition specified as a JSON5 string



## `iroha asset definition get`

Retrieve details of a specific asset definition

**Usage:** `iroha asset definition get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition in the format "asset#domain"



## `iroha asset definition register`

Register an asset definition

**Usage:** `iroha asset definition register [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition in the format "asset#domain"
* `-m`, `--mint-once` — Disables minting after the first instance
* `-s`, `--scale <SCALE>` — Numeric scale of the asset. No value means unconstrained



## `iroha asset definition unregister`

Unregister an asset definition

**Usage:** `iroha asset definition unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition in the format "asset#domain"



## `iroha asset definition transfer`

Transfer ownership of an asset definition

**Usage:** `iroha asset definition transfer --id <ID> --from <FROM> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition in the format "asset#domain"
* `-f`, `--from <FROM>` — Source account, in the format "multihash@domain"
* `-t`, `--to <TO>` — Destination account, in the format "multihash@domain"



## `iroha asset definition meta`

Read and write metadata

**Usage:** `iroha asset definition meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON5 input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha asset definition meta get`

Retrieve a value from the key-value store

**Usage:** `iroha asset definition meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha asset definition meta set`

Create or update an entry in the key-value store using JSON5 input from stdin

**Usage:** `iroha asset definition meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha asset definition meta remove`

Delete an entry from the key-value store

**Usage:** `iroha asset definition meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha asset get`

Retrieve details of a specific asset

**Usage:** `iroha asset get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in the format "asset##account@domain" or "asset#another_domain#account@domain"



## `iroha asset list`

List assets

**Usage:** `iroha asset list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha asset list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha asset list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs



## `iroha asset list filter`

Filter by a given predicate

**Usage:** `iroha asset list filter <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Filtering condition specified as a JSON5 string



## `iroha asset mint`

Increase the quantity of an asset

**Usage:** `iroha asset mint --id <ID> --quantity <QUANTITY>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in the format "asset##account@domain" or "asset#another_domain#account@domain"
* `-q`, `--quantity <QUANTITY>` — Amount of change (integer or decimal)



## `iroha asset burn`

Decrease the quantity of an asset

**Usage:** `iroha asset burn --id <ID> --quantity <QUANTITY>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in the format "asset##account@domain" or "asset#another_domain#account@domain"
* `-q`, `--quantity <QUANTITY>` — Amount of change (integer or decimal)



## `iroha asset transfer`

Transfer an asset between accounts

**Usage:** `iroha asset transfer --id <ID> --to <TO> --quantity <QUANTITY>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in the format "asset##account@domain" or "asset#another_domain#account@domain"
* `-t`, `--to <TO>` — Destination account, in the format "multihash@domain"
* `-q`, `--quantity <QUANTITY>` — Transfer amount (integer or decimal)



## `iroha nft`

Read and write NFTs

**Usage:** `iroha nft <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve details of a specific NFT
* `list` — List NFTs
* `register` — Register NFT with content provided from stdin in JSON5 format
* `unregister` — Unregister NFT
* `transfer` — Transfer ownership of NFT
* `getkv` — Get a value from NFT
* `setkv` — Create or update a key-value entry of NFT using JSON5 input from stdin
* `removekv` — Remove a key-value entry from NFT



## `iroha nft get`

Retrieve details of a specific NFT

**Usage:** `iroha nft get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"



## `iroha nft list`

List NFTs

**Usage:** `iroha nft list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha nft list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha nft list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs



## `iroha nft list filter`

Filter by a given predicate

**Usage:** `iroha nft list filter <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Filtering condition specified as a JSON5 string



## `iroha nft register`

Register NFT with content provided from stdin in JSON5 format

**Usage:** `iroha nft register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"



## `iroha nft unregister`

Unregister NFT

**Usage:** `iroha nft unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"



## `iroha nft transfer`

Transfer ownership of NFT

**Usage:** `iroha nft transfer --id <ID> --from <FROM> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"
* `-f`, `--from <FROM>` — Source account, in the format "multihash@domain"
* `-t`, `--to <TO>` — Destination account, in the format "multihash@domain"



## `iroha nft getkv`

Get a value from NFT

**Usage:** `iroha nft getkv --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"
* `-k`, `--key <KEY>`



## `iroha nft setkv`

Create or update a key-value entry of NFT using JSON5 input from stdin

**Usage:** `iroha nft setkv --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"
* `-k`, `--key <KEY>`



## `iroha nft removekv`

Remove a key-value entry from NFT

**Usage:** `iroha nft removekv --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"
* `-k`, `--key <KEY>`



## `iroha peer`

Read and write peers

**Usage:** `iroha peer <COMMAND>`

###### **Subcommands:**

* `list` — List registered peers expected to connect with each other
* `register` — Register a peer
* `unregister` — Unregister a peer



## `iroha peer list`

List registered peers expected to connect with each other

**Usage:** `iroha peer list <COMMAND>`

###### **Subcommands:**

* `all` — List all registered peers



## `iroha peer list all`

List all registered peers

**Usage:** `iroha peer list all`



## `iroha peer register`

Register a peer

**Usage:** `iroha peer register --key <KEY>`

###### **Options:**

* `-k`, `--key <KEY>` — Peer's public key in multihash format



## `iroha peer unregister`

Unregister a peer

**Usage:** `iroha peer unregister --key <KEY>`

###### **Options:**

* `-k`, `--key <KEY>` — Peer's public key in multihash format



## `iroha events`

Subscribe to events: state changes, transaction/block/trigger progress

**Usage:** `iroha events [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `state` — Notify when the world state undergoes certain changes
* `transaction` — Notify when a transaction reaches specific stages
* `block` — Notify when a block reaches specific stages
* `trigger-execute` — Notify when a trigger execution is ordered
* `trigger-complete` — Notify when a trigger execution is completed

###### **Options:**

* `-t`, `--timeout <TIMEOUT>` — Duration to listen for events. Example: "1y 6M 2w 3d 12h 30m 30s"



## `iroha events state`

Notify when the world state undergoes certain changes

**Usage:** `iroha events state`



## `iroha events transaction`

Notify when a transaction reaches specific stages

**Usage:** `iroha events transaction`



## `iroha events block`

Notify when a block reaches specific stages

**Usage:** `iroha events block`



## `iroha events trigger-execute`

Notify when a trigger execution is ordered

**Usage:** `iroha events trigger-execute`



## `iroha events trigger-complete`

Notify when a trigger execution is completed

**Usage:** `iroha events trigger-complete`



## `iroha blocks`

Subscribe to blocks

**Usage:** `iroha blocks [OPTIONS] <HEIGHT>`

###### **Arguments:**

* `<HEIGHT>` — Block height from which to start streaming blocks

###### **Options:**

* `-t`, `--timeout <TIMEOUT>` — Duration to listen for events. Example: "1y 6M 2w 3d 12h 30m 30s"



## `iroha multisig`

Read and write multi-signature accounts and transactions.

See the [usage guide](./docs/multisig.md) for details

**Usage:** `iroha multisig <COMMAND>`

###### **Subcommands:**

* `list` — List pending multisig transactions relevant to you
* `register` — Register a multisig account
* `propose` — Propose a multisig transaction using JSON5 input from stdin
* `approve` — Approve a multisig transaction



## `iroha multisig list`

List pending multisig transactions relevant to you

**Usage:** `iroha multisig list <COMMAND>`

###### **Subcommands:**

* `all` — List all pending multisig transactions relevant to you



## `iroha multisig list all`

List all pending multisig transactions relevant to you

**Usage:** `iroha multisig list all`



## `iroha multisig register`

Register a multisig account

**Usage:** `iroha multisig register [OPTIONS] --account <ACCOUNT> --quorum <QUORUM>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — ID of the multisig account to be registered
* `-s`, `--signatories <SIGNATORIES>` — List of signatories for the multisig account
* `-w`, `--weights <WEIGHTS>` — Relative weights of signatories' responsibilities
* `-q`, `--quorum <QUORUM>` — Threshold of total weight required for authentication
* `-t`, `--transaction-ttl <TRANSACTION_TTL>` — Time-to-live for multisig transactions. Example: "1y 6M 2w 3d 12h 30m 30s"

  Default value: `1h`



## `iroha multisig propose`

Propose a multisig transaction using JSON5 input from stdin

**Usage:** `iroha multisig propose [OPTIONS] --account <ACCOUNT>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — Multisig authority managing the proposed transaction
* `-t`, `--transaction-ttl <TRANSACTION_TTL>` — Overrides the default time-to-live for this transaction. Example: "1y 6M 2w 3d 12h 30m 30s"



## `iroha multisig approve`

Approve a multisig transaction

**Usage:** `iroha multisig approve --account <ACCOUNT> --instructions-hash <INSTRUCTIONS_HASH>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — Multisig authority of the transaction
* `-i`, `--instructions-hash <INSTRUCTIONS_HASH>` — Hash of the instructions to approve



## `iroha query`

Read various data

**Usage:** `iroha query <COMMAND>`

###### **Subcommands:**

* `stdin` — Query using JSON5 input from stdin



## `iroha query stdin`

Query using JSON5 input from stdin

**Usage:** `iroha query stdin`



## `iroha transaction`

Read transactions and write various data

**Usage:** `iroha transaction <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve details of a specific transaction
* `ping` — Send an empty transaction that logs a message
* `wasm` — Send a transaction using Wasm input
* `stdin` — Send a transaction using JSON5 input from stdin



## `iroha transaction get`

Retrieve details of a specific transaction

**Usage:** `iroha transaction get --hash <HASH>`

###### **Options:**

* `-H`, `--hash <HASH>` — Hash of the transaction to retrieve



## `iroha transaction ping`

Send an empty transaction that logs a message

**Usage:** `iroha transaction ping [OPTIONS] --msg <MSG>`

###### **Options:**

* `-l`, `--log-level <LOG_LEVEL>` — Log levels: TRACE, DEBUG, INFO, WARN, ERROR (in increasing order of visibility)

  Default value: `INFO`
* `-m`, `--msg <MSG>` — Log message



## `iroha transaction wasm`

Send a transaction using Wasm input

**Usage:** `iroha transaction wasm [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the Wasm file. If omitted, reads from stdin



## `iroha transaction stdin`

Send a transaction using JSON5 input from stdin

**Usage:** `iroha transaction stdin`



## `iroha role`

Read and write roles

**Usage:** `iroha role <COMMAND>`

###### **Subcommands:**

* `permission` — Read and write role permissions
* `list` — List role IDs
* `register` — Register a role and grant it to the registrant
* `unregister` — Unregister a role



## `iroha role permission`

Read and write role permissions

**Usage:** `iroha role permission <COMMAND>`

###### **Subcommands:**

* `list` — List role permissions
* `grant` — Grant role permission using JSON5 input from stdin
* `revoke` — Revoke role permission using JSON5 input from stdin



## `iroha role permission list`

List role permissions

**Usage:** `iroha role permission list --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha role permission grant`

Grant role permission using JSON5 input from stdin

**Usage:** `iroha role permission grant --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha role permission revoke`

Revoke role permission using JSON5 input from stdin

**Usage:** `iroha role permission revoke --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha role list`

List role IDs

**Usage:** `iroha role list <COMMAND>`

###### **Subcommands:**

* `all` — List all role IDs



## `iroha role list all`

List all role IDs

**Usage:** `iroha role list all`



## `iroha role register`

Register a role and grant it to the registrant

**Usage:** `iroha role register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha role unregister`

Unregister a role

**Usage:** `iroha role unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha parameter`

Read and write system parameters

**Usage:** `iroha parameter <COMMAND>`

###### **Subcommands:**

* `list` — List system parameters
* `set` — Set a system parameter using JSON5 input from stdin



## `iroha parameter list`

List system parameters

**Usage:** `iroha parameter list <COMMAND>`

###### **Subcommands:**

* `all` — List all system parameters



## `iroha parameter list all`

List all system parameters

**Usage:** `iroha parameter list all`



## `iroha parameter set`

Set a system parameter using JSON5 input from stdin

**Usage:** `iroha parameter set`



## `iroha trigger`

Read and write triggers

**Usage:** `iroha trigger <COMMAND>`

###### **Subcommands:**

* `list` — List trigger IDs
* `get` — Retrieve details of a specific trigger
* `register` — TODO: Register a trigger
* `unregister` — Unregister a trigger
* `mint` — Increase the number of trigger executions
* `burn` — Decrease the number of trigger executions
* `meta` — Read and write metadata



## `iroha trigger list`

List trigger IDs

**Usage:** `iroha trigger list <COMMAND>`

###### **Subcommands:**

* `all` — List all trigger IDs



## `iroha trigger list all`

List all trigger IDs

**Usage:** `iroha trigger list all`



## `iroha trigger get`

Retrieve details of a specific trigger

**Usage:** `iroha trigger get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name



## `iroha trigger register`

TODO: Register a trigger

**Usage:** `iroha trigger register`



## `iroha trigger unregister`

Unregister a trigger

**Usage:** `iroha trigger unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name



## `iroha trigger mint`

Increase the number of trigger executions

**Usage:** `iroha trigger mint --id <ID> --repetitions <REPETITIONS>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name
* `-r`, `--repetitions <REPETITIONS>` — Amount of change (integer)



## `iroha trigger burn`

Decrease the number of trigger executions

**Usage:** `iroha trigger burn --id <ID> --repetitions <REPETITIONS>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name
* `-r`, `--repetitions <REPETITIONS>` — Amount of change (integer)



## `iroha trigger meta`

Read and write metadata

**Usage:** `iroha trigger meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON5 input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha trigger meta get`

Retrieve a value from the key-value store

**Usage:** `iroha trigger meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha trigger meta set`

Create or update an entry in the key-value store using JSON5 input from stdin

**Usage:** `iroha trigger meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha trigger meta remove`

Delete an entry from the key-value store

**Usage:** `iroha trigger meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha executor`

Read and write the executor

**Usage:** `iroha executor <COMMAND>`

###### **Subcommands:**

* `data-model` — Retrieve the executor data model
* `upgrade` — Upgrade the executor



## `iroha executor data-model`

Retrieve the executor data model

**Usage:** `iroha executor data-model`



## `iroha executor upgrade`

Upgrade the executor

**Usage:** `iroha executor upgrade --path <PATH>`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the compiled Wasm file



## `iroha markdown-help`

Output CLI documentation in Markdown format

**Usage:** `iroha markdown-help`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

