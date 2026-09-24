# BPNG retail daily-limit admission

This is a source-level release finding, not a Taira allocation, owner approval,
installed policy, or live acceptance receipt. BPNG retail activation must remain
closed while the signed runtime advertises a `singleSignerDailyLimitKina` that
the admitted ledger cannot enforce for every applicable payment.

## Current native control

`SetAssetTransferControl` installs a DAY cap for one `(AccountId,
AssetDefinitionId)` pair. The executing asset path adds each outbound transfer
to that account's UTC calendar-day usage and rejects an over-cap transfer.
Atomic batch transfers aggregate repeated sources before this check. The
native record is held in reserved account metadata, and
`POST /v1/controls/asset-transfer/query` can return its limit, usage, and a
block checkpoint. Only the asset-definition owner or a delegate holding
`CanSetAssetTransferDailyLimit` for the exact asset, active account-alias
domain, and dataspace can set the cap. Existing tests cover exact-cap success,
over-cap rejection, unchanged usage after rejection, and batch aggregation.

This control is **opt-in per account**. An account without a record has no DAY
limit, and a direct Torii transaction is not required to pass through BPNG
Core. A signed application environment value or Core counter therefore cannot
establish a universal retail limit. The native limit also counts an account,
not the application `actor_id` used by the previous Core counter. The chain
has no authenticated mapping from every possible signing key and rekey lineage
to that application actor. Native transfer-control exceptions for protocol
movements must be assessed against the approved payment policy separately.

`AssetIssuerUsagePolicyV1` offers one useful on-chain admission primitive:
`require_subject_binding: true` rejects ordinary transfers involving an
unbound source or destination. For a newly registered retail account, the
asset owner could set the account DAY cap first and add its issuer subject
binding second. That ordering prevents an unbound account from receiving or
sending this asset through the ordinary transfer path during registration.
It does not make the limit mandatory for every bound account: the asset owner
can add a binding without a cap or remove the issuer policy, and a limit setter
can clear an installed cap. The issuer policy also has no signer-wide usage
field. The source now restricts issuer-policy metadata set/removal to the asset
definition owner in both executor admission and native execution, while leaving
ordinary metadata delegation intact. This closes a delegated-policy-mutation
path, but its deployment is only a possible component of a qualified protocol,
not an activation receipt by itself.

## First-release consensus design boundary

The needed control is an asset-and-physical-dataspace policy with a positive
whole-Kina DAY cap, an issuer-authorized opaque retail identity for every
participating retail account, and a closed list of institutional source
identities and typed protocol movement exceptions. The exact cap, identity
issuer, and exception roster must come from the owner-signed BPNG policy; no
fixture value or application configuration can supply them. A new account must
not receive or debit Digital Kina until its binding is committed under that
policy. The ledger must reject removal or weakening of an active policy while
the asset has a nonzero governed balance, except through a separately specified
owner-governed transition that preserves usage and finality. An ordinary asset
metadata permission must never be able to modify this control.

Debit usage belongs in dedicated consensus state keyed by `(asset definition,
physical dataspace, retail identity, UTC day)`, rather than in account metadata
or a portal/Kasumi counter. Every admitted numeric debit, including direct Torii,
contract and batch legs, must update that state in the same state transaction as
the balance change. Multiple accounts bound to one identity spend from the same
bucket. A controller rekey or alias reassignment must either carry the exact
identity binding atomically or be rejected before moving a governed balance;
historical usage never resets. Multisig and delegated transfers debit the
source account's retail identity, not the submitting authority's account.
Unbound sources and alternate balance scopes fail closed. A typed protocol
movement may bypass the cap only if the installed owner-signed policy names
that exact source class and purpose; an arbitrary transaction cannot self-label
an exception. Mint, burn, transparent/private bridge, reserve and KAGEMUSHA
movements each need an explicit owner decision before their treatment is coded.

Consensus can prove that two accounts share a committed opaque identity. It
cannot infer that two independently issued identities belong to one human.
Retail enrollment must therefore provide an owner-authorized uniqueness
attestation and rekey lineage for the identity commitment. If an offline
KAGEMUSHA payment itself is intended to consume this daily ceiling, an
on-chain debit-only counter cannot enforce that payment at spend time; the
owner must specify the offline accounting and reconciliation rule. These are
economic and identity-authority inputs, not values a node or application can
choose from available source code.

