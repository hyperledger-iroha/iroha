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
* [`iroha events governance`↴](#iroha-events-governance)
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
* [`iroha query stdin-raw`↴](#iroha-query-stdin-raw)
* [`iroha transaction`↴](#iroha-transaction)
* [`iroha transaction get`↴](#iroha-transaction-get)
* [`iroha transaction ping`↴](#iroha-transaction-ping)
* [`iroha transaction ivm`↴](#iroha-transaction-ivm)
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
* [`iroha offline`↴](#iroha-offline)
* [`iroha offline allowance`↴](#iroha-offline-allowance)
* [`iroha offline allowance list`↴](#iroha-offline-allowance-list)
* [`iroha offline allowance get`↴](#iroha-offline-allowance-get)
* [`iroha offline transfer`↴](#iroha-offline-transfer)
* [`iroha offline transfer list`↴](#iroha-offline-transfer-list)
* [`iroha offline transfer get`↴](#iroha-offline-transfer-get)
* [`iroha executor`↴](#iroha-executor)
* [`iroha executor data-model`↴](#iroha-executor-data-model)
* [`iroha executor upgrade`↴](#iroha-executor-upgrade)
* [`iroha markdown-help`↴](#iroha-markdown-help)
* [`iroha version`↴](#iroha-version)
* [`iroha zk`↴](#iroha-zk)
* [`iroha zk roots`↴](#iroha-zk-roots)
* [`iroha zk verify`↴](#iroha-zk-verify)
* [`iroha zk submit-proof`↴](#iroha-zk-submit-proof)
* [`iroha zk verify-batch`↴](#iroha-zk-verify-batch)
* [`iroha zk schema-hash`↴](#iroha-zk-schema-hash)
* [`iroha zk attachments`↴](#iroha-zk-attachments)
* [`iroha zk attachments upload`↴](#iroha-zk-attachments-upload)
* [`iroha zk attachments list`↴](#iroha-zk-attachments-list)
* [`iroha zk attachments get`↴](#iroha-zk-attachments-get)
* [`iroha zk attachments delete`↴](#iroha-zk-attachments-delete)
* [`iroha zk attachments cleanup`↴](#iroha-zk-attachments-cleanup)
* [`iroha zk register-asset`↴](#iroha-zk-register-asset)
* [`iroha zk shield`↴](#iroha-zk-shield)
* [`iroha zk unshield`↴](#iroha-zk-unshield)
* [`iroha zk vk`↴](#iroha-zk-vk)
* [`iroha zk vk register`↴](#iroha-zk-vk-register)
* [`iroha zk vk update`↴](#iroha-zk-vk-update)
* [`iroha zk vk deprecate`↴](#iroha-zk-vk-deprecate)
* [`iroha zk vk get`↴](#iroha-zk-vk-get)
* [`iroha zk proofs`↴](#iroha-zk-proofs)
* [`iroha zk proofs list`↴](#iroha-zk-proofs-list)
* [`iroha zk proofs count`↴](#iroha-zk-proofs-count)
* [`iroha zk proofs get`↴](#iroha-zk-proofs-get)
* [`iroha zk prover`↴](#iroha-zk-prover)
* [`iroha zk prover reports`↴](#iroha-zk-prover-reports)
* [`iroha zk prover reports list`↴](#iroha-zk-prover-reports-list)
* [`iroha zk prover reports get`↴](#iroha-zk-prover-reports-get)
* [`iroha zk prover reports delete`↴](#iroha-zk-prover-reports-delete)
* [`iroha zk prover reports cleanup`↴](#iroha-zk-prover-reports-cleanup)
* [`iroha zk prover reports count`↴](#iroha-zk-prover-reports-count)
* [`iroha zk vote`↴](#iroha-zk-vote)
* [`iroha zk vote tally`↴](#iroha-zk-vote-tally)
* [`iroha zk envelope`↴](#iroha-zk-envelope)
* [`iroha crypto`↴](#iroha-crypto)
* [`iroha crypto sm2`↴](#iroha-crypto-sm2)
* [`iroha crypto sm2 keygen`↴](#iroha-crypto-sm2-keygen)
* [`iroha crypto sm2 import`↴](#iroha-crypto-sm2-import)
* [`iroha crypto sm2 export`↴](#iroha-crypto-sm2-export)
* [`iroha crypto sm3`↴](#iroha-crypto-sm3)
* [`iroha crypto sm3 hash`↴](#iroha-crypto-sm3-hash)
* [`iroha crypto sm4`↴](#iroha-crypto-sm4)
* [`iroha crypto sm4 gcm-seal`↴](#iroha-crypto-sm4-gcm-seal)
* [`iroha crypto sm4 gcm-open`↴](#iroha-crypto-sm4-gcm-open)
* [`iroha confidential`↴](#iroha-confidential)
* [`iroha confidential create-keys`↴](#iroha-confidential-create-keys)
* [`iroha confidential gas`↴](#iroha-confidential-gas)
* [`iroha confidential gas get`↴](#iroha-confidential-gas-get)
* [`iroha confidential gas set`↴](#iroha-confidential-gas-set)
* [`iroha ivm`↴](#iroha-ivm)
* [`iroha ivm abi-hash`↴](#iroha-ivm-abi-hash)
* [`iroha ivm syscalls`↴](#iroha-ivm-syscalls)
* [`iroha ivm manifest-gen`↴](#iroha-ivm-manifest-gen)
* [`iroha gov`↴](#iroha-gov)
* [`iroha gov propose-deploy`↴](#iroha-gov-propose-deploy)
* [`iroha gov vote`↴](#iroha-gov-vote)
* [`iroha gov vote-zk`↴](#iroha-gov-vote-zk)
* [`iroha gov vote-plain`↴](#iroha-gov-vote-plain)
* [`iroha gov proposal-get`↴](#iroha-gov-proposal-get)
* [`iroha gov locks-get`↴](#iroha-gov-locks-get)
* [`iroha gov council`↴](#iroha-gov-council)
* [`iroha gov council derive-vrf`↴](#iroha-gov-council-derive-vrf)
* [`iroha gov council persist`↴](#iroha-gov-council-persist)
* [`iroha gov council gen-vrf`↴](#iroha-gov-council-gen-vrf)
* [`iroha gov council derive-and-persist`↴](#iroha-gov-council-derive-and-persist)
* [`iroha gov unlock-stats`↴](#iroha-gov-unlock-stats)
* [`iroha gov referendum-get`↴](#iroha-gov-referendum-get)
* [`iroha gov tally-get`↴](#iroha-gov-tally-get)
* [`iroha gov finalize`↴](#iroha-gov-finalize)
* [`iroha gov enact`↴](#iroha-gov-enact)
* [`iroha gov protected-set`↴](#iroha-gov-protected-set)
* [`iroha gov protected-apply`↴](#iroha-gov-protected-apply)
* [`iroha gov protected-get`↴](#iroha-gov-protected-get)
* [`iroha gov activate-instance`↴](#iroha-gov-activate-instance)
* [`iroha gov instances`↴](#iroha-gov-instances)
* [`iroha gov deploy-meta`↴](#iroha-gov-deploy-meta)
* [`iroha gov audit-deploy`↴](#iroha-gov-audit-deploy)
* [`iroha sumeragi`↴](#iroha-sumeragi)
* [`iroha sumeragi status`↴](#iroha-sumeragi-status)
* [`iroha sumeragi leader`↴](#iroha-sumeragi-leader)
* [`iroha sumeragi params`↴](#iroha-sumeragi-params)
* [`iroha sumeragi collectors`↴](#iroha-sumeragi-collectors)
* [`iroha sumeragi qc`↴](#iroha-sumeragi-qc)
* [`iroha sumeragi pacemaker`↴](#iroha-sumeragi-pacemaker)
* [`iroha sumeragi phases`↴](#iroha-sumeragi-phases)
* [`iroha sumeragi telemetry`↴](#iroha-sumeragi-telemetry)
* [`iroha sumeragi evidence`↴](#iroha-sumeragi-evidence)
* [`iroha sumeragi evidence list`↴](#iroha-sumeragi-evidence-list)
* [`iroha sumeragi evidence count`↴](#iroha-sumeragi-evidence-count)
* [`iroha sumeragi evidence submit`↴](#iroha-sumeragi-evidence-submit)
* [`iroha sumeragi rbc`↴](#iroha-sumeragi-rbc)
* [`iroha sumeragi rbc status`↴](#iroha-sumeragi-rbc-status)
* [`iroha sumeragi rbc sessions`↴](#iroha-sumeragi-rbc-sessions)
* [`iroha sumeragi vrf-penalties`↴](#iroha-sumeragi-vrf-penalties)
* [`iroha sumeragi vrf-epoch`↴](#iroha-sumeragi-vrf-epoch)
* [`iroha sumeragi exec-qc-get`↴](#iroha-sumeragi-exec-qc-get)
* [`iroha sumeragi exec-root-get`↴](#iroha-sumeragi-exec-root-get)
* [`iroha contracts`↴](#iroha-contracts)
* [`iroha contracts code-bytes-get`↴](#iroha-contracts-code-bytes-get)
* [`iroha contracts deploy`↴](#iroha-contracts-deploy)
* [`iroha contracts manifest`↴](#iroha-contracts-manifest)
* [`iroha contracts instances`↴](#iroha-contracts-instances)
* [`iroha runtime`↴](#iroha-runtime)
* [`iroha runtime abi`↴](#iroha-runtime-abi)
* [`iroha runtime abi active`↴](#iroha-runtime-abi-active)
* [`iroha runtime abi active-query`↴](#iroha-runtime-abi-active-query)
* [`iroha runtime abi hash`↴](#iroha-runtime-abi-hash)
* [`iroha runtime upgrade`↴](#iroha-runtime-upgrade)
* [`iroha runtime upgrade list`↴](#iroha-runtime-upgrade-list)
* [`iroha runtime upgrade propose`↴](#iroha-runtime-upgrade-propose)
* [`iroha runtime upgrade activate`↴](#iroha-runtime-upgrade-activate)
* [`iroha runtime upgrade cancel`↴](#iroha-runtime-upgrade-cancel)
* [`iroha runtime status`↴](#iroha-runtime-status)
* [`iroha audit`↴](#iroha-audit)
* [`iroha audit witness`↴](#iroha-audit-witness)
* [`iroha kaigi`↴](#iroha-kaigi)
* [`iroha kaigi create`↴](#iroha-kaigi-create)
* [`iroha kaigi join`↴](#iroha-kaigi-join)
* [`iroha kaigi leave`↴](#iroha-kaigi-leave)
* [`iroha kaigi end`↴](#iroha-kaigi-end)
* [`iroha kaigi record-usage`↴](#iroha-kaigi-record-usage)
* [`iroha kaigi report-relay-health`↴](#iroha-kaigi-report-relay-health)
* [`iroha alias`↴](#iroha-alias)
* [`iroha alias voprf-evaluate`↴](#iroha-alias-voprf-evaluate)
* [`iroha alias resolve`↴](#iroha-alias-resolve)
* [`iroha alias resolve-index`↴](#iroha-alias-resolve-index)
* [`iroha repo`↴](#iroha-repo)
* [`iroha repo initiate`↴](#iroha-repo-initiate)
* [`iroha repo unwind`↴](#iroha-repo-unwind)
* [`iroha repo query`↴](#iroha-repo-query)
* [`iroha repo query list`↴](#iroha-repo-query-list)
* [`iroha repo query get`↴](#iroha-repo-query-get)
* [`iroha repo margin`↴](#iroha-repo-margin)
* [`iroha settlement`↴](#iroha-settlement)
* [`iroha settlement dvp`↴](#iroha-settlement-dvp)
* [`iroha settlement pvp`↴](#iroha-settlement-pvp)

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
* `version` — Show versions and git SHA of client and server
* `zk` — Zero-knowledge helpers (roots, etc.)
* `crypto` — Cryptography helpers (SM2/SM3/SM4)
* `confidential` — Confidential asset tooling helpers
* `ivm` — IVM/ABI helpers (e.g., compute ABI hash)
* `gov` — Governance helpers (app API convenience)
* `sumeragi` — Sumeragi helpers (status)
* `contracts` — Contracts helpers (code storage)
* `runtime` — Runtime ABI/upgrades
* `audit` — Audit helpers (debug endpoints)
* `kaigi` — Kaigi session helpers
* `alias` — Alias helpers (placeholder pipeline)

###### **Options:**

* `-c`, `--config <PATH>` — Path to the configuration file.

   By default, `iroha` will try to read `client.toml` file, but would proceed if it is not found.
* `-v`, `--verbose` — Print configuration details to stderr
* `-m`, `--metadata <PATH>` — Path to a JSON file for attaching transaction metadata (optional)
* `-i`, `--input` — Reads instructions from stdin and appends new ones.

   Example usage:

   `echo "[]" | iroha -io domain register --id "domain" | iroha -i asset definition register --id "asset#domain" -t Numeric`
* `-o`, `--output` — Outputs instructions to stdout without submitting them.

   Example usage:

   `iroha -o domain register --id "domain" | iroha -io asset definition register --id "asset#domain" -t Numeric | iroha transaction stdin`
* `--language <LANG>` — Language code for messages, overrides system language



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
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha domain list filter`

Filter by a given predicate

**Usage:** `iroha domain list filter [OPTIONS] <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Filtering condition specified as a JSON string

###### **Options:**

* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



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
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha domain meta get`

Retrieve a value from the key-value store

**Usage:** `iroha domain meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha domain meta set`

Create or update an entry in the key-value store using JSON input from stdin

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

**Usage:** `iroha account role list [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in the format "multihash@domain"
* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



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
* `grant` — Grant an account permission using JSON input from stdin
* `revoke` — Revoke an account permission using JSON input from stdin



## `iroha account permission list`

List account permissions

**Usage:** `iroha account permission list [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in the format "multihash@domain"
* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha account permission grant`

Grant an account permission using JSON input from stdin

**Usage:** `iroha account permission grant --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account in the format "multihash@domain"



## `iroha account permission revoke`

Revoke an account permission using JSON input from stdin

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
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha account list filter`

Filter by a given predicate

**Usage:** `iroha account list filter [OPTIONS] <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Filtering condition specified as a JSON string

###### **Options:**

* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



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
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha account meta get`

Retrieve a value from the key-value store

**Usage:** `iroha account meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha account meta set`

Create or update an entry in the key-value store using JSON input from stdin

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
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha asset definition list filter`

Filter by a given predicate

**Usage:** `iroha asset definition list filter [OPTIONS] <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Filtering condition specified as a JSON string

###### **Options:**

* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



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
* `--confidential-mode <CONFIDENTIAL_MODE>` — Confidential policy mode for this asset definition

  Default value: `transparent-only`

  Possible values: `transparent-only`, `shielded-only`, `convertible`

* `--confidential-vk-set-hash <CONFIDENTIAL_VK_SET_HASH>` — Hex-encoded hash summarising the expected verifying key set
* `--confidential-poseidon-params <CONFIDENTIAL_POSEIDON_PARAMS>` — Poseidon parameter set identifier expected for confidential proofs
* `--confidential-pedersen-params <CONFIDENTIAL_PEDERSEN_PARAMS>` — Pedersen parameter set identifier expected for confidential commitments



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
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha asset definition meta get`

Retrieve a value from the key-value store

**Usage:** `iroha asset definition meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha asset definition meta set`

Create or update an entry in the key-value store using JSON input from stdin

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
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha asset list filter`

Filter by a given predicate

**Usage:** `iroha asset list filter [OPTIONS] <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Filtering condition specified as a JSON string

###### **Options:**

* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



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

**Usage:** `iroha asset transfer --id <ID> --to <TO> --quantity <QUANTITY> [--ensure-destination]`

###### **Options:**

* `-i`, `--id <ID>` — Asset in the format "asset##account@domain" or "asset#another_domain#account@domain"
* `-t`, `--to <TO>` — Destination account, in the format "multihash@domain"
* `-q`, `--quantity <QUANTITY>` — Transfer amount (integer or decimal)
* `--ensure-destination` — Prepend a `Register<Account>` when the destination domain disables implicit receive (fails if the account already exists)

Implicit-receive domains auto-create missing accounts on receipt; the CLI no longer pre-validates
whether the destination exists and surfaces policy errors from Torii directly.



## `iroha nft`

Read and write NFTs

**Usage:** `iroha nft <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve details of a specific NFT
* `list` — List NFTs
* `register` — Register NFT with content provided from stdin in JSON format
* `unregister` — Unregister NFT
* `transfer` — Transfer ownership of NFT
* `getkv` — Get a value from NFT
* `setkv` — Create or update a key-value entry of NFT using JSON input from stdin
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
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha nft list filter`

Filter by a given predicate

**Usage:** `iroha nft list filter [OPTIONS] <PREDICATE>`

###### **Arguments:**

* `<PREDICATE>` — Filtering condition specified as a JSON string

###### **Options:**

* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha nft register`

Register NFT with content provided from stdin in JSON format

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

Create or update a key-value entry of NFT using JSON input from stdin

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

**Usage:** `iroha peer list all [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha peer register`

Register a peer

**Usage:** `iroha peer register --key <KEY> --pop <HEX>`

###### **Options:**

* `-k`, `--key <KEY>` — Peer's public key in multihash format (must be BLS-normal)
* `--pop <HEX>` — Proof-of-possession bytes as hex (with or without 0x prefix)



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
* `governance` — Notify governance lifecycle events
* `transaction` — Notify when a transaction reaches specific stages
* `block` — Notify when a block reaches specific stages
* `trigger-execute` — Notify when a trigger execution is ordered
* `trigger-complete` — Notify when a trigger execution is completed

###### **Options:**

* `-t`, `--timeout <TIMEOUT>` — Duration to listen for events. Example: "1y 6M 2w 3d 12h 30m 30s"



## `iroha events state`

Notify when the world state undergoes certain changes

**Usage:** `iroha events state`



## `iroha events governance`

Notify governance lifecycle events

**Usage:** `iroha events governance [OPTIONS]`

###### **Options:**

* `--proposal-id <ID_HEX>` — Filter by proposal id (hex)
* `--referendum-id <RID>` — Filter by referendum id



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
* `propose` — Propose a multisig transaction using JSON input from stdin
* `approve` — Approve a multisig transaction



## `iroha multisig list`

List pending multisig transactions relevant to you

**Usage:** `iroha multisig list <COMMAND>`

###### **Subcommands:**

* `all` — List all pending multisig transactions relevant to you



## `iroha multisig list all`

List all pending multisig transactions relevant to you

**Usage:** `iroha multisig list all [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of role IDs to scan for multisig (server-side limit)
* `--offset <OFFSET>` — Offset into the role ID set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for roles query



## `iroha multisig register`

Register a multisig account

**Usage:** `iroha multisig register [OPTIONS] --quorum <QUORUM>`

###### **Options:**

* `-s`, `--signatories <SIGNATORIES>` — List of signatories for the multisig account
* `-w`, `--weights <WEIGHTS>` — Relative weights of signatories' responsibilities
* `-q`, `--quorum <QUORUM>` — Threshold of total weight required for authentication
* `-t`, `--transaction-ttl <TRANSACTION_TTL>` — Time-to-live for multisig transactions. Example: "1y 6M 2w 3d 12h 30m 30s"

  Default value: `1h`



## `iroha multisig propose`

Propose a multisig transaction using JSON input from stdin

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



## `iroha multisig inspect`

Inspect a multisig account controller and print the CTAP2 payload + digest

**Usage:** `iroha multisig inspect [OPTIONS] --account <ACCOUNT>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — Multisig account identifier to inspect
* `--json` — Emit JSON instead of human-readable output


## `iroha query`

Read various data

**Usage:** `iroha query <COMMAND>`

###### **Subcommands:**

* `stdin` — Query using JSON input from stdin
* `stdin-raw` — Query using raw SignedQuery (base64 or hex) from stdin



## `iroha query stdin`

Query using JSON input from stdin

**Usage:** `iroha query stdin`



## `iroha query stdin-raw`

Query using raw SignedQuery (base64 or hex) from stdin

**Usage:** `iroha query stdin-raw`



## `iroha transaction`

Read transactions and write various data

**Usage:** `iroha transaction <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve details of a specific transaction
* `ping` — Send an empty transaction that logs a message
* `ivm` — Send a transaction using IVM bytecode
* `stdin` — Send a transaction using JSON input from stdin



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



## `iroha transaction ivm`

Send a transaction using IVM bytecode

**Usage:** `iroha transaction ivm [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the IVM bytecode file. If omitted, reads from stdin



## `iroha transaction stdin`

Send a transaction using JSON input from stdin

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
* `grant` — Grant role permission using JSON input from stdin
* `revoke` — Revoke role permission using JSON input from stdin



## `iroha role permission list`

List role permissions

**Usage:** `iroha role permission list [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name
* `--limit <LIMIT>` — Maximum number of items to return (client-side for now)
* `--offset <OFFSET>` — Offset into the result set (client-side for now)

  Default value: `0`



## `iroha role permission grant`

Grant role permission using JSON input from stdin

**Usage:** `iroha role permission grant --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha role permission revoke`

Revoke role permission using JSON input from stdin

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

**Usage:** `iroha role list all [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



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
* `set` — Set a system parameter using JSON input from stdin



## `iroha parameter list`

List system parameters

**Usage:** `iroha parameter list <COMMAND>`

###### **Subcommands:**

* `all` — List all system parameters



## `iroha parameter list all`

List all system parameters

**Usage:** `iroha parameter list all`



## `iroha parameter set`

Set a system parameter using JSON input from stdin

**Usage:** `iroha parameter set`



## `iroha trigger`

Read and write triggers

**Usage:** `iroha trigger <COMMAND>`

###### **Subcommands:**

* `list` — List trigger IDs
* `get` — Retrieve details of a specific trigger
* `register` — Register a trigger
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

**Usage:** `iroha trigger list all [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha trigger get`

Retrieve details of a specific trigger

**Usage:** `iroha trigger get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name



## `iroha trigger register`

Register a trigger

**Usage:** `iroha trigger register [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name
* `-p`, `--path <PATH>` — Path to the compiled IVM bytecode to execute
* `--instructions-stdin` — Read JSON array of instructions from stdin instead of bytecode path Example: echo "[ {\"Log\": {\"level\": \"INFO\", \"message\": \"hi\"}} ]" | iroha trigger register -i my_trig --instructions-stdin
* `--instructions <PATH>` — Read JSON array of instructions from a file instead of bytecode path
* `-r`, `--repeats <REPEATS>` — Number of permitted executions (default: indefinitely)
* `--authority <AUTHORITY>` — Account executing the trigger (default: current config account)
* `--filter <FILTER>` — Filter type for the trigger

  Default value: `execute`

  Possible values: `execute`, `time`, `data`

* `--time-start-ms <TIME_START_MS>` — Start time in milliseconds since UNIX epoch for time filter
* `--time-period-ms <TIME_PERIOD_MS>` — Period in milliseconds for time filter (optional)
* `--data-filter <JSON>` — JSON for a DataEventFilter to use as filter
* `--data-domain <DATA_DOMAIN>` — Data filter preset: events within a domain
* `--data-account <DATA_ACCOUNT>` — Data filter preset: events for an account
* `--data-asset <DATA_ASSET>` — Data filter preset: events for an asset
* `--data-asset-definition <DATA_ASSET_DEFINITION>` — Data filter preset: events for an asset definition
* `--data-role <DATA_ROLE>` — Data filter preset: events for a role
* `--data-trigger <DATA_TRIGGER>` — Data filter preset: events for a trigger
* `--data-verifying-key <BACKEND:NAME>` — Data filter preset: events for a verifying key (format: `<backend>:<name>`)
* `--data-proof <BACKEND:HEX>` — Data filter preset: events for a proof (format: `<backend>:<64-hex-proof-hash>`)
* `--data-proof-only <PRESET>` — Restrict proof events to a preset when using `--data-proof`. Presets: `verified`, `rejected`, `all` (default)

  Possible values:
  - `all`:
    All proof events (default)
  - `verified`:
    Only Verified events
  - `rejected`:
    Only Rejected events

* `--data-vk-only <PRESET>` — Restrict verifying key events to a preset when using `--data-verifying-key`. Presets: `registered`, `updated`, `all` (default)

  Possible values:
  - `all`:
    All verifying key events (default)
  - `registered`:
    Only Registered events
  - `updated`:
    Only Updated events
* `--time-start <DURATION>` — Human-readable offset for time start (e.g., "5m", "1h"), added to current time
* `--time-start-rfc3339 <RFC3339>` — RFC3339 timestamp for time filter start (e.g., 2025-01-01T00:00:00Z)



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
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha trigger meta get`

Retrieve a value from the key-value store

**Usage:** `iroha trigger meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha trigger meta set`

Create or update an entry in the key-value store using JSON input from stdin

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



## `iroha offline`

Inspect offline allowances and pending offline-to-online transfer bundles

**Usage:** `iroha offline <COMMAND>`

###### **Subcommands:**

* `allowance` — Inspect offline allowances
* `transfer` — Inspect offline-to-online transfer bundles



## `iroha offline allowance`

Inspect offline allowances

**Usage:** `iroha offline allowance <COMMAND>`

###### **Subcommands:**

* `list` — List all registered offline allowances
* `get` — Fetch a specific offline allowance by certificate id



## `iroha offline allowance list`

List all registered offline allowances (`--verbose` prints the full records)

**Usage:** `iroha offline allowance list [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha offline allowance get`

Fetch a specific offline allowance by certificate id

**Usage:** `iroha offline allowance get --certificate-id <CERTIFICATE_ID>`

###### **Options:**

* `--certificate-id <CERTIFICATE_ID>` — Deterministic certificate identifier (hex)



## `iroha offline transfer`

Inspect pending offline-to-online transfer bundles

**Usage:** `iroha offline transfer <COMMAND>`

###### **Subcommands:**

* `list` — List all pending offline-to-online transfer bundles
* `get` — Fetch a specific offline-to-online transfer by bundle id



## `iroha offline transfer list`

List all pending offline-to-online transfer bundles (`--verbose` prints the full records)

**Usage:** `iroha offline transfer list [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha offline transfer get`

Fetch a specific offline-to-online transfer bundle by id

**Usage:** `iroha offline transfer get --bundle-id <BUNDLE_ID>`

###### **Options:**

* `--bundle-id <BUNDLE_ID>` — Deterministic bundle identifier (hex)



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

* `-p`, `--path <PATH>` — Path to the compiled IVM bytecode file



## `iroha markdown-help`

Output CLI documentation in Markdown format

**Usage:** `iroha markdown-help`



## `iroha version`

Show versions and git SHA of client and server

**Usage:** `iroha version`



## `iroha zk`

Zero-knowledge helpers (roots, etc.)

**Usage:** `iroha zk <COMMAND>`

###### **Subcommands:**

* `roots` — Get recent shielded roots for an asset (JSON). Posts to /v1/zk/roots
* `verify` — Verify a ZK proof by posting an OpenVerifyEnvelope (Norito) or a JSON DTO to /v1/zk/verify
* `submit-proof` — Submit a ZK proof envelope for later reference/inspection. Posts to /v1/zk/submit-proof
* `verify-batch` — Verify a batch of ZK OpenVerify envelopes (Norito vector) via /v1/zk/verify-batch
* `schema-hash` — Compute the Blake2b-32 hash required for `public_inputs_schema_hash` and print it
* `attachments` — Manage ZK attachments in the app API
* `register-asset` — Register a ZK-capable asset (Hybrid mode) with policy and VK ids
* `shield` — Shield public funds into a shielded ledger (demo flow)
* `unshield` — Unshield funds from shielded ledger to public (demo flow)
* `vk` — Verifying-key registry lifecycle (register/update/deprecate/get)
* `proofs` — Inspect proof registry (list/count/get)
* `prover` — Inspect background prover reports (list/get/delete)
* `vote` — ZK Vote helpers (tally)
* `envelope` — Encode a confidential encrypted payload (memo) into Norito bytes/base64



## `iroha zk roots`

Get recent shielded roots for an asset (JSON). Posts to /v1/zk/roots

**Usage:** `iroha zk roots [OPTIONS] --asset-id <ASSET_ID>`

###### **Options:**

* `--asset-id <ASSET_ID>` — AssetDefinitionId like `rose#wonderland`
* `--max <MAX>` — Maximum number of roots to return (0 = server cap)

  Default value: `0`



## `iroha zk verify`

Verify a ZK proof by posting an OpenVerifyEnvelope (Norito) or a JSON DTO to /v1/zk/verify

**Usage:** `iroha zk verify [OPTIONS]`

###### **Options:**

* `--norito <PATH>` — Path to Norito-encoded OpenVerifyEnvelope bytes (mutually exclusive with --json)
* `--json <PATH>` — Path to a JSON DTO describing the proof (backend, proof, vk) (mutually exclusive with --norito)



## `iroha zk submit-proof`

Submit a ZK proof envelope for later reference/inspection. Posts to /v1/zk/submit-proof

**Usage:** `iroha zk submit-proof [OPTIONS]`

###### **Options:**

* `--norito <PATH>` — Path to Norito-encoded proof envelope bytes (mutually exclusive with --json)
* `--json <PATH>` — Path to a JSON DTO describing the proof (backend, proof, vk) (mutually exclusive with --norito)



## `iroha zk verify-batch`

Verify a batch of ZK OpenVerify envelopes (Norito vector) via /v1/zk/verify-batch

**Usage:** `iroha zk verify-batch [OPTIONS]`

###### **Options:**

* `--norito <PATH>` — Path to a Norito-encoded Vec<OpenVerifyEnvelope> (mutually exclusive with --json)
* `--json <PATH>` — Path to a JSON array of base64-encoded Norito OpenVerifyEnvelope items (mutually exclusive with --norito)



## `iroha zk schema-hash`

Compute the Blake2b-32 hash required for `public_inputs_schema_hash` and print it

**Usage:** `iroha zk schema-hash [OPTIONS]`

###### **Options:**

* `--norito <PATH>` — Path to a Norito-encoded OpenVerifyEnvelope
* `--public-inputs-hex <HEX>` — Hex-encoded public inputs (when not using --norito)



## `iroha zk attachments`

Manage ZK attachments in the app API

**Usage:** `iroha zk attachments <COMMAND>`

###### **Subcommands:**

* `upload` — Upload a file as an attachment. Returns JSON metadata
* `list` — List stored attachments (JSON array of metadata)
* `get` — Download an attachment by id to a file
* `delete` — Delete an attachment by id
* `cleanup` — Cleanup attachments by filters (age/content-type/ids). Deletes individually via API



## `iroha zk attachments upload`

Upload a file as an attachment. Returns JSON metadata

**Usage:** `iroha zk attachments upload [OPTIONS] --file <PATH>`

###### **Options:**

* `--file <PATH>` — Path to the file to upload
* `--content-type <MIME>` — Content-Type to send with the file

  Default value: `application/octet-stream`



## `iroha zk attachments list`

List stored attachments (JSON array of metadata)

**Usage:** `iroha zk attachments list`



## `iroha zk attachments get`

Download an attachment by id to a file

**Usage:** `iroha zk attachments get --id <ID> --out <PATH>`

###### **Options:**

* `--id <ID>` — Attachment id (hex)
* `--out <PATH>` — Output path to write the downloaded bytes



## `iroha zk attachments delete`

Delete an attachment by id

**Usage:** `iroha zk attachments delete --id <ID>`

###### **Options:**

* `--id <ID>` — Attachment id (hex)



## `iroha zk attachments cleanup`

Cleanup attachments by filters (age/content-type/ids). Deletes individually via API

**Usage:** `iroha zk attachments cleanup [OPTIONS]`

###### **Options:**

* `--yes` — Proceed without confirmation
* `--all` — Delete all attachments (dangerous). Requires --yes
* `--content-type <MIME>` — Filter by content-type substring (e.g., application/x-norito)
* `--before-ms <MS>` — Filter attachments created strictly before this UNIX epoch in milliseconds
* `--older-than-secs <SECS>` — Filter attachments older than N seconds (relative to now)
* `--id <ID>` — Filter by specific id(s); may be repeated
* `--limit <N>` — Maximum number of attachments to delete (applied after filtering)
* `--ids-only` — Preview only: list matching ids instead of full metadata
* `--summary` — Preview only: print a summary table (id, content_type, size, created_ms)



## `iroha zk register-asset`

Register a ZK-capable asset (Hybrid mode) with policy and VK ids

**Usage:** `iroha zk register-asset [OPTIONS] --asset <ASSET_ID>`

###### **Options:**

* `--asset <ASSET_ID>` — AssetDefinitionId like `rose#wonderland`
* `--allow-shield` — Allow shielding from public to shielded (default: true)

  Default value: `true`
* `--allow-unshield` — Allow unshielding from shielded to public (default: true)

  Default value: `true`
* `--vk-transfer <BACKEND:NAME>` — Verifying key id for private transfers (format: `<backend>:<name>`, e.g., `halo2/ipa:vk_transfer`)
* `--vk-unshield <BACKEND:NAME>` — Verifying key id for unshield proofs (format: `<backend>:<name>`)
* `--vk-shield <BACKEND:NAME>` — Verifying key id for shield proofs (optional; format: `<backend>:<name>`)



## `iroha zk shield`

Shield public funds into a shielded ledger (demo flow)

**Usage:** `iroha zk shield [OPTIONS] --asset <ASSET_ID> --from <ACCOUNT_ID> --amount <AMOUNT> --note-commitment <HEX32>`

###### **Options:**

* `--asset <ASSET_ID>` — AssetDefinitionId like `rose#wonderland`
* `--from <ACCOUNT_ID>` — AccountId to debit (e.g., `alice@wonderland`)
* `--amount <AMOUNT>` — Public amount to debit
* `--note-commitment <HEX32>` — Output note commitment (hex, 64 chars)
* `--enc-payload <PATH>` — Encrypted recipient payload envelope (Norito bytes). Optional; empty if not provided
* `--ephemeral-pubkey <HEX32>` — Ephemeral public key for encrypted payload (hex, 64 chars)
* `--nonce-hex <HEX24>` — XChaCha20-Poly1305 nonce for encrypted payload (hex, 48 chars)
* `--ciphertext-b64 <BASE64>` — Ciphertext payload (base64). Includes Poly1305 authentication tag



## `iroha zk unshield`

Unshield funds from shielded ledger to public (demo flow)

**Usage:** `iroha zk unshield [OPTIONS] --asset <ASSET_ID> --to <ACCOUNT_ID> --amount <AMOUNT> --inputs <HEX32[,HEX32,...]> --proof-json <PATH>`

###### **Options:**

* `--asset <ASSET_ID>` — AssetDefinitionId like `rose#wonderland`
* `--to <ACCOUNT_ID>` — Recipient AccountId to credit (e.g., `alice@wonderland`)
* `--amount <AMOUNT>` — Public amount to credit
* `--inputs <HEX32[,HEX32,...]>` — Spent nullifiers (comma-separated list of 64-hex strings)
* `--proof-json <PATH>` — Proof attachment JSON file describing { backend, proof_b64, vk_ref{backend,name} | vk_inline{backend,bytes_b64}, optional vk_commitment_hex }
* `--root-hint <HEX32>` — Optional Merkle root hint (hex, 64 chars)



## `iroha zk vk`

Verifying-key registry lifecycle (register/update/deprecate/get)

**Usage:** `iroha zk vk <COMMAND>`

###### **Subcommands:**

* `register` — Register a verifying key record (signed transaction via Torii app API)
* `update` — Update an existing verifying key record (version must increase)
* `deprecate` — Deprecate a verifying key (disallow updates)
* `get` — Get a verifying key record by backend and name



## `iroha zk vk register`

Register a verifying key record (signed transaction via Torii app API)

**Usage:** `iroha zk vk register --json <PATH>`

###### **Options:**

* `--json <PATH>` — Path to a JSON DTO file for register (authority, private_key, backend, name, version, optional vk_bytes (base64) or commitment_hex)



## `iroha zk vk update`

Update an existing verifying key record (version must increase)

**Usage:** `iroha zk vk update --json <PATH>`

###### **Options:**

* `--json <PATH>` — Path to a JSON DTO file for update (authority, private_key, backend, name, version, optional vk_bytes or commitment_hex)



## `iroha zk vk deprecate`

Deprecate a verifying key (disallow updates)

**Usage:** `iroha zk vk deprecate --json <PATH>`

###### **Options:**

* `--json <PATH>` — Path to a JSON DTO file for deprecate (authority, private_key, backend, name)



## `iroha zk vk get`

Get a verifying key record by backend and name

**Usage:** `iroha zk vk get --backend <BACKEND> --name <NAME>`

###### **Options:**

* `--backend <BACKEND>` — Backend identifier (e.g., "halo2/ipa")
* `--name <NAME>` — Verifying key name



## `iroha zk proofs`

Inspect proof registry (list/count/get)

**Usage:** `iroha zk proofs <COMMAND>`

###### **Subcommands:**

* `list` — List proof records maintained by Torii
* `count` — Count proof records matching the filters
* `get` — Fetch a proof record by backend and proof hash (hex)



## `iroha zk proofs list`

List proof records maintained by Torii

**Usage:** `iroha zk proofs list [OPTIONS]`

###### **Options:**

* `--backend <BACKEND>` — Filter by backend identifier (e.g., `halo2/ipa`)
* `--status <STATUS>` — Filter by verification status (`Submitted`, `Verified`, `Rejected`)
* `--has-tag <TAG>` — Require a ZK1 TLV tag (4 ASCII characters, e.g., `PROF`)
* `--limit <LIMIT>` — Limit result size (server caps at 1000)
* `--offset <OFFSET>` — Offset for server-side pagination
* `--order <ORDER>` — Sort order (`asc` or `desc`) by verification height
* `--ids-only` — Return only `{ backend, hash }` identifiers



## `iroha zk proofs count`

Count proof records matching the filters

**Usage:** `iroha zk proofs count [OPTIONS]`

###### **Options:**

* `--backend <BACKEND>` — Filter by backend identifier (e.g., `halo2/ipa`)
* `--status <STATUS>` — Filter by verification status (`Submitted`, `Verified`, `Rejected`)
* `--has-tag <TAG>` — Require a ZK1 TLV tag (4 ASCII characters, e.g., `PROF`)
* `--limit <LIMIT>` — Limit result size (server caps at 1000)
* `--offset <OFFSET>` — Offset for server-side pagination
* `--order <ORDER>` — Sort order (`asc` or `desc`) by verification height



## `iroha zk proofs get`

Fetch a proof record by backend and proof hash (hex)

**Usage:** `iroha zk proofs get --backend <BACKEND> --hash <HASH>`

###### **Options:**

* `--backend <BACKEND>` — Backend identifier (e.g., `halo2/ipa`)
* `--hash <HASH>` — Proof hash (hex, with or without `0x` prefix)



## `iroha zk prover`

Inspect background prover reports (list/get/delete)

**Usage:** `iroha zk prover <COMMAND>`

###### **Subcommands:**

* `reports` — Manage prover reports



## `iroha zk prover reports`

Manage prover reports

**Usage:** `iroha zk prover reports <COMMAND>`

###### **Subcommands:**

* `list` — List available prover reports (JSON array)
* `get` — Get a single prover report by id (JSON)
* `delete` — Delete a prover report by id
* `cleanup` — Cleanup reports in bulk (apply filters, delete matches)
* `count` — Count reports matching filters (server-side)



## `iroha zk prover reports list`

List available prover reports (JSON array)

**Usage:** `iroha zk prover reports list [OPTIONS]`

###### **Options:**

* `--summary` — Print a one-line summary per report (id, ok, content_type, zk1_tags)
* `--ok-only` — Show only successful reports
* `--failed-only` — Show only failed reports
* `--errors-only` — Alias for failed-only (errors have ok=false)
* `--id <ID>` — Filter by exact id (hex)
* `--content-type <MIME>` — Filter by content-type substring (e.g., application/x-norito)
* `--has-tag <TAG>` — Filter reports that contain a ZK1 tag (e.g., PROF, IPAK)
* `--ids-only` — Return only ids (server-side projection)
* `--messages-only` — Return only `{ id, error }` objects for failed reports (server-side projection)
* `--fields <CSV>` — Project returned fields (client-side) from full objects, comma-separated (e.g., "id,ok,content_type,processed_ms"). Ignored with --summary/--ids-only/--messages-only
* `--limit <N>` — Limit number of reports returned (server-side). Max 1000
* `--since-ms <MS>` — Only reports with processed_ms >= this value (server-side)
* `--before-ms <MS>` — Only reports with processed_ms <= this value (server-side)
* `--order <ORDER>` — Result ordering: asc (default) or desc

  Default value: `asc`
* `--offset <N>` — Offset after ordering/filtering (server-side)
* `--latest` — Return only the latest report after filters



## `iroha zk prover reports get`

Get a single prover report by id (JSON)

**Usage:** `iroha zk prover reports get --id <ID>`

###### **Options:**

* `--id <ID>` — Report id (attachment id)



## `iroha zk prover reports delete`

Delete a prover report by id

**Usage:** `iroha zk prover reports delete --id <ID>`

###### **Options:**

* `--id <ID>` — Report id (attachment id)



## `iroha zk prover reports cleanup`

Cleanup reports in bulk (apply filters, delete matches)

**Usage:** `iroha zk prover reports cleanup [OPTIONS]`

###### **Options:**

* `--yes` — Proceed without confirmation (dangerous)
* `--ok-only` — Show only successful reports
* `--failed-only` — Show only failed reports
* `--errors-only` — Alias for failed-only (errors have ok=false)
* `--id <ID>` — Filter by exact id (hex)
* `--content-type <MIME>` — Filter by content-type substring (e.g., application/x-norito)
* `--has-tag <TAG>` — Filter reports that contain a ZK1 tag (e.g., PROF, IPAK)
* `--limit <N>` — Limit number of reports returned (server-side). Max 1000
* `--since-ms <MS>` — Only reports with processed_ms >= this value (server-side)
* `--before-ms <MS>` — Only reports with processed_ms <= this value (server-side)
* `--server` — Use server-side bulk deletion instead of client-side delete loop



## `iroha zk prover reports count`

Count reports matching filters (server-side)

**Usage:** `iroha zk prover reports count [OPTIONS]`

###### **Options:**

* `--ok-only` — Show only successful reports
* `--failed-only` — Show only failed reports
* `--errors-only` — Alias for failed-only (errors have ok=false)
* `--id <ID>` — Filter by exact id (hex)
* `--content-type <MIME>` — Filter by content-type substring (e.g., application/x-norito)
* `--has-tag <TAG>` — Filter reports that contain a ZK1 tag (e.g., PROF, IPAK)
* `--since-ms <MS>` — Only reports with processed_ms >= this value (server-side)
* `--before-ms <MS>` — Only reports with processed_ms <= this value (server-side)



## `iroha zk vote`

ZK Vote helpers (tally)

**Usage:** `iroha zk vote <COMMAND>`

###### **Subcommands:**

* `tally` — Get election tally (JSON)



## `iroha zk vote tally`

Get election tally (JSON)

**Usage:** `iroha zk vote tally --election-id <ELECTION_ID>`

###### **Options:**

* `--election-id <ELECTION_ID>` — Election identifier



## `iroha zk envelope`

Encode a confidential encrypted payload (memo) into Norito bytes/base64

**Usage:** `iroha zk envelope [OPTIONS] --ephemeral-pubkey <HEX32> --nonce-hex <HEX24> --ciphertext-b64 <BASE64>`

###### **Options:**

* `--ephemeral-pubkey <HEX32>` — Ephemeral public key (hex, 64 chars)
* `--nonce-hex <HEX24>` — XChaCha20-Poly1305 nonce (hex, 48 chars)
* `--ciphertext-b64 <BASE64>` — Ciphertext payload (base64) including Poly1305 tag
* `--output <PATH>` — Optional output path for Norito bytes
* `--print-base64` — Print base64 of the encoded envelope (default when no output file is provided)

  Default value: `false`
* `--print-hex` — Print hexadecimal representation of the encoded envelope

  Default value: `false`
* `--print-json` — Print JSON representation of the envelope

  Default value: `false`



## `iroha crypto`

Cryptography helpers (SM2/SM3/SM4)

**Usage:** `iroha crypto <COMMAND>`

###### **Subcommands:**

* `sm2` — SM2 key management helpers
* `sm3` — SM3 hashing helpers
* `sm4` — SM4 AEAD helpers (GCM mode)



## `iroha crypto sm2`

SM2 key management helpers

**Usage:** `iroha crypto sm2 <COMMAND>`

###### **Subcommands:**

* `keygen` — Generate a new SM2 key pair (distinguishing ID aware)
* `import` — Import an existing SM2 private key and derive metadata
* `export` — Export SM2 key material with config snippets



## `iroha crypto sm2 keygen`

Generate a new SM2 key pair (distinguishing ID aware)

**Usage:** `iroha crypto sm2 keygen [OPTIONS]`

###### **Options:**

* `--distid <DISTID>` — Distinguishing identifier embedded into SM2 signatures (defaults to `1234567812345678`)
* `--seed-hex <HEX>` — Optional seed (hex) for deterministic key generation. Helpful for tests/backups
* `--output <PATH>` — Write the generated JSON payload to a file instead of stdout
* `--quiet` — Suppress stdout printing of the JSON payload



## `iroha crypto sm2 import`

Import an existing SM2 private key and derive metadata

**Usage:** `iroha crypto sm2 import [OPTIONS]`

###### **Options:**

* `--private-key-hex <HEX>` — Existing SM2 private key in hex (32 bytes)
* `--private-key-file <PATH>` — Path to a file containing a hex-encoded SM2 private key (32 bytes)
* `--private-key-pem <PEM>` — Existing SM2 private key encoded as PKCS#8 PEM
* `--private-key-pem-file <PATH>` — Path to a PKCS#8 PEM file containing an SM2 private key
* `--public-key-pem <PEM>` — Optional SM2 public key in PEM (verified against derived public key)
* `--public-key-pem-file <PATH>` — Path to a PEM file containing an SM2 public key to verify against the derived key
* `--distid <DISTID>` — Distinguishing identifier used by the signer (defaults to `1234567812345678`)
* `--output <PATH>` — Write the derived JSON payload to a file instead of stdout
* `--quiet` — Suppress stdout printing of the JSON payload



## `iroha crypto sm2 export`

Export SM2 key material with config snippets

**Usage:** `iroha crypto sm2 export [OPTIONS]`

###### **Options:**

* `--private-key-hex <HEX>` — Existing SM2 private key in hex (32 bytes)
* `--private-key-file <PATH>` — Path to a file containing a hex-encoded SM2 private key (32 bytes)
* `--private-key-pem <PEM>` — PKCS#8 PEM-encoded SM2 private key
* `--private-key-pem-file <PATH>` — Path to a PKCS#8 PEM SM2 private key
* `--distid <DISTID>` — Distinguishing identifier used by the signer (defaults to `1234567812345678`)
* `--snippet-output <PATH>` — Write the TOML snippet to a file
* `--emit-json` — Emit the JSON key material alongside the config snippet
* `--quiet` — Suppress stdout output



## `iroha crypto sm3`

SM3 hashing helpers

**Usage:** `iroha crypto sm3 <COMMAND>`

###### **Subcommands:**

* `hash` — Hash input data with SM3



## `iroha crypto sm3 hash`

Hash input data with SM3

**Usage:** `iroha crypto sm3 hash [OPTIONS]`

###### **Options:**

* `--data <STRING>` — UTF-8 string to hash (mutually exclusive with other inputs)
* `--data-hex <HEX>` — Raw bytes to hash provided as hex
* `--file <PATH>` — Path to a file whose contents will be hashed
* `--output <PATH>` — Write the digest JSON to a file
* `--quiet` — Suppress stdout printing of the digest JSON



## `iroha crypto sm4`

SM4 AEAD helpers (GCM mode)

**Usage:** `iroha crypto sm4 <COMMAND>`

###### **Subcommands:**

* `gcm-seal` — Encrypt data with SM4-GCM
* `gcm-open` — Decrypt data with SM4-GCM



## `iroha crypto sm4 gcm-seal`

Encrypt data with SM4-GCM

**Usage:** `iroha crypto sm4 gcm-seal [OPTIONS] --key-hex <HEX32> --nonce-hex <HEX24>`

###### **Options:**

* `--key-hex <HEX32>` — SM4 key (16 bytes hex)
* `--nonce-hex <HEX24>` — GCM nonce (12 bytes hex)
* `--aad-hex <HEX>` — Additional authenticated data (hex, optional)

  Default value: ``
* `--plaintext-hex <HEX>` — Plaintext to encrypt (hex, mutually exclusive with file)
* `--plaintext-file <PATH>` — Path to plaintext bytes to encrypt
* `--ciphertext-file <PATH>` — Write the ciphertext bytes to a file
* `--tag-file <PATH>` — Write the authentication tag bytes to a file
* `--quiet` — Suppress stdout JSON output



## `iroha crypto sm4 gcm-open`

Decrypt data with SM4-GCM

**Usage:** `iroha crypto sm4 gcm-open [OPTIONS] --key-hex <HEX32> --nonce-hex <HEX24>`

###### **Options:**

* `--key-hex <HEX32>` — SM4 key (16 bytes hex)
* `--nonce-hex <HEX24>` — GCM nonce (12 bytes hex)
* `--aad-hex <HEX>` — Additional authenticated data (hex, optional)

  Default value: ``
* `--ciphertext-hex <HEX>` — Ciphertext to decrypt (hex, mutually exclusive with file)
* `--ciphertext-file <PATH>` — Path to ciphertext bytes
* `--tag-hex <HEX>` — Authentication tag (hex, mutually exclusive with file)
* `--tag-file <PATH>` — Path to authentication tag bytes
* `--plaintext-file <PATH>` — Write the decrypted plaintext to a file
* `--quiet` — Suppress stdout JSON output



## `iroha confidential`

Confidential asset tooling helpers

**Usage:** `iroha confidential <COMMAND>`

###### **Subcommands:**

* `create-keys` — Derive confidential key hierarchy (nk/ivk/ovk/fvk) from a spend key
* `gas` — Inspect or update the confidential gas schedule



## `iroha confidential create-keys`

Derive confidential key hierarchy (nk/ivk/ovk/fvk) from a spend key

**Usage:** `iroha confidential create-keys [OPTIONS]`

###### **Options:**

* `--seed-hex <HEX32>` — 32-byte spend key in hex (if omitted, a random key is generated)
* `--output <PATH>` — Write the derived keyset JSON to a file
* `--quiet` — Do not print the generated JSON to stdout



## `iroha confidential gas`

Inspect or update the confidential gas schedule

**Usage:** `iroha confidential gas <COMMAND>`

###### **Subcommands:**

* `get` — Fetch the current confidential gas schedule
* `set` — Update the confidential gas schedule



## `iroha confidential gas get`

Fetch the current confidential gas schedule

**Usage:** `iroha confidential gas get`



## `iroha confidential gas set`

Update the confidential gas schedule

**Usage:** `iroha confidential gas set --proof-base <UNITS> --per-public-input <UNITS> --per-proof-byte <UNITS> --per-nullifier <UNITS> --per-commitment <UNITS>`

###### **Options:**

* `--proof-base <UNITS>`
* `--per-public-input <UNITS>`
* `--per-proof-byte <UNITS>`
* `--per-nullifier <UNITS>`
* `--per-commitment <UNITS>`



## `iroha ivm`

IVM/ABI helpers (e.g., compute ABI hash)

**Usage:** `iroha ivm <COMMAND>`

###### **Subcommands:**

* `abi-hash` — Print the current ABI hash for a given policy (default: v1)
* `syscalls` — Print the canonical syscall list (min or markdown table)
* `manifest-gen` — Generate a minimal manifest (code_hash + abi_hash) from a compiled .to file



## `iroha ivm abi-hash`

Print the current ABI hash for a given policy (default: v1)

**Usage:** `iroha ivm abi-hash [OPTIONS]`

###### **Options:**

* `--policy <POLICY>` — Policy: v1, or exp:<n>

  Default value: `v1`
* `--uppercase` — Uppercase hex output (default: lowercase)



## `iroha ivm syscalls`

Print the canonical syscall list (min or markdown table)

**Usage:** `iroha ivm syscalls [OPTIONS]`

###### **Options:**

* `--format <FORMAT>` — Output format: 'min' (one per line) or 'markdown'

  Default value: `min`



## `iroha ivm manifest-gen`

Generate a minimal manifest (code_hash + abi_hash) from a compiled .to file

**Usage:** `iroha ivm manifest-gen --file <PATH>`

###### **Options:**

* `--file <PATH>` — Path to compiled IVM bytecode (.to)



## `iroha gov`

Governance helpers (app API convenience)

**Usage:** `iroha gov <COMMAND>`

###### **Subcommands:**

* `propose-deploy` — Propose deployment of IVM bytecode by code/abi hash via governance (build-only; server returns instruction skeleton)
* `vote` — Submit a governance ballot; auto-detects referendum mode unless overridden
* `vote-zk` — Submit a ZK ballot (server returns instruction skeleton)
* `vote-plain` — Submit a non-ZK quadratic ballot (server returns instruction skeleton)
* `proposal-get` — Get a governance proposal by id (hex)
* `locks-get` — Get locks for a referendum id
* `council` — Get current sortition council
* `unlock-stats` — Show governance unlock sweep stats (expired locks at current height)
* `referendum-get` — Get a referendum by id
* `tally-get` — Get a tally snapshot by referendum id
* `finalize` — Build a finalize transaction for a referendum (server returns instruction skeleton)
* `enact` — Build an enactment transaction for an approved proposal
* `protected-set` — Set protected namespaces (custom parameter gov_protected_namespaces)
* `protected-apply` — Apply protected namespaces on the server (requires API token if configured)
* `protected-get` — Get protected namespaces (custom parameter gov_protected_namespaces)
* `activate-instance` — Activate a contract instance (namespace, contract_id) -> code_hash (admin/testing)
* `instances` — List active contract instances for a namespace
* `deploy-meta` — Build deploy metadata JSON for protected namespace admission (optionally listing manifest approvers)
* `audit-deploy` — Audit stored manifests against governance proposals and code storage



## `iroha gov propose-deploy`

Propose deployment of IVM bytecode by code/abi hash via governance (build-only; server returns instruction skeleton)

**Usage:** `iroha gov propose-deploy [OPTIONS] --namespace <NAMESPACE> --contract-id <ID> --code-hash <CODE_HASH> --abi-hash <ABI_HASH>`

###### **Options:**

* `--namespace <NAMESPACE>`
* `--contract-id <ID>`
* `--code-hash <CODE_HASH>`
* `--abi-hash <ABI_HASH>`
* `--abi-version <ABI_VERSION>`

  Default value: `v1`
* `--window-lower <WINDOW_LOWER>` — Optional window lower bound (height)
* `--window-upper <WINDOW_UPPER>` — Optional window upper bound (height)
* `--mode <MODE>` — Optional voting mode for the referendum: Zk or Plain (default Zk)

  Default value: ``

  Possible values: `Zk`, `Plain`

* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov vote`

Submit a governance ballot; auto-detects referendum mode unless overridden

**Usage:** `iroha gov vote [OPTIONS] --referendum-id <REFERENDUM_ID>`

###### **Options:**

* `--referendum-id <REFERENDUM_ID>`
* `--mode <MODE>` — Voting mode override. Defaults to auto-detect via GET /v1/gov/referenda/{id}

  Default value: `auto`

  Possible values:
  - `auto`:
    Automatically detect the referendum mode from the node
  - `plain`:
    Force plain (non-ZK) voting mode
  - `zk`:
    Force zero-knowledge voting mode

* `--proof-b64 <PROOF_B64>` — Base64-encoded proof for ZK voting mode
* `--public <PATH>` — Optional JSON file containing public inputs for ZK voting mode
* `--owner <OWNER>` — Owner account id for plain voting mode (must equal transaction authority)
* `--amount <AMOUNT>` — Locked amount for plain voting mode (string to preserve large integers)
* `--duration-blocks <DURATION_BLOCKS>` — Lock duration (in blocks) for plain voting mode
* `--direction <DIRECTION>` — Ballot direction for plain voting mode: Aye, Nay, or Abstain
* `--salt-hex <SALT_HEX>` — Optional 32-byte salt hint for ZK ballots (hex)
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov vote-zk`

Submit a ZK ballot (server returns instruction skeleton)

**Usage:** `iroha gov vote-zk [OPTIONS] --election-id <ELECTION_ID> --proof-b64 <PROOF_B64>`

###### **Options:**

* `--election-id <ELECTION_ID>`
* `--proof-b64 <PROOF_B64>`
* `--public <PUBLIC>` — Path to a JSON file with additional public inputs (optional)
* `--owner <OWNER>` — Optional owner hint mirrored into public inputs
* `--amount <AMOUNT>` — Optional lock amount hint mirrored into public inputs
* `--duration-blocks <DURATION_BLOCKS>` — Optional lock duration hint mirrored into public inputs
* `--direction <DIRECTION>` — Optional direction hint mirrored into public inputs
* `--salt-hex <SALT_HEX>` — Optional 32-byte salt hint for nullifier derivation
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov vote-plain`

Submit a non-ZK quadratic ballot (server returns instruction skeleton)

**Usage:** `iroha gov vote-plain [OPTIONS] --referendum-id <REFERENDUM_ID> --owner <OWNER> --amount <AMOUNT> --duration-blocks <DURATION_BLOCKS> --direction <DIRECTION>`

###### **Options:**

* `--referendum-id <REFERENDUM_ID>`
* `--owner <OWNER>`
* `--amount <AMOUNT>`
* `--duration-blocks <DURATION_BLOCKS>`
* `--direction <DIRECTION>`
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov proposal-get`

Get a governance proposal by id (hex)

**Usage:** `iroha gov proposal-get [OPTIONS] --id <ID_HEX>`

###### **Options:**

* `--id <ID_HEX>`
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov locks-get`

Get locks for a referendum id

**Usage:** `iroha gov locks-get [OPTIONS] --referendum-id <REFERENDUM_ID>`

###### **Options:**

* `--referendum-id <REFERENDUM_ID>`
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov council`

Get current sortition council

**Usage:** `iroha gov council [OPTIONS]`

###### **Options:**

* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov council derive-vrf`

Derive council membership using VRF proofs (server feature: gov_vrf).

**Usage:** `iroha gov council derive-vrf [OPTIONS] --committee-size <N>`

###### **Options:**

* `--committee-size <N>` — Committee size to select
* `--epoch <EPOCH>` — Optional epoch override
* `--candidate <CANDIDATES>` — Candidate spec: "account_id,variant,pk_b64,proof_b64"; repeatable
* `--candidates-file <PATH>` — Path to a JSON file with an array of candidates ({account_id, variant, pk_b64, proof_b64})
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov council persist`

Persist council membership (on-chain) using VRF proofs (server feature: gov_vrf).

**Usage:** `iroha gov council persist [OPTIONS] --committee-size <COMMITTEE_SIZE> --candidates-file <PATH> --authority <AUTHORITY> --private-key <HEX>`

###### **Options:**

* `--committee-size <COMMITTEE_SIZE>` — Committee size to select (top-k by VRF output)
* `--epoch <EPOCH>` — Optional epoch override; defaults to height/TERM_BLOCKS
* `--candidates-file <PATH>` — Path to JSON file with candidates: [{ account_id, variant: Normal|Small, pk_b64, proof_b64 }, ...]
* `--authority <AUTHORITY>` — Authority AccountId for signing (e.g., alice@wonderland)
* `--private-key <HEX>` — Private key (hex) for signing
* `--summary-only` — Print only a summary line

  Default value: `false`
* `--no-summary` — Suppress summary (print raw JSON only)

  Default value: `false`



## `iroha gov council gen-vrf`

Generate a JSON array of VRF candidates for testing.

**Usage:** `iroha gov council gen-vrf [OPTIONS] --chain-id <CHAIN_ID>`

###### **Options:**

* `--count <COUNT>` — Number of candidates to generate

  Default value: `5`
* `--variant <VARIANT>` — Variant: Normal (pk in G1, proof in G2) or Small (pk in G2, proof in G1)

  Default value: `Normal`

  Possible values: `Normal`, `Small`

* `--chain-id <CHAIN_ID>` — Chain id string used for VRF domain separation
* `--seed-hex <SEED_HEX>` — Optional seed hex (32 bytes as 64 hex); if omitted, requires --epoch and --beacon-hex
* `--epoch <EPOCH>` — Epoch index used when deriving the seed (ignored if --seed-hex is provided)
* `--beacon-hex <BEACON_HEX>` — Beacon hash hex (32 bytes as 64 hex) to derive the seed (ignored if --seed-hex is provided)
* `--account-prefix <ACCOUNT_PREFIX>` — Account id prefix (final id is `${prefix}-${i}@${domain}`)

  Default value: `node`
* `--domain <DOMAIN>` — Domain used in generated account ids

  Default value: `wonderland`
* `--out <OUT>` — Output path; if omitted, prints JSON to stdout
* `--from-audit` — Fetch seed/epoch/chain_id from /v1/gov/council/audit (overrides --epoch/--beacon-hex when set)

  Default value: `false`



## `iroha gov council derive-and-persist`

Derive council via VRF and persist it on-chain in one step.

**Usage:** `iroha gov council derive-and-persist [OPTIONS] --committee-size <COMMITTEE_SIZE> --candidates-file <PATH> --authority <AUTHORITY> --private-key <HEX>`

###### **Options:**

* `--committee-size <COMMITTEE_SIZE>` — Committee size to select (top-k by VRF output)
* `--epoch <EPOCH>` — Optional epoch override; defaults to height/TERM_BLOCKS (server-side)
* `--candidates-file <PATH>` — Path to JSON file with candidates: [{ account_id, variant: Normal|Small, pk_b64, proof_b64 }, ...]
* `--authority <AUTHORITY>` — Authority AccountId for signing (e.g., alice@wonderland)
* `--private-key <HEX>` — Private key (hex) for signing
* `--summary-only` — Print only a summary line

  Default value: `false`
* `--no-summary` — Suppress summary (print raw JSON only)

  Default value: `false`
* `--wait` — Wait for CouncilPersisted event and verify via /v1/gov/council/current

  Default value: `false`



## `iroha gov unlock-stats`

Show governance unlock sweep stats (expired locks at current height)

**Usage:** `iroha gov unlock-stats [OPTIONS]`

###### **Options:**

* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov referendum-get`

Get a referendum by id

**Usage:** `iroha gov referendum-get [OPTIONS] --id <ID>`

###### **Options:**

* `--id <ID>`
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov tally-get`

Get a tally snapshot by referendum id

**Usage:** `iroha gov tally-get [OPTIONS] --id <ID>`

###### **Options:**

* `--id <ID>`
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov finalize`

Build a finalize transaction for a referendum (server returns instruction skeleton)

**Usage:** `iroha gov finalize [OPTIONS] --referendum-id <REFERENDUM_ID> --proposal-id <ID_HEX>`

###### **Options:**

* `--referendum-id <REFERENDUM_ID>` — Referendum id
* `--proposal-id <ID_HEX>` — Proposal id (hex 64)
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov enact`

Build an enactment transaction for an approved proposal

**Usage:** `iroha gov enact [OPTIONS] --proposal-id <ID_HEX>`

###### **Options:**

* `--proposal-id <ID_HEX>` — Proposal id (hex 64)
* `--preimage-hash <PREIMAGE_HASH>` — Optional preimage hash (hex 64)
* `--window-lower <WINDOW_LOWER>` — Optional window lower bound (height)
* `--window-upper <WINDOW_UPPER>` — Optional window upper bound (height)
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov protected-set`

Set protected namespaces (custom parameter gov_protected_namespaces)

**Usage:** `iroha gov protected-set [OPTIONS] --namespaces <NAMESPACES>`

###### **Options:**

* `--namespaces <NAMESPACES>` — Comma-separated namespaces (e.g., apps,system)
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov protected-apply`

Apply protected namespaces on the server (requires API token if configured)

**Usage:** `iroha gov protected-apply [OPTIONS] --namespaces <NAMESPACES>`

###### **Options:**

* `--namespaces <NAMESPACES>` — Comma-separated namespaces (e.g., apps,system)
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov protected-get`

Get protected namespaces (custom parameter gov_protected_namespaces)

**Usage:** `iroha gov protected-get [OPTIONS]`

###### **Options:**

* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov activate-instance`

Activate a contract instance (namespace, contract_id) -> code_hash (admin/testing)

**Usage:** `iroha gov activate-instance [OPTIONS] --namespace <NAMESPACE> --contract-id <CONTRACT_ID> --code-hash <HEX64>`

###### **Options:**

* `--namespace <NAMESPACE>`
* `--contract-id <CONTRACT_ID>`
* `--code-hash <HEX64>` — code hash hex (64 chars, 0x optional)
* `--blocking` — Submit and wait until committed or rejected

  Default value: `false`



## `iroha gov instances`

List active contract instances for a namespace

**Usage:** `iroha gov instances [OPTIONS] --namespace <NS>`

###### **Options:**

* `--namespace <NS>` — Namespace to list (e.g., apps)
* `--contains <CONTAINS>` — Filter: contract_id substring (case-sensitive)
* `--hash-prefix <HASH_PREFIX>` — Filter: code hash hex prefix (lowercase)
* `--offset <OFFSET>` — Pagination offset
* `--limit <LIMIT>` — Pagination limit
* `--order <ORDER>` — Order: cid_asc (default), cid_desc, hash_asc, hash_desc
* `--summary-only` — Print only the compact summary line (suppresses raw JSON)

  Default value: `false`
* `--no-summary` — Suppress the compact summary line (print raw JSON only)

  Default value: `false`



## `iroha gov deploy-meta`

Build deploy metadata JSON for protected namespace admission

**Usage:** `iroha gov deploy-meta --namespace <NAMESPACE> --contract-id <CONTRACT_ID> [--approver <ACCOUNT> ...]`

###### **Options:**

* `--namespace <NAMESPACE>`
* `--contract-id <CONTRACT_ID>`
* `--approver <ACCOUNT>` — Append a validator account id contributing to manifest quorum approval (repeatable)



## `iroha gov audit-deploy`

Audit stored manifests against governance proposals and code storage

**Usage:** `iroha gov audit-deploy [OPTIONS] --namespace <NS>`

###### **Options:**

* `--namespace <NS>` — Namespace to audit (e.g., apps)
* `--contains <CONTAINS>` — Filter: contract_id substring (case-sensitive)
* `--hash-prefix <HASH_PREFIX>` — Filter: code hash hex prefix (lowercase)
* `--offset <OFFSET>` — Pagination offset
* `--limit <LIMIT>` — Pagination limit
* `--order <ORDER>` — Order: cid_asc (default), cid_desc, hash_asc, hash_desc
* `--summary-only` — Print only a summary line

  Default value: `false`
* `--no-summary` — Suppress summary (print JSON only)

  Default value: `false`



## `iroha sumeragi`

Sumeragi helpers (status)

**Usage:** `iroha sumeragi <COMMAND>`

###### **Subcommands:**

* `status` — Show consensus status snapshot (leader, HighestQC, LockedQC)
* `leader` — Show leader index (and PRF context when available)
* `params` — Show on-chain Sumeragi parameters snapshot
* `collectors` — Show current collector indices and peers
* `qc` — Show HighestQC/LockedQC snapshot
* `pacemaker` — Show pacemaker timers/config snapshot
* `phases` — Show latest per-phase latencies (ms)
* `telemetry` — Show aggregated telemetry snapshot (availability, QC, RBC, VRF)
* `evidence` — Evidence helpers (list/count/submit)
* `rbc` — RBC helpers (status/sessions)
* `vrf-penalties` — Show VRF penalties for the given epoch
* `vrf-epoch` — Show persisted VRF epoch snapshot (seed, participants, penalties)
* `exec-qc-get` — Fetch full ExecutionQC record (if present) for a parent block hash
* `exec-root-get` — Fetch execution root (if present) for a parent block hash



## `iroha sumeragi status`

Show consensus status snapshot (leader, HighestQC, LockedQC, membership digest)

**Usage:** `iroha sumeragi status [OPTIONS]`

###### **Options:**

* `--summary` — Print a single compact line instead of JSON (includes membership height/view/epoch/hash when available)

  Default value: `false`



## `iroha sumeragi leader`

Show leader index (and PRF context when available)

**Usage:** `iroha sumeragi leader [OPTIONS]`

###### **Options:**

* `--summary` — Print a single compact line instead of JSON

  Default value: `false`



## `iroha sumeragi params`

Show on-chain Sumeragi parameters snapshot

**Usage:** `iroha sumeragi params [OPTIONS]`

###### **Options:**

* `--summary` — Print a compact summary instead of JSON

  Default value: `false`



## `iroha sumeragi collectors`

Show current collector indices and peers

**Usage:** `iroha sumeragi collectors [OPTIONS]`

###### **Options:**

* `--summary` — Print a compact summary instead of JSON

  Default value: `false`



## `iroha sumeragi qc`

Show HighestQC/LockedQC snapshot

**Usage:** `iroha sumeragi qc [OPTIONS]`

###### **Options:**

* `--summary` — Print a compact summary instead of JSON

  Default value: `false`



## `iroha sumeragi pacemaker`

Show pacemaker timers/config snapshot

**Usage:** `iroha sumeragi pacemaker [OPTIONS]`

###### **Options:**

* `--summary` — Print a compact summary instead of JSON

  Default value: `false`



## `iroha sumeragi phases`

Show latest per-phase latencies (ms)

**Usage:** `iroha sumeragi phases [OPTIONS]`

###### **Options:**

* `--summary` — Print a compact summary instead of JSON

  Default value: `false`



## `iroha sumeragi telemetry`

Show aggregated telemetry snapshot (availability, QC, RBC, VRF)

**Usage:** `iroha sumeragi telemetry [OPTIONS]`

###### **Options:**

* `--summary` — Print a compact summary instead of JSON

  Default value: `false`



## `iroha sumeragi evidence`

Evidence helpers (list/count/submit)

**Usage:** `iroha sumeragi evidence <COMMAND>`

###### **Subcommands:**

* `list` — List persisted evidence entries
* `count` — Show evidence count
* `submit` — Submit hex-encoded evidence payload



## `iroha sumeragi evidence list`

List persisted evidence entries

**Usage:** `iroha sumeragi evidence list [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of entries to return
* `--offset <OFFSET>` — Offset into the evidence list
* `--kind <KIND>` — Filter by evidence kind

  Possible values: `double-prevote`, `double-precommit`, `double-exec-vote`, `invalid-qc`, `invalid-proposal`

* `--summary` — Print human-readable summaries before JSON

  Default value: `false`
* `--summary-only` — Print summaries only (omit JSON)

  Default value: `false`



## `iroha sumeragi evidence count`

Show evidence count

**Usage:** `iroha sumeragi evidence count [OPTIONS]`

###### **Options:**

* `--summary` — Print human-readable summary before JSON

  Default value: `false`
* `--summary-only` — Print summary only (omit JSON)

  Default value: `false`



## `iroha sumeragi evidence submit`

Submit hex-encoded evidence payload

**Usage:** `iroha sumeragi evidence submit [OPTIONS]`

###### **Options:**

* `--evidence-hex <EVIDENCE_HEX>` — Hex-encoded Norito evidence payload (0x optional)
* `--evidence-hex-file <PATH>` — Path to file containing hex-encoded proof (whitespace ignored)
* `--summary` — Print human-readable summary before JSON

  Default value: `false`
* `--summary-only` — Print summary only (omit JSON)

  Default value: `false`



## `iroha sumeragi rbc`

RBC helpers (status/sessions)

**Usage:** `iroha sumeragi rbc <COMMAND>`

###### **Subcommands:**

* `status` — Show RBC session/throughput counters
* `sessions` — Show RBC sessions snapshot



## `iroha sumeragi rbc status`

Show RBC session/throughput counters

**Usage:** `iroha sumeragi rbc status [OPTIONS]`

###### **Options:**

* `--summary` — Print a compact summary instead of JSON

  Default value: `false`



## `iroha sumeragi rbc sessions`

Show RBC sessions snapshot

**Usage:** `iroha sumeragi rbc sessions [OPTIONS]`

###### **Options:**

* `--summary` — Print a compact summary instead of JSON

  Default value: `false`



## `iroha sumeragi vrf-penalties`

Show VRF penalties for the given epoch

**Usage:** `iroha sumeragi vrf-penalties [OPTIONS] --epoch <EPOCH>`

###### **Options:**

* `--epoch <EPOCH>` — Epoch index (decimal or 0x-prefixed hex)
* `--summary` — Print a compact summary instead of JSON

  Default value: `false`



## `iroha sumeragi vrf-epoch`

Show persisted VRF epoch snapshot (seed, participants, penalties)

**Usage:** `iroha sumeragi vrf-epoch [OPTIONS] --epoch <EPOCH>`

###### **Options:**

* `--epoch <EPOCH>` — Epoch index (decimal or 0x-prefixed hex)
* `--summary` — Print a compact summary instead of JSON

  Default value: `false`



## `iroha sumeragi exec-qc-get`

Fetch full ExecutionQC record (if present) for a parent block hash

**Usage:** `iroha sumeragi exec-qc-get --hash <HASH>`

###### **Options:**

* `--hash <HASH>` — Block hash for which the ExecutionQC should be fetched



## `iroha sumeragi exec-root-get`

Fetch execution root (if present) for a parent block hash

**Usage:** `iroha sumeragi exec-root-get --hash <HASH>`

###### **Options:**

* `--hash <HASH>` — Block hash for which the execution root should be fetched



## `iroha contracts`

Contracts helpers (code storage)

**Usage:** `iroha contracts <COMMAND>`

###### **Subcommands:**

* `code-bytes-get` — Fetch on-chain contract code bytes by code hash and write to a file
* `deploy` — Deploy compiled `.to` code via Torii (POST /v1/contracts/deploy)
* `manifest` — Fetch on-chain contract manifest by code hash and either print or save (if --out is provided)
* `instances` — List active contract instances in a namespace (supports filters and pagination)



## `iroha contracts code-bytes-get`

Fetch on-chain contract code bytes by code hash and write to a file

**Usage:** `iroha contracts code-bytes-get --code-hash <HEX64> --out <PATH>`

###### **Options:**

* `--code-hash <HEX64>` — Hex-encoded 32-byte code hash (0x optional)
* `--out <PATH>` — Output path to write the `.to` bytes



## `iroha contracts deploy`

Deploy compiled `.to` code via Torii (POST /v1/contracts/deploy)

**Usage:** `iroha contracts deploy [OPTIONS] --authority <AUTHORITY> --private-key <HEX>`

###### **Options:**

* `--authority <AUTHORITY>` — Authority AccountId (e.g., alice@wonderland)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--code-file <CODE_FILE>` — Path to compiled `.to` file (mutually exclusive with --code-b64)
* `--code-b64 <CODE_B64>` — Base64-encoded code (mutually exclusive with --code-file)



## `iroha contracts manifest`

Fetch on-chain contract manifest by code hash and either print or save (if --out is provided)

**Usage:** `iroha contracts manifest [OPTIONS] --code-hash <HEX64>`

###### **Options:**

* `--code-hash <HEX64>` — Hex-encoded 32-byte code hash (0x optional)
* `--out <PATH>` — Optional output path; if provided, writes JSON manifest to file, otherwise prints to stdout



## `iroha contracts instances`

List active contract instances in a namespace (supports filters and pagination)

**Usage:** `iroha contracts instances [OPTIONS] --namespace <NS>`

###### **Options:**

* `--namespace <NS>` — Namespace to list (e.g., apps)
* `--contains <CONTAINS>` — Filter: contract_id substring (case-sensitive)
* `--hash-prefix <HASH_PREFIX>` — Filter: code hash hex prefix (lowercase)
* `--offset <OFFSET>` — Pagination offset
* `--limit <LIMIT>` — Pagination limit
* `--order <ORDER>` — Order: cid_asc (default), cid_desc, hash_asc, hash_desc
* `--table` — Render as a table instead of raw JSON
* `--short-hash` — When rendering a table, truncate the code hash (first 12 hex chars with ellipsis)



## `iroha runtime`

Runtime ABI/upgrades

**Usage:** `iroha runtime <COMMAND>`

###### **Subcommands:**

* `abi` — Runtime ABI helpers
* `upgrade` — Runtime upgrade management
* `status` — Show runtime metrics/status summary



## `iroha runtime abi`

Runtime ABI helpers

**Usage:** `iroha runtime abi <COMMAND>`

###### **Subcommands:**

* `active` — Fetch active ABI versions from the node
* `active-query` — Fetch active ABI versions via signed Norito query (core /query)
* `hash` — Fetch the node's canonical ABI hash for the active policy



## `iroha runtime abi active`

Fetch active ABI versions from the node

**Usage:** `iroha runtime abi active`



## `iroha runtime abi active-query`

Fetch active ABI versions via signed Norito query (core /query)

**Usage:** `iroha runtime abi active-query`



## `iroha runtime abi hash`

Fetch the node's canonical ABI hash for the active policy

**Usage:** `iroha runtime abi hash`



## `iroha runtime upgrade`

Runtime upgrade management

**Usage:** `iroha runtime upgrade <COMMAND>`

###### **Subcommands:**

* `list` — List proposed/activated runtime upgrades
* `propose` — Build a ProposeRuntimeUpgrade instruction skeleton via Torii
* `activate` — Build an ActivateRuntimeUpgrade instruction skeleton via Torii
* `cancel` — Build a CancelRuntimeUpgrade instruction skeleton via Torii



## `iroha runtime upgrade list`

List proposed/activated runtime upgrades

**Usage:** `iroha runtime upgrade list`



## `iroha runtime upgrade propose`

Build a ProposeRuntimeUpgrade instruction skeleton via Torii

**Usage:** `iroha runtime upgrade propose --file <PATH>`

###### **Options:**

* `--file <PATH>` — Path to a JSON file with RuntimeUpgradeManifest fields



## `iroha runtime upgrade activate`

Build an ActivateRuntimeUpgrade instruction skeleton via Torii

**Usage:** `iroha runtime upgrade activate --id <HEX>`

###### **Options:**

* `--id <HEX>` — Upgrade id (hex)



## `iroha runtime upgrade cancel`

Build a CancelRuntimeUpgrade instruction skeleton via Torii

**Usage:** `iroha runtime upgrade cancel --id <HEX>`

###### **Options:**

* `--id <HEX>` — Upgrade id (hex)



## `iroha runtime status`

Show runtime metrics/status summary

**Usage:** `iroha runtime status`



## `iroha audit`

Audit helpers (debug endpoints)

**Usage:** `iroha audit <COMMAND>`

###### **Subcommands:**

* `witness` — Fetch current execution witness snapshot from Torii debug endpoints



## `iroha audit witness`

Fetch current execution witness snapshot from Torii debug endpoints

**Usage:** `iroha audit witness [OPTIONS]`

###### **Options:**

* `--binary` — Fetch Norito-encoded binary instead of JSON
* `--out <PATH>` — Output path for binary; if omitted with --binary, hex is printed to stdout
* `--decode <PATH>` — Decode a Norito-encoded ExecWitness from a file and print with human-readable keys
* `--filter <PREFIXES>` — Filter decoded entries by key namespace prefix (comma-separated). Shorthand groups supported: - roles => [role, role.binding, perm.account, perm.role] - assets => [asset, asset_def.total] - all_assets => [asset, asset_def.total, asset_def.detail] - metadata => [account.detail, domain.detail, nft.detail, asset_def.detail] - all_meta => [account.detail, domain.detail, nft.detail, asset_def.detail] (alias of metadata) - perm | perms | permissions => [perm.account, perm.role] Examples: "assets,metadata", "roles", "account.detail,domain.detail". Applied only with --decode; prefixes match the human-readable key labels.

   Matching on the identifier segment supports: - exact (e.g., `account.detail:alice@wonderland`) - partial substring (e.g., `account.detail:wonderland`) - glob wildcards `*` and `?` (e.g., `asset:rose#*#*@wonderland`) - regex-like syntax `/.../` (treated as a glob pattern inside the slashes)



## `iroha kaigi`

Kaigi session helpers

**Usage:** `iroha kaigi <COMMAND>`

###### **Subcommands:**

* `create` — Create a new Kaigi session
* `quickstart` — Bootstrap a Kaigi session for demos
* `join` — Join a Kaigi session
* `leave` — Leave a Kaigi session
* `end` — End an active Kaigi session
* `record-usage` — Record usage statistics for a Kaigi session
* `report-relay-health` — Report the health status of a relay used by a Kaigi session

## `iroha kaigi quickstart`

Bootstrap a Kaigi session for demos

**Usage:** `iroha kaigi quickstart [OPTIONS]`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call.

  Default value: `wonderland`
* `--call-name <NAME>` — Call name within the domain (defaults to a timestamp-based identifier)
* `--host <ACCOUNT-ID>` — Host account identifier responsible for the call (defaults to the CLI config account)
* `--privacy-mode <PRIVACY_MODE>` — Privacy mode for the session (defaults to `transparent`)

  Possible values: `transparent`, `zk-roster-v1`
* `--room-policy <ROOM_POLICY>` — Room access policy controlling viewer authentication

  Possible values: `public`, `authenticated`
* `--relay-manifest <PATH>` — Path to a JSON file describing the relay manifest (optional)
* `--metadata-json <PATH>` — Path to a JSON file providing additional metadata (object with string keys)
* `--auto-join-host` — Automatically join the host account immediately after creation
* `--summary-out <PATH>` — File path where the JSON summary should be written (defaults to stdout only)
* `--spool-hint <PATH>` — Root directory where SoraNet spool files are expected (informational only)

  Default value: `storage/streaming/soranet_routes`



## `iroha kaigi create`

Create a new Kaigi session

**Usage:** `iroha kaigi create [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --host <ACCOUNT-ID>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call (e.g. `kaigi`)
* `--call-name <NAME>` — Call name within the domain (e.g. `daily-sync`)
* `--host <ACCOUNT-ID>` — Host account identifier responsible for the call
* `--title <TITLE>` — Optional human friendly title
* `--description <DESCRIPTION>` — Optional description for participants
* `--max-participants <U32>` — Maximum concurrent participants (excluding host)
* `--gas-rate-per-minute <U64>` — Gas rate charged per minute (defaults to 0)

  Default value: `0`
* `--billing-account <ACCOUNT-ID>` — Optional billing account that will cover usage
* `--scheduled-start-ms <U64>` — Optional scheduled start timestamp (milliseconds since epoch)
* `--privacy-mode <PRIVACY_MODE>` — Privacy mode for the session (defaults to `transparent`)

  Default value: `transparent`

  Possible values: `transparent`, `zk-roster-v1`

* `--room-policy <ROOM_POLICY>` — Room access policy controlling viewer authentication (defaults to `authenticated`)

  Default value: `authenticated`

  Possible values: `public`, `authenticated`

* `--relay-manifest <PATH>` — Path to a JSON file describing the relay manifest (optional)
* `--metadata-json <PATH>` — Path to a JSON file providing additional metadata (object with string keys)



## `iroha kaigi join`

Join a Kaigi session

**Usage:** `iroha kaigi join [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --participant <ACCOUNT-ID>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--participant <ACCOUNT-ID>` — Participant account joining the call
* `--commitment-hex <HEX>` — Commitment hash (hex) for privacy mode joins
* `--commitment-alias <COMMITMENT_ALIAS>` — Alias tag describing the commitment (privacy mode)
* `--nullifier-hex <HEX>` — Nullifier hash (hex) preventing duplicate joins (privacy mode)
* `--nullifier-issued-at-ms <U64>` — Nullifier issuance timestamp (milliseconds since epoch)
* `--roster-root-hex <HEX>` — Roster Merkle root bound into the proof transcript (privacy mode)
* `--proof-hex <HEX>` — Proof bytes attesting ownership (hex encoding of raw bytes)



## `iroha kaigi leave`

Leave a Kaigi session

**Usage:** `iroha kaigi leave [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --participant <ACCOUNT-ID>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--participant <ACCOUNT-ID>` — Participant account leaving the call
* `--commitment-hex <HEX>` — Commitment hash (hex) identifying the participant in privacy mode
* `--nullifier-hex <HEX>` — Nullifier hash (hex) preventing duplicate leaves (privacy mode)
* `--nullifier-issued-at-ms <U64>` — Nullifier issuance timestamp (milliseconds since epoch)
* `--roster-root-hex <HEX>` — Roster Merkle root bound into the proof transcript (privacy mode)
* `--proof-hex <HEX>` — Proof bytes attesting ownership (hex encoding of raw bytes)



## `iroha kaigi end`

End an active Kaigi session

**Usage:** `iroha kaigi end [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--ended-at-ms <U64>` — Optional timestamp in milliseconds when the call ended



## `iroha kaigi record-usage`

Record usage statistics for a Kaigi session

**Usage:** `iroha kaigi record-usage [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --duration-ms <U64>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--duration-ms <U64>` — Duration in milliseconds for this usage segment
* `--billed-gas <U64>` — Gas billed for this segment

  Default value: `0`
* `--usage-commitment-hex <HEX>` — Optional usage commitment hash (privacy mode)
* `--proof-hex <HEX>` — Optional proof bytes attesting the usage delta (privacy mode)



## `iroha kaigi report-relay-health`

Report the health status of a relay used by a Kaigi session

**Usage:** `iroha kaigi report-relay-health [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --relay <ACCOUNT-ID> --status <STATUS> --reported-at-ms <U64>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--relay <ACCOUNT-ID>` — Relay account identifier being reported
* `--status <STATUS>` — Observed health status for the relay

  Possible values: `healthy`, `degraded`, `unavailable`

* `--reported-at-ms <U64>` — Timestamp in milliseconds when the status was observed
* `--notes <NOTES>` — Optional notes capturing failure or recovery context



## `iroha alias`

Utilities for interacting with Torii alias endpoints and the alias VOPRF service.

**Usage:** `iroha alias <COMMAND>`

###### **Subcommands:**

* `voprf-evaluate` — Evaluate a blinded element using the alias VOPRF service
* `resolve` — Resolve an alias by its canonical name (`namespace/name`)
* `resolve-index` — Resolve an alias by Merkle index



## `iroha alias voprf-evaluate`

Evaluate a blinded element using the alias VOPRF service. This command is primarily
for development and forwards the request to `/v1/alias/voprf`.

**Usage:** `iroha alias voprf-evaluate --blinded-element-hex <HEX>`

###### **Options:**

* `--blinded-element-hex <HEX>` — Blinded element in hex encoding



## `iroha alias resolve`

Resolve an alias by its canonical name (`namespace/name`). The command validates the
format locally and then forwards the request to `/v1/sorafs/alias` unless `--dry-run`
is supplied.

**Usage:** `iroha alias resolve [OPTIONS] --alias <ALIAS>`

###### **Options:**

* `--alias <ALIAS>` — Alias name to resolve
* `--dry-run` — Print only validation result (skip future network call)

  Default value: `false`



## `iroha alias resolve-index`

Resolve an alias by Merkle index. This is useful when looking up entries directly
from the alias Merkle tree.

**Usage:** `iroha alias resolve-index --index <INDEX>`

###### **Options:**

* `--index <INDEX>` — Alias Merkle index to resolve


## `iroha sorafs storage`

Utilities for interacting with Torii SoraFS storage endpoints.

**Usage:** `iroha sorafs storage <COMMAND>`

###### **Subcommands:**

* `pin` — Submit a manifest + payload bundle to the local storage runtime.
* `token` — Stream token helpers for chunk-range fetching gateways.



## `iroha sorafs pin list`

List manifests recorded in the on-chain pin registry by calling
`/v1/sorafs/pin`. Optional filters allow selecting a status or paginating the
response. Responses include the attestation metadata so operators can verify
the snapshot against the latest block hash.

**Usage:** `iroha sorafs pin list [--status <pending|approved|retired>] [--limit <COUNT>] [--offset <COUNT>]`

###### **Options:**

* `--status <pending|approved|retired>` — Optional status filter.
* `--limit <COUNT>` — Maximum number of entries to return (defaults to 50, capped by the server).
* `--offset <COUNT>` — Offset for pagination.



## `iroha sorafs pin show`

Fetch a single manifest, including bound aliases and replication orders, by
calling `/v1/sorafs/pin/{digest}`. Alias proofs are evaluated against the
configured cache policy; stale or expired proofs result in HTTP warnings or
failures. Successful responses include the attestation object.

**Usage:** `iroha sorafs pin show --digest <HEX>`

###### **Options:**

* `--digest <HEX>` — Hex-encoded manifest digest to inspect.



## `iroha sorafs pin register`

Submit a manifest registration transaction using `/v1/sorafs/pin/register`.
The command decodes the provided Norito manifest, validates auxiliary inputs,
and signs the registration with the configured account/key. Optional alias
fields must be supplied together, and `--alias-proof` should point to the raw
proof bytes.

**Usage:** `iroha sorafs pin register --manifest <PATH> --chunk-digest <HEX> --submitted-epoch <EPOCH> [--alias-namespace <STRING> --alias-name <STRING> --alias-proof <PATH>] [--successor-of <HEX>]`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded manifest (`.to` file).
* `--chunk-digest <HEX>` — SHA3-256 digest of the chunk plan emitted by the chunker.
* `--submitted-epoch <EPOCH>` — Epoch number recorded for the submission.
* `--alias-namespace <STRING>` — Optional alias namespace to bind (requires `--alias-name` and `--alias-proof`).
* `--alias-name <STRING>` — Optional alias name to bind (requires the other alias options).
* `--alias-proof <PATH>` — Path to the alias proof bytes (base64 encoding is handled automatically).
* `--successor-of <HEX>` — Optional predecessor manifest digest establishing succession.



## `iroha sorafs storage pin`

Submit a Norito-encoded manifest together with its payload bytes to the local
storage runtime. The command base64-encodes both payloads and forwards them to
`/v1/sorafs/storage/pin`.

**Usage:** `iroha sorafs storage pin --manifest <PATH> --payload <PATH>`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded manifest (`.to` file).
* `--payload <PATH>` — Path to the raw payload bytes referenced by the manifest.



## `iroha sorafs storage token`

Stream token helpers used by chunk-range gateways and SDKs.

**Usage:** `iroha sorafs storage token <COMMAND>`

###### **Subcommands:**

* `issue` — Issue a signed stream token for a manifest/provider pair.



## `iroha sorafs storage token issue`

Issue a signed stream token for the specified manifest and provider. The command
adds `X-SoraFS-Client` and `X-SoraFS-Nonce` headers (generating a random nonce
when omitted) and forwards the request to `/v1/sorafs/storage/token`.

**Usage:** `iroha sorafs storage token issue --manifest-id <HEX> --provider-id <HEX> --client-id <STRING> [OPTIONS]`

###### **Options:**

* `--manifest-id <HEX>` — Manifest identifier stored on the gateway (hex).
* `--provider-id <HEX>` — Provider identifier authorised to serve the manifest (hex).
* `--client-id <STRING>` — Logical client identifier used for quota accounting.
* `--nonce <STRING>` — Optional nonce echoed back by the gateway; if omitted a 12-byte hex nonce is generated.
* `--ttl-secs <SECONDS>` — Override the default TTL in seconds.
* `--max-streams <COUNT>` — Override the maximum concurrent streams budget.
* `--rate-limit-bytes <BYTES>` — Override the sustained throughput limit (bytes per second).
* `--requests-per-minute <COUNT>` — Override the refresh allowance in requests per minute.



## `iroha repo`

Repo settlement helpers

**Usage:** `iroha repo <COMMAND>`

###### **Subcommands:**

* `initiate` — Initiate or roll a repo agreement between two counterparties
* `unwind` — Unwind an active repo agreement (reverse repo leg)
* `query` — Inspect repo agreements stored on-chain
* `margin` — Compute the next margin checkpoint for an agreement
* `margin-call` — Record a margin call for an active repo agreement



## `iroha repo initiate`

Initiate or roll a repo agreement between two counterparties

**Usage:** `iroha repo initiate --agreement-id <AGREEMENT_ID> --initiator <INITIATOR> --counterparty <COUNTERPARTY> --cash-asset <CASH_ASSET> --cash-quantity <CASH_QUANTITY> --collateral-asset <COLLATERAL_ASSET> --collateral-quantity <COLLATERAL_QUANTITY> --rate-bps <RATE_BPS> --maturity-timestamp-ms <MATURITY_TIMESTAMP_MS> --haircut-bps <HAIRCUT_BPS> --margin-frequency-secs <MARGIN_FREQUENCY_SECS> [--custodian <CUSTODIAN>]`

###### **Options:**

* `--agreement-id <AGREEMENT_ID>` — Stable identifier assigned to the repo agreement lifecycle
* `--initiator <INITIATOR>` — Initiating account submitting the repo
* `--counterparty <COUNTERPARTY>` — Counterparty receiving the repo cash leg
* `--custodian <CUSTODIAN>` — Optional custodian account holding pledged collateral (tri-party repos)
* `--cash-asset <CASH_ASSET>` — Cash asset definition identifier
* `--cash-quantity <CASH_QUANTITY>` — Cash quantity exchanged at initiation (integer or decimal)
* `--collateral-asset <COLLATERAL_ASSET>` — Collateral asset definition identifier
* `--collateral-quantity <COLLATERAL_QUANTITY>` — Collateral quantity pledged at initiation (integer or decimal)
* `--rate-bps <RATE_BPS>` — Fixed interest rate in basis points
* `--maturity-timestamp-ms <MATURITY_TIMESTAMP_MS>` — Unix timestamp (milliseconds) when the repo matures
* `--haircut-bps <HAIRCUT_BPS>` — Haircut applied to the collateral leg, in basis points
* `--margin-frequency-secs <MARGIN_FREQUENCY_SECS>` — Cadence between margin checks, in seconds (0 disables margining)



## `iroha repo unwind`

Unwind an active repo agreement (reverse repo leg)

**Usage:** `iroha repo unwind --agreement-id <AGREEMENT_ID> --initiator <INITIATOR> --counterparty <COUNTERPARTY> --cash-asset <CASH_ASSET> --cash-quantity <CASH_QUANTITY> --collateral-asset <COLLATERAL_ASSET> --collateral-quantity <COLLATERAL_QUANTITY> --settlement-timestamp-ms <SETTLEMENT_TIMESTAMP_MS>`

###### **Options:**

* `--agreement-id <AGREEMENT_ID>` — Stable identifier assigned to the repo agreement lifecycle
* `--initiator <INITIATOR>` — Initiating account performing the unwind
* `--counterparty <COUNTERPARTY>` — Counterparty receiving the unwind settlement
* `--cash-asset <CASH_ASSET>` — Cash asset definition identifier
* `--cash-quantity <CASH_QUANTITY>` — Cash quantity returned at unwind (integer or decimal)
* `--collateral-asset <COLLATERAL_ASSET>` — Collateral asset definition identifier
* `--collateral-quantity <COLLATERAL_QUANTITY>` — Collateral quantity released at unwind (integer or decimal)
* `--settlement-timestamp-ms <SETTLEMENT_TIMESTAMP_MS>` — Unix timestamp (milliseconds) when the unwind was agreed



## `iroha repo query`

Inspect repo agreements stored on-chain

**Usage:** `iroha repo query <COMMAND>`

###### **Subcommands:**

* `list` — List all repo agreements recorded on-chain
* `get` — Fetch a single repo agreement by identifier



## `iroha repo query list`

List all repo agreements recorded on-chain

**Usage:** `iroha repo query list`



## `iroha repo query get`

Fetch a single repo agreement by identifier

**Usage:** `iroha repo query get --id <ID>`

###### **Options:**

* `--id <ID>` — Stable identifier assigned to the repo agreement lifecycle



## `iroha repo margin`

Compute the next margin checkpoint for an agreement

**Usage:** `iroha repo margin --agreement-id <AGREEMENT_ID> [--at-timestamp-ms <AT_TIMESTAMP_MS>]`

###### **Options:**

* `--agreement-id <AGREEMENT_ID>` — Stable identifier assigned to the repo agreement lifecycle
* `--at-timestamp-ms <AT_TIMESTAMP_MS>` — Timestamp (ms) used when evaluating margin schedule (defaults to current time)



## `iroha repo margin-call`

Record a margin call for an active repo agreement

**Usage:** `iroha repo margin-call --agreement-id <AGREEMENT_ID>`

###### **Options:**

* `--agreement-id <AGREEMENT_ID>` — Stable identifier assigned to the repo agreement lifecycle



## `iroha settlement`

Delivery-versus-payment and payment-versus-payment helpers

**Usage:** `iroha settlement <COMMAND>`

###### **Subcommands:**

* `dvp` — Create a delivery-versus-payment settlement instruction
* `pvp` — Create a payment-versus-payment settlement instruction



## `iroha settlement dvp`

Create a delivery-versus-payment settlement instruction

**Usage:** `iroha settlement dvp --settlement-id <SETTLEMENT_ID> --delivery-asset <DELIVERY_ASSET> --delivery-quantity <DELIVERY_QUANTITY> --delivery-from <DELIVERY_FROM> --delivery-to <DELIVERY_TO> --payment-asset <PAYMENT_ASSET> --payment-quantity <PAYMENT_QUANTITY> --payment-from <PAYMENT_FROM> --payment-to <PAYMENT_TO> [--order <ORDER>] [--atomicity <ATOMICITY>]`

###### **Options:**

* `--settlement-id <SETTLEMENT_ID>` — Stable identifier shared across the settlement lifecycle
* `--delivery-asset <DELIVERY_ASSET>` — Asset definition delivered in exchange
* `--delivery-quantity <DELIVERY_QUANTITY>` — Quantity delivered (integer or decimal)
* `--delivery-from <DELIVERY_FROM>` — Account delivering the asset
* `--delivery-to <DELIVERY_TO>` — Account receiving the delivery leg
* `--payment-asset <PAYMENT_ASSET>` — Payment asset definition completing the settlement
* `--payment-quantity <PAYMENT_QUANTITY>` — Payment quantity (integer or decimal)
* `--payment-from <PAYMENT_FROM>` — Account sending the payment leg
* `--payment-to <PAYMENT_TO>` — Account receiving the payment leg
* `--order <ORDER>` — Execution order for the legs (`delivery-then-payment`, `payment-then-delivery`)
* `--atomicity <ATOMICITY>` — Atomicity policy for partial failures (`all-or-nothing`, `commit-first-leg`, `commit-second-leg`)



## `iroha settlement pvp`

Create a payment-versus-payment settlement instruction

**Usage:** `iroha settlement pvp --settlement-id <SETTLEMENT_ID> --primary-asset <PRIMARY_ASSET> --primary-quantity <PRIMARY_QUANTITY> --primary-from <PRIMARY_FROM> --primary-to <PRIMARY_TO> --counter-asset <COUNTER_ASSET> --counter-quantity <COUNTER_QUANTITY> --counter-from <COUNTER_FROM> --counter-to <COUNTER_TO> [--order <ORDER>] [--atomicity <ATOMICITY>]`

###### **Options:**

* `--settlement-id <SETTLEMENT_ID>` — Stable identifier shared across the settlement lifecycle
* `--primary-asset <PRIMARY_ASSET>` — Asset definition for the primary currency leg
* `--primary-quantity <PRIMARY_QUANTITY>` — Primary currency quantity (integer or decimal)
* `--primary-from <PRIMARY_FROM>` — Account delivering the primary currency
* `--primary-to <PRIMARY_TO>` — Account receiving the primary currency
* `--counter-asset <COUNTER_ASSET>` — Asset definition for the counter currency leg
* `--counter-quantity <COUNTER_QUANTITY>` — Counter currency quantity (integer or decimal)
* `--counter-from <COUNTER_FROM>` — Account delivering the counter currency
* `--counter-to <COUNTER_TO>` — Account receiving the counter currency
* `--order <ORDER>` — Execution order for the legs (`delivery-then-payment`, `payment-then-delivery`)
* `--atomicity <ATOMICITY>` — Atomicity policy for partial failures (`all-or-nothing`, `commit-first-leg`, `commit-second-leg`)


<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
