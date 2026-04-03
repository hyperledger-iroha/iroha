# Command-Line Help for `iroha`

This document contains the help content for the `iroha` command-line program.

**Command Overview:**

* [`iroha`↴](#iroha)
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
* [`iroha tx`↴](#iroha-tx)
* [`iroha tx status`↴](#iroha-tx-status)
* [`iroha tx get`↴](#iroha-tx-get)
* [`iroha tx ping`↴](#iroha-tx-ping)
* [`iroha tx ivm`↴](#iroha-tx-ivm)
* [`iroha tx stdin`↴](#iroha-tx-stdin)
* [`iroha ledger`↴](#iroha-ledger)
* [`iroha ledger domain`↴](#iroha-ledger-domain)
* [`iroha ledger domain list`↴](#iroha-ledger-domain-list)
* [`iroha ledger domain list all`↴](#iroha-ledger-domain-list-all)
* [`iroha ledger domain list filter`↴](#iroha-ledger-domain-list-filter)
* [`iroha ledger domain get`↴](#iroha-ledger-domain-get)
* [`iroha ledger domain register`↴](#iroha-ledger-domain-register)
* [`iroha ledger domain unregister`↴](#iroha-ledger-domain-unregister)
* [`iroha ledger domain transfer`↴](#iroha-ledger-domain-transfer)
* [`iroha ledger domain meta`↴](#iroha-ledger-domain-meta)
* [`iroha ledger domain meta get`↴](#iroha-ledger-domain-meta-get)
* [`iroha ledger domain meta set`↴](#iroha-ledger-domain-meta-set)
* [`iroha ledger domain meta remove`↴](#iroha-ledger-domain-meta-remove)
* [`iroha ledger account`↴](#iroha-ledger-account)
* [`iroha ledger account role`↴](#iroha-ledger-account-role)
* [`iroha ledger account role list`↴](#iroha-ledger-account-role-list)
* [`iroha ledger account role grant`↴](#iroha-ledger-account-role-grant)
* [`iroha ledger account role revoke`↴](#iroha-ledger-account-role-revoke)
* [`iroha ledger account permission`↴](#iroha-ledger-account-permission)
* [`iroha ledger account permission list`↴](#iroha-ledger-account-permission-list)
* [`iroha ledger account permission grant`↴](#iroha-ledger-account-permission-grant)
* [`iroha ledger account permission revoke`↴](#iroha-ledger-account-permission-revoke)
* [`iroha ledger account list`↴](#iroha-ledger-account-list)
* [`iroha ledger account list all`↴](#iroha-ledger-account-list-all)
* [`iroha ledger account list filter`↴](#iroha-ledger-account-list-filter)
* [`iroha ledger account get`↴](#iroha-ledger-account-get)
* [`iroha ledger account register`↴](#iroha-ledger-account-register)
* [`iroha ledger account unregister`↴](#iroha-ledger-account-unregister)
* [`iroha ledger account meta`↴](#iroha-ledger-account-meta)
* [`iroha ledger account meta get`↴](#iroha-ledger-account-meta-get)
* [`iroha ledger account meta set`↴](#iroha-ledger-account-meta-set)
* [`iroha ledger account meta remove`↴](#iroha-ledger-account-meta-remove)
* [`iroha ledger asset`↴](#iroha-ledger-asset)
* [`iroha ledger asset definition`↴](#iroha-ledger-asset-definition)
* [`iroha ledger asset definition list`↴](#iroha-ledger-asset-definition-list)
* [`iroha ledger asset definition list all`↴](#iroha-ledger-asset-definition-list-all)
* [`iroha ledger asset definition list filter`↴](#iroha-ledger-asset-definition-list-filter)
* [`iroha ledger asset definition get`↴](#iroha-ledger-asset-definition-get)
* [`iroha ledger asset definition register`↴](#iroha-ledger-asset-definition-register)
* [`iroha ledger asset definition unregister`↴](#iroha-ledger-asset-definition-unregister)
* [`iroha ledger asset definition transfer`↴](#iroha-ledger-asset-definition-transfer)
* [`iroha ledger asset definition meta`↴](#iroha-ledger-asset-definition-meta)
* [`iroha ledger asset definition meta get`↴](#iroha-ledger-asset-definition-meta-get)
* [`iroha ledger asset definition meta set`↴](#iroha-ledger-asset-definition-meta-set)
* [`iroha ledger asset definition meta remove`↴](#iroha-ledger-asset-definition-meta-remove)
* [`iroha ledger asset get`↴](#iroha-ledger-asset-get)
* [`iroha ledger asset list`↴](#iroha-ledger-asset-list)
* [`iroha ledger asset list all`↴](#iroha-ledger-asset-list-all)
* [`iroha ledger asset list filter`↴](#iroha-ledger-asset-list-filter)
* [`iroha ledger asset mint`↴](#iroha-ledger-asset-mint)
* [`iroha ledger asset burn`↴](#iroha-ledger-asset-burn)
* [`iroha ledger asset transfer`↴](#iroha-ledger-asset-transfer)
* [`iroha ledger nft`↴](#iroha-ledger-nft)
* [`iroha ledger nft get`↴](#iroha-ledger-nft-get)
* [`iroha ledger nft list`↴](#iroha-ledger-nft-list)
* [`iroha ledger nft list all`↴](#iroha-ledger-nft-list-all)
* [`iroha ledger nft list filter`↴](#iroha-ledger-nft-list-filter)
* [`iroha ledger nft register`↴](#iroha-ledger-nft-register)
* [`iroha ledger nft unregister`↴](#iroha-ledger-nft-unregister)
* [`iroha ledger nft transfer`↴](#iroha-ledger-nft-transfer)
* [`iroha ledger nft meta`↴](#iroha-ledger-nft-meta)
* [`iroha ledger nft meta get`↴](#iroha-ledger-nft-meta-get)
* [`iroha ledger nft meta set`↴](#iroha-ledger-nft-meta-set)
* [`iroha ledger nft meta remove`↴](#iroha-ledger-nft-meta-remove)
* [`iroha ledger rwa`↴](#iroha-ledger-rwa)
* [`iroha ledger rwa get`↴](#iroha-ledger-rwa-get)
* [`iroha ledger rwa list`↴](#iroha-ledger-rwa-list)
* [`iroha ledger rwa list all`↴](#iroha-ledger-rwa-list-all)
* [`iroha ledger rwa list filter`↴](#iroha-ledger-rwa-list-filter)
* [`iroha ledger rwa register`↴](#iroha-ledger-rwa-register)
* [`iroha ledger rwa transfer`↴](#iroha-ledger-rwa-transfer)
* [`iroha ledger rwa merge`↴](#iroha-ledger-rwa-merge)
* [`iroha ledger rwa redeem`↴](#iroha-ledger-rwa-redeem)
* [`iroha ledger rwa freeze`↴](#iroha-ledger-rwa-freeze)
* [`iroha ledger rwa unfreeze`↴](#iroha-ledger-rwa-unfreeze)
* [`iroha ledger rwa hold`↴](#iroha-ledger-rwa-hold)
* [`iroha ledger rwa release`↴](#iroha-ledger-rwa-release)
* [`iroha ledger rwa force-transfer`↴](#iroha-ledger-rwa-force-transfer)
* [`iroha ledger rwa set-controls`↴](#iroha-ledger-rwa-set-controls)
* [`iroha ledger rwa meta`↴](#iroha-ledger-rwa-meta)
* [`iroha ledger rwa meta get`↴](#iroha-ledger-rwa-meta-get)
* [`iroha ledger rwa meta set`↴](#iroha-ledger-rwa-meta-set)
* [`iroha ledger rwa meta remove`↴](#iroha-ledger-rwa-meta-remove)
* [`iroha ledger peer`↴](#iroha-ledger-peer)
* [`iroha ledger peer list`↴](#iroha-ledger-peer-list)
* [`iroha ledger peer list all`↴](#iroha-ledger-peer-list-all)
* [`iroha ledger peer register`↴](#iroha-ledger-peer-register)
* [`iroha ledger peer unregister`↴](#iroha-ledger-peer-unregister)
* [`iroha ledger role`↴](#iroha-ledger-role)
* [`iroha ledger role permission`↴](#iroha-ledger-role-permission)
* [`iroha ledger role permission list`↴](#iroha-ledger-role-permission-list)
* [`iroha ledger role permission grant`↴](#iroha-ledger-role-permission-grant)
* [`iroha ledger role permission revoke`↴](#iroha-ledger-role-permission-revoke)
* [`iroha ledger role list`↴](#iroha-ledger-role-list)
* [`iroha ledger role list all`↴](#iroha-ledger-role-list-all)
* [`iroha ledger role register`↴](#iroha-ledger-role-register)
* [`iroha ledger role unregister`↴](#iroha-ledger-role-unregister)
* [`iroha ledger parameter`↴](#iroha-ledger-parameter)
* [`iroha ledger parameter list`↴](#iroha-ledger-parameter-list)
* [`iroha ledger parameter list all`↴](#iroha-ledger-parameter-list-all)
* [`iroha ledger parameter set`↴](#iroha-ledger-parameter-set)
* [`iroha ledger trigger`↴](#iroha-ledger-trigger)
* [`iroha ledger trigger list`↴](#iroha-ledger-trigger-list)
* [`iroha ledger trigger list all`↴](#iroha-ledger-trigger-list-all)
* [`iroha ledger trigger get`↴](#iroha-ledger-trigger-get)
* [`iroha ledger trigger register`↴](#iroha-ledger-trigger-register)
* [`iroha ledger trigger unregister`↴](#iroha-ledger-trigger-unregister)
* [`iroha ledger trigger mint`↴](#iroha-ledger-trigger-mint)
* [`iroha ledger trigger burn`↴](#iroha-ledger-trigger-burn)
* [`iroha ledger trigger meta`↴](#iroha-ledger-trigger-meta)
* [`iroha ledger trigger meta get`↴](#iroha-ledger-trigger-meta-get)
* [`iroha ledger trigger meta set`↴](#iroha-ledger-trigger-meta-set)
* [`iroha ledger trigger meta remove`↴](#iroha-ledger-trigger-meta-remove)
* [`iroha ledger query`↴](#iroha-ledger-query)
* [`iroha ledger query stdin`↴](#iroha-ledger-query-stdin)
* [`iroha ledger query stdin-raw`↴](#iroha-ledger-query-stdin-raw)
* [`iroha ledger transaction`↴](#iroha-ledger-transaction)
* [`iroha ledger transaction status`↴](#iroha-ledger-transaction-status)
* [`iroha ledger transaction get`↴](#iroha-ledger-transaction-get)
* [`iroha ledger transaction ping`↴](#iroha-ledger-transaction-ping)
* [`iroha ledger transaction ivm`↴](#iroha-ledger-transaction-ivm)
* [`iroha ledger transaction stdin`↴](#iroha-ledger-transaction-stdin)
* [`iroha ledger multisig`↴](#iroha-ledger-multisig)
* [`iroha ledger multisig list`↴](#iroha-ledger-multisig-list)
* [`iroha ledger multisig list all`↴](#iroha-ledger-multisig-list-all)
* [`iroha ledger multisig register`↴](#iroha-ledger-multisig-register)
* [`iroha ledger multisig propose`↴](#iroha-ledger-multisig-propose)
* [`iroha ledger multisig approve`↴](#iroha-ledger-multisig-approve)
* [`iroha ledger multisig cancel`↴](#iroha-ledger-multisig-cancel)
* [`iroha ledger multisig inspect`↴](#iroha-ledger-multisig-inspect)
* [`iroha ledger events`↴](#iroha-ledger-events)
* [`iroha ledger events state`↴](#iroha-ledger-events-state)
* [`iroha ledger events governance`↴](#iroha-ledger-events-governance)
* [`iroha ledger events transaction`↴](#iroha-ledger-events-transaction)
* [`iroha ledger events block`↴](#iroha-ledger-events-block)
* [`iroha ledger events trigger-execute`↴](#iroha-ledger-events-trigger-execute)
* [`iroha ledger events trigger-complete`↴](#iroha-ledger-events-trigger-complete)
* [`iroha ledger blocks`↴](#iroha-ledger-blocks)
* [`iroha ops`↴](#iroha-ops)
* [`iroha ops offline`↴](#iroha-ops-offline)
* [`iroha ops offline allowance`↴](#iroha-ops-offline-allowance)
* [`iroha ops offline allowance list`↴](#iroha-ops-offline-allowance-list)
* [`iroha ops offline allowance get`↴](#iroha-ops-offline-allowance-get)
* [`iroha ops offline transfer`↴](#iroha-ops-offline-transfer)
* [`iroha ops offline transfer list`↴](#iroha-ops-offline-transfer-list)
* [`iroha ops offline transfer get`↴](#iroha-ops-offline-transfer-get)
* [`iroha ops offline transfer proof`↴](#iroha-ops-offline-transfer-proof)
* [`iroha ops offline bundle`↴](#iroha-ops-offline-bundle)
* [`iroha ops offline bundle inspect`↴](#iroha-ops-offline-bundle-inspect)
* [`iroha ops offline summary`↴](#iroha-ops-offline-summary)
* [`iroha ops offline summary list`↴](#iroha-ops-offline-summary-list)
* [`iroha ops offline summary export`↴](#iroha-ops-offline-summary-export)
* [`iroha ops offline revocation`↴](#iroha-ops-offline-revocation)
* [`iroha ops offline revocation list`↴](#iroha-ops-offline-revocation-list)
* [`iroha ops offline rejection`↴](#iroha-ops-offline-rejection)
* [`iroha ops offline rejection stats`↴](#iroha-ops-offline-rejection-stats)
* [`iroha ops offline qr`↴](#iroha-ops-offline-qr)
* [`iroha ops offline qr encode`↴](#iroha-ops-offline-qr-encode)
* [`iroha ops offline qr decode`↴](#iroha-ops-offline-qr-decode)
* [`iroha ops offline petal`↴](#iroha-ops-offline-petal)
* [`iroha ops offline petal encode`↴](#iroha-ops-offline-petal-encode)
* [`iroha ops offline petal decode`↴](#iroha-ops-offline-petal-decode)
* [`iroha ops offline petal eval-capture`↴](#iroha-ops-offline-petal-eval-capture)
* [`iroha ops offline petal simulate-realtime`↴](#iroha-ops-offline-petal-simulate-realtime)
* [`iroha ops offline petal score-styles`↴](#iroha-ops-offline-petal-score-styles)
* [`iroha ops executor`↴](#iroha-ops-executor)
* [`iroha ops executor data-model`↴](#iroha-ops-executor-data-model)
* [`iroha ops executor upgrade`↴](#iroha-ops-executor-upgrade)
* [`iroha ops runtime`↴](#iroha-ops-runtime)
* [`iroha ops runtime abi`↴](#iroha-ops-runtime-abi)
* [`iroha ops runtime abi active`↴](#iroha-ops-runtime-abi-active)
* [`iroha ops runtime abi active-query`↴](#iroha-ops-runtime-abi-active-query)
* [`iroha ops runtime abi hash`↴](#iroha-ops-runtime-abi-hash)
* [`iroha ops runtime upgrade`↴](#iroha-ops-runtime-upgrade)
* [`iroha ops runtime upgrade list`↴](#iroha-ops-runtime-upgrade-list)
* [`iroha ops runtime upgrade propose`↴](#iroha-ops-runtime-upgrade-propose)
* [`iroha ops runtime upgrade activate`↴](#iroha-ops-runtime-upgrade-activate)
* [`iroha ops runtime upgrade cancel`↴](#iroha-ops-runtime-upgrade-cancel)
* [`iroha ops runtime status`↴](#iroha-ops-runtime-status)
* [`iroha ops runtime capabilities`↴](#iroha-ops-runtime-capabilities)
* [`iroha ops sumeragi`↴](#iroha-ops-sumeragi)
* [`iroha ops sumeragi status`↴](#iroha-ops-sumeragi-status)
* [`iroha ops sumeragi leader`↴](#iroha-ops-sumeragi-leader)
* [`iroha ops sumeragi params`↴](#iroha-ops-sumeragi-params)
* [`iroha ops sumeragi collectors`↴](#iroha-ops-sumeragi-collectors)
* [`iroha ops sumeragi qc`↴](#iroha-ops-sumeragi-qc)
* [`iroha ops sumeragi pacemaker`↴](#iroha-ops-sumeragi-pacemaker)
* [`iroha ops sumeragi phases`↴](#iroha-ops-sumeragi-phases)
* [`iroha ops sumeragi telemetry`↴](#iroha-ops-sumeragi-telemetry)
* [`iroha ops sumeragi evidence`↴](#iroha-ops-sumeragi-evidence)
* [`iroha ops sumeragi evidence list`↴](#iroha-ops-sumeragi-evidence-list)
* [`iroha ops sumeragi evidence count`↴](#iroha-ops-sumeragi-evidence-count)
* [`iroha ops sumeragi evidence submit`↴](#iroha-ops-sumeragi-evidence-submit)
* [`iroha ops sumeragi rbc`↴](#iroha-ops-sumeragi-rbc)
* [`iroha ops sumeragi rbc status`↴](#iroha-ops-sumeragi-rbc-status)
* [`iroha ops sumeragi rbc sessions`↴](#iroha-ops-sumeragi-rbc-sessions)
* [`iroha ops sumeragi vrf-penalties`↴](#iroha-ops-sumeragi-vrf-penalties)
* [`iroha ops sumeragi vrf-epoch`↴](#iroha-ops-sumeragi-vrf-epoch)
* [`iroha ops sumeragi commit-qc`↴](#iroha-ops-sumeragi-commit-qc)
* [`iroha ops sumeragi commit-qc get`↴](#iroha-ops-sumeragi-commit-qc-get)
* [`iroha ops audit`↴](#iroha-ops-audit)
* [`iroha ops audit witness`↴](#iroha-ops-audit-witness)
* [`iroha ops connect`↴](#iroha-ops-connect)
* [`iroha ops connect queue`↴](#iroha-ops-connect-queue)
* [`iroha ops connect queue inspect`↴](#iroha-ops-connect-queue-inspect)
* [`iroha offline`↴](#iroha-offline)
* [`iroha offline allowance`↴](#iroha-offline-allowance)
* [`iroha offline allowance list`↴](#iroha-offline-allowance-list)
* [`iroha offline allowance get`↴](#iroha-offline-allowance-get)
* [`iroha offline transfer`↴](#iroha-offline-transfer)
* [`iroha offline transfer list`↴](#iroha-offline-transfer-list)
* [`iroha offline transfer get`↴](#iroha-offline-transfer-get)
* [`iroha offline transfer proof`↴](#iroha-offline-transfer-proof)
* [`iroha offline bundle`↴](#iroha-offline-bundle)
* [`iroha offline bundle inspect`↴](#iroha-offline-bundle-inspect)
* [`iroha offline summary`↴](#iroha-offline-summary)
* [`iroha offline summary list`↴](#iroha-offline-summary-list)
* [`iroha offline summary export`↴](#iroha-offline-summary-export)
* [`iroha offline revocation`↴](#iroha-offline-revocation)
* [`iroha offline revocation list`↴](#iroha-offline-revocation-list)
* [`iroha offline rejection`↴](#iroha-offline-rejection)
* [`iroha offline rejection stats`↴](#iroha-offline-rejection-stats)
* [`iroha offline qr`↴](#iroha-offline-qr)
* [`iroha offline qr encode`↴](#iroha-offline-qr-encode)
* [`iroha offline qr decode`↴](#iroha-offline-qr-decode)
* [`iroha offline petal`↴](#iroha-offline-petal)
* [`iroha offline petal encode`↴](#iroha-offline-petal-encode)
* [`iroha offline petal decode`↴](#iroha-offline-petal-decode)
* [`iroha offline petal eval-capture`↴](#iroha-offline-petal-eval-capture)
* [`iroha offline petal simulate-realtime`↴](#iroha-offline-petal-simulate-realtime)
* [`iroha offline petal score-styles`↴](#iroha-offline-petal-score-styles)
* [`iroha app`↴](#iroha-app)
* [`iroha app gov`↴](#iroha-app-gov)
* [`iroha app gov deploy`↴](#iroha-app-gov-deploy)
* [`iroha app gov deploy propose`↴](#iroha-app-gov-deploy-propose)
* [`iroha app gov deploy meta`↴](#iroha-app-gov-deploy-meta)
* [`iroha app gov deploy audit`↴](#iroha-app-gov-deploy-audit)
* [`iroha app gov vote`↴](#iroha-app-gov-vote)
* [`iroha app gov proposal`↴](#iroha-app-gov-proposal)
* [`iroha app gov proposal get`↴](#iroha-app-gov-proposal-get)
* [`iroha app gov locks`↴](#iroha-app-gov-locks)
* [`iroha app gov locks get`↴](#iroha-app-gov-locks-get)
* [`iroha app gov council`↴](#iroha-app-gov-council)
* [`iroha app gov council derive-vrf`↴](#iroha-app-gov-council-derive-vrf)
* [`iroha app gov council persist`↴](#iroha-app-gov-council-persist)
* [`iroha app gov council gen-vrf`↴](#iroha-app-gov-council-gen-vrf)
* [`iroha app gov council derive-and-persist`↴](#iroha-app-gov-council-derive-and-persist)
* [`iroha app gov council replace`↴](#iroha-app-gov-council-replace)
* [`iroha app gov unlock`↴](#iroha-app-gov-unlock)
* [`iroha app gov unlock stats`↴](#iroha-app-gov-unlock-stats)
* [`iroha app gov referendum`↴](#iroha-app-gov-referendum)
* [`iroha app gov referendum get`↴](#iroha-app-gov-referendum-get)
* [`iroha app gov tally`↴](#iroha-app-gov-tally)
* [`iroha app gov tally get`↴](#iroha-app-gov-tally-get)
* [`iroha app gov finalize`↴](#iroha-app-gov-finalize)
* [`iroha app gov enact`↴](#iroha-app-gov-enact)
* [`iroha app gov protected`↴](#iroha-app-gov-protected)
* [`iroha app gov protected set`↴](#iroha-app-gov-protected-set)
* [`iroha app gov protected apply`↴](#iroha-app-gov-protected-apply)
* [`iroha app gov protected get`↴](#iroha-app-gov-protected-get)
* [`iroha app contracts`↴](#iroha-app-contracts)
* [`iroha app contracts code`↴](#iroha-app-contracts-code)
* [`iroha app contracts code get`↴](#iroha-app-contracts-code-get)
* [`iroha app contracts alias`↴](#iroha-app-contracts-alias)
* [`iroha app contracts alias lease`↴](#iroha-app-contracts-alias-lease)
* [`iroha app contracts alias release`↴](#iroha-app-contracts-alias-release)
* [`iroha app contracts alias resolve`↴](#iroha-app-contracts-alias-resolve)
* [`iroha app contracts deploy`↴](#iroha-app-contracts-deploy)
* [`iroha app contracts derive-address`↴](#iroha-app-contracts-derive-address)
* [`iroha app contracts call`↴](#iroha-app-contracts-call)
* [`iroha app contracts view`↴](#iroha-app-contracts-view)
* [`iroha app contracts debug-view`↴](#iroha-app-contracts-debug-view)
* [`iroha app contracts debug-call`↴](#iroha-app-contracts-debug-call)
* [`iroha app contracts manifest`↴](#iroha-app-contracts-manifest)
* [`iroha app contracts manifest get`↴](#iroha-app-contracts-manifest-get)
* [`iroha app contracts manifest build`↴](#iroha-app-contracts-manifest-build)
* [`iroha app contracts simulate`↴](#iroha-app-contracts-simulate)
* [`iroha app zk`↴](#iroha-app-zk)
* [`iroha app zk roots`↴](#iroha-app-zk-roots)
* [`iroha app zk verify`↴](#iroha-app-zk-verify)
* [`iroha app zk submit-proof`↴](#iroha-app-zk-submit-proof)
* [`iroha app zk verify-batch`↴](#iroha-app-zk-verify-batch)
* [`iroha app zk schema-hash`↴](#iroha-app-zk-schema-hash)
* [`iroha app zk attachments`↴](#iroha-app-zk-attachments)
* [`iroha app zk attachments upload`↴](#iroha-app-zk-attachments-upload)
* [`iroha app zk attachments list`↴](#iroha-app-zk-attachments-list)
* [`iroha app zk attachments get`↴](#iroha-app-zk-attachments-get)
* [`iroha app zk attachments delete`↴](#iroha-app-zk-attachments-delete)
* [`iroha app zk attachments cleanup`↴](#iroha-app-zk-attachments-cleanup)
* [`iroha app zk register-asset`↴](#iroha-app-zk-register-asset)
* [`iroha app zk shield`↴](#iroha-app-zk-shield)
* [`iroha app zk unshield`↴](#iroha-app-zk-unshield)
* [`iroha app zk vk`↴](#iroha-app-zk-vk)
* [`iroha app zk vk register`↴](#iroha-app-zk-vk-register)
* [`iroha app zk vk update`↴](#iroha-app-zk-vk-update)
* [`iroha app zk vk get`↴](#iroha-app-zk-vk-get)
* [`iroha app zk proofs`↴](#iroha-app-zk-proofs)
* [`iroha app zk proofs list`↴](#iroha-app-zk-proofs-list)
* [`iroha app zk proofs count`↴](#iroha-app-zk-proofs-count)
* [`iroha app zk proofs get`↴](#iroha-app-zk-proofs-get)
* [`iroha app zk proofs retention`↴](#iroha-app-zk-proofs-retention)
* [`iroha app zk proofs prune`↴](#iroha-app-zk-proofs-prune)
* [`iroha app zk prover`↴](#iroha-app-zk-prover)
* [`iroha app zk prover reports`↴](#iroha-app-zk-prover-reports)
* [`iroha app zk prover reports list`↴](#iroha-app-zk-prover-reports-list)
* [`iroha app zk prover reports get`↴](#iroha-app-zk-prover-reports-get)
* [`iroha app zk prover reports delete`↴](#iroha-app-zk-prover-reports-delete)
* [`iroha app zk prover reports cleanup`↴](#iroha-app-zk-prover-reports-cleanup)
* [`iroha app zk prover reports count`↴](#iroha-app-zk-prover-reports-count)
* [`iroha app zk ivm`↴](#iroha-app-zk-ivm)
* [`iroha app zk ivm derive`↴](#iroha-app-zk-ivm-derive)
* [`iroha app zk ivm prove`↴](#iroha-app-zk-ivm-prove)
* [`iroha app zk ivm get`↴](#iroha-app-zk-ivm-get)
* [`iroha app zk ivm delete`↴](#iroha-app-zk-ivm-delete)
* [`iroha app zk ivm derive-pk`↴](#iroha-app-zk-ivm-derive-pk)
* [`iroha app zk vote`↴](#iroha-app-zk-vote)
* [`iroha app zk vote tally`↴](#iroha-app-zk-vote-tally)
* [`iroha app zk envelope`↴](#iroha-app-zk-envelope)
* [`iroha app confidential`↴](#iroha-app-confidential)
* [`iroha app confidential create-keys`↴](#iroha-app-confidential-create-keys)
* [`iroha app confidential gas`↴](#iroha-app-confidential-gas)
* [`iroha app confidential gas get`↴](#iroha-app-confidential-gas-get)
* [`iroha app confidential gas set`↴](#iroha-app-confidential-gas-set)
* [`iroha app taikai`↴](#iroha-app-taikai)
* [`iroha app taikai bundle`↴](#iroha-app-taikai-bundle)
* [`iroha app taikai cek-rotate`↴](#iroha-app-taikai-cek-rotate)
* [`iroha app taikai rpt-attest`↴](#iroha-app-taikai-rpt-attest)
* [`iroha app taikai ingest`↴](#iroha-app-taikai-ingest)
* [`iroha app taikai ingest watch`↴](#iroha-app-taikai-ingest-watch)
* [`iroha app taikai ingest edge`↴](#iroha-app-taikai-ingest-edge)
* [`iroha app content`↴](#iroha-app-content)
* [`iroha app content publish`↴](#iroha-app-content-publish)
* [`iroha app content pack`↴](#iroha-app-content-pack)
* [`iroha app da`↴](#iroha-app-da)
* [`iroha app da submit`↴](#iroha-app-da-submit)
* [`iroha app da get`↴](#iroha-app-da-get)
* [`iroha app da get-blob`↴](#iroha-app-da-get-blob)
* [`iroha app da prove`↴](#iroha-app-da-prove)
* [`iroha app da prove-availability`↴](#iroha-app-da-prove-availability)
* [`iroha app da proof-policies`↴](#iroha-app-da-proof-policies)
* [`iroha app da proof-policy-snapshot`↴](#iroha-app-da-proof-policy-snapshot)
* [`iroha app da commitments-list`↴](#iroha-app-da-commitments-list)
* [`iroha app da commitments-prove`↴](#iroha-app-da-commitments-prove)
* [`iroha app da commitments-verify`↴](#iroha-app-da-commitments-verify)
* [`iroha app da pin-intents-list`↴](#iroha-app-da-pin-intents-list)
* [`iroha app da pin-intents-prove`↴](#iroha-app-da-pin-intents-prove)
* [`iroha app da pin-intents-verify`↴](#iroha-app-da-pin-intents-verify)
* [`iroha app da rent-quote`↴](#iroha-app-da-rent-quote)
* [`iroha app da rent-ledger`↴](#iroha-app-da-rent-ledger)
* [`iroha app streaming`↴](#iroha-app-streaming)
* [`iroha app streaming fingerprint`↴](#iroha-app-streaming-fingerprint)
* [`iroha app streaming suites`↴](#iroha-app-streaming-suites)
* [`iroha app nexus`↴](#iroha-app-nexus)
* [`iroha app nexus lane-report`↴](#iroha-app-nexus-lane-report)
* [`iroha app nexus public-lane`↴](#iroha-app-nexus-public-lane)
* [`iroha app nexus public-lane validators`↴](#iroha-app-nexus-public-lane-validators)
* [`iroha app nexus public-lane stake`↴](#iroha-app-nexus-public-lane-stake)
* [`iroha app staking`↴](#iroha-app-staking)
* [`iroha app staking register`↴](#iroha-app-staking-register)
* [`iroha app staking rebind`↴](#iroha-app-staking-rebind)
* [`iroha app staking activate`↴](#iroha-app-staking-activate)
* [`iroha app staking exit`↴](#iroha-app-staking-exit)
* [`iroha app subscriptions`↴](#iroha-app-subscriptions)
* [`iroha app subscriptions plan`↴](#iroha-app-subscriptions-plan)
* [`iroha app subscriptions plan create`↴](#iroha-app-subscriptions-plan-create)
* [`iroha app subscriptions plan list`↴](#iroha-app-subscriptions-plan-list)
* [`iroha app subscriptions subscription`↴](#iroha-app-subscriptions-subscription)
* [`iroha app subscriptions subscription create`↴](#iroha-app-subscriptions-subscription-create)
* [`iroha app subscriptions subscription list`↴](#iroha-app-subscriptions-subscription-list)
* [`iroha app subscriptions subscription get`↴](#iroha-app-subscriptions-subscription-get)
* [`iroha app subscriptions subscription pause`↴](#iroha-app-subscriptions-subscription-pause)
* [`iroha app subscriptions subscription resume`↴](#iroha-app-subscriptions-subscription-resume)
* [`iroha app subscriptions subscription cancel`↴](#iroha-app-subscriptions-subscription-cancel)
* [`iroha app subscriptions subscription keep`↴](#iroha-app-subscriptions-subscription-keep)
* [`iroha app subscriptions subscription charge-now`↴](#iroha-app-subscriptions-subscription-charge-now)
* [`iroha app subscriptions subscription usage`↴](#iroha-app-subscriptions-subscription-usage)
* [`iroha app endorsement`↴](#iroha-app-endorsement)
* [`iroha app endorsement prepare`↴](#iroha-app-endorsement-prepare)
* [`iroha app endorsement submit`↴](#iroha-app-endorsement-submit)
* [`iroha app endorsement list`↴](#iroha-app-endorsement-list)
* [`iroha app endorsement policy`↴](#iroha-app-endorsement-policy)
* [`iroha app endorsement committee`↴](#iroha-app-endorsement-committee)
* [`iroha app endorsement register-committee`↴](#iroha-app-endorsement-register-committee)
* [`iroha app endorsement set-policy`↴](#iroha-app-endorsement-set-policy)
* [`iroha app jurisdiction`↴](#iroha-app-jurisdiction)
* [`iroha app jurisdiction verify`↴](#iroha-app-jurisdiction-verify)
* [`iroha app compute`↴](#iroha-app-compute)
* [`iroha app compute simulate`↴](#iroha-app-compute-simulate)
* [`iroha app compute invoke`↴](#iroha-app-compute-invoke)
* [`iroha app soracloud`↴](#iroha-app-soracloud)
* [`iroha app soracloud init`↴](#iroha-app-soracloud-init)
* [`iroha app soracloud deploy`↴](#iroha-app-soracloud-deploy)
* [`iroha app soracloud status`↴](#iroha-app-soracloud-status)
* [`iroha app soracloud config-set`↴](#iroha-app-soracloud-config-set)
* [`iroha app soracloud config-delete`↴](#iroha-app-soracloud-config-delete)
* [`iroha app soracloud config-status`↴](#iroha-app-soracloud-config-status)
* [`iroha app soracloud secret-set`↴](#iroha-app-soracloud-secret-set)
* [`iroha app soracloud secret-delete`↴](#iroha-app-soracloud-secret-delete)
* [`iroha app soracloud secret-status`↴](#iroha-app-soracloud-secret-status)
* [`iroha app soracloud upgrade`↴](#iroha-app-soracloud-upgrade)
* [`iroha app soracloud rollback`↴](#iroha-app-soracloud-rollback)
* [`iroha app soracloud rollout`↴](#iroha-app-soracloud-rollout)
* [`iroha app soracloud agent-deploy`↴](#iroha-app-soracloud-agent-deploy)
* [`iroha app soracloud agent-lease-renew`↴](#iroha-app-soracloud-agent-lease-renew)
* [`iroha app soracloud agent-restart`↴](#iroha-app-soracloud-agent-restart)
* [`iroha app soracloud agent-status`↴](#iroha-app-soracloud-agent-status)
* [`iroha app soracloud agent-wallet-spend`↴](#iroha-app-soracloud-agent-wallet-spend)
* [`iroha app soracloud agent-wallet-approve`↴](#iroha-app-soracloud-agent-wallet-approve)
* [`iroha app soracloud agent-policy-revoke`↴](#iroha-app-soracloud-agent-policy-revoke)
* [`iroha app soracloud agent-message-send`↴](#iroha-app-soracloud-agent-message-send)
* [`iroha app soracloud agent-message-ack`↴](#iroha-app-soracloud-agent-message-ack)
* [`iroha app soracloud agent-mailbox-status`↴](#iroha-app-soracloud-agent-mailbox-status)
* [`iroha app soracloud agent-artifact-allow`↴](#iroha-app-soracloud-agent-artifact-allow)
* [`iroha app soracloud agent-autonomy-run`↴](#iroha-app-soracloud-agent-autonomy-run)
* [`iroha app soracloud agent-autonomy-status`↴](#iroha-app-soracloud-agent-autonomy-status)
* [`iroha app soracloud training-job-start`↴](#iroha-app-soracloud-training-job-start)
* [`iroha app soracloud training-job-checkpoint`↴](#iroha-app-soracloud-training-job-checkpoint)
* [`iroha app soracloud training-job-retry`↴](#iroha-app-soracloud-training-job-retry)
* [`iroha app soracloud training-job-status`↴](#iroha-app-soracloud-training-job-status)
* [`iroha app soracloud model-artifact-register`↴](#iroha-app-soracloud-model-artifact-register)
* [`iroha app soracloud model-artifact-status`↴](#iroha-app-soracloud-model-artifact-status)
* [`iroha app soracloud model-weight-register`↴](#iroha-app-soracloud-model-weight-register)
* [`iroha app soracloud model-weight-promote`↴](#iroha-app-soracloud-model-weight-promote)
* [`iroha app soracloud model-weight-rollback`↴](#iroha-app-soracloud-model-weight-rollback)
* [`iroha app soracloud model-weight-status`↴](#iroha-app-soracloud-model-weight-status)
* [`iroha app soracloud model-upload-encryption-recipient`↴](#iroha-app-soracloud-model-upload-encryption-recipient)
* [`iroha app soracloud model-upload-init`↴](#iroha-app-soracloud-model-upload-init)
* [`iroha app soracloud model-upload-chunk`↴](#iroha-app-soracloud-model-upload-chunk)
* [`iroha app soracloud model-upload-finalize`↴](#iroha-app-soracloud-model-upload-finalize)
* [`iroha app soracloud model-upload-status`↴](#iroha-app-soracloud-model-upload-status)
* [`iroha app soracloud model-compile`↴](#iroha-app-soracloud-model-compile)
* [`iroha app soracloud model-compile-status`↴](#iroha-app-soracloud-model-compile-status)
* [`iroha app soracloud model-allow`↴](#iroha-app-soracloud-model-allow)
* [`iroha app soracloud model-run-private`↴](#iroha-app-soracloud-model-run-private)
* [`iroha app soracloud model-run-status`↴](#iroha-app-soracloud-model-run-status)
* [`iroha app soracloud model-decrypt-output`↴](#iroha-app-soracloud-model-decrypt-output)
* [`iroha app soracloud model-publish-private`↴](#iroha-app-soracloud-model-publish-private)
* [`iroha app soracloud hf-deploy`↴](#iroha-app-soracloud-hf-deploy)
* [`iroha app soracloud hf-status`↴](#iroha-app-soracloud-hf-status)
* [`iroha app soracloud hf-lease-leave`↴](#iroha-app-soracloud-hf-lease-leave)
* [`iroha app soracloud hf-lease-renew`↴](#iroha-app-soracloud-hf-lease-renew)
* [`iroha app soracloud model-host-advertise`↴](#iroha-app-soracloud-model-host-advertise)
* [`iroha app soracloud model-host-heartbeat`↴](#iroha-app-soracloud-model-host-heartbeat)
* [`iroha app soracloud model-host-withdraw`↴](#iroha-app-soracloud-model-host-withdraw)
* [`iroha app soracloud model-host-status`↴](#iroha-app-soracloud-model-host-status)
* [`iroha app social`↴](#iroha-app-social)
* [`iroha app social claim-twitter-follow-reward`↴](#iroha-app-social-claim-twitter-follow-reward)
* [`iroha app social send-to-twitter`↴](#iroha-app-social-send-to-twitter)
* [`iroha app social cancel-twitter-escrow`↴](#iroha-app-social-cancel-twitter-escrow)
* [`iroha app space-directory`↴](#iroha-app-space-directory)
* [`iroha app space-directory manifest`↴](#iroha-app-space-directory-manifest)
* [`iroha app space-directory manifest publish`↴](#iroha-app-space-directory-manifest-publish)
* [`iroha app space-directory manifest encode`↴](#iroha-app-space-directory-manifest-encode)
* [`iroha app space-directory manifest revoke`↴](#iroha-app-space-directory-manifest-revoke)
* [`iroha app space-directory manifest expire`↴](#iroha-app-space-directory-manifest-expire)
* [`iroha app space-directory manifest audit-bundle`↴](#iroha-app-space-directory-manifest-audit-bundle)
* [`iroha app space-directory manifest fetch`↴](#iroha-app-space-directory-manifest-fetch)
* [`iroha app space-directory manifest scaffold`↴](#iroha-app-space-directory-manifest-scaffold)
* [`iroha app space-directory bindings`↴](#iroha-app-space-directory-bindings)
* [`iroha app space-directory bindings fetch`↴](#iroha-app-space-directory-bindings-fetch)
* [`iroha app kaigi`↴](#iroha-app-kaigi)
* [`iroha app kaigi create`↴](#iroha-app-kaigi-create)
* [`iroha app kaigi quickstart`↴](#iroha-app-kaigi-quickstart)
* [`iroha app kaigi register-relay`↴](#iroha-app-kaigi-register-relay)
* [`iroha app kaigi set-relay-manifest`↴](#iroha-app-kaigi-set-relay-manifest)
* [`iroha app kaigi join`↴](#iroha-app-kaigi-join)
* [`iroha app kaigi leave`↴](#iroha-app-kaigi-leave)
* [`iroha app kaigi end`↴](#iroha-app-kaigi-end)
* [`iroha app kaigi record-usage`↴](#iroha-app-kaigi-record-usage)
* [`iroha app kaigi report-relay-health`↴](#iroha-app-kaigi-report-relay-health)
* [`iroha app sorafs`↴](#iroha-app-sorafs)
* [`iroha app sorafs pin`↴](#iroha-app-sorafs-pin)
* [`iroha app sorafs pin list`↴](#iroha-app-sorafs-pin-list)
* [`iroha app sorafs pin show`↴](#iroha-app-sorafs-pin-show)
* [`iroha app sorafs pin register`↴](#iroha-app-sorafs-pin-register)
* [`iroha app sorafs alias`↴](#iroha-app-sorafs-alias)
* [`iroha app sorafs alias list`↴](#iroha-app-sorafs-alias-list)
* [`iroha app sorafs replication`↴](#iroha-app-sorafs-replication)
* [`iroha app sorafs replication list`↴](#iroha-app-sorafs-replication-list)
* [`iroha app sorafs storage`↴](#iroha-app-sorafs-storage)
* [`iroha app sorafs storage pin`↴](#iroha-app-sorafs-storage-pin)
* [`iroha app sorafs storage token`↴](#iroha-app-sorafs-storage-token)
* [`iroha app sorafs storage token issue`↴](#iroha-app-sorafs-storage-token-issue)
* [`iroha app sorafs gateway`↴](#iroha-app-sorafs-gateway)
* [`iroha app sorafs gateway lint-denylist`↴](#iroha-app-sorafs-gateway-lint-denylist)
* [`iroha app sorafs gateway update-denylist`↴](#iroha-app-sorafs-gateway-update-denylist)
* [`iroha app sorafs gateway template-config`↴](#iroha-app-sorafs-gateway-template-config)
* [`iroha app sorafs gateway generate-hosts`↴](#iroha-app-sorafs-gateway-generate-hosts)
* [`iroha app sorafs gateway route-plan`↴](#iroha-app-sorafs-gateway-route-plan)
* [`iroha app sorafs gateway cache-invalidate`↴](#iroha-app-sorafs-gateway-cache-invalidate)
* [`iroha app sorafs gateway evidence`↴](#iroha-app-sorafs-gateway-evidence)
* [`iroha app sorafs gateway direct-mode`↴](#iroha-app-sorafs-gateway-direct-mode)
* [`iroha app sorafs gateway direct-mode plan`↴](#iroha-app-sorafs-gateway-direct-mode-plan)
* [`iroha app sorafs gateway direct-mode enable`↴](#iroha-app-sorafs-gateway-direct-mode-enable)
* [`iroha app sorafs gateway direct-mode rollback`↴](#iroha-app-sorafs-gateway-direct-mode-rollback)
* [`iroha app sorafs gateway merkle`↴](#iroha-app-sorafs-gateway-merkle)
* [`iroha app sorafs gateway merkle snapshot`↴](#iroha-app-sorafs-gateway-merkle-snapshot)
* [`iroha app sorafs gateway merkle proof`↴](#iroha-app-sorafs-gateway-merkle-proof)
* [`iroha app sorafs incentives`↴](#iroha-app-sorafs-incentives)
* [`iroha app sorafs incentives compute`↴](#iroha-app-sorafs-incentives-compute)
* [`iroha app sorafs incentives open-dispute`↴](#iroha-app-sorafs-incentives-open-dispute)
* [`iroha app sorafs incentives dashboard`↴](#iroha-app-sorafs-incentives-dashboard)
* [`iroha app sorafs incentives service`↴](#iroha-app-sorafs-incentives-service)
* [`iroha app sorafs incentives service init`↴](#iroha-app-sorafs-incentives-service-init)
* [`iroha app sorafs incentives service process`↴](#iroha-app-sorafs-incentives-service-process)
* [`iroha app sorafs incentives service record`↴](#iroha-app-sorafs-incentives-service-record)
* [`iroha app sorafs incentives service dispute`↴](#iroha-app-sorafs-incentives-service-dispute)
* [`iroha app sorafs incentives service dispute file`↴](#iroha-app-sorafs-incentives-service-dispute-file)
* [`iroha app sorafs incentives service dispute resolve`↴](#iroha-app-sorafs-incentives-service-dispute-resolve)
* [`iroha app sorafs incentives service dispute reject`↴](#iroha-app-sorafs-incentives-service-dispute-reject)
* [`iroha app sorafs incentives service dashboard`↴](#iroha-app-sorafs-incentives-service-dashboard)
* [`iroha app sorafs incentives service audit`↴](#iroha-app-sorafs-incentives-service-audit)
* [`iroha app sorafs incentives service shadow-run`↴](#iroha-app-sorafs-incentives-service-shadow-run)
* [`iroha app sorafs incentives service reconcile`↴](#iroha-app-sorafs-incentives-service-reconcile)
* [`iroha app sorafs incentives service daemon`↴](#iroha-app-sorafs-incentives-service-daemon)
* [`iroha app sorafs handshake`↴](#iroha-app-sorafs-handshake)
* [`iroha app sorafs handshake show`↴](#iroha-app-sorafs-handshake-show)
* [`iroha app sorafs handshake update`↴](#iroha-app-sorafs-handshake-update)
* [`iroha app sorafs handshake token`↴](#iroha-app-sorafs-handshake-token)
* [`iroha app sorafs handshake token issue`↴](#iroha-app-sorafs-handshake-token-issue)
* [`iroha app sorafs handshake token id`↴](#iroha-app-sorafs-handshake-token-id)
* [`iroha app sorafs handshake token fingerprint`↴](#iroha-app-sorafs-handshake-token-fingerprint)
* [`iroha app sorafs toolkit`↴](#iroha-app-sorafs-toolkit)
* [`iroha app sorafs toolkit pack`↴](#iroha-app-sorafs-toolkit-pack)
* [`iroha app sorafs guard-directory`↴](#iroha-app-sorafs-guard-directory)
* [`iroha app sorafs guard-directory fetch`↴](#iroha-app-sorafs-guard-directory-fetch)
* [`iroha app sorafs guard-directory verify`↴](#iroha-app-sorafs-guard-directory-verify)
* [`iroha app sorafs reserve`↴](#iroha-app-sorafs-reserve)
* [`iroha app sorafs reserve quote`↴](#iroha-app-sorafs-reserve-quote)
* [`iroha app sorafs reserve ledger`↴](#iroha-app-sorafs-reserve-ledger)
* [`iroha app sorafs gar`↴](#iroha-app-sorafs-gar)
* [`iroha app sorafs gar receipt`↴](#iroha-app-sorafs-gar-receipt)
* [`iroha app sorafs repair`↴](#iroha-app-sorafs-repair)
* [`iroha app sorafs repair list`↴](#iroha-app-sorafs-repair-list)
* [`iroha app sorafs repair claim`↴](#iroha-app-sorafs-repair-claim)
* [`iroha app sorafs repair complete`↴](#iroha-app-sorafs-repair-complete)
* [`iroha app sorafs repair fail`↴](#iroha-app-sorafs-repair-fail)
* [`iroha app sorafs repair escalate`↴](#iroha-app-sorafs-repair-escalate)
* [`iroha app sorafs gc`↴](#iroha-app-sorafs-gc)
* [`iroha app sorafs gc inspect`↴](#iroha-app-sorafs-gc-inspect)
* [`iroha app sorafs gc dry-run`↴](#iroha-app-sorafs-gc-dry-run)
* [`iroha app sorafs fetch`↴](#iroha-app-sorafs-fetch)
* [`iroha app soracles`↴](#iroha-app-soracles)
* [`iroha app soracles bundle`↴](#iroha-app-soracles-bundle)
* [`iroha app soracles catalog`↴](#iroha-app-soracles-catalog)
* [`iroha app soracles evidence-gc`↴](#iroha-app-soracles-evidence-gc)
* [`iroha app sns`↴](#iroha-app-sns)
* [`iroha app sns register`↴](#iroha-app-sns-register)
* [`iroha app sns renew`↴](#iroha-app-sns-renew)
* [`iroha app sns transfer`↴](#iroha-app-sns-transfer)
* [`iroha app sns update-controllers`↴](#iroha-app-sns-update-controllers)
* [`iroha app sns freeze`↴](#iroha-app-sns-freeze)
* [`iroha app sns unfreeze`↴](#iroha-app-sns-unfreeze)
* [`iroha app sns registration`↴](#iroha-app-sns-registration)
* [`iroha app sns policy`↴](#iroha-app-sns-policy)
* [`iroha app sns governance`↴](#iroha-app-sns-governance)
* [`iroha app sns governance case`↴](#iroha-app-sns-governance-case)
* [`iroha app sns governance case create`↴](#iroha-app-sns-governance-case-create)
* [`iroha app sns governance case export`↴](#iroha-app-sns-governance-case-export)
* [`iroha app alias`↴](#iroha-app-alias)
* [`iroha app alias voprf-evaluate`↴](#iroha-app-alias-voprf-evaluate)
* [`iroha app alias resolve`↴](#iroha-app-alias-resolve)
* [`iroha app alias resolve-index`↴](#iroha-app-alias-resolve-index)
* [`iroha app alias by-account`↴](#iroha-app-alias-by-account)
* [`iroha app repo`↴](#iroha-app-repo)
* [`iroha app repo initiate`↴](#iroha-app-repo-initiate)
* [`iroha app repo unwind`↴](#iroha-app-repo-unwind)
* [`iroha app repo query`↴](#iroha-app-repo-query)
* [`iroha app repo query list`↴](#iroha-app-repo-query-list)
* [`iroha app repo query get`↴](#iroha-app-repo-query-get)
* [`iroha app repo margin`↴](#iroha-app-repo-margin)
* [`iroha app repo margin-call`↴](#iroha-app-repo-margin-call)
* [`iroha app settlement`↴](#iroha-app-settlement)
* [`iroha app settlement dvp`↴](#iroha-app-settlement-dvp)
* [`iroha app settlement pvp`↴](#iroha-app-settlement-pvp)
* [`iroha tools`↴](#iroha-tools)
* [`iroha tools address`↴](#iroha-tools-address)
* [`iroha tools address convert`↴](#iroha-tools-address-convert)
* [`iroha tools address audit`↴](#iroha-tools-address-audit)
* [`iroha tools address normalize`↴](#iroha-tools-address-normalize)
* [`iroha tools crypto`↴](#iroha-tools-crypto)
* [`iroha tools crypto sm2`↴](#iroha-tools-crypto-sm2)
* [`iroha tools crypto sm2 keygen`↴](#iroha-tools-crypto-sm2-keygen)
* [`iroha tools crypto sm2 import`↴](#iroha-tools-crypto-sm2-import)
* [`iroha tools crypto sm2 export`↴](#iroha-tools-crypto-sm2-export)
* [`iroha tools crypto sm3`↴](#iroha-tools-crypto-sm3)
* [`iroha tools crypto sm3 hash`↴](#iroha-tools-crypto-sm3-hash)
* [`iroha tools crypto sm4`↴](#iroha-tools-crypto-sm4)
* [`iroha tools crypto sm4 gcm-seal`↴](#iroha-tools-crypto-sm4-gcm-seal)
* [`iroha tools crypto sm4 gcm-open`↴](#iroha-tools-crypto-sm4-gcm-open)
* [`iroha tools crypto sm4 ccm-seal`↴](#iroha-tools-crypto-sm4-ccm-seal)
* [`iroha tools crypto sm4 ccm-open`↴](#iroha-tools-crypto-sm4-ccm-open)
* [`iroha tools ivm`↴](#iroha-tools-ivm)
* [`iroha tools ivm abi-hash`↴](#iroha-tools-ivm-abi-hash)
* [`iroha tools ivm syscalls`↴](#iroha-tools-ivm-syscalls)
* [`iroha tools ivm manifest-gen`↴](#iroha-tools-ivm-manifest-gen)
* [`iroha tools markdown-help`↴](#iroha-tools-markdown-help)
* [`iroha tools version`↴](#iroha-tools-version)

## `iroha`

Iroha Client CLI provides a simple way to interact with the Iroha Web API

**Usage:** `iroha [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `account` — Canonical account reads and account mutations
* `tx` — Typed transaction status and transaction helpers
* `ledger` — Ledger data and transaction helpers
* `ops` — Node and operator helpers
* `offline` — Inspect offline allowances and offline-to-online bundles
* `app` — App API helpers and product tooling
* `tools` — Developer utilities and diagnostics

###### **Options:**

* `-c`, `--config <PATH>` — Path to the configuration file.

   By default, `iroha` will try to read `client.toml` file, but would proceed if it is not found.
* `-v`, `--verbose` — Print configuration details to stderr
* `-m`, `--metadata <PATH>` — Path to a JSON file for attaching transaction metadata (optional)
* `-i`, `--input` — Reads instructions from stdin and appends new ones.

   Example usage:

   `echo "[]" | iroha -io domain register --id "domain" | iroha -i asset definition register --id "66owaQmAQMuHxPzxUN3bqZ6FJfDa" --name "USD" --scale 0`
* `-o`, `--output` — Outputs instructions to stdout without submitting them.

   Example usage:

   `iroha -o domain register --id "domain" | iroha -io asset definition register --id "66owaQmAQMuHxPzxUN3bqZ6FJfDa" --name "USD" --scale 0 | iroha transaction stdin`
* `--output-format <OUTPUT_FORMAT>` — Output format for command responses

  Default value: `json`

  Possible values:
  - `json`:
    Emit JSON only
  - `text`:
    Emit human-readable text when available

* `--language <LANG>` — Language code for messages, overrides system language
* `--machine` — Enable deterministic machine mode (no startup chatter, strict config loading)



## `iroha account`

Canonical account reads and account mutations

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

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)
* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha account role grant`

Grant a role to an account

**Usage:** `iroha account role grant --id <ID> --role <ROLE>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)
* `-r`, `--role <ROLE>` — Role name



## `iroha account role revoke`

Revoke a role from an account

**Usage:** `iroha account role revoke --id <ID> --role <ROLE>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)
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

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)
* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha account permission grant`

Grant an account permission using JSON input from stdin

**Usage:** `iroha account permission grant --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)



## `iroha account permission revoke`

Revoke an account permission using JSON input from stdin

**Usage:** `iroha account permission revoke --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)



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

* `-v`, `--verbose` — Display detailed entry information instead of just IDs (when supported)
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

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)



## `iroha account register`

Register an account

**Usage:** `iroha account register [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Canonical global account identifier for registration (canonical I105 literal)
* `--no-wait` — Submit without waiting for confirmation



## `iroha account unregister`

Unregister an account

**Usage:** `iroha account unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)



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



## `iroha tx`

Typed transaction status and transaction helpers

**Usage:** `iroha tx <COMMAND>`

###### **Subcommands:**

* `status` — Read the typed pipeline status of a submitted transaction
* `get` — Retrieve details of a specific transaction
* `ping` — Send an empty transaction that logs a message
* `ivm` — Send a transaction using IVM bytecode
* `stdin` — Send a transaction using JSON input from stdin



## `iroha tx status`

Read the typed pipeline status of a submitted transaction

**Usage:** `iroha tx status [OPTIONS] --hash <HASH>`

###### **Options:**

* `-H`, `--hash <HASH>` — Hash of the signed transaction to inspect
* `--wait` — Poll `/v1/pipeline/transactions/status` until the transaction reaches a stop state
* `--timeout-ms <TIMEOUT_MS>` — Maximum time to wait before failing

  Default value: `30000`
* `--poll-interval-ms <POLL_INTERVAL_MS>` — Poll interval used while waiting

  Default value: `500`
* `--terminal-status <TERMINAL_STATUSES>` — Stop when the pipeline reaches any of these statuses. Applied, rejected, and expired always stop

  Possible values: `queued`, `approved`, `committed`, `applied`, `rejected`, `expired`




## `iroha tx get`

Retrieve details of a specific transaction

**Usage:** `iroha tx get --hash <HASH>`

###### **Options:**

* `-H`, `--hash <HASH>` — Hash of the transaction to retrieve



## `iroha tx ping`

Send an empty transaction that logs a message

**Usage:** `iroha tx ping [OPTIONS] --msg <MSG>`

###### **Options:**

* `-l`, `--log-level <LOG_LEVEL>` — Log levels: TRACE, DEBUG, INFO, WARN, ERROR (in increasing order of visibility)

  Default value: `DEBUG`
* `-m`, `--msg <MSG>` — Log message
* `--count <COUNT>` — Number of ping transactions to send

  Default value: `1`
* `--parallel <PARALLEL>` — Number of parallel workers to use when sending multiple pings

  Default value: `1`
* `--parallel-cap <PARALLEL_CAP>` — Maximum number of parallel workers (0 disables the cap)

  Default value: `1024`
* `--no-wait` — Submit without waiting for confirmation
* `--no-index` — Do not suffix message with "-<index>" when count > 1



## `iroha tx ivm`

Send a transaction using IVM bytecode

**Usage:** `iroha tx ivm [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the IVM bytecode file. If omitted, reads from stdin



## `iroha tx stdin`

Send a transaction using JSON input from stdin

**Usage:** `iroha tx stdin`



## `iroha ledger`

Ledger data and transaction helpers

**Usage:** `iroha ledger <COMMAND>`

###### **Subcommands:**

* `domain` — Read and write domains
* `account` — Read and write accounts
* `asset` — Read and write assets
* `nft` — Read and write NFTs
* `rwa` — Read and write RWA lots
* `peer` — Read and write peers
* `role` — Read and write roles
* `parameter` — Read and write system parameters
* `trigger` — Read and write triggers
* `query` — Read various data
* `transaction` — Read transactions and write various data
* `multisig` — Read and write multi-signature accounts and transactions
* `events` — Subscribe to events: state changes, transaction/block/trigger progress
* `blocks` — Subscribe to blocks



## `iroha ledger domain`

Read and write domains

**Usage:** `iroha ledger domain <COMMAND>`

###### **Subcommands:**

* `list` — List domains
* `get` — Retrieve details of a specific domain
* `register` — Register a domain
* `unregister` — Unregister a domain
* `transfer` — Transfer ownership of a domain
* `meta` — Read and write metadata



## `iroha ledger domain list`

List domains

**Usage:** `iroha ledger domain list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha ledger domain list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha ledger domain list all [OPTIONS]`

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



## `iroha ledger domain list filter`

Filter by a given predicate

**Usage:** `iroha ledger domain list filter [OPTIONS] <PREDICATE>`

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



## `iroha ledger domain get`

Retrieve details of a specific domain

**Usage:** `iroha ledger domain get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name



## `iroha ledger domain register`

Register a domain

**Usage:** `iroha ledger domain register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name



## `iroha ledger domain unregister`

Unregister a domain

**Usage:** `iroha ledger domain unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name



## `iroha ledger domain transfer`

Transfer ownership of a domain

**Usage:** `iroha ledger domain transfer --id <ID> --from <FROM> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — Domain name
* `-f`, `--from <FROM>` — Source account identifier (canonical I105 literal)
* `-t`, `--to <TO>` — Destination account identifier (canonical I105 literal)



## `iroha ledger domain meta`

Read and write metadata

**Usage:** `iroha ledger domain meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha ledger domain meta get`

Retrieve a value from the key-value store

**Usage:** `iroha ledger domain meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger domain meta set`

Create or update an entry in the key-value store using JSON input from stdin

**Usage:** `iroha ledger domain meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger domain meta remove`

Delete an entry from the key-value store

**Usage:** `iroha ledger domain meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger account`

Read and write accounts

**Usage:** `iroha ledger account <COMMAND>`

###### **Subcommands:**

* `role` — Read and write account roles
* `permission` — Read and write account permissions
* `list` — List accounts
* `get` — Retrieve details of a specific account
* `register` — Register an account
* `unregister` — Unregister an account
* `meta` — Read and write metadata



## `iroha ledger account role`

Read and write account roles

**Usage:** `iroha ledger account role <COMMAND>`

###### **Subcommands:**

* `list` — List account role IDs
* `grant` — Grant a role to an account
* `revoke` — Revoke a role from an account



## `iroha ledger account role list`

List account role IDs

**Usage:** `iroha ledger account role list [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)
* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha ledger account role grant`

Grant a role to an account

**Usage:** `iroha ledger account role grant --id <ID> --role <ROLE>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)
* `-r`, `--role <ROLE>` — Role name



## `iroha ledger account role revoke`

Revoke a role from an account

**Usage:** `iroha ledger account role revoke --id <ID> --role <ROLE>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)
* `-r`, `--role <ROLE>` — Role name



## `iroha ledger account permission`

Read and write account permissions

**Usage:** `iroha ledger account permission <COMMAND>`

###### **Subcommands:**

* `list` — List account permissions
* `grant` — Grant an account permission using JSON input from stdin
* `revoke` — Revoke an account permission using JSON input from stdin



## `iroha ledger account permission list`

List account permissions

**Usage:** `iroha ledger account permission list [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)
* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha ledger account permission grant`

Grant an account permission using JSON input from stdin

**Usage:** `iroha ledger account permission grant --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)



## `iroha ledger account permission revoke`

Revoke an account permission using JSON input from stdin

**Usage:** `iroha ledger account permission revoke --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)



## `iroha ledger account list`

List accounts

**Usage:** `iroha ledger account list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha ledger account list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha ledger account list all [OPTIONS]`

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



## `iroha ledger account list filter`

Filter by a given predicate

**Usage:** `iroha ledger account list filter [OPTIONS] <PREDICATE>`

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



## `iroha ledger account get`

Retrieve details of a specific account

**Usage:** `iroha ledger account get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)



## `iroha ledger account register`

Register an account

**Usage:** `iroha ledger account register [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Canonical global account identifier for registration (canonical I105 literal)
* `--no-wait` — Submit without waiting for confirmation



## `iroha ledger account unregister`

Unregister an account

**Usage:** `iroha ledger account unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Account identifier (canonical I105 literal)



## `iroha ledger account meta`

Read and write metadata

**Usage:** `iroha ledger account meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha ledger account meta get`

Retrieve a value from the key-value store

**Usage:** `iroha ledger account meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger account meta set`

Create or update an entry in the key-value store using JSON input from stdin

**Usage:** `iroha ledger account meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger account meta remove`

Delete an entry from the key-value store

**Usage:** `iroha ledger account meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger asset`

Read and write assets

**Usage:** `iroha ledger asset <COMMAND>`

###### **Subcommands:**

* `definition` — Read and write asset definitions
* `get` — Retrieve details of a specific asset
* `list` — List assets
* `mint` — Increase the quantity of an asset
* `burn` — Decrease the quantity of an asset
* `transfer` — Transfer an asset between accounts



## `iroha ledger asset definition`

Read and write asset definitions

**Usage:** `iroha ledger asset definition <COMMAND>`

###### **Subcommands:**

* `list` — List asset definitions
* `get` — Retrieve details of a specific asset definition
* `register` — Register an asset definition
* `unregister` — Unregister an asset definition
* `transfer` — Transfer ownership of an asset definition
* `meta` — Read and write metadata



## `iroha ledger asset definition list`

List asset definitions

**Usage:** `iroha ledger asset definition list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha ledger asset definition list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha ledger asset definition list all [OPTIONS]`

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



## `iroha ledger asset definition list filter`

Filter by a given predicate

**Usage:** `iroha ledger asset definition list filter [OPTIONS] <PREDICATE>`

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



## `iroha ledger asset definition get`

Retrieve details of a specific asset definition

**Usage:** `iroha ledger asset definition get [OPTIONS]`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition identifier (unprefixed Base58 address)
* `--alias <ALIAS>` — Asset definition alias (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`)



## `iroha ledger asset definition register`

Register an asset definition

**Usage:** `iroha ledger asset definition register [OPTIONS] --id <ID> --name <NAME>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition identifier (unprefixed Base58 address)
* `--name <NAME>` — Human-readable asset name
* `--description <DESCRIPTION>` — Optional human-readable description
* `--alias <ALIAS>` — Optional explicit alias literal (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`)
* `--alias-domain <ALIAS_DOMAIN>` — Optional alias owner/domain segment used to build `<name>#<domain>.<dataspace>`
* `--alias-dataspace <ALIAS_DATASPACE>` — Optional alias dataspace segment used to build `<name>#<domain>.<dataspace>` or `<name>#<dataspace>`
* `--logo <LOGO>` — Optional logo URI. Must use `sorafs://...`
* `-m`, `--mint-once` — Disables minting after the first instance
* `-s`, `--scale <SCALE>` — Numeric scale of the asset. No value means unconstrained
* `--confidential-mode <CONFIDENTIAL_MODE>` — Confidential policy mode for this asset definition

  Default value: `transparent-only`

  Possible values: `transparent-only`, `shielded-only`, `convertible`

* `--confidential-vk-set-hash <CONFIDENTIAL_VK_SET_HASH>` — Hex-encoded hash summarising the expected verifying key set
* `--confidential-poseidon-params <CONFIDENTIAL_POSEIDON_PARAMS>` — Poseidon parameter set identifier expected for confidential proofs
* `--confidential-pedersen-params <CONFIDENTIAL_PEDERSEN_PARAMS>` — Pedersen parameter set identifier expected for confidential commitments



## `iroha ledger asset definition unregister`

Unregister an asset definition

**Usage:** `iroha ledger asset definition unregister [OPTIONS]`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition identifier (unprefixed Base58 address)
* `--alias <ALIAS>` — Asset definition alias (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`)



## `iroha ledger asset definition transfer`

Transfer ownership of an asset definition

**Usage:** `iroha ledger asset definition transfer [OPTIONS] --from <FROM> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — Asset definition identifier (unprefixed Base58 address)
* `--alias <ALIAS>` — Asset definition alias (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`)
* `-f`, `--from <FROM>` — Source account identifier (canonical I105 literal)
* `-t`, `--to <TO>` — Destination account identifier (canonical I105 literal)



## `iroha ledger asset definition meta`

Read and write metadata

**Usage:** `iroha ledger asset definition meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha ledger asset definition meta get`

Retrieve a value from the key-value store

**Usage:** `iroha ledger asset definition meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger asset definition meta set`

Create or update an entry in the key-value store using JSON input from stdin

**Usage:** `iroha ledger asset definition meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger asset definition meta remove`

Delete an entry from the key-value store

**Usage:** `iroha ledger asset definition meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger asset get`

Retrieve details of a specific asset

**Usage:** `iroha ledger asset get [OPTIONS]`

###### **Options:**

* `--definition <DEFINITION>` — Canonical asset definition id (unprefixed Base58 address) used with `--account`
* `--definition-alias <DEFINITION_ALIAS>` — Asset definition alias (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`) used with `--account`
* `--account <ACCOUNT>` — Account identifier (canonical I105), required with asset selectors
* `--scope <SCOPE>` — Optional balance scope (`global` or `dataspace:<id>`)



## `iroha ledger asset list`

List assets

**Usage:** `iroha ledger asset list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha ledger asset list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha ledger asset list all [OPTIONS]`

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



## `iroha ledger asset list filter`

Filter by a given predicate

**Usage:** `iroha ledger asset list filter [OPTIONS] <PREDICATE>`

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



## `iroha ledger asset mint`

Increase the quantity of an asset

**Usage:** `iroha ledger asset mint [OPTIONS] --quantity <QUANTITY>`

###### **Options:**

* `--definition <DEFINITION>` — Canonical asset definition id (unprefixed Base58 address) used with `--account`
* `--definition-alias <DEFINITION_ALIAS>` — Asset definition alias (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`) used with `--account`
* `--account <ACCOUNT>` — Account identifier (canonical I105), required with asset selectors
* `--scope <SCOPE>` — Optional balance scope (`global` or `dataspace:<id>`)
* `-q`, `--quantity <QUANTITY>` — Amount of change (integer or decimal)



## `iroha ledger asset burn`

Decrease the quantity of an asset

**Usage:** `iroha ledger asset burn [OPTIONS] --quantity <QUANTITY>`

###### **Options:**

* `--definition <DEFINITION>` — Canonical asset definition id (unprefixed Base58 address) used with `--account`
* `--definition-alias <DEFINITION_ALIAS>` — Asset definition alias (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`) used with `--account`
* `--account <ACCOUNT>` — Account identifier (canonical I105), required with asset selectors
* `--scope <SCOPE>` — Optional balance scope (`global` or `dataspace:<id>`)
* `-q`, `--quantity <QUANTITY>` — Amount of change (integer or decimal)



## `iroha ledger asset transfer`

Transfer an asset between accounts

**Usage:** `iroha ledger asset transfer [OPTIONS] --to <TO> --quantity <QUANTITY>`

###### **Options:**

* `--definition <DEFINITION>` — Canonical asset definition id (unprefixed Base58 address) used with `--account`
* `--definition-alias <DEFINITION_ALIAS>` — Asset definition alias (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`) used with `--account`
* `--account <ACCOUNT>` — Source account identifier (canonical I105), required with asset selectors
* `--scope <SCOPE>` — Optional balance scope (`global` or `dataspace:<id>`)
* `-t`, `--to <TO>` — Destination account identifier (canonical I105 literal)
* `-q`, `--quantity <QUANTITY>` — Transfer amount (integer or decimal)
* `--ensure-destination` — Attempt to register the destination when implicit receive is disabled



## `iroha ledger nft`

Read and write NFTs

**Usage:** `iroha ledger nft <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve details of a specific NFT
* `list` — List NFTs
* `register` — Register NFT with content provided from stdin in JSON format
* `unregister` — Unregister NFT
* `transfer` — Transfer ownership of NFT
* `meta` — Read and write metadata



## `iroha ledger nft get`

Retrieve details of a specific NFT

**Usage:** `iroha ledger nft get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"



## `iroha ledger nft list`

List NFTs

**Usage:** `iroha ledger nft list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha ledger nft list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha ledger nft list all [OPTIONS]`

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



## `iroha ledger nft list filter`

Filter by a given predicate

**Usage:** `iroha ledger nft list filter [OPTIONS] <PREDICATE>`

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



## `iroha ledger nft register`

Register NFT with content provided from stdin in JSON format

**Usage:** `iroha ledger nft register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"



## `iroha ledger nft unregister`

Unregister NFT

**Usage:** `iroha ledger nft unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"



## `iroha ledger nft transfer`

Transfer ownership of NFT

**Usage:** `iroha ledger nft transfer --id <ID> --from <FROM> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — NFT in the format "name$domain"
* `-f`, `--from <FROM>` — Source account identifier (canonical I105 literal)
* `-t`, `--to <TO>` — Destination account identifier (canonical I105 literal)



## `iroha ledger nft meta`

Read and write metadata

**Usage:** `iroha ledger nft meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha ledger nft meta get`

Retrieve a value from the key-value store

**Usage:** `iroha ledger nft meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger nft meta set`

Create or update an entry in the key-value store using JSON input from stdin

**Usage:** `iroha ledger nft meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger nft meta remove`

Delete an entry from the key-value store

**Usage:** `iroha ledger nft meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger rwa`

Read and write RWA lots

**Usage:** `iroha ledger rwa <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve details of a specific RWA lot
* `list` — List RWA lots
* `register` — Register an RWA lot using `NewRwa` JSON from stdin
* `transfer` — Transfer quantity from an existing lot
* `merge` — Merge parent lots using `MergeRwas` JSON from stdin
* `redeem` — Redeem quantity from an existing lot
* `freeze` — Freeze an existing lot
* `unfreeze` — Unfreeze an existing lot
* `hold` — Hold quantity on an existing lot
* `release` — Release held quantity from an existing lot
* `force-transfer` — Force-transfer quantity from an existing lot
* `set-controls` — Replace the lot control policy using `RwaControlPolicy` JSON from stdin
* `meta` — Read and write metadata



## `iroha ledger rwa get`

Retrieve details of a specific RWA lot

**Usage:** `iroha ledger rwa get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — RWA identifier in the format `hash$domain`



## `iroha ledger rwa list`

List RWA lots

**Usage:** `iroha ledger rwa list <COMMAND>`

###### **Subcommands:**

* `all` — List all IDs, or full entries when `--verbose` is specified
* `filter` — Filter by a given predicate



## `iroha ledger rwa list all`

List all IDs, or full entries when `--verbose` is specified

**Usage:** `iroha ledger rwa list all [OPTIONS]`

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



## `iroha ledger rwa list filter`

Filter by a given predicate

**Usage:** `iroha ledger rwa list filter [OPTIONS] <PREDICATE>`

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



## `iroha ledger rwa register`

Register an RWA lot using `NewRwa` JSON from stdin

**Usage:** `iroha ledger rwa register`



## `iroha ledger rwa transfer`

Transfer quantity from an existing lot

**Usage:** `iroha ledger rwa transfer --id <ID> --from <FROM> --quantity <QUANTITY> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — RWA identifier in the format `hash$domain`
* `-f`, `--from <FROM>` — Source account identifier (canonical I105 literal)
* `-q`, `--quantity <QUANTITY>` — Quantity to transfer
* `-t`, `--to <TO>` — Destination account identifier (canonical I105 literal)



## `iroha ledger rwa merge`

Merge parent lots using `MergeRwas` JSON from stdin

**Usage:** `iroha ledger rwa merge`



## `iroha ledger rwa redeem`

Redeem quantity from an existing lot

**Usage:** `iroha ledger rwa redeem --id <ID> --quantity <QUANTITY>`

###### **Options:**

* `-i`, `--id <ID>` — RWA identifier in the format `hash$domain`
* `-q`, `--quantity <QUANTITY>` — Quantity for the operation



## `iroha ledger rwa freeze`

Freeze an existing lot

**Usage:** `iroha ledger rwa freeze --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — RWA identifier in the format `hash$domain`



## `iroha ledger rwa unfreeze`

Unfreeze an existing lot

**Usage:** `iroha ledger rwa unfreeze --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — RWA identifier in the format `hash$domain`



## `iroha ledger rwa hold`

Hold quantity on an existing lot

**Usage:** `iroha ledger rwa hold --id <ID> --quantity <QUANTITY>`

###### **Options:**

* `-i`, `--id <ID>` — RWA identifier in the format `hash$domain`
* `-q`, `--quantity <QUANTITY>` — Quantity for the operation



## `iroha ledger rwa release`

Release held quantity from an existing lot

**Usage:** `iroha ledger rwa release --id <ID> --quantity <QUANTITY>`

###### **Options:**

* `-i`, `--id <ID>` — RWA identifier in the format `hash$domain`
* `-q`, `--quantity <QUANTITY>` — Quantity for the operation



## `iroha ledger rwa force-transfer`

Force-transfer quantity from an existing lot

**Usage:** `iroha ledger rwa force-transfer --id <ID> --quantity <QUANTITY> --to <TO>`

###### **Options:**

* `-i`, `--id <ID>` — RWA identifier in the format `hash$domain`
* `-q`, `--quantity <QUANTITY>` — Quantity to transfer
* `-t`, `--to <TO>` — Destination account identifier (canonical I105 literal)



## `iroha ledger rwa set-controls`

Replace the lot control policy using `RwaControlPolicy` JSON from stdin

**Usage:** `iroha ledger rwa set-controls --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — RWA identifier in the format `hash$domain`



## `iroha ledger rwa meta`

Read and write metadata

**Usage:** `iroha ledger rwa meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha ledger rwa meta get`

Retrieve a value from the key-value store

**Usage:** `iroha ledger rwa meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger rwa meta set`

Create or update an entry in the key-value store using JSON input from stdin

**Usage:** `iroha ledger rwa meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger rwa meta remove`

Delete an entry from the key-value store

**Usage:** `iroha ledger rwa meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger peer`

Read and write peers

**Usage:** `iroha ledger peer <COMMAND>`

###### **Subcommands:**

* `list` — List registered peers expected to connect with each other
* `register` — Register a peer
* `unregister` — Unregister a peer



## `iroha ledger peer list`

List registered peers expected to connect with each other

**Usage:** `iroha ledger peer list <COMMAND>`

###### **Subcommands:**

* `all` — List all registered peers



## `iroha ledger peer list all`

List all registered peers

**Usage:** `iroha ledger peer list all [OPTIONS]`

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



## `iroha ledger peer register`

Register a peer

**Usage:** `iroha ledger peer register --key <KEY> --pop <HEX>`

###### **Options:**

* `-k`, `--key <KEY>` — Peer's public key in multihash format (must be BLS-normal)
* `--pop <HEX>` — Proof-of-possession bytes as hex (with or without 0x prefix)



## `iroha ledger peer unregister`

Unregister a peer

**Usage:** `iroha ledger peer unregister --key <KEY>`

###### **Options:**

* `-k`, `--key <KEY>` — Peer's public key in multihash format



## `iroha ledger role`

Read and write roles

**Usage:** `iroha ledger role <COMMAND>`

###### **Subcommands:**

* `permission` — Read and write role permissions
* `list` — List role IDs
* `register` — Register a role and grant it to the registrant
* `unregister` — Unregister a role



## `iroha ledger role permission`

Read and write role permissions

**Usage:** `iroha ledger role permission <COMMAND>`

###### **Subcommands:**

* `list` — List role permissions
* `grant` — Grant role permission using JSON input from stdin
* `revoke` — Revoke role permission using JSON input from stdin



## `iroha ledger role permission list`

List role permissions

**Usage:** `iroha ledger role permission list [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name
* `--limit <LIMIT>` — Maximum number of items to return (client-side for now)
* `--offset <OFFSET>` — Offset into the result set (client-side for now)

  Default value: `0`



## `iroha ledger role permission grant`

Grant role permission using JSON input from stdin

**Usage:** `iroha ledger role permission grant --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha ledger role permission revoke`

Revoke role permission using JSON input from stdin

**Usage:** `iroha ledger role permission revoke --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha ledger role list`

List role IDs

**Usage:** `iroha ledger role list <COMMAND>`

###### **Subcommands:**

* `all` — List all role IDs



## `iroha ledger role list all`

List all role IDs

**Usage:** `iroha ledger role list all [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha ledger role register`

Register a role and grant it to the registrant

**Usage:** `iroha ledger role register --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha ledger role unregister`

Unregister a role

**Usage:** `iroha ledger role unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Role name



## `iroha ledger parameter`

Read and write system parameters

**Usage:** `iroha ledger parameter <COMMAND>`

###### **Subcommands:**

* `list` — List system parameters
* `set` — Set a system parameter using JSON input from stdin



## `iroha ledger parameter list`

List system parameters

**Usage:** `iroha ledger parameter list <COMMAND>`

###### **Subcommands:**

* `all` — List all system parameters



## `iroha ledger parameter list all`

List all system parameters

**Usage:** `iroha ledger parameter list all`



## `iroha ledger parameter set`

Set a system parameter using JSON input from stdin

**Usage:** `iroha ledger parameter set`



## `iroha ledger trigger`

Read and write triggers

**Usage:** `iroha ledger trigger <COMMAND>`

###### **Subcommands:**

* `list` — List trigger IDs
* `get` — Retrieve details of a specific trigger
* `register` — Register a trigger
* `unregister` — Unregister a trigger
* `mint` — Increase the number of trigger executions
* `burn` — Decrease the number of trigger executions
* `meta` — Read and write metadata



## `iroha ledger trigger list`

List trigger IDs

**Usage:** `iroha ledger trigger list <COMMAND>`

###### **Subcommands:**

* `all` — List all trigger IDs



## `iroha ledger trigger list all`

List all trigger IDs

**Usage:** `iroha ledger trigger list all [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of items to return (server-side limit)
* `--offset <OFFSET>` — Offset into the result set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for iterable queries



## `iroha ledger trigger get`

Retrieve details of a specific trigger

**Usage:** `iroha ledger trigger get --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name



## `iroha ledger trigger register`

Register a trigger

**Usage:** `iroha ledger trigger register [OPTIONS] --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name
* `-p`, `--path <PATH>` — Path to the compiled IVM bytecode to execute
* `--instructions-stdin` — Read JSON array of instructions from stdin instead of bytecode path Example: echo "[ {\"Log\": {\"level\": \"INFO\", \"message\": \"hi\"}} ]" | iroha trigger register -i `my_trig` --instructions-stdin
* `--instructions <PATH>` — Read JSON array of instructions from a file instead of bytecode path
* `-r`, `--repeats <REPEATS>` — Number of permitted executions (default: indefinitely)
* `--authority <AUTHORITY>` — Account executing the trigger (canonical I105 literal)
* `--filter <FILTER>` — Filter type for the trigger

  Default value: `execute`

  Possible values: `execute`, `time`, `data`

* `--time-start-ms <TIME_START_MS>` — Start time in milliseconds since UNIX epoch for time filter
* `--time-period-ms <TIME_PERIOD_MS>` — Period in milliseconds for time filter (optional)
* `--data-filter <JSON>` — JSON for a `DataEventFilter` to use as filter
* `--data-domain <DATA_DOMAIN>` — Data filter preset: events within a domain
* `--data-account <DATA_ACCOUNT>` — Data filter preset: events for an account (canonical I105 literal)
* `--data-asset <DATA_ASSET>` — Data filter preset: events for a specific asset definition; use with `--data-asset-account` for a concrete ownership bucket
* `--data-asset-account <DATA_ASSET_ACCOUNT>` — Data filter preset: account owning the selected asset bucket (canonical I105 literal)
* `--data-asset-scope <DATA_ASSET_SCOPE>` — Data filter preset: balance scope for the selected asset bucket (`global` or `dataspace:<id>`)
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



## `iroha ledger trigger unregister`

Unregister a trigger

**Usage:** `iroha ledger trigger unregister --id <ID>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name



## `iroha ledger trigger mint`

Increase the number of trigger executions

**Usage:** `iroha ledger trigger mint --id <ID> --repetitions <REPETITIONS>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name
* `-r`, `--repetitions <REPETITIONS>` — Amount of change (integer)



## `iroha ledger trigger burn`

Decrease the number of trigger executions

**Usage:** `iroha ledger trigger burn --id <ID> --repetitions <REPETITIONS>`

###### **Options:**

* `-i`, `--id <ID>` — Trigger name
* `-r`, `--repetitions <REPETITIONS>` — Amount of change (integer)



## `iroha ledger trigger meta`

Read and write metadata

**Usage:** `iroha ledger trigger meta <COMMAND>`

###### **Subcommands:**

* `get` — Retrieve a value from the key-value store
* `set` — Create or update an entry in the key-value store using JSON input from stdin
* `remove` — Delete an entry from the key-value store



## `iroha ledger trigger meta get`

Retrieve a value from the key-value store

**Usage:** `iroha ledger trigger meta get --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger trigger meta set`

Create or update an entry in the key-value store using JSON input from stdin

**Usage:** `iroha ledger trigger meta set --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger trigger meta remove`

Delete an entry from the key-value store

**Usage:** `iroha ledger trigger meta remove --id <ID> --key <KEY>`

###### **Options:**

* `-i`, `--id <ID>`
* `-k`, `--key <KEY>`



## `iroha ledger query`

Read various data

**Usage:** `iroha ledger query <COMMAND>`

###### **Subcommands:**

* `stdin` — Query using JSON input from stdin
* `stdin-raw` — Query using raw `SignedQuery` (base64 or hex) from stdin



## `iroha ledger query stdin`

Query using JSON input from stdin

**Usage:** `iroha ledger query stdin`



## `iroha ledger query stdin-raw`

Query using raw `SignedQuery` (base64 or hex) from stdin

**Usage:** `iroha ledger query stdin-raw`



## `iroha ledger transaction`

Read transactions and write various data

**Usage:** `iroha ledger transaction <COMMAND>`

###### **Subcommands:**

* `status` — Read the typed pipeline status of a submitted transaction
* `get` — Retrieve details of a specific transaction
* `ping` — Send an empty transaction that logs a message
* `ivm` — Send a transaction using IVM bytecode
* `stdin` — Send a transaction using JSON input from stdin



## `iroha ledger transaction status`

Read the typed pipeline status of a submitted transaction

**Usage:** `iroha ledger transaction status [OPTIONS] --hash <HASH>`

###### **Options:**

* `-H`, `--hash <HASH>` — Hash of the signed transaction to inspect
* `--wait` — Poll `/v1/pipeline/transactions/status` until the transaction reaches a stop state
* `--timeout-ms <TIMEOUT_MS>` — Maximum time to wait before failing

  Default value: `30000`
* `--poll-interval-ms <POLL_INTERVAL_MS>` — Poll interval used while waiting

  Default value: `500`
* `--terminal-status <TERMINAL_STATUSES>` — Stop when the pipeline reaches any of these statuses. Applied, rejected, and expired always stop

  Possible values: `queued`, `approved`, `committed`, `applied`, `rejected`, `expired`




## `iroha ledger transaction get`

Retrieve details of a specific transaction

**Usage:** `iroha ledger transaction get --hash <HASH>`

###### **Options:**

* `-H`, `--hash <HASH>` — Hash of the transaction to retrieve



## `iroha ledger transaction ping`

Send an empty transaction that logs a message

**Usage:** `iroha ledger transaction ping [OPTIONS] --msg <MSG>`

###### **Options:**

* `-l`, `--log-level <LOG_LEVEL>` — Log levels: TRACE, DEBUG, INFO, WARN, ERROR (in increasing order of visibility)

  Default value: `DEBUG`
* `-m`, `--msg <MSG>` — Log message
* `--count <COUNT>` — Number of ping transactions to send

  Default value: `1`
* `--parallel <PARALLEL>` — Number of parallel workers to use when sending multiple pings

  Default value: `1`
* `--parallel-cap <PARALLEL_CAP>` — Maximum number of parallel workers (0 disables the cap)

  Default value: `1024`
* `--no-wait` — Submit without waiting for confirmation
* `--no-index` — Do not suffix message with "-<index>" when count > 1



## `iroha ledger transaction ivm`

Send a transaction using IVM bytecode

**Usage:** `iroha ledger transaction ivm [OPTIONS]`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the IVM bytecode file. If omitted, reads from stdin



## `iroha ledger transaction stdin`

Send a transaction using JSON input from stdin

**Usage:** `iroha ledger transaction stdin`



## `iroha ledger multisig`

Read and write multi-signature accounts and transactions

**Usage:** `iroha ledger multisig <COMMAND>`

###### **Subcommands:**

* `list` — List pending multisig transactions relevant to you
* `register` — Register a multisig account
* `propose` — Propose a multisig transaction using JSON input from stdin
* `approve` — Approve a multisig transaction
* `cancel` — Propose cancellation of an existing multisig transaction
* `inspect` — Inspect a multisig account controller and print the CTAP2 payload + digest



## `iroha ledger multisig list`

List pending multisig transactions relevant to you

**Usage:** `iroha ledger multisig list <COMMAND>`

###### **Subcommands:**

* `all` — List all pending multisig transactions relevant to you



## `iroha ledger multisig list all`

List all pending multisig transactions relevant to you

**Usage:** `iroha ledger multisig list all [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of role IDs to scan for multisig (server-side limit)
* `--offset <OFFSET>` — Offset into the role ID set (server-side offset)

  Default value: `0`
* `--fetch-size <FETCH_SIZE>` — Batch fetch size for roles query



## `iroha ledger multisig register`

Register a multisig account

**Usage:** `iroha ledger multisig register [OPTIONS] --quorum <QUORUM>`

###### **Options:**

* `-s`, `--signatories <SIGNATORIES>` — List of signatories for the multisig account (canonical I105 literal)
* `-w`, `--weights <WEIGHTS>` — Relative weights of signatories' responsibilities
* `-q`, `--quorum <QUORUM>` — Threshold of total weight required for authentication
* `--account <ACCOUNT>` — Account id to use for the multisig controller. If omitted, a new random domainless account id is generated locally, the private key is discarded, and the registration uses the configured default home domain
* `-t`, `--transaction-ttl <TRANSACTION_TTL>` — Time-to-live for multisig transactions. Example: "1y 6M 2w 3d 12h 30m 30s"

  Default value: `1h`



## `iroha ledger multisig propose`

Propose a multisig transaction using JSON input from stdin

**Usage:** `iroha ledger multisig propose [OPTIONS] --account <ACCOUNT>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — Multisig authority managing the proposed transaction
* `-t`, `--transaction-ttl <TRANSACTION_TTL>` — Overrides the default time-to-live for this transaction. Example: "1y 6M 2w 3d 12h 30m 30s" Must not exceed the multisig policy TTL; the CLI will preview the effective expiry and reject overrides above the policy cap



## `iroha ledger multisig approve`

Approve a multisig transaction

**Usage:** `iroha ledger multisig approve --account <ACCOUNT> --instructions-hash <INSTRUCTIONS_HASH>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — Multisig authority of the transaction
* `-i`, `--instructions-hash <INSTRUCTIONS_HASH>` — Hash of the instructions to approve



## `iroha ledger multisig cancel`

Propose cancellation of an existing multisig transaction

**Usage:** `iroha ledger multisig cancel [OPTIONS] --account <ACCOUNT> --instructions-hash <INSTRUCTIONS_HASH>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — Multisig authority of the transaction
* `-i`, `--instructions-hash <INSTRUCTIONS_HASH>` — Hash of the target proposal instructions to cancel
* `-t`, `--transaction-ttl <TRANSACTION_TTL>` — Overrides the default time-to-live for the cancel proposal itself



## `iroha ledger multisig inspect`

Inspect a multisig account controller and print the CTAP2 payload + digest

**Usage:** `iroha ledger multisig inspect [OPTIONS] --account <ACCOUNT>`

###### **Options:**

* `-a`, `--account <ACCOUNT>` — Multisig account identifier to inspect
* `--json` — Emit JSON instead of human-readable output



## `iroha ledger events`

Subscribe to events: state changes, transaction/block/trigger progress

**Usage:** `iroha ledger events [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `state` — Notify when the world state undergoes certain changes
* `governance` — Notify governance lifecycle events
* `transaction` — Notify when a transaction reaches specific stages
* `block` — Notify when a block reaches specific stages
* `trigger-execute` — Notify when a trigger execution is ordered
* `trigger-complete` — Notify when a trigger execution is completed

###### **Options:**

* `-t`, `--timeout <TIMEOUT>` — Duration to listen for events. Example: "1y 6M 2w 3d 12h 30m 30s"



## `iroha ledger events state`

Notify when the world state undergoes certain changes

**Usage:** `iroha ledger events state`



## `iroha ledger events governance`

Notify governance lifecycle events

**Usage:** `iroha ledger events governance [OPTIONS]`

###### **Options:**

* `--proposal-id <ID_HEX>` — Filter by proposal id (hex)
* `--referendum-id <RID>` — Filter by referendum id



## `iroha ledger events transaction`

Notify when a transaction reaches specific stages

**Usage:** `iroha ledger events transaction`



## `iroha ledger events block`

Notify when a block reaches specific stages

**Usage:** `iroha ledger events block`



## `iroha ledger events trigger-execute`

Notify when a trigger execution is ordered

**Usage:** `iroha ledger events trigger-execute`



## `iroha ledger events trigger-complete`

Notify when a trigger execution is completed

**Usage:** `iroha ledger events trigger-complete`



## `iroha ledger blocks`

Subscribe to blocks

**Usage:** `iroha ledger blocks [OPTIONS] <HEIGHT>`

###### **Arguments:**

* `<HEIGHT>` — Block height from which to start streaming blocks

###### **Options:**

* `-t`, `--timeout <TIMEOUT>` — Duration to listen for events. Example: "1y 6M 2w 3d 12h 30m 30s"



## `iroha ops`

Node and operator helpers

**Usage:** `iroha ops <COMMAND>`

###### **Subcommands:**

* `offline` — Inspect offline allowances and offline-to-online bundles
* `executor` — Read and write the executor
* `runtime` — Runtime ABI/upgrades
* `sumeragi` — Sumeragi helpers (status)
* `audit` — Audit helpers (debug endpoints)
* `connect` — Connect diagnostics helpers (queue inspection, evidence export)



## `iroha ops offline`

Inspect offline allowances and offline-to-online bundles

**Usage:** `iroha ops offline <COMMAND>`

###### **Subcommands:**

* `allowance` — Inspect offline allowances registered on-ledger
* `transfer` — Inspect pending offline-to-online transfer bundles
* `bundle` — Inspect offline bundle fixtures and aggregate proofs
* `summary` — Inspect derived counter summaries per offline certificate
* `revocation` — Inspect recorded verdict revocations
* `rejection` — Fetch offline rejection telemetry snapshots
* `qr` — Encode/decode QR stream frames for offline payloads
* `petal` — Encode/decode petal stream frames for offline payloads



## `iroha ops offline allowance`

Inspect offline allowances registered on-ledger

**Usage:** `iroha ops offline allowance <COMMAND>`

###### **Subcommands:**

* `list` — List all registered offline allowances
* `get` — Fetch a specific allowance by certificate id



## `iroha ops offline allowance list`

List all registered offline allowances

**Usage:** `iroha ops offline allowance list [OPTIONS]`

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



## `iroha ops offline allowance get`

Fetch a specific allowance by certificate id

**Usage:** `iroha ops offline allowance get --certificate-id <CERTIFICATE_ID>`

###### **Options:**

* `--certificate-id <CERTIFICATE_ID>` — Deterministic certificate identifier (hex)



## `iroha ops offline transfer`

Inspect pending offline-to-online transfer bundles

**Usage:** `iroha ops offline transfer <COMMAND>`

###### **Subcommands:**

* `list` — List all pending offline-to-online transfer bundles
* `get` — Fetch a specific transfer bundle by id
* `proof` — Generate a FASTPQ witness request for a bundle payload



## `iroha ops offline transfer list`

List all pending offline-to-online transfer bundles

**Usage:** `iroha ops offline transfer list [OPTIONS]`

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

  Possible values: `settled`, `rejected`, `archived`

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



## `iroha ops offline transfer get`

Fetch a specific transfer bundle by id

**Usage:** `iroha ops offline transfer get --bundle-id <BUNDLE_ID>`

###### **Options:**

* `--bundle-id <BUNDLE_ID>` — Deterministic bundle identifier (hex)



## `iroha ops offline transfer proof`

Generate a FASTPQ witness request for a bundle payload

**Usage:** `iroha ops offline transfer proof [OPTIONS] --bundle <PATH> --kind <KIND>`

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



## `iroha ops offline bundle`

Inspect offline bundle fixtures and aggregate proofs

**Usage:** `iroha ops offline bundle <COMMAND>`

###### **Subcommands:**

* `inspect` — Inspect offline bundle fixtures and compute Poseidon receipts roots



## `iroha ops offline bundle inspect`

Inspect offline bundle fixtures and compute Poseidon receipts roots

**Usage:** `iroha ops offline bundle inspect [OPTIONS] <PATH>...`

###### **Arguments:**

* `<PATH>` — Paths to offline bundle fixtures (JSON or Norito)

###### **Options:**

* `--encoding <ENCODING>` — Override the bundle encoding detection

  Default value: `auto`

  Possible values: `auto`, `json`, `norito`

* `--proofs` — Include aggregate proof byte counts and metadata keys



## `iroha ops offline summary`

Inspect derived counter summaries per offline certificate

**Usage:** `iroha ops offline summary <COMMAND>`

###### **Subcommands:**

* `list` — List counter summaries derived from wallet allowances
* `export` — Export counter summaries to a JSON digest for receiver sharing



## `iroha ops offline summary list`

List counter summaries derived from wallet allowances

**Usage:** `iroha ops offline summary list [OPTIONS]`

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



## `iroha ops offline summary export`

Export counter summaries to a JSON digest for receiver sharing

**Usage:** `iroha ops offline summary export [OPTIONS] --output <PATH>`

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



## `iroha ops offline revocation`

Inspect recorded verdict revocations

**Usage:** `iroha ops offline revocation <COMMAND>`

###### **Subcommands:**

* `list` — List recorded verdict revocations



## `iroha ops offline revocation list`

List recorded verdict revocations

**Usage:** `iroha ops offline revocation list [OPTIONS]`

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



## `iroha ops offline rejection`

Fetch offline rejection telemetry snapshots

**Usage:** `iroha ops offline rejection <COMMAND>`

###### **Subcommands:**

* `stats` — Fetch aggregated offline rejection counters



## `iroha ops offline rejection stats`

Fetch aggregated offline rejection counters

**Usage:** `iroha ops offline rejection stats [OPTIONS]`

###### **Options:**

* `--telemetry-profile <PROFILE>` — Optional telemetry profile header used when fetching stats



## `iroha ops offline qr`

Encode/decode QR stream frames for offline payloads

**Usage:** `iroha ops offline qr <COMMAND>`

###### **Subcommands:**

* `encode` — Encode a payload into QR stream frames
* `decode` — Decode QR stream frames into the original payload



## `iroha ops offline qr encode`

Encode a payload into QR stream frames

**Usage:** `iroha ops offline qr encode [OPTIONS] --input <FILE> --output <DIR>`

###### **Options:**

* `--input <FILE>` — Path to the payload bytes to encode
* `--output <DIR>` — Output directory for generated frames and artifacts
* `--payload-kind <PAYLOAD_KIND>` — Payload kind tag embedded in the envelope

  Default value: `unspecified`

  Possible values: `unspecified`, `offline-to-online-transfer`, `offline-spend-receipt`, `offline-envelope`

* `--chunk-size <CHUNK_SIZE>` — Chunk size in bytes

  Default value: `360`
* `--parity-group <PARITY_GROUP>` — Parity group size (0 disables parity frames)

  Default value: `0`
* `--ecc <ECC>` — QR error correction level

  Default value: `m`

  Possible values: `l`, `m`, `q`, `h`

* `--frame-encoding <FRAME_ENCODING>` — QR frame encoding mode

  Default value: `binary`

  Possible values: `binary`, `base64`

* `--dimension <DIMENSION>` — Rendered QR image size in pixels

  Default value: `512`
* `--format <FORMAT>` — Output format for rendered frames

  Default value: `frames`

  Possible values: `frames`, `svg`, `png`, `gif`, `apng`

* `--style <STYLE>` — Render style for preview images (ignored for --format frames)

  Default value: `mono`

  Possible values: `mono`, `sakura`, `sakura-wind`, `sakura-storm`

* `--fps <FPS>` — Frames per second for animated outputs

  Default value: `12`



## `iroha ops offline qr decode`

Decode QR stream frames into the original payload

**Usage:** `iroha ops offline qr decode [OPTIONS] --input-dir <DIR> --output <FILE>`

###### **Options:**

* `--input-dir <DIR>` — Directory containing raw frame bytes
* `--output <FILE>` — Output file for the decoded payload
* `--frame-encoding <FRAME_ENCODING>` — Frame encoding used in the input

  Default value: `binary`

  Possible values: `binary`, `base64`

* `--output-manifest <FILE>` — Optional JSON manifest output path



## `iroha ops offline petal`

Encode/decode petal stream frames for offline payloads

**Usage:** `iroha ops offline petal <COMMAND>`

###### **Subcommands:**

* `encode` — Encode a payload into petal stream frames
* `decode` — Decode petal stream frames into the original payload
* `eval-capture` — Evaluate decode robustness under simulated distant/moving capture
* `simulate-realtime` — Simulate live camera reading and decode frame-by-frame in real time
* `score-styles` — Score render styles with deterministic capture simulation and throughput metrics



## `iroha ops offline petal encode`

Encode a payload into petal stream frames

**Usage:** `iroha ops offline petal encode [OPTIONS] --input <FILE> --output <DIR>`

###### **Options:**

* `--input <FILE>` — Path to the payload bytes to encode
* `--output <DIR>` — Output directory for generated frames and artifacts
* `--payload-kind <PAYLOAD_KIND>` — Payload kind tag embedded in the envelope

  Default value: `unspecified`

  Possible values: `unspecified`, `offline-to-online-transfer`, `offline-spend-receipt`, `offline-envelope`

* `--chunk-size <CHUNK_SIZE>` — Chunk size in bytes

  Default value: `140`
* `--parity-group <PARITY_GROUP>` — Parity group size (0 disables parity frames)

  Default value: `0`
* `--grid-size <GRID_SIZE>` — Grid size in cells (0 selects automatic sizing)

  Default value: `0`
* `--border <BORDER>` — Border thickness in cells

  Default value: `1`
* `--anchor-size <ANCHOR_SIZE>` — Anchor size in cells

  Default value: `3`
* `--dimension <DIMENSION>` — Rendered frame size in pixels

  Default value: `512`
* `--format <FORMAT>` — Output format for rendered frames

  Default value: `frames`

  Possible values: `frames`, `png`, `gif`, `apng`

* `--fps <FPS>` — Frames per second for animated outputs

  Default value: `24`
* `--style <STYLE>` — Render style for preview images (ignored for --format frames)

  Default value: `sora-temple`

  Possible values: `sakura-wind`, `sora-temple`, `sora-temple-bold`, `sora-temple-minimal`, `sora-temple-radiant`, `sora-temple-command`, `sora-temple-aegis`, `sora-temple-ghost`

* `--channel <CHANNEL>` — Data channel used for data cells in rendered outputs

  Default value: `binary`

  Possible values: `binary`, `katakana-base94`

* `--katakana-preset <KATAKANA_PRESET>` — Katakana channel tuning preset used when grid/chunk are left at defaults

  Default value: `balanced`

  Possible values: `balanced`, `distance-safe`




## `iroha ops offline petal decode`

Decode petal stream frames into the original payload

**Usage:** `iroha ops offline petal decode [OPTIONS] --input-dir <DIR> --output <FILE>`

###### **Options:**

* `--input-dir <DIR>` — Directory containing PNG frames
* `--output <FILE>` — Output file for the decoded payload
* `--grid-size <GRID_SIZE>` — Grid size in cells (0 to auto-detect)

  Default value: `0`
* `--border <BORDER>` — Border thickness in cells

  Default value: `1`
* `--anchor-size <ANCHOR_SIZE>` — Anchor size in cells

  Default value: `3`
* `--output-manifest <FILE>` — Optional JSON manifest output path
* `--channel <CHANNEL>` — Data channel used by rendered frames

  Default value: `binary`

  Possible values: `binary`, `katakana-base94`




## `iroha ops offline petal eval-capture`

Evaluate decode robustness under simulated distant/moving capture

**Usage:** `iroha ops offline petal eval-capture [OPTIONS] --input-dir <DIR>`

###### **Options:**

* `--input-dir <DIR>` — Directory containing rendered PNG frames
* `--grid-size <GRID_SIZE>` — Grid size in cells (0 to auto-detect from pristine frames)

  Default value: `0`
* `--border <BORDER>` — Border thickness in cells

  Default value: `1`
* `--anchor-size <ANCHOR_SIZE>` — Anchor size in cells

  Default value: `3`
* `--channel <CHANNEL>` — Data channel used by rendered frames

  Default value: `binary`

  Possible values: `binary`, `katakana-base94`

* `--profile <PROFILE>` — Capture perturbation profile

  Default value: `default`

  Possible values: `default`, `aggressive`

* `--seed <SEED>` — Deterministic seed for perturbation sampling

  Default value: `42`
* `--trials-per-frame <TRIALS_PER_FRAME>` — Number of perturbation trials per frame (0 uses profile default)

  Default value: `0`
* `--min-success-ratio <MIN_SUCCESS_RATIO>` — Minimum successful decode ratio required to pass

  Default value: `0.95`
* `--output-report <FILE>` — Optional JSON report output path



## `iroha ops offline petal simulate-realtime`

Simulate live camera reading and decode frame-by-frame in real time

**Usage:** `iroha ops offline petal simulate-realtime [OPTIONS] --input-dir <DIR>`

###### **Options:**

* `--input-dir <DIR>` — Directory containing rendered PNG frames
* `--output-payload <FILE>` — Optional output file for the decoded payload
* `--output-report <FILE>` — Optional JSON report output path
* `--grid-size <GRID_SIZE>` — Grid size in cells (0 to auto-detect from the first frame)

  Default value: `0`
* `--border <BORDER>` — Border thickness in cells

  Default value: `1`
* `--anchor-size <ANCHOR_SIZE>` — Anchor size in cells

  Default value: `3`
* `--channel <CHANNEL>` — Data channel used by rendered frames

  Default value: `binary`

  Possible values: `binary`, `katakana-base94`

* `--profile <PROFILE>` — Capture perturbation profile used to emulate a moving camera read

  Default value: `default`

  Possible values: `default`, `aggressive`

* `--seed <SEED>` — Deterministic seed for capture perturbation sampling

  Default value: `42`
* `--simulate-fps <SIMULATE_FPS>` — Simulated camera frame rate used to compute timeline metrics

  Default value: `24`
* `--frame-limit <FRAME_LIMIT>` — Optional cap on number of frames to process from the input directory
* `--realtime-loops <REALTIME_LOOPS>` — Number of playback loops to simulate over the same frame set

  Default value: `1`
* `--disable-capture-perturbation` — Disable capture perturbation and decode pristine frames only
* `--allow-incomplete` — Allow incomplete stream reconstruction without returning an error



## `iroha ops offline petal score-styles`

Score render styles with deterministic capture simulation and throughput metrics

**Usage:** `iroha ops offline petal score-styles [OPTIONS] --input <FILE> --output-report <FILE>`

###### **Options:**

* `--input <FILE>` — Path to payload bytes used for style scoring
* `--output-report <FILE>` — JSON report path for scored styles
* `--style <STYLE>` — Styles to evaluate (repeat flag). Empty means the default temple style set

  Possible values: `sakura-wind`, `sora-temple`, `sora-temple-bold`, `sora-temple-minimal`, `sora-temple-radiant`, `sora-temple-command`, `sora-temple-aegis`, `sora-temple-ghost`

* `--payload-kind <PAYLOAD_KIND>` — Payload kind tag embedded in the envelope

  Default value: `unspecified`

  Possible values: `unspecified`, `offline-to-online-transfer`, `offline-spend-receipt`, `offline-envelope`

* `--chunk-size <CHUNK_SIZE>` — Chunk size in bytes

  Default value: `140`
* `--parity-group <PARITY_GROUP>` — Parity group size (0 disables parity frames)

  Default value: `0`
* `--grid-size <GRID_SIZE>` — Grid size in cells (0 selects automatic sizing)

  Default value: `0`
* `--border <BORDER>` — Border thickness in cells

  Default value: `1`
* `--anchor-size <ANCHOR_SIZE>` — Anchor size in cells

  Default value: `3`
* `--dimension <DIMENSION>` — Rendered frame size in pixels

  Default value: `512`
* `--fps <FPS>` — Frames per second used for effective throughput scoring

  Default value: `24`
* `--profile <PROFILE>` — Capture perturbation profile

  Default value: `default`

  Possible values: `default`, `aggressive`

* `--seed <SEED>` — Deterministic seed for perturbation sampling

  Default value: `42`
* `--trials-per-frame <TRIALS_PER_FRAME>` — Number of perturbation trials per frame (0 uses profile default)

  Default value: `0`
* `--min-success-ratio <MIN_SUCCESS_RATIO>` — Minimum capture success ratio used for the pass gate in the report

  Default value: `0.95`
* `--target-effective-bps <TARGET_EFFECTIVE_BPS>` — Target effective throughput used to normalize throughput scoring

  Default value: `3000`



## `iroha ops executor`

Read and write the executor

**Usage:** `iroha ops executor <COMMAND>`

###### **Subcommands:**

* `data-model` — Retrieve the executor data model
* `upgrade` — Upgrade the executor



## `iroha ops executor data-model`

Retrieve the executor data model

**Usage:** `iroha ops executor data-model`



## `iroha ops executor upgrade`

Upgrade the executor

**Usage:** `iroha ops executor upgrade --path <PATH>`

###### **Options:**

* `-p`, `--path <PATH>` — Path to the compiled IVM bytecode file



## `iroha ops runtime`

Runtime ABI/upgrades

**Usage:** `iroha ops runtime <COMMAND>`

###### **Subcommands:**

* `abi` — Runtime ABI helpers
* `upgrade` — Runtime upgrade management
* `status` — Show runtime metrics/status summary
* `capabilities` — Fetch node capability advert (ABI + crypto manifest)



## `iroha ops runtime abi`

Runtime ABI helpers

**Usage:** `iroha ops runtime abi <COMMAND>`

###### **Subcommands:**

* `active` — Fetch the active ABI version from the node
* `active-query` — Fetch the active ABI version via signed Norito query (core /query)
* `hash` — Fetch the node's canonical ABI hash for the active policy



## `iroha ops runtime abi active`

Fetch the active ABI version from the node

**Usage:** `iroha ops runtime abi active`



## `iroha ops runtime abi active-query`

Fetch the active ABI version via signed Norito query (core /query)

**Usage:** `iroha ops runtime abi active-query`



## `iroha ops runtime abi hash`

Fetch the node's canonical ABI hash for the active policy

**Usage:** `iroha ops runtime abi hash`



## `iroha ops runtime upgrade`

Runtime upgrade management

**Usage:** `iroha ops runtime upgrade <COMMAND>`

###### **Subcommands:**

* `list` — List proposed/activated runtime upgrades
* `propose` — Build a `ProposeRuntimeUpgrade` instruction skeleton via Torii
* `activate` — Build an `ActivateRuntimeUpgrade` instruction skeleton via Torii
* `cancel` — Build a `CancelRuntimeUpgrade` instruction skeleton via Torii



## `iroha ops runtime upgrade list`

List proposed/activated runtime upgrades

**Usage:** `iroha ops runtime upgrade list`



## `iroha ops runtime upgrade propose`

Build a `ProposeRuntimeUpgrade` instruction skeleton via Torii

**Usage:** `iroha ops runtime upgrade propose --file <PATH>`

###### **Options:**

* `--file <PATH>` — Path to a JSON file with `RuntimeUpgradeManifest` fields



## `iroha ops runtime upgrade activate`

Build an `ActivateRuntimeUpgrade` instruction skeleton via Torii

**Usage:** `iroha ops runtime upgrade activate --id <HEX>`

###### **Options:**

* `--id <HEX>` — Upgrade id (hex)



## `iroha ops runtime upgrade cancel`

Build a `CancelRuntimeUpgrade` instruction skeleton via Torii

**Usage:** `iroha ops runtime upgrade cancel --id <HEX>`

###### **Options:**

* `--id <HEX>` — Upgrade id (hex)



## `iroha ops runtime status`

Show runtime metrics/status summary

**Usage:** `iroha ops runtime status`



## `iroha ops runtime capabilities`

Fetch node capability advert (ABI + crypto manifest)

**Usage:** `iroha ops runtime capabilities`



## `iroha ops sumeragi`

Sumeragi helpers (status)

**Usage:** `iroha ops sumeragi <COMMAND>`

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



## `iroha ops sumeragi status`

Show consensus status snapshot (leader, `HighestQC`, `LockedQC`)

**Usage:** `iroha ops sumeragi status`



## `iroha ops sumeragi leader`

Show leader index (and PRF context when available)

**Usage:** `iroha ops sumeragi leader`



## `iroha ops sumeragi params`

Show on-chain Sumeragi parameters snapshot

**Usage:** `iroha ops sumeragi params`



## `iroha ops sumeragi collectors`

Show current collector indices and peers

**Usage:** `iroha ops sumeragi collectors`



## `iroha ops sumeragi qc`

Show HighestQC/LockedQC snapshot

**Usage:** `iroha ops sumeragi qc`



## `iroha ops sumeragi pacemaker`

Show pacemaker timers/config snapshot

**Usage:** `iroha ops sumeragi pacemaker`



## `iroha ops sumeragi phases`

Show latest per-phase latencies (ms)

**Usage:** `iroha ops sumeragi phases`



## `iroha ops sumeragi telemetry`

Show aggregated telemetry snapshot (availability, QC, RBC, VRF)

**Usage:** `iroha ops sumeragi telemetry`



## `iroha ops sumeragi evidence`

Evidence helpers (list/count/submit)

**Usage:** `iroha ops sumeragi evidence <COMMAND>`

###### **Subcommands:**

* `list` — List persisted evidence entries
* `count` — Show evidence count
* `submit` — Submit hex-encoded evidence payload



## `iroha ops sumeragi evidence list`

List persisted evidence entries

**Usage:** `iroha ops sumeragi evidence list [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of entries to return
* `--offset <OFFSET>` — Offset into the evidence list
* `--kind <KIND>` — Filter by evidence kind

  Possible values: `double-prepare`, `double-commit`, `invalid-qc`, `invalid-proposal`




## `iroha ops sumeragi evidence count`

Show evidence count

**Usage:** `iroha ops sumeragi evidence count`



## `iroha ops sumeragi evidence submit`

Submit hex-encoded evidence payload

**Usage:** `iroha ops sumeragi evidence submit [OPTIONS]`

###### **Options:**

* `--evidence-hex <EVIDENCE_HEX>` — Hex-encoded Norito evidence payload (0x optional)
* `--evidence-hex-file <PATH>` — Path to file containing hex-encoded proof (whitespace ignored)



## `iroha ops sumeragi rbc`

RBC helpers (status/sessions)

**Usage:** `iroha ops sumeragi rbc <COMMAND>`

###### **Subcommands:**

* `status` — Show RBC session/throughput counters
* `sessions` — Show RBC sessions snapshot



## `iroha ops sumeragi rbc status`

Show RBC session/throughput counters

**Usage:** `iroha ops sumeragi rbc status`



## `iroha ops sumeragi rbc sessions`

Show RBC sessions snapshot

**Usage:** `iroha ops sumeragi rbc sessions`



## `iroha ops sumeragi vrf-penalties`

Show VRF penalties for the given epoch

**Usage:** `iroha ops sumeragi vrf-penalties --epoch <EPOCH>`

###### **Options:**

* `--epoch <EPOCH>` — Epoch index (decimal or 0x-prefixed hex)



## `iroha ops sumeragi vrf-epoch`

Show persisted VRF epoch snapshot (seed, participants, penalties)

**Usage:** `iroha ops sumeragi vrf-epoch --epoch <EPOCH>`

###### **Options:**

* `--epoch <EPOCH>` — Epoch index (decimal or 0x-prefixed hex)



## `iroha ops sumeragi commit-qc`

Fetch commit QC (if present) for a block hash

**Usage:** `iroha ops sumeragi commit-qc <COMMAND>`

###### **Subcommands:**

* `get` — Fetch commit QC (if present) for a block hash



## `iroha ops sumeragi commit-qc get`

Fetch commit QC (if present) for a block hash

**Usage:** `iroha ops sumeragi commit-qc get --hash <HASH>`

###### **Options:**

* `--hash <HASH>` — Block hash for which the commit QC should be fetched



## `iroha ops audit`

Audit helpers (debug endpoints)

**Usage:** `iroha ops audit <COMMAND>`

###### **Subcommands:**

* `witness` — Fetch current execution witness snapshot from Torii debug endpoints



## `iroha ops audit witness`

Fetch current execution witness snapshot from Torii debug endpoints

**Usage:** `iroha ops audit witness [OPTIONS]`

###### **Options:**

* `--binary` — Fetch Norito-encoded binary instead of JSON
* `--out <PATH>` — Output path for binary; if omitted with --binary, hex is printed to stdout
* `--decode <PATH>` — Decode a Norito-encoded `ExecWitness` from a file and print with human-readable keys
* `--filter <PREFIXES>` — Filter decoded entries by key namespace prefix (comma-separated). Shorthand groups supported: - roles => [role, role.binding, perm.account, perm.role] - assets => [asset, `asset_def.total`] - `all_assets` => [asset, `asset_def.total`, `asset_def.detail`] - metadata => [account.detail, domain.detail, nft.detail, `asset_def.detail`] - `all_meta` => [account.detail, domain.detail, nft.detail, `asset_def.detail`] (alias of metadata) - perm | perms | permissions => [perm.account, perm.role] Examples: "assets,metadata", "roles", "account.detail,domain.detail". Applied only with --decode; prefixes match the human-readable key labels.

   Matching on the identifier segment supports: - exact (e.g., `account.detail:sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB`) - partial substring (e.g., `account.detail:6cmzPVPX`) - glob wildcards `*` and `?` (e.g., `asset:rose#*#6cmz*`) - regex-like syntax `/.../` (treated as a glob pattern inside the slashes)
* `--fastpq-batches` — Include FASTPQ transition batches recorded in the witness when decoding (enabled by default)

  Default value: `true`
* `--no-fastpq-batches` — Disable FASTPQ batches to shrink the decoded output
* `--fastpq-parameter <NAME>` — Expected FASTPQ parameter set name; errors if batches use a different value

  Default value: `fastpq-lane-balanced`



## `iroha ops connect`

Connect diagnostics helpers (queue inspection, evidence export)

**Usage:** `iroha ops connect <COMMAND>`

###### **Subcommands:**

* `queue` — Queue inspection tooling



## `iroha ops connect queue`

Queue inspection tooling

**Usage:** `iroha ops connect queue <COMMAND>`

###### **Subcommands:**

* `inspect` — Inspect on-disk queue diagnostics for a Connect session



## `iroha ops connect queue inspect`

Inspect on-disk queue diagnostics for a Connect session

**Usage:** `iroha ops connect queue inspect [OPTIONS]`

###### **Options:**

* `--sid <SID>` — Connect session identifier (base64url, no padding). Required unless `--snapshot` is provided
* `--snapshot <SNAPSHOT>` — Path to an explicit snapshot JSON file (defaults to `<root>/<sid>/state.json`)
* `--root <ROOT>` — Root directory containing Connect queue state (defaults to `connect.queue.root` or `~/.iroha/connect`)
* `--metrics` — Include metrics summary derived from `metrics.ndjson`
* `--format <FORMAT>` — Output format for text mode (`table` or `json`).

   Ignored when `--output-format json` is used.

  Default value: `table`

  Possible values: `table`, `json`




## `iroha offline`

Inspect offline allowances and offline-to-online bundles

**Usage:** `iroha offline <COMMAND>`

###### **Subcommands:**

* `allowance` — Inspect offline allowances registered on-ledger
* `transfer` — Inspect pending offline-to-online transfer bundles
* `bundle` — Inspect offline bundle fixtures and aggregate proofs
* `summary` — Inspect derived counter summaries per offline certificate
* `revocation` — Inspect recorded verdict revocations
* `rejection` — Fetch offline rejection telemetry snapshots
* `qr` — Encode/decode QR stream frames for offline payloads
* `petal` — Encode/decode petal stream frames for offline payloads



## `iroha offline allowance`

Inspect offline allowances registered on-ledger

**Usage:** `iroha offline allowance <COMMAND>`

###### **Subcommands:**

* `list` — List all registered offline allowances
* `get` — Fetch a specific allowance by certificate id



## `iroha offline allowance list`

List all registered offline allowances

**Usage:** `iroha offline allowance list [OPTIONS]`

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



## `iroha offline allowance get`

Fetch a specific allowance by certificate id

**Usage:** `iroha offline allowance get --certificate-id <CERTIFICATE_ID>`

###### **Options:**

* `--certificate-id <CERTIFICATE_ID>` — Deterministic certificate identifier (hex)



## `iroha offline transfer`

Inspect pending offline-to-online transfer bundles

**Usage:** `iroha offline transfer <COMMAND>`

###### **Subcommands:**

* `list` — List all pending offline-to-online transfer bundles
* `get` — Fetch a specific transfer bundle by id
* `proof` — Generate a FASTPQ witness request for a bundle payload



## `iroha offline transfer list`

List all pending offline-to-online transfer bundles

**Usage:** `iroha offline transfer list [OPTIONS]`

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

  Possible values: `settled`, `rejected`, `archived`

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



## `iroha offline transfer get`

Fetch a specific transfer bundle by id

**Usage:** `iroha offline transfer get --bundle-id <BUNDLE_ID>`

###### **Options:**

* `--bundle-id <BUNDLE_ID>` — Deterministic bundle identifier (hex)



## `iroha offline transfer proof`

Generate a FASTPQ witness request for a bundle payload

**Usage:** `iroha offline transfer proof [OPTIONS] --bundle <PATH> --kind <KIND>`

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



## `iroha offline bundle`

Inspect offline bundle fixtures and aggregate proofs

**Usage:** `iroha offline bundle <COMMAND>`

###### **Subcommands:**

* `inspect` — Inspect offline bundle fixtures and compute Poseidon receipts roots



## `iroha offline bundle inspect`

Inspect offline bundle fixtures and compute Poseidon receipts roots

**Usage:** `iroha offline bundle inspect [OPTIONS] <PATH>...`

###### **Arguments:**

* `<PATH>` — Paths to offline bundle fixtures (JSON or Norito)

###### **Options:**

* `--encoding <ENCODING>` — Override the bundle encoding detection

  Default value: `auto`

  Possible values: `auto`, `json`, `norito`

* `--proofs` — Include aggregate proof byte counts and metadata keys



## `iroha offline summary`

Inspect derived counter summaries per offline certificate

**Usage:** `iroha offline summary <COMMAND>`

###### **Subcommands:**

* `list` — List counter summaries derived from wallet allowances
* `export` — Export counter summaries to a JSON digest for receiver sharing



## `iroha offline summary list`

List counter summaries derived from wallet allowances

**Usage:** `iroha offline summary list [OPTIONS]`

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



## `iroha offline summary export`

Export counter summaries to a JSON digest for receiver sharing

**Usage:** `iroha offline summary export [OPTIONS] --output <PATH>`

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



## `iroha offline revocation`

Inspect recorded verdict revocations

**Usage:** `iroha offline revocation <COMMAND>`

###### **Subcommands:**

* `list` — List recorded verdict revocations



## `iroha offline revocation list`

List recorded verdict revocations

**Usage:** `iroha offline revocation list [OPTIONS]`

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



## `iroha offline rejection`

Fetch offline rejection telemetry snapshots

**Usage:** `iroha offline rejection <COMMAND>`

###### **Subcommands:**

* `stats` — Fetch aggregated offline rejection counters



## `iroha offline rejection stats`

Fetch aggregated offline rejection counters

**Usage:** `iroha offline rejection stats [OPTIONS]`

###### **Options:**

* `--telemetry-profile <PROFILE>` — Optional telemetry profile header used when fetching stats



## `iroha offline qr`

Encode/decode QR stream frames for offline payloads

**Usage:** `iroha offline qr <COMMAND>`

###### **Subcommands:**

* `encode` — Encode a payload into QR stream frames
* `decode` — Decode QR stream frames into the original payload



## `iroha offline qr encode`

Encode a payload into QR stream frames

**Usage:** `iroha offline qr encode [OPTIONS] --input <FILE> --output <DIR>`

###### **Options:**

* `--input <FILE>` — Path to the payload bytes to encode
* `--output <DIR>` — Output directory for generated frames and artifacts
* `--payload-kind <PAYLOAD_KIND>` — Payload kind tag embedded in the envelope

  Default value: `unspecified`

  Possible values: `unspecified`, `offline-to-online-transfer`, `offline-spend-receipt`, `offline-envelope`

* `--chunk-size <CHUNK_SIZE>` — Chunk size in bytes

  Default value: `360`
* `--parity-group <PARITY_GROUP>` — Parity group size (0 disables parity frames)

  Default value: `0`
* `--ecc <ECC>` — QR error correction level

  Default value: `m`

  Possible values: `l`, `m`, `q`, `h`

* `--frame-encoding <FRAME_ENCODING>` — QR frame encoding mode

  Default value: `binary`

  Possible values: `binary`, `base64`

* `--dimension <DIMENSION>` — Rendered QR image size in pixels

  Default value: `512`
* `--format <FORMAT>` — Output format for rendered frames

  Default value: `frames`

  Possible values: `frames`, `svg`, `png`, `gif`, `apng`

* `--style <STYLE>` — Render style for preview images (ignored for --format frames)

  Default value: `mono`

  Possible values: `mono`, `sakura`, `sakura-wind`, `sakura-storm`

* `--fps <FPS>` — Frames per second for animated outputs

  Default value: `12`



## `iroha offline qr decode`

Decode QR stream frames into the original payload

**Usage:** `iroha offline qr decode [OPTIONS] --input-dir <DIR> --output <FILE>`

###### **Options:**

* `--input-dir <DIR>` — Directory containing raw frame bytes
* `--output <FILE>` — Output file for the decoded payload
* `--frame-encoding <FRAME_ENCODING>` — Frame encoding used in the input

  Default value: `binary`

  Possible values: `binary`, `base64`

* `--output-manifest <FILE>` — Optional JSON manifest output path



## `iroha offline petal`

Encode/decode petal stream frames for offline payloads

**Usage:** `iroha offline petal <COMMAND>`

###### **Subcommands:**

* `encode` — Encode a payload into petal stream frames
* `decode` — Decode petal stream frames into the original payload
* `eval-capture` — Evaluate decode robustness under simulated distant/moving capture
* `simulate-realtime` — Simulate live camera reading and decode frame-by-frame in real time
* `score-styles` — Score render styles with deterministic capture simulation and throughput metrics



## `iroha offline petal encode`

Encode a payload into petal stream frames

**Usage:** `iroha offline petal encode [OPTIONS] --input <FILE> --output <DIR>`

###### **Options:**

* `--input <FILE>` — Path to the payload bytes to encode
* `--output <DIR>` — Output directory for generated frames and artifacts
* `--payload-kind <PAYLOAD_KIND>` — Payload kind tag embedded in the envelope

  Default value: `unspecified`

  Possible values: `unspecified`, `offline-to-online-transfer`, `offline-spend-receipt`, `offline-envelope`

* `--chunk-size <CHUNK_SIZE>` — Chunk size in bytes

  Default value: `140`
* `--parity-group <PARITY_GROUP>` — Parity group size (0 disables parity frames)

  Default value: `0`
* `--grid-size <GRID_SIZE>` — Grid size in cells (0 selects automatic sizing)

  Default value: `0`
* `--border <BORDER>` — Border thickness in cells

  Default value: `1`
* `--anchor-size <ANCHOR_SIZE>` — Anchor size in cells

  Default value: `3`
* `--dimension <DIMENSION>` — Rendered frame size in pixels

  Default value: `512`
* `--format <FORMAT>` — Output format for rendered frames

  Default value: `frames`

  Possible values: `frames`, `png`, `gif`, `apng`

* `--fps <FPS>` — Frames per second for animated outputs

  Default value: `24`
* `--style <STYLE>` — Render style for preview images (ignored for --format frames)

  Default value: `sora-temple`

  Possible values: `sakura-wind`, `sora-temple`, `sora-temple-bold`, `sora-temple-minimal`, `sora-temple-radiant`, `sora-temple-command`, `sora-temple-aegis`, `sora-temple-ghost`

* `--channel <CHANNEL>` — Data channel used for data cells in rendered outputs

  Default value: `binary`

  Possible values: `binary`, `katakana-base94`

* `--katakana-preset <KATAKANA_PRESET>` — Katakana channel tuning preset used when grid/chunk are left at defaults

  Default value: `balanced`

  Possible values: `balanced`, `distance-safe`




## `iroha offline petal decode`

Decode petal stream frames into the original payload

**Usage:** `iroha offline petal decode [OPTIONS] --input-dir <DIR> --output <FILE>`

###### **Options:**

* `--input-dir <DIR>` — Directory containing PNG frames
* `--output <FILE>` — Output file for the decoded payload
* `--grid-size <GRID_SIZE>` — Grid size in cells (0 to auto-detect)

  Default value: `0`
* `--border <BORDER>` — Border thickness in cells

  Default value: `1`
* `--anchor-size <ANCHOR_SIZE>` — Anchor size in cells

  Default value: `3`
* `--output-manifest <FILE>` — Optional JSON manifest output path
* `--channel <CHANNEL>` — Data channel used by rendered frames

  Default value: `binary`

  Possible values: `binary`, `katakana-base94`




## `iroha offline petal eval-capture`

Evaluate decode robustness under simulated distant/moving capture

**Usage:** `iroha offline petal eval-capture [OPTIONS] --input-dir <DIR>`

###### **Options:**

* `--input-dir <DIR>` — Directory containing rendered PNG frames
* `--grid-size <GRID_SIZE>` — Grid size in cells (0 to auto-detect from pristine frames)

  Default value: `0`
* `--border <BORDER>` — Border thickness in cells

  Default value: `1`
* `--anchor-size <ANCHOR_SIZE>` — Anchor size in cells

  Default value: `3`
* `--channel <CHANNEL>` — Data channel used by rendered frames

  Default value: `binary`

  Possible values: `binary`, `katakana-base94`

* `--profile <PROFILE>` — Capture perturbation profile

  Default value: `default`

  Possible values: `default`, `aggressive`

* `--seed <SEED>` — Deterministic seed for perturbation sampling

  Default value: `42`
* `--trials-per-frame <TRIALS_PER_FRAME>` — Number of perturbation trials per frame (0 uses profile default)

  Default value: `0`
* `--min-success-ratio <MIN_SUCCESS_RATIO>` — Minimum successful decode ratio required to pass

  Default value: `0.95`
* `--output-report <FILE>` — Optional JSON report output path



## `iroha offline petal simulate-realtime`

Simulate live camera reading and decode frame-by-frame in real time

**Usage:** `iroha offline petal simulate-realtime [OPTIONS] --input-dir <DIR>`

###### **Options:**

* `--input-dir <DIR>` — Directory containing rendered PNG frames
* `--output-payload <FILE>` — Optional output file for the decoded payload
* `--output-report <FILE>` — Optional JSON report output path
* `--grid-size <GRID_SIZE>` — Grid size in cells (0 to auto-detect from the first frame)

  Default value: `0`
* `--border <BORDER>` — Border thickness in cells

  Default value: `1`
* `--anchor-size <ANCHOR_SIZE>` — Anchor size in cells

  Default value: `3`
* `--channel <CHANNEL>` — Data channel used by rendered frames

  Default value: `binary`

  Possible values: `binary`, `katakana-base94`

* `--profile <PROFILE>` — Capture perturbation profile used to emulate a moving camera read

  Default value: `default`

  Possible values: `default`, `aggressive`

* `--seed <SEED>` — Deterministic seed for capture perturbation sampling

  Default value: `42`
* `--simulate-fps <SIMULATE_FPS>` — Simulated camera frame rate used to compute timeline metrics

  Default value: `24`
* `--frame-limit <FRAME_LIMIT>` — Optional cap on number of frames to process from the input directory
* `--realtime-loops <REALTIME_LOOPS>` — Number of playback loops to simulate over the same frame set

  Default value: `1`
* `--disable-capture-perturbation` — Disable capture perturbation and decode pristine frames only
* `--allow-incomplete` — Allow incomplete stream reconstruction without returning an error



## `iroha offline petal score-styles`

Score render styles with deterministic capture simulation and throughput metrics

**Usage:** `iroha offline petal score-styles [OPTIONS] --input <FILE> --output-report <FILE>`

###### **Options:**

* `--input <FILE>` — Path to payload bytes used for style scoring
* `--output-report <FILE>` — JSON report path for scored styles
* `--style <STYLE>` — Styles to evaluate (repeat flag). Empty means the default temple style set

  Possible values: `sakura-wind`, `sora-temple`, `sora-temple-bold`, `sora-temple-minimal`, `sora-temple-radiant`, `sora-temple-command`, `sora-temple-aegis`, `sora-temple-ghost`

* `--payload-kind <PAYLOAD_KIND>` — Payload kind tag embedded in the envelope

  Default value: `unspecified`

  Possible values: `unspecified`, `offline-to-online-transfer`, `offline-spend-receipt`, `offline-envelope`

* `--chunk-size <CHUNK_SIZE>` — Chunk size in bytes

  Default value: `140`
* `--parity-group <PARITY_GROUP>` — Parity group size (0 disables parity frames)

  Default value: `0`
* `--grid-size <GRID_SIZE>` — Grid size in cells (0 selects automatic sizing)

  Default value: `0`
* `--border <BORDER>` — Border thickness in cells

  Default value: `1`
* `--anchor-size <ANCHOR_SIZE>` — Anchor size in cells

  Default value: `3`
* `--dimension <DIMENSION>` — Rendered frame size in pixels

  Default value: `512`
* `--fps <FPS>` — Frames per second used for effective throughput scoring

  Default value: `24`
* `--profile <PROFILE>` — Capture perturbation profile

  Default value: `default`

  Possible values: `default`, `aggressive`

* `--seed <SEED>` — Deterministic seed for perturbation sampling

  Default value: `42`
* `--trials-per-frame <TRIALS_PER_FRAME>` — Number of perturbation trials per frame (0 uses profile default)

  Default value: `0`
* `--min-success-ratio <MIN_SUCCESS_RATIO>` — Minimum capture success ratio used for the pass gate in the report

  Default value: `0.95`
* `--target-effective-bps <TARGET_EFFECTIVE_BPS>` — Target effective throughput used to normalize throughput scoring

  Default value: `3000`



## `iroha app`

App API helpers and product tooling

**Usage:** `iroha app <COMMAND>`

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
* `soracloud` — Soracloud deployment/control-plane helpers
* `social` — Social incentive helpers (viral follow rewards and escrows)
* `space-directory` — Space Directory helpers (UAID capability manifests)
* `kaigi` — Kaigi session helpers
* `sorafs` — SoraFS helpers (pin registry, aliases, replication orders, storage)
* `soracles` — Soracles helpers (evidence bundling)
* `sns` — Sora Name Service helpers (registrar + policy tooling)
* `alias` — Alias helpers (placeholder pipeline)
* `repo` — Repo settlement helpers
* `settlement` — Delivery-versus-payment and payment-versus-payment helpers



## `iroha app gov`

Governance helpers (app API convenience)

**Usage:** `iroha app gov <COMMAND>`

###### **Subcommands:**

* `deploy` — Deployment helpers (propose/meta/audit). Propose deployment of IVM bytecode
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



## `iroha app gov deploy`

Deployment helpers (propose/meta/audit). Propose deployment of IVM bytecode

**Usage:** `iroha app gov deploy <COMMAND>`

###### **Subcommands:**

* `propose` — Propose deployment of IVM bytecode by code/abi hash via governance (build-only; server returns instruction skeleton)
* `meta` — Build deploy metadata JSON for protected namespace admission
* `audit` — Audit stored manifests against governance proposals and code storage



## `iroha app gov deploy propose`

Propose deployment of IVM bytecode by code/abi hash via governance (build-only; server returns instruction skeleton)

**Usage:** `iroha app gov deploy propose [OPTIONS] --code-hash <CODE_HASH> --abi-hash <ABI_HASH>`

###### **Options:**

* `--contract-address <CONTRACT_ADDRESS>`
* `--contract-alias <CONTRACT_ALIAS>`
* `--code-hash <CODE_HASH>`
* `--abi-hash <ABI_HASH>`
* `--abi-version <ABI_VERSION>`

  Default value: `v1`
* `--window-lower <WINDOW_LOWER>` — Optional window lower bound (height)
* `--window-upper <WINDOW_UPPER>` — Optional window upper bound (height)
* `--mode <MODE>` — Optional voting mode for the referendum: Zk or Plain (defaults to server policy)

  Possible values: `Zk`, `Plain`




## `iroha app gov deploy meta`

Build deploy metadata JSON for protected namespace admission

**Usage:** `iroha app gov deploy meta [OPTIONS]`

###### **Options:**

* `--contract-address <CONTRACT_ADDRESS>`
* `--contract-alias <CONTRACT_ALIAS>`
* `--approver <ACCOUNT>` — Optional validator account IDs (canonical I105 account literals) authorizing the deployment alongside the authority



## `iroha app gov deploy audit`

Audit stored manifests against governance proposals and code storage

**Usage:** `iroha app gov deploy audit [OPTIONS]`

###### **Options:**

* `--contract-address <CONTRACT_ADDRESS>`
* `--contract-alias <CONTRACT_ALIAS>`



## `iroha app gov vote`

Submit a governance ballot; auto-detects referendum mode unless overridden

**Usage:** `iroha app gov vote [OPTIONS] --referendum-id <REFERENDUM_ID>`

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
* `--owner <OWNER>` — Owner account id for plain voting mode (canonical I105 account literal; must equal transaction authority)
* `--amount <AMOUNT>` — Locked amount for plain voting mode (string to preserve large integers)
* `--duration-blocks <DURATION_BLOCKS>` — Lock duration (in blocks) for plain voting mode
* `--direction <DIRECTION>` — Ballot direction for plain voting mode: Aye, Nay, or Abstain
* `--nullifier <NULLIFIER>` — Optional 32-byte nullifier hint for ZK ballots (hex)



## `iroha app gov proposal`

Proposal helpers

**Usage:** `iroha app gov proposal <COMMAND>`

###### **Subcommands:**

* `get` — Get a governance proposal by id (hex)



## `iroha app gov proposal get`

Get a governance proposal by id (hex)

**Usage:** `iroha app gov proposal get --id <ID_HEX>`

###### **Options:**

* `--id <ID_HEX>`



## `iroha app gov locks`

Lock helpers

**Usage:** `iroha app gov locks <COMMAND>`

###### **Subcommands:**

* `get` — Get locks for a referendum id



## `iroha app gov locks get`

Get locks for a referendum id

**Usage:** `iroha app gov locks get --referendum-id <REFERENDUM_ID>`

###### **Options:**

* `--referendum-id <REFERENDUM_ID>`



## `iroha app gov council`

Get current sortition council or manage council VRF flows

**Usage:** `iroha app gov council [COMMAND]`

###### **Subcommands:**

* `derive-vrf` — 
* `persist` — 
* `gen-vrf` — 
* `derive-and-persist` — 
* `replace` — 



## `iroha app gov council derive-vrf`

**Usage:** `iroha app gov council derive-vrf [OPTIONS]`

###### **Options:**

* `--committee-size <N>` — Committee size to select
* `--alternate-size <N>` — Optional alternates to keep
* `--epoch <EPOCH>` — Optional epoch override
* `--candidate <CANDIDATES>` — Candidate spec: "`account_id,variant,pk_b64,proof_b64`"; repeatable
* `--candidates-file <PATH>` — Path to a JSON file with an array of candidates ({`account_id`, variant, `pk_b64`, `proof_b64`})



## `iroha app gov council persist`

**Usage:** `iroha app gov council persist [OPTIONS] --candidates-file <PATH> --authority <AUTHORITY> --private-key <HEX>`

###### **Options:**

* `--committee-size <COMMITTEE_SIZE>` — Committee size to select (top-k by VRF output)
* `--alternate-size <ALTERNATE_SIZE>` — Optional number of alternates to keep (defaults to committee size)
* `--epoch <EPOCH>` — Optional epoch override; defaults to `height/TERM_BLOCKS`
* `--candidates-file <PATH>` — Path to JSON file with candidates: [{ `account_id`, variant: Normal|Small, `pk_b64`, `proof_b64` }, ...]
* `--authority <AUTHORITY>` — Authority `AccountId` for signing (canonical I105 account literal)
* `--private-key <HEX>` — Private key (hex) for signing



## `iroha app gov council gen-vrf`

**Usage:** `iroha app gov council gen-vrf [OPTIONS] --chain-id <CHAIN_ID>`

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
* `--account-prefix <ACCOUNT_PREFIX>` — Seed prefix used when deriving deterministic candidate account keys

  Default value: `node`
* `--domain <DOMAIN>` — Fully qualified domain literal used in generated account aliases

  Default value: `wonderland.universal`
* `--out <OUT>` — Output path; if omitted, prints JSON to stdout
* `--from-audit` — Fetch `seed/epoch/chain_id` from /v1/gov/council/audit (overrides --epoch/--beacon-hex when set)

  Default value: `false`



## `iroha app gov council derive-and-persist`

**Usage:** `iroha app gov council derive-and-persist [OPTIONS] --candidates-file <PATH> --authority <AUTHORITY> --private-key <HEX>`

###### **Options:**

* `--committee-size <COMMITTEE_SIZE>` — Committee size to select (top-k by VRF output)
* `--alternate-size <ALTERNATE_SIZE>` — Optional number of alternates to keep (defaults to committee size)
* `--epoch <EPOCH>` — Optional epoch override; defaults to `height/TERM_BLOCKS` (server-side)
* `--candidates-file <PATH>` — Path to JSON file with candidates: [{ `account_id`, variant: Normal|Small, `pk_b64`, `proof_b64` }, ...]
* `--authority <AUTHORITY>` — Authority `AccountId` for signing (canonical I105 account literal)
* `--private-key <HEX>` — Private key (hex) for signing
* `--wait` — Wait for `CouncilPersisted` event and verify via /v1/gov/council/current

  Default value: `false`



## `iroha app gov council replace`

**Usage:** `iroha app gov council replace [OPTIONS] --missing <MISSING> --authority <AUTHORITY> --private-key <HEX>`

###### **Options:**

* `--missing <MISSING>` — Account id of the member to replace (canonical I105 account literal)
* `--epoch <EPOCH>` — Optional epoch override; defaults to the latest persisted epoch
* `--authority <AUTHORITY>` — Authority `AccountId` for signing (canonical I105 account literal)
* `--private-key <HEX>` — Private key (hex) for signing



## `iroha app gov unlock`

Unlock helpers (expired lock stats)

**Usage:** `iroha app gov unlock <COMMAND>`

###### **Subcommands:**

* `stats` — Show governance unlock sweep stats (expired locks at current height)



## `iroha app gov unlock stats`

Show governance unlock sweep stats (expired locks at current height)

**Usage:** `iroha app gov unlock stats`



## `iroha app gov referendum`

Referendum helpers

**Usage:** `iroha app gov referendum <COMMAND>`

###### **Subcommands:**

* `get` — Get a referendum by id



## `iroha app gov referendum get`

Get a referendum by id

**Usage:** `iroha app gov referendum get --referendum-id <REFERENDUM_ID>`

###### **Options:**

* `--referendum-id <REFERENDUM_ID>`



## `iroha app gov tally`

Tally helpers

**Usage:** `iroha app gov tally <COMMAND>`

###### **Subcommands:**

* `get` — Get a tally snapshot by referendum id



## `iroha app gov tally get`

Get a tally snapshot by referendum id

**Usage:** `iroha app gov tally get --referendum-id <REFERENDUM_ID>`

###### **Options:**

* `--referendum-id <REFERENDUM_ID>`



## `iroha app gov finalize`

Build a finalize transaction for a referendum (server returns instruction skeleton)

**Usage:** `iroha app gov finalize --referendum-id <REFERENDUM_ID> --proposal-id <ID_HEX>`

###### **Options:**

* `--referendum-id <REFERENDUM_ID>` — Referendum id
* `--proposal-id <ID_HEX>` — Proposal id (hex 64)



## `iroha app gov enact`

Build an enactment transaction for an approved proposal

**Usage:** `iroha app gov enact [OPTIONS] --proposal-id <ID_HEX>`

###### **Options:**

* `--proposal-id <ID_HEX>` — Proposal id (hex 64)
* `--preimage-hash <PREIMAGE_HASH>` — Optional preimage hash (hex 64)
* `--window-lower <WINDOW_LOWER>` — Optional window lower bound (height)
* `--window-upper <WINDOW_UPPER>` — Optional window upper bound (height)



## `iroha app gov protected`

Protected namespace helpers

**Usage:** `iroha app gov protected <COMMAND>`

###### **Subcommands:**

* `set` — Set protected namespaces (custom parameter `gov_protected_namespaces`)
* `apply` — Apply protected namespaces on the server (requires API token if configured)
* `get` — Get protected namespaces (custom parameter `gov_protected_namespaces`)



## `iroha app gov protected set`

Set protected namespaces (custom parameter `gov_protected_namespaces`)

**Usage:** `iroha app gov protected set --namespaces <NAMESPACES>`

###### **Options:**

* `--namespaces <NAMESPACES>` — Comma-separated namespaces (e.g., apps,system)



## `iroha app gov protected apply`

Apply protected namespaces on the server (requires API token if configured)

**Usage:** `iroha app gov protected apply --namespaces <NAMESPACES>`

###### **Options:**

* `--namespaces <NAMESPACES>` — Comma-separated namespaces (e.g., apps,system)



## `iroha app gov protected get`

Get protected namespaces (custom parameter `gov_protected_namespaces`)

**Usage:** `iroha app gov protected get`



## `iroha app contracts`

Contracts helpers (code storage)

**Usage:** `iroha app contracts <COMMAND>`

###### **Subcommands:**

* `code` — Contract code helpers
* `alias` — Contract alias helpers
* `deploy` — Deploy compiled `.to` code via Torii (POST /v1/contracts/deploy)
* `derive-address` — Derive a canonical contract address locally from authority, deploy nonce, and dataspace
* `call` — Submit a contract call through Torii (POST /v1/contracts/call)
* `view` — Execute a read-only contract view through Torii (POST /v1/contracts/view)
* `debug-view` — Execute a read-only contract view locally against compiled bytecode and optional fixtures
* `debug-call` — Execute a public contract entrypoint locally against compiled bytecode and optional fixtures
* `manifest` — Contract manifest helpers
* `simulate` — Run an offline simulation of IVM bytecode to see the queued ISIs and header metadata



## `iroha app contracts code`

Contract code helpers

**Usage:** `iroha app contracts code <COMMAND>`

###### **Subcommands:**

* `get` — Fetch on-chain contract code bytes by code hash and write to a file



## `iroha app contracts code get`

Fetch on-chain contract code bytes by code hash and write to a file

**Usage:** `iroha app contracts code get --code-hash <HEX64> --out <PATH>`

###### **Options:**

* `--code-hash <HEX64>` — Hex-encoded 32-byte code hash (0x optional)
* `--out <PATH>` — Output path to write the `.to` bytes



## `iroha app contracts alias`

Contract alias helpers

**Usage:** `iroha app contracts alias <COMMAND>`

###### **Subcommands:**

* `lease` — Lease or renew an on-chain contract alias for a contract address
* `release` — Release the current on-chain alias binding for a contract address
* `resolve` — Resolve an on-chain contract alias to its current canonical contract address



## `iroha app contracts alias lease`

Lease or renew an on-chain contract alias for a contract address

**Usage:** `iroha app contracts alias lease [OPTIONS] --contract-address <CONTRACT_ADDRESS> --contract-alias <CONTRACT_ALIAS>`

###### **Options:**

* `--contract-address <CONTRACT_ADDRESS>` — Canonical contract address to bind
* `--contract-alias <CONTRACT_ALIAS>` — Alias literal in `name::domain.dataspace` or `name::dataspace` format
* `--lease-expiry-ms <LEASE_EXPIRY_MS>` — Optional lease expiry timestamp in unix milliseconds. Omit for a permanent binding



## `iroha app contracts alias release`

Release the current on-chain alias binding for a contract address

**Usage:** `iroha app contracts alias release --contract-address <CONTRACT_ADDRESS>`

###### **Options:**

* `--contract-address <CONTRACT_ADDRESS>` — Canonical contract address whose alias binding should be cleared



## `iroha app contracts alias resolve`

Resolve an on-chain contract alias to its current canonical contract address

**Usage:** `iroha app contracts alias resolve <CONTRACT_ALIAS>`

###### **Arguments:**

* `<CONTRACT_ALIAS>` — Alias literal in `name::domain.dataspace` or `name::dataspace` format



## `iroha app contracts deploy`

Deploy compiled `.to` code via Torii (POST /v1/contracts/deploy)

**Usage:** `iroha app contracts deploy [OPTIONS] --authority <AUTHORITY> --private-key <HEX>`

###### **Options:**

* `--authority <AUTHORITY>` — Authority account identifier (canonical I105 account literal)
* `--dataspace <DATASPACE>` — Target dataspace alias for public address-first deploys (defaults to `universal`)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--code-file <CODE_FILE>` — Path to compiled `.to` file (mutually exclusive with --code-b64)
* `--code-b64 <CODE_B64>` — Base64-encoded code (mutually exclusive with --code-file)
* `--wait` — Poll `/v1/pipeline/transactions/status` until the transaction reaches a stop state
* `--timeout-ms <TIMEOUT_MS>` — Maximum time to wait before failing

  Default value: `30000`
* `--poll-interval-ms <POLL_INTERVAL_MS>` — Poll interval used while waiting

  Default value: `500`
* `--terminal-status <TERMINAL_STATUSES>` — Stop when the pipeline reaches any of these statuses. Applied, rejected, and expired always stop

  Possible values: `queued`, `approved`, `committed`, `applied`, `rejected`, `expired`




## `iroha app contracts derive-address`

Derive a canonical contract address locally from authority, deploy nonce, and dataspace

**Usage:** `iroha app contracts derive-address [OPTIONS] --authority <AUTHORITY> --deploy-nonce <DEPLOY_NONCE> --chain-discriminant <CHAIN_DISCRIMINANT>`

###### **Options:**

* `--authority <AUTHORITY>` — Authority account identifier (canonical I105 account literal)
* `--dataspace <DATASPACE>` — Target dataspace alias or numeric dataspace id (defaults to `universal`)

  Default value: `universal`
* `--deploy-nonce <DEPLOY_NONCE>` — Successful deploy nonce consumed for address derivation
* `--chain-discriminant <CHAIN_DISCRIMINANT>` — Explicit chain discriminant used for Bech32m contract-address derivation
* `--dataspace-id <DATASPACE_ID>` — Optional numeric dataspace id override for non-default dataspaces



## `iroha app contracts call`

Submit a contract call through Torii (POST /v1/contracts/call)

**Usage:** `iroha app contracts call [OPTIONS]`

###### **Options:**

* `--authority <AUTHORITY>` — Authority account identifier. Defaults to the configured client authority
* `--private-key <HEX>` — Hex-encoded private key override used to sign and submit the call directly
* `--scaffold-only` — Request an unsigned transaction scaffold instead of direct submission
* `--simulate` — Simulate the contract call locally on Torii without submitting a transaction
* `--entrypoint <ENTRYPOINT>` — Optional contract entrypoint selector (defaults to `main`)
* `--gas-asset-id <GAS_ASSET_ID>` — Optional gas asset id forwarded to transaction metadata
* `--fee-sponsor <FEE_SPONSOR>` — Optional fee sponsor account charged for gas/fees when supported
* `--gas-limit <GAS_LIMIT>` — Gas limit metadata forwarded to the contract call

  Default value: `100000`
* `--contract-address <CONTRACT_ADDRESS>` — Canonical contract address
* `--contract-alias <CONTRACT_ALIAS>` — On-chain contract alias (`name::domain.dataspace` or `name::dataspace`)
* `--payload-json <JSON>` — Inline Norito JSON payload object or value
* `--payload-file <PATH>` — File containing a Norito JSON payload object or value
* `--wait` — Poll `/v1/pipeline/transactions/status` until the transaction reaches a stop state
* `--timeout-ms <TIMEOUT_MS>` — Maximum time to wait before failing

  Default value: `30000`
* `--poll-interval-ms <POLL_INTERVAL_MS>` — Poll interval used while waiting

  Default value: `500`
* `--terminal-status <TERMINAL_STATUSES>` — Stop when the pipeline reaches any of these statuses. Applied, rejected, and expired always stop

  Possible values: `queued`, `approved`, `committed`, `applied`, `rejected`, `expired`




## `iroha app contracts view`

Execute a read-only contract view through Torii (POST /v1/contracts/view)

**Usage:** `iroha app contracts view [OPTIONS]`

###### **Options:**

* `--authority <AUTHORITY>` — Authority account identifier used as the read context. Defaults to the configured client authority
* `--entrypoint <ENTRYPOINT>` — Optional contract entrypoint selector (defaults to `main`)
* `--gas-limit <GAS_LIMIT>` — Gas limit applied to the local view execution

  Default value: `100000`
* `--contract-address <CONTRACT_ADDRESS>` — Canonical contract address
* `--contract-alias <CONTRACT_ALIAS>` — On-chain contract alias (`name::domain.dataspace` or `name::dataspace`)
* `--payload-json <JSON>` — Inline Norito JSON payload object or value
* `--payload-file <PATH>` — File containing a Norito JSON payload object or value



## `iroha app contracts debug-view`

Execute a read-only contract view locally against compiled bytecode and optional fixtures

**Usage:** `iroha app contracts debug-view [OPTIONS]`

###### **Options:**

* `--authority <AUTHORITY>` — Authority account identifier used as the local read context. Defaults to the configured client authority
* `--code-file <CODE_FILE>` — Path to compiled `.to` file (mutually exclusive with --code-b64)
* `--code-b64 <CODE_B64>` — Base64-encoded code (mutually exclusive with --code-file)
* `--entrypoint <ENTRYPOINT>` — Optional contract entrypoint selector (defaults to `main`)
* `--gas-limit <GAS_LIMIT>` — Gas limit applied to the local view execution

  Default value: `100000`
* `--source-file <PATH>` — Optional source file used to render snippet context for trapped debug locations
* `--accounts-json <JSON>` — Optional JSON array of canonical account ids available to iterator helpers
* `--accounts-file <PATH>` — File containing a JSON array of canonical account ids available to iterator helpers
* `--durable-state-json <JSON>` — Optional JSON object mapping durable state keys to encoded values (`0x...` hex or base64)
* `--durable-state-file <PATH>` — File containing a JSON object mapping durable state keys to encoded values (`0x...` hex or base64)
* `--payload-json <JSON>` — Inline Norito JSON payload object or value
* `--payload-file <PATH>` — File containing a Norito JSON payload object or value



## `iroha app contracts debug-call`

Execute a public contract entrypoint locally against compiled bytecode and optional fixtures

**Usage:** `iroha app contracts debug-call [OPTIONS]`

###### **Options:**

* `--authority <AUTHORITY>` — Authority account identifier used as the local call context. Defaults to the configured client authority
* `--code-file <CODE_FILE>` — Path to compiled `.to` file (mutually exclusive with --code-b64)
* `--code-b64 <CODE_B64>` — Base64-encoded code (mutually exclusive with --code-file)
* `--entrypoint <ENTRYPOINT>` — Optional contract entrypoint selector (defaults to `main`)
* `--gas-limit <GAS_LIMIT>` — Gas limit applied to the local call execution

  Default value: `100000`
* `--source-file <PATH>` — Optional source file used to render snippet context for trapped debug locations
* `--accounts-json <JSON>` — Optional JSON array of canonical account ids available to iterator helpers
* `--accounts-file <PATH>` — File containing a JSON array of canonical account ids available to iterator helpers
* `--durable-state-json <JSON>` — Optional JSON object mapping durable state keys to encoded values (`0x...` hex or base64)
* `--durable-state-file <PATH>` — File containing a JSON object mapping durable state keys to encoded values (`0x...` hex or base64)
* `--payload-json <JSON>` — Inline Norito JSON payload object or value
* `--payload-file <PATH>` — File containing a Norito JSON payload object or value



## `iroha app contracts manifest`

Contract manifest helpers

**Usage:** `iroha app contracts manifest <COMMAND>`

###### **Subcommands:**

* `get` — Fetch on-chain contract manifest by code hash and either print or save (if --out is provided)
* `build` — Inspect the manifest embedded in compiled bytecode (with optional signing)



## `iroha app contracts manifest get`

Fetch on-chain contract manifest by code hash and either print or save (if --out is provided)

**Usage:** `iroha app contracts manifest get [OPTIONS] --code-hash <HEX64>`

###### **Options:**

* `--code-hash <HEX64>` — Hex-encoded 32-byte code hash (0x optional)
* `--out <PATH>` — Optional output path; if provided, writes JSON manifest to file, otherwise prints to stdout



## `iroha app contracts manifest build`

Inspect the manifest embedded in compiled bytecode (with optional signing)

**Usage:** `iroha app contracts manifest build [OPTIONS]`

###### **Options:**

* `--code-file <CODE_FILE>` — Path to compiled `.to` file (mutually exclusive with --code-b64)
* `--code-b64 <CODE_B64>` — Base64-encoded code (mutually exclusive with --code-file)
* `--sign-with <HEX>` — Hex-encoded private key for signing the manifest (optional)
* `--out <PATH>` — Optional output path; if omitted, prints to stdout



## `iroha app contracts simulate`

Run an offline simulation of IVM bytecode to see the queued ISIs and header metadata

**Usage:** `iroha app contracts simulate [OPTIONS] --authority <AUTHORITY> --private-key <HEX> --gas-limit <GAS_LIMIT>`

###### **Options:**

* `--authority <AUTHORITY>` — Authority account identifier (canonical I105 account literal)
* `--private-key <HEX>` — Hex-encoded private key used to sign the simulated transaction
* `--code-file <CODE_FILE>` — Path to compiled `.to` file (mutually exclusive with --code-b64)
* `--code-b64 <CODE_B64>` — Base64-encoded code (mutually exclusive with --code-file)
* `--gas-limit <GAS_LIMIT>` — Required `gas_limit` metadata to include in the simulated transaction
* `--contract-address <CONTRACT_ADDRESS>` — Canonical contract address
* `--contract-alias <CONTRACT_ALIAS>` — On-chain contract alias (`name::domain.dataspace` or `name::dataspace`)



## `iroha app zk`

Zero-knowledge helpers (roots, etc.)

**Usage:** `iroha app zk <COMMAND>`

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
* `ivm` — IVM prove helpers (non-consensus, app API)
* `vote` — ZK Vote helpers (tally)
* `envelope` — Encode a confidential encrypted payload (memo) into Norito bytes/base64



## `iroha app zk roots`

Get recent shielded roots for an asset (JSON). Posts to /v1/zk/roots

**Usage:** `iroha app zk roots [OPTIONS] --asset-id <ASSET_ID>`

###### **Options:**

* `--asset-id <ASSET_ID>` — Canonical unprefixed Base58 `AssetDefinitionId`
* `--max <MAX>` — Maximum number of roots to return (0 = server cap)

  Default value: `0`



## `iroha app zk verify`

Verify a ZK proof by posting an `OpenVerifyEnvelope` (Norito) or a JSON DTO to /v1/zk/verify

**Usage:** `iroha app zk verify [OPTIONS]`

###### **Options:**

* `--norito <PATH>` — Path to Norito-encoded `OpenVerifyEnvelope` bytes (mutually exclusive with --json)
* `--json <PATH>` — Path to a JSON DTO describing the proof (backend, proof, vk) (mutually exclusive with --norito)



## `iroha app zk submit-proof`

Submit a ZK proof envelope for later reference/inspection. Posts to /v1/zk/submit-proof

**Usage:** `iroha app zk submit-proof [OPTIONS]`

###### **Options:**

* `--norito <PATH>` — Path to Norito-encoded proof envelope bytes (mutually exclusive with --json)
* `--json <PATH>` — Path to a JSON DTO describing the proof (backend, proof, vk) (mutually exclusive with --norito)



## `iroha app zk verify-batch`

Verify a batch of ZK `OpenVerify` envelopes (Norito vector) via /v1/zk/verify-batch

**Usage:** `iroha app zk verify-batch [OPTIONS]`

###### **Options:**

* `--norito <PATH>` — Path to a Norito-encoded Vec<OpenVerifyEnvelope> (mutually exclusive with --json)
* `--json <PATH>` — Path to a JSON array of base64-encoded Norito `OpenVerifyEnvelope` items (mutually exclusive with --norito)



## `iroha app zk schema-hash`

Compute the Blake2b-32 hash required for `public_inputs_schema_hash` and print it

**Usage:** `iroha app zk schema-hash [OPTIONS]`

###### **Options:**

* `--norito <PATH>` — Path to a Norito-encoded `OpenVerifyEnvelope`
* `--public-inputs-hex <HEX>` — Hex-encoded public inputs (when not using --norito)



## `iroha app zk attachments`

Manage ZK attachments in the app API

**Usage:** `iroha app zk attachments <COMMAND>`

###### **Subcommands:**

* `upload` — Upload a file as an attachment. Returns JSON metadata
* `list` — List stored attachments (JSON array of metadata)
* `get` — Download an attachment by id to a file
* `delete` — Delete an attachment by id
* `cleanup` — Cleanup attachments by filters (age/content-type/ids). Deletes individually via API



## `iroha app zk attachments upload`

Upload a file as an attachment. Returns JSON metadata

**Usage:** `iroha app zk attachments upload [OPTIONS] --file <PATH>`

###### **Options:**

* `--file <PATH>` — Path to the file to upload
* `--content-type <MIME>` — Content-Type to send with the file

  Default value: `application/octet-stream`



## `iroha app zk attachments list`

List stored attachments (JSON array of metadata)

**Usage:** `iroha app zk attachments list`



## `iroha app zk attachments get`

Download an attachment by id to a file

**Usage:** `iroha app zk attachments get --id <ID> --out <PATH>`

###### **Options:**

* `--id <ID>` — Attachment id (hex)
* `--out <PATH>` — Output path to write the downloaded bytes



## `iroha app zk attachments delete`

Delete an attachment by id

**Usage:** `iroha app zk attachments delete --id <ID>`

###### **Options:**

* `--id <ID>` — Attachment id (hex)



## `iroha app zk attachments cleanup`

Cleanup attachments by filters (age/content-type/ids). Deletes individually via API

**Usage:** `iroha app zk attachments cleanup [OPTIONS]`

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



## `iroha app zk register-asset`

Register a ZK-capable asset (Hybrid mode) with policy and VK ids

**Usage:** `iroha app zk register-asset [OPTIONS] --asset <ASSET_ID>`

###### **Options:**

* `--asset <ASSET_ID>` — Canonical unprefixed Base58 `AssetDefinitionId`
* `--allow-shield` — Allow shielding from public to shielded (default: true)

  Default value: `true`
* `--allow-unshield` — Allow unshielding from shielded to public (default: true)

  Default value: `true`
* `--vk-transfer <BACKEND:NAME>` — Verifying key id for private transfers (format: `<backend>:<name>`, e.g., `halo2/ipa:vk_transfer`)
* `--vk-unshield <BACKEND:NAME>` — Verifying key id for unshield proofs (format: `<backend>:<name>`)
* `--vk-shield <BACKEND:NAME>` — Verifying key id for shield proofs (optional; format: `<backend>:<name>`)



## `iroha app zk shield`

Shield public funds into a shielded ledger (demo flow)

**Usage:** `iroha app zk shield [OPTIONS] --asset <ASSET_ID> --from <ACCOUNT_ID> --amount <AMOUNT> --note-commitment <HEX32>`

###### **Options:**

* `--asset <ASSET_ID>` — Canonical unprefixed Base58 `AssetDefinitionId`
* `--from <ACCOUNT_ID>` — Account identifier to debit (canonical I105 account literal)
* `--amount <AMOUNT>` — Public amount to debit
* `--note-commitment <HEX32>` — Output note commitment (hex, 64 chars)
* `--enc-payload <PATH>` — Encrypted recipient payload envelope (Norito bytes). Optional; empty if not provided
* `--ephemeral-pubkey <HEX32>` — Ephemeral public key for encrypted payload (hex, 64 chars)
* `--nonce-hex <HEX24>` — XChaCha20-Poly1305 nonce for encrypted payload (hex, 48 chars)
* `--ciphertext-b64 <BASE64>` — Ciphertext payload (base64). Includes Poly1305 authentication tag



## `iroha app zk unshield`

Unshield funds from shielded ledger to public (demo flow)

**Usage:** `iroha app zk unshield [OPTIONS] --asset <ASSET_ID> --to <ACCOUNT_ID> --amount <AMOUNT> --inputs <HEX32[,HEX32,...]> --proof-json <PATH>`

###### **Options:**

* `--asset <ASSET_ID>` — Canonical unprefixed Base58 `AssetDefinitionId`
* `--to <ACCOUNT_ID>` — Recipient account identifier to credit (canonical I105 account literal)
* `--amount <AMOUNT>` — Public amount to credit
* `--inputs <HEX32[,HEX32,...]>` — Spent nullifiers (comma-separated list of 64-hex strings)
* `--proof-json <PATH>` — Proof attachment JSON file describing { backend, `proof_b64`, `vk_ref{backend,name}`, `vk_inline{backend,bytes_b64}`, optional `vk_commitment_hex` }
* `--root-hint <HEX32>` — Optional Merkle root hint (hex, 64 chars)



## `iroha app zk vk`

Verifying-key registry lifecycle (register/update/deprecate/get)

**Usage:** `iroha app zk vk <COMMAND>`

###### **Subcommands:**

* `register` — Register a verifying key record (signed transaction via Torii app API)
* `update` — Update an existing verifying key record (version must increase)
* `get` — Get a verifying key record by backend and name



## `iroha app zk vk register`

Register a verifying key record (signed transaction via Torii app API)

**Usage:** `iroha app zk vk register --json <PATH>`

###### **Options:**

* `--json <PATH>` — Path to a JSON DTO file for register (authority, `private_key`, backend, name, version, optional `vk_bytes` (base64) or `commitment_hex`)



## `iroha app zk vk update`

Update an existing verifying key record (version must increase)

**Usage:** `iroha app zk vk update --json <PATH>`

###### **Options:**

* `--json <PATH>` — Path to a JSON DTO file for update (authority, `private_key`, backend, name, version, optional `vk_bytes` or `commitment_hex`)



## `iroha app zk vk get`

Get a verifying key record by backend and name

**Usage:** `iroha app zk vk get --backend <BACKEND> --name <NAME>`

###### **Options:**

* `--backend <BACKEND>` — Backend identifier (e.g., "halo2/ipa")
* `--name <NAME>` — Verifying key name



## `iroha app zk proofs`

Inspect proof registry (list/count/get)

**Usage:** `iroha app zk proofs <COMMAND>`

###### **Subcommands:**

* `list` — List proof records maintained by Torii
* `count` — Count proof records matching the filters
* `get` — Fetch a proof record by backend and proof hash (hex)
* `retention` — Inspect proof retention configuration and live counters
* `prune` — Submit a pruning transaction to enforce proof retention immediately



## `iroha app zk proofs list`

List proof records maintained by Torii

**Usage:** `iroha app zk proofs list [OPTIONS]`

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



## `iroha app zk proofs count`

Count proof records matching the filters

**Usage:** `iroha app zk proofs count [OPTIONS]`

###### **Options:**

* `--backend <BACKEND>` — Filter by backend identifier (e.g., `halo2/ipa`)
* `--status <STATUS>` — Filter by verification status (`Submitted`, `Verified`, `Rejected`)
* `--has-tag <TAG>` — Require a ZK1 TLV tag (4 ASCII characters, e.g., `PROF`)
* `--verified-from-height <HEIGHT>` — Minimum verification height (inclusive)
* `--verified-until-height <HEIGHT>` — Maximum verification height (inclusive)
* `--limit <LIMIT>` — Limit result size (server caps at 1000)
* `--offset <OFFSET>` — Offset for server-side pagination
* `--order <ORDER>` — Sort order (`asc` or `desc`) by verification height



## `iroha app zk proofs get`

Fetch a proof record by backend and proof hash (hex)

**Usage:** `iroha app zk proofs get --backend <BACKEND> --hash <HASH>`

###### **Options:**

* `--backend <BACKEND>` — Backend identifier (e.g., `halo2/ipa`)
* `--hash <HASH>` — Proof hash (hex, with or without `0x` prefix)



## `iroha app zk proofs retention`

Inspect proof retention configuration and live counters

**Usage:** `iroha app zk proofs retention`



## `iroha app zk proofs prune`

Submit a pruning transaction to enforce proof retention immediately

**Usage:** `iroha app zk proofs prune [OPTIONS]`

###### **Options:**

* `--backend <BACKEND>` — Restrict pruning to a single backend (e.g., `halo2/ipa`). Omit to prune all backends



## `iroha app zk prover`

Inspect background prover reports (list/get/delete)

**Usage:** `iroha app zk prover <COMMAND>`

###### **Subcommands:**

* `reports` — Manage prover reports



## `iroha app zk prover reports`

Manage prover reports

**Usage:** `iroha app zk prover reports <COMMAND>`

###### **Subcommands:**

* `list` — List available prover reports (JSON array)
* `get` — Get a single prover report by id (JSON)
* `delete` — Delete a prover report by id
* `cleanup` — Cleanup reports in bulk (apply filters, delete matches)
* `count` — Count reports matching filters (server-side)



## `iroha app zk prover reports list`

List available prover reports (JSON array)

**Usage:** `iroha app zk prover reports list [OPTIONS]`

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



## `iroha app zk prover reports get`

Get a single prover report by id (JSON)

**Usage:** `iroha app zk prover reports get --id <ID>`

###### **Options:**

* `--id <ID>` — Report id (attachment id)



## `iroha app zk prover reports delete`

Delete a prover report by id

**Usage:** `iroha app zk prover reports delete --id <ID>`

###### **Options:**

* `--id <ID>` — Report id (attachment id)



## `iroha app zk prover reports cleanup`

Cleanup reports in bulk (apply filters, delete matches)

**Usage:** `iroha app zk prover reports cleanup [OPTIONS]`

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



## `iroha app zk prover reports count`

Count reports matching filters (server-side)

**Usage:** `iroha app zk prover reports count [OPTIONS]`

###### **Options:**

* `--ok-only` — Show only successful reports
* `--failed-only` — Show only failed reports
* `--errors-only` — Alias for failed-only (errors have ok=false)
* `--id <ID>` — Filter by exact id (hex)
* `--content-type <MIME>` — Filter by content-type substring (e.g., application/x-norito)
* `--has-tag <TAG>` — Filter reports that contain a ZK1 tag (e.g., PROF, IPAK)
* `--since-ms <MS>` — Only reports with `processed_ms` >= this value (server-side)
* `--before-ms <MS>` — Only reports with `processed_ms` <= this value (server-side)



## `iroha app zk ivm`

IVM prove helpers (non-consensus, app API)

**Usage:** `iroha app zk ivm <COMMAND>`

###### **Subcommands:**

* `derive` — Derive an `IvmProved` payload via `/v1/zk/ivm/derive`
* `prove` — Submit a prove job for an `IvmProved` payload via `/v1/zk/ivm/prove`
* `get` — Get a prove job status via `/v1/zk/ivm/prove/{job_id}`
* `delete` — Delete a prove job via `/v1/zk/ivm/prove/{job_id}`
* `derive-pk` — Derive a proving key (.pk) from verifying key bytes (.vk) for the Halo2 IPA IVM bind circuit



## `iroha app zk ivm derive`

Derive an `IvmProved` payload via `/v1/zk/ivm/derive`

**Usage:** `iroha app zk ivm derive --json <PATH>`

###### **Options:**

* `--json <PATH>` — Path to a JSON request DTO `{ vk_ref, authority, metadata, bytecode }`



## `iroha app zk ivm prove`

Submit a prove job for an `IvmProved` payload via `/v1/zk/ivm/prove`

**Usage:** `iroha app zk ivm prove [OPTIONS] --json <PATH>`

###### **Options:**

* `--json <PATH>` — Path to a JSON request DTO `{ vk_ref, authority, metadata, bytecode, proved? }`
* `--wait` — Poll the job until it reaches `done` or `error`
* `--poll-interval-ms <POLL_INTERVAL_MS>` — Poll interval (milliseconds) when using --wait

  Default value: `250`
* `--timeout-secs <TIMEOUT_SECS>` — Optional timeout (seconds) when using --wait (0 = no timeout)

  Default value: `0`



## `iroha app zk ivm get`

Get a prove job status via `/v1/zk/ivm/prove/{job_id}`

**Usage:** `iroha app zk ivm get --job-id <JOB_ID>`

###### **Options:**

* `--job-id <JOB_ID>` — Prove job id returned by `iroha zk ivm prove`



## `iroha app zk ivm delete`

Delete a prove job via `/v1/zk/ivm/prove/{job_id}`

**Usage:** `iroha app zk ivm delete --job-id <JOB_ID>`

###### **Options:**

* `--job-id <JOB_ID>` — Prove job id returned by `iroha zk ivm prove`



## `iroha app zk ivm derive-pk`

Derive a proving key (.pk) from verifying key bytes (.vk) for the Halo2 IPA IVM bind circuit

**Usage:** `iroha app zk ivm derive-pk [OPTIONS] --vk <PATH> --out <PATH>`

###### **Options:**

* `--backend <BACKEND>` — Backend label for the verifying key bytes (must match Torii `vk_ref.backend`), e.g. `halo2/ipa`

  Default value: `halo2/ipa`
* `--vk <PATH>` — Path to verifying key bytes (`.vk`) in Halo2 "processed" format
* `--out <PATH>` — Output path for proving key bytes (`.pk`)



## `iroha app zk vote`

ZK Vote helpers (tally)

**Usage:** `iroha app zk vote <COMMAND>`

###### **Subcommands:**

* `tally` — Get election tally (JSON)



## `iroha app zk vote tally`

Get election tally (JSON)

**Usage:** `iroha app zk vote tally --election-id <ELECTION_ID>`

###### **Options:**

* `--election-id <ELECTION_ID>` — Election identifier



## `iroha app zk envelope`

Encode a confidential encrypted payload (memo) into Norito bytes/base64

**Usage:** `iroha app zk envelope [OPTIONS] --ephemeral-pubkey <HEX32> --nonce-hex <HEX24> --ciphertext-b64 <BASE64>`

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



## `iroha app confidential`

Confidential asset tooling helpers

**Usage:** `iroha app confidential <COMMAND>`

###### **Subcommands:**

* `create-keys` — Derive confidential key hierarchy (nk/ivk/ovk/fvk) from a spend key
* `gas` — Inspect or update the confidential gas schedule



## `iroha app confidential create-keys`

Derive confidential key hierarchy (nk/ivk/ovk/fvk) from a spend key

**Usage:** `iroha app confidential create-keys [OPTIONS]`

###### **Options:**

* `--seed-hex <HEX32>` — 32-byte spend key in hex (if omitted, a random key is generated)
* `--output <PATH>` — Write the derived keyset JSON to a file
* `--quiet` — Do not print the generated JSON to stdout



## `iroha app confidential gas`

Inspect or update the confidential gas schedule

**Usage:** `iroha app confidential gas <COMMAND>`

###### **Subcommands:**

* `get` — Fetch the current confidential gas schedule
* `set` — Update the confidential gas schedule



## `iroha app confidential gas get`

Fetch the current confidential gas schedule

**Usage:** `iroha app confidential gas get`



## `iroha app confidential gas set`

Update the confidential gas schedule

**Usage:** `iroha app confidential gas set --proof-base <UNITS> --per-public-input <UNITS> --per-proof-byte <UNITS> --per-nullifier <UNITS> --per-commitment <UNITS>`

###### **Options:**

* `--proof-base <UNITS>`
* `--per-public-input <UNITS>`
* `--per-proof-byte <UNITS>`
* `--per-nullifier <UNITS>`
* `--per-commitment <UNITS>`



## `iroha app taikai`

Taikai publisher tooling (CAR bundler, envelopes)

**Usage:** `iroha app taikai <COMMAND>`

###### **Subcommands:**

* `bundle` — Bundle a Taikai segment into a CAR archive and Norito envelope
* `cek-rotate` — Emit a CEK rotation receipt for a Taikai stream
* `rpt-attest` — Generate a replication proof token (RPT) attestation
* `ingest` — Taikai ingest helpers (watchers, automation)



## `iroha app taikai bundle`

Bundle a Taikai segment into a CAR archive and Norito envelope

**Usage:** `iroha app taikai bundle [OPTIONS] --payload <PATH> --car-out <PATH> --envelope-out <PATH> --event-id <NAME> --stream-id <NAME> --rendition-id <NAME> --track-kind <TRACK_KIND> --codec <CODEC> --bitrate-kbps <KBPS> --segment-sequence <SEGMENT_SEQUENCE> --segment-start-pts <SEGMENT_START_PTS> --segment-duration <SEGMENT_DURATION> --wallclock-unix-ms <WALLCLOCK_UNIX_MS> --manifest-hash <HEX> --storage-ticket <HEX>`

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



## `iroha app taikai cek-rotate`

Emit a CEK rotation receipt for a Taikai stream

**Usage:** `iroha app taikai cek-rotate [OPTIONS] --event-id <NAME> --stream-id <NAME> --kms-profile <KMS_PROFILE> --new-wrap-key-label <NEW_WRAP_KEY_LABEL> --effective-segment <SEQ> --out <PATH>`

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



## `iroha app taikai rpt-attest`

Generate a replication proof token (RPT) attestation

**Usage:** `iroha app taikai rpt-attest [OPTIONS] --event-id <NAME> --stream-id <NAME> --rendition-id <NAME> --gar <PATH> --cek-receipt <PATH> --bundle <PATH> --out <PATH>`

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



## `iroha app taikai ingest`

Taikai ingest helpers (watchers, automation)

**Usage:** `iroha app taikai ingest <COMMAND>`

###### **Subcommands:**

* `watch` — Watch a directory for CMAF fragments and bundle them into CAR + Norito artifacts
* `edge` — Prototype edge receiver that emits CMAF fragments and drift logs for the watcher



## `iroha app taikai ingest watch`

Watch a directory for CMAF fragments and bundle them into CAR + Norito artifacts

**Usage:** `iroha app taikai ingest watch [OPTIONS] --source-dir <PATH> --event-id <NAME> --stream-id <NAME> --rendition-id <NAME>`

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



## `iroha app taikai ingest edge`

Prototype edge receiver that emits CMAF fragments and drift logs for the watcher

**Usage:** `iroha app taikai ingest edge [OPTIONS] --payload <PATH>`

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




## `iroha app content`

Content hosting helpers

**Usage:** `iroha app content <COMMAND>`

###### **Subcommands:**

* `publish` — Publish a content bundle (tar archive) to the content lane
* `pack` — Pack a directory into a deterministic tarball + manifest without submitting it



## `iroha app content publish`

Publish a content bundle (tar archive) to the content lane

**Usage:** `iroha app content publish [OPTIONS]`

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



## `iroha app content pack`

Pack a directory into a deterministic tarball + manifest without submitting it

**Usage:** `iroha app content pack [OPTIONS] --root <DIR> --bundle-out <PATH> --manifest-out <PATH>`

###### **Options:**

* `--root <DIR>` — Directory to pack into a tarball
* `--bundle-out <PATH>` — Path to write the tarball
* `--manifest-out <PATH>` — Path to write the generated manifest JSON
* `--dataspace <ID>` — Optional dataspace id override for the bundle manifest
* `--lane <ID>` — Optional lane id override for the bundle manifest
* `--auth <MODE>` — Auth mode (`public`, `role:<role_id>`, `sponsor:<uaid>`)
* `--cache-max-age-secs <SECS>` — Cache-Control max-age override (seconds)
* `--immutable` — Mark bundle as immutable (adds `immutable` to Cache-Control)



## `iroha app da`

Data availability helpers (ingest tooling)

**Usage:** `iroha app da <COMMAND>`

###### **Subcommands:**

* `submit` — Submit a raw blob to `/v1/da/ingest` and capture the signed receipt
* `get` — Fetch blobs via the multi-source orchestrator (thin wrapper over `sorafs fetch`)
* `get-blob` — Download manifest + chunk plan artifacts for an existing DA storage ticket
* `prove` — Generate Proof-of-Retrievability witnesses for a manifest/payload pair
* `prove-availability` — Download + verify availability for a storage ticket using a Torii manifest
* `proof-policies` — Fetch the current DA proof-policy bundle from Torii
* `proof-policy-snapshot` — Fetch the DA proof-policy snapshot from Torii
* `commitments-list` — List DA commitments with optional filters
* `commitments-prove` — Build a DA commitment proof with optional filters
* `commitments-verify` — Verify a DA commitment proof from a JSON file
* `pin-intents-list` — List DA pin intents with optional filters
* `pin-intents-prove` — Build a DA pin intent proof with optional filters
* `pin-intents-verify` — Verify a DA pin intent proof from a JSON file
* `rent-quote` — Quote rent/incentive breakdown for a blob size/retention combo
* `rent-ledger` — Convert a rent quote into deterministic ledger transfer instructions



## `iroha app da submit`

Submit a raw blob to `/v1/da/ingest` and capture the signed receipt

**Usage:** `iroha app da submit [OPTIONS] --payload <PATH>`

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



## `iroha app da get`

Fetch blobs via the multi-source orchestrator (thin wrapper over `sorafs fetch`)

**Usage:** `iroha app da get [OPTIONS] --gateway-provider <SPEC>`

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



## `iroha app da get-blob`

Download manifest + chunk plan artifacts for an existing DA storage ticket

**Usage:** `iroha app da get-blob [OPTIONS] --storage-ticket <HEX>`

###### **Options:**

* `--storage-ticket <HEX>` — Storage ticket identifier (hex string) issued by Torii
* `--block-hash <HEX>` — Optional block hash used to seed deterministic sampling in the manifest response
* `--endpoint <URL>` — Optional override for the Torii manifest endpoint (defaults to `$TORII/v1/da/manifests/`)
* `--output-dir <PATH>` — Directory for storing the fetched manifest + chunk plan artefacts



## `iroha app da prove`

Generate Proof-of-Retrievability witnesses for a manifest/payload pair

**Usage:** `iroha app da prove [OPTIONS] --manifest <PATH> --payload <PATH>`

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



## `iroha app da prove-availability`

Download + verify availability for a storage ticket using a Torii manifest

**Usage:** `iroha app da prove-availability [OPTIONS] --storage-ticket <HEX> --gateway-provider <SPEC>`

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



## `iroha app da proof-policies`

Fetch the current DA proof-policy bundle from Torii

**Usage:** `iroha app da proof-policies`



## `iroha app da proof-policy-snapshot`

Fetch the DA proof-policy snapshot from Torii

**Usage:** `iroha app da proof-policy-snapshot`



## `iroha app da commitments-list`

List DA commitments with optional filters

**Usage:** `iroha app da commitments-list [OPTIONS]`

###### **Options:**

* `--manifest-hash <HEX>` — Optional manifest hash filter (32-byte hex)
* `--lane-id <U32>` — Optional lane id filter (requires epoch + sequence for direct lookup)
* `--epoch <U64>` — Optional epoch filter (requires lane-id + sequence for direct lookup)
* `--sequence <U64>` — Optional sequence filter (requires lane-id + epoch for direct lookup)
* `--limit <U64>` — Optional list limit (`>0`)
* `--offset <U64>` — Optional list offset

  Default value: `0`



## `iroha app da commitments-prove`

Build a DA commitment proof with optional filters

**Usage:** `iroha app da commitments-prove [OPTIONS]`

###### **Options:**

* `--manifest-hash <HEX>` — Optional manifest hash filter (32-byte hex)
* `--lane-id <U32>` — Optional lane id filter (requires epoch + sequence for direct lookup)
* `--epoch <U64>` — Optional epoch filter (requires lane-id + sequence for direct lookup)
* `--sequence <U64>` — Optional sequence filter (requires lane-id + epoch for direct lookup)
* `--limit <U64>` — Optional list limit (`>0`)
* `--offset <U64>` — Optional list offset

  Default value: `0`



## `iroha app da commitments-verify`

Verify a DA commitment proof from a JSON file

**Usage:** `iroha app da commitments-verify --proof-json <PATH>`

###### **Options:**

* `--proof-json <PATH>` — Path to a JSON-encoded `DaCommitmentProof`



## `iroha app da pin-intents-list`

List DA pin intents with optional filters

**Usage:** `iroha app da pin-intents-list [OPTIONS]`

###### **Options:**

* `--manifest-hash <HEX>` — Optional manifest hash filter (32-byte hex)
* `--storage-ticket <HEX>` — Optional storage ticket filter (32-byte hex)
* `--alias <TEXT>` — Optional alias filter
* `--lane-id <U32>` — Optional lane id filter (requires epoch + sequence for direct lookup)
* `--epoch <U64>` — Optional epoch filter (requires lane-id + sequence for direct lookup)
* `--sequence <U64>` — Optional sequence filter (requires lane-id + epoch for direct lookup)
* `--limit <U64>` — Optional list limit (`>0`)
* `--offset <U64>` — Optional list offset

  Default value: `0`



## `iroha app da pin-intents-prove`

Build a DA pin intent proof with optional filters

**Usage:** `iroha app da pin-intents-prove [OPTIONS]`

###### **Options:**

* `--manifest-hash <HEX>` — Optional manifest hash filter (32-byte hex)
* `--storage-ticket <HEX>` — Optional storage ticket filter (32-byte hex)
* `--alias <TEXT>` — Optional alias filter
* `--lane-id <U32>` — Optional lane id filter (requires epoch + sequence for direct lookup)
* `--epoch <U64>` — Optional epoch filter (requires lane-id + sequence for direct lookup)
* `--sequence <U64>` — Optional sequence filter (requires lane-id + epoch for direct lookup)
* `--limit <U64>` — Optional list limit (`>0`)
* `--offset <U64>` — Optional list offset

  Default value: `0`



## `iroha app da pin-intents-verify`

Verify a DA pin intent proof from a JSON file

**Usage:** `iroha app da pin-intents-verify --proof-json <PATH>`

###### **Options:**

* `--proof-json <PATH>` — Path to a JSON-encoded `DaPinIntentWithLocation`



## `iroha app da rent-quote`

Quote rent/incentive breakdown for a blob size/retention combo

**Usage:** `iroha app da rent-quote [OPTIONS] --gib <GIB> --months <MONTHS>`

###### **Options:**

* `--gib <GIB>` — Logical GiB stored in the blob (post-chunking)
* `--months <MONTHS>` — Retention duration measured in months
* `--policy-json <PATH>` — Optional path to a JSON-encoded `DaRentPolicyV1`
* `--policy-norito <PATH>` — Optional path to a Norito-encoded `DaRentPolicyV1`
* `--policy-label <TEXT>` — Optional human-readable label recorded in the quote metadata (defaults to source path)
* `--quote-out <PATH>` — Optional path for persisting the rendered quote JSON



## `iroha app da rent-ledger`

Convert a rent quote into deterministic ledger transfer instructions

**Usage:** `iroha app da rent-ledger --quote <PATH> --payer-account <ACCOUNT_ID> --treasury-account <ACCOUNT_ID> --protocol-reserve-account <ACCOUNT_ID> --provider-account <ACCOUNT_ID> --pdp-bonus-account <ACCOUNT_ID> --potr-bonus-account <ACCOUNT_ID> --asset-definition <AID>`

###### **Options:**

* `--quote <PATH>` — Path to the rent quote JSON file (output of `iroha da rent-quote`)
* `--payer-account <ACCOUNT_ID>` — Account responsible for paying the rent and funding bonus pools
* `--treasury-account <ACCOUNT_ID>` — Treasury or escrow account receiving the base rent before distribution
* `--protocol-reserve-account <ACCOUNT_ID>` — Protocol reserve account that receives the configured reserve share
* `--provider-account <ACCOUNT_ID>` — Provider payout account that receives the base rent remainder
* `--pdp-bonus-account <ACCOUNT_ID>` — Account earmarked for PDP bonus payouts
* `--potr-bonus-account <ACCOUNT_ID>` — Account earmarked for `PoTR` bonus payouts
* `--asset-definition <AID>` — Asset definition identifier used for transfers (canonical unprefixed Base58 address)



## `iroha app streaming`

Streaming helpers (HPKE fingerprints, suite listings)

**Usage:** `iroha app streaming <COMMAND>`

###### **Subcommands:**

* `fingerprint` — Compute the ML-KEM fingerprint advertised in `EncryptionSuite::Kyber*`
* `suites` — List supported ML-KEM suite identifiers



## `iroha app streaming fingerprint`

Compute the ML-KEM fingerprint advertised in `EncryptionSuite::Kyber*`

**Usage:** `iroha app streaming fingerprint [OPTIONS] --public-key <HEX>`

###### **Options:**

* `--suite <NAME>` — ML-KEM suite to use (e.g., `mlkem512`, `mlkem768`, `mlkem1024`)
* `--public-key <HEX>` — Hex-encoded ML-KEM public key



## `iroha app streaming suites`

List supported ML-KEM suite identifiers

**Usage:** `iroha app streaming suites`



## `iroha app nexus`

Nexus helpers (lanes, governance)

**Usage:** `iroha app nexus <COMMAND>`

###### **Subcommands:**

* `lane-report` — Show governance manifest status per lane
* `public-lane` — Inspect public-lane validator lifecycle and stake state



## `iroha app nexus lane-report`

Show governance manifest status per lane

**Usage:** `iroha app nexus lane-report [OPTIONS]`

###### **Options:**

* `--summary` — Print a compact table instead of JSON

  Default value: `false`
* `--only-missing` — Show only lanes that require a manifest but remain sealed

  Default value: `false`
* `--fail-on-sealed` — Exit with non-zero status if any manifest is missing

  Default value: `false`



## `iroha app nexus public-lane`

Inspect public-lane validator lifecycle and stake state

**Usage:** `iroha app nexus public-lane <COMMAND>`

###### **Subcommands:**

* `validators` — List validators for a public lane with lifecycle hints
* `stake` — List bonded stake and pending unbonds for a public lane



## `iroha app nexus public-lane validators`

List validators for a public lane with lifecycle hints

**Usage:** `iroha app nexus public-lane validators [OPTIONS]`

###### **Options:**

* `--lane <LANE>` — Public lane identifier (defaults to SINGLE lane)

  Default value: `0`
* `--summary` — Render a compact table instead of raw JSON

  Default value: `false`



## `iroha app nexus public-lane stake`

List bonded stake and pending unbonds for a public lane

**Usage:** `iroha app nexus public-lane stake [OPTIONS]`

###### **Options:**

* `--lane <LANE>` — Public lane identifier (defaults to SINGLE lane)

  Default value: `0`
* `--validator <ACCOUNT_ID>` — Filter for a specific validator account (optional)
* `--summary` — Render a compact table instead of raw JSON

  Default value: `false`



## `iroha app staking`

Public-lane staking helpers (register/activate/exit)

**Usage:** `iroha app staking <COMMAND>`

###### **Subcommands:**

* `register` — Register a stake-elected validator on a public lane
* `rebind` — Rebind an existing validator to a replacement consensus peer
* `activate` — Activate a pending validator once its activation epoch is reached
* `exit` — Schedule or finalize a validator exit



## `iroha app staking register`

Register a stake-elected validator on a public lane

**Usage:** `iroha app staking register [OPTIONS] --lane-id <LANE_ID> --validator <ACCOUNT_ID> --peer-id <PEER_ID> --initial-stake <AMOUNT>`

###### **Options:**

* `--lane-id <LANE_ID>` — Lane id to register against
* `--validator <ACCOUNT_ID>` — Validator account identifier (canonical I105 account literal)
* `--peer-id <PEER_ID>` — Peer identity that will participate in consensus for this validator
* `--stake-account <ACCOUNT_ID>` — Optional staking account (defaults to validator)
* `--initial-stake <AMOUNT>` — Initial self-bond (integer, uses the staking asset scale)
* `--metadata <PATH>` — Optional metadata JSON (Norito JSON object)



## `iroha app staking rebind`

Rebind an existing validator to a replacement consensus peer

**Usage:** `iroha app staking rebind --lane-id <LANE_ID> --validator <ACCOUNT_ID> --peer-id <PEER_ID>`

###### **Options:**

* `--lane-id <LANE_ID>` — Lane id containing the validator
* `--validator <ACCOUNT_ID>` — Validator account identifier (canonical I105 account literal)
* `--peer-id <PEER_ID>` — Replacement peer identity that will participate in consensus for this validator



## `iroha app staking activate`

Activate a pending validator once its activation epoch is reached

**Usage:** `iroha app staking activate --lane-id <LANE_ID> --validator <ACCOUNT_ID>`

###### **Options:**

* `--lane-id <LANE_ID>` — Lane id containing the pending validator
* `--validator <ACCOUNT_ID>` — Validator account identifier (canonical I105 account literal)



## `iroha app staking exit`

Schedule or finalize a validator exit

**Usage:** `iroha app staking exit --lane-id <LANE_ID> --validator <ACCOUNT_ID> --release-at-ms <MILLIS>`

###### **Options:**

* `--lane-id <LANE_ID>` — Lane id containing the validator
* `--validator <ACCOUNT_ID>` — Validator account identifier (canonical I105 account literal)
* `--release-at-ms <MILLIS>` — Release timestamp in milliseconds (must not precede current block timestamp)



## `iroha app subscriptions`

Subscription plan and billing helpers

**Usage:** `iroha app subscriptions <COMMAND>`

###### **Subcommands:**

* `plan` — Manage subscription plans (asset definition metadata)
* `subscription` — Manage subscriptions and billing actions



## `iroha app subscriptions plan`

Manage subscription plans (asset definition metadata)

**Usage:** `iroha app subscriptions plan <COMMAND>`

###### **Subcommands:**

* `create` — Register a subscription plan on an asset definition
* `list` — List subscription plans, optionally filtered by provider



## `iroha app subscriptions plan create`

Register a subscription plan on an asset definition

**Usage:** `iroha app subscriptions plan create [OPTIONS] --authority <ACCOUNT_ID> --private-key <HEX> --plan-id <ASSET_DEF_ID>`

###### **Options:**

* `--authority <ACCOUNT_ID>` — Authority account identifier (canonical I105 account literal)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--plan-id <ASSET_DEF_ID>` — Asset definition id where the plan metadata is stored
* `--plan-json <PATH>` — Path to JSON plan payload (reads stdin when omitted)



## `iroha app subscriptions plan list`

List subscription plans, optionally filtered by provider

**Usage:** `iroha app subscriptions plan list [OPTIONS]`

###### **Options:**

* `--provider <ACCOUNT_ID>` — Filter by plan provider (account id)
* `--limit <LIMIT>` — Limit number of results
* `--offset <OFFSET>` — Offset for pagination (default 0)

  Default value: `0`



## `iroha app subscriptions subscription`

Manage subscriptions and billing actions

**Usage:** `iroha app subscriptions subscription <COMMAND>`

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



## `iroha app subscriptions subscription create`

Create a subscription and billing trigger

**Usage:** `iroha app subscriptions subscription create [OPTIONS] --authority <ACCOUNT_ID> --private-key <HEX> --subscription-id <NFT_ID> --plan-id <ASSET_DEF_ID>`

###### **Options:**

* `--authority <ACCOUNT_ID>` — Authority account identifier (canonical I105 account literal)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--subscription-id <NFT_ID>` — Subscription NFT id to register
* `--plan-id <ASSET_DEF_ID>` — Subscription plan asset definition id
* `--billing-trigger-id <BILLING_TRIGGER_ID>` — Optional billing trigger id to use
* `--usage-trigger-id <USAGE_TRIGGER_ID>` — Optional usage trigger id to use (usage plans only)
* `--first-charge-ms <FIRST_CHARGE_MS>` — Optional first charge timestamp in UTC milliseconds
* `--grant-usage-to-provider <GRANT_USAGE_TO_PROVIDER>` — Grant usage reporting permission to the plan provider

  Possible values: `true`, `false`




## `iroha app subscriptions subscription list`

List subscriptions with optional filters

**Usage:** `iroha app subscriptions subscription list [OPTIONS]`

###### **Options:**

* `--owned-by <ACCOUNT_ID>` — Filter by subscriber account
* `--provider <ACCOUNT_ID>` — Filter by plan provider account
* `--status <STATUS>` — Filter by status (active, paused, `past_due`, canceled, suspended)
* `--limit <LIMIT>` — Limit number of results
* `--offset <OFFSET>` — Offset for pagination (default 0)

  Default value: `0`



## `iroha app subscriptions subscription get`

Fetch a subscription by id

**Usage:** `iroha app subscriptions subscription get --subscription-id <NFT_ID>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id



## `iroha app subscriptions subscription pause`

Pause billing for a subscription

**Usage:** `iroha app subscriptions subscription pause [OPTIONS] --subscription-id <NFT_ID> --authority <ACCOUNT_ID> --private-key <HEX>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id
* `--authority <ACCOUNT_ID>` — Authority account identifier (canonical I105 account literal)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--charge-at-ms <CHARGE_AT_MS>` — Optional charge time override in UTC milliseconds
* `--cancel-at-period-end` — Cancel at the end of the current billing period (cancel only)



## `iroha app subscriptions subscription resume`

Resume billing for a subscription

**Usage:** `iroha app subscriptions subscription resume [OPTIONS] --subscription-id <NFT_ID> --authority <ACCOUNT_ID> --private-key <HEX>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id
* `--authority <ACCOUNT_ID>` — Authority account identifier (canonical I105 account literal)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--charge-at-ms <CHARGE_AT_MS>` — Optional charge time override in UTC milliseconds
* `--cancel-at-period-end` — Cancel at the end of the current billing period (cancel only)



## `iroha app subscriptions subscription cancel`

Cancel a subscription and remove its billing trigger

**Usage:** `iroha app subscriptions subscription cancel [OPTIONS] --subscription-id <NFT_ID> --authority <ACCOUNT_ID> --private-key <HEX>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id
* `--authority <ACCOUNT_ID>` — Authority account identifier (canonical I105 account literal)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--charge-at-ms <CHARGE_AT_MS>` — Optional charge time override in UTC milliseconds
* `--cancel-at-period-end` — Cancel at the end of the current billing period (cancel only)



## `iroha app subscriptions subscription keep`

Undo a scheduled period-end cancellation

**Usage:** `iroha app subscriptions subscription keep [OPTIONS] --subscription-id <NFT_ID> --authority <ACCOUNT_ID> --private-key <HEX>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id
* `--authority <ACCOUNT_ID>` — Authority account identifier (canonical I105 account literal)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--charge-at-ms <CHARGE_AT_MS>` — Optional charge time override in UTC milliseconds
* `--cancel-at-period-end` — Cancel at the end of the current billing period (cancel only)



## `iroha app subscriptions subscription charge-now`

Execute billing immediately

**Usage:** `iroha app subscriptions subscription charge-now [OPTIONS] --subscription-id <NFT_ID> --authority <ACCOUNT_ID> --private-key <HEX>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id
* `--authority <ACCOUNT_ID>` — Authority account identifier (canonical I105 account literal)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--charge-at-ms <CHARGE_AT_MS>` — Optional charge time override in UTC milliseconds
* `--cancel-at-period-end` — Cancel at the end of the current billing period (cancel only)



## `iroha app subscriptions subscription usage`

Record usage for a subscription usage plan

**Usage:** `iroha app subscriptions subscription usage [OPTIONS] --subscription-id <NFT_ID> --authority <ACCOUNT_ID> --private-key <HEX> --unit-key <UNIT_KEY> --delta <DELTA>`

###### **Options:**

* `--subscription-id <NFT_ID>` — Subscription NFT id
* `--authority <ACCOUNT_ID>` — Authority account identifier (canonical I105 account literal)
* `--private-key <HEX>` — Hex-encoded private key for signing
* `--unit-key <UNIT_KEY>` — Usage counter key to update
* `--delta <DELTA>` — Usage increment (must be non-negative)
* `--usage-trigger-id <USAGE_TRIGGER_ID>` — Optional usage trigger id override



## `iroha app endorsement`

Domain endorsement helpers (committees, policies, submissions)

**Usage:** `iroha app endorsement <COMMAND>`

###### **Subcommands:**

* `prepare` — Build a domain endorsement (optionally signing it) and emit JSON to stdout
* `submit` — Submit a domain endorsement into the chain state for later reuse
* `list` — List recorded endorsements for a domain
* `policy` — Fetch the endorsement policy for a domain
* `committee` — Fetch a registered endorsement committee
* `register-committee` — Register an endorsement committee (quorum + members)
* `set-policy` — Set or replace the endorsement policy for a domain



## `iroha app endorsement prepare`

Build a domain endorsement (optionally signing it) and emit JSON to stdout

**Usage:** `iroha app endorsement prepare [OPTIONS] --domain <DOMAIN> --issued-at-height <HEIGHT> --expires-at-height <HEIGHT>`

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



## `iroha app endorsement submit`

Submit a domain endorsement into the chain state for later reuse

**Usage:** `iroha app endorsement submit [OPTIONS]`

###### **Options:**

* `--file <PATH>` — Path to the endorsement JSON. If omitted, read from stdin



## `iroha app endorsement list`

List recorded endorsements for a domain

**Usage:** `iroha app endorsement list --domain <DOMAIN>`

###### **Options:**

* `--domain <DOMAIN>` — Domain to query



## `iroha app endorsement policy`

Fetch the endorsement policy for a domain

**Usage:** `iroha app endorsement policy --domain <DOMAIN>`

###### **Options:**

* `--domain <DOMAIN>` — Domain to query



## `iroha app endorsement committee`

Fetch a registered endorsement committee

**Usage:** `iroha app endorsement committee --committee-id <COMMITTEE_ID>`

###### **Options:**

* `--committee-id <COMMITTEE_ID>` — Committee identifier to fetch



## `iroha app endorsement register-committee`

Register an endorsement committee (quorum + members)

**Usage:** `iroha app endorsement register-committee [OPTIONS] --committee-id <COMMITTEE_ID> --quorum <QUORUM> --member <PUBLIC_KEY>`

###### **Options:**

* `--committee-id <COMMITTEE_ID>` — New committee identifier
* `--quorum <QUORUM>` — Quorum required to accept an endorsement
* `--member <PUBLIC_KEY>` — Member public keys allowed to sign endorsements (string form)
* `--metadata <PATH>` — Optional metadata payload (Norito JSON file) to attach



## `iroha app endorsement set-policy`

Set or replace the endorsement policy for a domain

**Usage:** `iroha app endorsement set-policy [OPTIONS] --domain <DOMAIN> --committee-id <COMMITTEE_ID> --max-endorsement-age <BLOCKS>`

###### **Options:**

* `--domain <DOMAIN>` — Domain requiring endorsements
* `--committee-id <COMMITTEE_ID>` — Committee identifier to trust
* `--max-endorsement-age <BLOCKS>` — Maximum age (in blocks) allowed between issuance and acceptance
* `--required` — Whether an endorsement is required for the domain

  Default value: `true`



## `iroha app jurisdiction`

Jurisdiction Data Guardian helpers (attestations and SDN registries)

**Usage:** `iroha app jurisdiction <COMMAND>`

###### **Subcommands:**

* `verify` — Validate a JDG attestation (structural + SDN commitments)



## `iroha app jurisdiction verify`

Validate a JDG attestation (structural + SDN commitments)

**Usage:** `iroha app jurisdiction verify [OPTIONS]`

###### **Options:**

* `--attestation <PATH>` — Path to the JDG attestation payload (Norito JSON or binary). Reads stdin when omitted
* `--sdn-registry <PATH>` — Optional SDN registry payload (Norito JSON or binary)
* `--require-sdn-commitments` — Whether SDN commitments are mandatory for this attestation

  Default value: `false`
* `--dual-publish-blocks <DUAL_PUBLISH_BLOCKS>` — Number of blocks the previous SDN key remains valid after rotation

  Default value: `0`
* `--current-height <HEIGHT>` — Current block height for expiry/block-window checks
* `--expect-dataspace <ID>` — Expected dataspace id; validation fails if it does not match



## `iroha app compute`

Compute lane simulation helpers

**Usage:** `iroha app compute <COMMAND>`

###### **Subcommands:**

* `simulate` — Simulate a compute call offline and emit the receipt/response
* `invoke` — Invoke a running compute gateway using the shared fixtures



## `iroha app compute simulate`

Simulate a compute call offline and emit the receipt/response

**Usage:** `iroha app compute simulate [OPTIONS]`

###### **Options:**

* `--manifest <PATH>` — Path to the compute manifest to validate against

  Default value: `fixtures/compute/manifest_compute_payments.json`
* `--call <PATH>` — Path to the canonical compute call fixture

  Default value: `fixtures/compute/call_compute_payments.json`
* `--payload <PATH>` — Path to the payload to send (ignored when --payload-inline is supplied)

  Default value: `fixtures/compute/payload_compute_payments.json`
* `--payload-inline <BYTES>` — Inline payload bytes (UTF-8) (mutually exclusive with --payload)
* `--json-out <PATH>` — Optional JSON output path (stdout when omitted)



## `iroha app compute invoke`

Invoke a running compute gateway using the shared fixtures

**Usage:** `iroha app compute invoke [OPTIONS]`

###### **Options:**

* `--endpoint <URL>` — Base endpoint for the compute gateway (without the route path)

  Default value: `http://127.0.0.1:8088`
* `--manifest <PATH>` — Path to the compute manifest used for validation

  Default value: `fixtures/compute/manifest_compute_payments.json`
* `--call <PATH>` — Path to the compute call fixture

  Default value: `fixtures/compute/call_compute_payments.json`
* `--payload <PATH>` — Path to the payload to send with the call

  Default value: `fixtures/compute/payload_compute_payments.json`



## `iroha app soracloud`

Soracloud deployment/control-plane helpers

**Usage:** `iroha app soracloud <COMMAND>`

###### **Subcommands:**

* `init` — Scaffold baseline container/service manifests
* `deploy` — Validate manifests and register a new service deployment
* `status` — Show authoritative Soracloud service state (all services or one service)
* `config-set` — Record or replace an authoritative service config entry
* `config-delete` — Delete an authoritative service config entry
* `config-status` — Query authoritative service config state
* `secret-set` — Record or replace an authoritative service secret entry
* `secret-delete` — Delete an authoritative service secret entry
* `secret-status` — Query authoritative service secret state
* `upgrade` — Validate manifests and upgrade an existing deployed service
* `rollback` — Roll back a deployed service to a previous (or specified) version
* `rollout` — Advance or fail a rollout step using health-gated canary controls
* `agent-deploy` — Register a persistent AI apartment manifest in the live control plane
* `agent-lease-renew` — Renew an apartment lease in the live control plane
* `agent-restart` — Request deterministic apartment restart in the live control plane
* `agent-status` — Show authoritative apartment runtime status
* `agent-wallet-spend` — Submit an apartment wallet spend request under policy guardrails
* `agent-wallet-approve` — Approve a pending apartment wallet spend request
* `agent-policy-revoke` — Revoke an apartment policy capability
* `agent-message-send` — Send a deterministic mailbox message between apartments
* `agent-message-ack` — Acknowledge (consume) a mailbox message from an apartment queue
* `agent-mailbox-status` — Inspect mailbox queue state for an apartment
* `agent-artifact-allow` — Add an artifact hash (and optional provenance hash) to autonomy allowlist
* `agent-autonomy-run` — Approve an autonomous run under allowlist/provenance/budget guardrails
* `agent-autonomy-status` — Show autonomous-run policy state for an apartment
* `training-job-start` — Start a distributed training job in live Torii control-plane mode
* `training-job-checkpoint` — Record a training checkpoint in live Torii control-plane mode
* `training-job-retry` — Submit a training retry request in live Torii control-plane mode
* `training-job-status` — Query training job status in live Torii control-plane mode
* `model-artifact-register` — Register model-artifact metadata in live Torii control-plane mode
* `model-artifact-status` — Query model-artifact status in live Torii control-plane mode
* `model-weight-register` — Register a model weight version in live Torii control-plane mode
* `model-weight-promote` — Promote a model weight version in live Torii control-plane mode
* `model-weight-rollback` — Roll back a model weight version in live Torii control-plane mode
* `model-weight-status` — Query model weight status in live Torii control-plane mode
* `model-upload-encryption-recipient` — Fetch the active uploaded-model encryption recipient from Torii
* `model-upload-init` — Register an uploaded-model bundle root in live Torii control-plane mode
* `model-upload-chunk` — Append one uploaded-model encrypted chunk in live Torii control-plane mode
* `model-upload-finalize` — Finalize an uploaded-model bundle into the model registry
* `model-upload-status` — Query uploaded-model bundle/chunk status in live Torii control-plane mode
* `model-compile` — Admit a deterministic private compile profile for an uploaded model
* `model-compile-status` — Query uploaded-model compile status in live Torii control-plane mode
* `model-allow` — Bind an uploaded model to an apartment that already admits model inference
* `model-run-private` — Start and finalize a private uploaded-model inference session
* `model-run-status` — Query private uploaded-model inference session status
* `model-decrypt-output` — Release governed output material for a private uploaded-model session
* `model-publish-private` — Orchestrate uploaded-model publish/init/chunk/finalize/compile/allow from a plan file
* `hf-deploy` — Join or create a shared Hugging Face lease pool in live Torii control-plane mode
* `hf-status` — Query shared Hugging Face lease pool status in live Torii control-plane mode
* `hf-lease-leave` — Leave a shared Hugging Face lease pool in live Torii control-plane mode
* `hf-lease-renew` — Renew an expired or drained shared Hugging Face lease pool window
* `model-host-advertise` — Advertise validator-host capabilities for authoritative HF placement
* `model-host-heartbeat` — Refresh the heartbeat TTL for an advertised model host
* `model-host-withdraw` — Withdraw an advertised model host capability
* `model-host-status` — Query authoritative model-host capability adverts



## `iroha app soracloud init`

Scaffold baseline container/service manifests

**Usage:** `iroha app soracloud init [OPTIONS]`

###### **Options:**

* `--output-dir <DIR>` — Directory where manifests and template artifacts will be created

  Default value: `.soracloud`
* `--service-name <NAME>` — Logical service name used in the scaffolded service manifest

  Default value: `web_portal`
* `--service-version <VERSION>` — Version string used in the scaffolded service manifest

  Default value: `0.1.0`
* `--template <TEMPLATE>` — Scaffolding template to generate in addition to control-plane manifests

  Default value: `baseline`

  Possible values:
  - `baseline`:
    Generate only Soracloud control-plane manifests
  - `site`:
    Generate a Vue3/Vite static SPA starter with SoraFS publish workflow
  - `webapp`:
    Generate a Vue3 SPA + API starter with deterministic challenge-signature auth
  - `pii-app`:
    Generate a private PII app starter with consent + retention workflows

* `--overwrite` — Overwrite existing files in the output directory



## `iroha app soracloud deploy`

Validate manifests and register a new service deployment

**Usage:** `iroha app soracloud deploy [OPTIONS]`

###### **Options:**

* `--container <PATH>` — Path to a `SoraContainerManifestV1` JSON document

  Default value: `fixtures/soracloud/sora_container_manifest_v1.json`
* `--service <PATH>` — Path to a `SoraServiceManifestV1` JSON document

  Default value: `fixtures/soracloud/sora_service_manifest_v1.json`
* `--initial-configs <PATH>` — Optional JSON file containing a map of inline config values committed atomically with deploy
* `--initial-secrets <PATH>` — Optional JSON file containing a map of inline secret envelopes committed atomically with deploy
* `--torii-url <URL>` — Torii base URL to execute deploy against authoritative control-plane APIs
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for Torii mutation requests

  Default value: `10`



## `iroha app soracloud status`

Show authoritative Soracloud service state (all services or one service)

**Usage:** `iroha app soracloud status [OPTIONS]`

###### **Options:**

* `--service-name <NAME>` — Optional service name filter
* `--torii-url <URL>` — Torii base URL (for example `http://127.0.0.1:8080/`) to query `/v1/soracloud/status` from the authoritative control plane
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when querying Torii
* `--timeout-secs <SECS>` — HTTP timeout for Torii status requests

  Default value: `10`



## `iroha app soracloud config-set`

Record or replace an authoritative service config entry

**Usage:** `iroha app soracloud config-set [OPTIONS] --service-name <NAME> --config-name <NAME>`

###### **Options:**

* `--service-name <NAME>` — Service name owning the config entry
* `--config-name <NAME>` — Stable service-scoped config name
* `--value-json <JSON>` — Inline JSON value for the config entry
* `--value-file <PATH>` — Path to a JSON document used as the config value
* `--torii-url <URL>` — Torii base URL for authoritative `service/config/set`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud config-delete`

Delete an authoritative service config entry

**Usage:** `iroha app soracloud config-delete [OPTIONS] --service-name <NAME> --config-name <NAME>`

###### **Options:**

* `--service-name <NAME>` — Service name owning the config entry
* `--config-name <NAME>` — Stable service-scoped config name
* `--torii-url <URL>` — Torii base URL for authoritative `service/config/delete`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud config-status`

Query authoritative service config state

**Usage:** `iroha app soracloud config-status [OPTIONS] --service-name <NAME>`

###### **Options:**

* `--service-name <NAME>` — Service name owning the config entries
* `--config-name <NAME>` — Optional config name filter
* `--torii-url <URL>` — Torii base URL for authoritative `service/config/status`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when querying live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane queries

  Default value: `10`



## `iroha app soracloud secret-set`

Record or replace an authoritative service secret entry

**Usage:** `iroha app soracloud secret-set [OPTIONS] --service-name <NAME> --secret-name <NAME> --secret-file <PATH>`

###### **Options:**

* `--service-name <NAME>` — Service name owning the secret entry
* `--secret-name <NAME>` — Stable service-scoped secret name
* `--secret-file <PATH>` — Path to a `SecretEnvelopeV1` JSON document
* `--torii-url <URL>` — Torii base URL for authoritative `service/secret/set`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud secret-delete`

Delete an authoritative service secret entry

**Usage:** `iroha app soracloud secret-delete [OPTIONS] --service-name <NAME> --secret-name <NAME>`

###### **Options:**

* `--service-name <NAME>` — Service name owning the secret entry
* `--secret-name <NAME>` — Stable service-scoped secret name
* `--torii-url <URL>` — Torii base URL for authoritative `service/secret/delete`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud secret-status`

Query authoritative service secret state

**Usage:** `iroha app soracloud secret-status [OPTIONS] --service-name <NAME>`

###### **Options:**

* `--service-name <NAME>` — Service name owning the secret entries
* `--secret-name <NAME>` — Optional secret name filter
* `--torii-url <URL>` — Torii base URL for authoritative `service/secret/status`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when querying live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane queries

  Default value: `10`



## `iroha app soracloud upgrade`

Validate manifests and upgrade an existing deployed service

**Usage:** `iroha app soracloud upgrade [OPTIONS]`

###### **Options:**

* `--container <PATH>` — Path to a `SoraContainerManifestV1` JSON document

  Default value: `fixtures/soracloud/sora_container_manifest_v1.json`
* `--service <PATH>` — Path to a `SoraServiceManifestV1` JSON document

  Default value: `fixtures/soracloud/sora_service_manifest_v1.json`
* `--initial-configs <PATH>` — Optional JSON file containing a map of inline config values committed atomically with upgrade
* `--initial-secrets <PATH>` — Optional JSON file containing a map of inline secret envelopes committed atomically with upgrade
* `--torii-url <URL>` — Torii base URL to execute upgrade against authoritative control-plane APIs
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for Torii mutation requests

  Default value: `10`



## `iroha app soracloud rollback`

Roll back a deployed service to a previous (or specified) version

**Usage:** `iroha app soracloud rollback [OPTIONS] --service-name <NAME>`

###### **Options:**

* `--service-name <NAME>` — Service name to roll back
* `--target-version <VERSION>` — Optional target version. When omitted, rolls back to the previous version
* `--torii-url <URL>` — Torii base URL to execute rollback against authoritative control-plane APIs
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for Torii mutation requests

  Default value: `10`



## `iroha app soracloud rollout`

Advance or fail a rollout step using health-gated canary controls

**Usage:** `iroha app soracloud rollout [OPTIONS] --service-name <NAME> --rollout-handle <HANDLE> --governance-tx-hash <HASH>`

###### **Options:**

* `--service-name <NAME>` — Service name with an active rollout
* `--rollout-handle <HANDLE>` — Rollout handle emitted by `upgrade` output (`rollout_handle`)
* `--health <HEALTH>` — Health signal for this rollout step

  Default value: `healthy`

  Possible values: `healthy`, `unhealthy`

* `--promote-to-percent <PERCENT>` — Optional target traffic percentage for healthy promotions
* `--governance-tx-hash <HASH>` — Governance transaction hash linked to this rollout action
* `--torii-url <URL>` — Torii base URL to execute rollout against authoritative control-plane APIs
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for Torii mutation requests

  Default value: `10`



## `iroha app soracloud agent-deploy`

Register a persistent AI apartment manifest in the live control plane

**Usage:** `iroha app soracloud agent-deploy [OPTIONS]`

###### **Options:**

* `--manifest <PATH>` — Path to an `AgentApartmentManifestV1` JSON document

  Default value: `fixtures/soracloud/agent_apartment_manifest_v1.json`
* `--lease-ticks <TICKS>` — Lease length, measured in deterministic control-plane sequence ticks

  Default value: `120`
* `--autonomy-budget-units <UNITS>` — Initial autonomy execution budget units

  Default value: `10000`
* `--torii-url <URL>` — Torii base URL for authoritative `agent/deploy`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud agent-lease-renew`

Renew an apartment lease in the live control plane

**Usage:** `iroha app soracloud agent-lease-renew [OPTIONS] --apartment-name <NAME>`

###### **Options:**

* `--apartment-name <NAME>` — Apartment name to renew
* `--lease-ticks <TICKS>` — Lease extension ticks

  Default value: `120`
* `--torii-url <URL>` — Torii base URL for authoritative `agent/lease/renew`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud agent-restart`

Request deterministic apartment restart in the live control plane

**Usage:** `iroha app soracloud agent-restart [OPTIONS] --apartment-name <NAME> --reason <TEXT>`

###### **Options:**

* `--apartment-name <NAME>` — Apartment name to restart
* `--reason <TEXT>` — Human-readable reason captured in scheduler events
* `--torii-url <URL>` — Torii base URL for authoritative `agent/restart`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud agent-status`

Show authoritative apartment runtime status

**Usage:** `iroha app soracloud agent-status [OPTIONS]`

###### **Options:**

* `--apartment-name <NAME>` — Optional apartment name filter
* `--torii-url <URL>` — Torii base URL for authoritative `agent/status`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when querying live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane status query

  Default value: `10`



## `iroha app soracloud agent-wallet-spend`

Submit an apartment wallet spend request under policy guardrails

**Usage:** `iroha app soracloud agent-wallet-spend [OPTIONS] --apartment-name <NAME> --asset-definition <ASSET> --amount-nanos <NANOS>`

###### **Options:**

* `--apartment-name <NAME>` — Apartment name issuing the spend request
* `--asset-definition <ASSET>` — Asset definition identifier (canonical unprefixed Base58 address)
* `--amount-nanos <NANOS>` — Spend amount in nanos
* `--torii-url <URL>` — Torii base URL for authoritative `agent/wallet/spend`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud agent-wallet-approve`

Approve a pending apartment wallet spend request

**Usage:** `iroha app soracloud agent-wallet-approve [OPTIONS] --apartment-name <NAME> --request-id <REQUEST>`

###### **Options:**

* `--apartment-name <NAME>` — Apartment name owning the request
* `--request-id <REQUEST>` — Wallet request identifier emitted by `agent-wallet-spend`
* `--torii-url <URL>` — Torii base URL for authoritative `agent/wallet/approve`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud agent-policy-revoke`

Revoke an apartment policy capability

**Usage:** `iroha app soracloud agent-policy-revoke [OPTIONS] --apartment-name <NAME> --capability <CAPABILITY>`

###### **Options:**

* `--apartment-name <NAME>` — Apartment name whose policy should be updated
* `--capability <CAPABILITY>` — Capability identifier to revoke (for example `wallet.sign`)
* `--reason <TEXT>` — Optional reason included in audit events
* `--torii-url <URL>` — Torii base URL for authoritative `agent/policy/revoke`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud agent-message-send`

Send a deterministic mailbox message between apartments

**Usage:** `iroha app soracloud agent-message-send [OPTIONS] --from-apartment <NAME> --to-apartment <NAME> --payload <TEXT>`

###### **Options:**

* `--from-apartment <NAME>` — Sender apartment name
* `--to-apartment <NAME>` — Recipient apartment name
* `--channel <CHANNEL>` — Logical mailbox channel

  Default value: `default`
* `--payload <TEXT>` — Message payload (UTF-8 text)
* `--torii-url <URL>` — Torii base URL for authoritative `agent/message/send`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud agent-message-ack`

Acknowledge (consume) a mailbox message from an apartment queue

**Usage:** `iroha app soracloud agent-message-ack [OPTIONS] --apartment-name <NAME> --message-id <MESSAGE>`

###### **Options:**

* `--apartment-name <NAME>` — Apartment name consuming the message
* `--message-id <MESSAGE>` — Message identifier emitted by `agent-message-send`
* `--torii-url <URL>` — Torii base URL for authoritative `agent/message/ack`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud agent-mailbox-status`

Inspect mailbox queue state for an apartment

**Usage:** `iroha app soracloud agent-mailbox-status [OPTIONS] --apartment-name <NAME>`

###### **Options:**

* `--apartment-name <NAME>` — Apartment name to inspect
* `--torii-url <URL>` — Torii base URL for authoritative `agent/mailbox/status`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when querying live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane status query

  Default value: `10`



## `iroha app soracloud agent-artifact-allow`

Add an artifact hash (and optional provenance hash) to autonomy allowlist

**Usage:** `iroha app soracloud agent-artifact-allow [OPTIONS] --apartment-name <NAME> --artifact-hash <HASH>`

###### **Options:**

* `--apartment-name <NAME>` — Apartment name whose allowlist should be updated
* `--artifact-hash <HASH>` — Artifact hash identifier
* `--provenance-hash <HASH>` — Optional provenance hash required for this artifact
* `--torii-url <URL>` — Torii base URL for authoritative `agent/autonomy/allow`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud agent-autonomy-run`

Approve an autonomous run under allowlist/provenance/budget guardrails

**Usage:** `iroha app soracloud agent-autonomy-run [OPTIONS] --apartment-name <NAME> --artifact-hash <HASH> --budget-units <UNITS> --run-label <LABEL>`

###### **Options:**

* `--apartment-name <NAME>` — Apartment name requesting autonomous execution
* `--artifact-hash <HASH>` — Artifact hash identifier
* `--provenance-hash <HASH>` — Optional provenance hash for this run request
* `--budget-units <UNITS>` — Budget units requested for this run
* `--run-label <LABEL>` — Human-readable run label
* `--workflow-input-json <JSON>` — Optional canonical JSON body to forward to the generated HF `/infer` handler
* `--workflow-input-json-file <PATH>` — Optional path to a JSON file forwarded to the generated HF `/infer` handler
* `--torii-url <URL>` — Torii base URL for authoritative `agent/autonomy/run`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud agent-autonomy-status`

Show autonomous-run policy state for an apartment

**Usage:** `iroha app soracloud agent-autonomy-status [OPTIONS] --apartment-name <NAME>`

###### **Options:**

* `--apartment-name <NAME>` — Apartment name to inspect
* `--torii-url <URL>` — Torii base URL for authoritative `agent/autonomy/status`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when querying Torii
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane query

  Default value: `10`



## `iroha app soracloud training-job-start`

Start a distributed training job in live Torii control-plane mode

**Usage:** `iroha app soracloud training-job-start [OPTIONS] --service-name <NAME> --model-name <NAME> --job-id <ID> --target-steps <STEPS> --checkpoint-interval-steps <STEPS> --step-compute-units <UNITS> --compute-budget-units <UNITS> --storage-budget-bytes <BYTES>`

###### **Options:**

* `--service-name <NAME>` — Service name that owns the training job
* `--model-name <NAME>` — Model name for the training job
* `--job-id <ID>` — Deterministic training job identifier
* `--worker-group-size <COUNT>` — Worker-group size for the distributed training run

  Default value: `1`
* `--target-steps <STEPS>` — Target number of steps to complete the training job
* `--checkpoint-interval-steps <STEPS>` — Step cadence for checkpoint creation
* `--max-retries <COUNT>` — Maximum allowed retries for the training job

  Default value: `3`
* `--step-compute-units <UNITS>` — Compute units charged per step
* `--compute-budget-units <UNITS>` — Total compute budget units for the training job
* `--storage-budget-bytes <BYTES>` — Total storage budget bytes for checkpoints
* `--torii-url <URL>` — Torii base URL for live control-plane mutation
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud training-job-checkpoint`

Record a training checkpoint in live Torii control-plane mode

**Usage:** `iroha app soracloud training-job-checkpoint [OPTIONS] --service-name <NAME> --job-id <ID> --completed-step <STEP> --checkpoint-size-bytes <BYTES> --metrics-hash <HASH>`

###### **Options:**

* `--service-name <NAME>` — Service name that owns the training job
* `--job-id <ID>` — Training job identifier
* `--completed-step <STEP>` — Completed step represented by this checkpoint
* `--checkpoint-size-bytes <BYTES>` — Checkpoint payload size in bytes
* `--metrics-hash <HASH>` — Hash of metrics/telemetry emitted for this checkpoint
* `--torii-url <URL>` — Torii base URL for live control-plane mutation
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud training-job-retry`

Submit a training retry request in live Torii control-plane mode

**Usage:** `iroha app soracloud training-job-retry [OPTIONS] --service-name <NAME> --job-id <ID> --reason <TEXT>`

###### **Options:**

* `--service-name <NAME>` — Service name that owns the training job
* `--job-id <ID>` — Training job identifier
* `--reason <TEXT>` — Human-readable retry reason recorded in audit logs
* `--torii-url <URL>` — Torii base URL for live control-plane mutation
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud training-job-status`

Query training job status in live Torii control-plane mode

**Usage:** `iroha app soracloud training-job-status [OPTIONS] --service-name <NAME> --job-id <ID>`

###### **Options:**

* `--service-name <NAME>` — Service name that owns the training job
* `--job-id <ID>` — Training job identifier
* `--torii-url <URL>` — Torii base URL for live control-plane query
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane query

  Default value: `10`



## `iroha app soracloud model-artifact-register`

Register model-artifact metadata in live Torii control-plane mode

**Usage:** `iroha app soracloud model-artifact-register [OPTIONS] --service-name <NAME> --model-name <NAME> --training-job-id <ID> --weight-artifact-hash <HASH> --dataset-ref <REF> --training-config-hash <HASH> --reproducibility-hash <HASH> --provenance-attestation-hash <HASH>`

###### **Options:**

* `--service-name <NAME>` — Service name that owns the model
* `--model-name <NAME>` — Model name
* `--training-job-id <ID>` — Training job identifier backing this artifact registration
* `--weight-artifact-hash <HASH>` — Weight artifact hash
* `--dataset-ref <REF>` — Dataset reference identifier
* `--training-config-hash <HASH>` — Hash of training config used for the run
* `--reproducibility-hash <HASH>` — Reproducibility metadata hash
* `--provenance-attestation-hash <HASH>` — Provenance attestation hash
* `--torii-url <URL>` — Torii base URL for live control-plane mutation
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud model-artifact-status`

Query model-artifact status in live Torii control-plane mode

**Usage:** `iroha app soracloud model-artifact-status [OPTIONS] --service-name <NAME> --training-job-id <ID>`

###### **Options:**

* `--service-name <NAME>` — Service name that owns the model artifact
* `--training-job-id <ID>` — Training job identifier associated with the artifact
* `--torii-url <URL>` — Torii base URL for live control-plane query
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane query

  Default value: `10`



## `iroha app soracloud model-weight-register`

Register a model weight version in live Torii control-plane mode

**Usage:** `iroha app soracloud model-weight-register [OPTIONS] --service-name <NAME> --model-name <NAME> --weight-version <VERSION> --training-job-id <ID> --weight-artifact-hash <HASH> --dataset-ref <REF> --training-config-hash <HASH> --reproducibility-hash <HASH> --provenance-attestation-hash <HASH>`

###### **Options:**

* `--service-name <NAME>` — Service name that owns the model
* `--model-name <NAME>` — Model name
* `--weight-version <VERSION>` — New weight version identifier
* `--training-job-id <ID>` — Training job identifier backing this weight version
* `--parent-version <VERSION>` — Optional lineage parent version
* `--weight-artifact-hash <HASH>` — Weight artifact hash
* `--dataset-ref <REF>` — Dataset reference identifier
* `--training-config-hash <HASH>` — Hash of training config used for the run
* `--reproducibility-hash <HASH>` — Reproducibility metadata hash
* `--provenance-attestation-hash <HASH>` — Provenance attestation hash
* `--torii-url <URL>` — Torii base URL for live control-plane mutation
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud model-weight-promote`

Promote a model weight version in live Torii control-plane mode

**Usage:** `iroha app soracloud model-weight-promote [OPTIONS] --service-name <NAME> --model-name <NAME> --weight-version <VERSION> --gate-report-hash <HASH>`

###### **Options:**

* `--service-name <NAME>` — Service name that owns the model
* `--model-name <NAME>` — Model name
* `--weight-version <VERSION>` — Weight version to promote
* `--gate-approved` — Gate approval flag
* `--gate-report-hash <HASH>` — Hash of gate report/evidence for this promotion decision
* `--torii-url <URL>` — Torii base URL for live control-plane mutation
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud model-weight-rollback`

Roll back a model weight version in live Torii control-plane mode

**Usage:** `iroha app soracloud model-weight-rollback [OPTIONS] --service-name <NAME> --model-name <NAME> --target-version <VERSION> --reason <TEXT>`

###### **Options:**

* `--service-name <NAME>` — Service name that owns the model
* `--model-name <NAME>` — Model name
* `--target-version <VERSION>` — Target version to roll back to
* `--reason <TEXT>` — Human-readable rollback reason
* `--torii-url <URL>` — Torii base URL for live control-plane mutation
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud model-weight-status`

Query model weight status in live Torii control-plane mode

**Usage:** `iroha app soracloud model-weight-status [OPTIONS] --service-name <NAME> --model-name <NAME>`

###### **Options:**

* `--service-name <NAME>` — Service name that owns the model
* `--model-name <NAME>` — Model name
* `--torii-url <URL>` — Torii base URL for live control-plane query
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane query

  Default value: `10`



## `iroha app soracloud model-upload-encryption-recipient`

Fetch the active uploaded-model encryption recipient from Torii

**Usage:** `iroha app soracloud model-upload-encryption-recipient [OPTIONS]`

###### **Options:**

* `--torii-url <URL>` — Torii base URL for authoritative `model/upload/encryption-recipient`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when querying Torii
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane query

  Default value: `10`



## `iroha app soracloud model-upload-init`

Register an uploaded-model bundle root in live Torii control-plane mode

**Usage:** `iroha app soracloud model-upload-init [OPTIONS] --bundle-file <PATH>`

###### **Options:**

* `--bundle-file <PATH>` — Path to a `SoraUploadedModelBundleV1` JSON document
* `--torii-url <URL>` — Torii base URL for authoritative `model/upload/init`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud model-upload-chunk`

Append one uploaded-model encrypted chunk in live Torii control-plane mode

**Usage:** `iroha app soracloud model-upload-chunk [OPTIONS] --chunk-file <PATH>`

###### **Options:**

* `--chunk-file <PATH>` — Path to a `SoraUploadedModelChunkV1` JSON document
* `--torii-url <URL>` — Torii base URL for authoritative `model/upload/chunk`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud model-upload-finalize`

Finalize an uploaded-model bundle into the model registry

**Usage:** `iroha app soracloud model-upload-finalize [OPTIONS] --request-file <PATH>`

###### **Options:**

* `--request-file <PATH>` — Path to an `UploadedModelFinalizePayload` JSON document
* `--torii-url <URL>` — Torii base URL for authoritative `model/upload/finalize`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud model-upload-status`

Query uploaded-model bundle/chunk status in live Torii control-plane mode

**Usage:** `iroha app soracloud model-upload-status [OPTIONS] --service-name <NAME> --weight-version <VERSION>`

###### **Options:**

* `--service-name <NAME>` — Service name that owns the uploaded model
* `--weight-version <VERSION>` — Uploaded-model pinned weight version
* `--model-id <ID>` — Optional uploaded-model identifier
* `--model-name <NAME>` — Optional logical model name used to resolve the uploaded-model record
* `--bundle-root <HASH>` — Optional bundle-root filter
* `--compile-profile-hash <HASH>` — Optional compile-profile hash filter
* `--torii-url <URL>` — Torii base URL for authoritative `model/upload/status`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane query

  Default value: `10`



## `iroha app soracloud model-compile`

Admit a deterministic private compile profile for an uploaded model

**Usage:** `iroha app soracloud model-compile [OPTIONS] --request-file <PATH>`

###### **Options:**

* `--request-file <PATH>` — Path to a `PrivateCompilePayload` JSON document
* `--torii-url <URL>` — Torii base URL for authoritative `model/compile`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud model-compile-status`

Query uploaded-model compile status in live Torii control-plane mode

**Usage:** `iroha app soracloud model-compile-status [OPTIONS] --service-name <NAME> --weight-version <VERSION>`

###### **Options:**

* `--service-name <NAME>` — Service name that owns the uploaded model
* `--weight-version <VERSION>` — Uploaded-model pinned weight version
* `--model-id <ID>` — Optional uploaded-model identifier
* `--model-name <NAME>` — Optional logical model name used to resolve the uploaded-model record
* `--bundle-root <HASH>` — Optional bundle-root filter
* `--compile-profile-hash <HASH>` — Optional compile-profile hash filter
* `--torii-url <URL>` — Torii base URL for authoritative `model/compile/status`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane query

  Default value: `10`



## `iroha app soracloud model-allow`

Bind an uploaded model to an apartment that already admits model inference

**Usage:** `iroha app soracloud model-allow [OPTIONS] --request-file <PATH>`

###### **Options:**

* `--request-file <PATH>` — Path to an `UploadedModelAllowPayload` JSON document
* `--torii-url <URL>` — Torii base URL for authoritative `model/allow`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud model-run-private`

Start and finalize a private uploaded-model inference session

**Usage:** `iroha app soracloud model-run-private [OPTIONS] --session-file <PATH>`

###### **Options:**

* `--session-file <PATH>` — Path to a `SoraPrivateInferenceSessionV1` JSON document
* `--torii-url <URL>` — Torii base URL for authoritative `model/run-private`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud model-run-status`

Query private uploaded-model inference session status

**Usage:** `iroha app soracloud model-run-status [OPTIONS] --session-id <ID>`

###### **Options:**

* `--session-id <ID>` — Private inference session identifier
* `--torii-url <URL>` — Torii base URL for authoritative `model/run-status`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane query

  Default value: `10`



## `iroha app soracloud model-decrypt-output`

Release governed output material for a private uploaded-model session

**Usage:** `iroha app soracloud model-decrypt-output [OPTIONS] --session-id <ID> --decrypt-request-id <ID>`

###### **Options:**

* `--session-id <ID>` — Private inference session identifier
* `--decrypt-request-id <ID>` — Decryption request identifier to release
* `--torii-url <URL>` — Torii base URL for authoritative `model/decrypt-output`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutation

  Default value: `10`



## `iroha app soracloud model-publish-private`

Orchestrate uploaded-model publish/init/chunk/finalize/compile/allow from a plan file

**Usage:** `iroha app soracloud model-publish-private [OPTIONS]`

###### **Options:**

* `--plan-file <PATH>` — Path to a prepared `PrivateModelPublishPlan` JSON document
* `--draft-file <PATH>` — Path to a source-backed `PrivateModelPublishDraft` JSON document
* `--emit-plan-file <PATH>` — Optional path where the prepared publish plan should be written
* `--torii-url <URL>` — Torii base URL for authoritative uploaded-model control-plane routes
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token`
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations and queries

  Default value: `10`



## `iroha app soracloud hf-deploy`

Join or create a shared Hugging Face lease pool in live Torii control-plane mode

**Usage:** `iroha app soracloud hf-deploy [OPTIONS] --repo-id <REPO> --service-name <NAME> --lease-term-ms <MS> --lease-asset-definition <ASSET> --base-fee-nanos <NANOS>`

###### **Options:**

* `--repo-id <REPO>` — Hugging Face repository identifier (for example `openai/gpt-oss`)
* `--revision <REVISION>` — Optional Hugging Face revision. Defaults to `main` when omitted
* `--model-name <NAME>` — Optional local model label. Defaults to the repo slug
* `--service-name <NAME>` — Soracloud service name bound to this lease membership
* `--apartment-name <NAME>` — Optional agent apartment name bound to this lease membership
* `--storage-class <STORAGE_CLASS>` — Shared-lease storage tier

  Default value: `warm`

  Possible values: `hot`, `warm`, `cold`

* `--lease-term-ms <MS>` — Shared-lease window length in milliseconds
* `--lease-asset-definition <ASSET>` — Settlement asset definition identifier
* `--base-fee-nanos <NANOS>` — Base lease fee, charged in nanos of the settlement asset
* `--torii-url <URL>` — Torii base URL for authoritative `hf/deploy`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud hf-status`

Query shared Hugging Face lease pool status in live Torii control-plane mode

**Usage:** `iroha app soracloud hf-status [OPTIONS] --repo-id <REPO> --lease-term-ms <MS>`

###### **Options:**

* `--repo-id <REPO>` — Hugging Face repository identifier (for example `openai/gpt-oss`)
* `--revision <REVISION>` — Optional Hugging Face revision. Defaults to `main` when omitted
* `--storage-class <STORAGE_CLASS>` — Shared-lease storage tier

  Default value: `warm`

  Possible values: `hot`, `warm`, `cold`

* `--lease-term-ms <MS>` — Shared-lease window length in milliseconds
* `--account-id <ACCOUNT>` — Optional account filter for membership-specific status
* `--torii-url <URL>` — Torii base URL for authoritative `hf/status`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when querying live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane queries

  Default value: `10`



## `iroha app soracloud hf-lease-leave`

Leave a shared Hugging Face lease pool in live Torii control-plane mode

**Usage:** `iroha app soracloud hf-lease-leave [OPTIONS] --repo-id <REPO> --lease-term-ms <MS>`

###### **Options:**

* `--repo-id <REPO>` — Hugging Face repository identifier
* `--revision <REVISION>` — Optional Hugging Face revision. Defaults to `main` when omitted
* `--storage-class <STORAGE_CLASS>` — Shared-lease storage tier

  Default value: `warm`

  Possible values: `hot`, `warm`, `cold`

* `--lease-term-ms <MS>` — Shared-lease window length in milliseconds
* `--service-name <NAME>` — Optional service binding to include in the signed leave request
* `--apartment-name <NAME>` — Optional apartment binding to include in the signed leave request
* `--torii-url <URL>` — Torii base URL for authoritative `hf/lease/leave`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud hf-lease-renew`

Renew an expired or drained shared Hugging Face lease pool window

**Usage:** `iroha app soracloud hf-lease-renew [OPTIONS] --repo-id <REPO> --service-name <NAME> --lease-term-ms <MS> --lease-asset-definition <ASSET> --base-fee-nanos <NANOS>`

###### **Options:**

* `--repo-id <REPO>` — Hugging Face repository identifier
* `--revision <REVISION>` — Optional Hugging Face revision. Defaults to `main` when omitted
* `--model-name <NAME>` — Optional local model label. Defaults to the repo slug
* `--service-name <NAME>` — Soracloud service name bound to the renewed lease membership
* `--apartment-name <NAME>` — Optional agent apartment name bound to the renewed lease membership
* `--storage-class <STORAGE_CLASS>` — Shared-lease storage tier

  Default value: `warm`

  Possible values: `hot`, `warm`, `cold`

* `--lease-term-ms <MS>` — Shared-lease window length in milliseconds
* `--lease-asset-definition <ASSET>` — Settlement asset definition identifier
* `--base-fee-nanos <NANOS>` — Base lease fee, charged in nanos of the settlement asset
* `--torii-url <URL>` — Torii base URL for authoritative `hf/lease/renew`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud model-host-advertise`

Advertise validator-host capabilities for authoritative HF placement

**Usage:** `iroha app soracloud model-host-advertise [OPTIONS] --peer-id <PEER_ID> --max-model-bytes <BYTES> --max-disk-cache-bytes <BYTES> --max-ram-bytes <BYTES> --max-concurrent-resident-models <COUNT> --host-class <CLASS> --heartbeat-expires-at-ms <UNIX_MS>`

###### **Options:**

* `--peer-id <PEER_ID>` — Peer identifier used for Soracloud routing
* `--backends <BACKENDS>` — Supported backend families

  Possible values: `transformers`, `gguf`

* `--formats <FORMATS>` — Supported model formats

  Possible values: `safetensors`, `pytorch`, `gguf`

* `--max-model-bytes <BYTES>` — Maximum canonical model bytes accepted by this host
* `--max-disk-cache-bytes <BYTES>` — Maximum disk cache bytes reserved for resident models
* `--max-ram-bytes <BYTES>` — Maximum system RAM bytes reserved for resident models
* `--max-vram-bytes <BYTES>` — Maximum accelerator VRAM bytes reserved for resident models

  Default value: `0`
* `--max-concurrent-resident-models <COUNT>` — Maximum concurrent resident-model slots
* `--host-class <CLASS>` — Governance-defined host class used for compute tariff lookup
* `--heartbeat-expires-at-ms <UNIX_MS>` — Heartbeat expiry timestamp (unix ms) for this advert
* `--torii-url <URL>` — Torii base URL for authoritative `model-host/advertise`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud model-host-heartbeat`

Refresh the heartbeat TTL for an advertised model host

**Usage:** `iroha app soracloud model-host-heartbeat [OPTIONS] --heartbeat-expires-at-ms <UNIX_MS>`

###### **Options:**

* `--heartbeat-expires-at-ms <UNIX_MS>` — Heartbeat expiry timestamp (unix ms) for this advert
* `--torii-url <URL>` — Torii base URL for authoritative `model-host/heartbeat`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud model-host-withdraw`

Withdraw an advertised model host capability

**Usage:** `iroha app soracloud model-host-withdraw [OPTIONS]`

###### **Options:**

* `--torii-url <URL>` — Torii base URL for authoritative `model-host/withdraw`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when mutating live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane mutations

  Default value: `10`



## `iroha app soracloud model-host-status`

Query authoritative model-host capability adverts

**Usage:** `iroha app soracloud model-host-status [OPTIONS]`

###### **Options:**

* `--validator-account-id <ACCOUNT>` — Optional validator account identifier filter
* `--torii-url <URL>` — Torii base URL for authoritative `model-host/status`
* `--api-token <TOKEN>` — Optional API token sent as `x-api-token` when querying live control-plane APIs
* `--timeout-secs <SECS>` — HTTP timeout for live control-plane queries

  Default value: `10`



## `iroha app social`

Social incentive helpers (viral follow rewards and escrows)

**Usage:** `iroha app social <COMMAND>`

###### **Subcommands:**

* `claim-twitter-follow-reward` — Claim a promotional reward for a verified Twitter follow binding
* `send-to-twitter` — Send funds to a Twitter handle; funds are escrowed until a follow binding appears
* `cancel-twitter-escrow` — Cancel an existing escrow created by `send-to-twitter`



## `iroha app social claim-twitter-follow-reward`

Claim a promotional reward for a verified Twitter follow binding

**Usage:** `iroha app social claim-twitter-follow-reward --binding-hash-json <PATH>`

###### **Options:**

* `--binding-hash-json <PATH>` — Path to a JSON file containing a `KeyedHash` (binding hash) payload.

   The JSON shape must match `iroha_data_model::oracle::KeyedHash`.



## `iroha app social send-to-twitter`

Send funds to a Twitter handle; funds are escrowed until a follow binding appears

**Usage:** `iroha app social send-to-twitter --binding-hash-json <PATH> --amount <AMOUNT>`

###### **Options:**

* `--binding-hash-json <PATH>` — Path to a JSON file containing a `KeyedHash` (binding hash) payload.

   The JSON shape must match `iroha_data_model::oracle::KeyedHash`.
* `--amount <AMOUNT>` — Amount to escrow or deliver immediately when the binding is already active.

   Parsed as `Numeric` (mantissa/scale) using the standard string format.



## `iroha app social cancel-twitter-escrow`

Cancel an existing escrow created by `send-to-twitter`

**Usage:** `iroha app social cancel-twitter-escrow --binding-hash-json <PATH>`

###### **Options:**

* `--binding-hash-json <PATH>` — Path to a JSON file containing a `KeyedHash` (binding hash) payload.

   The JSON shape must match `iroha_data_model::oracle::KeyedHash`.



## `iroha app space-directory`

Space Directory helpers (UAID capability manifests)

**Usage:** `iroha app space-directory <COMMAND>`

###### **Subcommands:**

* `manifest` — Manage UAID capability manifests
* `bindings` — Inspect UAID bindings surfaced by Torii



## `iroha app space-directory manifest`

Manage UAID capability manifests

**Usage:** `iroha app space-directory manifest <COMMAND>`

###### **Subcommands:**

* `publish` — Publish or replace a capability manifest (.to payload)
* `encode` — Encode manifest JSON into Norito bytes and record its hash
* `revoke` — Revoke a manifest for a UAID/dataspace pair
* `expire` — Expire a manifest that reached its scheduled end-of-life
* `audit-bundle` — Produce an audit bundle for an existing capability manifest + dataspace profile
* `fetch` — Fetch manifests for a UAID via Torii
* `scaffold` — Scaffold manifest/profile templates for a UAID + dataspace pair



## `iroha app space-directory manifest publish`

Publish or replace a capability manifest (.to payload)

**Usage:** `iroha app space-directory manifest publish [OPTIONS]`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded `AssetPermissionManifest` (.to)
* `--manifest-json <PATH>` — Path to the JSON `AssetPermissionManifest` (encoded on submit)
* `--reason <TEXT>` — Optional CLI-level reason used when publishing a new manifest (added to metadata)



## `iroha app space-directory manifest encode`

Encode manifest JSON into Norito bytes and record its hash

**Usage:** `iroha app space-directory manifest encode [OPTIONS] --json <PATH>`

###### **Options:**

* `--json <PATH>` — Path to the JSON `AssetPermissionManifest`
* `--out <PATH>` — Target path for the Norito `.to` payload (defaults to `<json>.manifest.to`)
* `--hash-out <PATH>` — Optional file for the manifest hash (defaults to `<out>.hash`)



## `iroha app space-directory manifest revoke`

Revoke a manifest for a UAID/dataspace pair

**Usage:** `iroha app space-directory manifest revoke [OPTIONS] --uaid <UAID> --dataspace <ID> --revoked-epoch <EPOCH>`

###### **Options:**

* `--uaid <UAID>` — UAID whose manifest should be revoked
* `--dataspace <ID>` — Dataspace identifier hosting the manifest
* `--revoked-epoch <EPOCH>` — Epoch (inclusive) when the revocation takes effect
* `--reason <TEXT>` — Optional reason recorded with the revocation



## `iroha app space-directory manifest expire`

Expire a manifest that reached its scheduled end-of-life

**Usage:** `iroha app space-directory manifest expire --uaid <UAID> --dataspace <ID> --expired-epoch <EPOCH>`

###### **Options:**

* `--uaid <UAID>` — UAID whose manifest should be expired
* `--dataspace <ID>` — Dataspace identifier hosting the manifest
* `--expired-epoch <EPOCH>` — Epoch (inclusive) when the expiry occurred



## `iroha app space-directory manifest audit-bundle`

Produce an audit bundle for an existing capability manifest + dataspace profile

**Usage:** `iroha app space-directory manifest audit-bundle [OPTIONS] --profile <PATH> --out-dir <DIR>`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded `AssetPermissionManifest` (.to)
* `--manifest-json <PATH>` — Path to the JSON `AssetPermissionManifest` (encoded on export)
* `--profile <PATH>` — Dataspace profile JSON used to capture governance/audit hooks
* `--out-dir <DIR>` — Directory where the bundle (manifest/profile/hash/audit metadata) will be written
* `--notes <TEXT>` — Optional operator note recorded inside the bundle metadata



## `iroha app space-directory manifest fetch`

Fetch manifests for a UAID via Torii

**Usage:** `iroha app space-directory manifest fetch [OPTIONS] --uaid <UAID>`

###### **Options:**

* `--uaid <UAID>` — UAID literal whose manifests should be fetched
* `--dataspace <ID>` — Optional dataspace id filter
* `--status <STATUS>` — Manifest lifecycle status filter (active, inactive, all)

  Default value: `all`

  Possible values: `active`, `inactive`, `all`

* `--limit <N>` — Maximum number of manifests to return
* `--offset <N>` — Offset for pagination
* `--json-out <PATH>` — Optional path where the JSON response will be stored



## `iroha app space-directory manifest scaffold`

Scaffold manifest/profile templates for a UAID + dataspace pair

**Usage:** `iroha app space-directory manifest scaffold [OPTIONS] --uaid <UAID> --dataspace <ID> --activation-epoch <EPOCH>`

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
* `--allow-asset <ASSET-ID>` — Asset identifier (e.g. `61CtjvNd9T3THAR65GsMVHr82Bjc`) for the allow entry
* `--allow-role <ROLE>` — AMX role enforced by the allow entry (`initiator` or `participant`)
* `--allow-max-amount <DECIMAL>` — Deterministic allowance cap (decimal string)
* `--allow-window <WINDOW>` — Allowance window (`per-slot`, `per-minute`, or `per-day`)
* `--allow-notes <TEXT>` — Optional operator note stored alongside the entry
* `--deny-dataspace <ID>` — Optional dataspace override for the deny entry scope
* `--deny-program <PROGRAM>` — Program identifier (`contract.name`) for the deny entry
* `--deny-method <NAME>` — Method/entry-point for the deny entry
* `--deny-asset <ASSET-ID>` — Asset identifier (e.g. `61CtjvNd9T3THAR65GsMVHr82Bjc`) for the deny entry
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



## `iroha app space-directory bindings`

Inspect UAID bindings surfaced by Torii

**Usage:** `iroha app space-directory bindings <COMMAND>`

###### **Subcommands:**

* `fetch` — Fetch UAID dataspace bindings via Torii



## `iroha app space-directory bindings fetch`

Fetch UAID dataspace bindings via Torii

**Usage:** `iroha app space-directory bindings fetch [OPTIONS] --uaid <UAID>`

###### **Options:**

* `--uaid <UAID>` — UAID literal whose bindings should be fetched
* `--json-out <PATH>` — Optional path where the JSON response will be stored



## `iroha app kaigi`

Kaigi session helpers

**Usage:** `iroha app kaigi <COMMAND>`

###### **Subcommands:**

* `create` — Create a new Kaigi session
* `quickstart` — Bootstrap a Kaigi session for demos and shareable testing metadata
* `register-relay` — Register or update a Kaigi relay descriptor
* `set-relay-manifest` — Replace or clear the relay manifest for an existing Kaigi session
* `join` — Join a Kaigi session
* `leave` — Leave a Kaigi session
* `end` — End an active Kaigi session
* `record-usage` — Record usage statistics for a Kaigi session
* `report-relay-health` — Report the health status of a relay used by a Kaigi session



## `iroha app kaigi create`

Create a new Kaigi session

**Usage:** `iroha app kaigi create [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --host <ACCOUNT-ID>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call (e.g. `kaigi`)
* `--call-name <NAME>` — Call name within the domain (e.g. `daily-sync`)
* `--host <ACCOUNT-ID>` — Host account identifier responsible for the call (canonical I105 account literal)
* `--title <TITLE>` — Optional human friendly title
* `--description <DESCRIPTION>` — Optional description for participants
* `--max-participants <U32>` — Maximum concurrent participants (excluding host)
* `--gas-rate-per-minute <U64>` — Gas rate charged per minute (defaults to 0)

  Default value: `0`
* `--billing-account <ACCOUNT-ID>` — Optional billing account that will cover usage (canonical I105 account literal)
* `--scheduled-start-ms <U64>` — Optional scheduled start timestamp (milliseconds since epoch)
* `--privacy-mode <PRIVACY_MODE>` — Privacy mode for the session (defaults to `transparent`)

  Default value: `transparent`

  Possible values: `transparent`, `zk-roster-v1`

* `--room-policy <ROOM_POLICY>` — Room access policy controlling viewer authentication

  Default value: `authenticated`

  Possible values: `public`, `authenticated`

* `--relay-manifest <PATH>` — Path to a JSON file describing the relay manifest (optional)
* `--metadata-json <PATH>` — Path to a JSON file providing additional metadata (object with string keys)
* `--commitment-hex <HEX>` — Commitment hash (hex) for privacy mode creation
* `--commitment-alias <COMMITMENT_ALIAS>` — Alias tag describing the host commitment (privacy mode)
* `--nullifier-hex <HEX>` — Nullifier hash (hex) preventing proof replay (privacy mode)
* `--nullifier-issued-at-ms <U64>` — Nullifier issuance timestamp (milliseconds since epoch)
* `--roster-root-hex <HEX>` — Roster Merkle root bound into the proof transcript (privacy mode)
* `--proof-hex <HEX>` — Proof bytes attesting ownership (hex encoding of raw bytes)



## `iroha app kaigi quickstart`

Bootstrap a Kaigi session for demos and shareable testing metadata

**Usage:** `iroha app kaigi quickstart [OPTIONS]`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call

  Default value: `wonderland`
* `--call-name <NAME>` — Call name within the domain (defaults to a timestamp-based identifier)
* `--host <ACCOUNT-ID>` — Host account identifier responsible for the call (canonical I105 account literal)
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



## `iroha app kaigi register-relay`

Register or update a Kaigi relay descriptor

**Usage:** `iroha app kaigi register-relay --relay <ACCOUNT-ID> --hpke-public-key-b64 <BASE64> --bandwidth-class <U8>`

###### **Options:**

* `--relay <ACCOUNT-ID>` — Relay account identifier advertising relay capabilities (canonical I105 account literal)
* `--hpke-public-key-b64 <BASE64>` — HPKE public key bytes advertised by the relay (base64-encoded raw bytes)
* `--bandwidth-class <U8>` — Relative bandwidth class advertised by the relay



## `iroha app kaigi set-relay-manifest`

Replace or clear the relay manifest for an existing Kaigi session

**Usage:** `iroha app kaigi set-relay-manifest [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--relay-manifest <PATH>` — Path to a JSON file describing the relay manifest
* `--clear` — Clear the stored relay manifest entirely



## `iroha app kaigi join`

Join a Kaigi session

**Usage:** `iroha app kaigi join [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --participant <ACCOUNT-ID>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--participant <ACCOUNT-ID>` — Participant account joining the call (canonical I105 account literal)
* `--commitment-hex <HEX>` — Commitment hash (hex) for privacy mode joins
* `--commitment-alias <COMMITMENT_ALIAS>` — Alias tag describing the commitment (privacy mode)
* `--nullifier-hex <HEX>` — Nullifier hash (hex) preventing duplicate joins (privacy mode)
* `--nullifier-issued-at-ms <U64>` — Nullifier issuance timestamp (milliseconds since epoch)
* `--roster-root-hex <HEX>` — Roster Merkle root bound into the proof transcript (privacy mode)
* `--proof-hex <HEX>` — Proof bytes attesting ownership (hex encoding of raw bytes)



## `iroha app kaigi leave`

Leave a Kaigi session

**Usage:** `iroha app kaigi leave [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --participant <ACCOUNT-ID>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--participant <ACCOUNT-ID>` — Participant account leaving the call (canonical I105 account literal)
* `--commitment-hex <HEX>` — Commitment hash (hex) identifying the participant in privacy mode
* `--nullifier-hex <HEX>` — Nullifier hash (hex) preventing duplicate leaves (privacy mode)
* `--nullifier-issued-at-ms <U64>` — Nullifier issuance timestamp (milliseconds since epoch)
* `--roster-root-hex <HEX>` — Roster Merkle root bound into the proof transcript (privacy mode)
* `--proof-hex <HEX>` — Proof bytes attesting ownership (hex encoding of raw bytes)



## `iroha app kaigi end`

End an active Kaigi session

**Usage:** `iroha app kaigi end [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--ended-at-ms <U64>` — Optional timestamp in milliseconds when the call ended
* `--commitment-hex <HEX>` — Commitment hash (hex) for privacy mode end requests
* `--commitment-alias <COMMITMENT_ALIAS>` — Alias tag describing the host commitment (privacy mode)
* `--nullifier-hex <HEX>` — Nullifier hash (hex) preventing proof replay (privacy mode)
* `--nullifier-issued-at-ms <U64>` — Nullifier issuance timestamp (milliseconds since epoch)
* `--roster-root-hex <HEX>` — Roster Merkle root bound into the proof transcript (privacy mode)
* `--proof-hex <HEX>` — Proof bytes attesting ownership (hex encoding of raw bytes)



## `iroha app kaigi record-usage`

Record usage statistics for a Kaigi session

**Usage:** `iroha app kaigi record-usage [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --duration-ms <U64>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--duration-ms <U64>` — Duration in milliseconds for this usage segment
* `--billed-gas <U64>` — Gas billed for this segment

  Default value: `0`
* `--usage-commitment-hex <HEX>` — Optional usage commitment hash (privacy mode)
* `--proof-hex <HEX>` — Optional proof bytes attesting the usage delta (privacy mode)



## `iroha app kaigi report-relay-health`

Report the health status of a relay used by a Kaigi session

**Usage:** `iroha app kaigi report-relay-health [OPTIONS] --domain <DOMAIN-ID> --call-name <NAME> --relay <ACCOUNT-ID> --status <STATUS> --reported-at-ms <U64>`

###### **Options:**

* `--domain <DOMAIN-ID>` — Domain identifier hosting the call
* `--call-name <NAME>` — Call name within the domain
* `--relay <ACCOUNT-ID>` — Relay account identifier being reported (canonical I105 account literal)
* `--status <STATUS>` — Observed health status for the relay

  Possible values: `healthy`, `degraded`, `unavailable`

* `--reported-at-ms <U64>` — Timestamp in milliseconds when the status was observed
* `--notes <NOTES>` — Optional notes capturing failure or recovery context



## `iroha app sorafs`

SoraFS helpers (pin registry, aliases, replication orders, storage)

**Usage:** `iroha app sorafs <COMMAND>`

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



## `iroha app sorafs pin`

Interact with the pin registry

**Usage:** `iroha app sorafs pin <COMMAND>`

###### **Subcommands:**

* `list` — List manifests registered in the pin registry
* `show` — Fetch a single manifest, aliases, and replication orders
* `register` — Register a manifest in the pin registry via Torii



## `iroha app sorafs pin list`

List manifests registered in the pin registry

**Usage:** `iroha app sorafs pin list [OPTIONS]`

###### **Options:**

* `--status <STATUS>` — Optional status filter (pending, approved, retired)
* `--limit <LIMIT>` — Maximum number of manifests to return
* `--offset <OFFSET>` — Offset for pagination



## `iroha app sorafs pin show`

Fetch a single manifest, aliases, and replication orders

**Usage:** `iroha app sorafs pin show --digest <HEX>`

###### **Options:**

* `--digest <HEX>` — Hex-encoded manifest digest



## `iroha app sorafs pin register`

Register a manifest in the pin registry via Torii

**Usage:** `iroha app sorafs pin register [OPTIONS] --manifest <PATH> --chunk-digest <HEX> --submitted-epoch <SUBMITTED_EPOCH>`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded manifest (`.to`) file
* `--chunk-digest <HEX>` — Hex-encoded SHA3-256 digest of the chunk metadata plan
* `--submitted-epoch <SUBMITTED_EPOCH>` — Epoch recorded when submitting the manifest
* `--alias-namespace <ALIAS_NAMESPACE>` — Optional alias namespace to bind alongside the manifest
* `--alias-name <ALIAS_NAME>` — Optional alias name to bind alongside the manifest
* `--alias-proof <PATH>` — Optional path to the alias proof payload (binary)
* `--successor-of <HEX>` — Optional predecessor manifest digest (hex)



## `iroha app sorafs alias`

List alias bindings

**Usage:** `iroha app sorafs alias <COMMAND>`

###### **Subcommands:**

* `list` — List alias bindings exposed via Torii



## `iroha app sorafs alias list`

List alias bindings exposed via Torii

**Usage:** `iroha app sorafs alias list [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of aliases to return
* `--offset <OFFSET>` — Offset for pagination
* `--namespace <NAMESPACE>` — Restrict aliases to a namespace (case-insensitive)
* `--manifest-digest <HEX>` — Restrict aliases bound to a manifest digest (hex-encoded)



## `iroha app sorafs replication`

List replication orders

**Usage:** `iroha app sorafs replication <COMMAND>`

###### **Subcommands:**

* `list` — List replication orders



## `iroha app sorafs replication list`

List replication orders

**Usage:** `iroha app sorafs replication list [OPTIONS]`

###### **Options:**

* `--limit <LIMIT>` — Maximum number of orders to return
* `--offset <OFFSET>` — Offset for pagination
* `--status <STATUS>` — Optional status filter (pending, completed, expired)
* `--manifest-digest <HEX>` — Restrict to orders for a manifest digest (hex-encoded)



## `iroha app sorafs storage`

Storage helpers (pin, etc.)

**Usage:** `iroha app sorafs storage <COMMAND>`

###### **Subcommands:**

* `pin` — Submit a manifest + payload to local storage for pinning
* `token` — Issue and inspect stream tokens for chunk-range gateways



## `iroha app sorafs storage pin`

Submit a manifest + payload to local storage for pinning

**Usage:** `iroha app sorafs storage pin --manifest <PATH> --payload <PATH>`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded manifest (`.to` file)
* `--payload <PATH>` — Path to the raw payload bytes referenced by the manifest



## `iroha app sorafs storage token`

Issue and inspect stream tokens for chunk-range gateways

**Usage:** `iroha app sorafs storage token <COMMAND>`

###### **Subcommands:**

* `issue` — Issue a stream token for a manifest/provider pair



## `iroha app sorafs storage token issue`

Issue a stream token for a manifest/provider pair

**Usage:** `iroha app sorafs storage token issue [OPTIONS] --manifest-id <HEX> --provider-id <HEX> --client-id <STRING>`

###### **Options:**

* `--manifest-id <HEX>` — Hex-encoded manifest identifier stored on the gateway
* `--provider-id <HEX>` — Hex-encoded provider identifier authorised to serve the manifest
* `--client-id <STRING>` — Logical client identifier used for quota accounting
* `--nonce <STRING>` — Optional nonce to send in the request headers (auto-generated when omitted)
* `--ttl-secs <SECONDS>` — Override the default TTL expressed in seconds
* `--max-streams <COUNT>` — Override the maximum concurrent stream count
* `--rate-limit-bytes <BYTES>` — Override the sustained throughput limit in bytes per second
* `--requests-per-minute <COUNT>` — Override the allowed number of refresh requests per minute



## `iroha app sorafs gateway`

Gateway policy and configuration helpers

**Usage:** `iroha app sorafs gateway <COMMAND>`

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



## `iroha app sorafs gateway lint-denylist`

Validate a denylist file against gateway policy rules

**Usage:** `iroha app sorafs gateway lint-denylist --path <PATH>`

###### **Options:**

* `--path <PATH>` — Path to the JSON denylist file to validate



## `iroha app sorafs gateway update-denylist`

Apply additions/removals to a denylist bundle with deterministic ordering

**Usage:** `iroha app sorafs gateway update-denylist [OPTIONS] --base <PATH>`

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



## `iroha app sorafs gateway template-config`

Emit a TOML snippet with gateway configuration defaults

**Usage:** `iroha app sorafs gateway template-config [OPTIONS]`

###### **Options:**

* `--host <HOSTNAME>` — Hostname to include in the ACME / gateway sample (repeatable)
* `--denylist-path <PATH>` — Optional denylist path to include in the template

  Default value: `docs/source/sorafs_gateway_denylist_sample.json`



## `iroha app sorafs gateway generate-hosts`

Derive canonical/vanity hostnames for a provider

**Usage:** `iroha app sorafs gateway generate-hosts [OPTIONS] --provider-id <HEX>`

###### **Options:**

* `--provider-id <HEX>` — Provider identifier (hex, 32 bytes)
* `--chain-id <CHAIN_ID>` — Chain id (network identifier)

  Default value: `nexus`



## `iroha app sorafs gateway route-plan`

Render the headers + route binding plan for a manifest rollout

**Usage:** `iroha app sorafs gateway route-plan [OPTIONS] --manifest-json <PATH> --hostname <HOSTNAME>`

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



## `iroha app sorafs gateway cache-invalidate`

Generate a cache invalidation payload and curl snippet for GAR/SoraFS gateways

**Usage:** `iroha app sorafs gateway cache-invalidate [OPTIONS] --endpoint <URL> --alias <NAMESPACE:NAME> --manifest-digest <HEX>`

###### **Options:**

* `--endpoint <URL>` — Cache invalidation API endpoint (HTTP/S)
* `--alias <NAMESPACE:NAME>` — Alias bindings (`namespace:name`) that should be purged (repeatable)
* `--manifest-digest <HEX>` — Manifest digest (hex, 32 bytes) associated with the release
* `--car-digest <HEX>` — Optional CAR digest (hex, 32 bytes) to attach to the request
* `--release-tag <STRING>` — Optional release tag metadata included in the payload
* `--auth-env <ENV>` — Environment variable that stores the cache purge bearer token

  Default value: `CACHE_PURGE_TOKEN`
* `--output <PATH>` — Optional path where the JSON payload will be written



## `iroha app sorafs gateway evidence`

Emit an evidence summary for a denylist bundle

**Usage:** `iroha app sorafs gateway evidence [OPTIONS]`

###### **Options:**

* `--denylist <PATH>` — Path to the JSON denylist file to summarise

  Default value: `docs/source/sorafs_gateway_denylist_sample.json`
* `--out <PATH>` — Output path for the evidence JSON bundle

  Default value: `artifacts/sorafs_gateway/denylist_evidence.json`
* `--label <STRING>` — Optional evidence label embedded in the output



## `iroha app sorafs gateway direct-mode`

Direct-mode planning and configuration helpers

**Usage:** `iroha app sorafs gateway direct-mode <COMMAND>`

###### **Subcommands:**

* `plan` — Analyse manifest/admission data and emit a direct-mode readiness plan
* `enable` — Emit a configuration snippet enabling direct-mode overrides from a plan
* `rollback` — Emit a configuration snippet restoring default gateway security settings



## `iroha app sorafs gateway direct-mode plan`

Analyse manifest/admission data and emit a direct-mode readiness plan

**Usage:** `iroha app sorafs gateway direct-mode plan [OPTIONS] --manifest <PATH>`

###### **Options:**

* `--manifest <PATH>` — Path to the Norito-encoded manifest (`.to`) file to analyse
* `--admission-envelope <PATH>` — Optional provider admission envelope (`.to`) for capability detection
* `--provider-id <HEX>` — Override provider identifier (hex) when no admission envelope is supplied
* `--chain-id <CHAIN_ID>` — Override chain id (defaults to the CLI configuration chain id)
* `--scheme <SCHEME>` — URL scheme to use for generated direct-CAR endpoints (default: https)

  Default value: `https`



## `iroha app sorafs gateway direct-mode enable`

Emit a configuration snippet enabling direct-mode overrides from a plan

**Usage:** `iroha app sorafs gateway direct-mode enable --plan <PATH>`

###### **Options:**

* `--plan <PATH>` — Path to the JSON output produced by `sorafs gateway direct-mode plan`



## `iroha app sorafs gateway direct-mode rollback`

Emit a configuration snippet restoring default gateway security settings

**Usage:** `iroha app sorafs gateway direct-mode rollback`



## `iroha app sorafs gateway merkle`

Merkle snapshot/proof tooling for denylist bundles

**Usage:** `iroha app sorafs gateway merkle <COMMAND>`

###### **Subcommands:**

* `snapshot` — Compute the Merkle root summary for a denylist bundle
* `proof` — Emit a membership proof for a single denylist entry



## `iroha app sorafs gateway merkle snapshot`

Compute the Merkle root summary for a denylist bundle

**Usage:** `iroha app sorafs gateway merkle snapshot [OPTIONS]`

###### **Options:**

* `--denylist <PATH>` — Path to the denylist JSON bundle

  Default value: `docs/source/sorafs_gateway_denylist_sample.json`
* `--json-out <PATH>` — Optional path to persist the JSON summary

  Default value: `artifacts/sorafs_gateway/denylist_merkle_snapshot.json`
* `--norito-out <PATH>` — Optional path to persist the Norito-encoded snapshot artefact



## `iroha app sorafs gateway merkle proof`

Emit a membership proof for a single denylist entry

**Usage:** `iroha app sorafs gateway merkle proof [OPTIONS]`

###### **Options:**

* `--denylist <PATH>` — Path to the denylist JSON bundle

  Default value: `docs/source/sorafs_gateway_denylist_sample.json`
* `--index <INDEX>` — Zero-based index of the entry to prove (see the snapshot listing)
* `--descriptor <KIND:VALUE>` — Descriptor of the entry to prove (`kind:value` from the snapshot output)
* `--json-out <PATH>` — Optional path to persist the JSON proof artefact

  Default value: `artifacts/sorafs_gateway/denylist_merkle_proof.json`
* `--norito-out <PATH>` — Optional path to persist the Norito-encoded proof artefact



## `iroha app sorafs incentives`

Offline helpers for relay payouts, disputes, and dashboards

**Usage:** `iroha app sorafs incentives <COMMAND>`

###### **Subcommands:**

* `compute` — Compute a relay reward instruction from metrics and bond state
* `open-dispute` — Open a dispute against an existing reward instruction
* `dashboard` — Summarise reward instructions into an earnings dashboard
* `service` — Manage the persistent treasury payout state and disputes



## `iroha app sorafs incentives compute`

Compute a relay reward instruction from metrics and bond state

**Usage:** `iroha app sorafs incentives compute [OPTIONS] --config <PATH> --metrics <PATH> --bond <PATH> --beneficiary <ACCOUNT_ID>`

###### **Options:**

* `--config <PATH>` — Path to the reward configuration JSON
* `--metrics <PATH>` — Norito-encoded relay metrics (`RelayEpochMetricsV1`)
* `--bond <PATH>` — Norito-encoded bond ledger entry (`RelayBondLedgerEntryV1`)
* `--beneficiary <ACCOUNT_ID>` — Account ID that will receive the payout
* `--norito-out <PATH>` — Optional path where the Norito-encoded reward instruction will be written
* `--pretty` — Emit pretty-printed JSON.

   Ignored when `--output-format json` is used.

  Default value: `false`



## `iroha app sorafs incentives open-dispute`

Open a dispute against an existing reward instruction

**Usage:** `iroha app sorafs incentives open-dispute [OPTIONS] --instruction <PATH> --treasury-account <ACCOUNT_ID> --submitted-by <ACCOUNT_ID> --requested-amount <NUMERIC> --reason <TEXT>`

###### **Options:**

* `--instruction <PATH>` — Norito-encoded reward instruction (`RelayRewardInstructionV1`)
* `--treasury-account <ACCOUNT_ID>` — Treasury account initiating the dispute
* `--submitted-by <ACCOUNT_ID>` — Account ID submitting the dispute
* `--requested-amount <NUMERIC>` — Requested adjustment amount (Numeric)
* `--reason <TEXT>` — Reason provided by the operator
* `--submitted-at <SECONDS>` — Optional UNIX timestamp when the dispute is filed
* `--norito-out <PATH>` — Optional path where the Norito-encoded dispute will be written
* `--pretty` — Emit pretty-printed JSON.

   Ignored when `--output-format json` is used.

  Default value: `false`



## `iroha app sorafs incentives dashboard`

Summarise reward instructions into an earnings dashboard

**Usage:** `iroha app sorafs incentives dashboard --instruction <PATH>...`

###### **Options:**

* `--instruction <PATH>` — Reward instruction payloads to include in the dashboard



## `iroha app sorafs incentives service`

Manage the persistent treasury payout state and disputes

**Usage:** `iroha app sorafs incentives service <COMMAND>`

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



## `iroha app sorafs incentives service init`

Initialise a new payout ledger state file

**Usage:** `iroha app sorafs incentives service init [OPTIONS] --state <PATH> --config <PATH> --treasury-account <ACCOUNT_ID>`

###### **Options:**

* `--state <PATH>` — Path where the incentives state JSON will be stored
* `--config <PATH>` — Reward configuration JSON consumed by the payout engine
* `--treasury-account <ACCOUNT_ID>` — Treasury account debited when materialising payouts
* `--force` — Overwrite an existing state file if it already exists

  Default value: `false`
* `--allow-missing-budget-approval` — Allow missing `budget_approval_id` in the reward configuration (for lab/staging replays)

  Default value: `false`



## `iroha app sorafs incentives service process`

Evaluate metrics, record the payout, and persist the updated state

**Usage:** `iroha app sorafs incentives service process [OPTIONS] --state <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--metrics <PATH>` — Norito-encoded relay metrics (`RelayEpochMetricsV1`)
* `--bond <PATH>` — Norito-encoded bond ledger entry (`RelayBondLedgerEntryV1`)
* `--beneficiary <ACCOUNT_ID>` — Beneficiary account that receives the payout
* `--instruction-out <PATH>` — Write the Norito-encoded reward instruction to this path
* `--transfer-out <PATH>` — Write the Norito-encoded transfer instruction to this path
* `--submit-transfer` — Submit the resulting transfer to Torii after recording the payout

  Default value: `false`
* `--pretty` — Emit pretty JSON instead of a compact payload.

   Ignored when `--output-format json` is used.

  Default value: `false`



## `iroha app sorafs incentives service record`

Record an externally prepared reward instruction into the state

**Usage:** `iroha app sorafs incentives service record [OPTIONS] --state <PATH> --instruction <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--instruction <PATH>` — Norito-encoded reward instruction to record
* `--transfer-out <PATH>` — Write the Norito-encoded transfer instruction to this path if non-zero
* `--submit-transfer` — Submit the transfer to Torii after recording the payout

  Default value: `false`
* `--pretty` — Emit pretty JSON instead of a compact payload.

   Ignored when `--output-format json` is used.

  Default value: `false`



## `iroha app sorafs incentives service dispute`

Manage payout disputes recorded in the state

**Usage:** `iroha app sorafs incentives service dispute <COMMAND>`

###### **Subcommands:**

* `file` — File a new dispute against a recorded payout
* `resolve` — Resolve a dispute with the supplied outcome
* `reject` — Reject a dispute without altering the ledger



## `iroha app sorafs incentives service dispute file`

File a new dispute against a recorded payout

**Usage:** `iroha app sorafs incentives service dispute file [OPTIONS] --state <PATH> --relay-id <HEX> --epoch <EPOCH> --submitted-by <ACCOUNT_ID> --requested-amount <NUMERIC> --reason <TEXT>`

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
* `--pretty` — Emit pretty JSON instead of a compact payload.

   Ignored when `--output-format json` is used.

  Default value: `false`



## `iroha app sorafs incentives service dispute resolve`

Resolve a dispute with the supplied outcome

**Usage:** `iroha app sorafs incentives service dispute resolve [OPTIONS] --state <PATH> --dispute-id <ID> --resolution <RESOLUTION> --notes <TEXT>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--dispute-id <ID>` — Dispute identifier to resolve
* `--resolution <RESOLUTION>` — Resolution kind (`no-change`, `credit`, or `debit`)

  Possible values: `no-change`, `credit`, `debit`

* `--amount <NUMERIC>` — Amount applied when resolving with `credit` or `debit`
* `--notes <TEXT>` — Resolution notes recorded in the dispute metadata
* `--resolved-at <SECONDS>` — Optional UNIX timestamp when the dispute was resolved (defaults to now)
* `--transfer-out <PATH>` — Write the Norito-encoded transfer instruction generated by the resolution (if any)
* `--pretty` — Emit pretty JSON instead of a compact payload.

   Ignored when `--output-format json` is used.

  Default value: `false`



## `iroha app sorafs incentives service dispute reject`

Reject a dispute without altering the ledger

**Usage:** `iroha app sorafs incentives service dispute reject [OPTIONS] --state <PATH> --dispute-id <ID> --notes <TEXT>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--dispute-id <ID>` — Dispute identifier to reject
* `--notes <TEXT>` — Rejection notes captured in the dispute metadata
* `--rejected-at <SECONDS>` — Optional UNIX timestamp when the dispute was rejected (defaults to now)
* `--pretty` — Emit pretty JSON instead of a compact payload.

   Ignored when `--output-format json` is used.

  Default value: `false`



## `iroha app sorafs incentives service dashboard`

Render an earnings dashboard sourced from the persisted ledger

**Usage:** `iroha app sorafs incentives service dashboard --state <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON



## `iroha app sorafs incentives service audit`

Audit bond/payout governance readiness for relay incentives

**Usage:** `iroha app sorafs incentives service audit [OPTIONS] --state <PATH> --config <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--config <PATH>` — Daemon configuration describing relay beneficiaries and bond sources
* `--scope <SCOPES>` — Audit scopes to evaluate (repeat to combine); defaults to bond checks

  Default value: `bond`

  Possible values: `bond`, `budget`, `all`

* `--pretty` — Emit pretty JSON instead of a compact payload.

   Ignored when `--output-format json` is used.

  Default value: `false`



## `iroha app sorafs incentives service shadow-run`

Run a shadow simulation across relay metrics and summarise fairness

**Usage:** `iroha app sorafs incentives service shadow-run [OPTIONS] --state <PATH> --config <PATH> --metrics-dir <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--config <PATH>` — Shadow simulation configuration mapping relays to beneficiaries and bonds
* `--metrics-dir <PATH>` — Directory containing Norito-encoded relay metrics snapshots (`relay-<id>-epoch-<n>.to`)
* `--report-out <PATH>` — Optional path to write the shadow simulation report JSON
* `--pretty` — Emit pretty JSON instead of a compact payload.

   Ignored when `--output-format json` is used.

  Default value: `false`
* `--allow-missing-budget-approval` — Allow payouts without `budget_approval_id` (for local testing only)

  Default value: `false`



## `iroha app sorafs incentives service reconcile`

Reconcile recorded payouts against XOR ledger exports

**Usage:** `iroha app sorafs incentives service reconcile [OPTIONS] --state <PATH> --ledger-export <PATH>`

###### **Options:**

* `--state <PATH>` — Path to the persisted incentives state JSON
* `--ledger-export <PATH>` — Norito-encoded XOR ledger export to reconcile against
* `--pretty` — Emit pretty JSON instead of a compact payload.

   Ignored when `--output-format json` is used.

  Default value: `false`



## `iroha app sorafs incentives service daemon`

Run the treasury daemon against a metrics spool

**Usage:** `iroha app sorafs incentives service daemon [OPTIONS] --state <PATH> --config <PATH> --metrics-dir <PATH>`

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
* `--pretty` — Emit JSON summaries instead of plain-text logs.

   Ignored when `--output-format json` is used.

  Default value: `false`
* `--allow-missing-budget-approval` — Allow payouts without `budget_approval_id` (for local testing only)

  Default value: `false`



## `iroha app sorafs handshake`

Observe or modify the Torii `SoraNet` handshake configuration

**Usage:** `iroha app sorafs handshake <COMMAND>`

###### **Subcommands:**

* `show` — Display the current `SoraNet` handshake summary as reported by Torii
* `update` — Update one or more `SoraNet` handshake parameters via `/v1/config`
* `token` — Admission token helpers (issuance, fingerprinting, revocation digests)



## `iroha app sorafs handshake show`

Display the current `SoraNet` handshake summary as reported by Torii

**Usage:** `iroha app sorafs handshake show`



## `iroha app sorafs handshake update`

Update one or more `SoraNet` handshake parameters via `/v1/config`

**Usage:** `iroha app sorafs handshake update [OPTIONS]`

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



## `iroha app sorafs handshake token`

Admission token helpers (issuance, fingerprinting, revocation digests)

**Usage:** `iroha app sorafs handshake token <COMMAND>`

###### **Subcommands:**

* `issue` — Issue an ML-DSA admission token bound to a relay and transcript hash
* `id` — Compute the canonical revocation identifier for an admission token
* `fingerprint` — Compute the issuer fingerprint from an ML-DSA public key



## `iroha app sorafs handshake token issue`

Issue an ML-DSA admission token bound to a relay and transcript hash

**Usage:** `iroha app sorafs handshake token issue [OPTIONS] --relay-id <HEX> --transcript-hash <HEX>`

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
* `--flags <FLAGS>` — Token flags (reserved; must be 0 for v1 tokens)
* `--output <PATH>` — Optional path to write the encoded token
* `--token-encoding <TOKEN_ENCODING>` — Encoding used when writing the token to --output (base64, hex, binary)

  Default value: `base64`

  Possible values: `base64`, `hex`, `binary`




## `iroha app sorafs handshake token id`

Compute the canonical revocation identifier for an admission token

**Usage:** `iroha app sorafs handshake token id [OPTIONS]`

###### **Options:**

* `--token <PATH>` — Path to the admission token frame (binary)
* `--token-hex <HEX>` — Hex-encoded admission token frame
* `--token-base64 <BASE64>` — Base64url-encoded admission token frame



## `iroha app sorafs handshake token fingerprint`

Compute the issuer fingerprint from an ML-DSA public key

**Usage:** `iroha app sorafs handshake token fingerprint [OPTIONS]`

###### **Options:**

* `--public-key <PATH>` — Path to the ML-DSA public key (raw bytes)
* `--public-key-hex <HEX>` — Hex-encoded ML-DSA public key



## `iroha app sorafs toolkit`

Local tooling for packaging manifests and payloads

**Usage:** `iroha app sorafs toolkit <COMMAND>`

###### **Subcommands:**

* `pack` — Package a payload into a CAR + manifest bundle using the canonical tooling



## `iroha app sorafs toolkit pack`

Package a payload into a CAR + manifest bundle using the canonical tooling

**Usage:** `iroha app sorafs toolkit pack [OPTIONS] <INPUT>`

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



## `iroha app sorafs guard-directory`

Guard directory helpers (fetch/verify snapshots)

**Usage:** `iroha app sorafs guard-directory <COMMAND>`

###### **Subcommands:**

* `fetch` — Fetch a guard directory snapshot over HTTPS, verify it, and emit a summary
* `verify` — Verify a guard directory snapshot stored on disk



## `iroha app sorafs guard-directory fetch`

Fetch a guard directory snapshot over HTTPS, verify it, and emit a summary

**Usage:** `iroha app sorafs guard-directory fetch [OPTIONS] --url <URL>`

###### **Options:**

* `--url <URL>` — URLs publishing the guard directory snapshot (first success wins)
* `--output <PATH>` — Path where the verified snapshot will be stored (optional)
* `--expected-directory-hash <HEX>` — Expected directory hash (hex). Command fails when the snapshot hash differs
* `--timeout-secs <SECS>` — HTTP timeout in seconds (defaults to 30s)

  Default value: `30`
* `--overwrite` — Allow overwriting an existing file at --output



## `iroha app sorafs guard-directory verify`

Verify a guard directory snapshot stored on disk

**Usage:** `iroha app sorafs guard-directory verify [OPTIONS] --path <PATH>`

###### **Options:**

* `--path <PATH>` — Path to the guard directory snapshot to verify
* `--expected-directory-hash <HEX>` — Expected directory hash (hex). Command fails when the snapshot hash differs



## `iroha app sorafs reserve`

Reserve + rent policy helpers

**Usage:** `iroha app sorafs reserve <COMMAND>`

###### **Subcommands:**

* `quote` — Quote reserve requirements and effective rent for a given tier/capacity
* `ledger` — Convert a reserve quote into rent/reserve transfer instructions



## `iroha app sorafs reserve quote`

Quote reserve requirements and effective rent for a given tier/capacity

**Usage:** `iroha app sorafs reserve quote [OPTIONS] --storage-class <STORAGE_CLASS> --tier <TIER> --gib <GIB>`

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



## `iroha app sorafs reserve ledger`

Convert a reserve quote into rent/reserve transfer instructions

**Usage:** `iroha app sorafs reserve ledger --quote <PATH> --provider-account <ACCOUNT_ID> --treasury-account <ACCOUNT_ID> --reserve-account <ACCOUNT_ID> --asset-definition <AID>`

###### **Options:**

* `--quote <PATH>` — Path to the reserve quote JSON (output of `sorafs reserve quote`)
* `--provider-account <ACCOUNT_ID>` — Provider account paying the rent and reserve top-ups
* `--treasury-account <ACCOUNT_ID>` — Treasury account receiving the rent payment
* `--reserve-account <ACCOUNT_ID>` — Reserve escrow account receiving the reserve top-up
* `--asset-definition <AID>` — Asset definition identifier used for transfers (canonical unprefixed Base58 address)



## `iroha app sorafs gar`

GAR policy evidence helpers

**Usage:** `iroha app sorafs gar <COMMAND>`

###### **Subcommands:**

* `receipt` — Render a GAR enforcement receipt artefact (JSON + optional Norito bytes)



## `iroha app sorafs gar receipt`

Render a GAR enforcement receipt artefact (JSON + optional Norito bytes)

**Usage:** `iroha app sorafs gar receipt [OPTIONS] --gar-name <LABEL> --canonical-host <HOST> --operator <ACCOUNT_ID> --reason <TEXT>`

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



## `iroha app sorafs repair`

Repair queue helpers (list, claim, close, escalate)

**Usage:** `iroha app sorafs repair <COMMAND>`

###### **Subcommands:**

* `list` — List repair tickets (optionally filtered by manifest/provider/status)
* `claim` — Claim a queued repair ticket as a repair worker
* `complete` — Mark a repair ticket as completed
* `fail` — Mark a repair ticket as failed
* `escalate` — Escalate a repair ticket into a slash proposal



## `iroha app sorafs repair list`

List repair tickets (optionally filtered by manifest/provider/status)

**Usage:** `iroha app sorafs repair list [OPTIONS]`

###### **Options:**

* `--manifest-digest <HEX>` — Optional manifest digest to scope the listing
* `--status <STATUS>` — Optional status filter (queued, verifying, in_progress, completed, failed, escalated)
* `--provider-id <HEX>` — Optional provider identifier filter (hex-encoded)



## `iroha app sorafs repair claim`

Claim a queued repair ticket as a repair worker

**Usage:** `iroha app sorafs repair claim [OPTIONS] --ticket-id <ID> --manifest-digest <HEX> --provider-id <HEX>`

###### **Options:**

* `--ticket-id <ID>` — Repair ticket identifier (e.g., `REP-401`)
* `--manifest-digest <HEX>` — Manifest digest bound to the ticket (hex-encoded)
* `--provider-id <HEX>` — Provider identifier owning the ticket (hex-encoded)
* `--claimed-at <RFC3339|@UNIX>` — Optional timestamp for the claim (RFC3339 or `@unix_seconds`)
* `--idempotency-key <KEY>` — Optional idempotency key (auto-generated when omitted)



## `iroha app sorafs repair complete`

Mark a repair ticket as completed

**Usage:** `iroha app sorafs repair complete [OPTIONS] --ticket-id <ID> --manifest-digest <HEX> --provider-id <HEX>`

###### **Options:**

* `--ticket-id <ID>` — Repair ticket identifier (e.g., `REP-401`)
* `--manifest-digest <HEX>` — Manifest digest bound to the ticket (hex-encoded)
* `--provider-id <HEX>` — Provider identifier owning the ticket (hex-encoded)
* `--completed-at <RFC3339|@UNIX>` — Optional timestamp for the completion (RFC3339 or `@unix_seconds`)
* `--resolution-notes <TEXT>` — Optional resolution notes
* `--idempotency-key <KEY>` — Optional idempotency key (auto-generated when omitted)



## `iroha app sorafs repair fail`

Mark a repair ticket as failed

**Usage:** `iroha app sorafs repair fail [OPTIONS] --ticket-id <ID> --manifest-digest <HEX> --provider-id <HEX> --reason <TEXT>`

###### **Options:**

* `--ticket-id <ID>` — Repair ticket identifier (e.g., `REP-401`)
* `--manifest-digest <HEX>` — Manifest digest bound to the ticket (hex-encoded)
* `--provider-id <HEX>` — Provider identifier owning the ticket (hex-encoded)
* `--failed-at <RFC3339|@UNIX>` — Optional timestamp for the failure (RFC3339 or `@unix_seconds`)
* `--reason <TEXT>` — Failure reason
* `--idempotency-key <KEY>` — Optional idempotency key (auto-generated when omitted)



## `iroha app sorafs repair escalate`

Escalate a repair ticket into a slash proposal

**Usage:** `iroha app sorafs repair escalate [OPTIONS] --ticket-id <ID> --manifest-digest <HEX> --provider-id <HEX> --penalty-nano <NANO> --rationale <TEXT>`

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



## `iroha app sorafs gc`

GC inspection helpers (no manual deletions)

**Usage:** `iroha app sorafs gc <COMMAND>`

###### **Subcommands:**

* `inspect` — Inspect retained manifests and retention deadlines
* `dry-run` — Report which manifests would be evicted by GC (dry-run only)



## `iroha app sorafs gc inspect`

Inspect retained manifests and retention deadlines

**Usage:** `iroha app sorafs gc inspect [OPTIONS]`

###### **Options:**

* `--data-dir <PATH>` — Root directory for SoraFS storage data (defaults to the node config default)
* `--now <RFC3339|@UNIX>` — Override the reference timestamp (RFC3339 or `@unix_seconds`)
* `--grace-secs <SECONDS>` — Override the retention grace window in seconds



## `iroha app sorafs gc dry-run`

Report which manifests would be evicted by GC (dry-run only)

**Usage:** `iroha app sorafs gc dry-run [OPTIONS]`

###### **Options:**

* `--data-dir <PATH>` — Root directory for SoraFS storage data (defaults to the node config default)
* `--now <RFC3339|@UNIX>` — Override the reference timestamp (RFC3339 or `@unix_seconds`)
* `--grace-secs <SECONDS>` — Override the retention grace window in seconds



## `iroha app sorafs fetch`

Orchestrate multi-provider chunk fetches via gateways

**Usage:** `iroha app sorafs fetch [OPTIONS] --gateway-provider <SPEC>`

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



## `iroha app soracles`

Soracles helpers (evidence bundling)

**Usage:** `iroha app soracles <COMMAND>`

###### **Subcommands:**

* `bundle` — Build an audit bundle containing oracle feed events and evidence files
* `catalog` — Show the oracle rejection/error catalog for SDK parity
* `evidence-gc` — Garbage-collect evidence bundles and prune unreferenced artifacts



## `iroha app soracles bundle`

Build an audit bundle containing oracle feed events and evidence files

**Usage:** `iroha app soracles bundle [OPTIONS] --events <PATH> --output <DIR>`

###### **Options:**

* `--events <PATH>` — Path to a JSON file containing `FeedEventRecord` values (array or single record)
* `--output <DIR>` — Directory where the bundle (manifest + hashed artefacts) will be written
* `--observations <DIR>` — Directory of observation JSON files to include (hashed and copied into the bundle)
* `--reports <DIR>` — Directory of report JSON files to include
* `--responses <DIR>` — Directory of connector response JSON files to include
* `--disputes <DIR>` — Directory of dispute evidence JSON files to include
* `--telemetry <PATH>` — Optional telemetry snapshot (JSON) to include in the bundle



## `iroha app soracles catalog`

Show the oracle rejection/error catalog for SDK parity

**Usage:** `iroha app soracles catalog [OPTIONS]`

###### **Options:**

* `--format <FORMAT>` — Output format (`json` for machine consumption, `markdown` for docs/runbooks).

   Ignored when `--output-format json` is used.

  Default value: `json`

  Possible values: `json`, `markdown`




## `iroha app soracles evidence-gc`

Garbage-collect evidence bundles and prune unreferenced artifacts

**Usage:** `iroha app soracles evidence-gc [OPTIONS]`

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



## `iroha app sns`

Sora Name Service helpers (registrar + policy tooling)

**Usage:** `iroha app sns <COMMAND>`

###### **Subcommands:**

* `register` — Register a SNS name via `/v1/sns/names`
* `renew` — Renew a SNS name via `/v1/sns/names/domain/{literal}/renew`
* `transfer` — Transfer ownership of a SNS name
* `update-controllers` — Replace controllers on a SNS name
* `freeze` — Freeze a SNS name
* `unfreeze` — Unfreeze a SNS name
* `registration` — Fetch a SNS name record
* `policy` — Fetch the policy for a suffix
* `governance` — Governance helper placeholders retained for dry-run validation only



## `iroha app sns register`

Register a SNS name via `/v1/sns/names`

**Usage:** `iroha app sns register [OPTIONS] --label <LABEL> --suffix-id <U16>`

###### **Options:**

* `--label <LABEL>` — Label (without suffix) to register. Automatically lower-cased & NFC-normalised
* `--suffix-id <U16>` — Numeric suffix identifier (see `SuffixPolicyV1::suffix_id`)
* `--owner <ACCOUNT-ID>` — Owner account identifier; defaults to the CLI config account
* `--controller <ACCOUNT-ID>` — Controller account identifiers (repeatable). Defaults to `[owner]`
* `--term-years <U8>` — Registration term in years

  Default value: `1`
* `--pricing-class <U8>` — Optional pricing class hint advertised by the steward
* `--payment-json <PATH>` — Optional path to a JSON file containing `PaymentProofV1`. When omitted the inline flags are used
* `--payment-asset-id <ASSET-ID>` — Payment asset identifier (e.g., `61CtjvNd9T3THAR65GsMVHr82Bjc`)
* `--payment-gross <U64>` — Gross payment amount (base + surcharges) in native units
* `--payment-net <U64>` — Net payment amount forwarded to the registry. Defaults to `payment-gross`
* `--payment-settlement <JSON>` — Settlement transaction reference (JSON literal)
* `--payment-payer <ACCOUNT-ID>` — Account that authorised the payment. Defaults to the CLI config account
* `--payment-signature <JSON>` — Steward/treasury signature attesting to the payment (JSON literal)
* `--metadata-json <PATH>` — Optional path to a JSON object that will populate `Metadata`
* `--governance-json <PATH>` — Optional path to a JSON document describing `GovernanceHookV1`



## `iroha app sns renew`

Renew a SNS name via `/v1/sns/names/domain/{literal}/renew`

**Usage:** `iroha app sns renew [OPTIONS] --selector <LABEL.SUFFIX>`

###### **Options:**

* `--selector <LABEL.SUFFIX>` — Legacy domain selector literal (e.g. `makoto.sora`)
* `--term-years <U8>` — Additional term to purchase (years)

  Default value: `1`
* `--payment-json <PATH>` — Optional path to a JSON file containing `PaymentProofV1`. When omitted the inline flags are used
* `--payment-asset-id <ASSET-ID>` — Payment asset identifier (e.g., `61CtjvNd9T3THAR65GsMVHr82Bjc`)
* `--payment-gross <U64>` — Gross payment amount (base + surcharges) in native units
* `--payment-net <U64>` — Net payment amount forwarded to the registry. Defaults to `payment-gross`
* `--payment-settlement <JSON>` — Settlement transaction reference (JSON literal)
* `--payment-payer <ACCOUNT-ID>` — Account that authorised the payment. Defaults to the CLI config account
* `--payment-signature <JSON>` — Steward/treasury signature attesting to the payment (JSON literal)



## `iroha app sns transfer`

Transfer ownership of a SNS name

**Usage:** `iroha app sns transfer --selector <LABEL.SUFFIX> --new-owner <ACCOUNT-ID> --governance-json <PATH>`

###### **Options:**

* `--selector <LABEL.SUFFIX>` — Legacy domain selector literal (e.g. `makoto.sora`)
* `--new-owner <ACCOUNT-ID>` — New owner account identifier
* `--governance-json <PATH>` — Path to `GovernanceHookV1` JSON proving transfer approval



## `iroha app sns update-controllers`

Replace controllers on a SNS name

**Usage:** `iroha app sns update-controllers [OPTIONS] --selector <LABEL.SUFFIX>`

###### **Options:**

* `--selector <LABEL.SUFFIX>` — Legacy domain selector literal (e.g. `makoto.sora`)
* `--controller <ACCOUNT-ID>` — Replacement controller account identifiers (repeatable). Defaults to `[config account]`



## `iroha app sns freeze`

Freeze a SNS name

**Usage:** `iroha app sns freeze --selector <LABEL.SUFFIX> --reason <TEXT> --until-ms <U64> --guardian-ticket <JSON>`

###### **Options:**

* `--selector <LABEL.SUFFIX>` — Legacy domain selector literal (e.g. `makoto.sora`)
* `--reason <TEXT>` — Reason recorded in the freeze log
* `--until-ms <U64>` — Timestamp (ms since epoch) when the freeze should auto-expire
* `--guardian-ticket <JSON>` — Guardian ticket signature (JSON literal)



## `iroha app sns unfreeze`

Unfreeze a SNS name

**Usage:** `iroha app sns unfreeze --selector <LABEL.SUFFIX> --governance-json <PATH>`

###### **Options:**

* `--selector <LABEL.SUFFIX>` — Legacy domain selector literal (e.g. `makoto.sora`)
* `--governance-json <PATH>` — Path to `GovernanceHookV1` JSON authorising the unfreeze



## `iroha app sns registration`

Fetch a SNS name record

**Usage:** `iroha app sns registration --selector <SELECTOR>`

###### **Options:**

* `--selector <SELECTOR>` — Legacy domain selector literal (`label.suffix`) in canonical I105 form



## `iroha app sns policy`

Fetch the policy for a suffix

**Usage:** `iroha app sns policy --suffix-id <U16>`

###### **Options:**

* `--suffix-id <U16>` — Numeric suffix identifier (`SuffixPolicyV1::suffix_id`)



## `iroha app sns governance`

Governance helper placeholders retained for dry-run validation only

**Usage:** `iroha app sns governance <COMMAND>`

###### **Subcommands:**

* `case` — Manage arbitration cases referenced by SN-6a



## `iroha app sns governance case`

Manage arbitration cases referenced by SN-6a

**Usage:** `iroha app sns governance case <COMMAND>`

###### **Subcommands:**

* `create` — Validate and submit a dispute case payload
* `export` — Export cases for transparency reporting



## `iroha app sns governance case create`

Validate and submit a dispute case payload

**Usage:** `iroha app sns governance case create [OPTIONS] --case-json <PATH>`

###### **Options:**

* `--case-json <PATH>` — Path to the arbitration case payload (JSON)
* `--schema <PATH>` — Optional path to a JSON schema. Defaults to the embedded SN-6a schema
* `--dry-run` — Validate the payload only; do not submit to Torii



## `iroha app sns governance case export`

Export cases for transparency reporting

**Usage:** `iroha app sns governance case export [OPTIONS]`

###### **Options:**

* `--since <ISO-8601>` — Filter to cases updated after the provided ISO-8601 timestamp
* `--status <STATUS>` — Optional status filter (open, triage, decision, remediation, closed, suspended)
* `--limit <U32>` — Maximum number of cases to return



## `iroha app alias`

Alias helpers (placeholder pipeline)

**Usage:** `iroha app alias <COMMAND>`

###### **Subcommands:**

* `voprf-evaluate` — Evaluate a blinded element using the alias VOPRF service (placeholder)
* `resolve` — Resolve an alias by its canonical name (placeholder)
* `resolve-index` — Resolve an alias by Merkle index (placeholder)
* `by-account` — List aliases bound to a canonical account id



## `iroha app alias voprf-evaluate`

Evaluate a blinded element using the alias VOPRF service (placeholder)

**Usage:** `iroha app alias voprf-evaluate --blinded-element-hex <HEX>`

###### **Options:**

* `--blinded-element-hex <HEX>` — Blinded element in hex encoding



## `iroha app alias resolve`

Resolve an alias by its canonical name (placeholder)

**Usage:** `iroha app alias resolve [OPTIONS] --alias <ALIAS>`

###### **Options:**

* `--alias <ALIAS>` — Alias name to resolve
* `--dry-run` — Print only validation result (skip future network call)

  Default value: `false`



## `iroha app alias resolve-index`

Resolve an alias by Merkle index (placeholder)

**Usage:** `iroha app alias resolve-index --index <INDEX>`

###### **Options:**

* `--index <INDEX>` — Alias Merkle index to resolve



## `iroha app alias by-account`

List aliases bound to a canonical account id

**Usage:** `iroha app alias by-account [OPTIONS] --account-id <ACCOUNT_ID>`

###### **Options:**

* `--account-id <ACCOUNT_ID>` — Canonical I105 account id
* `--dataspace <DATASPACE>` — Optional dataspace alias filter such as `centralbank`
* `--domain <DOMAIN>` — Optional exact domain filter such as `banka`



## `iroha app repo`

Repo settlement helpers

**Usage:** `iroha app repo <COMMAND>`

###### **Subcommands:**

* `initiate` — Initiate or roll a repo agreement between two counterparties
* `unwind` — Unwind an active repo agreement (reverse repo leg)
* `query` — Inspect repo agreements stored on-chain
* `margin` — Compute the next margin checkpoint for an agreement
* `margin-call` — Record a margin call for an active repo agreement



## `iroha app repo initiate`

Initiate or roll a repo agreement between two counterparties

**Usage:** `iroha app repo initiate [OPTIONS] --agreement-id <AGREEMENT_ID> --initiator <INITIATOR> --counterparty <COUNTERPARTY> --cash-asset <CASH_ASSET> --cash-quantity <CASH_QUANTITY> --collateral-asset <COLLATERAL_ASSET> --collateral-quantity <COLLATERAL_QUANTITY> --rate-bps <RATE_BPS> --maturity-timestamp-ms <MATURITY_TIMESTAMP_MS> --haircut-bps <HAIRCUT_BPS> --margin-frequency-secs <MARGIN_FREQUENCY_SECS>`

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



## `iroha app repo unwind`

Unwind an active repo agreement (reverse repo leg)

**Usage:** `iroha app repo unwind --agreement-id <AGREEMENT_ID> --initiator <INITIATOR> --counterparty <COUNTERPARTY> --cash-asset <CASH_ASSET> --cash-quantity <CASH_QUANTITY> --collateral-asset <COLLATERAL_ASSET> --collateral-quantity <COLLATERAL_QUANTITY> --settlement-timestamp-ms <SETTLEMENT_TIMESTAMP_MS>`

###### **Options:**

* `--agreement-id <AGREEMENT_ID>` — Stable identifier assigned to the repo agreement lifecycle
* `--initiator <INITIATOR>` — Initiating account performing the unwind
* `--counterparty <COUNTERPARTY>` — Counterparty receiving the unwind settlement
* `--cash-asset <CASH_ASSET>` — Cash asset definition identifier
* `--cash-quantity <CASH_QUANTITY>` — Cash quantity returned at unwind (integer or decimal)
* `--collateral-asset <COLLATERAL_ASSET>` — Collateral asset definition identifier
* `--collateral-quantity <COLLATERAL_QUANTITY>` — Collateral quantity released at unwind (integer or decimal)
* `--settlement-timestamp-ms <SETTLEMENT_TIMESTAMP_MS>` — Unix timestamp (milliseconds) when the unwind was agreed



## `iroha app repo query`

Inspect repo agreements stored on-chain

**Usage:** `iroha app repo query <COMMAND>`

###### **Subcommands:**

* `list` — List all repo agreements recorded on-chain
* `get` — Fetch a single repo agreement by identifier



## `iroha app repo query list`

List all repo agreements recorded on-chain

**Usage:** `iroha app repo query list`



## `iroha app repo query get`

Fetch a single repo agreement by identifier

**Usage:** `iroha app repo query get --id <ID>`

###### **Options:**

* `--id <ID>` — Stable identifier assigned to the repo agreement lifecycle



## `iroha app repo margin`

Compute the next margin checkpoint for an agreement

**Usage:** `iroha app repo margin [OPTIONS] --agreement-id <AGREEMENT_ID>`

###### **Options:**

* `--agreement-id <AGREEMENT_ID>` — Stable identifier assigned to the repo agreement lifecycle
* `--at-timestamp-ms <AT_TIMESTAMP_MS>` — Timestamp (ms) used when evaluating margin schedule (defaults to current time)



## `iroha app repo margin-call`

Record a margin call for an active repo agreement

**Usage:** `iroha app repo margin-call --agreement-id <AGREEMENT_ID>`

###### **Options:**

* `--agreement-id <AGREEMENT_ID>` — Stable identifier assigned to the repo agreement lifecycle



## `iroha app settlement`

Delivery-versus-payment and payment-versus-payment helpers

**Usage:** `iroha app settlement <COMMAND>`

###### **Subcommands:**

* `dvp` — Create a delivery-versus-payment instruction
* `pvp` — Create a payment-versus-payment instruction



## `iroha app settlement dvp`

Create a delivery-versus-payment instruction

**Usage:** `iroha app settlement dvp [OPTIONS] --settlement-id <SETTLEMENT_ID> --delivery-asset <DELIVERY_ASSET> --delivery-quantity <DELIVERY_QUANTITY> --delivery-from <DELIVERY_FROM> --delivery-to <DELIVERY_TO> --payment-asset <PAYMENT_ASSET> --payment-quantity <PAYMENT_QUANTITY> --payment-from <PAYMENT_FROM> --payment-to <PAYMENT_TO>`

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



## `iroha app settlement pvp`

Create a payment-versus-payment instruction

**Usage:** `iroha app settlement pvp [OPTIONS] --settlement-id <SETTLEMENT_ID> --primary-asset <PRIMARY_ASSET> --primary-quantity <PRIMARY_QUANTITY> --primary-from <PRIMARY_FROM> --primary-to <PRIMARY_TO> --counter-asset <COUNTER_ASSET> --counter-quantity <COUNTER_QUANTITY> --counter-from <COUNTER_FROM> --counter-to <COUNTER_TO>`

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



## `iroha tools`

Developer utilities and diagnostics

**Usage:** `iroha tools <COMMAND>`

###### **Subcommands:**

* `address` — Account address helpers (canonical I105 conversions)
* `crypto` — Cryptography helpers (SM2/SM3/SM4)
* `ivm` — IVM/ABI helpers (e.g., compute ABI hash)
* `markdown-help` — Output CLI documentation in Markdown format
* `version` — Show versions and git SHA of client and server



## `iroha tools address`

Account address helpers (canonical I105 conversions)

**Usage:** `iroha tools address <COMMAND>`

###### **Subcommands:**

* `convert` — Convert account addresses between supported textual encodings
* `audit` — Scan a list of addresses and emit conversion summaries
* `normalize` — Rewrite newline-separated addresses into canonical encodings



## `iroha tools address convert`

Convert account addresses between supported textual encodings

**Usage:** `iroha tools address convert [OPTIONS] <ADDRESS>`

###### **Arguments:**

* `<ADDRESS>` — Address literal to parse (canonical I105 or public key)

###### **Options:**

* `--expect-prefix <PREFIX>` — Require I105 inputs to match the provided network prefix
* `--network-prefix <PREFIX>` — Network prefix to use when emitting i105 output

  Default value: `753`
* `--format <FORMAT>` — Desired output format (defaults to I105)

  Default value: `i105`

  Possible values: `i105`, `canonical-hex`, `json`




## `iroha tools address audit`

Scan a list of addresses and emit conversion summaries

**Usage:** `iroha tools address audit [OPTIONS]`

###### **Options:**

* `--input <PATH>` — Path to a file containing newline-separated addresses (defaults to STDIN)
* `--expect-prefix <PREFIX>` — Require I105 inputs to match the provided network prefix
* `--network-prefix <PREFIX>` — Network prefix to use when emitting i105 output

  Default value: `753`
* `--allow-errors` — Succeed even if parse errors were encountered (allow auditing large dumps)
* `--format <FORMAT>` — Output format (`json` for structured reports, `csv` for spreadsheet ingestion)

  Default value: `json`

  Possible values: `json`, `csv`




## `iroha tools address normalize`

Rewrite newline-separated addresses into canonical encodings

**Usage:** `iroha tools address normalize [OPTIONS]`

###### **Options:**

* `--input <PATH>` — Path to a file containing newline-separated addresses (defaults to STDIN)
* `--output <PATH>` — Write the converted addresses to a file (defaults to STDOUT)
* `--expect-prefix <PREFIX>` — Require I105 inputs to match the provided network prefix
* `--network-prefix <PREFIX>` — Network prefix to use when emitting i105 output

  Default value: `753`
* `--format <FORMAT>` — Desired output format (defaults to I105)

  Default value: `i105`

  Possible values: `i105`, `canonical-hex`, `json`

* `--allow-errors` — Succeed even if parse errors were encountered (allow auditing large dumps)



## `iroha tools crypto`

Cryptography helpers (SM2/SM3/SM4)

**Usage:** `iroha tools crypto <COMMAND>`

###### **Subcommands:**

* `sm2` — SM2 key management helpers
* `sm3` — SM3 hashing helpers
* `sm4` — SM4 AEAD helpers (GCM/CCM modes)



## `iroha tools crypto sm2`

SM2 key management helpers

**Usage:** `iroha tools crypto sm2 <COMMAND>`

###### **Subcommands:**

* `keygen` — Generate a new SM2 key pair (distinguishing ID aware)
* `import` — Import an existing SM2 private key and derive metadata
* `export` — Export SM2 key material with config snippets



## `iroha tools crypto sm2 keygen`

Generate a new SM2 key pair (distinguishing ID aware)

**Usage:** `iroha tools crypto sm2 keygen [OPTIONS]`

###### **Options:**

* `--distid <DISTID>` — Distinguishing identifier embedded into SM2 signatures (defaults to `1234567812345678`)
* `--seed-hex <HEX>` — Optional seed (hex) for deterministic key generation. Helpful for tests/backups
* `--output <PATH>` — Write the generated JSON payload to a file instead of stdout
* `--quiet` — Suppress stdout printing of the JSON payload



## `iroha tools crypto sm2 import`

Import an existing SM2 private key and derive metadata

**Usage:** `iroha tools crypto sm2 import [OPTIONS]`

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



## `iroha tools crypto sm2 export`

Export SM2 key material with config snippets

**Usage:** `iroha tools crypto sm2 export [OPTIONS]`

###### **Options:**

* `--private-key-hex <HEX>` — Existing SM2 private key in hex (32 bytes)
* `--private-key-file <PATH>` — Path to a file containing a hex-encoded SM2 private key (32 bytes)
* `--private-key-pem <PEM>` — PKCS#8 PEM-encoded SM2 private key
* `--private-key-pem-file <PATH>` — Path to a PKCS#8 PEM SM2 private key
* `--distid <DISTID>` — Distinguishing identifier used by the signer (defaults to `1234567812345678`)
* `--snippet-output <PATH>` — Write the TOML snippet to a file
* `--emit-json` — Emit the JSON key material alongside the config snippet
* `--quiet` — Suppress stdout output



## `iroha tools crypto sm3`

SM3 hashing helpers

**Usage:** `iroha tools crypto sm3 <COMMAND>`

###### **Subcommands:**

* `hash` — Hash input data with SM3



## `iroha tools crypto sm3 hash`

Hash input data with SM3

**Usage:** `iroha tools crypto sm3 hash [OPTIONS]`

###### **Options:**

* `--data <STRING>` — UTF-8 string to hash (mutually exclusive with other inputs)
* `--data-hex <HEX>` — Raw bytes to hash provided as hex
* `--file <PATH>` — Path to a file whose contents will be hashed
* `--output <PATH>` — Write the digest JSON to a file
* `--quiet` — Suppress stdout printing of the digest JSON



## `iroha tools crypto sm4`

SM4 AEAD helpers (GCM/CCM modes)

**Usage:** `iroha tools crypto sm4 <COMMAND>`

###### **Subcommands:**

* `gcm-seal` — Encrypt data with SM4-GCM
* `gcm-open` — Decrypt data with SM4-GCM
* `ccm-seal` — Encrypt data with SM4-CCM
* `ccm-open` — Decrypt data with SM4-CCM



## `iroha tools crypto sm4 gcm-seal`

Encrypt data with SM4-GCM

**Usage:** `iroha tools crypto sm4 gcm-seal [OPTIONS] --key-hex <HEX32> --nonce-hex <HEX24>`

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



## `iroha tools crypto sm4 gcm-open`

Decrypt data with SM4-GCM

**Usage:** `iroha tools crypto sm4 gcm-open [OPTIONS] --key-hex <HEX32> --nonce-hex <HEX24>`

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



## `iroha tools crypto sm4 ccm-seal`

Encrypt data with SM4-CCM

**Usage:** `iroha tools crypto sm4 ccm-seal [OPTIONS] --key-hex <HEX32> --nonce-hex <HEX14-26>`

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



## `iroha tools crypto sm4 ccm-open`

Decrypt data with SM4-CCM

**Usage:** `iroha tools crypto sm4 ccm-open [OPTIONS] --key-hex <HEX32> --nonce-hex <HEX14-26>`

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



## `iroha tools ivm`

IVM/ABI helpers (e.g., compute ABI hash)

**Usage:** `iroha tools ivm <COMMAND>`

###### **Subcommands:**

* `abi-hash` — Print the current ABI hash for a given policy (default: v1)
* `syscalls` — Print the canonical syscall list (min or markdown table)
* `manifest-gen` — Generate a minimal manifest (`code_hash` + `abi_hash`) from a compiled .to file



## `iroha tools ivm abi-hash`

Print the current ABI hash for a given policy (default: v1)

**Usage:** `iroha tools ivm abi-hash [OPTIONS]`

###### **Options:**

* `--policy <POLICY>` — Policy: v1

  Default value: `v1`
* `--uppercase` — Uppercase hex output (default: lowercase)



## `iroha tools ivm syscalls`

Print the canonical syscall list (min or markdown table)

**Usage:** `iroha tools ivm syscalls [OPTIONS]`

###### **Options:**

* `--format <FORMAT>` — Output format: 'min' (one per line) or 'markdown'

  Default value: `min`



## `iroha tools ivm manifest-gen`

Generate a minimal manifest (`code_hash` + `abi_hash`) from a compiled .to file

**Usage:** `iroha tools ivm manifest-gen --file <PATH>`

###### **Options:**

* `--file <PATH>` — Path to compiled IVM bytecode (.to)



## `iroha tools markdown-help`

Output CLI documentation in Markdown format

**Usage:** `iroha tools markdown-help`



## `iroha tools version`

Show versions and git SHA of client and server

**Usage:** `iroha tools version`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

