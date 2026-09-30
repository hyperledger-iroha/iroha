# Regulated account-recovery application API

Torii exposes a detached-signature application API for alias-bound account
recovery. It is deliberately narrower than the generic recovery instructions:

- exactly three distinct, registered, single-key Ed25519 guardian accounts;
- guardian weight `1`, quorum `2`, and timelock `259200000` ms (72 hours);
- a replacement `AccountController::Multisig` whose members are weight-one
  Ed25519 keys and whose threshold is `1` for one member or `2..=N` otherwise;
- guardian-signed approval, independent cancellation votes, and finalization; and
- terminal evidence that every outstanding native multisig proposal was
  canceled or had already expired before the account controller changed.

Corporate authorization, identity/KYB reverification, notifications, and the
two independent Inori operator approvals remain application-level gates. A DPN
service must complete those gates before asking the corresponding guardian
accounts to sign the native approval transactions. An off-chain approval is
never a substitute for an Ed25519 transaction signature.

## Routes

All routes require the configured Torii application API token. Request and
response schemas are published in Torii's OpenAPI document.

| Route | Purpose |
|---|---|
| `POST /v1/accounts/recovery/policy/set` | Prepare or submit the exact regulated policy for an active single-key account. |
| `POST /v1/accounts/recovery/propose` | Prepare or submit a controller-replacement proposal signed by the active single-key account or a configured guardian. |
| `POST /v1/accounts/recovery/approve` | Prepare or submit one configured guardian's approval. |
| `POST /v1/accounts/recovery/finalize` | Prepare or submit guardian-triggered finalization after native quorum and the full ledger timelock. |
| `POST /v1/accounts/recovery/cancel` | Prepare or submit an exact-request guardian cancellation vote, or immediate cancellation by the active single-key account. |
| `POST /v1/accounts/recovery/status` | Resolve the stable alias and return policy, lifecycle, and proposal-invalidation evidence. |

A native multisig company account cannot directly sign
`/v1/accounts/recovery/policy/set`. Build the same
`SetAccountRecoveryPolicy` instruction and authorize it through
`POST /v1/multisig/propose` and the current company threshold. This prevents
policy installation from bypassing the controller that it protects.

## Detached signing

Mutation routes use the same two-call protocol as Torii's native multisig
participation routes.

PROPOSE, APPROVE, CANCEL, and FINALIZE require `request_generation`, a nonzero
integer retained on the native request. PROPOSE names the next generation
(one for a new alias, otherwise the retained terminal generation plus one).
Every follow-up names the exact pending generation. Clearing or replacing the
policy does not reset this sequence, and generation exhaustion rejects a new
proposal. This binds every detached signature to one recovery request even
when a later request uses the same alias and replacement controller.

1. Send the complete intent with `signer_account_id`, `fee_payment`, and no
   `public_key_hex` or `signature_b64`.
2. Retain the returned `creation_time_ms`, resolved active account, fee payment,
   and the original request body. Base64-decode `signing_message_b64` and sign
   those bytes with the personal Ed25519 key in the user's wallet.
3. Submit the retained body unchanged with the returned `creation_time_ms`, the
   canonical lowercase 32-byte `public_key_hex`, and the padded-base64 detached
   signature.
4. Wait for the returned transaction hash to reach finality. For finalization,
   also query `/v1/accounts/recovery/status` and require the stable alias to
   resolve to the proposed controller with complete invalidation evidence.
   For cancellation, require the exact generation to be `Cancelled` and the
   original controller to remain active. Guardian cancellation requires two
   distinct cancellation votes and does not wait for its cooling period. A
   first applied vote leaves the native request `Pending`; its signer must
   appear in `cancellation_approvals` before the application records that vote.

Cancellation votes are separate from replacement approvals. A guardian's
cancellation vote irrevocably removes their replacement approval for that
generation; that guardian cannot later approve replacement, including through
an older signed approval. The other two guardians can still authorize
replacement before a cancellation quorum is reached. One cancellation vote
therefore cannot veto the configured two-of-three policy. Neither recovery
policy replacement nor clearing is permitted while a request is pending.
After a terminal outcome, the policy can change or be cleared. The status
route retains the exact terminal request and its votes independently of that
current policy, while still validating the alias, controller lineage and
finalization evidence. Clients must use their captured original policy to
interpret historical guardian votes; a current policy never authorizes a
new mutation of an old terminal request.
Terminal history also survives a later controller change proven by the exact
alias's native account-id rekey suffix. Finalized requests require both the
original and recovered controllers to resolve through that suffix to the
current controller; canceled requests require the original controller's
continuity. Ordinary alias reassignment never authenticates this history.
Historical readback does not authorize completing an application operation
whose captured controller differs from the current controller.

The signing key never belongs in a request and Torii never stores it. A service
should persist only public intent, detached signatures, transaction hashes, and
the resulting audit evidence. Rebuilding the transaction or fee intent between
prepare and submit changes the signing message and is rejected.

Preparation example for one guardian approval:

```json
{
  "account_alias": "dpncompany…@universal",
  "request_generation": 1,
  "signer_account_id": "<guardian-account-id>",
  "fee_payment": {
    "payer": "authority",
    "value": {
      "charge_limits": [],
      "gas_limit": null
    }
  }
}
```

The exact `FeePaymentIntent` JSON representation is defined by the running
Iroha build; clients should serialize the data-model type or reuse the value
returned during preparation.

## Finalization and proposal invalidation

`FinalizeAccountRecovery` performs these changes in one state transaction:

1. revalidate the stable alias, active controller lineage, guardian quorum, and
   72-hour ledger timelock;
2. enumerate every still-actionable native multisig proposal under the active
   controller;
3. write `CANCELED` terminal state, or `EXPIRED` when its native expiry has
   already passed, and prune the proposal tree;
4. retain the exact proposal hashes on the terminal recovery request; and
5. replace the controller while preserving the stable alias and retaining
   terminal proposal evidence under the original controller identity.

Any error rolls back the whole finalization. A live proposal is never migrated
to the recovered controller.

The status response sets `invalidation_evidence_complete` only for a finalized
request whose retained hashes all satisfy the following checks:

- no active proposal exists under either the pre-recovery or current
  controller id;
- a terminal record exists under the pre-recovery controller id;
- the record is bound to the exact account and proposal hash;
- `terminal_at_ms` is a positive ledger timestamp; and
- its status is `CANCELED` or `EXPIRED`, never `FINALIZED`.

An empty invalidation list is complete when no proposal was outstanding. A DPN
reconciler must fail closed when the completion flag is false, the alias or
controller does not match its frozen intent, or any expected proposal hash is
missing.

Deliberate signer/threshold policy changes use the related explicit flow in
[`multisig_policy_change.md`](./multisig_policy_change.md); ordinary controller
rekey intentionally retains its existing proposal-preservation semantics.

## Operational constraints

- Derive and persist a permanent company alias; never treat a concrete
  controller id as permanent identity.
- Configure the council guardian accounts before activating recovery and keep
  them registered and independently controlled.
- Use ledger time, not an application clock, for the cooling period.
- Notify affected company and compliance subjects outside Torii before and
  throughout the cooling period.
- Retain the DPN corporate-authorization/KYB evidence alongside the native
  request, guardian signer ids, finalization transaction, resolved controller,
  and status response.
