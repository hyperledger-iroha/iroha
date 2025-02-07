# Multi-Signature Usage Guide

This guide explains how to create and operate a multi-signature account shared by multiple users.

## Registering a Multi-Signature Account

__Prerequisites:__

- All __signatories__ must be registered in advance.
- The registrant must have sufficient permissions to create an account.

__Example usage:__

```bash
iroha multisig register \
--account \
ed0120987EE8092B2CE4622B4F66D6FE87F5D61575F0D0DFCB2D6B2E8905FE68F685B6@domain \
--signatories \
ed01203EB45C199FD3998A18FCA1E567F5F228C714BFF5203FEFF00FF06230836BAD22@domain \
ed01206D75010256E96805161387608125326DD0068F29B4D4FC6755C98E5DA5413EC5@domain \
ed01201F8E7213A3064DF569776E2ED861D21E574BED93EE5BC41B5540593E70182F8B@domain \
--weights 1 2 3 \
--quorum 3 \
--transaction-ttl "1y 6M 2w 3d 12h 30m 30s"
```

__Explanation:__

To simplify explanations, accounts are identified by the last four digits of their multihash.

- `85B6` represents a multi-signature __account__.

  ⚠️ __Warning:__ Any private key associated with the account can control it as a personal account. This security issue will be addressed in [#5022](https://github.com/hyperledger-iroha/iroha/issues/5022).

- The multi-signature account consists of three __signatories__: `AD22`, `3EC5`, and `2F8B`.
- Each signatory has an assigned __weight__:
  - `AD22`: __1__
  - `3EC5`: __2__
  - `2F8B`: __3__
- A transaction is executed once the total weight of approving signatories meets the __quorum__:
  - For example, `AD22` (weight __1__) and `3EC5` (weight __2__) together meet the quorum (__3__).
  - Alternatively, `2F8B` (weight __3__) alone meets the quorum.
- If the __transaction TTL__ expires before reaching the quorum, the proposal is discarded.

## Proposing a Multi-Signature Transaction

__Prerequisites:__

- The multi-signature account must already be registered.
- The proposer must be one of the signatories.

__Example usage:__

```bash
echo '"congratulations"' | iroha -o account meta set \
--id ed0120987EE8092B2CE4622B4F66D6FE87F5D61575F0D0DFCB2D6B2E8905FE68F685B6@domain \
--key success_marker \
| iroha multisig propose \
--account ed0120987EE8092B2CE4622B4F66D6FE87F5D61575F0D0DFCB2D6B2E8905FE68F685B6@domain
```

__Explanation:__

- Proposes setting the string value "congratulations" for the key "success_marker" in the metadata of the multi-signature __account__.
- The proposer automatically becomes the first __approver__.

## Listing Multi-Signature Transactions

__Assumptions:__

- `AD22` (weight __1__) proposed the transaction.
- `3EC5` (weight __2__) is your account, listing the transactions involved.

__Usage:__

```bash
iroha multisig list all
```

__Example output:__

```json
{
  "FB8AEBB405236A9B4CCD26BBA4988D0B8E03957FDC52DD2A1F9F0A6953079989": {
    "instructions": [
      {
        "SetKeyValue": {
          "Account": {
            "object": "ed0120987EE8092B2CE4622B4F66D6FE87F5D61575F0D0DFCB2D6B2E8905FE68F685B6@domain",
            "key": "success_marker",
            "value": "congratulations"
          }
        }
      }
    ],
    "proposed_at": "2025-02-06T19:59:58Z",
    "expires_in": "1year 6months 17days 12h 26m 39s",
    "approval_path": [
      "2 -> [1/3] ed0120987EE8092B2CE4622B4F66D6FE87F5D61575F0D0DFCB2D6B2E8905FE68F685B6@domain"
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
iroha multisig approve \
--account ed0120987EE8092B2CE4622B4F66D6FE87F5D61575F0D0DFCB2D6B2E8905FE68F685B6@domain \
--instructions-hash FB8AEBB405236A9B4CCD26BBA4988D0B8E03957FDC52DD2A1F9F0A6953079989
```

__Explanation:__

- Approves a proposal linked to the given __instructions hash__ for the multi-signature __account__.
- Approval may lead to either execution or expiration of the proposal.
- If the approval meets the quorum but the multi-signature account lacks the necessary permissions to execute it, the final approval is discarded. Signatories who have not yet approved it can retry after the multi-signature account has acquired the required permissions.
