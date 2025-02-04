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
* [`iroha asset transferkvs`↴](#iroha-asset-transferkvs)
* [`iroha asset getkv`↴](#iroha-asset-getkv)
* [`iroha asset setkv`↴](#iroha-asset-setkv)
* [`iroha asset removekv`↴](#iroha-asset-removekv)
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
* [`iroha trigger meta`↴](#iroha-trigger-meta)
* [`iroha trigger meta get`↴](#iroha-trigger-meta-get)
* [`iroha trigger meta set`↴](#iroha-trigger-meta-set)
* [`iroha trigger meta remove`↴](#iroha-trigger-meta-remove)
* [`iroha executor`↴](#iroha-executor)
* [`iroha executor upgrade`↴](#iroha-executor-upgrade)
* [`iroha markdown-help`↴](#iroha-markdown-help)

## `iroha`

Iroha CLI Client provides an ability to interact with Iroha Peers Web API without direct network usage

**Usage:** `iroha [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `domain` — Read/Write domains
* `account` — Read/Write accounts
* `asset` — Read/Write assets
* `peer` — Read/Write peers
* `events` — Subscribe events: state changes, status of transactions/blocks/triggers
* `blocks` — Subscribe blocks
* `multisig` — Read/Write multisig accounts and transactions
* `query` — Read in general
* `transaction` — Read transactions, Write in general
* `role` — Read/Write roles
* `parameter` — Read/Write parameters
* `trigger` — TODO Read/Write triggers
* `executor` — Update executor
* `markdown-help` — Dump a markdown help of this CLI to stdout

###### **Options:**

* `-c`, `--config <PATH>` — Path to the configuration file

  Default value: `client.toml`
* `-v`, `--verbose` — Print the config to stderr
* `-m`, `--metadata <PATH>` — Optional path to read a JSON5 file to attach transaction metadata
* `-a`, `--accumulate` — Whether to accumulate instructions into a single transaction: If specified, loads instructions from stdin, appends some, and returns them to stdout

   Usage: `echo "[]" | iroha -a domain register -i "domain" | iroha -a asset definition register -i "asset#domain" -t Numeric | iroha transaction stdin`



## `iroha domain`

Read/Write domains

**Usage:** `iroha domain <COMMAND>`

###### **Subcommands:**

* `list` — List domains
* `get` — Read a single domain details
* `register` — Register domain
* `unregister` — Unregister domain
* `transfer` — Transfer domain
* `meta` — Read/Write metadata



## `iroha domain list`

List domains

**Usage:** `iroha domain list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or entries when `--verbose` specified
* `filter` — Filter by given predicate



## `iroha domain list all`

List all IDs, or entries when `--verbose` specified

**Usage:** `iroha domain list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Show entry details, not only IDs



## `iroha domain list filter`

Filter by given predicate

**Usage:** `iroha domain list filter <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Predicate for filtering given as JSON5 string



## `iroha domain get`

Read a single domain details

**Usage:** `iroha domain get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name as double-quoted string



## `iroha domain register`

Register domain

**Usage:** `iroha domain register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name as double-quoted string



## `iroha domain unregister`

Unregister domain

**Usage:** `iroha domain unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name as double-quoted string



## `iroha domain transfer`

Transfer domain

**Usage:** `iroha domain transfer --id <ID> --from <FROM> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name as double-quoted string
* `-f`, `--from <FROM>` — Account from which to transfer, in form "multihash@domain"
* `-t`, `--to <TO>` — Account to which to transfer, in form "multihash@domain"



## `iroha domain meta`

Read/Write metadata

**Usage:** `iroha domain meta <COMMAND>`

###### **Subcommands:**

* `get` — Read a value from a key-value store
* `set` — Create or update an entry in a key-value store, with a value constructed from a JSON5 stdin
* `remove` — Delete an entry from a key-value store



## `iroha domain meta get`

Read a value from a key-value store

**Usage:** `iroha domain meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha domain meta set`

Create or update an entry in a key-value store, with a value constructed from a JSON5 stdin

**Usage:** `iroha domain meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha domain meta remove`

Delete an entry from a key-value store

**Usage:** `iroha domain meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha account`

Read/Write accounts

**Usage:** `iroha account <COMMAND>`

###### **Subcommands:**

* `role` — Read/Write account roles
* `permission` — Read/Write account permissions
* `list` — List accounts
* `get` — Read a single account details
* `register` — Register account
* `unregister` — Unregister account
* `meta` — Read/Write metadata



## `iroha account role`

Read/Write account roles

**Usage:** `iroha account role <COMMAND>`

###### **Subcommands:**

* `list` — List account role IDs
* `grant` — Grant account role
* `revoke` — Revoke account role



## `iroha account role list`

List account role IDs

**Usage:** `iroha account role list --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in form "multihash@domain"



## `iroha account role grant`

Grant account role

**Usage:** `iroha account role grant --id <ID> --role <ROLE>`

###### **Options:**

* `-i`, `--id <ID>` — Account in form "multihash@domain"
* `-r`, `--role <ROLE>` — Role name as double-quoted string



## `iroha account role revoke`

Revoke account role

**Usage:** `iroha account role revoke --id <ID> --role <ROLE>`

###### **Options:**

* `-i`, `--id <ID>` — Account in form "multihash@domain"
* `-r`, `--role <ROLE>` — Role name as double-quoted string



## `iroha account permission`

Read/Write account permissions

**Usage:** `iroha account permission <COMMAND>`

###### **Subcommands:**

* `list` — List account permissions
* `grant` — Grant account permission constructed from a JSON5 stdin
* `revoke` — Revoke account permission constructed from a JSON5 stdin



## `iroha account permission list`

List account permissions

**Usage:** `iroha account permission list --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in form "multihash@domain"



## `iroha account permission grant`

Grant account permission constructed from a JSON5 stdin

**Usage:** `iroha account permission grant --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in form "multihash@domain"



## `iroha account permission revoke`

Revoke account permission constructed from a JSON5 stdin

**Usage:** `iroha account permission revoke --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in form "multihash@domain"



## `iroha account list`

List accounts

**Usage:** `iroha account list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or entries when `--verbose` specified
* `filter` — Filter by given predicate



## `iroha account list all`

List all IDs, or entries when `--verbose` specified

**Usage:** `iroha account list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Show entry details, not only IDs



## `iroha account list filter`

Filter by given predicate

**Usage:** `iroha account list filter <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Predicate for filtering given as JSON5 string



## `iroha account get`

Read a single account details

**Usage:** `iroha account get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in form "multihash@domain"



## `iroha account register`

Register account

**Usage:** `iroha account register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in form "multihash@domain"



## `iroha account unregister`

Unregister account

**Usage:** `iroha account unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in form "multihash@domain"



## `iroha account meta`

Read/Write metadata

**Usage:** `iroha account meta <COMMAND>`

###### **Subcommands:**

* `get` — Read a value from a key-value store
* `set` — Create or update an entry in a key-value store, with a value constructed from a JSON5 stdin
* `remove` — Delete an entry from a key-value store



## `iroha account meta get`

Read a value from a key-value store

**Usage:** `iroha account meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha account meta set`

Create or update an entry in a key-value store, with a value constructed from a JSON5 stdin

**Usage:** `iroha account meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha account meta remove`

Delete an entry from a key-value store

**Usage:** `iroha account meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha asset`

Read/Write assets

**Usage:** `iroha asset <COMMAND>`

###### **Subcommands:**

* `definition` — Read/Write asset definitions
* `get` — Read a single asset details
* `list` — List assets
* `mint` — Increase an amount of asset
* `burn` — Decrease an amount of asset
* `transfer` — Transfer an amount of asset between accounts
* `transferkvs` — Transfer a key-value store between accounts
* `getkv` — Read a value from a key-value store
* `setkv` — Create or update an entry in a key-value store, with a value constructed from a JSON5 stdin
* `removekv` — Delete an entry from a key-value store



## `iroha asset definition`

Read/Write asset definitions

**Usage:** `iroha asset definition <COMMAND>`

###### **Subcommands:**

* `list` — List asset definitions
* `get` — Read a single asset definition details
* `register` — Register asset definition
* `unregister` — Unregister asset definition
* `transfer` — Transfer asset definition
* `meta` — Read/Write metadata



## `iroha asset definition list`

List asset definitions

**Usage:** `iroha asset definition list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or entries when `--verbose` specified
* `filter` — Filter by given predicate



## `iroha asset definition list all`

List all IDs, or entries when `--verbose` specified

**Usage:** `iroha asset definition list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Show entry details, not only IDs



## `iroha asset definition list filter`

Filter by given predicate

**Usage:** `iroha asset definition list filter <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Predicate for filtering given as JSON5 string



## `iroha asset definition get`

Read a single asset definition details

**Usage:** `iroha asset definition get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition in form "asset#domain"



## `iroha asset definition register`

Register asset definition

**Usage:** `iroha asset definition register [OPTIONS] --id <ID> --type <TYPE>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition in form "asset#domain"
* `-u`, `--unmintable` — Mintability of asset
* `-t`, `--type <TYPE>` — Value type stored in asset



## `iroha asset definition unregister`

Unregister asset definition

**Usage:** `iroha asset definition unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition in form "asset#domain"



## `iroha asset definition transfer`

Transfer asset definition

**Usage:** `iroha asset definition transfer --id <ID> --from <FROM> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition in form "asset#domain"
* `-f`, `--from <FROM>` — Account from which to transfer, in form "multihash@domain"
* `-t`, `--to <TO>` — Account to which to transfer, in form "multihash@domain"



## `iroha asset definition meta`

Read/Write metadata

**Usage:** `iroha asset definition meta <COMMAND>`

###### **Subcommands:**

* `get` — Read a value from a key-value store
* `set` — Create or update an entry in a key-value store, with a value constructed from a JSON5 stdin
* `remove` — Delete an entry from a key-value store



## `iroha asset definition meta get`

Read a value from a key-value store

**Usage:** `iroha asset definition meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha asset definition meta set`

Create or update an entry in a key-value store, with a value constructed from a JSON5 stdin

**Usage:** `iroha asset definition meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha asset definition meta remove`

Delete an entry from a key-value store

**Usage:** `iroha asset definition meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha asset get`

Read a single asset details

**Usage:** `iroha asset get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in form "asset##account@domain" or "asset#another_domain#account@domain"



## `iroha asset list`

List assets

**Usage:** `iroha asset list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or entries when `--verbose` specified
* `filter` — Filter by given predicate



## `iroha asset list all`

List all IDs, or entries when `--verbose` specified

**Usage:** `iroha asset list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Show entry details, not only IDs



## `iroha asset list filter`

Filter by given predicate

**Usage:** `iroha asset list filter <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Predicate for filtering given as JSON5 string



## `iroha asset mint`

Increase an amount of asset

**Usage:** `iroha asset mint --id <ID> --quantity <QUANTITY>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in form "asset##account@domain" or "asset#another_domain#account@domain"
* `-q`, `--quantity <QUANTITY>` — Amount in an integer or decimal



## `iroha asset burn`

Decrease an amount of asset

**Usage:** `iroha asset burn --id <ID> --quantity <QUANTITY>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in form "asset##account@domain" or "asset#another_domain#account@domain"
* `-q`, `--quantity <QUANTITY>` — Amount in an integer or decimal



## `iroha asset transfer`

Transfer an amount of asset between accounts

**Usage:** `iroha asset transfer --id <ID> --to <TO> --quantity <QUANTITY>`

###### **Options:**

* `-i`, `--id <ID>` — Asset to transfer, in form "asset##account@domain" or "asset#another_domain#account@domain"
* `-t`, `--to <TO>` — Account to which to transfer, in form "multihash@domain"
* `-q`, `--quantity <QUANTITY>` — Amount to transfer, in an integer or decimal



## `iroha asset transferkvs`

Transfer a key-value store between accounts

**Usage:** `iroha asset transferkvs --id <ID> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — Asset to transfer, in form "asset##account@domain" or "asset#another_domain#account@domain"
* `-t`, `--to <TO>` — Account to which to transfer, in form "multihash@domain"



## `iroha asset getkv`

Read a value from a key-value store

**Usage:** `iroha asset getkv --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in form "asset##account@domain" or "asset#another_domain#account@domain"
* `-k`, `--key <KEY>` — Key for the value



## `iroha asset setkv`

Create or update an entry in a key-value store, with a value constructed from a JSON5 stdin

**Usage:** `iroha asset setkv --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in form "asset##account@domain" or "asset#another_domain#account@domain"
* `-k`, `--key <KEY>` — Key for the value



## `iroha asset removekv`

Delete an entry from a key-value store

**Usage:** `iroha asset removekv --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in form "asset##account@domain" or "asset#another_domain#account@domain"
* `-k`, `--key <KEY>` — Key for the value



## `iroha peer`

Read/Write peers

**Usage:** `iroha peer <COMMAND>`

###### **Subcommands:**

* `list` — List registered peers expected to connect with each other
* `register` — Register peer
* `unregister` — Unregister peer



## `iroha peer list`

List registered peers expected to connect with each other

**Usage:** `iroha peer list <COMMAND>`

###### **Subcommands:**

* `all` — List all registered peers



## `iroha peer list all`

List all registered peers

**Usage:** `iroha peer list all`



## `iroha peer register`

Register peer

**Usage:** `iroha peer register --key <KEY>`

###### **Options:**

* `-k`, `--key <KEY>` — Peer's public key in multihash



## `iroha peer unregister`

Unregister peer

**Usage:** `iroha peer unregister --key <KEY>`

###### **Options:**

* `-k`, `--key <KEY>` — Peer's public key in multihash



## `iroha events`

Subscribe events: state changes, status of transactions/blocks/triggers

**Usage:** `iroha events [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `state` — Notify when world state has certain changes
* `transaction` — Notify when transaction passes certain processes
* `block` — Notify when block passes certain processes
* `trigger-execute` — Notify when trigger execution is ordered
* `trigger-complete` — Notify when trigger execution is completed

###### **Options:**

* `-t`, `--timeout <TIMEOUT>` — How long to listen for events ex. "1y 6M 2w 3d 12h 30m 30s 500ms"



## `iroha events state`

Notify when world state has certain changes

**Usage:** `iroha events state`



## `iroha events transaction`

Notify when transaction passes certain processes

**Usage:** `iroha events transaction`



## `iroha events block`

Notify when block passes certain processes

**Usage:** `iroha events block`



## `iroha events trigger-execute`

Notify when trigger execution is ordered

**Usage:** `iroha events trigger-execute`



## `iroha events trigger-complete`

Notify when trigger execution is completed

**Usage:** `iroha events trigger-complete`



## `iroha blocks`

Subscribe blocks

**Usage:** `iroha blocks [OPTIONS] <HEIGHT>`

###### **Arguments:**

* `<HEIGHT>` — Block height from which to start streaming blocks

###### **Options:**

* `-t`, `--timeout <TIMEOUT>` — How long to listen for blocks ex. "1y 6M 2w 3d 12h 30m 30s 500ms"



## `iroha multisig`

Read/Write multisig accounts and transactions

**Usage:** `iroha multisig <COMMAND>`

###### **Subcommands:**

* `list` — List pending multisig transactions relevant to you
* `register` — Register a multisig account
* `propose` — Propose a multisig transaction, constructed from instructions as a JSON5 stdin
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
* `-s`, `--signatories <SIGNATORIES>` — Signatories of the multisig account
* `-w`, `--weights <WEIGHTS>` — Relative weights of responsibility of respective signatories
* `-q`, `--quorum <QUORUM>` — Threshold of total weight at which the multisig is considered authenticated
* `-t`, `--transaction-ttl <TRANSACTION_TTL>` — Time-to-live of multisig transactions made by the multisig account ex. "1y 6M 2w 3d 12h 30m 30s 500ms"

  Default value: `1h`



## `iroha multisig propose`

Propose a multisig transaction, constructed from instructions as a JSON5 stdin

**Usage:** `iroha multisig propose [OPTIONS] --account <ACCOUNT>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — Multisig authority of the multisig transaction
* `-t`, `--transaction-ttl <TRANSACTION_TTL>` — Time-to-live of multisig transactions that overrides to shorten the account default ex. "1y 6M 2w 3d 12h 30m 30s 500ms"



## `iroha multisig approve`

Approve a multisig transaction

**Usage:** `iroha multisig approve --account <ACCOUNT> --instructions-hash <INSTRUCTIONS_HASH>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — Multisig authority of the multisig transaction
* `-i`, `--instructions-hash <INSTRUCTIONS_HASH>` — Instructions to approve



## `iroha query`

Read in general

**Usage:** `iroha query <COMMAND>`

###### **Subcommands:**

* `stdin` — Query constructed from a JSON5 stdin



## `iroha query stdin`

Query constructed from a JSON5 stdin

**Usage:** `iroha query stdin`



## `iroha transaction`

Read transactions, Write in general

**Usage:** `iroha transaction <COMMAND>`

###### **Subcommands:**

* `get` — Read a single transaction details
* `ping` — Empty transaction that just leaves a log message
* `wasm` — Transaction constructed from a Wasm executable input
* `stdin` — Transaction constructed from instructions as a JSON5 stdin



## `iroha transaction get`

Read a single transaction details

**Usage:** `iroha transaction get --hash <HASH>`

###### **Options:**

* `-H`, `--hash <HASH>` — Transaction hash



## `iroha transaction ping`

Empty transaction that just leaves a log message

**Usage:** `iroha transaction ping [OPTIONS] --msg <MSG>`

###### **Options:**

* `-l`, `--log-level <LOG_LEVEL>` — TRACE, DEBUG, INFO, WARN, ERROR: grows more noticeable in this order

  Default value: `INFO`
* `-m`, `--msg <MSG>` — Log message



## `iroha transaction wasm`

Transaction constructed from a Wasm executable input

**Usage:** `iroha transaction wasm [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Specify a path to the Wasm file or skip this arg to read from stdin



## `iroha transaction stdin`

Transaction constructed from instructions as a JSON5 stdin

**Usage:** `iroha transaction stdin`



## `iroha role`

Read/Write roles

**Usage:** `iroha role <COMMAND>`

###### **Subcommands:**

* `permission` — Read/Write role permissions
* `list` — List role IDs
* `register` — Register role and grant it to you registrant
* `unregister` — Unregister role



## `iroha role permission`

Read/Write role permissions

**Usage:** `iroha role permission <COMMAND>`

###### **Subcommands:**

* `list` — List role permissions
* `grant` — Grant role permission constructed from a JSON5 stdin
* `revoke` — Revoke role permission constructed from a JSON5 stdin



## `iroha role permission list`

List role permissions

**Usage:** `iroha role permission list --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name as double-quoted string



## `iroha role permission grant`

Grant role permission constructed from a JSON5 stdin

**Usage:** `iroha role permission grant --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name as double-quoted string



## `iroha role permission revoke`

Revoke role permission constructed from a JSON5 stdin

**Usage:** `iroha role permission revoke --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name as double-quoted string



## `iroha role list`

List role IDs

**Usage:** `iroha role list <COMMAND>`

###### **Subcommands:**

* `all` — List all role IDs



## `iroha role list all`

List all role IDs

**Usage:** `iroha role list all`



## `iroha role register`

Register role and grant it to you registrant

**Usage:** `iroha role register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name as double-quoted string



## `iroha role unregister`

Unregister role

**Usage:** `iroha role unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name as double-quoted string



## `iroha parameter`

Read/Write parameters

**Usage:** `iroha parameter <COMMAND>`

###### **Subcommands:**

* `list` — List parameters
* `set` — Set parameter constructed from a JSON5 stdin



## `iroha parameter list`

List parameters

**Usage:** `iroha parameter list <COMMAND>`

###### **Subcommands:**

* `all` — List all parameters



## `iroha parameter list all`

List all parameters

**Usage:** `iroha parameter list all`



## `iroha parameter set`

Set parameter constructed from a JSON5 stdin

**Usage:** `iroha parameter set`



## `iroha trigger`

TODO Read/Write triggers

**Usage:** `iroha trigger <COMMAND>`

###### **Subcommands:**

* `list` — List trigger IDs
* `get` — Read a single trigger details
* `register` — TODO Register trigger
* `unregister` — Unregister trigger
* `meta` — Read/Write metadata



## `iroha trigger list`

List trigger IDs

**Usage:** `iroha trigger list <COMMAND>`

###### **Subcommands:**

* `all` — List all trigger IDs



## `iroha trigger list all`

List all trigger IDs

**Usage:** `iroha trigger list all`



## `iroha trigger get`

Read a single trigger details

**Usage:** `iroha trigger get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name as double-quoted string



## `iroha trigger register`

TODO Register trigger

**Usage:** `iroha trigger register`



## `iroha trigger unregister`

Unregister trigger

**Usage:** `iroha trigger unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name as double-quoted string



## `iroha trigger meta`

Read/Write metadata

**Usage:** `iroha trigger meta <COMMAND>`

###### **Subcommands:**

* `get` — Read a value from a key-value store
* `set` — Create or update an entry in a key-value store, with a value constructed from a JSON5 stdin
* `remove` — Delete an entry from a key-value store



## `iroha trigger meta get`

Read a value from a key-value store

**Usage:** `iroha trigger meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha trigger meta set`

Create or update an entry in a key-value store, with a value constructed from a JSON5 stdin

**Usage:** `iroha trigger meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha trigger meta remove`

Delete an entry from a key-value store

**Usage:** `iroha trigger meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha executor`

Update executor

**Usage:** `iroha executor <COMMAND>`

###### **Subcommands:**

* `upgrade` — Upgrade executor



## `iroha executor upgrade`

Upgrade executor

**Usage:** `iroha executor upgrade --path <PATH>`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the compiled Wasm file



## `iroha markdown-help`

Dump a markdown help of this CLI to stdout

**Usage:** `iroha markdown-help`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

