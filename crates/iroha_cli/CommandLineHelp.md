# Command-Line Help for `iroha_cli`

This document contains the help content for the `iroha_cli` command-line program.

**Command Overview:**

* [`iroha_cli`↴](#iroha_cli)
* [`iroha_cli ledger`↴](#iroha_cli-ledger)
* [`iroha_cli ledger domain`↴](#iroha_cli-ledger-domain)
* [`iroha_cli ledger domain list`↴](#iroha_cli-ledger-domain-list)
* [`iroha_cli ledger domain list all`↴](#iroha_cli-ledger-domain-list-all)
* [`iroha_cli ledger domain list filter`↴](#iroha_cli-ledger-domain-list-filter)
* [`iroha_cli ledger domain get`↴](#iroha_cli-ledger-domain-get)
* [`iroha_cli ledger domain register`↴](#iroha_cli-ledger-domain-register)
* [`iroha_cli ledger domain unregister`↴](#iroha_cli-ledger-domain-unregister)
* [`iroha_cli ledger domain transfer`↴](#iroha_cli-ledger-domain-transfer)
* [`iroha_cli ledger domain meta`↴](#iroha_cli-ledger-domain-meta)
* [`iroha_cli ledger domain meta get`↴](#iroha_cli-ledger-domain-meta-get)
* [`iroha_cli ledger domain meta set`↴](#iroha_cli-ledger-domain-meta-set)
* [`iroha_cli ledger domain meta remove`↴](#iroha_cli-ledger-domain-meta-remove)
* [`iroha_cli ledger account`↴](#iroha_cli-ledger-account)
* [`iroha_cli ledger account role`↴](#iroha_cli-ledger-account-role)
* [`iroha_cli ledger account role list`↴](#iroha_cli-ledger-account-role-list)
* [`iroha_cli ledger account role grant`↴](#iroha_cli-ledger-account-role-grant)
* [`iroha_cli ledger account role revoke`↴](#iroha_cli-ledger-account-role-revoke)
* [`iroha_cli ledger account permission`↴](#iroha_cli-ledger-account-permission)
* [`iroha_cli ledger account permission list`↴](#iroha_cli-ledger-account-permission-list)
* [`iroha_cli ledger account permission grant`↴](#iroha_cli-ledger-account-permission-grant)
* [`iroha_cli ledger account permission revoke`↴](#iroha_cli-ledger-account-permission-revoke)
* [`iroha_cli ledger account list`↴](#iroha_cli-ledger-account-list)
* [`iroha_cli ledger account list all`↴](#iroha_cli-ledger-account-list-all)
* [`iroha_cli ledger account list filter`↴](#iroha_cli-ledger-account-list-filter)
* [`iroha_cli ledger account get`↴](#iroha_cli-ledger-account-get)
* [`iroha_cli ledger account register`↴](#iroha_cli-ledger-account-register)
* [`iroha_cli ledger account unregister`↴](#iroha_cli-ledger-account-unregister)
* [`iroha_cli ledger account meta`↴](#iroha_cli-ledger-account-meta)
* [`iroha_cli ledger account meta get`↴](#iroha_cli-ledger-account-meta-get)
* [`iroha_cli ledger account meta set`↴](#iroha_cli-ledger-account-meta-set)
* [`iroha_cli ledger account meta remove`↴](#iroha_cli-ledger-account-meta-remove)
* [`iroha_cli ledger asset`↴](#iroha_cli-ledger-asset)
* [`iroha_cli ledger asset definition`↴](#iroha_cli-ledger-asset-definition)
* [`iroha_cli ledger asset definition list`↴](#iroha_cli-ledger-asset-definition-list)
* [`iroha_cli ledger asset definition list all`↴](#iroha_cli-ledger-asset-definition-list-all)
* [`iroha_cli ledger asset definition list filter`↴](#iroha_cli-ledger-asset-definition-list-filter)
* [`iroha_cli ledger asset definition get`↴](#iroha_cli-ledger-asset-definition-get)
* [`iroha_cli ledger asset definition register`↴](#iroha_cli-ledger-asset-definition-register)
* [`iroha_cli ledger asset definition unregister`↴](#iroha_cli-ledger-asset-definition-unregister)
* [`iroha_cli ledger asset definition transfer`↴](#iroha_cli-ledger-asset-definition-transfer)
* [`iroha_cli ledger asset definition meta`↴](#iroha_cli-ledger-asset-definition-meta)
* [`iroha_cli ledger asset definition meta get`↴](#iroha_cli-ledger-asset-definition-meta-get)
* [`iroha_cli ledger asset definition meta set`↴](#iroha_cli-ledger-asset-definition-meta-set)
* [`iroha_cli ledger asset definition meta remove`↴](#iroha_cli-ledger-asset-definition-meta-remove)
* [`iroha_cli ledger asset get`↴](#iroha_cli-ledger-asset-get)
* [`iroha_cli ledger asset list`↴](#iroha_cli-ledger-asset-list)
* [`iroha_cli ledger asset list all`↴](#iroha_cli-ledger-asset-list-all)
* [`iroha_cli ledger asset list filter`↴](#iroha_cli-ledger-asset-list-filter)
* [`iroha_cli ledger asset mint`↴](#iroha_cli-ledger-asset-mint)
* [`iroha_cli ledger asset burn`↴](#iroha_cli-ledger-asset-burn)
* [`iroha_cli ledger asset transfer`↴](#iroha_cli-ledger-asset-transfer)
* [`iroha_cli ledger nft`↴](#iroha_cli-ledger-nft)
* [`iroha_cli ledger nft get`↴](#iroha_cli-ledger-nft-get)
* [`iroha_cli ledger nft list`↴](#iroha_cli-ledger-nft-list)
* [`iroha_cli ledger nft list all`↴](#iroha_cli-ledger-nft-list-all)
* [`iroha_cli ledger nft list filter`↴](#iroha_cli-ledger-nft-list-filter)
* [`iroha_cli ledger nft register`↴](#iroha_cli-ledger-nft-register)
* [`iroha_cli ledger nft unregister`↴](#iroha_cli-ledger-nft-unregister)
* [`iroha_cli ledger nft transfer`↴](#iroha_cli-ledger-nft-transfer)
* [`iroha_cli ledger nft meta`↴](#iroha_cli-ledger-nft-meta)
* [`iroha_cli ledger nft meta get`↴](#iroha_cli-ledger-nft-meta-get)
* [`iroha_cli ledger nft meta set`↴](#iroha_cli-ledger-nft-meta-set)
* [`iroha_cli ledger nft meta remove`↴](#iroha_cli-ledger-nft-meta-remove)
* [`iroha_cli ledger peer`↴](#iroha_cli-ledger-peer)
* [`iroha_cli ledger peer list`↴](#iroha_cli-ledger-peer-list)
* [`iroha_cli ledger peer list all`↴](#iroha_cli-ledger-peer-list-all)
* [`iroha_cli ledger peer register`↴](#iroha_cli-ledger-peer-register)
* [`iroha_cli ledger peer unregister`↴](#iroha_cli-ledger-peer-unregister)
* [`iroha_cli ledger role`↴](#iroha_cli-ledger-role)
* [`iroha_cli ledger role permission`↴](#iroha_cli-ledger-role-permission)
* [`iroha_cli ledger role permission list`↴](#iroha_cli-ledger-role-permission-list)
* [`iroha_cli ledger role permission grant`↴](#iroha_cli-ledger-role-permission-grant)
* [`iroha_cli ledger role permission revoke`↴](#iroha_cli-ledger-role-permission-revoke)
* [`iroha_cli ledger role list`↴](#iroha_cli-ledger-role-list)
* [`iroha_cli ledger role list all`↴](#iroha_cli-ledger-role-list-all)
* [`iroha_cli ledger role register`↴](#iroha_cli-ledger-role-register)
* [`iroha_cli ledger role unregister`↴](#iroha_cli-ledger-role-unregister)
* [`iroha_cli ledger parameter`↴](#iroha_cli-ledger-parameter)
* [`iroha_cli ledger parameter list`↴](#iroha_cli-ledger-parameter-list)
* [`iroha_cli ledger parameter list all`↴](#iroha_cli-ledger-parameter-list-all)
* [`iroha_cli ledger parameter set`↴](#iroha_cli-ledger-parameter-set)
* [`iroha_cli ledger trigger`↴](#iroha_cli-ledger-trigger)
* [`iroha_cli ledger trigger list`↴](#iroha_cli-ledger-trigger-list)
* [`iroha_cli ledger trigger list all`↴](#iroha_cli-ledger-trigger-list-all)
* [`iroha_cli ledger trigger get`↴](#iroha_cli-ledger-trigger-get)
* [`iroha_cli ledger trigger register`↴](#iroha_cli-ledger-trigger-register)
* [`iroha_cli ledger trigger unregister`↴](#iroha_cli-ledger-trigger-unregister)
* [`iroha_cli ledger trigger mint`↴](#iroha_cli-ledger-trigger-mint)
* [`iroha_cli ledger trigger burn`↴](#iroha_cli-ledger-trigger-burn)
* [`iroha_cli ledger trigger meta`↴](#iroha_cli-ledger-trigger-meta)
* [`iroha_cli ledger trigger meta get`↴](#iroha_cli-ledger-trigger-meta-get)
* [`iroha_cli ledger trigger meta set`↴](#iroha_cli-ledger-trigger-meta-set)
* [`iroha_cli ledger trigger meta remove`↴](#iroha_cli-ledger-trigger-meta-remove)
* [`iroha_cli ledger query`↴](#iroha_cli-ledger-query)
* [`iroha_cli ledger query stdin`↴](#iroha_cli-ledger-query-stdin)
* [`iroha_cli ledger query stdin-raw`↴](#iroha_cli-ledger-query-stdin-raw)
* [`iroha_cli ledger transaction`↴](#iroha_cli-ledger-transaction)
* [`iroha_cli ledger transaction get`↴](#iroha_cli-ledger-transaction-get)
* [`iroha_cli ledger transaction ping`↴](#iroha_cli-ledger-transaction-ping)
* [`iroha_cli ledger transaction ivm`↴](#iroha_cli-ledger-transaction-ivm)
* [`iroha_cli ledger transaction stdin`↴](#iroha_cli-ledger-transaction-stdin)
* [`iroha_cli ledger multisig`↴](#iroha_cli-ledger-multisig)
* [`iroha_cli ledger multisig list`↴](#iroha_cli-ledger-multisig-list)
* [`iroha_cli ledger multisig list all`↴](#iroha_cli-ledger-multisig-list-all)
* [`iroha_cli ledger multisig register`↴](#iroha_cli-ledger-multisig-register)
* [`iroha_cli ledger multisig propose`↴](#iroha_cli-ledger-multisig-propose)
* [`iroha_cli ledger multisig approve`↴](#iroha_cli-ledger-multisig-approve)
* [`iroha_cli ledger multisig inspect`↴](#iroha_cli-ledger-multisig-inspect)
* [`iroha_cli ledger events`↴](#iroha_cli-ledger-events)
* [`iroha_cli ledger events state`↴](#iroha_cli-ledger-events-state)
* [`iroha_cli ledger events governance`↴](#iroha_cli-ledger-events-governance)
* [`iroha_cli ledger events transaction`↴](#iroha_cli-ledger-events-transaction)
* [`iroha_cli ledger events block`↴](#iroha_cli-ledger-events-block)
* [`iroha_cli ledger events trigger-execute`↴](#iroha_cli-ledger-events-trigger-execute)
* [`iroha_cli ledger events trigger-complete`↴](#iroha_cli-ledger-events-trigger-complete)
* [`iroha_cli ledger blocks`↴](#iroha_cli-ledger-blocks)
* [`iroha_cli ops`↴](#iroha_cli-ops)
* [`iroha_cli ops offline`↴](#iroha_cli-ops-offline)
* [`iroha_cli ops offline allowance`↴](#iroha_cli-ops-offline-allowance)
* [`iroha_cli ops offline allowance list`↴](#iroha_cli-ops-offline-allowance-list)
* [`iroha_cli ops offline allowance get`↴](#iroha_cli-ops-offline-allowance-get)
* [`iroha_cli ops offline transfer`↴](#iroha_cli-ops-offline-transfer)
* [`iroha_cli ops offline transfer list`↴](#iroha_cli-ops-offline-transfer-list)
* [`iroha_cli ops offline transfer get`↴](#iroha_cli-ops-offline-transfer-get)
* [`iroha_cli ops offline transfer proof`↴](#iroha_cli-ops-offline-transfer-proof)
* [`iroha_cli ops offline bundle`↴](#iroha_cli-ops-offline-bundle)
* [`iroha_cli ops offline bundle inspect`↴](#iroha_cli-ops-offline-bundle-inspect)
* [`iroha_cli ops offline summary`↴](#iroha_cli-ops-offline-summary)
* [`iroha_cli ops offline summary list`↴](#iroha_cli-ops-offline-summary-list)
* [`iroha_cli ops offline summary export`↴](#iroha_cli-ops-offline-summary-export)
* [`iroha_cli ops offline revocation`↴](#iroha_cli-ops-offline-revocation)
* [`iroha_cli ops offline revocation list`↴](#iroha_cli-ops-offline-revocation-list)
* [`iroha_cli ops offline rejection`↴](#iroha_cli-ops-offline-rejection)
* [`iroha_cli ops offline rejection stats`↴](#iroha_cli-ops-offline-rejection-stats)
* [`iroha_cli ops executor`↴](#iroha_cli-ops-executor)
* [`iroha_cli ops executor data-model`↴](#iroha_cli-ops-executor-data-model)
* [`iroha_cli ops executor upgrade`↴](#iroha_cli-ops-executor-upgrade)
* [`iroha_cli ops runtime`↴](#iroha_cli-ops-runtime)
* [`iroha_cli ops runtime abi`↴](#iroha_cli-ops-runtime-abi)
* [`iroha_cli ops runtime abi active`↴](#iroha_cli-ops-runtime-abi-active)
* [`iroha_cli ops runtime abi active-query`↴](#iroha_cli-ops-runtime-abi-active-query)
* [`iroha_cli ops runtime abi hash`↴](#iroha_cli-ops-runtime-abi-hash)
* [`iroha_cli ops runtime upgrade`↴](#iroha_cli-ops-runtime-upgrade)
* [`iroha_cli ops runtime upgrade list`↴](#iroha_cli-ops-runtime-upgrade-list)
* [`iroha_cli ops runtime upgrade propose`↴](#iroha_cli-ops-runtime-upgrade-propose)
* [`iroha_cli ops runtime upgrade activate`↴](#iroha_cli-ops-runtime-upgrade-activate)
* [`iroha_cli ops runtime upgrade cancel`↴](#iroha_cli-ops-runtime-upgrade-cancel)
* [`iroha_cli ops runtime status`↴](#iroha_cli-ops-runtime-status)
* [`iroha_cli ops runtime capabilities`↴](#iroha_cli-ops-runtime-capabilities)
* [`iroha_cli ops sumeragi`↴](#iroha_cli-ops-sumeragi)
* [`iroha_cli ops sumeragi status`↴](#iroha_cli-ops-sumeragi-status)
* [`iroha_cli ops sumeragi leader`↴](#iroha_cli-ops-sumeragi-leader)
* [`iroha_cli ops sumeragi params`↴](#iroha_cli-ops-sumeragi-params)
* [`iroha_cli ops sumeragi collectors`↴](#iroha_cli-ops-sumeragi-collectors)
* [`iroha_cli ops sumeragi qc`↴](#iroha_cli-ops-sumeragi-qc)
* [`iroha_cli ops sumeragi pacemaker`↴](#iroha_cli-ops-sumeragi-pacemaker)
* [`iroha_cli ops sumeragi phases`↴](#iroha_cli-ops-sumeragi-phases)
* [`iroha_cli ops sumeragi telemetry`↴](#iroha_cli-ops-sumeragi-telemetry)
* [`iroha_cli ops sumeragi evidence`↴](#iroha_cli-ops-sumeragi-evidence)
* [`iroha_cli ops sumeragi evidence list`↴](#iroha_cli-ops-sumeragi-evidence-list)
* [`iroha_cli ops sumeragi evidence count`↴](#iroha_cli-ops-sumeragi-evidence-count)
* [`iroha_cli ops sumeragi evidence submit`↴](#iroha_cli-ops-sumeragi-evidence-submit)
* [`iroha_cli ops sumeragi rbc`↴](#iroha_cli-ops-sumeragi-rbc)
* [`iroha_cli ops sumeragi rbc status`↴](#iroha_cli-ops-sumeragi-rbc-status)
* [`iroha_cli ops sumeragi rbc sessions`↴](#iroha_cli-ops-sumeragi-rbc-sessions)
* [`iroha_cli ops sumeragi vrf-penalties`↴](#iroha_cli-ops-sumeragi-vrf-penalties)
* [`iroha_cli ops sumeragi vrf-epoch`↴](#iroha_cli-ops-sumeragi-vrf-epoch)
* [`iroha_cli ops sumeragi commit-qc`↴](#iroha_cli-ops-sumeragi-commit-qc)
* [`iroha_cli ops sumeragi commit-qc get`↴](#iroha_cli-ops-sumeragi-commit-qc-get)
* [`iroha_cli ops audit`↴](#iroha_cli-ops-audit)
* [`iroha_cli ops audit witness`↴](#iroha_cli-ops-audit-witness)
* [`iroha_cli ops connect`↴](#iroha_cli-ops-connect)
* [`iroha_cli ops connect queue`↴](#iroha_cli-ops-connect-queue)
* [`iroha_cli ops connect queue inspect`↴](#iroha_cli-ops-connect-queue-inspect)
* [`iroha_cli app`↴](#iroha_cli-app)
* [`iroha_cli app gov`↴](#iroha_cli-app-gov)
* [`iroha_cli app gov deploy`↴](#iroha_cli-app-gov-deploy)
* [`iroha_cli app gov deploy propose`↴](#iroha_cli-app-gov-deploy-propose)
* [`iroha_cli app gov deploy meta`↴](#iroha_cli-app-gov-deploy-meta)
* [`iroha_cli app gov deploy audit`↴](#iroha_cli-app-gov-deploy-audit)
* [`iroha_cli app gov vote`↴](#iroha_cli-app-gov-vote)
* [`iroha_cli app gov proposal`↴](#iroha_cli-app-gov-proposal)
* [`iroha_cli app gov proposal get`↴](#iroha_cli-app-gov-proposal-get)
* [`iroha_cli app gov locks`↴](#iroha_cli-app-gov-locks)
* [`iroha_cli app gov locks get`↴](#iroha_cli-app-gov-locks-get)
* [`iroha_cli app gov council`↴](#iroha_cli-app-gov-council)
* [`iroha_cli app gov council derive-vrf`↴](#iroha_cli-app-gov-council-derive-vrf)
* [`iroha_cli app gov council persist`↴](#iroha_cli-app-gov-council-persist)
* [`iroha_cli app gov council gen-vrf`↴](#iroha_cli-app-gov-council-gen-vrf)
* [`iroha_cli app gov council derive-and-persist`↴](#iroha_cli-app-gov-council-derive-and-persist)
* [`iroha_cli app gov council replace`↴](#iroha_cli-app-gov-council-replace)
* [`iroha_cli app gov unlock`↴](#iroha_cli-app-gov-unlock)
* [`iroha_cli app gov unlock stats`↴](#iroha_cli-app-gov-unlock-stats)
* [`iroha_cli app gov referendum`↴](#iroha_cli-app-gov-referendum)
* [`iroha_cli app gov referendum get`↴](#iroha_cli-app-gov-referendum-get)
* [`iroha_cli app gov tally`↴](#iroha_cli-app-gov-tally)
* [`iroha_cli app gov tally get`↴](#iroha_cli-app-gov-tally-get)
* [`iroha_cli app gov finalize`↴](#iroha_cli-app-gov-finalize)
* [`iroha_cli app gov enact`↴](#iroha_cli-app-gov-enact)
* [`iroha_cli app gov protected`↴](#iroha_cli-app-gov-protected)
* [`iroha_cli app gov protected set`↴](#iroha_cli-app-gov-protected-set)
* [`iroha_cli app gov protected apply`↴](#iroha_cli-app-gov-protected-apply)
* [`iroha_cli app gov protected get`↴](#iroha_cli-app-gov-protected-get)
* [`iroha_cli app gov instance`↴](#iroha_cli-app-gov-instance)
* [`iroha_cli app gov instance activate`↴](#iroha_cli-app-gov-instance-activate)
* [`iroha_cli app gov instance list`↴](#iroha_cli-app-gov-instance-list)
* [`iroha_cli app contracts`↴](#iroha_cli-app-contracts)
* [`iroha_cli app contracts code`↴](#iroha_cli-app-contracts-code)
* [`iroha_cli app contracts code get`↴](#iroha_cli-app-contracts-code-get)
* [`iroha_cli app contracts deploy`↴](#iroha_cli-app-contracts-deploy)
* [`iroha_cli app contracts deploy-activate`↴](#iroha_cli-app-contracts-deploy-activate)
* [`iroha_cli app contracts manifest`↴](#iroha_cli-app-contracts-manifest)
* [`iroha_cli app contracts manifest get`↴](#iroha_cli-app-contracts-manifest-get)
* [`iroha_cli app contracts manifest build`↴](#iroha_cli-app-contracts-manifest-build)
* [`iroha_cli app contracts simulate`↴](#iroha_cli-app-contracts-simulate)
* [`iroha_cli app contracts instances`↴](#iroha_cli-app-contracts-instances)
* [`iroha_cli app zk`↴](#iroha_cli-app-zk)
* [`iroha_cli app zk roots`↴](#iroha_cli-app-zk-roots)
* [`iroha_cli app zk verify`↴](#iroha_cli-app-zk-verify)
* [`iroha_cli app zk submit-proof`↴](#iroha_cli-app-zk-submit-proof)
* [`iroha_cli app zk verify-batch`↴](#iroha_cli-app-zk-verify-batch)
* [`iroha_cli app zk schema-hash`↴](#iroha_cli-app-zk-schema-hash)
* [`iroha_cli app zk attachments`↴](#iroha_cli-app-zk-attachments)
* [`iroha_cli app zk attachments upload`↴](#iroha_cli-app-zk-attachments-upload)
* [`iroha_cli app zk attachments list`↴](#iroha_cli-app-zk-attachments-list)
* [`iroha_cli app zk attachments get`↴](#iroha_cli-app-zk-attachments-get)
* [`iroha_cli app zk attachments delete`↴](#iroha_cli-app-zk-attachments-delete)
* [`iroha_cli app zk attachments cleanup`↴](#iroha_cli-app-zk-attachments-cleanup)
* [`iroha_cli app zk register-asset`↴](#iroha_cli-app-zk-register-asset)
* [`iroha_cli app zk shield`↴](#iroha_cli-app-zk-shield)
* [`iroha_cli app zk unshield`↴](#iroha_cli-app-zk-unshield)
* [`iroha_cli app zk vk`↴](#iroha_cli-app-zk-vk)
* [`iroha_cli app zk vk register`↴](#iroha_cli-app-zk-vk-register)
* [`iroha_cli app zk vk update`↴](#iroha_cli-app-zk-vk-update)
* [`iroha_cli app zk vk get`↴](#iroha_cli-app-zk-vk-get)
* [`iroha_cli app zk proofs`↴](#iroha_cli-app-zk-proofs)
* [`iroha_cli app zk proofs list`↴](#iroha_cli-app-zk-proofs-list)
* [`iroha_cli app zk proofs count`↴](#iroha_cli-app-zk-proofs-count)
* [`iroha_cli app zk proofs get`↴](#iroha_cli-app-zk-proofs-get)
* [`iroha_cli app zk proofs retention`↴](#iroha_cli-app-zk-proofs-retention)
* [`iroha_cli app zk proofs prune`↴](#iroha_cli-app-zk-proofs-prune)
* [`iroha_cli app zk prover`↴](#iroha_cli-app-zk-prover)
* [`iroha_cli app zk prover reports`↴](#iroha_cli-app-zk-prover-reports)
* [`iroha_cli app zk prover reports list`↴](#iroha_cli-app-zk-prover-reports-list)
* [`iroha_cli app zk prover reports get`↴](#iroha_cli-app-zk-prover-reports-get)
* [`iroha_cli app zk prover reports delete`↴](#iroha_cli-app-zk-prover-reports-delete)
* [`iroha_cli app zk prover reports cleanup`↴](#iroha_cli-app-zk-prover-reports-cleanup)
* [`iroha_cli app zk prover reports count`↴](#iroha_cli-app-zk-prover-reports-count)
* [`iroha_cli app zk vote`↴](#iroha_cli-app-zk-vote)
* [`iroha_cli app zk vote tally`↴](#iroha_cli-app-zk-vote-tally)
* [`iroha_cli app zk envelope`↴](#iroha_cli-app-zk-envelope)
* [`iroha_cli app confidential`↴](#iroha_cli-app-confidential)
* [`iroha_cli app confidential create-keys`↴](#iroha_cli-app-confidential-create-keys)
* [`iroha_cli app confidential gas`↴](#iroha_cli-app-confidential-gas)
* [`iroha_cli app confidential gas get`↴](#iroha_cli-app-confidential-gas-get)
* [`iroha_cli app confidential gas set`↴](#iroha_cli-app-confidential-gas-set)
* [`iroha_cli app taikai`↴](#iroha_cli-app-taikai)
* [`iroha_cli app taikai bundle`↴](#iroha_cli-app-taikai-bundle)
* [`iroha_cli app taikai cek-rotate`↴](#iroha_cli-app-taikai-cek-rotate)
* [`iroha_cli app taikai rpt-attest`↴](#iroha_cli-app-taikai-rpt-attest)
* [`iroha_cli app taikai ingest`↴](#iroha_cli-app-taikai-ingest)
* [`iroha_cli app taikai ingest watch`↴](#iroha_cli-app-taikai-ingest-watch)
* [`iroha_cli app taikai ingest edge`↴](#iroha_cli-app-taikai-ingest-edge)
* [`iroha_cli app content`↴](#iroha_cli-app-content)
* [`iroha_cli app content publish`↴](#iroha_cli-app-content-publish)
* [`iroha_cli app content pack`↴](#iroha_cli-app-content-pack)
* [`iroha_cli app da`↴](#iroha_cli-app-da)
* [`iroha_cli app da submit`↴](#iroha_cli-app-da-submit)
* [`iroha_cli app da get`↴](#iroha_cli-app-da-get)
* [`iroha_cli app da get-blob`↴](#iroha_cli-app-da-get-blob)
* [`iroha_cli app da prove`↴](#iroha_cli-app-da-prove)
* [`iroha_cli app da prove-availability`↴](#iroha_cli-app-da-prove-availability)
* [`iroha_cli app da rent-quote`↴](#iroha_cli-app-da-rent-quote)
* [`iroha_cli app da rent-ledger`↴](#iroha_cli-app-da-rent-ledger)
* [`iroha_cli app streaming`↴](#iroha_cli-app-streaming)
* [`iroha_cli app streaming fingerprint`↴](#iroha_cli-app-streaming-fingerprint)
* [`iroha_cli app streaming suites`↴](#iroha_cli-app-streaming-suites)
* [`iroha_cli app nexus`↴](#iroha_cli-app-nexus)
* [`iroha_cli app nexus lane-report`↴](#iroha_cli-app-nexus-lane-report)
* [`iroha_cli app nexus public-lane`↴](#iroha_cli-app-nexus-public-lane)
* [`iroha_cli app nexus public-lane validators`↴](#iroha_cli-app-nexus-public-lane-validators)
* [`iroha_cli app nexus public-lane stake`↴](#iroha_cli-app-nexus-public-lane-stake)
* [`iroha_cli app staking`↴](#iroha_cli-app-staking)
* [`iroha_cli app staking register`↴](#iroha_cli-app-staking-register)
* [`iroha_cli app staking activate`↴](#iroha_cli-app-staking-activate)
* [`iroha_cli app staking exit`↴](#iroha_cli-app-staking-exit)
* [`iroha_cli app subscriptions`↴](#iroha_cli-app-subscriptions)
* [`iroha_cli app subscriptions plan`↴](#iroha_cli-app-subscriptions-plan)
* [`iroha_cli app subscriptions plan create`↴](#iroha_cli-app-subscriptions-plan-create)
* [`iroha_cli app subscriptions plan list`↴](#iroha_cli-app-subscriptions-plan-list)
* [`iroha_cli app subscriptions subscription`↴](#iroha_cli-app-subscriptions-subscription)
* [`iroha_cli app subscriptions subscription create`↴](#iroha_cli-app-subscriptions-subscription-create)
* [`iroha_cli app subscriptions subscription list`↴](#iroha_cli-app-subscriptions-subscription-list)
* [`iroha_cli app subscriptions subscription get`↴](#iroha_cli-app-subscriptions-subscription-get)
* [`iroha_cli app subscriptions subscription pause`↴](#iroha_cli-app-subscriptions-subscription-pause)
* [`iroha_cli app subscriptions subscription resume`↴](#iroha_cli-app-subscriptions-subscription-resume)
* [`iroha_cli app subscriptions subscription cancel`↴](#iroha_cli-app-subscriptions-subscription-cancel)
* [`iroha_cli app subscriptions subscription keep`↴](#iroha_cli-app-subscriptions-subscription-keep)
* [`iroha_cli app subscriptions subscription charge-now`↴](#iroha_cli-app-subscriptions-subscription-charge-now)
* [`iroha_cli app subscriptions subscription usage`↴](#iroha_cli-app-subscriptions-subscription-usage)
* [`iroha_cli app endorsement`↴](#iroha_cli-app-endorsement)
* [`iroha_cli app endorsement prepare`↴](#iroha_cli-app-endorsement-prepare)
* [`iroha_cli app endorsement submit`↴](#iroha_cli-app-endorsement-submit)
* [`iroha_cli app endorsement list`↴](#iroha_cli-app-endorsement-list)
* [`iroha_cli app endorsement policy`↴](#iroha_cli-app-endorsement-policy)
* [`iroha_cli app endorsement committee`↴](#iroha_cli-app-endorsement-committee)
* [`iroha_cli app endorsement register-committee`↴](#iroha_cli-app-endorsement-register-committee)
* [`iroha_cli app endorsement set-policy`↴](#iroha_cli-app-endorsement-set-policy)
* [`iroha_cli app jurisdiction`↴](#iroha_cli-app-jurisdiction)
* [`iroha_cli app jurisdiction verify`↴](#iroha_cli-app-jurisdiction-verify)
* [`iroha_cli app compute`↴](#iroha_cli-app-compute)
* [`iroha_cli app compute simulate`↴](#iroha_cli-app-compute-simulate)
* [`iroha_cli app compute invoke`↴](#iroha_cli-app-compute-invoke)
* [`iroha_cli app social`↴](#iroha_cli-app-social)
* [`iroha_cli app social claim-twitter-follow-reward`↴](#iroha_cli-app-social-claim-twitter-follow-reward)
* [`iroha_cli app social send-to-twitter`↴](#iroha_cli-app-social-send-to-twitter)
* [`iroha_cli app social cancel-twitter-escrow`↴](#iroha_cli-app-social-cancel-twitter-escrow)
* [`iroha_cli app space-directory`↴](#iroha_cli-app-space-directory)
* [`iroha_cli app space-directory manifest`↴](#iroha_cli-app-space-directory-manifest)
* [`iroha_cli app space-directory manifest publish`↴](#iroha_cli-app-space-directory-manifest-publish)
* [`iroha_cli app space-directory manifest encode`↴](#iroha_cli-app-space-directory-manifest-encode)
* [`iroha_cli app space-directory manifest revoke`↴](#iroha_cli-app-space-directory-manifest-revoke)
* [`iroha_cli app space-directory manifest expire`↴](#iroha_cli-app-space-directory-manifest-expire)
* [`iroha_cli app space-directory manifest audit-bundle`↴](#iroha_cli-app-space-directory-manifest-audit-bundle)
* [`iroha_cli app space-directory manifest fetch`↴](#iroha_cli-app-space-directory-manifest-fetch)
* [`iroha_cli app space-directory manifest scaffold`↴](#iroha_cli-app-space-directory-manifest-scaffold)
* [`iroha_cli app space-directory bindings`↴](#iroha_cli-app-space-directory-bindings)
* [`iroha_cli app space-directory bindings fetch`↴](#iroha_cli-app-space-directory-bindings-fetch)
* [`iroha_cli app kaigi`↴](#iroha_cli-app-kaigi)
* [`iroha_cli app kaigi create`↴](#iroha_cli-app-kaigi-create)
* [`iroha_cli app kaigi quickstart`↴](#iroha_cli-app-kaigi-quickstart)
* [`iroha_cli app kaigi join`↴](#iroha_cli-app-kaigi-join)
* [`iroha_cli app kaigi leave`↴](#iroha_cli-app-kaigi-leave)
* [`iroha_cli app kaigi end`↴](#iroha_cli-app-kaigi-end)
* [`iroha_cli app kaigi record-usage`↴](#iroha_cli-app-kaigi-record-usage)
* [`iroha_cli app kaigi report-relay-health`↴](#iroha_cli-app-kaigi-report-relay-health)
* [`iroha_cli app sorafs`↴](#iroha_cli-app-sorafs)
* [`iroha_cli app sorafs pin`↴](#iroha_cli-app-sorafs-pin)
* [`iroha_cli app sorafs pin list`↴](#iroha_cli-app-sorafs-pin-list)
* [`iroha_cli app sorafs pin show`↴](#iroha_cli-app-sorafs-pin-show)
* [`iroha_cli app sorafs pin register`↴](#iroha_cli-app-sorafs-pin-register)
* [`iroha_cli app sorafs alias`↴](#iroha_cli-app-sorafs-alias)
* [`iroha_cli app sorafs alias list`↴](#iroha_cli-app-sorafs-alias-list)
* [`iroha_cli app sorafs replication`↴](#iroha_cli-app-sorafs-replication)
* [`iroha_cli app sorafs replication list`↴](#iroha_cli-app-sorafs-replication-list)
* [`iroha_cli app sorafs storage`↴](#iroha_cli-app-sorafs-storage)
* [`iroha_cli app sorafs storage pin`↴](#iroha_cli-app-sorafs-storage-pin)
* [`iroha_cli app sorafs storage token`↴](#iroha_cli-app-sorafs-storage-token)
* [`iroha_cli app sorafs storage token issue`↴](#iroha_cli-app-sorafs-storage-token-issue)
* [`iroha_cli app sorafs gateway`↴](#iroha_cli-app-sorafs-gateway)
* [`iroha_cli app sorafs gateway lint-denylist`↴](#iroha_cli-app-sorafs-gateway-lint-denylist)
* [`iroha_cli app sorafs gateway update-denylist`↴](#iroha_cli-app-sorafs-gateway-update-denylist)
* [`iroha_cli app sorafs gateway template-config`↴](#iroha_cli-app-sorafs-gateway-template-config)
* [`iroha_cli app sorafs gateway generate-hosts`↴](#iroha_cli-app-sorafs-gateway-generate-hosts)
* [`iroha_cli app sorafs gateway route-plan`↴](#iroha_cli-app-sorafs-gateway-route-plan)
* [`iroha_cli app sorafs gateway cache-invalidate`↴](#iroha_cli-app-sorafs-gateway-cache-invalidate)
* [`iroha_cli app sorafs gateway evidence`↴](#iroha_cli-app-sorafs-gateway-evidence)
* [`iroha_cli app sorafs gateway direct-mode`↴](#iroha_cli-app-sorafs-gateway-direct-mode)
* [`iroha_cli app sorafs gateway direct-mode plan`↴](#iroha_cli-app-sorafs-gateway-direct-mode-plan)
* [`iroha_cli app sorafs gateway direct-mode enable`↴](#iroha_cli-app-sorafs-gateway-direct-mode-enable)
* [`iroha_cli app sorafs gateway direct-mode rollback`↴](#iroha_cli-app-sorafs-gateway-direct-mode-rollback)
* [`iroha_cli app sorafs gateway merkle`↴](#iroha_cli-app-sorafs-gateway-merkle)
* [`iroha_cli app sorafs gateway merkle snapshot`↴](#iroha_cli-app-sorafs-gateway-merkle-snapshot)
* [`iroha_cli app sorafs gateway merkle proof`↴](#iroha_cli-app-sorafs-gateway-merkle-proof)
* [`iroha_cli app sorafs incentives`↴](#iroha_cli-app-sorafs-incentives)
* [`iroha_cli app sorafs incentives compute`↴](#iroha_cli-app-sorafs-incentives-compute)
* [`iroha_cli app sorafs incentives open-dispute`↴](#iroha_cli-app-sorafs-incentives-open-dispute)
* [`iroha_cli app sorafs incentives dashboard`↴](#iroha_cli-app-sorafs-incentives-dashboard)
* [`iroha_cli app sorafs incentives service`↴](#iroha_cli-app-sorafs-incentives-service)
* [`iroha_cli app sorafs incentives service init`↴](#iroha_cli-app-sorafs-incentives-service-init)
* [`iroha_cli app sorafs incentives service process`↴](#iroha_cli-app-sorafs-incentives-service-process)
* [`iroha_cli app sorafs incentives service record`↴](#iroha_cli-app-sorafs-incentives-service-record)
* [`iroha_cli app sorafs incentives service dispute`↴](#iroha_cli-app-sorafs-incentives-service-dispute)
* [`iroha_cli app sorafs incentives service dispute file`↴](#iroha_cli-app-sorafs-incentives-service-dispute-file)
* [`iroha_cli app sorafs incentives service dispute resolve`↴](#iroha_cli-app-sorafs-incentives-service-dispute-resolve)
* [`iroha_cli app sorafs incentives service dispute reject`↴](#iroha_cli-app-sorafs-incentives-service-dispute-reject)
* [`iroha_cli app sorafs incentives service dashboard`↴](#iroha_cli-app-sorafs-incentives-service-dashboard)
* [`iroha_cli app sorafs incentives service audit`↴](#iroha_cli-app-sorafs-incentives-service-audit)
* [`iroha_cli app sorafs incentives service shadow-run`↴](#iroha_cli-app-sorafs-incentives-service-shadow-run)
* [`iroha_cli app sorafs incentives service reconcile`↴](#iroha_cli-app-sorafs-incentives-service-reconcile)
* [`iroha_cli app sorafs incentives service daemon`↴](#iroha_cli-app-sorafs-incentives-service-daemon)
* [`iroha_cli app sorafs handshake`↴](#iroha_cli-app-sorafs-handshake)
* [`iroha_cli app sorafs handshake show`↴](#iroha_cli-app-sorafs-handshake-show)
* [`iroha_cli app sorafs handshake update`↴](#iroha_cli-app-sorafs-handshake-update)
* [`iroha_cli app sorafs handshake token`↴](#iroha_cli-app-sorafs-handshake-token)
* [`iroha_cli app sorafs handshake token issue`↴](#iroha_cli-app-sorafs-handshake-token-issue)
* [`iroha_cli app sorafs handshake token id`↴](#iroha_cli-app-sorafs-handshake-token-id)
* [`iroha_cli app sorafs handshake token fingerprint`↴](#iroha_cli-app-sorafs-handshake-token-fingerprint)
* [`iroha_cli app sorafs toolkit`↴](#iroha_cli-app-sorafs-toolkit)
* [`iroha_cli app sorafs toolkit pack`↴](#iroha_cli-app-sorafs-toolkit-pack)
* [`iroha_cli app sorafs guard-directory`↴](#iroha_cli-app-sorafs-guard-directory)
* [`iroha_cli app sorafs guard-directory fetch`↴](#iroha_cli-app-sorafs-guard-directory-fetch)
* [`iroha_cli app sorafs guard-directory verify`↴](#iroha_cli-app-sorafs-guard-directory-verify)
* [`iroha_cli app sorafs reserve`↴](#iroha_cli-app-sorafs-reserve)
* [`iroha_cli app sorafs reserve quote`↴](#iroha_cli-app-sorafs-reserve-quote)
* [`iroha_cli app sorafs reserve ledger`↴](#iroha_cli-app-sorafs-reserve-ledger)
* [`iroha_cli app sorafs gar`↴](#iroha_cli-app-sorafs-gar)
* [`iroha_cli app sorafs gar receipt`↴](#iroha_cli-app-sorafs-gar-receipt)
* [`iroha_cli app sorafs repair`↴](#iroha_cli-app-sorafs-repair)
* [`iroha_cli app sorafs repair list`↴](#iroha_cli-app-sorafs-repair-list)
* [`iroha_cli app sorafs repair claim`↴](#iroha_cli-app-sorafs-repair-claim)
* [`iroha_cli app sorafs repair complete`↴](#iroha_cli-app-sorafs-repair-complete)
* [`iroha_cli app sorafs repair fail`↴](#iroha_cli-app-sorafs-repair-fail)
* [`iroha_cli app sorafs repair escalate`↴](#iroha_cli-app-sorafs-repair-escalate)
* [`iroha_cli app sorafs gc`↴](#iroha_cli-app-sorafs-gc)
* [`iroha_cli app sorafs gc inspect`↴](#iroha_cli-app-sorafs-gc-inspect)
* [`iroha_cli app sorafs gc dry-run`↴](#iroha_cli-app-sorafs-gc-dry-run)
* [`iroha_cli app sorafs fetch`↴](#iroha_cli-app-sorafs-fetch)
* [`iroha_cli app soracles`↴](#iroha_cli-app-soracles)
* [`iroha_cli app soracles bundle`↴](#iroha_cli-app-soracles-bundle)
* [`iroha_cli app soracles catalog`↴](#iroha_cli-app-soracles-catalog)
* [`iroha_cli app soracles evidence-gc`↴](#iroha_cli-app-soracles-evidence-gc)
* [`iroha_cli app sns`↴](#iroha_cli-app-sns)
* [`iroha_cli app sns register`↴](#iroha_cli-app-sns-register)
* [`iroha_cli app sns renew`↴](#iroha_cli-app-sns-renew)
* [`iroha_cli app sns transfer`↴](#iroha_cli-app-sns-transfer)
* [`iroha_cli app sns update-controllers`↴](#iroha_cli-app-sns-update-controllers)
* [`iroha_cli app sns freeze`↴](#iroha_cli-app-sns-freeze)
* [`iroha_cli app sns unfreeze`↴](#iroha_cli-app-sns-unfreeze)
* [`iroha_cli app sns registration`↴](#iroha_cli-app-sns-registration)
* [`iroha_cli app sns policy`↴](#iroha_cli-app-sns-policy)
* [`iroha_cli app sns governance`↴](#iroha_cli-app-sns-governance)
* [`iroha_cli app sns governance case`↴](#iroha_cli-app-sns-governance-case)
* [`iroha_cli app sns governance case create`↴](#iroha_cli-app-sns-governance-case-create)
* [`iroha_cli app sns governance case export`↴](#iroha_cli-app-sns-governance-case-export)
* [`iroha_cli app alias`↴](#iroha_cli-app-alias)
* [`iroha_cli app alias voprf-evaluate`↴](#iroha_cli-app-alias-voprf-evaluate)
* [`iroha_cli app alias resolve`↴](#iroha_cli-app-alias-resolve)
* [`iroha_cli app alias resolve-index`↴](#iroha_cli-app-alias-resolve-index)
* [`iroha_cli app repo`↴](#iroha_cli-app-repo)
* [`iroha_cli app repo initiate`↴](#iroha_cli-app-repo-initiate)
* [`iroha_cli app repo unwind`↴](#iroha_cli-app-repo-unwind)
* [`iroha_cli app repo query`↴](#iroha_cli-app-repo-query)
* [`iroha_cli app repo query list`↴](#iroha_cli-app-repo-query-list)
* [`iroha_cli app repo query get`↴](#iroha_cli-app-repo-query-get)
* [`iroha_cli app repo margin`↴](#iroha_cli-app-repo-margin)
* [`iroha_cli app repo margin-call`↴](#iroha_cli-app-repo-margin-call)
* [`iroha_cli app settlement`↴](#iroha_cli-app-settlement)
* [`iroha_cli app settlement dvp`↴](#iroha_cli-app-settlement-dvp)
* [`iroha_cli app settlement pvp`↴](#iroha_cli-app-settlement-pvp)
* [`iroha_cli tools`↴](#iroha_cli-tools)
* [`iroha_cli tools address`↴](#iroha_cli-tools-address)
* [`iroha_cli tools address convert`↴](#iroha_cli-tools-address-convert)
* [`iroha_cli tools address audit`↴](#iroha_cli-tools-address-audit)
* [`iroha_cli tools address normalize`↴](#iroha_cli-tools-address-normalize)
* [`iroha_cli tools crypto`↴](#iroha_cli-tools-crypto)
* [`iroha_cli tools crypto sm2`↴](#iroha_cli-tools-crypto-sm2)
* [`iroha_cli tools crypto sm2 keygen`↴](#iroha_cli-tools-crypto-sm2-keygen)
* [`iroha_cli tools crypto sm2 import`↴](#iroha_cli-tools-crypto-sm2-import)
* [`iroha_cli tools crypto sm2 export`↴](#iroha_cli-tools-crypto-sm2-export)
* [`iroha_cli tools crypto sm3`↴](#iroha_cli-tools-crypto-sm3)
* [`iroha_cli tools crypto sm3 hash`↴](#iroha_cli-tools-crypto-sm3-hash)
* [`iroha_cli tools crypto sm4`↴](#iroha_cli-tools-crypto-sm4)
* [`iroha_cli tools crypto sm4 gcm-seal`↴](#iroha_cli-tools-crypto-sm4-gcm-seal)
* [`iroha_cli tools crypto sm4 gcm-open`↴](#iroha_cli-tools-crypto-sm4-gcm-open)
* [`iroha_cli tools crypto sm4 ccm-seal`↴](#iroha_cli-tools-crypto-sm4-ccm-seal)
* [`iroha_cli tools crypto sm4 ccm-open`↴](#iroha_cli-tools-crypto-sm4-ccm-open)
* [`iroha_cli tools ivm`↴](#iroha_cli-tools-ivm)
* [`iroha_cli tools ivm abi-hash`↴](#iroha_cli-tools-ivm-abi-hash)
* [`iroha_cli tools ivm syscalls`↴](#iroha_cli-tools-ivm-syscalls)
* [`iroha_cli tools ivm manifest-gen`↴](#iroha_cli-tools-ivm-manifest-gen)
* [`iroha_cli tools markdown-help`↴](#iroha_cli-tools-markdown-help)
* [`iroha_cli tools version`↴](#iroha_cli-tools-version)

## `iroha_cli`

Iroha Client CLI provides a simple way to interact with the Iroha Web API

**Usage:** `iroha_cli [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `ledger` — Ledger data and transaction helpers
* `ops` — Node and operator helpers
* `app` — App API helpers and product tooling
* `tools` — Developer utilities and diagnostics

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
* `--output-format <OUTPUT_FORMAT>` — Output format for command responses

  Default value: `json`

  Possible values:
  - `json`:
    Emit JSON only
  - `text`:
    Emit human-readable text when available

* `--language <LANG>` — Language code for messages, overrides system language



## `iroha_cli ledger`

Ledger data and transaction helpers

**Usage:** `iroha_cli ledger <COMMAND>`

###### **Subcommands:**

* `domain` — Read and write domains
* `account` — Read and write accounts
* `asset` — Read and write assets
* `nft` — Read and write NFTs
* `peer` — Read and write peers
* `role` — Read and write roles
* `parameter` — Read and write system parameters
* `trigger` — Read and write triggers
* `query` — Read various data
* `transaction` — Read transactions and write various data
* `multisig` — Read and write multi-signature accounts and transactions
* `events` — Subscribe to events: state changes, transaction/block/trigger progress
* `blocks` — Subscribe to blocks



## `iroha_cli ledger domain`

Read and write domains

**Usage:** `iroha_cli ledger domain <COMMAND>`

###### **Subcommands:**

* `list` — List domains
* `get` — Retrieve details of a specific domain
* `register` — Register a domain
* `unregister` — Unregister a domain
* `transfer` — Transfer ownership of a domain
* `meta` — Read and write metadata



## `iroha_cli ledger domain list`

List domains

**Usage:** `iroha_cli ledger domain list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha_cli ledger domain list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha_cli ledger domain list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs (when supported)
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha_cli ledger domain list filter`

Filter by a given predicate

**Usage:** `iroha_cli ledger domain list filter [OPTIONS] <PREDICATE>`

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



## `iroha_cli ledger domain get`

Retrieve details of a specific domain

**Usage:** `iroha_cli ledger domain get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name



## `iroha_cli ledger domain register`

Register a domain

**Usage:** `iroha_cli ledger domain register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name



## `iroha_cli ledger domain unregister`

Unregister a domain

**Usage:** `iroha_cli ledger domain unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name



## `iroha_cli ledger domain transfer`

Transfer ownership of a domain

**Usage:** `iroha_cli ledger domain transfer --id <ID> --from <FROM> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name
* `-f`, `--from <FROM>` — Source account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `-t`, `--to <TO>` — Destination account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)



## `iroha_cli ledger domain meta`

Read and write metadata

**Usage:** `iroha_cli ledger domain meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha_cli ledger domain meta get`

Retrieve a value from the key-value store

**Usage:** `iroha_cli ledger domain meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger domain meta set`

Create or update an entry in the key-value store using JSON input from stdin

**Usage:** `iroha_cli ledger domain meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger domain meta remove`

Delete an entry from the key-value store

**Usage:** `iroha_cli ledger domain meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger account`

Read and write accounts

**Usage:** `iroha_cli ledger account <COMMAND>`

###### **Subcommands:**

* `role` — Read and write account roles
* `permission` — Read and write account permissions
* `list` — List accounts
* `get` — Retrieve details of a specific account
* `register` — Register an account
* `unregister` — Unregister an account
* `meta` — Read and write metadata



## `iroha_cli ledger account role`

Read and write account roles

**Usage:** `iroha_cli ledger account role <COMMAND>`

###### **Subcommands:**

* `list` — List account role IDs
* `grant` — Grant a role to an account
* `revoke` — Revoke a role from an account



## `iroha_cli ledger account role list`

List account role IDs

**Usage:** `iroha_cli ledger account role list [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha_cli ledger account role grant`

Grant a role to an account

**Usage:** `iroha_cli ledger account role grant --id <ID> --role <ROLE>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `-r`, `--role <ROLE>` — Role name



## `iroha_cli ledger account role revoke`

Revoke a role from an account

**Usage:** `iroha_cli ledger account role revoke --id <ID> --role <ROLE>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `-r`, `--role <ROLE>` — Role name



## `iroha_cli ledger account permission`

Read and write account permissions

**Usage:** `iroha_cli ledger account permission <COMMAND>`

###### **Subcommands:**

* `list` — List account permissions
* `grant` — Grant an account permission using JSON input from stdin
* `revoke` — Revoke an account permission using JSON input from stdin



## `iroha_cli ledger account permission list`

List account permissions

**Usage:** `iroha_cli ledger account permission list [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha_cli ledger account permission grant`

Grant an account permission using JSON input from stdin

**Usage:** `iroha_cli ledger account permission grant --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)



## `iroha_cli ledger account permission revoke`

Revoke an account permission using JSON input from stdin

**Usage:** `iroha_cli ledger account permission revoke --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)



## `iroha_cli ledger account list`

List accounts

**Usage:** `iroha_cli ledger account list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha_cli ledger account list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha_cli ledger account list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs (when supported)
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha_cli ledger account list filter`

Filter by a given predicate

**Usage:** `iroha_cli ledger account list filter [OPTIONS] <PREDICATE>`

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



## `iroha_cli ledger account get`

Retrieve details of a specific account

**Usage:** `iroha_cli ledger account get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)



## `iroha_cli ledger account register`

Register an account

**Usage:** `iroha_cli ledger account register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)



## `iroha_cli ledger account unregister`

Unregister an account

**Usage:** `iroha_cli ledger account unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)



## `iroha_cli ledger account meta`

Read and write metadata

**Usage:** `iroha_cli ledger account meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha_cli ledger account meta get`

Retrieve a value from the key-value store

**Usage:** `iroha_cli ledger account meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger account meta set`

Create or update an entry in the key-value store using JSON input from stdin

**Usage:** `iroha_cli ledger account meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger account meta remove`

Delete an entry from the key-value store

**Usage:** `iroha_cli ledger account meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger asset`

Read and write assets

**Usage:** `iroha_cli ledger asset <COMMAND>`

###### **Subcommands:**

* `definition` — Read and write asset definitions
* `get` — Retrieve details of a specific asset
* `list` — List assets
* `mint` — Increase the quantity of an asset
* `burn` — Decrease the quantity of an asset
* `transfer` — Transfer an asset between accounts



## `iroha_cli ledger asset definition`

Read and write asset definitions

**Usage:** `iroha_cli ledger asset definition <COMMAND>`

###### **Subcommands:**

* `list` — List asset definitions
* `get` — Retrieve details of a specific asset definition
* `register` — Register an asset definition
* `unregister` — Unregister an asset definition
* `transfer` — Transfer ownership of an asset definition
* `meta` — Read and write metadata



## `iroha_cli ledger asset definition list`

List asset definitions

**Usage:** `iroha_cli ledger asset definition list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha_cli ledger asset definition list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha_cli ledger asset definition list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs (when supported)
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha_cli ledger asset definition list filter`

Filter by a given predicate

**Usage:** `iroha_cli ledger asset definition list filter [OPTIONS] <PREDICATE>`

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



## `iroha_cli ledger asset definition get`

Retrieve details of a specific asset definition

**Usage:** `iroha_cli ledger asset definition get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition in the format "asset#domain"



## `iroha_cli ledger asset definition register`

Register an asset definition

**Usage:** `iroha_cli ledger asset definition register [OPTIONS] --id <ID>`

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



## `iroha_cli ledger asset definition unregister`

Unregister an asset definition

**Usage:** `iroha_cli ledger asset definition unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition in the format "asset#domain"



## `iroha_cli ledger asset definition transfer`

Transfer ownership of an asset definition

**Usage:** `iroha_cli ledger asset definition transfer --id <ID> --from <FROM> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition in the format "asset#domain"
* `-f`, `--from <FROM>` — Source account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `-t`, `--to <TO>` — Destination account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)



## `iroha_cli ledger asset definition meta`

Read and write metadata

**Usage:** `iroha_cli ledger asset definition meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha_cli ledger asset definition meta get`

Retrieve a value from the key-value store

**Usage:** `iroha_cli ledger asset definition meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger asset definition meta set`

Create or update an entry in the key-value store using JSON input from stdin

**Usage:** `iroha_cli ledger asset definition meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger asset definition meta remove`

Delete an entry from the key-value store

**Usage:** `iroha_cli ledger asset definition meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger asset get`

Retrieve details of a specific asset

**Usage:** `iroha_cli ledger asset get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in the format `asset#domain#account` or `asset##account`



## `iroha_cli ledger asset list`

List assets

**Usage:** `iroha_cli ledger asset list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha_cli ledger asset list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha_cli ledger asset list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs (when supported)
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha_cli ledger asset list filter`

Filter by a given predicate

**Usage:** `iroha_cli ledger asset list filter [OPTIONS] <PREDICATE>`

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



## `iroha_cli ledger asset mint`

Increase the quantity of an asset

**Usage:** `iroha_cli ledger asset mint --id <ID> --quantity <QUANTITY>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in the format `asset#domain#account` or `asset##account`
* `-q`, `--quantity <QUANTITY>` — Amount of change (integer or decimal)



## `iroha_cli ledger asset burn`

Decrease the quantity of an asset

**Usage:** `iroha_cli ledger asset burn --id <ID> --quantity <QUANTITY>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in the format `asset#domain#account` or `asset##account`
* `-q`, `--quantity <QUANTITY>` — Amount of change (integer or decimal)



## `iroha_cli ledger asset transfer`

Transfer an asset between accounts

**Usage:** `iroha_cli ledger asset transfer [OPTIONS] --id <ID> --to <TO> --quantity <QUANTITY>`

###### **Options:**

* `-i`, `--id <ID>` — Asset in the format `asset#domain#account` or `asset##account`
* `-t`, `--to <TO>` — Destination account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `-q`, `--quantity <QUANTITY>` — Transfer amount (integer or decimal)
* `--ensure-destination` — Attempt to register the destination when implicit receive is disabled



## `iroha_cli ledger nft`

Read and write NFTs

**Usage:** `iroha_cli ledger nft <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve details of a specific NFT
* `list` — List NFTs
* `register` — Register NFT with content provided from stdin in JSON format
* `unregister` — Unregister NFT
* `transfer` — Transfer ownership of NFT
* `meta` — Read and write metadata



## `iroha_cli ledger nft get`

Retrieve details of a specific NFT

**Usage:** `iroha_cli ledger nft get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"



## `iroha_cli ledger nft list`

List NFTs

**Usage:** `iroha_cli ledger nft list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha_cli ledger nft list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha_cli ledger nft list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs (when supported)
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha_cli ledger nft list filter`

Filter by a given predicate

**Usage:** `iroha_cli ledger nft list filter [OPTIONS] <PREDICATE>`

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



## `iroha_cli ledger nft register`

Register NFT with content provided from stdin in JSON format

**Usage:** `iroha_cli ledger nft register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"



## `iroha_cli ledger nft unregister`

Unregister NFT

**Usage:** `iroha_cli ledger nft unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"



## `iroha_cli ledger nft transfer`

Transfer ownership of NFT

**Usage:** `iroha_cli ledger nft transfer --id <ID> --from <FROM> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"
* `-f`, `--from <FROM>` — Source account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `-t`, `--to <TO>` — Destination account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)



## `iroha_cli ledger nft meta`

Read and write metadata

**Usage:** `iroha_cli ledger nft meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha_cli ledger nft meta get`

Retrieve a value from the key-value store

**Usage:** `iroha_cli ledger nft meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger nft meta set`

Create or update an entry in the key-value store using JSON input from stdin

**Usage:** `iroha_cli ledger nft meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger nft meta remove`

Delete an entry from the key-value store

**Usage:** `iroha_cli ledger nft meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger peer`

Read and write peers

**Usage:** `iroha_cli ledger peer <COMMAND>`

###### **Subcommands:**

* `list` — List registered peers expected to connect with each other
* `register` — Register a peer
* `unregister` — Unregister a peer



## `iroha_cli ledger peer list`

List registered peers expected to connect with each other

**Usage:** `iroha_cli ledger peer list <COMMAND>`

###### **Subcommands:**

* `all` — List all registered peers



## `iroha_cli ledger peer list all`

List all registered peers

**Usage:** `iroha_cli ledger peer list all [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs (when supported)
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha_cli ledger peer register`

Register a peer

**Usage:** `iroha_cli ledger peer register --key <KEY> --pop <HEX>`

###### **Options:**

* `-k`, `--key <KEY>` — Peer's public key in multihash format (must be BLS-normal)
* `--pop <HEX>` — Proof-of-possession bytes as hex (with or without 0x prefix)



## `iroha_cli ledger peer unregister`

Unregister a peer

**Usage:** `iroha_cli ledger peer unregister --key <KEY>`

###### **Options:**

* `-k`, `--key <KEY>` — Peer's public key in multihash format



## `iroha_cli ledger role`

Read and write roles

**Usage:** `iroha_cli ledger role <COMMAND>`

###### **Subcommands:**

* `permission` — Read and write role permissions
* `list` — List role IDs
* `register` — Register a role and grant it to the registrant
* `unregister` — Unregister a role



## `iroha_cli ledger role permission`

Read and write role permissions

**Usage:** `iroha_cli ledger role permission <COMMAND>`

###### **Subcommands:**

* `list` — List role permissions
* `grant` — Grant role permission using JSON input from stdin
* `revoke` — Revoke role permission using JSON input from stdin



## `iroha_cli ledger role permission list`

List role permissions

**Usage:** `iroha_cli ledger role permission list [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name
* `--limit <LIMIT>` — Maximum number of items to return (client-side for now)
* `--offset <OFFSET>` — Offset into the result set (client-side for now)

  Default value: `0`



## `iroha_cli ledger role permission grant`

Grant role permission using JSON input from stdin

**Usage:** `iroha_cli ledger role permission grant --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha_cli ledger role permission revoke`

Revoke role permission using JSON input from stdin

**Usage:** `iroha_cli ledger role permission revoke --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha_cli ledger role list`

List role IDs

**Usage:** `iroha_cli ledger role list <COMMAND>`

###### **Subcommands:**

* `all` — List all role IDs



## `iroha_cli ledger role list all`

List all role IDs

**Usage:** `iroha_cli ledger role list all [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha_cli ledger role register`

Register a role and grant it to the registrant

**Usage:** `iroha_cli ledger role register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha_cli ledger role unregister`

Unregister a role

**Usage:** `iroha_cli ledger role unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha_cli ledger parameter`

Read and write system parameters

**Usage:** `iroha_cli ledger parameter <COMMAND>`

###### **Subcommands:**

* `list` — List system parameters
* `set` — Set a system parameter using JSON input from stdin



## `iroha_cli ledger parameter list`

List system parameters

**Usage:** `iroha_cli ledger parameter list <COMMAND>`

###### **Subcommands:**

* `all` — List all system parameters



## `iroha_cli ledger parameter list all`

List all system parameters

**Usage:** `iroha_cli ledger parameter list all`



## `iroha_cli ledger parameter set`

Set a system parameter using JSON input from stdin

**Usage:** `iroha_cli ledger parameter set`



## `iroha_cli ledger trigger`

Read and write triggers

**Usage:** `iroha_cli ledger trigger <COMMAND>`

###### **Subcommands:**

* `list` — List trigger IDs
* `get` — Retrieve details of a specific trigger
* `register` — Register a trigger
* `unregister` — Unregister a trigger
* `mint` — Increase the number of trigger executions
* `burn` — Decrease the number of trigger executions
* `meta` — Read and write metadata



## `iroha_cli ledger trigger list`

List trigger IDs

**Usage:** `iroha_cli ledger trigger list <COMMAND>`

###### **Subcommands:**

* `all` — List all trigger IDs



## `iroha_cli ledger trigger list all`

List all trigger IDs

**Usage:** `iroha_cli ledger trigger list all [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha_cli ledger trigger get`

Retrieve details of a specific trigger

**Usage:** `iroha_cli ledger trigger get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name



## `iroha_cli ledger trigger register`

Register a trigger

**Usage:** `iroha_cli ledger trigger register [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name
* `-p`, `--path <PATH>` — Path to the compiled IVM bytecode to execute
* `--instructions-stdin` — Read JSON array of instructions from stdin instead of bytecode path Example: echo "[ {\"Log\": {\"level\": \"INFO\", \"message\": \"hi\"}} ]" | iroha trigger register -i `my_trig` --instructions-stdin
* `--instructions <PATH>` — Read JSON array of instructions from a file instead of bytecode path
* `-r`, `--repeats <REPEATS>` — Number of permitted executions (default: indefinitely)
* `--authority <AUTHORITY>` — Account executing the trigger (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--filter <FILTER>` — Filter type for the trigger

  Default value: `execute`

  Possible values: `execute`, `time`, `data`

* `--time-start-ms <TIME_START_MS>` — Start time in milliseconds since UNIX epoch for time filter
* `--time-period-ms <TIME_PERIOD_MS>` — Period in milliseconds for time filter (optional)
* `--data-filter <JSON>` — JSON for a `DataEventFilter` to use as filter
* `--data-domain <DATA_DOMAIN>` — Data filter preset: events within a domain
* `--data-account <DATA_ACCOUNT>` — Data filter preset: events for an account (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
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



## `iroha_cli ledger trigger unregister`

Unregister a trigger

**Usage:** `iroha_cli ledger trigger unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name



## `iroha_cli ledger trigger mint`

Increase the number of trigger executions

**Usage:** `iroha_cli ledger trigger mint --id <ID> --repetitions <REPETITIONS>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name
* `-r`, `--repetitions <REPETITIONS>` — Amount of change (integer)



## `iroha_cli ledger trigger burn`

Decrease the number of trigger executions

**Usage:** `iroha_cli ledger trigger burn --id <ID> --repetitions <REPETITIONS>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name
* `-r`, `--repetitions <REPETITIONS>` — Amount of change (integer)



## `iroha_cli ledger trigger meta`

Read and write metadata

**Usage:** `iroha_cli ledger trigger meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha_cli ledger trigger meta get`

Retrieve a value from the key-value store

**Usage:** `iroha_cli ledger trigger meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger trigger meta set`

Create or update an entry in the key-value store using JSON input from stdin

**Usage:** `iroha_cli ledger trigger meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger trigger meta remove`

Delete an entry from the key-value store

**Usage:** `iroha_cli ledger trigger meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha_cli ledger query`

Read various data

**Usage:** `iroha_cli ledger query <COMMAND>`

###### **Subcommands:**

* `stdin` — Query using JSON input from stdin
* `stdin-raw` — Query using raw `SignedQuery` (base64 or hex) from stdin



## `iroha_cli ledger query stdin`

Query using JSON input from stdin

**Usage:** `iroha_cli ledger query stdin`



## `iroha_cli ledger query stdin-raw`

Query using raw `SignedQuery` (base64 or hex) from stdin

**Usage:** `iroha_cli ledger query stdin-raw`



## `iroha_cli ledger transaction`

Read transactions and write various data

**Usage:** `iroha_cli ledger transaction <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve details of a specific transaction
* `ping` — Send an empty transaction that logs a message
* `ivm` — Send a transaction using IVM bytecode
* `stdin` — Send a transaction using JSON input from stdin



## `iroha_cli ledger transaction get`

Retrieve details of a specific transaction

**Usage:** `iroha_cli ledger transaction get --hash <HASH>`

###### **Options:**

* `-H`, `--hash <HASH>` — Hash of the transaction to retrieve



## `iroha_cli ledger transaction ping`

Send an empty transaction that logs a message

**Usage:** `iroha_cli ledger transaction ping [OPTIONS] --msg <MSG>`

###### **Options:**

* `-l`, `--log-level <LOG_LEVEL>` — Log levels: TRACE, DEBUG, INFO, WARN, ERROR (in increasing order of visibility)

  Default value: `INFO`
* `-m`, `--msg <MSG>` — Log message
* `--count <COUNT>` — Number of ping transactions to send

  Default value: `1`
* `--parallel <PARALLEL>` — Number of parallel workers to use when sending multiple pings

  Default value: `1`
* `--parallel-cap <PARALLEL_CAP>` — Maximum number of parallel workers (0 disables the cap)

  Default value: `1024`
* `--no-wait` — Submit without waiting for confirmation
* `--no-index` — Do not suffix message with "-<index>" when count > 1



## `iroha_cli ledger transaction ivm`

Send a transaction using IVM bytecode

**Usage:** `iroha_cli ledger transaction ivm [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the IVM bytecode file. If omitted, reads from stdin



## `iroha_cli ledger transaction stdin`

Send a transaction using JSON input from stdin

**Usage:** `iroha_cli ledger transaction stdin`



## `iroha_cli ledger multisig`

Read and write multi-signature accounts and transactions

**Usage:** `iroha_cli ledger multisig <COMMAND>`

###### **Subcommands:**

* `list` — List pending multisig transactions relevant to you
* `register` — Register a multisig account
* `propose` — Propose a multisig transaction using JSON input from stdin
* `approve` — Approve a multisig transaction
* `inspect` — Inspect a multisig account controller and print the CTAP2 payload + digest



## `iroha_cli ledger multisig list`

List pending multisig transactions relevant to you

**Usage:** `iroha_cli ledger multisig list <COMMAND>`

###### **Subcommands:**

* `all` — List all pending multisig transactions relevant to you



## `iroha_cli ledger multisig list all`

List all pending multisig transactions relevant to you

**Usage:** `iroha_cli ledger multisig list all [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of role IDs to scan for multisig (server-side limit)
* `--offset <OFFSET>` — Offset into the role ID set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for roles query



## `iroha_cli ledger multisig register`

Register a multisig account

**Usage:** `iroha_cli ledger multisig register [OPTIONS] --quorum <QUORUM>`

###### **Options:**

* `-s`, `--signatories <SIGNATORIES>` — List of signatories for the multisig account (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `-w`, `--weights <WEIGHTS>` — Relative weights of signatories' responsibilities
* `-q`, `--quorum <QUORUM>` — Threshold of total weight required for authentication
* `--account <ACCOUNT>` — Account id to use for the multisig controller. If omitted, a new random account is generated in the signatory domain and the private key is discarded locally
* `-t`, `--transaction-ttl <TRANSACTION_TTL>` — Time-to-live for multisig transactions. Example: "1y 6M 2w 3d 12h 30m 30s"

  Default value: `1h`



## `iroha_cli ledger multisig propose`

Propose a multisig transaction using JSON input from stdin

**Usage:** `iroha_cli ledger multisig propose [OPTIONS] --account <ACCOUNT>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — Multisig authority managing the proposed transaction
* `-t`, `--transaction-ttl <TRANSACTION_TTL>` — Overrides the default time-to-live for this transaction. Example: "1y 6M 2w 3d 12h 30m 30s" Must not exceed the multisig policy TTL; the CLI will preview the effective expiry and reject overrides above the policy cap



## `iroha_cli ledger multisig approve`

Approve a multisig transaction

**Usage:** `iroha_cli ledger multisig approve --account <ACCOUNT> --instructions-hash <INSTRUCTIONS_HASH>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — Multisig authority of the transaction
* `-i`, `--instructions-hash <INSTRUCTIONS_HASH>` — Hash of the instructions to approve



## `iroha_cli ledger multisig inspect`

Inspect a multisig account controller and print the CTAP2 payload + digest

**Usage:** `iroha_cli ledger multisig inspect [OPTIONS] --account <ACCOUNT>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — Multisig account identifier to inspect
* `--json` — Emit JSON instead of human-readable output



## `iroha_cli ledger events`

Subscribe to events: state changes, transaction/block/trigger progress

**Usage:** `iroha_cli ledger events [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `state` — Notify when the world state undergoes certain changes
* `governance` — Notify governance lifecycle events
* `transaction` — Notify when a transaction reaches specific stages
* `block` — Notify when a block reaches specific stages
* `trigger-execute` — Notify when a trigger execution is ordered
* `trigger-complete` — Notify when a trigger execution is completed

###### **Options:**

* `-t`, `--timeout <TIMEOUT>` — Duration to listen for events. Example: "1y 6M 2w 3d 12h 30m 30s"



## `iroha_cli ledger events state`

Notify when the world state undergoes certain changes

**Usage:** `iroha_cli ledger events state`



## `iroha_cli ledger events governance`

Notify governance lifecycle events

**Usage:** `iroha_cli ledger events governance [OPTIONS]`

###### **Options:**

* `--proposal-id <ID_HEX>` — Filter by proposal id (hex)
* `--referendum-id <RID>` — Filter by referendum id



## `iroha_cli ledger events transaction`

Notify when a transaction reaches specific stages

**Usage:** `iroha_cli ledger events transaction`



## `iroha_cli ledger events block`

Notify when a block reaches specific stages

**Usage:** `iroha_cli ledger events block`



## `iroha_cli ledger events trigger-execute`

Notify when a trigger execution is ordered

**Usage:** `iroha_cli ledger events trigger-execute`



## `iroha_cli ledger events trigger-complete`

Notify when a trigger execution is completed

**Usage:** `iroha_cli ledger events trigger-complete`



## `iroha_cli ledger blocks`

Subscribe to blocks

**Usage:** `iroha_cli ledger blocks [OPTIONS] <HEIGHT>`

###### **Arguments:**

* `<HEIGHT>` — Block height from which to start streaming blocks

###### **Options:**

* `-t`, `--timeout <TIMEOUT>` — Duration to listen for events. Example: "1y 6M 2w 3d 12h 30m 30s"



## `iroha_cli ops`

Node and operator helpers

**Usage:** `iroha_cli ops <COMMAND>`

###### **Subcommands:**

* `offline` — Inspect offline allowances and offline-to-online bundles
* `executor` — Read and write the executor
* `runtime` — Runtime ABI/upgrades
* `sumeragi` — Sumeragi helpers (status)
* `audit` — Audit helpers (debug endpoints)
* `connect` — Connect diagnostics helpers (queue inspection, evidence export)



## `iroha_cli ops offline`

Inspect offline allowances and offline-to-online bundles

**Usage:** `iroha_cli ops offline <COMMAND>`

###### **Subcommands:**

* `allowance` — Inspect offline allowances registered on-ledger
* `transfer` — Inspect pending offline-to-online transfer bundles
* `bundle` — Inspect offline bundle fixtures and aggregate proofs
* `summary` — Inspect derived counter summaries per offline certificate
* `revocation` — Inspect recorded verdict revocations
* `rejection` — Fetch offline rejection telemetry snapshots



## `iroha_cli ops offline allowance`

Inspect offline allowances registered on-ledger

**Usage:** `iroha_cli ops offline allowance <COMMAND>`

###### **Subcommands:**

* `list` — List all registered offline allowances
* `get` — Fetch a specific allowance by certificate id



## `iroha_cli ops offline allowance list`

List all registered offline allowances

**Usage:** `iroha_cli ops offline allowance list [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs (when supported)
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection
* `--controller <ACCOUNT_ID>` — Optional controller filter (account identifier)
* `--verdict-id <HEX>` — Optional verdict identifier filter (hex)
* `--attestation-nonce <HEX>` — Optional attestation nonce filter (hex)
* `--certificate-expires-before-ms <CERTIFICATE_EXPIRES_BEFORE_MS>` — Only show allowances whose certificate expiry is at or before this value
* `--certificate-expires-after-ms <CERTIFICATE_EXPIRES_AFTER_MS>` — Only show allowances whose certificate expiry is at or after this value
* `--policy-expires-before-ms <POLICY_EXPIRES_BEFORE_MS>` — Only show allowances whose policy expiry is at or before this value
* `--policy-expires-after-ms <POLICY_EXPIRES_AFTER_MS>` — Only show allowances whose policy expiry is at or after this value
* `--refresh-before-ms <REFRESH_BEFORE_MS>` — Only show allowances whose attestation refresh-by timestamp is at or before this value
* `--refresh-after-ms <REFRESH_AFTER_MS>` — Only show allowances whose attestation refresh-by timestamp is at or after this value
* `--summary` — Emit summary rows with expiry/verdict metadata instead of bare certificate ids
* `--include-expired` — Include certificates that have already expired (default skips them)



## `iroha_cli ops offline allowance get`

Fetch a specific allowance by certificate id

**Usage:** `iroha_cli ops offline allowance get --certificate-id <CERTIFICATE_ID>`

###### **Options:**

* `--certificate-id <CERTIFICATE_ID>` — Deterministic certificate identifier (hex)



## `iroha_cli ops offline transfer`

Inspect pending offline-to-online transfer bundles

**Usage:** `iroha_cli ops offline transfer <COMMAND>`

###### **Subcommands:**

* `list` — List all pending offline-to-online transfer bundles
* `get` — Fetch a specific transfer bundle by id
* `proof` — Generate a FASTPQ witness request for a bundle payload



## `iroha_cli ops offline transfer list`

List all pending offline-to-online transfer bundles

**Usage:** `iroha_cli ops offline transfer list [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs (when supported)
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection
* `--controller <ACCOUNT_ID>` — Optional controller filter (account identifier)
* `--receiver <ACCOUNT_ID>` — Optional receiver filter (account identifier)
* `--status <STATUS>` — Optional lifecycle status filter

  Possible values: `settled`, `archived`

* `--certificate-id <HEX>` — Only show bundles whose certificate id matches the provided hex value
* `--certificate-expires-before-ms <CERTIFICATE_EXPIRES_BEFORE_MS>` — Only show bundles whose certificate expiry is at or before this value
* `--certificate-expires-after-ms <CERTIFICATE_EXPIRES_AFTER_MS>` — Only show bundles whose certificate expiry is at or after this value
* `--policy-expires-before-ms <POLICY_EXPIRES_BEFORE_MS>` — Only show bundles whose policy expiry is at or before this value
* `--policy-expires-after-ms <POLICY_EXPIRES_AFTER_MS>` — Only show bundles whose policy expiry is at or after this value
* `--refresh-before-ms <REFRESH_BEFORE_MS>` — Only show bundles whose attestation refresh deadline is at or before this value
* `--refresh-after-ms <REFRESH_AFTER_MS>` — Only show bundles whose attestation refresh deadline is at or after this value
* `--verdict-id <HEX>` — Optional verdict identifier filter (hex)
* `--attestation-nonce <HEX>` — Optional attestation nonce filter (hex)
* `--platform-policy <PLATFORM_POLICY>` — Restrict settled bundles to a specific Android integrity policy (requires Play Integrity or HMS tokens)

  Possible values: `play-integrity`, `hms-safety-detect`

* `--require-verdict` — Include only bundles that already carry verdict metadata
* `--only-missing-verdict` — Include only bundles that are missing verdict metadata
* `--audit-log <PATH>` — Write a canonical audit log JSON file containing `{tx_id,sender_id,receiver_id,asset_id,amount,timestamp_ms}` entries
* `--summary` — Emit summary rows with certificate/verdict metadata instead of bare bundle ids



## `iroha_cli ops offline transfer get`

Fetch a specific transfer bundle by id

**Usage:** `iroha_cli ops offline transfer get --bundle-id <BUNDLE_ID>`

###### **Options:**

* `--bundle-id <BUNDLE_ID>` — Deterministic bundle identifier (hex)



## `iroha_cli ops offline transfer proof`

Generate a FASTPQ witness request for a bundle payload

**Usage:** `iroha_cli ops offline transfer proof [OPTIONS] --bundle <PATH> --kind <KIND>`

###### **Options:**

* `--bundle <PATH>` — Path to offline bundle payload (JSON or Norito)
* `--encoding <ENCODING>` — Override the bundle encoding detection

  Default value: `auto`

  Possible values: `auto`, `json`, `norito`

* `--kind <KIND>` — Witness type to build

  Possible values: `sum`, `counter`, `replay`

* `--counter-checkpoint <COUNTER_CHECKPOINT>` — Optional counter checkpoint (defaults to first counter - 1)
* `--replay-log-head <REPLAY_LOG_HEAD>` — Replay log head hash (required for replay proofs)
* `--replay-log-tail <REPLAY_LOG_TAIL>` — Replay log tail hash (required for replay proofs)



## `iroha_cli ops offline bundle`

Inspect offline bundle fixtures and aggregate proofs

**Usage:** `iroha_cli ops offline bundle <COMMAND>`

###### **Subcommands:**

* `inspect` — Inspect offline bundle fixtures and compute Poseidon receipts roots



## `iroha_cli ops offline bundle inspect`

Inspect offline bundle fixtures and compute Poseidon receipts roots

**Usage:** `iroha_cli ops offline bundle inspect [OPTIONS] <PATH>...`

###### **Arguments:**

* `<PATH>` — Paths to offline bundle fixtures (JSON or Norito)

###### **Options:**

* `--encoding <ENCODING>` — Override the bundle encoding detection

  Default value: `auto`

  Possible values: `auto`, `json`, `norito`

* `--proofs` — Include aggregate proof byte counts and metadata keys



## `iroha_cli ops offline summary`

Inspect derived counter summaries per offline certificate

**Usage:** `iroha_cli ops offline summary <COMMAND>`

###### **Subcommands:**

* `list` — List counter summaries derived from wallet allowances
* `export` — Export counter summaries to a JSON digest for receiver sharing



## `iroha_cli ops offline summary list`

List counter summaries derived from wallet allowances

**Usage:** `iroha_cli ops offline summary list [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs (when supported)
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha_cli ops offline summary export`

Export counter summaries to a JSON digest for receiver sharing

**Usage:** `iroha_cli ops offline summary export [OPTIONS] --output <PATH>`

###### **Options:**

* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection
* `--output <PATH>` — Destination file for the digest (JSON)
* `--pretty` — Pretty-print the JSON export instead of emitting a compact document

  Default value: `false`



## `iroha_cli ops offline revocation`

Inspect recorded verdict revocations

**Usage:** `iroha_cli ops offline revocation <COMMAND>`

###### **Subcommands:**

* `list` — List recorded verdict revocations



## `iroha_cli ops offline revocation list`

List recorded verdict revocations

**Usage:** `iroha_cli ops offline revocation list [OPTIONS]`

###### **Options:**

* `-v`, `--verbose` — Display detailed entry information instead of just IDs (when supported)
* `--sort-by-metadata-key <SORT_BY_METADATA_KEY>` — Sort by metadata key
* `--order <ORDER>` — Sort order (asc or desc)

  Possible values: `asc`, `desc`

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries
* `--select <SELECT>` — Experimental selector (JSON). Currently ignored; reserved for future server-side projection



## `iroha_cli ops offline rejection`

Fetch offline rejection telemetry snapshots

**Usage:** `iroha_cli ops offline rejection <COMMAND>`

###### **Subcommands:**

* `stats` — Fetch aggregated offline rejection counters



## `iroha_cli ops offline rejection stats`

Fetch aggregated offline rejection counters

**Usage:** `iroha_cli ops offline rejection stats [OPTIONS]`

###### **Options:**

* `--telemetry-profile <PROFILE>` — Optional telemetry profile header used when fetching stats



## `iroha_cli ops executor`

Read and write the executor

**Usage:** `iroha_cli ops executor <COMMAND>`

###### **Subcommands:**

* `data-model` — Retrieve the executor data model
* `upgrade` — Upgrade the executor



## `iroha_cli ops executor data-model`

Retrieve the executor data model

**Usage:** `iroha_cli ops executor data-model`



## `iroha_cli ops executor upgrade`

Upgrade the executor

**Usage:** `iroha_cli ops executor upgrade --path <PATH>`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the compiled IVM bytecode file



## `iroha_cli ops runtime`

Runtime ABI/upgrades

**Usage:** `iroha_cli ops runtime <COMMAND>`

###### **Subcommands:**

* `abi` — Runtime ABI helpers
* `upgrade` — Runtime upgrade management
* `status` — Show runtime metrics/status summary
* `capabilities` — Fetch node capability advert (ABI + crypto manifest)



## `iroha_cli ops runtime abi`

Runtime ABI helpers

**Usage:** `iroha_cli ops runtime abi <COMMAND>`

###### **Subcommands:**

* `active` — Fetch active ABI versions from the node
* `active-query` — Fetch active ABI versions via signed Norito query (core /query)
* `hash` — Fetch the node's canonical ABI hash for the active policy



## `iroha_cli ops runtime abi active`

Fetch active ABI versions from the node

**Usage:** `iroha_cli ops runtime abi active`



## `iroha_cli ops runtime abi active-query`

Fetch active ABI versions via signed Norito query (core /query)

**Usage:** `iroha_cli ops runtime abi active-query`



## `iroha_cli ops runtime abi hash`

Fetch the node's canonical ABI hash for the active policy

**Usage:** `iroha_cli ops runtime abi hash`



## `iroha_cli ops runtime upgrade`

Runtime upgrade management

**Usage:** `iroha_cli ops runtime upgrade <COMMAND>`

###### **Subcommands:**

* `list` — List proposed/activated runtime upgrades
* `propose` — Build a `ProposeRuntimeUpgrade` instruction skeleton via Torii
* `activate` — Build an `ActivateRuntimeUpgrade` instruction skeleton via Torii
* `cancel` — Build a `CancelRuntimeUpgrade` instruction skeleton via Torii



## `iroha_cli ops runtime upgrade list`

List proposed/activated runtime upgrades

**Usage:** `iroha_cli ops runtime upgrade list`



## `iroha_cli ops runtime upgrade propose`

Build a `ProposeRuntimeUpgrade` instruction skeleton via Torii

**Usage:** `iroha_cli ops runtime upgrade propose --file <PATH>`

###### **Options:**

* `--file <PATH>` — Path to a JSON file with `RuntimeUpgradeManifest` fields



## `iroha_cli ops runtime upgrade activate`

Build an `ActivateRuntimeUpgrade` instruction skeleton via Torii

**Usage:** `iroha_cli ops runtime upgrade activate --id <HEX>`

###### **Options:**

* `--id <HEX>` — Upgrade id (hex)



## `iroha_cli ops runtime upgrade cancel`

Build a `CancelRuntimeUpgrade` instruction skeleton via Torii

**Usage:** `iroha_cli ops runtime upgrade cancel --id <HEX>`

###### **Options:**

* `--id <HEX>` — Upgrade id (hex)



## `iroha_cli ops runtime status`

Show runtime metrics/status summary

**Usage:** `iroha_cli ops runtime status`



## `iroha_cli ops runtime capabilities`

Fetch node capability advert (ABI + crypto manifest)

**Usage:** `iroha_cli ops runtime capabilities`



## `iroha_cli ops sumeragi`

Sumeragi helpers (status)

**Usage:** `iroha_cli ops sumeragi <COMMAND>`

###### **Subcommands:**

* `status` — Show consensus status snapshot (leader, `HighestQC`, `LockedQC`)
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
* `commit-qc` — Fetch commit QC (if present) for a block hash



## `iroha_cli ops sumeragi status`

Show consensus status snapshot (leader, `HighestQC`, `LockedQC`)

**Usage:** `iroha_cli ops sumeragi status`



## `iroha_cli ops sumeragi leader`

Show leader index (and PRF context when available)

**Usage:** `iroha_cli ops sumeragi leader`



## `iroha_cli ops sumeragi params`

Show on-chain Sumeragi parameters snapshot

**Usage:** `iroha_cli ops sumeragi params`



## `iroha_cli ops sumeragi collectors`

Show current collector indices and peers

**Usage:** `iroha_cli ops sumeragi collectors`



## `iroha_cli ops sumeragi qc`

Show HighestQC/LockedQC snapshot

**Usage:** `iroha_cli ops sumeragi qc`



## `iroha_cli ops sumeragi pacemaker`

Show pacemaker timers/config snapshot

**Usage:** `iroha_cli ops sumeragi pacemaker`



## `iroha_cli ops sumeragi phases`

Show latest per-phase latencies (ms)

**Usage:** `iroha_cli ops sumeragi phases`



## `iroha_cli ops sumeragi telemetry`

Show aggregated telemetry snapshot (availability, QC, RBC, VRF)

**Usage:** `iroha_cli ops sumeragi telemetry`



## `iroha_cli ops sumeragi evidence`

Evidence helpers (list/count/submit)

**Usage:** `iroha_cli ops sumeragi evidence <COMMAND>`

###### **Subcommands:**

* `list` — List persisted evidence entries
* `count` — Show evidence count
* `submit` — Submit hex-encoded evidence payload



## `iroha_cli ops sumeragi evidence list`

List persisted evidence entries

**Usage:** `iroha_cli ops sumeragi evidence list [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of entries to return
* `--offset <OFFSET>` — Offset into the evidence list
* `--kind <KIND>` — Filter by evidence kind

  Possible values: `double-prepare`, `double-commit`, `invalid-qc`, `invalid-proposal`




## `iroha_cli ops sumeragi evidence count`

Show evidence count

**Usage:** `iroha_cli ops sumeragi evidence count`



## `iroha_cli ops sumeragi evidence submit`

Submit hex-encoded evidence payload

**Usage:** `iroha_cli ops sumeragi evidence submit [OPTIONS]`

###### **Options:**

* `--evidence-hex <EVIDENCE_HEX>` — Hex-encoded Norito evidence payload (0x optional)
* `--evidence-hex-file <PATH>` — Path to file containing hex-encoded proof (whitespace ignored)



## `iroha_cli ops sumeragi rbc`

RBC helpers (status/sessions)

**Usage:** `iroha_cli ops sumeragi rbc <COMMAND>`

###### **Subcommands:**

* `status` — Show RBC session/throughput counters
* `sessions` — Show RBC sessions snapshot



## `iroha_cli ops sumeragi rbc status`

Show RBC session/throughput counters

**Usage:** `iroha_cli ops sumeragi rbc status`



## `iroha_cli ops sumeragi rbc sessions`

Show RBC sessions snapshot

**Usage:** `iroha_cli ops sumeragi rbc sessions`



## `iroha_cli ops sumeragi vrf-penalties`

Show VRF penalties for the given epoch

**Usage:** `iroha_cli ops sumeragi vrf-penalties --epoch <EPOCH>`

###### **Options:**

* `--epoch <EPOCH>` — Epoch index (decimal or 0x-prefixed hex)



## `iroha_cli ops sumeragi vrf-epoch`

Show persisted VRF epoch snapshot (seed, participants, penalties)

**Usage:** `iroha_cli ops sumeragi vrf-epoch --epoch <EPOCH>`

###### **Options:**

* `--epoch <EPOCH>` — Epoch index (decimal or 0x-prefixed hex)



## `iroha_cli ops sumeragi commit-qc`

Fetch commit QC (if present) for a block hash

**Usage:** `iroha_cli ops sumeragi commit-qc <COMMAND>`

###### **Subcommands:**

* `get` — Fetch commit QC (if present) for a block hash



## `iroha_cli ops sumeragi commit-qc get`

Fetch commit QC (if present) for a block hash

**Usage:** `iroha_cli ops sumeragi commit-qc get --hash <HASH>`

###### **Options:**

* `--hash <HASH>` — Block hash for which the commit QC should be fetched



## `iroha_cli ops audit`

Audit helpers (debug endpoints)

**Usage:** `iroha_cli ops audit <COMMAND>`

###### **Subcommands:**

* `witness` — Fetch current execution witness snapshot from Torii debug endpoints



## `iroha_cli ops audit witness`

Fetch current execution witness snapshot from Torii debug endpoints

**Usage:** `iroha_cli ops audit witness [OPTIONS]`

###### **Options:**

* `--binary` — Fetch Norito-encoded binary instead of JSON
* `--out <PATH>` — Output path for binary; if omitted with --binary, hex is printed to stdout
* `--decode <PATH>` — Decode a Norito-encoded `ExecWitness` from a file and print with human-readable keys
* `--filter <PREFIXES>` — Filter decoded entries by key namespace prefix (comma-separated). Shorthand groups supported: - roles => [role, role.binding, perm.account, perm.role] - assets => [asset, `asset_def.total`] - `all_assets` => [asset, `asset_def.total`, `asset_def.detail`] - metadata => [account.detail, domain.detail, nft.detail, `asset_def.detail`] - `all_meta` => [account.detail, domain.detail, nft.detail, `asset_def.detail`] (alias of metadata) - perm | perms | permissions => [perm.account, perm.role] Examples: "assets,metadata", "roles", "account.detail,domain.detail". Applied only with --decode; prefixes match the human-readable key labels.

   Matching on the identifier segment supports: - exact (e.g., `account.detail:alice@wonderland`) - partial substring (e.g., `account.detail:wonderland`) - glob wildcards `*` and `?` (e.g., `asset:rose#*#*@wonderland`) - regex-like syntax `/.../` (treated as a glob pattern inside the slashes)
* `--fastpq-batches` — Include FASTPQ transition batches recorded in the witness when decoding (enabled by default)

  Default value: `true`
* `--no-fastpq-batches` — Disable FASTPQ batches to shrink the decoded output
* `--fastpq-parameter <NAME>` — Expected FASTPQ parameter set name; errors if batches use a different value

  Default value: `fastpq-lane-balanced`



## `iroha_cli ops connect`

Connect diagnostics helpers (queue inspection, evidence export)

**Usage:** `iroha_cli ops connect <COMMAND>`

###### **Subcommands:**

* `queue` — Queue inspection tooling



## `iroha_cli ops connect queue`

Queue inspection tooling

**Usage:** `iroha_cli ops connect queue <COMMAND>`

###### **Subcommands:**

* `inspect` — Inspect on-disk queue diagnostics for a Connect session



## `iroha_cli ops connect queue inspect`

Inspect on-disk queue diagnostics for a Connect session

**Usage:** `iroha_cli ops connect queue inspect [OPTIONS]`

###### **Options:**

* `--sid <SID>` — Connect session identifier (base64/base64url/hex). Required unless `--snapshot` is provided
* `--snapshot <SNAPSHOT>` — Path to an explicit snapshot JSON file (defaults to `<root>/<sid>/state.json`)
* `--root <ROOT>` — Root directory containing Connect queue state (defaults to `connect.queue.root` or `~/.iroha/connect`)
* `--metrics` — Include metrics summary derived from `metrics.ndjson`
* `--format <FORMAT>` — Output format (`table` or `json`)

  Default value: `table`

  Possible values: `table`, `json`




## `iroha_cli app`

App API helpers and product tooling

**Usage:** `iroha_cli app <COMMAND>`

###### **Subcommands:**

* `gov` — Governance helpers (app API convenience)
* `contracts` — Contracts helpers (code storage)
* `zk` — Zero-knowledge helpers (roots, etc.)
* `confidential` — Confidential asset tooling helpers
* `taikai` — Taikai publisher tooling (CAR bundler, envelopes)
* `content` — Content hosting helpers
* `da` — Data availability helpers (ingest tooling)
* `streaming` — Streaming helpers (HPKE fingerprints, suite listings)
* `nexus` — Nexus helpers (lanes, governance)
* `staking` — Public-lane staking helpers (register/activate/exit)
* `subscriptions` — Subscription plan and billing helpers
* `endorsement` — Domain endorsement helpers (committees, policies, submissions)
* `jurisdiction` — Jurisdiction Data Guardian helpers (attestations and SDN registries)
* `compute` — Compute lane simulation helpers
* `social` — Social incentive helpers (viral follow rewards and escrows)
* `space-directory` — Space Directory helpers (UAID capability manifests)
* `kaigi` — Kaigi session helpers
* `sorafs` — SoraFS helpers (pin registry, aliases, replication orders, storage)
* `soracles` — Soracles helpers (evidence bundling)
* `sns` — Sora Name Service helpers (registrar + policy tooling)
* `alias` — Alias helpers (placeholder pipeline)
* `repo` — Repo settlement helpers
* `settlement` — Delivery-versus-payment and payment-versus-payment helpers



## `iroha_cli app gov`

Governance helpers (app API convenience)

**Usage:** `iroha_cli app gov <COMMAND>`

###### **Subcommands:**

* `deploy` — Deployment helpers (propose/meta/audit)
* `vote` — Submit a governance ballot; auto-detects referendum mode unless overridden
* `proposal` — Proposal helpers
* `locks` — Lock helpers
* `council` — Get current sortition council or manage council VRF flows
* `unlock` — Unlock helpers (expired lock stats)
* `referendum` — Referendum helpers
* `tally` — Tally helpers
* `finalize` — Build a finalize transaction for a referendum (server returns instruction skeleton)
* `enact` — Build an enactment transaction for an approved proposal
* `protected` — Protected namespace helpers
* `instance` — Contract instance helpers



## `iroha_cli app gov deploy`

Deployment helpers (propose/meta/audit)

**Usage:** `iroha_cli app gov deploy <COMMAND>`

###### **Subcommands:**

* `propose` — Propose deployment of IVM bytecode by code/abi hash via governance (build-only; server returns instruction skeleton)
* `meta` — Build deploy metadata JSON for protected namespace admission
* `audit` — Audit stored manifests against governance proposals and code storage



## `iroha_cli app gov deploy propose`

Propose deployment of IVM bytecode by code/abi hash via governance (build-only; server returns instruction skeleton)

**Usage:** `iroha_cli app gov deploy propose [OPTIONS] --namespace <NAMESPACE> --contract-id <ID> --code-hash <CODE_HASH> --abi-hash <ABI_HASH>`

###### **Options:**

* `--namespace <NAMESPACE>`
* `--contract-id <ID>`
* `--code-hash <CODE_HASH>`
* `--abi-hash <ABI_HASH>`
* `--abi-version <ABI_VERSION>`

  Default value: `v1`
* `--window-lower <WINDOW_LOWER>` — Optional window lower bound (height)
* `--window-upper <WINDOW_UPPER>` — Optional window upper bound (height)
* `--mode <MODE>` — Optional voting mode for the referendum: Zk or Plain (defaults to server policy)

  Possible values: `Zk`, `Plain`




## `iroha_cli app gov deploy meta`

Build deploy metadata JSON for protected namespace admission

**Usage:** `iroha_cli app gov deploy meta [OPTIONS] --namespace <NAMESPACE> --contract-id <CONTRACT_ID>`

###### **Options:**

* `--namespace <NAMESPACE>`
* `--contract-id <CONTRACT_ID>`
* `--approver <ACCOUNT>` — Optional validator account IDs authorizing the deployment alongside the authority



## `iroha_cli app gov deploy audit`

Audit stored manifests against governance proposals and code storage

**Usage:** `iroha_cli app gov deploy audit [OPTIONS] --namespace <NS>`

###### **Options:**

* `--namespace <NS>` — Namespace to audit (e.g., apps)
* `--contains <CONTAINS>` — Filter: `contract_id` substring (case-sensitive)
* `--hash-prefix <HASH_PREFIX>` — Filter: code hash hex prefix (lowercase)
* `--offset <OFFSET>` — Pagination offset
* `--limit <LIMIT>` — Pagination limit
* `--order <ORDER>` — Order: `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`



## `iroha_cli app gov vote`

Submit a governance ballot; auto-detects referendum mode unless overridden

**Usage:** `iroha_cli app gov vote [OPTIONS] --referendum-id <REFERENDUM_ID>`

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
* `--nullifier <NULLIFIER>` — Optional 32-byte nullifier hint for ZK ballots (hex)



## `iroha_cli app gov proposal`

Proposal helpers

**Usage:** `iroha_cli app gov proposal <COMMAND>`

###### **Subcommands:**

* `get` — Get a governance proposal by id (hex)



## `iroha_cli app gov proposal get`

Get a governance proposal by id (hex)

**Usage:** `iroha_cli app gov proposal get --id <ID_HEX>`

###### **Options:**

* `--id <ID_HEX>`



## `iroha_cli app gov locks`

Lock helpers

**Usage:** `iroha_cli app gov locks <COMMAND>`

###### **Subcommands:**

* `get` — Get locks for a referendum id



## `iroha_cli app gov locks get`

Get locks for a referendum id

**Usage:** `iroha_cli app gov locks get --referendum-id <REFERENDUM_ID>`

###### **Options:**

* `--referendum-id <REFERENDUM_ID>`



## `iroha_cli app gov council`

Get current sortition council or manage council VRF flows

**Usage:** `iroha_cli app gov council [COMMAND]`

###### **Subcommands:**

* `derive-vrf` — 
* `persist` — 
* `gen-vrf` — 
* `derive-and-persist` — 
* `replace` — 



## `iroha_cli app gov council derive-vrf`

**Usage:** `iroha_cli app gov council derive-vrf [OPTIONS]`

###### **Options:**

* `--committee-size <N>` — Committee size to select
* `--alternate-size <N>` — Optional alternates to keep
* `--epoch <EPOCH>` — Optional epoch override
* `--candidate <CANDIDATES>` — Candidate spec: "`account_id,variant,pk_b64,proof_b64`"; repeatable
* `--candidates-file <PATH>` — Path to a JSON file with an array of candidates ({`account_id`, variant, `pk_b64`, `proof_b64`})



## `iroha_cli app gov council persist`

**Usage:** `iroha_cli app gov council persist [OPTIONS] --candidates-file <PATH> --authority <AUTHORITY> --private-key <HEX>`

###### **Options:**

* `--committee-size <COMMITTEE_SIZE>` — Committee size to select (top-k by VRF output)
* `--alternate-size <ALTERNATE_SIZE>` — Optional number of alternates to keep (defaults to committee size)
* `--epoch <EPOCH>` — Optional epoch override; defaults to `height/TERM_BLOCKS`
* `--candidates-file <PATH>` — Path to JSON file with candidates: [{ `account_id`, variant: Normal|Small, `pk_b64`, `proof_b64` }, ...]
* `--authority <AUTHORITY>` — Authority `AccountId` for signing (e.g., alice@wonderland)
* `--private-key <HEX>` — Private key (hex) for signing



## `iroha_cli app gov council gen-vrf`

**Usage:** `iroha_cli app gov council gen-vrf [OPTIONS] --chain-id <CHAIN_ID>`

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
* `--from-audit` — Fetch `seed/epoch/chain_id` from /v1/gov/council/audit (overrides --epoch/--beacon-hex when set)

  Default value: `false`



## `iroha_cli app gov council derive-and-persist`

**Usage:** `iroha_cli app gov council derive-and-persist [OPTIONS] --candidates-file <PATH> --authority <AUTHORITY> --private-key <HEX>`

###### **Options:**

* `--committee-size <COMMITTEE_SIZE>` — Committee size to select (top-k by VRF output)
* `--alternate-size <ALTERNATE_SIZE>` — Optional number of alternates to keep (defaults to committee size)
* `--epoch <EPOCH>` — Optional epoch override; defaults to `height/TERM_BLOCKS` (server-side)
* `--candidates-file <PATH>` — Path to JSON file with candidates: [{ `account_id`, variant: Normal|Small, `pk_b64`, `proof_b64` }, ...]
* `--authority <AUTHORITY>` — Authority `AccountId` for signing (e.g., alice@wonderland)
* `--private-key <HEX>` — Private key (hex) for signing
* `--wait` — Wait for `CouncilPersisted` event and verify via /v1/gov/council/current

  Default value: `false`



## `iroha_cli app gov council replace`

**Usage:** `iroha_cli app gov council replace [OPTIONS] --missing <MISSING> --authority <AUTHORITY> --private-key <HEX>`

###### **Options:**

* `--missing <MISSING>` — Account id of the member to replace (e.g., alice@wonderland)
* `--epoch <EPOCH>` — Optional epoch override; defaults to the latest persisted epoch
* `--authority <AUTHORITY>` — Authority `AccountId` for signing (e.g., alice@wonderland)
* `--private-key <HEX>` — Private key (hex) for signing



## `iroha_cli app gov unlock`

Unlock helpers (expired lock stats)

**Usage:** `iroha_cli app gov unlock <COMMAND>`

###### **Subcommands:**

* `stats` — Show governance unlock sweep stats (expired locks at current height)



## `iroha_cli app gov unlock stats`

Show governance unlock sweep stats (expired locks at current height)

**Usage:** `iroha_cli app gov unlock stats`



## `iroha_cli app gov referendum`

Referendum helpers

**Usage:** `iroha_cli app gov referendum <COMMAND>`

###### **Subcommands:**

* `get` — Get a referendum by id



## `iroha_cli app gov referendum get`

Get a referendum by id

**Usage:** `iroha_cli app gov referendum get --referendum-id <REFERENDUM_ID>`

###### **Options:**

* `--referendum-id <REFERENDUM_ID>`



## `iroha_cli app gov tally`

Tally helpers

**Usage:** `iroha_cli app gov tally <COMMAND>`

###### **Subcommands:**

* `get` — Get a tally snapshot by referendum id



## `iroha_cli app gov tally get`

Get a tally snapshot by referendum id

**Usage:** `iroha_cli app gov tally get --referendum-id <REFERENDUM_ID>`

###### **Options:**

* `--referendum-id <REFERENDUM_ID>`



## `iroha_cli app gov finalize`

Build a finalize transaction for a referendum (server returns instruction skeleton)

**Usage:** `iroha_cli app gov finalize --referendum-id <REFERENDUM_ID> --proposal-id <ID_HEX>`

###### **Options:**

* `--referendum-id <REFERENDUM_ID>` — Referendum id
* `--proposal-id <ID_HEX>` — Proposal id (hex 64)



## `iroha_cli app gov enact`

Build an enactment transaction for an approved proposal

**Usage:** `iroha_cli app gov enact [OPTIONS] --proposal-id <ID_HEX>`

###### **Options:**

* `--proposal-id <ID_HEX>` — Proposal id (hex 64)
* `--preimage-hash <PREIMAGE_HASH>` — Optional preimage hash (hex 64)
* `--window-lower <WINDOW_LOWER>` — Optional window lower bound (height)
* `--window-upper <WINDOW_UPPER>` — Optional window upper bound (height)



## `iroha_cli app gov protected`

Protected namespace helpers

**Usage:** `iroha_cli app gov protected <COMMAND>`

###### **Subcommands:**

* `set` — Set protected namespaces (custom parameter `gov_protected_namespaces`)
* `apply` — Apply protected namespaces on the server (requires API token if configured)
* `get` — Get protected namespaces (custom parameter `gov_protected_namespaces`)



## `iroha_cli app gov protected set`

Set protected namespaces (custom parameter `gov_protected_namespaces`)

**Usage:** `iroha_cli app gov protected set --namespaces <NAMESPACES>`

###### **Options:**

* `--namespaces <NAMESPACES>` — Comma-separated namespaces (e.g., apps,system)



## `iroha_cli app gov protected apply`

Apply protected namespaces on the server (requires API token if configured)

**Usage:** `iroha_cli app gov protected apply --namespaces <NAMESPACES>`

###### **Options:**

* `--namespaces <NAMESPACES>` — Comma-separated namespaces (e.g., apps,system)



## `iroha_cli app gov protected get`

Get protected namespaces (custom parameter `gov_protected_namespaces`)

**Usage:** `iroha_cli app gov protected get`



## `iroha_cli app gov instance`

Contract instance helpers

**Usage:** `iroha_cli app gov instance <COMMAND>`

###### **Subcommands:**

* `activate` — Activate a contract instance (namespace, `contract_id`) -> `code_hash` (admin/testing)
* `list` — List active contract instances for a namespace



## `iroha_cli app gov instance activate`

Activate a contract instance (namespace, `contract_id`) -> `code_hash` (admin/testing)

**Usage:** `iroha_cli app gov instance activate [OPTIONS] --namespace <NAMESPACE> --contract-id <CONTRACT_ID> --code-hash <HEX64>`

###### **Options:**

* `--namespace <NAMESPACE>`
* `--contract-id <CONTRACT_ID>`
* `--code-hash <HEX64>` — code hash hex (64 chars, 0x optional)
* `--blocking` — Submit and wait until committed or rejected

  Default value: `false`



## `iroha_cli app gov instance list`

List active contract instances for a namespace

**Usage:** `iroha_cli app gov instance list [OPTIONS] --namespace <NS>`

###### **Options:**

* `--namespace <NS>` — Namespace to list (e.g., apps)
* `--contains <CONTAINS>` — Filter: `contract_id` substring (case-sensitive)
* `--hash-prefix <HASH_PREFIX>` — Filter: code hash hex prefix (lowercase)
* `--offset <OFFSET>` — Pagination offset
* `--limit <LIMIT>` — Pagination limit
* `--order <ORDER>` — Order: `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`



## `iroha_cli app contracts`

Contracts helpers (code storage)

**Usage:** `iroha_cli app contracts <COMMAND>`

###### **Subcommands:**

* `code` — Contract code helpers
* `deploy` — Deploy compiled `.to` code via Torii (POST /v1/contracts/deploy)
* `deploy-activate` — Deploy bytecode, register manifest, and activate a namespace binding in one transaction
* `manifest` — Contract manifest helpers
* `simulate` — Run an offline simulation of IVM bytecode to see the queued ISIs and header metadata
* `instances` — List active contract instances in a namespace (supports filters and pagination)



## `iroha_cli app contracts code`

Contract code helpers

**Usage:** `iroha_cli app contracts code <COMMAND>`

###### **Subcommands:**

* `get` — Fetch on-chain contract code bytes by code hash and write to a file



## `iroha_cli app contracts code get`

Fetch on-chain contract code bytes by code hash and write to a file

**Usage:** `iroha_cli app contracts code get --code-hash <HEX64> --out <PATH>`

###### **Options:**

* `--code-hash <HEX64>` — Hex-encoded 32-byte code hash (0x optional)
* `--out <PATH>` — Output path to write the `.to` bytes



## `iroha_cli app contracts deploy`

Deploy compiled `.to` code via Torii (POST /v1/contracts/deploy)

**Usage:** `iroha_cli app contracts deploy [OPTIONS] --authority <AUTHORITY> --private-key <HEX>`

###### **Options:**

* `--authority <AUTHORITY>` — Authority account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--code-file <CODE_FILE>` — Path to compiled `.to` file (mutually exclusive with --code-b64)
* `--code-b64 <CODE_B64>` — Base64-encoded code (mutually exclusive with --code-file)



## `iroha_cli app contracts deploy-activate`

Deploy bytecode, register manifest, and activate a namespace binding in one transaction

**Usage:** `iroha_cli app contracts deploy-activate [OPTIONS] --authority <AUTHORITY> --private-key <HEX> --namespace <NAMESPACE> --contract-id <ID>`

###### **Options:**

* `--authority <AUTHORITY>` — Authority account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--private-key <HEX>` — Hex-encoded private key for signing and manifest provenance
* `--namespace <NAMESPACE>` — Governance namespace to bind (e.g., apps)
* `--contract-id <ID>` — Contract identifier within the namespace
* `--code-file <CODE_FILE>` — Path to compiled `.to` file (mutually exclusive with --code-b64)
* `--code-b64 <CODE_B64>` — Base64-encoded code (mutually exclusive with --code-file)
* `--manifest-out <PATH>` — Optional path to write the manifest JSON used in the transaction
* `--dry-run` — Preview transaction contents without submitting



## `iroha_cli app contracts manifest`

Contract manifest helpers

**Usage:** `iroha_cli app contracts manifest <COMMAND>`

###### **Subcommands:**

* `get` — Fetch on-chain contract manifest by code hash and either print or save (if --out is provided)
* `build` — Build a manifest for compiled bytecode (with optional signing)



## `iroha_cli app contracts manifest get`

Fetch on-chain contract manifest by code hash and either print or save (if --out is provided)

**Usage:** `iroha_cli app contracts manifest get [OPTIONS] --code-hash <HEX64>`

###### **Options:**

* `--code-hash <HEX64>` — Hex-encoded 32-byte code hash (0x optional)
* `--out <PATH>` — Optional output path; if provided, writes JSON manifest to file, otherwise prints to stdout



## `iroha_cli app contracts manifest build`

Build a manifest for compiled bytecode (with optional signing)

**Usage:** `iroha_cli app contracts manifest build [OPTIONS]`

###### **Options:**

* `--code-file <CODE_FILE>` — Path to compiled `.to` file (mutually exclusive with --code-b64)
* `--code-b64 <CODE_B64>` — Base64-encoded code (mutually exclusive with --code-file)
* `--sign-with <HEX>` — Hex-encoded private key for signing the manifest (optional)
* `--out <PATH>` — Optional output path; if omitted, prints to stdout



## `iroha_cli app contracts simulate`

Run an offline simulation of IVM bytecode to see the queued ISIs and header metadata

**Usage:** `iroha_cli app contracts simulate [OPTIONS] --authority <AUTHORITY> --private-key <HEX> --gas-limit <GAS_LIMIT>`

###### **Options:**

* `--authority <AUTHORITY>` — Authority account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--private-key <HEX>` — Hex-encoded private key used to sign the simulated transaction
* `--code-file <CODE_FILE>` — Path to compiled `.to` file (mutually exclusive with --code-b64)
* `--code-b64 <CODE_B64>` — Base64-encoded code (mutually exclusive with --code-file)
* `--gas-limit <GAS_LIMIT>` — Required `gas_limit` metadata to include in the simulated transaction
* `--namespace <NAMESPACE>` — Optional contract namespace metadata for call-time binding checks
* `--contract-id <CONTRACT_ID>` — Optional contract identifier metadata for call-time binding checks



## `iroha_cli app contracts instances`

List active contract instances in a namespace (supports filters and pagination)

**Usage:** `iroha_cli app contracts instances [OPTIONS] --namespace <NS>`

###### **Options:**

* `--namespace <NS>` — Namespace to list (e.g., apps)
* `--contains <CONTAINS>` — Filter: `contract_id` substring (case-sensitive)
* `--hash-prefix <HASH_PREFIX>` — Filter: code hash hex prefix (lowercase)
* `--offset <OFFSET>` — Pagination offset
* `--limit <LIMIT>` — Pagination limit
* `--order <ORDER>` — Order: `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`
* `--table` — Render as a table instead of raw JSON
* `--short-hash` — When rendering a table, truncate the code hash (first 12 hex chars with ellipsis)



## `iroha_cli app zk`

Zero-knowledge helpers (roots, etc.)

**Usage:** `iroha_cli app zk <COMMAND>`

###### **Subcommands:**

* `roots` — Get recent shielded roots for an asset (JSON). Posts to /v1/zk/roots
* `verify` — Verify a ZK proof by posting an `OpenVerifyEnvelope` (Norito) or a JSON DTO to /v1/zk/verify
* `submit-proof` — Submit a ZK proof envelope for later reference/inspection. Posts to /v1/zk/submit-proof
* `verify-batch` — Verify a batch of ZK `OpenVerify` envelopes (Norito vector) via /v1/zk/verify-batch
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



## `iroha_cli app zk roots`

Get recent shielded roots for an asset (JSON). Posts to /v1/zk/roots

**Usage:** `iroha_cli app zk roots [OPTIONS] --asset-id <ASSET_ID>`

###### **Options:**

* `--asset-id <ASSET_ID>` — `AssetDefinitionId` like `rose#wonderland`
* `--max <MAX>` — Maximum number of roots to return (0 = server cap)

  Default value: `0`



## `iroha_cli app zk verify`

Verify a ZK proof by posting an `OpenVerifyEnvelope` (Norito) or a JSON DTO to /v1/zk/verify

**Usage:** `iroha_cli app zk verify [OPTIONS]`

###### **Options:**

* `--norito <PATH>` — Path to Norito-encoded `OpenVerifyEnvelope` bytes (mutually exclusive with --json)
* `--json <PATH>` — Path to a JSON DTO describing the proof (backend, proof, vk) (mutually exclusive with --norito)



## `iroha_cli app zk submit-proof`

Submit a ZK proof envelope for later reference/inspection. Posts to /v1/zk/submit-proof

**Usage:** `iroha_cli app zk submit-proof [OPTIONS]`

###### **Options:**

* `--norito <PATH>` — Path to Norito-encoded proof envelope bytes (mutually exclusive with --json)
* `--json <PATH>` — Path to a JSON DTO describing the proof (backend, proof, vk) (mutually exclusive with --norito)



## `iroha_cli app zk verify-batch`

Verify a batch of ZK `OpenVerify` envelopes (Norito vector) via /v1/zk/verify-batch

**Usage:** `iroha_cli app zk verify-batch [OPTIONS]`

###### **Options:**

* `--norito <PATH>` — Path to a Norito-encoded Vec<OpenVerifyEnvelope> (mutually exclusive with --json)
* `--json <PATH>` — Path to a JSON array of base64-encoded Norito `OpenVerifyEnvelope` items (mutually exclusive with --norito)



## `iroha_cli app zk schema-hash`

Compute the Blake2b-32 hash required for `public_inputs_schema_hash` and print it

**Usage:** `iroha_cli app zk schema-hash [OPTIONS]`

###### **Options:**

* `--norito <PATH>` — Path to a Norito-encoded `OpenVerifyEnvelope`
* `--public-inputs-hex <HEX>` — Hex-encoded public inputs (when not using --norito)



## `iroha_cli app zk attachments`

Manage ZK attachments in the app API

**Usage:** `iroha_cli app zk attachments <COMMAND>`

###### **Subcommands:**

* `upload` — Upload a file as an attachment. Returns JSON metadata
* `list` — List stored attachments (JSON array of metadata)
* `get` — Download an attachment by id to a file
* `delete` — Delete an attachment by id
* `cleanup` — Cleanup attachments by filters (age/content-type/ids). Deletes individually via API



## `iroha_cli app zk attachments upload`

Upload a file as an attachment. Returns JSON metadata

**Usage:** `iroha_cli app zk attachments upload [OPTIONS] --file <PATH>`

###### **Options:**

* `--file <PATH>` — Path to the file to upload
* `--content-type <MIME>` — Content-Type to send with the file

  Default value: `application/octet-stream`



## `iroha_cli app zk attachments list`

List stored attachments (JSON array of metadata)

**Usage:** `iroha_cli app zk attachments list`



## `iroha_cli app zk attachments get`

Download an attachment by id to a file

**Usage:** `iroha_cli app zk attachments get --id <ID> --out <PATH>`

###### **Options:**

* `--id <ID>` — Attachment id (hex)
* `--out <PATH>` — Output path to write the downloaded bytes



## `iroha_cli app zk attachments delete`

Delete an attachment by id

**Usage:** `iroha_cli app zk attachments delete --id <ID>`

###### **Options:**

* `--id <ID>` — Attachment id (hex)



## `iroha_cli app zk attachments cleanup`

Cleanup attachments by filters (age/content-type/ids). Deletes individually via API

**Usage:** `iroha_cli app zk attachments cleanup [OPTIONS]`

###### **Options:**

* `--yes` — Proceed without confirmation
* `--all` — Delete all attachments (dangerous). Requires --yes
* `--content-type <MIME>` — Filter by content-type substring (e.g., application/x-norito)
* `--before-ms <MS>` — Filter attachments created strictly before this UNIX epoch in milliseconds
* `--older-than-secs <SECS>` — Filter attachments older than N seconds (relative to now)
* `--id <ID>` — Filter by specific id(s); may be repeated
* `--limit <N>` — Maximum number of attachments to delete (applied after filtering)
* `--ids-only` — Preview only: list matching ids instead of full metadata
* `--summary` — Preview only: print a summary table (id, `content_type`, size, `created_ms`)



## `iroha_cli app zk register-asset`

Register a ZK-capable asset (Hybrid mode) with policy and VK ids

**Usage:** `iroha_cli app zk register-asset [OPTIONS] --asset <ASSET_ID>`

###### **Options:**

* `--asset <ASSET_ID>` — `AssetDefinitionId` like `rose#wonderland`
* `--allow-shield` — Allow shielding from public to shielded (default: true)

  Default value: `true`
* `--allow-unshield` — Allow unshielding from shielded to public (default: true)

  Default value: `true`
* `--vk-transfer <BACKEND:NAME>` — Verifying key id for private transfers (format: `<backend>:<name>`, e.g., `halo2/ipa:vk_transfer`)
* `--vk-unshield <BACKEND:NAME>` — Verifying key id for unshield proofs (format: `<backend>:<name>`)
* `--vk-shield <BACKEND:NAME>` — Verifying key id for shield proofs (optional; format: `<backend>:<name>`)



## `iroha_cli app zk shield`

Shield public funds into a shielded ledger (demo flow)

**Usage:** `iroha_cli app zk shield [OPTIONS] --asset <ASSET_ID> --from <ACCOUNT_ID> --amount <AMOUNT> --note-commitment <HEX32>`

###### **Options:**

* `--asset <ASSET_ID>` — `AssetDefinitionId` like `rose#wonderland`
* `--from <ACCOUNT_ID>` — Account identifier to debit (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--amount <AMOUNT>` — Public amount to debit
* `--note-commitment <HEX32>` — Output note commitment (hex, 64 chars)
* `--enc-payload <PATH>` — Encrypted recipient payload envelope (Norito bytes). Optional; empty if not provided
* `--ephemeral-pubkey <HEX32>` — Ephemeral public key for encrypted payload (hex, 64 chars)
* `--nonce-hex <HEX24>` — XChaCha20-Poly1305 nonce for encrypted payload (hex, 48 chars)
* `--ciphertext-b64 <BASE64>` — Ciphertext payload (base64). Includes Poly1305 authentication tag



## `iroha_cli app zk unshield`

Unshield funds from shielded ledger to public (demo flow)

**Usage:** `iroha_cli app zk unshield [OPTIONS] --asset <ASSET_ID> --to <ACCOUNT_ID> --amount <AMOUNT> --inputs <HEX32[,HEX32,...]> --proof-json <PATH>`

###### **Options:**

* `--asset <ASSET_ID>` — `AssetDefinitionId` like `rose#wonderland`
* `--to <ACCOUNT_ID>` — Recipient account identifier to credit (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--amount <AMOUNT>` — Public amount to credit
* `--inputs <HEX32[,HEX32,...]>` — Spent nullifiers (comma-separated list of 64-hex strings)
* `--proof-json <PATH>` — Proof attachment JSON file describing { backend, `proof_b64`, `vk_ref{backend,name}`, `vk_inline{backend,bytes_b64}`, optional `vk_commitment_hex` }
* `--root-hint <HEX32>` — Optional Merkle root hint (hex, 64 chars)



## `iroha_cli app zk vk`

Verifying-key registry lifecycle (register/update/deprecate/get)

**Usage:** `iroha_cli app zk vk <COMMAND>`

###### **Subcommands:**

* `register` — Register a verifying key record (signed transaction via Torii app API)
* `update` — Update an existing verifying key record (version must increase)
* `get` — Get a verifying key record by backend and name



## `iroha_cli app zk vk register`

Register a verifying key record (signed transaction via Torii app API)

**Usage:** `iroha_cli app zk vk register --json <PATH>`

###### **Options:**

* `--json <PATH>` — Path to a JSON DTO file for register (authority, `private_key`, backend, name, version, optional `vk_bytes` (base64) or `commitment_hex`)



## `iroha_cli app zk vk update`

Update an existing verifying key record (version must increase)

**Usage:** `iroha_cli app zk vk update --json <PATH>`

###### **Options:**

* `--json <PATH>` — Path to a JSON DTO file for update (authority, `private_key`, backend, name, version, optional `vk_bytes` or `commitment_hex`)



## `iroha_cli app zk vk get`

Get a verifying key record by backend and name

**Usage:** `iroha_cli app zk vk get --backend <BACKEND> --name <NAME>`

###### **Options:**

* `--backend <BACKEND>` — Backend identifier (e.g., "halo2/ipa")
* `--name <NAME>` — Verifying key name



## `iroha_cli app zk proofs`

Inspect proof registry (list/count/get)

**Usage:** `iroha_cli app zk proofs <COMMAND>`

###### **Subcommands:**

* `list` — List proof records maintained by Torii
* `count` — Count proof records matching the filters
* `get` — Fetch a proof record by backend and proof hash (hex)
* `retention` — Inspect proof retention configuration and live counters
* `prune` — Submit a pruning transaction to enforce proof retention immediately



## `iroha_cli app zk proofs list`

List proof records maintained by Torii

**Usage:** `iroha_cli app zk proofs list [OPTIONS]`

###### **Options:**

* `--backend <BACKEND>` — Filter by backend identifier (e.g., `halo2/ipa`)
* `--status <STATUS>` — Filter by verification status (`Submitted`, `Verified`, `Rejected`)
* `--has-tag <TAG>` — Require a ZK1 TLV tag (4 ASCII characters, e.g., `PROF`)
* `--verified-from-height <HEIGHT>` — Minimum verification height (inclusive)
* `--verified-until-height <HEIGHT>` — Maximum verification height (inclusive)
* `--limit <LIMIT>` — Limit result size (server caps at 1000)
* `--offset <OFFSET>` — Offset for server-side pagination
* `--order <ORDER>` — Sort order (`asc` or `desc`) by verification height
* `--ids-only` — Return only `{ backend, hash }` identifiers



## `iroha_cli app zk proofs count`

Count proof records matching the filters

**Usage:** `iroha_cli app zk proofs count [OPTIONS]`

###### **Options:**

* `--backend <BACKEND>` — Filter by backend identifier (e.g., `halo2/ipa`)
* `--status <STATUS>` — Filter by verification status (`Submitted`, `Verified`, `Rejected`)
* `--has-tag <TAG>` — Require a ZK1 TLV tag (4 ASCII characters, e.g., `PROF`)
* `--verified-from-height <HEIGHT>` — Minimum verification height (inclusive)
* `--verified-until-height <HEIGHT>` — Maximum verification height (inclusive)
* `--limit <LIMIT>` — Limit result size (server caps at 1000)
* `--offset <OFFSET>` — Offset for server-side pagination
* `--order <ORDER>` — Sort order (`asc` or `desc`) by verification height



## `iroha_cli app zk proofs get`

Fetch a proof record by backend and proof hash (hex)

**Usage:** `iroha_cli app zk proofs get --backend <BACKEND> --hash <HASH>`

###### **Options:**

* `--backend <BACKEND>` — Backend identifier (e.g., `halo2/ipa`)
* `--hash <HASH>` — Proof hash (hex, with or without `0x` prefix)



## `iroha_cli app zk proofs retention`

Inspect proof retention configuration and live counters

**Usage:** `iroha_cli app zk proofs retention`



## `iroha_cli app zk proofs prune`

Submit a pruning transaction to enforce proof retention immediately

**Usage:** `iroha_cli app zk proofs prune [OPTIONS]`

###### **Options:**

* `--backend <BACKEND>` — Restrict pruning to a single backend (e.g., `halo2/ipa`). Omit to prune all backends



## `iroha_cli app zk prover`

Inspect background prover reports (list/get/delete)

**Usage:** `iroha_cli app zk prover <COMMAND>`

###### **Subcommands:**

* `reports` — Manage prover reports



## `iroha_cli app zk prover reports`

Manage prover reports

**Usage:** `iroha_cli app zk prover reports <COMMAND>`

###### **Subcommands:**

* `list` — List available prover reports (JSON array)
* `get` — Get a single prover report by id (JSON)
* `delete` — Delete a prover report by id
* `cleanup` — Cleanup reports in bulk (apply filters, delete matches)
* `count` — Count reports matching filters (server-side)



## `iroha_cli app zk prover reports list`

List available prover reports (JSON array)

**Usage:** `iroha_cli app zk prover reports list [OPTIONS]`

###### **Options:**

* `--summary` — Print a one-line summary per report (id, ok, `content_type`, `zk1_tags`)
* `--ok-only` — Show only successful reports
* `--failed-only` — Show only failed reports
* `--errors-only` — Alias for failed-only (errors have ok=false)
* `--id <ID>` — Filter by exact id (hex)
* `--content-type <MIME>` — Filter by content-type substring (e.g., application/x-norito)
* `--has-tag <TAG>` — Filter reports that contain a ZK1 tag (e.g., PROF, IPAK)
* `--ids-only` — Return only ids (server-side projection)
* `--messages-only` — Return only `{ id, error }` objects for failed reports (server-side projection)
* `--fields <CSV>` — Project returned fields (client-side) from full objects, comma-separated (e.g., "`id,ok,content_type,processed_ms`"). Ignored with --summary/--ids-only/--messages-only
* `--limit <N>` — Limit number of reports returned (server-side). Max 1000
* `--since-ms <MS>` — Only reports with `processed_ms` >= this value (server-side)
* `--before-ms <MS>` — Only reports with `processed_ms` <= this value (server-side)
* `--order <ORDER>` — Result ordering: asc (default) or desc

  Default value: `asc`
* `--offset <N>` — Offset after ordering/filtering (server-side)
* `--latest` — Return only the latest report after filters



## `iroha_cli app zk prover reports get`

Get a single prover report by id (JSON)

**Usage:** `iroha_cli app zk prover reports get --id <ID>`

###### **Options:**

* `--id <ID>` — Report id (attachment id)



## `iroha_cli app zk prover reports delete`

Delete a prover report by id

**Usage:** `iroha_cli app zk prover reports delete --id <ID>`

###### **Options:**

* `--id <ID>` — Report id (attachment id)



## `iroha_cli app zk prover reports cleanup`

Cleanup reports in bulk (apply filters, delete matches)

**Usage:** `iroha_cli app zk prover reports cleanup [OPTIONS]`

###### **Options:**

* `--yes` — Proceed without confirmation (dangerous)
* `--ok-only` — Show only successful reports
* `--failed-only` — Show only failed reports
* `--errors-only` — Alias for failed-only (errors have ok=false)
* `--id <ID>` — Filter by exact id (hex)
* `--content-type <MIME>` — Filter by content-type substring (e.g., application/x-norito)
* `--has-tag <TAG>` — Filter reports that contain a ZK1 tag (e.g., PROF, IPAK)
* `--limit <N>` — Limit number of reports returned (server-side). Max 1000
* `--since-ms <MS>` — Only reports with `processed_ms` >= this value (server-side)
* `--before-ms <MS>` — Only reports with `processed_ms` <= this value (server-side)
* `--server` — Use server-side bulk deletion instead of client-side delete loop



## `iroha_cli app zk prover reports count`

Count reports matching filters (server-side)

**Usage:** `iroha_cli app zk prover reports count [OPTIONS]`

###### **Options:**

* `--ok-only` — Show only successful reports
* `--failed-only` — Show only failed reports
* `--errors-only` — Alias for failed-only (errors have ok=false)
* `--id <ID>` — Filter by exact id (hex)
* `--content-type <MIME>` — Filter by content-type substring (e.g., application/x-norito)
* `--has-tag <TAG>` — Filter reports that contain a ZK1 tag (e.g., PROF, IPAK)
* `--since-ms <MS>` — Only reports with `processed_ms` >= this value (server-side)
* `--before-ms <MS>` — Only reports with `processed_ms` <= this value (server-side)



## `iroha_cli app zk vote`

ZK Vote helpers (tally)

**Usage:** `iroha_cli app zk vote <COMMAND>`

###### **Subcommands:**

* `tally` — Get election tally (JSON)



## `iroha_cli app zk vote tally`

Get election tally (JSON)

**Usage:** `iroha_cli app zk vote tally --election-id <ELECTION_ID>`

###### **Options:**

* `--election-id <ELECTION_ID>` — Election identifier



## `iroha_cli app zk envelope`

Encode a confidential encrypted payload (memo) into Norito bytes/base64

**Usage:** `iroha_cli app zk envelope [OPTIONS] --ephemeral-pubkey <HEX32> --nonce-hex <HEX24> --ciphertext-b64 <BASE64>`

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



## `iroha_cli app confidential`

Confidential asset tooling helpers

**Usage:** `iroha_cli app confidential <COMMAND>`

###### **Subcommands:**

* `create-keys` — Derive confidential key hierarchy (nk/ivk/ovk/fvk) from a spend key
* `gas` — Inspect or update the confidential gas schedule



## `iroha_cli app confidential create-keys`

Derive confidential key hierarchy (nk/ivk/ovk/fvk) from a spend key

**Usage:** `iroha_cli app confidential create-keys [OPTIONS]`

###### **Options:**

* `--seed-hex <HEX32>` — 32-byte spend key in hex (if omitted, a random key is generated)
* `--output <PATH>` — Write the derived keyset JSON to a file
* `--quiet` — Do not print the generated JSON to stdout



## `iroha_cli app confidential gas`

Inspect or update the confidential gas schedule

**Usage:** `iroha_cli app confidential gas <COMMAND>`

###### **Subcommands:**

* `get` — Fetch the current confidential gas schedule
* `set` — Update the confidential gas schedule



## `iroha_cli app confidential gas get`

Fetch the current confidential gas schedule

**Usage:** `iroha_cli app confidential gas get`



## `iroha_cli app confidential gas set`

Update the confidential gas schedule

**Usage:** `iroha_cli app confidential gas set --proof-base <UNITS> --per-public-input <UNITS> --per-proof-byte <UNITS> --per-nullifier <UNITS> --per-commitment <UNITS>`

###### **Options:**

* `--proof-base <UNITS>`
* `--per-public-input <UNITS>`
* `--per-proof-byte <UNITS>`
* `--per-nullifier <UNITS>`
* `--per-commitment <UNITS>`



## `iroha_cli app taikai`

Taikai publisher tooling (CAR bundler, envelopes)

**Usage:** `iroha_cli app taikai <COMMAND>`

###### **Subcommands:**

* `bundle` — Bundle a Taikai segment into a CAR archive and Norito envelope
* `cek-rotate` — Emit a CEK rotation receipt for a Taikai stream
* `rpt-attest` — Generate a replication proof token (RPT) attestation
* `ingest` — Taikai ingest helpers (watchers, automation)



## `iroha_cli app taikai bundle`

Bundle a Taikai segment into a CAR archive and Norito envelope

**Usage:** `iroha_cli app taikai bundle [OPTIONS] --payload <PATH> --car-out <PATH> --envelope-out <PATH> --event-id <NAME> --stream-id <NAME> --rendition-id <NAME> --track-kind <TRACK_KIND> --codec <CODEC> --bitrate-kbps <KBPS> --segment-sequence <SEGMENT_SEQUENCE> --segment-start-pts <SEGMENT_START_PTS> --segment-duration <SEGMENT_DURATION> --wallclock-unix-ms <WALLCLOCK_UNIX_MS> --manifest-hash <HEX> --storage-ticket <HEX>`

###### **Options:**

* `--payload <PATH>` — Path to the CMAF fragment or segment payload to ingest
* `--car-out <PATH>` — Where to write the generated `CARv2` archive
* `--envelope-out <PATH>` — Where to write the Norito-encoded Taikai segment envelope
* `--indexes-out <PATH>` — Optional path for a JSON file containing the time/CID index keys
* `--ingest-metadata-out <PATH>` — Optional path for the ingest metadata JSON map consumed by `/v1/da/ingest`
* `--event-id <NAME>` — Identifier of the Taikai event
* `--stream-id <NAME>` — Logical stream identifier within the event
* `--rendition-id <NAME>` — Rendition identifier (ladder rung)
* `--track-kind <TRACK_KIND>` — Track kind carried by the segment

  Possible values: `video`, `audio`, `data`

* `--codec <CODEC>` — Codec identifier (`avc-high`, `hevc-main10`, `av1-main`, `aac-lc`, `opus`, or `custom:<name>`)
* `--bitrate-kbps <KBPS>` — Average bitrate in kilobits per second
* `--resolution <RESOLUTION>` — Video resolution (`WIDTHxHEIGHT`). Required for `video` tracks
* `--audio-layout <AUDIO_LAYOUT>` — Audio layout (`mono`, `stereo`, `5.1`, `7.1`, or `custom:<channels>`). Required for `audio` tracks
* `--segment-sequence <SEGMENT_SEQUENCE>` — Monotonic segment sequence number
* `--segment-start-pts <SEGMENT_START_PTS>` — Presentation timestamp (start) in microseconds since stream origin
* `--segment-duration <SEGMENT_DURATION>` — Presentation duration in microseconds
* `--wallclock-unix-ms <WALLCLOCK_UNIX_MS>` — Wall-clock reference (Unix milliseconds) when the segment was finalised
* `--manifest-hash <HEX>` — Deterministic manifest hash emitted by the ingest pipeline (hex)
* `--storage-ticket <HEX>` — Storage ticket identifier assigned by the orchestrator (hex)
* `--ingest-latency-ms <INGEST_LATENCY_MS>` — Optional encoder-to-ingest latency in milliseconds
* `--live-edge-drift-ms <LIVE_EDGE_DRIFT_MS>` — Optional live-edge drift measurement in milliseconds (negative = stream ahead of ingest)
* `--ingest-node-id <INGEST_NODE_ID>` — Optional identifier for the ingest node that sealed the segment
* `--metadata-json <PATH>` — Optional JSON file describing additional metadata entries



## `iroha_cli app taikai cek-rotate`

Emit a CEK rotation receipt for a Taikai stream

**Usage:** `iroha_cli app taikai cek-rotate [OPTIONS] --event-id <NAME> --stream-id <NAME> --kms-profile <KMS_PROFILE> --new-wrap-key-label <NEW_WRAP_KEY_LABEL> --effective-segment <SEQ> --out <PATH>`

###### **Options:**

* `--event-id <NAME>` — Identifier of the Taikai event
* `--stream-id <NAME>` — Stream identifier within the event
* `--kms-profile <KMS_PROFILE>` — Named KMS profile (e.g., `nitro:prod`)
* `--new-wrap-key-label <NEW_WRAP_KEY_LABEL>` — Label of the new wrap key minted by the KMS
* `--previous-wrap-key-label <PREVIOUS_WRAP_KEY_LABEL>` — Optional label for the previously active wrap key
* `--effective-segment <SEQ>` — Segment sequence where the new CEK becomes active
* `--hkdf-salt <HEX>` — Optional HKDF salt (hex). Generated randomly when omitted
* `--issued-at-unix <ISSUED_AT_UNIX>` — Optional Unix timestamp override for the issued-at field
* `--notes <NOTES>` — Optional operator or governance notes
* `--out <PATH>` — Path to the Norito-encoded receipt output
* `--json-out <PATH>` — Optional JSON summary output path



## `iroha_cli app taikai rpt-attest`

Generate a replication proof token (RPT) attestation

**Usage:** `iroha_cli app taikai rpt-attest [OPTIONS] --event-id <NAME> --stream-id <NAME> --rendition-id <NAME> --gar <PATH> --cek-receipt <PATH> --bundle <PATH> --out <PATH>`

###### **Options:**

* `--event-id <NAME>` — Identifier of the Taikai event
* `--stream-id <NAME>` — Stream identifier within the event
* `--rendition-id <NAME>` — Rendition identifier (ladder rung)
* `--gar <PATH>` — Path to the GAR JWS payload (used for digest computation)
* `--cek-receipt <PATH>` — Path to the CEK rotation receipt referenced by the rollout
* `--bundle <PATH>` — Path to the rollout evidence bundle (directory or single archive)
* `--out <PATH>` — Output path for the Norito-encoded RPT
* `--json-out <PATH>` — Optional JSON summary output path
* `--valid-from-unix <VALID_FROM_UNIX>` — Optional attestation validity start (Unix seconds)
* `--valid-until-unix <VALID_UNTIL_UNIX>` — Optional attestation validity end (Unix seconds)
* `--policy-label <LABEL>` — Optional telemetry labels to embed in the attestation (repeatable)
* `--notes <NOTES>` — Optional governance notes or ticket reference



## `iroha_cli app taikai ingest`

Taikai ingest helpers (watchers, automation)

**Usage:** `iroha_cli app taikai ingest <COMMAND>`

###### **Subcommands:**

* `watch` — Watch a directory for CMAF fragments and bundle them into CAR + Norito artifacts
* `edge` — Prototype edge receiver that emits CMAF fragments and drift logs for the watcher



## `iroha_cli app taikai ingest watch`

Watch a directory for CMAF fragments and bundle them into CAR + Norito artifacts

**Usage:** `iroha_cli app taikai ingest watch [OPTIONS] --source-dir <PATH> --event-id <NAME> --stream-id <NAME> --rendition-id <NAME>`

###### **Options:**

* `--source-dir <PATH>` — Directory that receives CMAF fragments (e.g., `.m4s` files)
* `--output-root <PATH>` — Optional output root; defaults to `./artifacts/taikai/ingest_run_<timestamp>/`
* `--summary-out <PATH>` — Optional NDJSON summary file containing one entry per processed segment
* `--event-id <NAME>` — Identifier of the Taikai event
* `--stream-id <NAME>` — Logical stream identifier within the event
* `--rendition-id <NAME>` — Rendition identifier (ladder rung)
* `--segment-duration <MICROS>` — CMAF segment duration in microseconds (defaults to 2 s)

  Default value: `2000000`
* `--first-segment-pts <MICROS>` — Presentation timestamp (start) in microseconds for the first processed segment

  Default value: `0`
* `--sequence-start <SEQUENCE_START>` — Sequence number to use for the first processed segment

  Default value: `0`
* `--ladder-preset <LADDER_PRESET>` — Optional ladder preset identifier (see `fixtures/taikai/ladder_presets.json`)
* `--ladder-presets <PATH>` — Optional override path for the ladder preset JSON catalog
* `--track-kind <TRACK_KIND>` — Override for the track kind when not using a preset

  Possible values: `video`, `audio`, `data`

* `--codec <CODEC>` — Override for the codec identifier
* `--bitrate-kbps <BITRATE_KBPS>` — Override for the average bitrate in kilobits per second
* `--resolution <RESOLUTION>` — Override for the video resolution (`WIDTHxHEIGHT`)
* `--audio-layout <AUDIO_LAYOUT>` — Override for the audio layout (`mono`, `stereo`, etc.)
* `--ingest-latency-ms <INGEST_LATENCY_MS>` — Optional encoder-to-ingest latency in milliseconds (computed from file timestamps when omitted)
* `--ingest-node-id <INGEST_NODE_ID>` — Optional identifier for the ingest node that sealed the segment
* `--metadata-json <PATH>` — Optional JSON file describing additional metadata entries to attach to each envelope
* `--match-ext <EXT>` — File extensions to watch (repeat the flag to add more)

  Default value: `m4s`
* `--max-segments <COUNT>` — Optional limit on the number of processed segments before exiting
* `--poll-interval-ms <MILLIS>` — Poll interval in milliseconds between directory scans

  Default value: `1000`
* `--drift-warn-ms <MILLIS>` — Drift warning threshold in milliseconds

  Default value: `1500`
* `--da-lane <DA_LANE>` — Lane identifier supplied in DA ingest requests (default: 0 / single-lane)

  Default value: `0`
* `--da-epoch <DA_EPOCH>` — Epoch identifier for DA ingest requests

  Default value: `0`
* `--da-blob-class <DA_BLOB_CLASS>` — Blob-class label (`taikai_segment`, `nexus_lane_sidecar`, `governance_artifact`, `custom:<id>`)

  Default value: `taikai_segment`
* `--da-blob-codec <DA_BLOB_CODEC>` — Codec label recorded in DA ingest requests (default `taikai.cmaf`)

  Default value: `taikai.cmaf`
* `--da-chunk-size <BYTES>` — Chunk size in bytes used for DA ingest requests

  Default value: `262144`
* `--da-data-shards <DA_DATA_SHARDS>` — Number of data shards for the erasure profile (default 10)

  Default value: `10`
* `--da-parity-shards <DA_PARITY_SHARDS>` — Number of parity shards for the erasure profile (default 4)

  Default value: `4`
* `--da-chunk-alignment <DA_CHUNK_ALIGNMENT>` — Chunk alignment (chunks per availability slice)

  Default value: `10`
* `--da-fec-scheme <DA_FEC_SCHEME>` — FEC scheme label (`rs12_10`, `rswin14_10`, `rs18_14`, `custom:<id>`)

  Default value: `rs12_10`
* `--da-hot-retention-secs <DA_HOT_RETENTION_SECS>` — Hot-retention period in seconds

  Default value: `604800`
* `--da-cold-retention-secs <DA_COLD_RETENTION_SECS>` — Cold-retention period in seconds

  Default value: `7776000`
* `--da-required-replicas <DA_REQUIRED_REPLICAS>` — Required replica count for DA retention

  Default value: `3`
* `--da-storage-class <DA_STORAGE_CLASS>` — Storage class label for DA retention (`hot`, `warm`, `cold`)

  Default value: `hot`
* `--da-governance-tag <DA_GOVERNANCE_TAG>` — Governance tag recorded in the retention policy (default `da.taikai.live`)

  Default value: `da.taikai.live`
* `--publish-da` — Toggle automatic publishing to `/v1/da/ingest` using the CLI config
* `--da-endpoint <URL>` — Override the Torii DA ingest endpoint (defaults to `$TORII/v1/da/ingest`)



## `iroha_cli app taikai ingest edge`

Prototype edge receiver that emits CMAF fragments and drift logs for the watcher

**Usage:** `iroha_cli app taikai ingest edge [OPTIONS] --payload <PATH>`

###### **Options:**

* `--payload <PATH>` — Path to a sample fragment payload (treated as CMAF bytes)
* `--output-root <PATH>` — Optional output root; defaults to `./artifacts/taikai/ingest_edge_run_<timestamp>/`
* `--segments <SEGMENTS>` — Number of fragments to emit into the watcher source directory

  Default value: `4`
* `--first-segment-pts <MICROS>` — Presentation timestamp (start) in microseconds for the first emitted segment

  Default value: `0`
* `--segment-interval-ms <MILLIS>` — Interval between segments in milliseconds (controls PTS and wallclock spacing)

  Default value: `2000`
* `--drift-ms <MILLIS>` — Base drift in milliseconds applied to every segment (positive = ingest behind live edge)

  Default value: `0`
* `--drift-jitter-ms <MILLIS>` — Jitter window in milliseconds applied around the base drift

  Default value: `0`
* `--drift-seed <SEED>` — Optional RNG seed for drift jitter so CI runs stay deterministic
* `--start-unix-ms <UNIX_MS>` — Optional Unix timestamp for the first emitted segment; defaults to now
* `--ingest-node-id <INGEST_NODE_ID>` — Optional identifier for the ingest edge node recorded in drift logs
* `--protocol <PROTOCOL>` — Protocol label attached to the emitted fragments

  Default value: `srt`

  Possible values: `srt`, `rtmp`




## `iroha_cli app content`

Content hosting helpers

**Usage:** `iroha_cli app content <COMMAND>`

###### **Subcommands:**

* `publish` — Publish a content bundle (tar archive) to the content lane
* `pack` — Pack a directory into a deterministic tarball + manifest without submitting it



## `iroha_cli app content publish`

Publish a content bundle (tar archive) to the content lane

**Usage:** `iroha_cli app content publish [OPTIONS]`

###### **Options:**

* `--bundle <PATH>` — Path to a tar archive containing the static bundle
* `--root <DIR>` — Directory to pack into a tarball before publishing
* `--expires-at-height <HEIGHT>` — Optional block height when the bundle expires
* `--dataspace <ID>` — Optional dataspace id override for the bundle manifest
* `--lane <ID>` — Optional lane id override for the bundle manifest
* `--auth <MODE>` — Auth mode (`public`, `role:<role_id>`, `sponsor:<uaid>`)
* `--cache-max-age-secs <SECS>` — Cache-Control max-age override (seconds)
* `--immutable` — Mark bundle as immutable (adds `immutable` to Cache-Control)
* `--bundle-out <PATH>` — Optional path to write the packed tarball when using `--root`
* `--manifest-out <PATH>` — Optional path to write the generated manifest JSON



## `iroha_cli app content pack`

Pack a directory into a deterministic tarball + manifest without submitting it

**Usage:** `iroha_cli app content pack [OPTIONS] --root <DIR> --bundle-out <PATH> --manifest-out <PATH>`

###### **Options:**

* `--root <DIR>` — Directory to pack into a tarball
* `--bundle-out <PATH>` — Path to write the tarball
* `--manifest-out <PATH>` — Path to write the generated manifest JSON
* `--dataspace <ID>` — Optional dataspace id override for the bundle manifest
* `--lane <ID>` — Optional lane id override for the bundle manifest
* `--auth <MODE>` — Auth mode (`public`, `role:<role_id>`, `sponsor:<uaid>`)
* `--cache-max-age-secs <SECS>` — Cache-Control max-age override (seconds)
* `--immutable` — Mark bundle as immutable (adds `immutable` to Cache-Control)



## `iroha_cli app da`

Data availability helpers (ingest tooling)

**Usage:** `iroha_cli app da <COMMAND>`

###### **Subcommands:**

* `submit` — Submit a raw blob to `/v1/da/ingest` and capture the signed receipt
* `get` — Fetch blobs via the multi-source orchestrator (thin wrapper over `sorafs fetch`)
* `get-blob` — Download manifest + chunk plan artifacts for an existing DA storage ticket
* `prove` — Generate Proof-of-Retrievability witnesses for a manifest/payload pair
* `prove-availability` — Download + verify availability for a storage ticket using a Torii manifest
* `rent-quote` — Quote rent/incentive breakdown for a blob size/retention combo
* `rent-ledger` — Convert a rent quote into deterministic ledger transfer instructions



## `iroha_cli app da submit`

Submit a raw blob to `/v1/da/ingest` and capture the signed receipt

**Usage:** `iroha_cli app da submit [OPTIONS] --payload <PATH>`

###### **Options:**

* `--payload <PATH>` — Path to the blob payload (CAR, manifest bundle, governance file, etc.)
* `--lane-id <LANE_ID>` — Lane identifier recorded in the DA request

  Default value: `0`
* `--epoch <EPOCH>` — Epoch identifier recorded in the DA request

  Default value: `0`
* `--sequence <SEQUENCE>` — Monotonic sequence scoped to (lane, epoch)

  Default value: `0`
* `--blob-class <BLOB_CLASS>` — Blob-class label (`taikai_segment`, `nexus_lane_sidecar`, `governance_artifact`, `custom:<id>`)

  Default value: `nexus_lane_sidecar`
* `--blob-codec <BLOB_CODEC>` — Codec label describing the payload

  Default value: `custom.binary`
* `--chunk-size <CHUNK_SIZE>` — Chunk size in bytes used for DA chunking

  Default value: `262144`
* `--data-shards <DATA_SHARDS>` — Number of data shards in the erasure profile

  Default value: `10`
* `--parity-shards <PARITY_SHARDS>` — Number of parity shards in the erasure profile

  Default value: `4`
* `--chunk-alignment <CHUNK_ALIGNMENT>` — Chunk alignment (chunks per availability slice)

  Default value: `10`
* `--fec-scheme <FEC_SCHEME>` — FEC scheme label (`rs12_10`, `rswin14_10`, `rs18_14`, `custom:<id>`)

  Default value: `rs12_10`
* `--hot-retention-secs <HOT_RETENTION_SECS>` — Hot retention in seconds

  Default value: `604800`
* `--cold-retention-secs <COLD_RETENTION_SECS>` — Cold retention in seconds

  Default value: `7776000`
* `--required-replicas <REQUIRED_REPLICAS>` — Required replica count enforced by retention policy

  Default value: `3`
* `--storage-class <STORAGE_CLASS>` — Storage-class label (`hot`, `warm`, `cold`)

  Default value: `warm`
* `--governance-tag <GOVERNANCE_TAG>` — Governance tag recorded in the retention policy

  Default value: `da.generic`
* `--metadata-json <PATH>` — Optional metadata JSON file providing string key/value pairs
* `--manifest <PATH>` — Optional pre-generated Norito manifest to embed in the request
* `--endpoint <URL>` — Override for the Torii DA ingest endpoint (defaults to `$TORII/v1/da/ingest`)
* `--client-blob-id <HEX>` — Override the caller-supplied blob identifier (hex). Defaults to BLAKE3(payload)
* `--artifact-dir <PATH>` — Directory for storing Norito/JSON artefacts (defaults to `artifacts/da/submission_<timestamp>`)
* `--no-submit` — Skip HTTP submission and only emit the signed request artefacts



## `iroha_cli app da get`

Fetch blobs via the multi-source orchestrator (thin wrapper over `sorafs fetch`)

**Usage:** `iroha_cli app da get [OPTIONS] --gateway-provider <SPEC>`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded manifest (`.to`) describing the payload layout
* `--plan <PATH>` — Path to the chunk fetch plan JSON (for example, `chunk_fetch_specs` from `iroha sorafs toolkit pack --json-out`)
* `--manifest-id <HEX>` — Hex-encoded manifest hash used as the manifest identifier on gateways
* `--gateway-provider <SPEC>` — Gateway provider descriptor (`name=... , provider-id=... , base-url=... , stream-token=...`)
* `--storage-ticket <HEX>` — Storage ticket identifier to fetch manifest + chunk plan automatically from Torii
* `--manifest-endpoint <URL>` — Optional override for the Torii manifest endpoint used with `--storage-ticket`
* `--manifest-cache-dir <PATH>` — Directory for storing manifest/chunk-plan artefacts fetched via `--storage-ticket`
* `--client-id <STRING>` — Optional client identifier forwarded to the gateway for auditing
* `--manifest-envelope <PATH>` — Optional path to a Norito-encoded manifest envelope to satisfy gateway policy checks
* `--manifest-cid <HEX>` — Override the expected manifest CID (defaults to the manifest digest)
* `--blinded-cid <BASE64>` — Canonical blinded CID (base64url, no padding) forwarded via `SoraNet` headers
* `--salt-epoch <EPOCH>` — Salt epoch corresponding to the blinded CID headers
* `--salt-hex <HEX>` — Hex-encoded 32-byte salt used to derive the canonical blinded CID (computes `--blinded-cid`)
* `--chunker-handle <STRING>` — Override the chunker handle advertised to gateways
* `--max-peers <COUNT>` — Limit the number of providers participating in the session
* `--retry-budget <COUNT>` — Maximum retry attempts per chunk (0 disables the cap)
* `--transport-policy <POLICY>` — Override the default `soranet-first` transport policy (`soranet-first`, `soranet-strict`, or `direct-only`). Supply `direct-only` only when staging a downgrade or rehearsing the compliance drills captured in `roadmap.md`
* `--anonymity-policy <POLICY>` — Override the staged anonymity policy (default `stage-a` / `anon-guard-pq`; accepts `anon-*` or `stage-*` labels)
* `--write-mode <MODE>` — Hint that tightens PQ expectations for write paths (`read-only` or `upload-pq-only`)
* `--transport-policy-override <POLICY>` — Force the orchestrator to stay on a specific transport stage (`soranet-first`, `soranet-strict`, or `direct-only`)
* `--anonymity-policy-override <POLICY>` — Force the orchestrator to stay on a specific anonymity stage (`stage-a`, `anon-guard-pq`, etc.)
* `--guard-cache <PATH>` — Path to the persisted guard cache (Norito-encoded guard set)
* `--guard-cache-key <HEX>` — Optional 32-byte hex key used to tag guard caches when persisting to disk
* `--guard-directory <PATH>` — Path to a guard directory JSON payload used to refresh guard selections
* `--guard-target <COUNT>` — Target number of entry guards to pin (defaults to 3 when the guard directory is provided)
* `--guard-retention-days <DAYS>` — Guard retention window in days (defaults to 30 when the guard directory is provided)
* `--output <PATH>` — Write the assembled payload to a file
* `--json-out <PATH>` — Override the summary JSON path (defaults to `artifacts/sorafs_orchestrator/latest/summary.json`)
* `--scoreboard-out <PATH>` — Override the scoreboard JSON path (defaults to `artifacts/sorafs_orchestrator/latest/scoreboard.json`)
* `--scoreboard-now <UNIX_SECS>` — Override the Unix timestamp used when evaluating provider adverts
* `--telemetry-source-label <LABEL>` — Label describing the telemetry stream captured alongside the scoreboard (persisted in metadata)
* `--telemetry-region <LABEL>` — Optional telemetry region label persisted in both the scoreboard metadata and summary JSON



## `iroha_cli app da get-blob`

Download manifest + chunk plan artifacts for an existing DA storage ticket

**Usage:** `iroha_cli app da get-blob [OPTIONS] --storage-ticket <HEX>`

###### **Options:**

* `--storage-ticket <HEX>` — Storage ticket identifier (hex string) issued by Torii
* `--block-hash <HEX>` — Optional block hash used to seed deterministic sampling in the manifest response
* `--endpoint <URL>` — Optional override for the Torii manifest endpoint (defaults to `$TORII/v1/da/manifests/`)
* `--output-dir <PATH>` — Directory for storing the fetched manifest + chunk plan artefacts



## `iroha_cli app da prove`

Generate Proof-of-Retrievability witnesses for a manifest/payload pair

**Usage:** `iroha_cli app da prove [OPTIONS] --manifest <PATH> --payload <PATH>`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded manifest describing the chunk layout
* `--payload <PATH>` — Path to the assembled payload bytes that match the manifest
* `--json-out <PATH>` — Optional JSON output path; defaults to stdout only
* `--sample-count <SAMPLE_COUNT>` — Number of random leaves to sample for `PoR` proofs (0 disables sampling)

  Default value: `8`
* `--sample-seed <SAMPLE_SEED>` — Seed used for deterministic `PoR` sampling

  Default value: `0`
* `--block-hash <HEX>` — Optional block hash used to derive deterministic sampling (overrides sample-count/seed)
* `--leaf-index <INDEX>` — Explicit `PoR` leaf indexes to prove (0-based flattened index)



## `iroha_cli app da prove-availability`

Download + verify availability for a storage ticket using a Torii manifest

**Usage:** `iroha_cli app da prove-availability [OPTIONS] --storage-ticket <HEX> --gateway-provider <SPEC>`

###### **Options:**

* `--storage-ticket <HEX>` — Storage ticket issued by Torii (hex string)
* `--gateway-provider <SPEC>` — Gateway provider descriptor reused by `sorafs fetch` (name=... , provider-id=... , base-url=... , stream-token=...)
* `--manifest-endpoint <URL>` — Optional override for Torii manifest endpoint
* `--manifest-cache-dir <PATH>` — Directory where manifests and plans downloaded from Torii are cached (defaults to `artifacts/da/fetch_<ts>`)
* `--json-out <PATH>` — JSON output path for the combined proof summary (defaults to stdout)
* `--scoreboard-out <PATH>` — Path to persist the orchestrator scoreboard (defaults to temp dir if omitted)
* `--max-peers <COUNT>` — Optional limit on concurrent provider downloads
* `--sample-count <SAMPLE_COUNT>` — Proof sampling count for `PoR` verification (defaults to 8, set 0 to disable random sampling)

  Default value: `8`
* `--sample-seed <SAMPLE_SEED>` — Seed used for deterministic `PoR` sampling during verification

  Default value: `0`
* `--block-hash <HEX>` — Optional block hash used to derive deterministic sampling (overrides sample-count/seed)
* `--leaf-index <INDEX>` — Explicit `PoR` leaf indexes to verify in addition to sampled values
* `--artifact-dir <PATH>` — Directory for storing assembled payload/artefacts (defaults to `artifacts/da/prove_availability_<ts>`)



## `iroha_cli app da rent-quote`

Quote rent/incentive breakdown for a blob size/retention combo

**Usage:** `iroha_cli app da rent-quote [OPTIONS] --gib <GIB> --months <MONTHS>`

###### **Options:**

* `--gib <GIB>` — Logical GiB stored in the blob (post-chunking)
* `--months <MONTHS>` — Retention duration measured in months
* `--policy-json <PATH>` — Optional path to a JSON-encoded `DaRentPolicyV1`
* `--policy-norito <PATH>` — Optional path to a Norito-encoded `DaRentPolicyV1`
* `--policy-label <TEXT>` — Optional human-readable label recorded in the quote metadata (defaults to source path)
* `--quote-out <PATH>` — Optional path for persisting the rendered quote JSON



## `iroha_cli app da rent-ledger`

Convert a rent quote into deterministic ledger transfer instructions

**Usage:** `iroha_cli app da rent-ledger --quote <PATH> --payer-account <ACCOUNT_ID> --treasury-account <ACCOUNT_ID> --protocol-reserve-account <ACCOUNT_ID> --provider-account <ACCOUNT_ID> --pdp-bonus-account <ACCOUNT_ID> --potr-bonus-account <ACCOUNT_ID> --asset-definition <NAME#DOMAIN>`

###### **Options:**

* `--quote <PATH>` — Path to the rent quote JSON file (output of `iroha da rent-quote`)
* `--payer-account <ACCOUNT_ID>` — Account responsible for paying the rent and funding bonus pools
* `--treasury-account <ACCOUNT_ID>` — Treasury or escrow account receiving the base rent before distribution
* `--protocol-reserve-account <ACCOUNT_ID>` — Protocol reserve account that receives the configured reserve share
* `--provider-account <ACCOUNT_ID>` — Provider payout account that receives the base rent remainder
* `--pdp-bonus-account <ACCOUNT_ID>` — Account earmarked for PDP bonus payouts
* `--potr-bonus-account <ACCOUNT_ID>` — Account earmarked for `PoTR` bonus payouts
* `--asset-definition <NAME#DOMAIN>` — Asset definition identifier used for XOR transfers (e.g., `xor#sora`)



## `iroha_cli app streaming`

Streaming helpers (HPKE fingerprints, suite listings)

**Usage:** `iroha_cli app streaming <COMMAND>`

###### **Subcommands:**

* `fingerprint` — Compute the ML-KEM fingerprint advertised in `EncryptionSuite::Kyber*`
* `suites` — List supported ML-KEM suite identifiers



## `iroha_cli app streaming fingerprint`

Compute the ML-KEM fingerprint advertised in `EncryptionSuite::Kyber*`

**Usage:** `iroha_cli app streaming fingerprint [OPTIONS] --public-key <HEX>`

###### **Options:**

* `--suite <NAME>` — ML-KEM suite to use (e.g., `mlkem512`, `mlkem768`, `mlkem1024`)
* `--public-key <HEX>` — Hex-encoded ML-KEM public key



## `iroha_cli app streaming suites`

List supported ML-KEM suite identifiers

**Usage:** `iroha_cli app streaming suites`



## `iroha_cli app nexus`

Nexus helpers (lanes, governance)

**Usage:** `iroha_cli app nexus <COMMAND>`

###### **Subcommands:**

* `lane-report` — Show governance manifest status per lane
* `public-lane` — Inspect public-lane validator lifecycle and stake state



## `iroha_cli app nexus lane-report`

Show governance manifest status per lane

**Usage:** `iroha_cli app nexus lane-report [OPTIONS]`

###### **Options:**

* `--summary` — Print a compact table instead of JSON

  Default value: `false`
* `--only-missing` — Show only lanes that require a manifest but remain sealed

  Default value: `false`
* `--fail-on-sealed` — Exit with non-zero status if any manifest is missing

  Default value: `false`



## `iroha_cli app nexus public-lane`

Inspect public-lane validator lifecycle and stake state

**Usage:** `iroha_cli app nexus public-lane <COMMAND>`

###### **Subcommands:**

* `validators` — List validators for a public lane with lifecycle hints
* `stake` — List bonded stake and pending unbonds for a public lane



## `iroha_cli app nexus public-lane validators`

List validators for a public lane with lifecycle hints

**Usage:** `iroha_cli app nexus public-lane validators [OPTIONS]`

###### **Options:**

* `--lane <LANE>` — Public lane identifier (defaults to SINGLE lane)

  Default value: `0`
* `--summary` — Render a compact table instead of raw JSON

  Default value: `false`
* `--address-format <ADDRESS_FORMAT>` — Preferred address literal encoding (`ih58` or `compressed`)

  Default value: `ih58`

  Possible values: `ih58`, `compressed`




## `iroha_cli app nexus public-lane stake`

List bonded stake and pending unbonds for a public lane

**Usage:** `iroha_cli app nexus public-lane stake [OPTIONS]`

###### **Options:**

* `--lane <LANE>` — Public lane identifier (defaults to SINGLE lane)

  Default value: `0`
* `--validator <ACCOUNT_ID>` — Filter for a specific validator account (optional)
* `--summary` — Render a compact table instead of raw JSON

  Default value: `false`
* `--address-format <ADDRESS_FORMAT>` — Preferred address literal encoding (`ih58` or `compressed`)

  Default value: `ih58`

  Possible values: `ih58`, `compressed`




## `iroha_cli app staking`

Public-lane staking helpers (register/activate/exit)

**Usage:** `iroha_cli app staking <COMMAND>`

###### **Subcommands:**

* `register` — Register a stake-elected validator on a public lane
* `activate` — Activate a pending validator once its activation epoch is reached
* `exit` — Schedule or finalize a validator exit



## `iroha_cli app staking register`

Register a stake-elected validator on a public lane

**Usage:** `iroha_cli app staking register [OPTIONS] --lane-id <LANE_ID> --validator <ACCOUNT_ID> --initial-stake <AMOUNT>`

###### **Options:**

* `--lane-id <LANE_ID>` — Lane id to register against
* `--validator <ACCOUNT_ID>` — Validator account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--stake-account <ACCOUNT_ID>` — Optional staking account (defaults to validator)
* `--initial-stake <AMOUNT>` — Initial self-bond (integer, uses the staking asset scale)
* `--metadata <PATH>` — Optional metadata JSON (Norito JSON object)



## `iroha_cli app staking activate`

Activate a pending validator once its activation epoch is reached

**Usage:** `iroha_cli app staking activate --lane-id <LANE_ID> --validator <ACCOUNT_ID>`

###### **Options:**

* `--lane-id <LANE_ID>` — Lane id containing the pending validator
* `--validator <ACCOUNT_ID>` — Validator account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)



## `iroha_cli app staking exit`

Schedule or finalize a validator exit

**Usage:** `iroha_cli app staking exit --lane-id <LANE_ID> --validator <ACCOUNT_ID> --release-at-ms <MILLIS>`

###### **Options:**

* `--lane-id <LANE_ID>` — Lane id containing the validator
* `--validator <ACCOUNT_ID>` — Validator account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--release-at-ms <MILLIS>` — Release timestamp in milliseconds (must not precede current block timestamp)



## `iroha_cli app subscriptions`

Subscription plan and billing helpers

**Usage:** `iroha_cli app subscriptions <COMMAND>`

###### **Subcommands:**

* `plan` — Manage subscription plans (asset definition metadata)
* `subscription` — Manage subscriptions and billing actions



## `iroha_cli app subscriptions plan`

Manage subscription plans (asset definition metadata)

**Usage:** `iroha_cli app subscriptions plan <COMMAND>`

###### **Subcommands:**

* `create` — Register a subscription plan on an asset definition
* `list` — List subscription plans, optionally filtered by provider



## `iroha_cli app subscriptions plan create`

Register a subscription plan on an asset definition

**Usage:** `iroha_cli app subscriptions plan create [OPTIONS] --authority <ACCOUNT_ID> --private-key <HEX> --plan-id <ASSET_DEF_ID>`

###### **Options:**

* `--authority <ACCOUNT_ID>` — Authority account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--plan-id <ASSET_DEF_ID>` — Asset definition id where the plan metadata is stored
* `--plan-json <PATH>` — Path to JSON plan payload (reads stdin when omitted)



## `iroha_cli app subscriptions plan list`

List subscription plans, optionally filtered by provider

**Usage:** `iroha_cli app subscriptions plan list [OPTIONS]`

###### **Options:**

* `--provider <ACCOUNT_ID>` — Filter by plan provider (account id)
* `--limit <LIMIT>` — Limit number of results
* `--offset <OFFSET>` — Offset for pagination (default 0)

  Default value: `0`



## `iroha_cli app subscriptions subscription`

Manage subscriptions and billing actions

**Usage:** `iroha_cli app subscriptions subscription <COMMAND>`

###### **Subcommands:**

* `create` — Create a subscription and billing trigger
* `list` — List subscriptions with optional filters
* `get` — Fetch a subscription by id
* `pause` — Pause billing for a subscription
* `resume` — Resume billing for a subscription
* `cancel` — Cancel a subscription and remove its billing trigger
* `keep` — Undo a scheduled period-end cancellation
* `charge-now` — Execute billing immediately
* `usage` — Record usage for a subscription usage plan



## `iroha_cli app subscriptions subscription create`

Create a subscription and billing trigger

**Usage:** `iroha_cli app subscriptions subscription create [OPTIONS] --authority <ACCOUNT_ID> --private-key <HEX> --subscription-id <NFT_ID> --plan-id <ASSET_DEF_ID>`

###### **Options:**

* `--authority <ACCOUNT_ID>` — Authority account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--subscription-id <NFT_ID>` — Subscription NFT id to register
* `--plan-id <ASSET_DEF_ID>` — Subscription plan asset definition id
* `--billing-trigger-id <BILLING_TRIGGER_ID>` — Optional billing trigger id to use
* `--usage-trigger-id <USAGE_TRIGGER_ID>` — Optional usage trigger id to use (usage plans only)
* `--first-charge-ms <FIRST_CHARGE_MS>` — Optional first charge timestamp in UTC milliseconds
* `--grant-usage-to-provider <GRANT_USAGE_TO_PROVIDER>` — Grant usage reporting permission to the plan provider

  Possible values: `true`, `false`




## `iroha_cli app subscriptions subscription list`

List subscriptions with optional filters

**Usage:** `iroha_cli app subscriptions subscription list [OPTIONS]`

###### **Options:**

* `--owned-by <ACCOUNT_ID>` — Filter by subscriber account
* `--provider <ACCOUNT_ID>` — Filter by plan provider account
* `--status <STATUS>` — Filter by status (active, paused, `past_due`, canceled, suspended)
* `--limit <LIMIT>` — Limit number of results
* `--offset <OFFSET>` — Offset for pagination (default 0)

  Default value: `0`



## `iroha_cli app subscriptions subscription get`

Fetch a subscription by id

**Usage:** `iroha_cli app subscriptions subscription get --subscription-id <NFT_ID>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id



## `iroha_cli app subscriptions subscription pause`

Pause billing for a subscription

**Usage:** `iroha_cli app subscriptions subscription pause [OPTIONS] --subscription-id <NFT_ID> --authority <ACCOUNT_ID> --private-key <HEX>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id
* `--authority <ACCOUNT_ID>` — Authority account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--charge-at-ms <CHARGE_AT_MS>` — Optional charge time override in UTC milliseconds
* `--cancel-at-period-end` — Cancel at the end of the current billing period (cancel only)



## `iroha_cli app subscriptions subscription resume`

Resume billing for a subscription

**Usage:** `iroha_cli app subscriptions subscription resume [OPTIONS] --subscription-id <NFT_ID> --authority <ACCOUNT_ID> --private-key <HEX>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id
* `--authority <ACCOUNT_ID>` — Authority account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--charge-at-ms <CHARGE_AT_MS>` — Optional charge time override in UTC milliseconds
* `--cancel-at-period-end` — Cancel at the end of the current billing period (cancel only)



## `iroha_cli app subscriptions subscription cancel`

Cancel a subscription and remove its billing trigger

**Usage:** `iroha_cli app subscriptions subscription cancel [OPTIONS] --subscription-id <NFT_ID> --authority <ACCOUNT_ID> --private-key <HEX>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id
* `--authority <ACCOUNT_ID>` — Authority account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--charge-at-ms <CHARGE_AT_MS>` — Optional charge time override in UTC milliseconds
* `--cancel-at-period-end` — Cancel at the end of the current billing period (cancel only)



## `iroha_cli app subscriptions subscription keep`

Undo a scheduled period-end cancellation

**Usage:** `iroha_cli app subscriptions subscription keep [OPTIONS] --subscription-id <NFT_ID> --authority <ACCOUNT_ID> --private-key <HEX>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id
* `--authority <ACCOUNT_ID>` — Authority account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--charge-at-ms <CHARGE_AT_MS>` — Optional charge time override in UTC milliseconds
* `--cancel-at-period-end` — Cancel at the end of the current billing period (cancel only)



## `iroha_cli app subscriptions subscription charge-now`

Execute billing immediately

**Usage:** `iroha_cli app subscriptions subscription charge-now [OPTIONS] --subscription-id <NFT_ID> --authority <ACCOUNT_ID> --private-key <HEX>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id
* `--authority <ACCOUNT_ID>` — Authority account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--charge-at-ms <CHARGE_AT_MS>` — Optional charge time override in UTC milliseconds
* `--cancel-at-period-end` — Cancel at the end of the current billing period (cancel only)



## `iroha_cli app subscriptions subscription usage`

Record usage for a subscription usage plan

**Usage:** `iroha_cli app subscriptions subscription usage [OPTIONS] --subscription-id <NFT_ID> --authority <ACCOUNT_ID> --private-key <HEX> --unit-key <UNIT_KEY> --delta <DELTA>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id
* `--authority <ACCOUNT_ID>` — Authority account identifier (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--unit-key <UNIT_KEY>` — Usage counter key to update
* `--delta <DELTA>` — Usage increment (must be non-negative)
* `--usage-trigger-id <USAGE_TRIGGER_ID>` — Optional usage trigger id override



## `iroha_cli app endorsement`

Domain endorsement helpers (committees, policies, submissions)

**Usage:** `iroha_cli app endorsement <COMMAND>`

###### **Subcommands:**

* `prepare` — Build a domain endorsement (optionally signing it) and emit JSON to stdout
* `submit` — Submit a domain endorsement into the chain state for later reuse
* `list` — List recorded endorsements for a domain
* `policy` — Fetch the endorsement policy for a domain
* `committee` — Fetch a registered endorsement committee
* `register-committee` — Register an endorsement committee (quorum + members)
* `set-policy` — Set or replace the endorsement policy for a domain



## `iroha_cli app endorsement prepare`

Build a domain endorsement (optionally signing it) and emit JSON to stdout

**Usage:** `iroha_cli app endorsement prepare [OPTIONS] --domain <DOMAIN> --issued-at-height <HEIGHT> --expires-at-height <HEIGHT>`

###### **Options:**

* `--domain <DOMAIN>` — Domain identifier being endorsed
* `--committee-id <COMMITTEE_ID>` — Committee identifier backing this endorsement

  Default value: `default`
* `--issued-at-height <HEIGHT>` — Block height when the endorsement was issued
* `--expires-at-height <HEIGHT>` — Block height when the endorsement expires
* `--block-start <BLOCK_START>` — Optional block height (inclusive) when the endorsement becomes valid
* `--block-end <BLOCK_END>` — Optional block height (inclusive) after which the endorsement is invalid
* `--dataspace <DATASPACE>` — Optional dataspace binding for the endorsement
* `--metadata <PATH>` — Optional metadata payload (Norito JSON file) to embed
* `--signer-key <PRIVATE_KEY>` — Private keys to sign the endorsement body (multiple allowed)



## `iroha_cli app endorsement submit`

Submit a domain endorsement into the chain state for later reuse

**Usage:** `iroha_cli app endorsement submit [OPTIONS]`

###### **Options:**

* `--file <PATH>` — Path to the endorsement JSON. If omitted, read from stdin



## `iroha_cli app endorsement list`

List recorded endorsements for a domain

**Usage:** `iroha_cli app endorsement list --domain <DOMAIN>`

###### **Options:**

* `--domain <DOMAIN>` — Domain to query



## `iroha_cli app endorsement policy`

Fetch the endorsement policy for a domain

**Usage:** `iroha_cli app endorsement policy --domain <DOMAIN>`

###### **Options:**

* `--domain <DOMAIN>` — Domain to query



## `iroha_cli app endorsement committee`

Fetch a registered endorsement committee

**Usage:** `iroha_cli app endorsement committee --committee-id <COMMITTEE_ID>`

###### **Options:**

* `--committee-id <COMMITTEE_ID>` — Committee identifier to fetch



## `iroha_cli app endorsement register-committee`

Register an endorsement committee (quorum + members)

**Usage:** `iroha_cli app endorsement register-committee [OPTIONS] --committee-id <COMMITTEE_ID> --quorum <QUORUM> --member <PUBLIC_KEY>`

###### **Options:**

* `--committee-id <COMMITTEE_ID>` — New committee identifier
* `--quorum <QUORUM>` — Quorum required to accept an endorsement
* `--member <PUBLIC_KEY>` — Member public keys allowed to sign endorsements (string form)
* `--metadata <PATH>` — Optional metadata payload (Norito JSON file) to attach



## `iroha_cli app endorsement set-policy`

Set or replace the endorsement policy for a domain

**Usage:** `iroha_cli app endorsement set-policy [OPTIONS] --domain <DOMAIN> --committee-id <COMMITTEE_ID> --max-endorsement-age <BLOCKS>`

###### **Options:**

* `--domain <DOMAIN>` — Domain requiring endorsements
* `--committee-id <COMMITTEE_ID>` — Committee identifier to trust
* `--max-endorsement-age <BLOCKS>` — Maximum age (in blocks) allowed between issuance and acceptance
* `--required` — Whether an endorsement is required for the domain

  Default value: `true`



## `iroha_cli app jurisdiction`

Jurisdiction Data Guardian helpers (attestations and SDN registries)

**Usage:** `iroha_cli app jurisdiction <COMMAND>`

###### **Subcommands:**

* `verify` — Validate a JDG attestation (structural + SDN commitments)



## `iroha_cli app jurisdiction verify`

Validate a JDG attestation (structural + SDN commitments)

**Usage:** `iroha_cli app jurisdiction verify [OPTIONS]`

###### **Options:**

* `--attestation <PATH>` — Path to the JDG attestation payload (Norito JSON or binary). Reads stdin when omitted
* `--sdn-registry <PATH>` — Optional SDN registry payload (Norito JSON or binary)
* `--require-sdn-commitments` — Whether SDN commitments are mandatory for this attestation

  Default value: `false`
* `--dual-publish-blocks <DUAL_PUBLISH_BLOCKS>` — Number of blocks the previous SDN key remains valid after rotation

  Default value: `0`
* `--current-height <HEIGHT>` — Current block height for expiry/block-window checks
* `--expect-dataspace <ID>` — Expected dataspace id; validation fails if it does not match



## `iroha_cli app compute`

Compute lane simulation helpers

**Usage:** `iroha_cli app compute <COMMAND>`

###### **Subcommands:**

* `simulate` — Simulate a compute call offline and emit the receipt/response
* `invoke` — Invoke a running compute gateway using the shared fixtures



## `iroha_cli app compute simulate`

Simulate a compute call offline and emit the receipt/response

**Usage:** `iroha_cli app compute simulate [OPTIONS]`

###### **Options:**

* `--manifest <PATH>` — Path to the compute manifest to validate against

  Default value: `fixtures/compute/manifest_compute_payments.json`
* `--call <PATH>` — Path to the canonical compute call fixture

  Default value: `fixtures/compute/call_compute_payments.json`
* `--payload <PATH>` — Path to the payload to send (ignored when --payload-inline is supplied)

  Default value: `fixtures/compute/payload_compute_payments.json`
* `--payload-inline <BYTES>` — Inline payload bytes (UTF-8) (mutually exclusive with --payload)
* `--json-out <PATH>` — Optional JSON output path (stdout when omitted)



## `iroha_cli app compute invoke`

Invoke a running compute gateway using the shared fixtures

**Usage:** `iroha_cli app compute invoke [OPTIONS]`

###### **Options:**

* `--endpoint <URL>` — Base endpoint for the compute gateway (without the route path)

  Default value: `http://127.0.0.1:8088`
* `--manifest <PATH>` — Path to the compute manifest used for validation

  Default value: `fixtures/compute/manifest_compute_payments.json`
* `--call <PATH>` — Path to the compute call fixture

  Default value: `fixtures/compute/call_compute_payments.json`
* `--payload <PATH>` — Path to the payload to send with the call

  Default value: `fixtures/compute/payload_compute_payments.json`



## `iroha_cli app social`

Social incentive helpers (viral follow rewards and escrows)

**Usage:** `iroha_cli app social <COMMAND>`

###### **Subcommands:**

* `claim-twitter-follow-reward` — Claim a promotional reward for a verified Twitter follow binding
* `send-to-twitter` — Send funds to a Twitter handle; funds are escrowed until a follow binding appears
* `cancel-twitter-escrow` — Cancel an existing escrow created by `send-to-twitter`



## `iroha_cli app social claim-twitter-follow-reward`

Claim a promotional reward for a verified Twitter follow binding

**Usage:** `iroha_cli app social claim-twitter-follow-reward --binding-hash-json <PATH>`

###### **Options:**

* `--binding-hash-json <PATH>` — Path to a JSON file containing a `KeyedHash` (binding hash) payload.

   The JSON shape must match `iroha_data_model::oracle::KeyedHash`.



## `iroha_cli app social send-to-twitter`

Send funds to a Twitter handle; funds are escrowed until a follow binding appears

**Usage:** `iroha_cli app social send-to-twitter --binding-hash-json <PATH> --amount <AMOUNT>`

###### **Options:**

* `--binding-hash-json <PATH>` — Path to a JSON file containing a `KeyedHash` (binding hash) payload.

   The JSON shape must match `iroha_data_model::oracle::KeyedHash`.
* `--amount <AMOUNT>` — Amount to escrow or deliver immediately when the binding is already active.

   Parsed as `Numeric` (mantissa/scale) using the standard string format.



## `iroha_cli app social cancel-twitter-escrow`

Cancel an existing escrow created by `send-to-twitter`

**Usage:** `iroha_cli app social cancel-twitter-escrow --binding-hash-json <PATH>`

###### **Options:**

* `--binding-hash-json <PATH>` — Path to a JSON file containing a `KeyedHash` (binding hash) payload.

   The JSON shape must match `iroha_data_model::oracle::KeyedHash`.



## `iroha_cli app space-directory`

Space Directory helpers (UAID capability manifests)

**Usage:** `iroha_cli app space-directory <COMMAND>`

###### **Subcommands:**

* `manifest` — Manage UAID capability manifests
* `bindings` — Inspect UAID bindings surfaced by Torii



## `iroha_cli app space-directory manifest`

Manage UAID capability manifests

**Usage:** `iroha_cli app space-directory manifest <COMMAND>`

###### **Subcommands:**

* `publish` — Publish or replace a capability manifest (.to payload)
* `encode` — Encode manifest JSON into Norito bytes and record its hash
* `revoke` — Revoke a manifest for a UAID/dataspace pair
* `expire` — Expire a manifest that reached its scheduled end-of-life
* `audit-bundle` — Produce an audit bundle for an existing capability manifest + dataspace profile
* `fetch` — Fetch manifests for a UAID via Torii
* `scaffold` — Scaffold manifest/profile templates for a UAID + dataspace pair



## `iroha_cli app space-directory manifest publish`

Publish or replace a capability manifest (.to payload)

**Usage:** `iroha_cli app space-directory manifest publish [OPTIONS]`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded `AssetPermissionManifest` (.to)
* `--manifest-json <PATH>` — Path to the JSON `AssetPermissionManifest` (encoded on submit)
* `--reason <TEXT>` — Optional CLI-level reason used when publishing a new manifest (added to metadata)



## `iroha_cli app space-directory manifest encode`

Encode manifest JSON into Norito bytes and record its hash

**Usage:** `iroha_cli app space-directory manifest encode [OPTIONS] --json <PATH>`

###### **Options:**

* `--json <PATH>` — Path to the JSON `AssetPermissionManifest`
* `--out <PATH>` — Target path for the Norito `.to` payload (defaults to `<json>.manifest.to`)
* `--hash-out <PATH>` — Optional file for the manifest hash (defaults to `<out>.hash`)



## `iroha_cli app space-directory manifest revoke`

Revoke a manifest for a UAID/dataspace pair

**Usage:** `iroha_cli app space-directory manifest revoke [OPTIONS] --uaid <UAID> --dataspace <ID> --revoked-epoch <EPOCH>`

###### **Options:**

* `--uaid <UAID>` — UAID whose manifest should be revoked
* `--dataspace <ID>` — Dataspace identifier hosting the manifest
* `--revoked-epoch <EPOCH>` — Epoch (inclusive) when the revocation takes effect
* `--reason <TEXT>` — Optional reason recorded with the revocation



## `iroha_cli app space-directory manifest expire`

Expire a manifest that reached its scheduled end-of-life

**Usage:** `iroha_cli app space-directory manifest expire --uaid <UAID> --dataspace <ID> --expired-epoch <EPOCH>`

###### **Options:**

* `--uaid <UAID>` — UAID whose manifest should be expired
* `--dataspace <ID>` — Dataspace identifier hosting the manifest
* `--expired-epoch <EPOCH>` — Epoch (inclusive) when the expiry occurred



## `iroha_cli app space-directory manifest audit-bundle`

Produce an audit bundle for an existing capability manifest + dataspace profile

**Usage:** `iroha_cli app space-directory manifest audit-bundle [OPTIONS] --profile <PATH> --out-dir <DIR>`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded `AssetPermissionManifest` (.to)
* `--manifest-json <PATH>` — Path to the JSON `AssetPermissionManifest` (encoded on export)
* `--profile <PATH>` — Dataspace profile JSON used to capture governance/audit hooks
* `--out-dir <DIR>` — Directory where the bundle (manifest/profile/hash/audit metadata) will be written
* `--notes <TEXT>` — Optional operator note recorded inside the bundle metadata



## `iroha_cli app space-directory manifest fetch`

Fetch manifests for a UAID via Torii

**Usage:** `iroha_cli app space-directory manifest fetch [OPTIONS] --uaid <UAID>`

###### **Options:**

* `--uaid <UAID>` — UAID literal whose manifests should be fetched
* `--dataspace <ID>` — Optional dataspace id filter
* `--status <STATUS>` — Manifest lifecycle status filter (active, inactive, all)

  Default value: `all`

  Possible values: `active`, `inactive`, `all`

* `--limit <N>` — Maximum number of manifests to return
* `--offset <N>` — Offset for pagination
* `--address-format <ADDRESS_FORMAT>` — Preferred account literal encoding (`ih58` or `compressed`)

  Default value: `ih58`

  Possible values: `ih58`, `compressed`

* `--json-out <PATH>` — Optional path where the JSON response will be stored



## `iroha_cli app space-directory manifest scaffold`

Scaffold manifest/profile templates for a UAID + dataspace pair

**Usage:** `iroha_cli app space-directory manifest scaffold [OPTIONS] --uaid <UAID> --dataspace <ID> --activation-epoch <EPOCH>`

###### **Options:**

* `--uaid <UAID>` — Universal account identifier (`uaid:<hex>` or raw 64-hex digest, LSB=1)
* `--dataspace <ID>` — Dataspace identifier the manifest targets
* `--activation-epoch <EPOCH>` — Activation epoch recorded in the manifest
* `--expiry-epoch <EPOCH>` — Optional expiry epoch recorded in the manifest
* `--issued-ms <MS>` — Override the issued timestamp (milliseconds since UNIX epoch)
* `--notes <TEXT>` — Optional notes propagated to scaffolded entries
* `--manifest-out <PATH>` — Output path for the manifest JSON (defaults to `artifacts/space_directory/scaffold/<timestamp>/manifest.json`)
* `--profile-out <PATH>` — Optional output path for the dataspace profile skeleton (defaults beside the manifest)
* `--allow-dataspace <ID>` — Optional dataspace override for the allow entry scope
* `--allow-program <PROGRAM>` — Program identifier (`contract.name`) for the allow entry
* `--allow-method <NAME>` — Method/entry-point for the allow entry
* `--allow-asset <DEF#DOMAIN>` — Asset identifier (e.g. `xor#sora`) for the allow entry
* `--allow-role <ROLE>` — AMX role enforced by the allow entry (`initiator` or `participant`)
* `--allow-max-amount <DECIMAL>` — Deterministic allowance cap (decimal string)
* `--allow-window <WINDOW>` — Allowance window (`per-slot`, `per-minute`, or `per-day`)
* `--allow-notes <TEXT>` — Optional operator note stored alongside the entry
* `--deny-dataspace <ID>` — Optional dataspace override for the deny entry scope
* `--deny-program <PROGRAM>` — Program identifier (`contract.name`) for the deny entry
* `--deny-method <NAME>` — Method/entry-point for the deny entry
* `--deny-asset <DEF#DOMAIN>` — Asset identifier (e.g. `xor#sora`) for the deny entry
* `--deny-role <ROLE>` — AMX role enforced by the deny entry
* `--deny-reason <TEXT>` — Optional reason recorded for the deny directive
* `--deny-notes <TEXT>` — Optional operator note stored alongside the entry
* `--profile-id <ID>` — Dataspace profile identifier (default `profile.<dataspace>.v1`)
* `--profile-activation-epoch <EPOCH>` — Epoch recorded in the profile metadata
* `--profile-governance-issuer <ACCOUNT_ID>` — Dataspace governance issuer account
* `--profile-governance-ticket <TEXT>` — Governance ticket/evidence label
* `--profile-governance-quorum <N>` — Governance quorum threshold
* `--profile-validator <ACCOUNT_ID>` — Validator account identifiers
* `--profile-validator-quorum <N>` — Validator quorum threshold
* `--profile-protected-namespace <NAME>` — Protected namespace entries
* `--profile-da-class <TEXT>` — DA class label (default `A`)
* `--profile-da-quorum <N>` — DA attester quorum
* `--profile-da-attester <ACCOUNT_ID>` — DA attester identifiers
* `--profile-da-rotation-epochs <EPOCHS>` — DA rotation cadence in epochs
* `--profile-composability-group <HEX>` — Composability group identifier (hex string)
* `--profile-audit-log-schema <TEXT>` — Optional audit log schema hint
* `--profile-pagerduty-service <TEXT>` — Optional `PagerDuty` service label



## `iroha_cli app space-directory bindings`

Inspect UAID bindings surfaced by Torii

**Usage:** `iroha_cli app space-directory bindings <COMMAND>`

###### **Subcommands:**

* `fetch` — Fetch UAID dataspace bindings via Torii



## `iroha_cli app space-directory bindings fetch`

Fetch UAID dataspace bindings via Torii

**Usage:** `iroha_cli app space-directory bindings fetch [OPTIONS] --uaid <UAID>`

###### **Options:**

* `--uaid <UAID>` — UAID literal whose bindings should be fetched
* `--address-format <ADDRESS_FORMAT>` — Preferred account literal encoding (`ih58` or `compressed`)

  Default value: `ih58`

  Possible values: `ih58`, `compressed`

* `--json-out <PATH>` — Optional path where the JSON response will be stored



## `iroha_cli app kaigi`

Kaigi session helpers

**Usage:** `iroha_cli app kaigi <COMMAND>`

###### **Subcommands:**

* `create` — Create a new Kaigi session
* `quickstart` — Bootstrap a Kaigi session for demos and shareable testing metadata
* `join` — Join a Kaigi session
* `leave` — Leave a Kaigi session
* `end` — End an active Kaigi session
* `record-usage` — Record usage statistics for a Kaigi session
* `report-relay-health` — Report the health status of a relay used by a Kaigi session



## `iroha_cli app kaigi create`

Create a new Kaigi session

**Usage:** `iroha_cli app kaigi create [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --host <ACCOUNT-ID>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call (e.g. `kaigi`)
* `--call-name <NAME>` — Call name within the domain (e.g. `daily-sync`)
* `--host <ACCOUNT-ID>` — Host account identifier responsible for the call (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--title <TITLE>` — Optional human friendly title
* `--description <DESCRIPTION>` — Optional description for participants
* `--max-participants <U32>` — Maximum concurrent participants (excluding host)
* `--gas-rate-per-minute <U64>` — Gas rate charged per minute (defaults to 0)

  Default value: `0`
* `--billing-account <ACCOUNT-ID>` — Optional billing account that will cover usage (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--scheduled-start-ms <U64>` — Optional scheduled start timestamp (milliseconds since epoch)
* `--privacy-mode <PRIVACY_MODE>` — Privacy mode for the session (defaults to `transparent`)

  Default value: `transparent`

  Possible values: `transparent`, `zk-roster-v1`

* `--room-policy <ROOM_POLICY>` — Room access policy controlling viewer authentication

  Default value: `authenticated`

  Possible values: `public`, `authenticated`

* `--relay-manifest <PATH>` — Path to a JSON file describing the relay manifest (optional)
* `--metadata-json <PATH>` — Path to a JSON file providing additional metadata (object with string keys)



## `iroha_cli app kaigi quickstart`

Bootstrap a Kaigi session for demos and shareable testing metadata

**Usage:** `iroha_cli app kaigi quickstart [OPTIONS]`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call

  Default value: `wonderland`
* `--call-name <NAME>` — Call name within the domain (defaults to a timestamp-based identifier)
* `--host <ACCOUNT-ID>` — Host account identifier responsible for the call (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--privacy-mode <PRIVACY_MODE>` — Privacy mode for the session (defaults to `transparent`)

  Default value: `transparent`

  Possible values: `transparent`, `zk-roster-v1`

* `--room-policy <ROOM_POLICY>` — Room access policy controlling viewer authentication

  Default value: `authenticated`

  Possible values: `public`, `authenticated`

* `--relay-manifest <PATH>` — Path to a JSON file describing the relay manifest (optional)
* `--metadata-json <PATH>` — Path to a JSON file providing additional metadata (object with string keys)
* `--auto-join-host` — Automatically join the host account immediately after creation
* `--summary-out <PATH>` — File path where the JSON summary should be written (defaults to stdout only)
* `--spool-hint <PATH>` — Root directory where `SoraNet` spool files are expected (informational only)

  Default value: `storage/streaming/soranet_routes`



## `iroha_cli app kaigi join`

Join a Kaigi session

**Usage:** `iroha_cli app kaigi join [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --participant <ACCOUNT-ID>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--participant <ACCOUNT-ID>` — Participant account joining the call (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--commitment-hex <HEX>` — Commitment hash (hex) for privacy mode joins
* `--commitment-alias <COMMITMENT_ALIAS>` — Alias tag describing the commitment (privacy mode)
* `--nullifier-hex <HEX>` — Nullifier hash (hex) preventing duplicate joins (privacy mode)
* `--nullifier-issued-at-ms <U64>` — Nullifier issuance timestamp (milliseconds since epoch)
* `--roster-root-hex <HEX>` — Roster Merkle root bound into the proof transcript (privacy mode)
* `--proof-hex <HEX>` — Proof bytes attesting ownership (hex encoding of raw bytes)



## `iroha_cli app kaigi leave`

Leave a Kaigi session

**Usage:** `iroha_cli app kaigi leave [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --participant <ACCOUNT-ID>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--participant <ACCOUNT-ID>` — Participant account leaving the call (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--commitment-hex <HEX>` — Commitment hash (hex) identifying the participant in privacy mode
* `--nullifier-hex <HEX>` — Nullifier hash (hex) preventing duplicate leaves (privacy mode)
* `--nullifier-issued-at-ms <U64>` — Nullifier issuance timestamp (milliseconds since epoch)
* `--roster-root-hex <HEX>` — Roster Merkle root bound into the proof transcript (privacy mode)
* `--proof-hex <HEX>` — Proof bytes attesting ownership (hex encoding of raw bytes)



## `iroha_cli app kaigi end`

End an active Kaigi session

**Usage:** `iroha_cli app kaigi end [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--ended-at-ms <U64>` — Optional timestamp in milliseconds when the call ended



## `iroha_cli app kaigi record-usage`

Record usage statistics for a Kaigi session

**Usage:** `iroha_cli app kaigi record-usage [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --duration-ms <U64>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--duration-ms <U64>` — Duration in milliseconds for this usage segment
* `--billed-gas <U64>` — Gas billed for this segment

  Default value: `0`
* `--usage-commitment-hex <HEX>` — Optional usage commitment hash (privacy mode)
* `--proof-hex <HEX>` — Optional proof bytes attesting the usage delta (privacy mode)



## `iroha_cli app kaigi report-relay-health`

Report the health status of a relay used by a Kaigi session

**Usage:** `iroha_cli app kaigi report-relay-health [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --relay <ACCOUNT-ID> --status <STATUS> --reported-at-ms <U64>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--relay <ACCOUNT-ID>` — Relay account identifier being reported (IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or <alias|public_key>@domain)
* `--status <STATUS>` — Observed health status for the relay

  Possible values: `healthy`, `degraded`, `unavailable`

* `--reported-at-ms <U64>` — Timestamp in milliseconds when the status was observed
* `--notes <NOTES>` — Optional notes capturing failure or recovery context



## `iroha_cli app sorafs`

SoraFS helpers (pin registry, aliases, replication orders, storage)

**Usage:** `iroha_cli app sorafs <COMMAND>`

###### **Subcommands:**

* `pin` — Interact with the pin registry
* `alias` — List alias bindings
* `replication` — List replication orders
* `storage` — Storage helpers (pin, etc.)
* `gateway` — Gateway policy and configuration helpers
* `incentives` — Offline helpers for relay payouts, disputes, and dashboards
* `handshake` — Observe or modify the Torii `SoraNet` handshake configuration
* `toolkit` — Local tooling for packaging manifests and payloads
* `guard-directory` — Guard directory helpers (fetch/verify snapshots)
* `reserve` — Reserve + rent policy helpers
* `gar` — GAR policy evidence helpers
* `repair` — Repair queue helpers (list, claim, close, escalate)
* `gc` — GC inspection helpers (no manual deletions)
* `fetch` — Orchestrate multi-provider chunk fetches via gateways



## `iroha_cli app sorafs pin`

Interact with the pin registry

**Usage:** `iroha_cli app sorafs pin <COMMAND>`

###### **Subcommands:**

* `list` — List manifests registered in the pin registry
* `show` — Fetch a single manifest, aliases, and replication orders
* `register` — Register a manifest in the pin registry via Torii



## `iroha_cli app sorafs pin list`

List manifests registered in the pin registry

**Usage:** `iroha_cli app sorafs pin list [OPTIONS]`

###### **Options:**

* `--status <STATUS>` — Optional status filter (pending, approved, retired)
* `--limit <LIMIT>` — Maximum number of manifests to return
* `--offset <OFFSET>` — Offset for pagination



## `iroha_cli app sorafs pin show`

Fetch a single manifest, aliases, and replication orders

**Usage:** `iroha_cli app sorafs pin show --digest <HEX>`

###### **Options:**

* `--digest <HEX>` — Hex-encoded manifest digest



## `iroha_cli app sorafs pin register`

Register a manifest in the pin registry via Torii

**Usage:** `iroha_cli app sorafs pin register [OPTIONS] --manifest <PATH> --chunk-digest <HEX> --submitted-epoch <SUBMITTED_EPOCH>`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded manifest (`.to`) file
* `--chunk-digest <HEX>` — Hex-encoded SHA3-256 digest of the chunk metadata plan
* `--submitted-epoch <SUBMITTED_EPOCH>` — Epoch recorded when submitting the manifest
* `--alias-namespace <ALIAS_NAMESPACE>` — Optional alias namespace to bind alongside the manifest
* `--alias-name <ALIAS_NAME>` — Optional alias name to bind alongside the manifest
* `--alias-proof <PATH>` — Optional path to the alias proof payload (binary)
* `--successor-of <HEX>` — Optional predecessor manifest digest (hex)



## `iroha_cli app sorafs alias`

List alias bindings

**Usage:** `iroha_cli app sorafs alias <COMMAND>`

###### **Subcommands:**

* `list` — List alias bindings exposed via Torii



## `iroha_cli app sorafs alias list`

List alias bindings exposed via Torii

**Usage:** `iroha_cli app sorafs alias list [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of aliases to return
* `--offset <OFFSET>` — Offset for pagination
* `--namespace <NAMESPACE>` — Restrict aliases to a namespace (case-insensitive)
* `--manifest-digest <HEX>` — Restrict aliases bound to a manifest digest (hex-encoded)



## `iroha_cli app sorafs replication`

List replication orders

**Usage:** `iroha_cli app sorafs replication <COMMAND>`

###### **Subcommands:**

* `list` — List replication orders



## `iroha_cli app sorafs replication list`

List replication orders

**Usage:** `iroha_cli app sorafs replication list [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of orders to return
* `--offset <OFFSET>` — Offset for pagination
* `--status <STATUS>` — Optional status filter (pending, completed, expired)
* `--manifest-digest <HEX>` — Restrict to orders for a manifest digest (hex-encoded)



## `iroha_cli app sorafs storage`

Storage helpers (pin, etc.)

**Usage:** `iroha_cli app sorafs storage <COMMAND>`

###### **Subcommands:**

* `pin` — Submit a manifest + payload to local storage for pinning
* `token` — Issue and inspect stream tokens for chunk-range gateways



## `iroha_cli app sorafs storage pin`

Submit a manifest + payload to local storage for pinning

**Usage:** `iroha_cli app sorafs storage pin --manifest <PATH> --payload <PATH>`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded manifest (`.to` file)
* `--payload <PATH>` — Path to the raw payload bytes referenced by the manifest



## `iroha_cli app sorafs storage token`

Issue and inspect stream tokens for chunk-range gateways

**Usage:** `iroha_cli app sorafs storage token <COMMAND>`

###### **Subcommands:**

* `issue` — Issue a stream token for a manifest/provider pair



## `iroha_cli app sorafs storage token issue`

Issue a stream token for a manifest/provider pair

**Usage:** `iroha_cli app sorafs storage token issue [OPTIONS] --manifest-id <HEX> --provider-id <HEX> --client-id <STRING>`

###### **Options:**

* `--manifest-id <HEX>` — Hex-encoded manifest identifier stored on the gateway
* `--provider-id <HEX>` — Hex-encoded provider identifier authorised to serve the manifest
* `--client-id <STRING>` — Logical client identifier used for quota accounting
* `--nonce <STRING>` — Optional nonce to send in the request headers (auto-generated when omitted)
* `--ttl-secs <SECONDS>` — Override the default TTL expressed in seconds
* `--max-streams <COUNT>` — Override the maximum concurrent stream count
* `--rate-limit-bytes <BYTES>` — Override the sustained throughput limit in bytes per second
* `--requests-per-minute <COUNT>` — Override the allowed number of refresh requests per minute



## `iroha_cli app sorafs gateway`

Gateway policy and configuration helpers

**Usage:** `iroha_cli app sorafs gateway <COMMAND>`

###### **Subcommands:**

* `lint-denylist` — Validate a denylist file against gateway policy rules
* `update-denylist` — Apply additions/removals to a denylist bundle with deterministic ordering
* `template-config` — Emit a TOML snippet with gateway configuration defaults
* `generate-hosts` — Derive canonical/vanity hostnames for a provider
* `route-plan` — Render the headers + route binding plan for a manifest rollout
* `cache-invalidate` — Generate a cache invalidation payload and curl snippet for GAR/SoraFS gateways
* `evidence` — Emit an evidence summary for a denylist bundle
* `direct-mode` — Direct-mode planning and configuration helpers
* `merkle` — Merkle snapshot/proof tooling for denylist bundles



## `iroha_cli app sorafs gateway lint-denylist`

Validate a denylist file against gateway policy rules

**Usage:** `iroha_cli app sorafs gateway lint-denylist --path <PATH>`

###### **Options:**

* `--path <PATH>` — Path to the JSON denylist file to validate



## `iroha_cli app sorafs gateway update-denylist`

Apply additions/removals to a denylist bundle with deterministic ordering

**Usage:** `iroha_cli app sorafs gateway update-denylist [OPTIONS] --base <PATH>`

###### **Options:**

* `--base <PATH>` — Base denylist JSON bundle to update
* `--add <PATH>` — Additional denylist fragments to merge (JSON array of entries)
* `--remove-descriptor <KIND:VALUE>` — Descriptors to remove (use output from the Merkle snapshot for accuracy)
* `--out <PATH>` — Destination path for the updated denylist (defaults to in-place)
* `--snapshot-out <PATH>` — Optional Merkle snapshot JSON artefact path
* `--snapshot-norito-out <PATH>` — Optional Merkle snapshot Norito artefact path
* `--evidence-out <PATH>` — Optional evidence summary output path
* `--label <STRING>` — Optional label stored in evidence output
* `--force` — Allow overwriting the destination file
* `--allow-replacement` — Permit replacing existing descriptors when merging additions
* `--allow-missing-removals` — Do not error if a requested removal is missing from the base



## `iroha_cli app sorafs gateway template-config`

Emit a TOML snippet with gateway configuration defaults

**Usage:** `iroha_cli app sorafs gateway template-config [OPTIONS]`

###### **Options:**

* `--host <HOSTNAME>` — Hostname to include in the ACME / gateway sample (repeatable)
* `--denylist-path <PATH>` — Optional denylist path to include in the template

  Default value: `docs/source/sorafs_gateway_denylist_sample.json`



## `iroha_cli app sorafs gateway generate-hosts`

Derive canonical/vanity hostnames for a provider

**Usage:** `iroha_cli app sorafs gateway generate-hosts [OPTIONS] --provider-id <HEX>`

###### **Options:**

* `--provider-id <HEX>` — Provider identifier (hex, 32 bytes)
* `--chain-id <CHAIN_ID>` — Chain id (network identifier)

  Default value: `nexus`



## `iroha_cli app sorafs gateway route-plan`

Render the headers + route binding plan for a manifest rollout

**Usage:** `iroha_cli app sorafs gateway route-plan [OPTIONS] --manifest-json <PATH> --hostname <HOSTNAME>`

###### **Options:**

* `--manifest-json <PATH>` — Manifest JSON path for the route being promoted
* `--hostname <HOSTNAME>` — Hostname that serves the manifest after promotion
* `--alias <NAMESPACE:NAME>` — Optional alias binding (`namespace:name`) to embed in the headers
* `--route-label <LABEL>` — Optional logical label applied to the rendered `Sora-Route-Binding`
* `--proof-status <STATUS>` — Optional proof-status string for the generated `Sora-Proof-Status`
* `--release-tag <STRING>` — Optional release tag stored alongside the plan
* `--cutover-window <WINDOW>` — Optional cutover window (RFC3339 interval or freeform note)
* `--out <PATH>` — Path where the JSON plan will be written

  Default value: `artifacts/sorafs_gateway/route_plan.json`
* `--headers-out <PATH>` — Optional path storing the primary header block
* `--rollback-manifest-json <PATH>` — Optional rollback manifest path (renders a secondary header block)
* `--rollback-headers-out <PATH>` — Optional path for the rollback header block
* `--rollback-route-label <LABEL>` — Optional label applied to the rollback binding
* `--rollback-release-tag <STRING>` — Optional release tag for the rollback binding metadata
* `--no-csp` — Skip emitting the default Content-Security-Policy header
* `--no-permissions-policy` — Skip emitting the default Permissions-Policy header
* `--no-hsts` — Skip emitting the default `Strict-Transport-Security` header



## `iroha_cli app sorafs gateway cache-invalidate`

Generate a cache invalidation payload and curl snippet for GAR/SoraFS gateways

**Usage:** `iroha_cli app sorafs gateway cache-invalidate [OPTIONS] --endpoint <URL> --alias <NAMESPACE:NAME> --manifest-digest <HEX>`

###### **Options:**

* `--endpoint <URL>` — Cache invalidation API endpoint (HTTP/S)
* `--alias <NAMESPACE:NAME>` — Alias bindings (`namespace:name`) that should be purged (repeatable)
* `--manifest-digest <HEX>` — Manifest digest (hex, 32 bytes) associated with the release
* `--car-digest <HEX>` — Optional CAR digest (hex, 32 bytes) to attach to the request
* `--release-tag <STRING>` — Optional release tag metadata included in the payload
* `--auth-env <ENV>` — Environment variable that stores the cache purge bearer token

  Default value: `CACHE_PURGE_TOKEN`
* `--output <PATH>` — Optional path where the JSON payload will be written



## `iroha_cli app sorafs gateway evidence`

Emit an evidence summary for a denylist bundle

**Usage:** `iroha_cli app sorafs gateway evidence [OPTIONS]`

###### **Options:**

* `--denylist <PATH>` — Path to the JSON denylist file to summarise

  Default value: `docs/source/sorafs_gateway_denylist_sample.json`
* `--out <PATH>` — Output path for the evidence JSON bundle

  Default value: `artifacts/sorafs_gateway/denylist_evidence.json`
* `--label <STRING>` — Optional evidence label embedded in the output



## `iroha_cli app sorafs gateway direct-mode`

Direct-mode planning and configuration helpers

**Usage:** `iroha_cli app sorafs gateway direct-mode <COMMAND>`

###### **Subcommands:**

* `plan` — Analyse manifest/admission data and emit a direct-mode readiness plan
* `enable` — Emit a configuration snippet enabling direct-mode overrides from a plan
* `rollback` — Emit a configuration snippet restoring default gateway security settings



## `iroha_cli app sorafs gateway direct-mode plan`

Analyse manifest/admission data and emit a direct-mode readiness plan

**Usage:** `iroha_cli app sorafs gateway direct-mode plan [OPTIONS] --manifest <PATH>`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded manifest (`.to`) file to analyse
* `--admission-envelope <PATH>` — Optional provider admission envelope (`.to`) for capability detection
* `--provider-id <HEX>` — Override provider identifier (hex) when no admission envelope is supplied
* `--chain-id <CHAIN_ID>` — Override chain id (defaults to the CLI configuration chain id)
* `--scheme <SCHEME>` — URL scheme to use for generated direct-CAR endpoints (default: https)

  Default value: `https`



## `iroha_cli app sorafs gateway direct-mode enable`

Emit a configuration snippet enabling direct-mode overrides from a plan

**Usage:** `iroha_cli app sorafs gateway direct-mode enable --plan <PATH>`

###### **Options:**

* `--plan <PATH>` — Path to the JSON output produced by `sorafs gateway direct-mode plan`



## `iroha_cli app sorafs gateway direct-mode rollback`

Emit a configuration snippet restoring default gateway security settings

**Usage:** `iroha_cli app sorafs gateway direct-mode rollback`



## `iroha_cli app sorafs gateway merkle`

Merkle snapshot/proof tooling for denylist bundles

**Usage:** `iroha_cli app sorafs gateway merkle <COMMAND>`

###### **Subcommands:**

* `snapshot` — Compute the Merkle root summary for a denylist bundle
* `proof` — Emit a membership proof for a single denylist entry



## `iroha_cli app sorafs gateway merkle snapshot`

Compute the Merkle root summary for a denylist bundle

**Usage:** `iroha_cli app sorafs gateway merkle snapshot [OPTIONS]`

###### **Options:**

* `--denylist <PATH>` — Path to the denylist JSON bundle

  Default value: `docs/source/sorafs_gateway_denylist_sample.json`
* `--json-out <PATH>` — Optional path to persist the JSON summary

  Default value: `artifacts/sorafs_gateway/denylist_merkle_snapshot.json`
* `--norito-out <PATH>` — Optional path to persist the Norito-encoded snapshot artefact



## `iroha_cli app sorafs gateway merkle proof`

Emit a membership proof for a single denylist entry

**Usage:** `iroha_cli app sorafs gateway merkle proof [OPTIONS]`

###### **Options:**

* `--denylist <PATH>` — Path to the denylist JSON bundle

  Default value: `docs/source/sorafs_gateway_denylist_sample.json`
* `--index <INDEX>` — Zero-based index of the entry to prove (see the snapshot listing)
* `--descriptor <KIND:VALUE>` — Descriptor of the entry to prove (`kind:value` from the snapshot output)
* `--json-out <PATH>` — Optional path to persist the JSON proof artefact

  Default value: `artifacts/sorafs_gateway/denylist_merkle_proof.json`
* `--norito-out <PATH>` — Optional path to persist the Norito-encoded proof artefact



## `iroha_cli app sorafs incentives`

Offline helpers for relay payouts, disputes, and dashboards

**Usage:** `iroha_cli app sorafs incentives <COMMAND>`

###### **Subcommands:**

* `compute` — Compute a relay reward instruction from metrics and bond state
* `open-dispute` — Open a dispute against an existing reward instruction
* `dashboard` — Summarise reward instructions into an earnings dashboard
* `service` — Manage the persistent treasury payout state and disputes



## `iroha_cli app sorafs incentives compute`

Compute a relay reward instruction from metrics and bond state

**Usage:** `iroha_cli app sorafs incentives compute [OPTIONS] --config <PATH> --metrics <PATH> --bond <PATH> --beneficiary <ACCOUNT_ID>`

###### **Options:**

* `--config <PATH>` — Path to the reward configuration JSON
* `--metrics <PATH>` — Norito-encoded relay metrics (`RelayEpochMetricsV1`)
* `--bond <PATH>` — Norito-encoded bond ledger entry (`RelayBondLedgerEntryV1`)
* `--beneficiary <ACCOUNT_ID>` — Account ID that will receive the payout
* `--norito-out <PATH>` — Optional path where the Norito-encoded reward instruction will be written
* `--pretty` — Emit pretty-printed JSON

  Default value: `false`



## `iroha_cli app sorafs incentives open-dispute`

Open a dispute against an existing reward instruction

**Usage:** `iroha_cli app sorafs incentives open-dispute [OPTIONS] --instruction <PATH> --treasury-account <ACCOUNT_ID> --submitted-by <ACCOUNT_ID> --requested-amount <NUMERIC> --reason <TEXT>`

###### **Options:**

* `--instruction <PATH>` — Norito-encoded reward instruction (`RelayRewardInstructionV1`)
* `--treasury-account <ACCOUNT_ID>` — Treasury account initiating the dispute
* `--submitted-by <ACCOUNT_ID>` — Account ID submitting the dispute
* `--requested-amount <NUMERIC>` — Requested adjustment amount (Numeric)
* `--reason <TEXT>` — Reason provided by the operator
* `--submitted-at <SECONDS>` — Optional UNIX timestamp when the dispute is filed
* `--norito-out <PATH>` — Optional path where the Norito-encoded dispute will be written
* `--pretty` — Emit pretty-printed JSON

  Default value: `false`



## `iroha_cli app sorafs incentives dashboard`

Summarise reward instructions into an earnings dashboard

**Usage:** `iroha_cli app sorafs incentives dashboard --instruction <PATH>...`

###### **Options:**

* `--instruction <PATH>` — Reward instruction payloads to include in the dashboard



## `iroha_cli app sorafs incentives service`

Manage the persistent treasury payout state and disputes

**Usage:** `iroha_cli app sorafs incentives service <COMMAND>`

###### **Subcommands:**

* `init` — Initialise a new payout ledger state file
* `process` — Evaluate metrics, record the payout, and persist the updated state
* `record` — Record an externally prepared reward instruction into the state
* `dispute` — Manage payout disputes recorded in the state
* `dashboard` — Render an earnings dashboard sourced from the persisted ledger
* `audit` — Audit bond/payout governance readiness for relay incentives
* `shadow-run` — Run a shadow simulation across relay metrics and summarise fairness
* `reconcile` — Reconcile recorded payouts against XOR ledger exports
* `daemon` — Run the treasury daemon against a metrics spool



## `iroha_cli app sorafs incentives service init`

Initialise a new payout ledger state file

**Usage:** `iroha_cli app sorafs incentives service init [OPTIONS] --state <PATH> --config <PATH> --treasury-account <ACCOUNT_ID>`

###### **Options:**

* `--state <PATH>` — Path where the incentives state JSON will be stored
* `--config <PATH>` — Reward configuration JSON consumed by the payout engine
* `--treasury-account <ACCOUNT_ID>` — Treasury account debited when materialising payouts
* `--force` — Overwrite an existing state file if it already exists

  Default value: `false`
* `--allow-missing-budget-approval` — Allow missing `budget_approval_id` in the reward configuration (for lab/staging replays)

  Default value: `false`



## `iroha_cli app sorafs incentives service process`

Evaluate metrics, record the payout, and persist the updated state

**Usage:** `iroha_cli app sorafs incentives service process [OPTIONS] --state <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--metrics <PATH>` — Norito-encoded relay metrics (`RelayEpochMetricsV1`)
* `--bond <PATH>` — Norito-encoded bond ledger entry (`RelayBondLedgerEntryV1`)
* `--beneficiary <ACCOUNT_ID>` — Beneficiary account that receives the payout
* `--instruction-out <PATH>` — Write the Norito-encoded reward instruction to this path
* `--transfer-out <PATH>` — Write the Norito-encoded transfer instruction to this path
* `--submit-transfer` — Submit the resulting transfer to Torii after recording the payout

  Default value: `false`
* `--pretty` — Emit pretty JSON instead of a compact payload

  Default value: `false`



## `iroha_cli app sorafs incentives service record`

Record an externally prepared reward instruction into the state

**Usage:** `iroha_cli app sorafs incentives service record [OPTIONS] --state <PATH> --instruction <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--instruction <PATH>` — Norito-encoded reward instruction to record
* `--transfer-out <PATH>` — Write the Norito-encoded transfer instruction to this path if non-zero
* `--submit-transfer` — Submit the transfer to Torii after recording the payout

  Default value: `false`
* `--pretty` — Emit pretty JSON instead of a compact payload

  Default value: `false`



## `iroha_cli app sorafs incentives service dispute`

Manage payout disputes recorded in the state

**Usage:** `iroha_cli app sorafs incentives service dispute <COMMAND>`

###### **Subcommands:**

* `file` — File a new dispute against a recorded payout
* `resolve` — Resolve a dispute with the supplied outcome
* `reject` — Reject a dispute without altering the ledger



## `iroha_cli app sorafs incentives service dispute file`

File a new dispute against a recorded payout

**Usage:** `iroha_cli app sorafs incentives service dispute file [OPTIONS] --state <PATH> --relay-id <HEX> --epoch <EPOCH> --submitted-by <ACCOUNT_ID> --requested-amount <NUMERIC> --reason <TEXT>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--relay-id <HEX>` — Hex-encoded relay identifier (32 bytes, 64 hex chars)
* `--epoch <EPOCH>` — Epoch number associated with the disputed payout
* `--submitted-by <ACCOUNT_ID>` — Account ID submitting the dispute
* `--requested-amount <NUMERIC>` — Requested payout amount (Numeric)
* `--reason <TEXT>` — Free-form reason describing the dispute
* `--filed-at <SECONDS>` — Optional UNIX timestamp indicating when the dispute was filed (defaults to now)
* `--adjust-credit <NUMERIC>` — Credit adjustment requested by the operator
* `--adjust-debit <NUMERIC>` — Debit adjustment requested by the operator
* `--norito-out <PATH>` — Write the Norito-encoded dispute payload to this path
* `--pretty` — Emit pretty JSON instead of a compact payload

  Default value: `false`



## `iroha_cli app sorafs incentives service dispute resolve`

Resolve a dispute with the supplied outcome

**Usage:** `iroha_cli app sorafs incentives service dispute resolve [OPTIONS] --state <PATH> --dispute-id <ID> --resolution <RESOLUTION> --notes <TEXT>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--dispute-id <ID>` — Dispute identifier to resolve
* `--resolution <RESOLUTION>` — Resolution kind (`no-change`, `credit`, or `debit`)

  Possible values: `no-change`, `credit`, `debit`

* `--amount <NUMERIC>` — Amount applied when resolving with `credit` or `debit`
* `--notes <TEXT>` — Resolution notes recorded in the dispute metadata
* `--resolved-at <SECONDS>` — Optional UNIX timestamp when the dispute was resolved (defaults to now)
* `--transfer-out <PATH>` — Write the Norito-encoded transfer instruction generated by the resolution (if any)
* `--pretty` — Emit pretty JSON instead of a compact payload

  Default value: `false`



## `iroha_cli app sorafs incentives service dispute reject`

Reject a dispute without altering the ledger

**Usage:** `iroha_cli app sorafs incentives service dispute reject [OPTIONS] --state <PATH> --dispute-id <ID> --notes <TEXT>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--dispute-id <ID>` — Dispute identifier to reject
* `--notes <TEXT>` — Rejection notes captured in the dispute metadata
* `--rejected-at <SECONDS>` — Optional UNIX timestamp when the dispute was rejected (defaults to now)
* `--pretty` — Emit pretty JSON instead of a compact payload

  Default value: `false`



## `iroha_cli app sorafs incentives service dashboard`

Render an earnings dashboard sourced from the persisted ledger

**Usage:** `iroha_cli app sorafs incentives service dashboard --state <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON



## `iroha_cli app sorafs incentives service audit`

Audit bond/payout governance readiness for relay incentives

**Usage:** `iroha_cli app sorafs incentives service audit [OPTIONS] --state <PATH> --config <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--config <PATH>` — Daemon configuration describing relay beneficiaries and bond sources
* `--scope <SCOPES>` — Audit scopes to evaluate (repeat to combine); defaults to bond checks

  Default value: `bond`

  Possible values: `bond`, `budget`, `all`

* `--pretty` — Emit pretty JSON instead of a compact payload

  Default value: `false`



## `iroha_cli app sorafs incentives service shadow-run`

Run a shadow simulation across relay metrics and summarise fairness

**Usage:** `iroha_cli app sorafs incentives service shadow-run [OPTIONS] --state <PATH> --config <PATH> --metrics-dir <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--config <PATH>` — Shadow simulation configuration mapping relays to beneficiaries and bonds
* `--metrics-dir <PATH>` — Directory containing Norito-encoded relay metrics snapshots (`relay-<id>-epoch-<n>.to`)
* `--report-out <PATH>` — Optional path to write the shadow simulation report JSON
* `--pretty` — Emit pretty JSON instead of a compact payload

  Default value: `false`
* `--allow-missing-budget-approval` — Allow payouts without `budget_approval_id` (for local testing only)

  Default value: `false`



## `iroha_cli app sorafs incentives service reconcile`

Reconcile recorded payouts against XOR ledger exports

**Usage:** `iroha_cli app sorafs incentives service reconcile [OPTIONS] --state <PATH> --ledger-export <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--ledger-export <PATH>` — Norito-encoded XOR ledger export to reconcile against
* `--pretty` — Emit pretty JSON instead of a compact payload

  Default value: `false`



## `iroha_cli app sorafs incentives service daemon`

Run the treasury daemon against a metrics spool

**Usage:** `iroha_cli app sorafs incentives service daemon [OPTIONS] --state <PATH> --config <PATH> --metrics-dir <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--config <PATH>` — Daemon configuration describing relay beneficiaries and bond sources
* `--metrics-dir <PATH>` — Directory containing Norito-encoded relay metrics snapshots
* `--instruction-out-dir <PATH>` — Directory where reward instructions will be written
* `--transfer-out-dir <PATH>` — Directory where transfer instructions will be written
* `--archive-dir <PATH>` — Directory where processed metrics snapshots will be archived
* `--poll-interval <SECONDS>` — Poll interval (seconds) when running continuously

  Default value: `30`
* `--once` — Process the spool once and exit (do not watch for changes)

  Default value: `false`
* `--pretty` — Emit JSON summaries instead of plain-text logs

  Default value: `false`
* `--allow-missing-budget-approval` — Allow payouts without `budget_approval_id` (for local testing only)

  Default value: `false`



## `iroha_cli app sorafs handshake`

Observe or modify the Torii `SoraNet` handshake configuration

**Usage:** `iroha_cli app sorafs handshake <COMMAND>`

###### **Subcommands:**

* `show` — Display the current `SoraNet` handshake summary as reported by Torii
* `update` — Update one or more `SoraNet` handshake parameters via `/v1/config`
* `token` — Admission token helpers (issuance, fingerprinting, revocation digests)



## `iroha_cli app sorafs handshake show`

Display the current `SoraNet` handshake summary as reported by Torii

**Usage:** `iroha_cli app sorafs handshake show`



## `iroha_cli app sorafs handshake update`

Update one or more `SoraNet` handshake parameters via `/v1/config`

**Usage:** `iroha_cli app sorafs handshake update [OPTIONS]`

###### **Options:**

* `--descriptor-commit <HEX>` — Override the descriptor commitment advertised during handshake (hex)
* `--client-capabilities <HEX>` — Override the client capability TLV vector (hex)
* `--relay-capabilities <HEX>` — Override the relay capability TLV vector (hex)
* `--kem-id <KEM_ID>` — Override the negotiated ML-KEM identifier
* `--sig-id <SIG_ID>` — Override the negotiated signature suite identifier
* `--resume-hash <HEX>` — Override the resume hash advertised to peers (64 hex chars)
* `--clear-resume-hash` — Clear the configured resume hash
* `--pow-required` — Require proof-of-work tickets for admission (`--pow-optional` disables)
* `--pow-optional` — Disable mandatory proof-of-work tickets
* `--pow-difficulty <POW_DIFFICULTY>` — Override the proof-of-work difficulty
* `--pow-max-future-skew <POW_MAX_FUTURE_SKEW>` — Override the maximum clock skew accepted on `PoW` tickets (seconds)
* `--pow-min-ttl <POW_MIN_TTL>` — Override the minimum `PoW` ticket TTL (seconds)
* `--pow-ttl <POW_TTL>` — Override the `PoW` ticket TTL (seconds)
* `--pow-puzzle-enable` — Enable the Argon2 puzzle gate for handshake admission (`--pow-puzzle-disable` clears)
* `--pow-puzzle-disable` — Disable the Argon2 puzzle gate
* `--pow-puzzle-memory <POW_PUZZLE_MEMORY>` — Override the puzzle memory cost (KiB)
* `--pow-puzzle-time <POW_PUZZLE_TIME>` — Override the puzzle time cost (iterations)
* `--pow-puzzle-lanes <POW_PUZZLE_LANES>` — Override the puzzle parallelism (lanes)
* `--require-sm-handshake-match` — Require peers to match SM helper availability
* `--allow-sm-handshake-mismatch` — Allow mismatched SM helper availability
* `--require-sm-openssl-preview-match` — Require peers to match the OpenSSL preview flag
* `--allow-sm-openssl-preview-mismatch` — Allow mismatched OpenSSL preview flags



## `iroha_cli app sorafs handshake token`

Admission token helpers (issuance, fingerprinting, revocation digests)

**Usage:** `iroha_cli app sorafs handshake token <COMMAND>`

###### **Subcommands:**

* `issue` — Issue an ML-DSA admission token bound to a relay and transcript hash
* `id` — Compute the canonical revocation identifier for an admission token
* `fingerprint` — Compute the issuer fingerprint from an ML-DSA public key



## `iroha_cli app sorafs handshake token issue`

Issue an ML-DSA admission token bound to a relay and transcript hash

**Usage:** `iroha_cli app sorafs handshake token issue [OPTIONS] --relay-id <HEX> --transcript-hash <HEX>`

###### **Options:**

* `--suite <SUITE>` — ML-DSA suite used to sign the token (mldsa44, mldsa65, mldsa87)

  Default value: `mldsa44`

  Possible values: `mldsa44`, `mldsa65`, `mldsa87`

* `--issuer-secret-key <PATH>` — Path to the issuer ML-DSA secret key (raw bytes)
* `--issuer-secret-hex <HEX>` — Hex-encoded issuer ML-DSA secret key
* `--issuer-public-key <PATH>` — Path to the issuer ML-DSA public key (raw bytes)
* `--issuer-public-hex <HEX>` — Hex-encoded issuer ML-DSA public key
* `--relay-id <HEX>` — Hex-encoded 32-byte relay identifier bound into the token
* `--transcript-hash <HEX>` — Hex-encoded 32-byte transcript hash bound into the token
* `--issued-at <RFC3339>` — RFC3339 issuance timestamp (defaults to current UTC time)
* `--expires-at <RFC3339>` — RFC3339 expiry timestamp
* `--ttl <SECONDS>` — Token lifetime in seconds (defaults to 600s when --expires-at is omitted)
* `--flags <FLAGS>` — Token flags (opaque 8-bit field, defaults to 0)
* `--output <PATH>` — Optional path to write the encoded token
* `--output-format <OUTPUT_FORMAT>` — Encoding used when writing the token to --output (base64, hex, binary)

  Default value: `base64`

  Possible values: `base64`, `hex`, `binary`




## `iroha_cli app sorafs handshake token id`

Compute the canonical revocation identifier for an admission token

**Usage:** `iroha_cli app sorafs handshake token id [OPTIONS]`

###### **Options:**

* `--token <PATH>` — Path to the admission token frame (binary)
* `--token-hex <HEX>` — Hex-encoded admission token frame
* `--token-base64 <BASE64>` — Base64url-encoded admission token frame



## `iroha_cli app sorafs handshake token fingerprint`

Compute the issuer fingerprint from an ML-DSA public key

**Usage:** `iroha_cli app sorafs handshake token fingerprint [OPTIONS]`

###### **Options:**

* `--public-key <PATH>` — Path to the ML-DSA public key (raw bytes)
* `--public-key-hex <HEX>` — Hex-encoded ML-DSA public key



## `iroha_cli app sorafs toolkit`

Local tooling for packaging manifests and payloads

**Usage:** `iroha_cli app sorafs toolkit <COMMAND>`

###### **Subcommands:**

* `pack` — Package a payload into a CAR + manifest bundle using the canonical tooling



## `iroha_cli app sorafs toolkit pack`

Package a payload into a CAR + manifest bundle using the canonical tooling

**Usage:** `iroha_cli app sorafs toolkit pack [OPTIONS] <INPUT>`

###### **Arguments:**

* `<INPUT>` — Payload path (file or directory) to package into a CAR archive

###### **Options:**

* `--manifest-out <PATH>` — Path to write the Norito manifest (`.to`). If omitted, no manifest file is emitted
* `--car-out <PATH>` — Path to write the CAR archive
* `--json-out <PATH>` — Path to write the JSON report (defaults to stdout)
* `--hybrid-envelope-out <PATH>` — Path to write the hybrid payload envelope (binary)
* `--hybrid-envelope-json-out <PATH>` — Path to write the hybrid payload envelope (JSON)
* `--hybrid-recipient-x25519 <HEX>` — Hex-encoded X25519 public key used for hybrid envelope encryption
* `--hybrid-recipient-kyber <HEX>` — Hex-encoded Kyber public key used for hybrid envelope encryption



## `iroha_cli app sorafs guard-directory`

Guard directory helpers (fetch/verify snapshots)

**Usage:** `iroha_cli app sorafs guard-directory <COMMAND>`

###### **Subcommands:**

* `fetch` — Fetch a guard directory snapshot over HTTPS, verify it, and emit a summary
* `verify` — Verify a guard directory snapshot stored on disk



## `iroha_cli app sorafs guard-directory fetch`

Fetch a guard directory snapshot over HTTPS, verify it, and emit a summary

**Usage:** `iroha_cli app sorafs guard-directory fetch [OPTIONS] --url <URL>`

###### **Options:**

* `--url <URL>` — URLs publishing the guard directory snapshot (first success wins)
* `--output <PATH>` — Path where the verified snapshot will be stored (optional)
* `--expected-directory-hash <HEX>` — Expected directory hash (hex). Command fails when the snapshot hash differs
* `--timeout-secs <SECS>` — HTTP timeout in seconds (defaults to 30s)

  Default value: `30`
* `--overwrite` — Allow overwriting an existing file at --output



## `iroha_cli app sorafs guard-directory verify`

Verify a guard directory snapshot stored on disk

**Usage:** `iroha_cli app sorafs guard-directory verify [OPTIONS] --path <PATH>`

###### **Options:**

* `--path <PATH>` — Path to the guard directory snapshot to verify
* `--expected-directory-hash <HEX>` — Expected directory hash (hex). Command fails when the snapshot hash differs



## `iroha_cli app sorafs reserve`

Reserve + rent policy helpers

**Usage:** `iroha_cli app sorafs reserve <COMMAND>`

###### **Subcommands:**

* `quote` — Quote reserve requirements and effective rent for a given tier/capacity
* `ledger` — Convert a reserve quote into rent/reserve transfer instructions



## `iroha_cli app sorafs reserve quote`

Quote reserve requirements and effective rent for a given tier/capacity

**Usage:** `iroha_cli app sorafs reserve quote [OPTIONS] --storage-class <STORAGE_CLASS> --tier <TIER> --gib <GIB>`

###### **Options:**

* `--storage-class <STORAGE_CLASS>` — Storage class targeted by the commitment (hot, warm, cold)

  Possible values: `hot`, `warm`, `cold`

* `--tier <TIER>` — Provider tier (tier-a, tier-b, tier-c)

  Possible values: `tier-a`, `tier-b`, `tier-c`

* `--duration <DURATION>` — Commitment duration (`monthly`, `quarterly`, `annual`)

  Default value: `monthly`

  Possible values: `monthly`, `quarterly`, `annual`

* `--gib <GIB>` — Logical GiB covered by the quote
* `--reserve-balance <XOR>` — Reserve balance applied while computing the effective rent (XOR, up to 6 fractional digits)

  Default value: `0`
* `--policy-json <PATH>` — Optional path to a JSON-encoded reserve policy (`ReservePolicyV1`)
* `--policy-norito <PATH>` — Optional path to a Norito-encoded reserve policy (`ReservePolicyV1`)
* `--quote-out <PATH>` — Optional path for persisting the rendered quote JSON



## `iroha_cli app sorafs reserve ledger`

Convert a reserve quote into rent/reserve transfer instructions

**Usage:** `iroha_cli app sorafs reserve ledger --quote <PATH> --provider-account <ACCOUNT_ID> --treasury-account <ACCOUNT_ID> --reserve-account <ACCOUNT_ID> --asset-definition <NAME#DOMAIN>`

###### **Options:**

* `--quote <PATH>` — Path to the reserve quote JSON (output of `sorafs reserve quote`)
* `--provider-account <ACCOUNT_ID>` — Provider account paying the rent and reserve top-ups
* `--treasury-account <ACCOUNT_ID>` — Treasury account receiving the rent payment
* `--reserve-account <ACCOUNT_ID>` — Reserve escrow account receiving the reserve top-up
* `--asset-definition <NAME#DOMAIN>` — Asset definition identifier used for XOR transfers (e.g., `xor#sora`)



## `iroha_cli app sorafs gar`

GAR policy evidence helpers

**Usage:** `iroha_cli app sorafs gar <COMMAND>`

###### **Subcommands:**

* `receipt` — Render a GAR enforcement receipt artefact (JSON + optional Norito bytes)



## `iroha_cli app sorafs gar receipt`

Render a GAR enforcement receipt artefact (JSON + optional Norito bytes)

**Usage:** `iroha_cli app sorafs gar receipt [OPTIONS] --gar-name <LABEL> --canonical-host <HOST> --operator <ACCOUNT_ID> --reason <TEXT>`

###### **Options:**

* `--gar-name <LABEL>` — Registered GAR name (`SoraDNS` label, e.g., `docs.sora`)
* `--canonical-host <HOST>` — Canonical host affected by the enforcement action
* `--action <ACTION>` — Enforcement action recorded in the receipt

  Default value: `audit-notice`

  Possible values: `purge-static-zone`, `cache-bypass`, `ttl-override`, `rate-limit-override`, `geo-fence`, `legal-hold`, `moderation`, `audit-notice`, `custom`

* `--custom-action-slug <SLUG>` — Slug recorded when `--action custom` is selected
* `--receipt-id <HEX16>` — Optional receipt identifier (32 hex chars / 16 bytes). Defaults to a random ULID-like value
* `--triggered-at <RFC3339|@UNIX>` — Override the triggered timestamp (RFC3339 or `@unix_seconds`). Defaults to `now`
* `--expires-at <RFC3339|@UNIX>` — Optional expiry timestamp (RFC3339 or `@unix_seconds`)
* `--policy-version <STRING>` — Policy version label recorded in the receipt
* `--policy-digest <HEX32>` — Policy digest (64 hex chars / 32 bytes) referenced by the receipt
* `--operator <ACCOUNT_ID>` — Operator account that executed the action
* `--reason <TEXT>` — Human-readable reason for the enforcement action
* `--notes <TEXT>` — Optional notes captured for auditors
* `--evidence-uri <URI>` — Evidence URIs (repeatable) recorded with the receipt
* `--label <TAG>` — Machine-readable labels (repeatable) applied to the receipt
* `--json-out <PATH>` — Path for persisting the JSON artefact (pretty-printed)
* `--norito-out <PATH>` — Path for persisting the Norito-encoded receipt (`.to` bytes)



## `iroha_cli app sorafs repair`

Repair queue helpers (list, claim, close, escalate)

**Usage:** `iroha_cli app sorafs repair <COMMAND>`

###### **Subcommands:**

* `list` — List repair tickets (optionally filtered by manifest/provider/status)
* `claim` — Claim a queued repair ticket as a repair worker
* `complete` — Mark a repair ticket as completed
* `fail` — Mark a repair ticket as failed
* `escalate` — Escalate a repair ticket into a slash proposal



## `iroha_cli app sorafs repair list`

List repair tickets (optionally filtered by manifest/provider/status)

**Usage:** `iroha_cli app sorafs repair list [OPTIONS]`

###### **Options:**

* `--manifest-digest <HEX>` — Optional manifest digest to scope the listing
* `--status <STATUS>` — Optional status filter (queued, verifying, in_progress, completed, failed, escalated)
* `--provider-id <HEX>` — Optional provider identifier filter (hex-encoded)



## `iroha_cli app sorafs repair claim`

Claim a queued repair ticket as a repair worker

**Usage:** `iroha_cli app sorafs repair claim [OPTIONS] --ticket-id <ID> --manifest-digest <HEX> --provider-id <HEX>`

###### **Options:**

* `--ticket-id <ID>` — Repair ticket identifier (e.g., `REP-401`)
* `--manifest-digest <HEX>` — Manifest digest bound to the ticket (hex-encoded)
* `--provider-id <HEX>` — Provider identifier owning the ticket (hex-encoded)
* `--claimed-at <RFC3339|@UNIX>` — Optional timestamp for the claim (RFC3339 or `@unix_seconds`)
* `--idempotency-key <KEY>` — Optional idempotency key (auto-generated when omitted)



## `iroha_cli app sorafs repair complete`

Mark a repair ticket as completed

**Usage:** `iroha_cli app sorafs repair complete [OPTIONS] --ticket-id <ID> --manifest-digest <HEX> --provider-id <HEX>`

###### **Options:**

* `--ticket-id <ID>` — Repair ticket identifier (e.g., `REP-401`)
* `--manifest-digest <HEX>` — Manifest digest bound to the ticket (hex-encoded)
* `--provider-id <HEX>` — Provider identifier owning the ticket (hex-encoded)
* `--completed-at <RFC3339|@UNIX>` — Optional timestamp for the completion (RFC3339 or `@unix_seconds`)
* `--resolution-notes <TEXT>` — Optional resolution notes
* `--idempotency-key <KEY>` — Optional idempotency key (auto-generated when omitted)



## `iroha_cli app sorafs repair fail`

Mark a repair ticket as failed

**Usage:** `iroha_cli app sorafs repair fail [OPTIONS] --ticket-id <ID> --manifest-digest <HEX> --provider-id <HEX> --reason <TEXT>`

###### **Options:**

* `--ticket-id <ID>` — Repair ticket identifier (e.g., `REP-401`)
* `--manifest-digest <HEX>` — Manifest digest bound to the ticket (hex-encoded)
* `--provider-id <HEX>` — Provider identifier owning the ticket (hex-encoded)
* `--failed-at <RFC3339|@UNIX>` — Optional timestamp for the failure (RFC3339 or `@unix_seconds`)
* `--reason <TEXT>` — Failure reason
* `--idempotency-key <KEY>` — Optional idempotency key (auto-generated when omitted)



## `iroha_cli app sorafs repair escalate`

Escalate a repair ticket into a slash proposal

**Usage:** `iroha_cli app sorafs repair escalate [OPTIONS] --ticket-id <ID> --manifest-digest <HEX> --provider-id <HEX> --penalty-nano <NANO> --rationale <TEXT>`

###### **Options:**

* `--ticket-id <ID>` — Repair ticket identifier (e.g., `REP-401`)
* `--manifest-digest <HEX>` — Manifest digest bound to the ticket (hex-encoded)
* `--provider-id <HEX>` — Provider identifier owning the ticket (hex-encoded)
* `--penalty-nano <NANO>` — Proposed penalty amount in nano-XOR
* `--rationale <TEXT>` — Escalation rationale for governance review
* `--auditor <ACCOUNT_ID>` — Optional auditor account (defaults to the CLI account)
* `--submitted-at <RFC3339|@UNIX>` — Optional timestamp for the proposal (RFC3339 or `@unix_seconds`)
* `--approve-votes <COUNT>` — Optional approval votes in favor of the slash decision
* `--reject-votes <COUNT>` — Optional approval votes against the slash decision
* `--abstain-votes <COUNT>` — Optional approval abstain votes
* `--approved-at <RFC3339|@UNIX>` — Optional timestamp when approval was recorded (RFC3339 or `@unix_seconds`)
* `--finalized-at <RFC3339|@UNIX>` — Optional timestamp when the decision became final after appeals (RFC3339 or `@unix_seconds`)



## `iroha_cli app sorafs gc`

GC inspection helpers (no manual deletions)

**Usage:** `iroha_cli app sorafs gc <COMMAND>`

###### **Subcommands:**

* `inspect` — Inspect retained manifests and retention deadlines
* `dry-run` — Report which manifests would be evicted by GC (dry-run only)



## `iroha_cli app sorafs gc inspect`

Inspect retained manifests and retention deadlines

**Usage:** `iroha_cli app sorafs gc inspect [OPTIONS]`

###### **Options:**

* `--data-dir <PATH>` — Root directory for SoraFS storage data (defaults to the node config default)
* `--now <RFC3339|@UNIX>` — Override the reference timestamp (RFC3339 or `@unix_seconds`)
* `--grace-secs <SECONDS>` — Override the retention grace window in seconds



## `iroha_cli app sorafs gc dry-run`

Report which manifests would be evicted by GC (dry-run only)

**Usage:** `iroha_cli app sorafs gc dry-run [OPTIONS]`

###### **Options:**

* `--data-dir <PATH>` — Root directory for SoraFS storage data (defaults to the node config default)
* `--now <RFC3339|@UNIX>` — Override the reference timestamp (RFC3339 or `@unix_seconds`)
* `--grace-secs <SECONDS>` — Override the retention grace window in seconds



## `iroha_cli app sorafs fetch`

Orchestrate multi-provider chunk fetches via gateways

**Usage:** `iroha_cli app sorafs fetch [OPTIONS] --gateway-provider <SPEC>`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded manifest (`.to`) describing the payload layout
* `--plan <PATH>` — Path to the chunk fetch plan JSON (for example, `chunk_fetch_specs` from `iroha sorafs toolkit pack --json-out`)
* `--manifest-id <HEX>` — Hex-encoded manifest hash used as the manifest identifier on gateways
* `--gateway-provider <SPEC>` — Gateway provider descriptor (`name=... , provider-id=... , base-url=... , stream-token=...`)
* `--storage-ticket <HEX>` — Storage ticket identifier to fetch manifest + chunk plan automatically from Torii
* `--manifest-endpoint <URL>` — Optional override for the Torii manifest endpoint used with `--storage-ticket`
* `--manifest-cache-dir <PATH>` — Directory for storing manifest/chunk-plan artefacts fetched via `--storage-ticket`
* `--client-id <STRING>` — Optional client identifier forwarded to the gateway for auditing
* `--manifest-envelope <PATH>` — Optional path to a Norito-encoded manifest envelope to satisfy gateway policy checks
* `--manifest-cid <HEX>` — Override the expected manifest CID (defaults to the manifest digest)
* `--blinded-cid <BASE64>` — Canonical blinded CID (base64url, no padding) forwarded via `SoraNet` headers
* `--salt-epoch <EPOCH>` — Salt epoch corresponding to the blinded CID headers
* `--salt-hex <HEX>` — Hex-encoded 32-byte salt used to derive the canonical blinded CID (computes `--blinded-cid`)
* `--chunker-handle <STRING>` — Override the chunker handle advertised to gateways
* `--max-peers <COUNT>` — Limit the number of providers participating in the session
* `--retry-budget <COUNT>` — Maximum retry attempts per chunk (0 disables the cap)
* `--transport-policy <POLICY>` — Override the default `soranet-first` transport policy (`soranet-first`, `soranet-strict`, or `direct-only`). Supply `direct-only` only when staging a downgrade or rehearsing the compliance drills captured in `roadmap.md`
* `--anonymity-policy <POLICY>` — Override the staged anonymity policy (default `stage-a` / `anon-guard-pq`; accepts `anon-*` or `stage-*` labels)
* `--write-mode <MODE>` — Hint that tightens PQ expectations for write paths (`read-only` or `upload-pq-only`)
* `--transport-policy-override <POLICY>` — Force the orchestrator to stay on a specific transport stage (`soranet-first`, `soranet-strict`, or `direct-only`)
* `--anonymity-policy-override <POLICY>` — Force the orchestrator to stay on a specific anonymity stage (`stage-a`, `anon-guard-pq`, etc.)
* `--guard-cache <PATH>` — Path to the persisted guard cache (Norito-encoded guard set)
* `--guard-cache-key <HEX>` — Optional 32-byte hex key used to tag guard caches when persisting to disk
* `--guard-directory <PATH>` — Path to a guard directory JSON payload used to refresh guard selections
* `--guard-target <COUNT>` — Target number of entry guards to pin (defaults to 3 when the guard directory is provided)
* `--guard-retention-days <DAYS>` — Guard retention window in days (defaults to 30 when the guard directory is provided)
* `--output <PATH>` — Write the assembled payload to a file
* `--json-out <PATH>` — Override the summary JSON path (defaults to `artifacts/sorafs_orchestrator/latest/summary.json`)
* `--scoreboard-out <PATH>` — Override the scoreboard JSON path (defaults to `artifacts/sorafs_orchestrator/latest/scoreboard.json`)
* `--scoreboard-now <UNIX_SECS>` — Override the Unix timestamp used when evaluating provider adverts
* `--telemetry-source-label <LABEL>` — Label describing the telemetry stream captured alongside the scoreboard (persisted in metadata)
* `--telemetry-region <LABEL>` — Optional telemetry region label persisted in both the scoreboard metadata and summary JSON



## `iroha_cli app soracles`

Soracles helpers (evidence bundling)

**Usage:** `iroha_cli app soracles <COMMAND>`

###### **Subcommands:**

* `bundle` — Build an audit bundle containing oracle feed events and evidence files
* `catalog` — Show the oracle rejection/error catalog for SDK parity
* `evidence-gc` — Garbage-collect evidence bundles and prune unreferenced artifacts



## `iroha_cli app soracles bundle`

Build an audit bundle containing oracle feed events and evidence files

**Usage:** `iroha_cli app soracles bundle [OPTIONS] --events <PATH> --output <DIR>`

###### **Options:**

* `--events <PATH>` — Path to a JSON file containing `FeedEventRecord` values (array or single record)
* `--output <DIR>` — Directory where the bundle (manifest + hashed artefacts) will be written
* `--observations <DIR>` — Directory of observation JSON files to include (hashed and copied into the bundle)
* `--reports <DIR>` — Directory of report JSON files to include
* `--responses <DIR>` — Directory of connector response JSON files to include
* `--disputes <DIR>` — Directory of dispute evidence JSON files to include
* `--telemetry <PATH>` — Optional telemetry snapshot (JSON) to include in the bundle



## `iroha_cli app soracles catalog`

Show the oracle rejection/error catalog for SDK parity

**Usage:** `iroha_cli app soracles catalog [OPTIONS]`

###### **Options:**

* `--format <FORMAT>` — Output format (`json` for machine consumption, `markdown` for docs/runbooks)

  Default value: `json`

  Possible values: `json`, `markdown`




## `iroha_cli app soracles evidence-gc`

Garbage-collect evidence bundles and prune unreferenced artifacts

**Usage:** `iroha_cli app soracles evidence-gc [OPTIONS]`

###### **Options:**

* `--root <DIR>` — Root directory containing soracles evidence bundles (each with `bundle.json`)

  Default value: `artifacts/soracles`
* `--retention-days <DAYS>` — Retention period in days; bundles older than this are removed

  Default value: `180`
* `--dispute-retention-days <DAYS>` — Retention period for bundles containing dispute evidence (defaults to a longer window)

  Default value: `365`
* `--report <PATH>` — Emit a GC summary report to this path (defaults to `<root>/gc_report.json`)
* `--prune-unreferenced` — Remove artifact files that are not referenced by `bundle.json`
* `--dry-run` — Perform a dry run and only report what would be removed



## `iroha_cli app sns`

Sora Name Service helpers (registrar + policy tooling)

**Usage:** `iroha_cli app sns <COMMAND>`

###### **Subcommands:**

* `register` — Register a SNS name via `/v1/sns/registrations`
* `renew` — Renew a SNS name via `/v1/sns/registrations/{selector}/renew`
* `transfer` — Transfer ownership of a SNS name
* `update-controllers` — Replace controllers on a SNS name
* `freeze` — Freeze a SNS name
* `unfreeze` — Unfreeze a SNS name
* `registration` — Fetch a SNS name record
* `policy` — Fetch the policy for a suffix
* `governance` — Governance helpers (arbitration, transparency exports, etc.)



## `iroha_cli app sns register`

Register a SNS name via `/v1/sns/registrations`

**Usage:** `iroha_cli app sns register [OPTIONS] --label <LABEL> --suffix-id <U16>`

###### **Options:**

* `--label <LABEL>` — Label (without suffix) to register. Automatically lower-cased & NFC-normalised
* `--suffix-id <U16>` — Numeric suffix identifier (see `SuffixPolicyV1::suffix_id`)
* `--owner <ACCOUNT-ID>` — Owner account identifier; defaults to the CLI config account
* `--controller <ACCOUNT-ID>` — Controller account identifiers (repeatable). Defaults to `[owner]`
* `--term-years <U8>` — Registration term in years

  Default value: `1`
* `--pricing-class <U8>` — Optional pricing class hint advertised by the steward
* `--payment-json <PATH>` — Optional path to a JSON file containing `PaymentProofV1`. When omitted the inline flags are used
* `--payment-asset-id <ASSET-ID>` — Payment asset identifier (e.g., `xor#sora`)
* `--payment-gross <U64>` — Gross payment amount (base + surcharges) in native units
* `--payment-net <U64>` — Net payment amount forwarded to the registry. Defaults to `payment-gross`
* `--payment-settlement <JSON-OR-STRING>` — Settlement transaction reference (string or JSON literal)
* `--payment-payer <ACCOUNT-ID>` — Account that authorised the payment. Defaults to the CLI config account
* `--payment-signature <JSON-OR-STRING>` — Steward/treasury signature attesting to the payment (string or JSON literal)
* `--metadata-json <PATH>` — Optional path to a JSON object that will populate `Metadata`
* `--governance-json <PATH>` — Optional path to a JSON document describing `GovernanceHookV1`



## `iroha_cli app sns renew`

Renew a SNS name via `/v1/sns/registrations/{selector}/renew`

**Usage:** `iroha_cli app sns renew [OPTIONS] --selector <LABEL.SUFFIX>`

###### **Options:**

* `--selector <LABEL.SUFFIX>` — Selector literal (e.g. `makoto.sora`)
* `--term-years <U8>` — Additional term to purchase (years)

  Default value: `1`
* `--payment-json <PATH>` — Optional path to a JSON file containing `PaymentProofV1`. When omitted the inline flags are used
* `--payment-asset-id <ASSET-ID>` — Payment asset identifier (e.g., `xor#sora`)
* `--payment-gross <U64>` — Gross payment amount (base + surcharges) in native units
* `--payment-net <U64>` — Net payment amount forwarded to the registry. Defaults to `payment-gross`
* `--payment-settlement <JSON-OR-STRING>` — Settlement transaction reference (string or JSON literal)
* `--payment-payer <ACCOUNT-ID>` — Account that authorised the payment. Defaults to the CLI config account
* `--payment-signature <JSON-OR-STRING>` — Steward/treasury signature attesting to the payment (string or JSON literal)



## `iroha_cli app sns transfer`

Transfer ownership of a SNS name

**Usage:** `iroha_cli app sns transfer --selector <LABEL.SUFFIX> --new-owner <ACCOUNT-ID> --governance-json <PATH>`

###### **Options:**

* `--selector <LABEL.SUFFIX>` — Selector literal (e.g. `makoto.sora`)
* `--new-owner <ACCOUNT-ID>` — New owner account identifier
* `--governance-json <PATH>` — Path to `GovernanceHookV1` JSON proving transfer approval



## `iroha_cli app sns update-controllers`

Replace controllers on a SNS name

**Usage:** `iroha_cli app sns update-controllers [OPTIONS] --selector <LABEL.SUFFIX>`

###### **Options:**

* `--selector <LABEL.SUFFIX>` — Selector literal (e.g. `makoto.sora`)
* `--controller <ACCOUNT-ID>` — Replacement controller account identifiers (repeatable). Defaults to `[config account]`



## `iroha_cli app sns freeze`

Freeze a SNS name

**Usage:** `iroha_cli app sns freeze --selector <LABEL.SUFFIX> --reason <TEXT> --until-ms <U64> --guardian-ticket <JSON-OR-STRING>`

###### **Options:**

* `--selector <LABEL.SUFFIX>` — Selector literal (e.g. `makoto.sora`)
* `--reason <TEXT>` — Reason recorded in the freeze log
* `--until-ms <U64>` — Timestamp (ms since epoch) when the freeze should auto-expire
* `--guardian-ticket <JSON-OR-STRING>` — Guardian ticket signature (string or JSON literal)



## `iroha_cli app sns unfreeze`

Unfreeze a SNS name

**Usage:** `iroha_cli app sns unfreeze --selector <LABEL.SUFFIX> --governance-json <PATH>`

###### **Options:**

* `--selector <LABEL.SUFFIX>` — Selector literal (e.g. `makoto.sora`)
* `--governance-json <PATH>` — Path to `GovernanceHookV1` JSON authorising the unfreeze



## `iroha_cli app sns registration`

Fetch a SNS name record

**Usage:** `iroha_cli app sns registration --selector <SELECTOR>`

###### **Options:**

* `--selector <SELECTOR>` — Selector literal (`label.suffix`). IH58 (preferred)/snx1 (second-best) inputs are accepted



## `iroha_cli app sns policy`

Fetch the policy for a suffix

**Usage:** `iroha_cli app sns policy --suffix-id <U16>`

###### **Options:**

* `--suffix-id <U16>` — Numeric suffix identifier (`SuffixPolicyV1::suffix_id`)



## `iroha_cli app sns governance`

Governance helpers (arbitration, transparency exports, etc.)

**Usage:** `iroha_cli app sns governance <COMMAND>`

###### **Subcommands:**

* `case` — Manage arbitration cases referenced by SN-6a



## `iroha_cli app sns governance case`

Manage arbitration cases referenced by SN-6a

**Usage:** `iroha_cli app sns governance case <COMMAND>`

###### **Subcommands:**

* `create` — Validate and submit a dispute case payload
* `export` — Export cases for transparency reporting



## `iroha_cli app sns governance case create`

Validate and submit a dispute case payload

**Usage:** `iroha_cli app sns governance case create [OPTIONS] --case-json <PATH>`

###### **Options:**

* `--case-json <PATH>` — Path to the arbitration case payload (JSON)
* `--schema <PATH>` — Optional path to a JSON schema. Defaults to the embedded SN-6a schema
* `--dry-run` — Validate the payload only; do not submit to Torii



## `iroha_cli app sns governance case export`

Export cases for transparency reporting

**Usage:** `iroha_cli app sns governance case export [OPTIONS]`

###### **Options:**

* `--since <ISO-8601>` — Filter to cases updated after the provided ISO-8601 timestamp
* `--status <STATUS>` — Optional status filter (open, triage, decision, remediation, closed, suspended)
* `--limit <U32>` — Maximum number of cases to return



## `iroha_cli app alias`

Alias helpers (placeholder pipeline)

**Usage:** `iroha_cli app alias <COMMAND>`

###### **Subcommands:**

* `voprf-evaluate` — Evaluate a blinded element using the alias VOPRF service (placeholder)
* `resolve` — Resolve an alias by its canonical name (placeholder)
* `resolve-index` — Resolve an alias by Merkle index (placeholder)



## `iroha_cli app alias voprf-evaluate`

Evaluate a blinded element using the alias VOPRF service (placeholder)

**Usage:** `iroha_cli app alias voprf-evaluate --blinded-element-hex <HEX>`

###### **Options:**

* `--blinded-element-hex <HEX>` — Blinded element in hex encoding



## `iroha_cli app alias resolve`

Resolve an alias by its canonical name (placeholder)

**Usage:** `iroha_cli app alias resolve [OPTIONS] --alias <ALIAS>`

###### **Options:**

* `--alias <ALIAS>` — Alias name to resolve
* `--dry-run` — Print only validation result (skip future network call)

  Default value: `false`



## `iroha_cli app alias resolve-index`

Resolve an alias by Merkle index (placeholder)

**Usage:** `iroha_cli app alias resolve-index --index <INDEX>`

###### **Options:**

* `--index <INDEX>` — Alias Merkle index to resolve



## `iroha_cli app repo`

Repo settlement helpers

**Usage:** `iroha_cli app repo <COMMAND>`

###### **Subcommands:**

* `initiate` — Initiate or roll a repo agreement between two counterparties
* `unwind` — Unwind an active repo agreement (reverse repo leg)
* `query` — Inspect repo agreements stored on-chain
* `margin` — Compute the next margin checkpoint for an agreement
* `margin-call` — Record a margin call for an active repo agreement



## `iroha_cli app repo initiate`

Initiate or roll a repo agreement between two counterparties

**Usage:** `iroha_cli app repo initiate [OPTIONS] --agreement-id <AGREEMENT_ID> --initiator <INITIATOR> --counterparty <COUNTERPARTY> --cash-asset <CASH_ASSET> --cash-quantity <CASH_QUANTITY> --collateral-asset <COLLATERAL_ASSET> --collateral-quantity <COLLATERAL_QUANTITY> --rate-bps <RATE_BPS> --maturity-timestamp-ms <MATURITY_TIMESTAMP_MS> --haircut-bps <HAIRCUT_BPS> --margin-frequency-secs <MARGIN_FREQUENCY_SECS>`

###### **Options:**

* `--agreement-id <AGREEMENT_ID>` — Stable identifier assigned to the repo agreement lifecycle
* `--initiator <INITIATOR>` — Initiating account submitting the repo
* `--counterparty <COUNTERPARTY>` — Counterparty receiving the repo cash leg
* `--custodian <CUSTODIAN>` — Optional custodian account holding pledged collateral in tri-party agreements
* `--cash-asset <CASH_ASSET>` — Cash asset definition identifier
* `--cash-quantity <CASH_QUANTITY>` — Cash quantity exchanged at initiation (integer or decimal)
* `--collateral-asset <COLLATERAL_ASSET>` — Collateral asset definition identifier
* `--collateral-quantity <COLLATERAL_QUANTITY>` — Collateral quantity pledged at initiation (integer or decimal)
* `--rate-bps <RATE_BPS>` — Fixed interest rate in basis points
* `--maturity-timestamp-ms <MATURITY_TIMESTAMP_MS>` — Unix timestamp (milliseconds) when the repo matures
* `--haircut-bps <HAIRCUT_BPS>` — Haircut applied to the collateral leg, in basis points
* `--margin-frequency-secs <MARGIN_FREQUENCY_SECS>` — Cadence between margin checks, in seconds (0 disables margining)



## `iroha_cli app repo unwind`

Unwind an active repo agreement (reverse repo leg)

**Usage:** `iroha_cli app repo unwind --agreement-id <AGREEMENT_ID> --initiator <INITIATOR> --counterparty <COUNTERPARTY> --cash-asset <CASH_ASSET> --cash-quantity <CASH_QUANTITY> --collateral-asset <COLLATERAL_ASSET> --collateral-quantity <COLLATERAL_QUANTITY> --settlement-timestamp-ms <SETTLEMENT_TIMESTAMP_MS>`

###### **Options:**

* `--agreement-id <AGREEMENT_ID>` — Stable identifier assigned to the repo agreement lifecycle
* `--initiator <INITIATOR>` — Initiating account performing the unwind
* `--counterparty <COUNTERPARTY>` — Counterparty receiving the unwind settlement
* `--cash-asset <CASH_ASSET>` — Cash asset definition identifier
* `--cash-quantity <CASH_QUANTITY>` — Cash quantity returned at unwind (integer or decimal)
* `--collateral-asset <COLLATERAL_ASSET>` — Collateral asset definition identifier
* `--collateral-quantity <COLLATERAL_QUANTITY>` — Collateral quantity released at unwind (integer or decimal)
* `--settlement-timestamp-ms <SETTLEMENT_TIMESTAMP_MS>` — Unix timestamp (milliseconds) when the unwind was agreed



## `iroha_cli app repo query`

Inspect repo agreements stored on-chain

**Usage:** `iroha_cli app repo query <COMMAND>`

###### **Subcommands:**

* `list` — List all repo agreements recorded on-chain
* `get` — Fetch a single repo agreement by identifier



## `iroha_cli app repo query list`

List all repo agreements recorded on-chain

**Usage:** `iroha_cli app repo query list`



## `iroha_cli app repo query get`

Fetch a single repo agreement by identifier

**Usage:** `iroha_cli app repo query get --id <ID>`

###### **Options:**

* `--id <ID>` — Stable identifier assigned to the repo agreement lifecycle



## `iroha_cli app repo margin`

Compute the next margin checkpoint for an agreement

**Usage:** `iroha_cli app repo margin [OPTIONS] --agreement-id <AGREEMENT_ID>`

###### **Options:**

* `--agreement-id <AGREEMENT_ID>` — Stable identifier assigned to the repo agreement lifecycle
* `--at-timestamp-ms <AT_TIMESTAMP_MS>` — Timestamp (ms) used when evaluating margin schedule (defaults to current time)



## `iroha_cli app repo margin-call`

Record a margin call for an active repo agreement

**Usage:** `iroha_cli app repo margin-call --agreement-id <AGREEMENT_ID>`

###### **Options:**

* `--agreement-id <AGREEMENT_ID>` — Stable identifier assigned to the repo agreement lifecycle



## `iroha_cli app settlement`

Delivery-versus-payment and payment-versus-payment helpers

**Usage:** `iroha_cli app settlement <COMMAND>`

###### **Subcommands:**

* `dvp` — Create a delivery-versus-payment instruction
* `pvp` — Create a payment-versus-payment instruction



## `iroha_cli app settlement dvp`

Create a delivery-versus-payment instruction

**Usage:** `iroha_cli app settlement dvp [OPTIONS] --settlement-id <SETTLEMENT_ID> --delivery-asset <DELIVERY_ASSET> --delivery-quantity <DELIVERY_QUANTITY> --delivery-from <DELIVERY_FROM> --delivery-to <DELIVERY_TO> --payment-asset <PAYMENT_ASSET> --payment-quantity <PAYMENT_QUANTITY> --payment-from <PAYMENT_FROM> --payment-to <PAYMENT_TO>`

###### **Options:**

* `--settlement-id <SETTLEMENT_ID>` — Stable identifier shared across the settlement lifecycle
* `--delivery-asset <DELIVERY_ASSET>` — Asset definition delivered in exchange
* `--delivery-quantity <DELIVERY_QUANTITY>` — Quantity delivered (integer or decimal)
* `--delivery-from <DELIVERY_FROM>` — Account delivering the asset
* `--delivery-to <DELIVERY_TO>` — Account receiving the delivery leg
* `--delivery-instrument-id <DELIVERY_INSTRUMENT_ID>` — Regulated identifier (ISIN or CUSIP) for the delivery instrument when producing ISO previews
* `--iso-reference-crosswalk <ISO_REFERENCE_CROSSWALK>` — Optional path to an ISIN↔CUSIP crosswalk used to validate `--delivery-instrument-id`
* `--payment-asset <PAYMENT_ASSET>` — Payment asset definition completing the settlement
* `--payment-quantity <PAYMENT_QUANTITY>` — Payment quantity (integer or decimal)
* `--payment-from <PAYMENT_FROM>` — Account sending the payment leg
* `--payment-to <PAYMENT_TO>` — Account receiving the payment leg
* `--order <ORDER>` — Execution order for the two legs

  Default value: `delivery-then-payment`

  Possible values: `delivery-then-payment`, `payment-then-delivery`

* `--atomicity <ATOMICITY>` — Atomicity policy for partial failures (currently only all-or-nothing)

  Default value: `all-or-nothing`

  Possible values: `all-or-nothing`, `commit-first-leg`, `commit-second-leg`

* `--place-of-settlement-mic <PLACE_OF_SETTLEMENT_MIC>` — Optional MIC to emit under PlcOfSttlm/MktId
* `--partial-indicator <PARTIAL_INDICATOR>` — Settlement partial indicator for SttlmParams/PrtlSttlmInd (NPAR/PART/PARQ/PARC)

  Default value: `npar`

  Possible values: `npar`, `part`, `parq`, `parc`

* `--hold-indicator` — Whether to set SttlmParams/HldInd=true in the generated ISO preview
* `--settlement-condition <SETTLEMENT_CONDITION>` — Optional settlement condition code for SttlmParams/SttlmTxCond/Cd
* `--linkage <LINKAGE>` — Optional settlement linkage (TYPE:REFERENCE, TYPE = WITH|BEFO|AFTE). May be repeated
* `--iso-xml-out <ISO_XML_OUT>` — Optional path to emit a sese.023 XML preview of the settlement



## `iroha_cli app settlement pvp`

Create a payment-versus-payment instruction

**Usage:** `iroha_cli app settlement pvp [OPTIONS] --settlement-id <SETTLEMENT_ID> --primary-asset <PRIMARY_ASSET> --primary-quantity <PRIMARY_QUANTITY> --primary-from <PRIMARY_FROM> --primary-to <PRIMARY_TO> --counter-asset <COUNTER_ASSET> --counter-quantity <COUNTER_QUANTITY> --counter-from <COUNTER_FROM> --counter-to <COUNTER_TO>`

###### **Options:**

* `--settlement-id <SETTLEMENT_ID>` — Stable identifier shared across the settlement lifecycle
* `--primary-asset <PRIMARY_ASSET>` — Primary currency leg asset definition
* `--primary-quantity <PRIMARY_QUANTITY>` — Quantity of the primary currency (integer or decimal)
* `--primary-from <PRIMARY_FROM>` — Account delivering the primary currency
* `--primary-to <PRIMARY_TO>` — Account receiving the primary currency
* `--counter-asset <COUNTER_ASSET>` — Counter currency leg asset definition
* `--counter-quantity <COUNTER_QUANTITY>` — Quantity of the counter currency (integer or decimal)
* `--counter-from <COUNTER_FROM>` — Account delivering the counter currency
* `--counter-to <COUNTER_TO>` — Account receiving the counter currency
* `--order <ORDER>` — Execution order for the two legs

  Default value: `delivery-then-payment`

  Possible values: `delivery-then-payment`, `payment-then-delivery`

* `--atomicity <ATOMICITY>` — Atomicity policy for partial failures (currently only all-or-nothing)

  Default value: `all-or-nothing`

  Possible values: `all-or-nothing`, `commit-first-leg`, `commit-second-leg`

* `--place-of-settlement-mic <PLACE_OF_SETTLEMENT_MIC>` — Optional MIC to emit under PlcOfSttlm/MktId
* `--partial-indicator <PARTIAL_INDICATOR>` — Settlement partial indicator for SttlmParams/PrtlSttlmInd (NPAR/PART/PARQ/PARC)

  Default value: `npar`

  Possible values: `npar`, `part`, `parq`, `parc`

* `--hold-indicator` — Whether to set SttlmParams/HldInd=true in the generated ISO preview
* `--settlement-condition <SETTLEMENT_CONDITION>` — Optional settlement condition code for SttlmParams/SttlmTxCond/Cd
* `--iso-xml-out <ISO_XML_OUT>` — Optional path to emit a sese.025 XML preview of the settlement



## `iroha_cli tools`

Developer utilities and diagnostics

**Usage:** `iroha_cli tools <COMMAND>`

###### **Subcommands:**

* `address` — Account address helpers (IH58 (preferred)/snx1 (second-best) conversions)
* `crypto` — Cryptography helpers (SM2/SM3/SM4)
* `ivm` — IVM/ABI helpers (e.g., compute ABI hash)
* `markdown-help` — Output CLI documentation in Markdown format
* `version` — Show versions and git SHA of client and server



## `iroha_cli tools address`

Account address helpers (IH58 (preferred)/snx1 (second-best) conversions)

**Usage:** `iroha_cli tools address <COMMAND>`

###### **Subcommands:**

* `convert` — Convert account addresses between supported textual encodings
* `audit` — Scan a list of addresses and emit conversion summaries
* `normalize` — Rewrite newline-separated addresses into canonical encodings



## `iroha_cli tools address convert`

Convert account addresses between supported textual encodings

**Usage:** `iroha_cli tools address convert [OPTIONS] <ADDRESS>`

###### **Arguments:**

* `<ADDRESS>` — Address literal to parse (IH58, `snx1…`, or canonical `0x…`)

###### **Options:**

* `--expect-prefix <PREFIX>` — Require IH58 inputs to match the provided network prefix
* `--network-prefix <PREFIX>` — Network prefix to use when emitting IH58 output

  Default value: `753`
* `--format <FORMAT>` — Desired output format (defaults to IH58)

  Default value: `ih58`

  Possible values: `ih58`, `compressed`, `canonical-hex`, `json`

* `--append-domain` — Append the provided domain to the output (requires `<address>@<domain>` input)



## `iroha_cli tools address audit`

Scan a list of addresses and emit conversion summaries

**Usage:** `iroha_cli tools address audit [OPTIONS]`

###### **Options:**

* `--input <PATH>` — Path to a file containing newline-separated addresses (defaults to STDIN)
* `--expect-prefix <PREFIX>` — Require IH58 inputs to match the provided network prefix
* `--network-prefix <PREFIX>` — Network prefix to use when emitting IH58 output

  Default value: `753`
* `--fail-on-warning` — Return a non-zero status code when Local-domain selectors are detected
* `--allow-errors` — Succeed even if parse errors were encountered (allow auditing large dumps)
* `--format <FORMAT>` — Output format (`json` for structured reports, `csv` for spreadsheet ingestion)

  Default value: `json`

  Possible values: `json`, `csv`




## `iroha_cli tools address normalize`

Rewrite newline-separated addresses into canonical encodings

**Usage:** `iroha_cli tools address normalize [OPTIONS]`

###### **Options:**

* `--input <PATH>` — Path to a file containing newline-separated addresses (defaults to STDIN)
* `--output <PATH>` — Write the converted addresses to a file (defaults to STDOUT)
* `--expect-prefix <PREFIX>` — Require IH58 inputs to match the provided network prefix
* `--network-prefix <PREFIX>` — Network prefix to use when emitting IH58 output

  Default value: `753`
* `--format <FORMAT>` — Desired output format (defaults to IH58)

  Default value: `ih58`

  Possible values: `ih58`, `compressed`, `canonical-hex`, `json`

* `--append-domain` — Append the provided domain to the output (requires `<address>@<domain>` input)
* `--only-local` — Only emit conversions for Local-domain selectors
* `--allow-errors` — Succeed even if parse errors were encountered (allow auditing large dumps)



## `iroha_cli tools crypto`

Cryptography helpers (SM2/SM3/SM4)

**Usage:** `iroha_cli tools crypto <COMMAND>`

###### **Subcommands:**

* `sm2` — SM2 key management helpers
* `sm3` — SM3 hashing helpers
* `sm4` — SM4 AEAD helpers (GCM/CCM modes)



## `iroha_cli tools crypto sm2`

SM2 key management helpers

**Usage:** `iroha_cli tools crypto sm2 <COMMAND>`

###### **Subcommands:**

* `keygen` — Generate a new SM2 key pair (distinguishing ID aware)
* `import` — Import an existing SM2 private key and derive metadata
* `export` — Export SM2 key material with config snippets



## `iroha_cli tools crypto sm2 keygen`

Generate a new SM2 key pair (distinguishing ID aware)

**Usage:** `iroha_cli tools crypto sm2 keygen [OPTIONS]`

###### **Options:**

* `--distid <DISTID>` — Distinguishing identifier embedded into SM2 signatures (defaults to `1234567812345678`)
* `--seed-hex <HEX>` — Optional seed (hex) for deterministic key generation. Helpful for tests/backups
* `--output <PATH>` — Write the generated JSON payload to a file instead of stdout
* `--quiet` — Suppress stdout printing of the JSON payload



## `iroha_cli tools crypto sm2 import`

Import an existing SM2 private key and derive metadata

**Usage:** `iroha_cli tools crypto sm2 import [OPTIONS]`

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



## `iroha_cli tools crypto sm2 export`

Export SM2 key material with config snippets

**Usage:** `iroha_cli tools crypto sm2 export [OPTIONS]`

###### **Options:**

* `--private-key-hex <HEX>` — Existing SM2 private key in hex (32 bytes)
* `--private-key-file <PATH>` — Path to a file containing a hex-encoded SM2 private key (32 bytes)
* `--private-key-pem <PEM>` — PKCS#8 PEM-encoded SM2 private key
* `--private-key-pem-file <PATH>` — Path to a PKCS#8 PEM SM2 private key
* `--distid <DISTID>` — Distinguishing identifier used by the signer (defaults to `1234567812345678`)
* `--snippet-output <PATH>` — Write the TOML snippet to a file
* `--emit-json` — Emit the JSON key material alongside the config snippet
* `--quiet` — Suppress stdout output



## `iroha_cli tools crypto sm3`

SM3 hashing helpers

**Usage:** `iroha_cli tools crypto sm3 <COMMAND>`

###### **Subcommands:**

* `hash` — Hash input data with SM3



## `iroha_cli tools crypto sm3 hash`

Hash input data with SM3

**Usage:** `iroha_cli tools crypto sm3 hash [OPTIONS]`

###### **Options:**

* `--data <STRING>` — UTF-8 string to hash (mutually exclusive with other inputs)
* `--data-hex <HEX>` — Raw bytes to hash provided as hex
* `--file <PATH>` — Path to a file whose contents will be hashed
* `--output <PATH>` — Write the digest JSON to a file
* `--quiet` — Suppress stdout printing of the digest JSON



## `iroha_cli tools crypto sm4`

SM4 AEAD helpers (GCM/CCM modes)

**Usage:** `iroha_cli tools crypto sm4 <COMMAND>`

###### **Subcommands:**

* `gcm-seal` — Encrypt data with SM4-GCM
* `gcm-open` — Decrypt data with SM4-GCM
* `ccm-seal` — Encrypt data with SM4-CCM
* `ccm-open` — Decrypt data with SM4-CCM



## `iroha_cli tools crypto sm4 gcm-seal`

Encrypt data with SM4-GCM

**Usage:** `iroha_cli tools crypto sm4 gcm-seal [OPTIONS] --key-hex <HEX32> --nonce-hex <HEX24>`

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



## `iroha_cli tools crypto sm4 gcm-open`

Decrypt data with SM4-GCM

**Usage:** `iroha_cli tools crypto sm4 gcm-open [OPTIONS] --key-hex <HEX32> --nonce-hex <HEX24>`

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



## `iroha_cli tools crypto sm4 ccm-seal`

Encrypt data with SM4-CCM

**Usage:** `iroha_cli tools crypto sm4 ccm-seal [OPTIONS] --key-hex <HEX32> --nonce-hex <HEX14-26>`

###### **Options:**

* `--key-hex <HEX32>` — SM4 key (16 bytes hex)
* `--nonce-hex <HEX14-26>` — CCM nonce (7–13 bytes hex)
* `--aad-hex <HEX>` — Additional authenticated data (hex, optional)

  Default value: ``
* `--plaintext-hex <HEX>` — Plaintext to encrypt (hex, mutually exclusive with file)
* `--plaintext-file <PATH>` — Path to plaintext bytes to encrypt
* `--tag-len <BYTES>` — CCM authentication tag length (bytes). Supported: 4,6,8,10,12,14,16. Defaults to 16

  Default value: `16`
* `--ciphertext-file <PATH>` — Write the ciphertext bytes to a file
* `--tag-file <PATH>` — Write the authentication tag bytes to a file
* `--quiet` — Suppress stdout JSON output



## `iroha_cli tools crypto sm4 ccm-open`

Decrypt data with SM4-CCM

**Usage:** `iroha_cli tools crypto sm4 ccm-open [OPTIONS] --key-hex <HEX32> --nonce-hex <HEX14-26>`

###### **Options:**

* `--key-hex <HEX32>` — SM4 key (16 bytes hex)
* `--nonce-hex <HEX14-26>` — CCM nonce (7–13 bytes hex)
* `--aad-hex <HEX>` — Additional authenticated data (hex, optional)

  Default value: ``
* `--ciphertext-hex <HEX>` — Ciphertext to decrypt (hex, mutually exclusive with file)
* `--ciphertext-file <PATH>` — Path to ciphertext bytes
* `--tag-hex <HEX>` — Authentication tag (hex, mutually exclusive with file)
* `--tag-file <PATH>` — Path to authentication tag bytes
* `--tag-len <BYTES>` — Expected CCM tag length (bytes). If omitted, inferred from the tag input
* `--plaintext-file <PATH>` — Write the decrypted plaintext to a file
* `--quiet` — Suppress stdout JSON output



## `iroha_cli tools ivm`

IVM/ABI helpers (e.g., compute ABI hash)

**Usage:** `iroha_cli tools ivm <COMMAND>`

###### **Subcommands:**

* `abi-hash` — Print the current ABI hash for a given policy (default: v1)
* `syscalls` — Print the canonical syscall list (min or markdown table)
* `manifest-gen` — Generate a minimal manifest (`code_hash` + `abi_hash`) from a compiled .to file



## `iroha_cli tools ivm abi-hash`

Print the current ABI hash for a given policy (default: v1)

**Usage:** `iroha_cli tools ivm abi-hash [OPTIONS]`

###### **Options:**

* `--policy <POLICY>` — Policy: v1

  Default value: `v1`
* `--uppercase` — Uppercase hex output (default: lowercase)



## `iroha_cli tools ivm syscalls`

Print the canonical syscall list (min or markdown table)

**Usage:** `iroha_cli tools ivm syscalls [OPTIONS]`

###### **Options:**

* `--format <FORMAT>` — Output format: 'min' (one per line) or 'markdown'

  Default value: `min`



## `iroha_cli tools ivm manifest-gen`

Generate a minimal manifest (`code_hash` + `abi_hash`) from a compiled .to file

**Usage:** `iroha_cli tools ivm manifest-gen --file <PATH>`

###### **Options:**

* `--file <PATH>` — Path to compiled IVM bytecode (.to)



## `iroha_cli tools markdown-help`

Output CLI documentation in Markdown format

**Usage:** `iroha_cli tools markdown-help`



## `iroha_cli tools version`

Show versions and git SHA of client and server

**Usage:** `iroha_cli tools version`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

