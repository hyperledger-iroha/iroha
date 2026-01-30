---
lang: fr
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2026-01-03T18:08:00.700192+00:00"
translation_last_reviewed: 2026-01-30
---

# Domain Endorsements

Domain endorsements let operators gate domain creation and reuse under a committee‑signed statement. The endorsement payload is a Norito object recorded on chain so clients can audit who attested to which domain and when.

## Payload shape

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: canonical domain identifier
- `committee_id`: human‑readable committee label
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: block heights bounding validity
- `scope`: optional dataspace plus an optional `[block_start, block_end]` window (inclusive) that **must** cover the accepting block height
- `signatures`: signatures over `body_hash()` (endorsement with `signatures = []`)
- `metadata`: optional Norito metadata (proposal ids, audit links, etc.)

## Enforcement

- Endorsements are required when Nexus is enabled and `nexus.endorsement.quorum > 0`, or when a per‑domain policy marks the domain as required.
- Validation enforces domain/statement hash binding, version, block window, dataspace membership, expiry/age, and committee quorum. Signers must have live consensus keys with the `Endorsement` role. Replays are rejected by `body_hash`.
- Endorsements attached to domain registration use metadata key `endorsement`. The same validation path is used by the `SubmitDomainEndorsement` instruction, which records endorsements for auditing without registering a new domain.

## Committees and policies

- Committees can be registered on‑chain (`RegisterDomainCommittee`) or derived from config defaults (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `default`).
- Per‑domain policies are configured via `SetDomainEndorsementPolicy` (committee id, `max_endorsement_age`, `required` flag). When absent, Nexus defaults are used.

## CLI helpers

- Build/sign an endorsement (outputs Norito JSON to stdout):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- Submit an endorsement:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- Manage governance:
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

Validation failures return stable error strings (quorum mismatch, stale/expired endorsement, scope mismatch, unknown dataspace, missing committee).
