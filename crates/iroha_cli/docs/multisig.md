# Multi-Signature Usage Guide

This guide explains how to create and operate a multi-signature account shared by multiple users.

## Registering a Multi-Signature Account

__Prerequisites:__

- Any account may submit the multisig registration transaction; the runtime materializes the
  controller and related roles under the multisig home domain automatically.
- Signatory accounts do not need to exist in advance. Missing signatories are materialized during
  registration and tagged with metadata `iroha:created_via = "multisig"`.
- The CLI expects canonical Katakana i105 account literals for `--account` and `--signatories`. The
  multisig home domain is taken from the configured default domain.

__Example usage:__

```bash
iroha ledger multisig register \
--account <canonical-i105-controller> \
--signatories \
<canonical-i105-signatory-1> \
<canonical-i105-signatory-2> \
<canonical-i105-signatory-3> \
--weights 1 2 3 \
--quorum 3 \
--transaction-ttl "1y 6M 2w 3d 12h 30m 30s"
```

__Explanation:__

To simplify explanations, the multisig controller is labeled `MSA` and the signatories are labeled
`S1`, `S2`, and `S3`.

- `MSA` represents the multi-signature __account__ you provide with `--account` (or let the CLI
  generate in the signatory domain). The generated controller key is random and its private half is
  discarded because direct multisig signing is forbidden (`torii_multisig_direct_sign_reject_total`).
  Omit `--account` to generate a fresh controller or supply your own random id.

- The multi-signature account consists of three __signatories__: `S1`, `S2`, and `S3`.
- Each signatory has an assigned __weight__:
  - `S1`: __1__
  - `S2`: __2__
  - `S3`: __3__
- A transaction is executed once the total weight of approving signatories meets the __quorum__:
  - For example, `S1` (weight __1__) and `S2` (weight __2__) together meet the quorum (__3__).
  - Alternatively, `S3` (weight __3__) alone meets the quorum.
- If the __transaction TTL__ expires before reaching the quorum, the proposal is discarded.
  This TTL also acts as the policy cap for future proposals and nested relayers; overrides cannot exceed the registered value.

After submitting the registration, query for accounts whose metadata contains the key
`multisig/spec` to discover the multisig account ID:

```bash
iroha ledger query account list \
  filter '{"Atom":{"Metadata":{"Atom":{"Contains":{"key":"multisig/spec"}}}}}'
```

The resulting account ID, along with the stored specification, uniquely identifies the multisig
authority and can be reused for future proposals and approvals.

## Proposing a Multi-Signature Transaction

__Prerequisites:__

- The multi-signature account must already be registered.
- The proposer must be one of the signatories.
- The proposal TTL is capped by the multisig policy (`transaction_ttl_ms` set at registration). The CLI will preview the effective expiry and will refuse overrides above the policy cap before sending the transaction.

__Example usage:__

```bash
echo '"congratulations"' | iroha -o ledger account meta set \
--id <canonical-i105-multisig> \
--key success_marker \
| iroha ledger multisig propose \
--account <canonical-i105-multisig>
```

__Explanation:__

- Proposes setting the string value "congratulations" for the key "success_marker" in the metadata of the multi-signature __account__.
- The proposer automatically becomes the first __approver__.

## Listing Multi-Signature Transactions

__Assumptions:__

- `S1` (weight __1__) proposed the transaction.
- `S2` (weight __2__) is your account, listing the transactions involved.

__Usage:__

```bash
iroha ledger multisig list all
```

__Example output:__

```json
{
  "FB8AEBB405236A9B4CCD26BBA4988D0B8E03957FDC52DD2A1F9F0A6953079989": {
    "instructions": [
      {
        "SetKeyValue": {
          "Account": {
            "object": "<canonical-i105-multisig>",
            "key": "success_marker",
            "value": "congratulations"
          }
        }
      }
    ],
    "proposed_at": "2025-02-06T19:59:58Z",
    "expires_in": "1year 6months 17days 12h 26m 39s",
    "approval_path": [
      "2 -> [1/3] <canonical-i105-multisig>"
    ]
  }
}
```

__Explanation:__

- The key `FB8A..9989` is the __instructions hash__, identifying the proposal.
- `instructions` contains the proposed changes that will be executed once the quorum is reached.
- `approval_path` represents the approval chain from your account to the root multi-signature account for this proposal.

  The notation `2 -> [1/3]` means:
  You are adding a weight of 2 to an existing 1 (by the proposer), out of a required 3 (quorum).

## Approving a Multi-Signature Transaction

__Prerequisites:__

- The proposal must have been submitted.
- The approver must be a signatory of the multi-signature account.

__Example usage:__

```bash
iroha ledger multisig approve \
--account <canonical-i105-multisig> \
--instructions-hash FB8AEBB405236A9B4CCD26BBA4988D0B8E03957FDC52DD2A1F9F0A6953079989
```

__Explanation:__

- Approves a proposal linked to the given __instructions hash__ for the multi-signature __account__.
- Approval may lead to either execution or expiration of the proposal.
- If the approval meets the quorum but the multi-signature account lacks the necessary permissions to execute it, the final approval is discarded. Signatories who have not yet approved it can retry after the multi-signature account has acquired the required permissions.