This is a coordinated consensus change, not a new flag on the existing account
cap. The current source has these distinct mutation surfaces:

| Source path | Required first-release treatment |
| --- | --- |
| `asset.rs::PreparedNumericTransferPlan::prepare/apply` and `PreparedNumericAssetMovementBatch` | Resolve each source to the owner-attested identity, aggregate all same-identity legs before applying a batch, and persist one conditional DAY usage update with the balance transcript. Direct Torii and IVM `Transfer` share this native path. |
| `asset.rs::NumericAssetMovementAuthorization` | Replace implicit cap bypass with a closed typed-purpose decision against the owner-signed institutional exception list. A direct transaction must never request an exception by metadata. |
| `asset.rs` burn, SCCP release, privacy bridge and KAGEMUSHA top-up/redemption | Specify whether each debit consumes a retail bucket or is an exact institutional exception; reject any uncovered path for a governed asset. |
| `multisig.rs::rekey_account_id` and account recovery | Preserve the same retail identity and its prior usage when the controller changes, or reject before moving the account and balances. Alias reassignment alone never establishes identity continuity. |
| Asset-definition policy mutation and state snapshot/query | Require the owner-authorized policy revision and preserve dedicated usage across policy changes, restart and signed query. Generic account-control metadata must not become a second usage ledger. |

The current `AssetIssuerUsagePolicyV1` cannot be interpreted as that identity
contract: its binding has only allowed domains and dataspaces, and
`AccountId` is derived from a single-key or multisig controller. The policy
does not carry an opaque person commitment, signed uniqueness attestation,
cap, exception roster or usage. Adding a cap field there alone would still
permit alternate-account and rekey resets. A first-release implementation
must introduce the complete policy/permission instruction, consensus usage
state and movement/rekey transitions together, with Norito roundtrip and
four-peer restart tests. There is no safe partially enabled decoder or
account-metadata fallback for the missing pieces.

## Registration and activation boundary

The current MiBank registration plan commits `Register<Account>` before it
binds the account's `mibank.bpng` alias. A scoped daily-limit delegate needs a
strictly active alias, so it cannot add `SetAssetTransferControl` to that first
registration transaction with its existing permission. The asset-definition
owner could atomically register and cap an account, but that is a different
authority and still would not constrain direct Torii transfers from other
uncapped accounts that acquire Digital Kina.

A release gate may advertise a positive retail daily ceiling only after an
owner-approved, signed on-chain policy supplies all of these properties:

1. The policy binds the exact Digital Kina asset definition, BPNG physical
   dataspace, retail identity scope, approved whole-Kina cap, and exact
   institutional exceptions. No matching retail debit may use an uncapped
   account, alternate balance route, other account controlled by the same
   applicable signer, or uncapped native/contract/batch path.
2. The policy states how single-key, multisig, alias rotation/rekey, account
   replacement, and approved KAGEMUSHA/protocol movements contribute to the
   same daily total. It must define the signer or retail identity from
   consensus-verifiable state, not a mutable application claim.
3. A current qualified Iroha release enforces that policy in its shared
   movement path. Positive and negative tests cover direct client-signed
   Torii transactions, new uncapped accounts, same-signer alternate accounts,
   batch and contract calls, UTC day rollover, cap change, and every permitted
   exception. There is no decoder, shim, or old counter fallback.
4. The BPNG signed runtime and provisioning receipt identify the exact policy
   revision and activation transaction. A signed finalized read at an exact
   block proves the installed ledger policy, cap, scope and exceptions equal
   the signed claim before `/readyz` or retail money routes open. Live
   direct-Torii rejection and exact-cap success are captured as original
   finality evidence.

The current Iroha asset-definition metadata policy controls subject domain
and dataspace bindings, not a mandatory signer-wide daily total. Extending it
only with a per-account default DAY cap would still fail item 1 if one signer
can control another account, and would not establish item 2's application
actor mapping. It would also need a complete review of institutional
exceptions and protocol movements. No safe narrow source change to the
current account control alone can satisfy this gate.
