# Repo Settlement Runbook

This runbook is the operator sequence for Iroha's immutable, fixed-maturity
repo protocol. See [`repo_ops.md`](./repo_ops.md) for the security model and
evidence requirements.

## 1. Prepare one canonical proposal

Construct one `RepoIsi` with the final agreement identifier, parties, optional
custodian, cash and collateral terms, rate, maturity, haircut, and margin
cadence. Do not create permissions from a draft that may still change.

Verify:

- initiator, counterparty, and custodian roles;
- exact cash balance `AssetId` owned by the counterparty;
- exact collateral custody `AssetId` owned by the counterparty or custodian;
- both asset balance scopes, especially for dataspace-restricted definitions;
- maturity expressed in Unix milliseconds and later than expected admission;
- positive quantities and valid asset precision; and
- haircut at or below 10,000 basis points.

Archive the canonical Norito proposal bytes and its decoded term sheet.

## 2. Collect on-chain consent

Derive from that exact proposal:

- `settlement_id = RepoIsi::settlement_id()`;
- cash `intent_hash = RepoIsi::initiation_intent_hash()`; and
- collateral `intent_hash = RepoIsi::maturity_intent_hash()`.

The counterparty signs a Grant of:

```text
CanExecuteSettlement {
    debited_asset: <exact counterparty cash AssetId>,
    settlement_id: <agreement id>,
    intent_hash: <cash initiation hash>,
}
```

The collateral holder signs a separate Grant:

```text
CanExecuteSettlement {
    debited_asset: <exact collateral custody AssetId>,
    settlement_id: <agreement id>,
    intent_hash: <maturity release hash>,
}
```

In a bilateral repo the counterparty signs both. In a tri-party repo the
counterparty signs the cash Grant and the custodian signs the maturity Grant.
The destination of both Grants is the initiator.

Wait for both Grant transactions to finalize. A Grant signed by any account
other than `debited_asset.account()` is invalid.

## 3. Open atomically

Submit the byte-identical `RepoIsi` as the initiator. A changed term requires
new hashes and new Grants.

After finality, verify:

- the counterparty cash balance decreased by the principal;
- the initiator received cash in the same exact balance scope;
- the initiator collateral balance decreased by the pledged quantity;
- the counterparty or custodian received collateral in the same exact scope;
- `RepoAgreement.cash_source` matches the cash Grant;
- `RepoAgreement.collateral_custody_asset` matches the maturity Grant;
- `RepoAgreement::is_active()` is true (Torii reports `status = active`); and
- initiation events exist for all roles.

Any failure must leave both balances and the agreement map unchanged.

## 4. Monitor margining

Inspect the recorded schedule:

```bash
iroha --config client.toml app repo margin --agreement-id daily_repo
```

When due, a participant may submit:

```bash
iroha --config client.toml app repo margin-call --agreement-id daily_repo
```

Calls before the cadence boundary or at/after maturity are rejected. Margin
events are evidence only; no term or custody asset can be changed.

## 5. Settle at recorded maturity

Ensure the initiator has principal plus deterministic ACT/360 interest in the
same cash scope recorded by the agreement. Confirm current transfer controls
permit the initiator's outgoing cash and the holder's outgoing collateral.

Any recorded participant—the initiator, counterparty, or custodian—may submit
the fixed settlement. Non-participants cannot trigger it.

Submit:

```bash
iroha --config client.toml app repo unwind --agreement-id daily_repo
```

The instruction carries no parties, assets, quantities, substitute collateral,
or timestamp. Submitting it before maturity fails. Submitting after maturity
still settles at the recorded maturity amount.

After finality, verify:

- cash returned to `RepoAgreement.cash_source`;
- collateral returned from
  `RepoAgreement.collateral_custody_asset` to the initiator in the same scope;
- `settlement_timestamp_ms` equals the recorded maturity;
- `RepoAgreement::is_active()` is false (Torii reports `status = settled`);
- settlement events exist for all roles; and
- a replay or an attempt to reuse the agreement identifier is rejected.

## 6. Failure and incident procedure

Before open, a balance owner can revoke its exact permission. After open, the
agreement is immutable and permission revocation does not cancel it.

If a transfer policy or freeze blocks maturity settlement:

1. retain the rejected transaction and current agreement snapshot;
2. inspect controls for both exact source balances;
3. use the ordinary governed control-change process where justified;
4. resubmit the same ID-only settlement; and
5. archive both rejection and final settlement evidence.

Do not attempt an early unwind, caller-selected substitution, agreement state
edit, or reuse of the identifier. Those operations are intentionally absent
from the protocol.
