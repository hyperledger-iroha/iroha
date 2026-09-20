# Public reset beacon boundary

The signed inventory requires a `beacon_bootstrap` plan. The unsigned draft must
omit this generated field. `prepare-public-inputs` validates and retains exactly
five public artifacts, including `genesis.json` and its manifest hash; incomplete
older bundles are rejected and must be prepared into a fresh output directory.
`prepare-beacon-inputs --inventory-draft PATH --public-inputs DIR --output PATH`
derives the nonce-bound fresh request and ordered seat paths from native validated
genesis. The maintained caller renders each final unit with the authenticated
renderer and supplies `--beacon-inputs` and four `--beacon-validator-unit` paths to
assembly/authorization. Assembly independently regenerates and matches the public
request and seat map before authorizing the final unit bytes.
The final units select `config/beacon.toml` and the fixed seat credential under
`/var/lib/taira/.public-reset-control-v1/beacon/<authorization_nonce>/ceremony/`.
The original seven artifacts, initial configuration and authorization stay immutable.

After initial Start, the cohost controller owns one native `beacon-bootstrap
provision` process outside the host action lock. It persists genesis-anchored,
four-peer finality evidence before delivering any height to the process. The
three existing onboarding, faucet and canary operations supply the actual DKG
progress. No empty block or synthetic observation is permitted. Losing the
process before its final bundle is a retained failure requiring explicit fresh
preparation; replaying those operations is forbidden.

The same-release daemon validates the bundle and signs the exact lifecycle
certificate using disposable consumed config descriptors. The controller retains
one exact fee-quoted installation transaction before its sole submission. Recovery
only verifies that envelope and its successful inclusion against authenticated
native execution. Provider activation follows that proof: only the three provider
binding fields derive `beacon.toml`, and only the separately signed final unit may
select it. Convergence and `/readyz` qualification run after all four activations.

Read-only recovery may prove an activation applied or identify its exact unfinished
host publication. The latter retains Submitted and requires a subsequent normal
apply with the original, still-valid authorization. This continuation cannot run
DKG or resubmit a ledger transaction. Rollback accepts the initial unit or its exact
durably bound beacon successor and restores the original admitted unit.
