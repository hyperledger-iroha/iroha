---
lang: fr
direction: ltr
source: docs/source/ministry/policy_jury_ballots.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff3faabda5f1c277f545b7edbbc93f3b58dee65cec943cfd464a026b2984a146
source_last_modified: "2026-01-03T18:07:57.647015+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Policy Jury Sortition & Ballots
---

Roadmap item **MINFO-5 — Policy jury voting toolkit** requires a portable format
for deterministic juror selection plus sealed commit → reveal ballots.  The
`iroha_data_model::ministry::jury` module now ships three Norito payloads that
cover the entire voting workflow:

1. **`PolicyJurySortitionV1`** – records the draw metadata (proposal id,
   round id, proof-of-personhood snapshot digest, randomness beacon),
   committee size, selected jurors, and the waitlist used for automatic
   failover.  Each primary slot may include a `PolicyJuryFailoverPlan`
   pointing at the waitlist rank it should escalate to after its grace
   period lapses.  The structure is intentionally deterministic so auditors
   can replay the draw and regenerate the manifest from the same POP
   snapshot + beacon.
2. **`PolicyJuryBallotCommitV1`** – sealed commitment written before ballots
   are revealed.  It stores the round/proposal/juror identifiers, the
   Blake2b‑256 digest of the juror id + vote choice + nonce tuple, the capture
   timestamp, and the ballot mode (`plaintext` or `zk-envelope` when the
   `zk-ballot` feature is active).  `PolicyJuryBallotCommitV1::verify_reveal`
   ensures the stored digest matches the reveal payload.
3. **`PolicyJuryBallotRevealV1`** – the public reveal object containing the
   vote choice, the nonce used at commit time, and optional ZK proof URIs.
   Reveals require a minimum 16-byte nonce so governance can treat the
   commitment as binding even when jurors operate over insecure channels.

The `PolicyJurySortitionV1::validate` helper enforces committee sizing,
duplicate detection (no juror may appear in both the committee and the
waitlist), ordered waitlist ranks, and valid failover references.  The ballot
validation routines raise `PolicyJuryBallotError` when proposal or round ids
drift, when jurors attempt to reveal with an incorrect nonce, or when a
`zk-envelope` commitment fails to provide matching proof references in its
reveal.

### Integrating with clients

- Governance tools should persist the sortition manifest and include it in
  policy packets so observers can recompute the POP snapshot digest and
  confirm that the randomness beacon plus candidate set lead to the same
  juror assignments.
- Juror clients record a `PolicyJuryBallotCommitV1` immediately after
  generating the nonce for their vote.  The derived commitment bytes can be
  submitted to Torii as a base64 value or embedded directly into Norito
  events.
- During the reveal phase, jurors emit `PolicyJuryBallotRevealV1`.  Operators
  feed the payload to `PolicyJuryBallotCommitV1::verify_reveal` before
  accepting the vote, ensuring the reveal was not swapped or tampered with.
- When the `zk-ballot` feature is enabled, jurors can attach deterministic
  proof URIs (e.g., `sorafs://proofs/pj-2026-02/juror-5`) so downstream
  auditors can retrieve the zero-knowledge witness bundle referenced by the
  commitment.

All three structures derive `Encode`, `Decode`, and `IntoSchema`, meaning they
are available to ISI flows, CLI tooling, SDKs, and the governance REST API.
See `crates/iroha_data_model/src/ministry/jury.rs` for the canonical Rust
definitions and helper methods.

### CLI support for sortition manifests

Roadmap item **MINFO-5** also called for reproducible tooling so governance can
ship verifiable policy-jury rosters before each referendum packet is published.
The workspace now exposes the `cargo xtask ministry-jury sortition` command:

```bash
cargo xtask ministry-jury sortition \
  --roster docs/examples/ministry/policy_jury_roster_example.json \
  --proposal AC-2026-042 \
  --round PJ-2026-02 \
  --beacon 22b1e48d47123f5c9e3f0cc0c8e34aa3c5f9c49a2cbb70559d3cb0ddc1a6ef01 \
  --committee-size 3 \
  --waitlist-size 2 \
  --drawn-at 2026-01-15T09:00:00Z \
  --waitlist-ttl-hours 72 \
  --out artifacts/ministry/policy_jury_sortition.json
```

- `--roster` accepts a deterministic PoP roster (JSON example:
  `docs/examples/ministry/policy_jury_roster_example.json`).  Each entry
  declares the `juror_id`, `pop_identity`, weight, and optional
  `grace_period_secs`.  Ineligible entries are filtered automatically.
- `--beacon` injects the 32-byte randomness beacon captured in the governance
  minutes.  The CLI wires the beacon directly into the ChaCha20 RNG so auditors
  can replay the draw byte-for-byte.
- `--committee-size`, `--waitlist-size`, and `--waitlist-ttl-hours` control the
  number of seated jurors, the failover buffer, and the expiry timestamp applied
  to the waitlist entries.  When a failover rank exists for a slot, the command
  records a `PolicyJuryFailoverPlan` pointing at the matching waitlist rank.
- `--drawn-at` records the wall-clock timestamp for the sortition; the tool
  converts it into Unix milliseconds for the manifest.

The generated manifest is a fully validated `PolicyJurySortitionV1` payload.
Large deployments typically save the output under `artifacts/ministry/` so it
can be bundled directly into referendum packets alongside the review-panel
summary.  An illustrative output is available in
`docs/examples/ministry/policy_jury_sortition_example.json` so SDK teams can
exercise their Norito decoders without replaying an entire draw locally.

### Ballot commit/reveal helpers

Juror clients need deterministic tooling for the commit → reveal flow as well.
The same `cargo xtask ministry-jury` command now exposes the following helpers:

```bash
cargo xtask ministry-jury ballot commit \
  --proposal AC-2026-042 \
  --round PJ-2026-02 \
  --juror citizen:ada \
  --choice approve \
  --nonce-hex aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899 \
  --committed-at 2026-02-01T13:00:00Z \
  --out artifacts/ministry/policy_jury_commit_ada.json \
  --reveal-out artifacts/ministry/policy_jury_reveal_ada.json

cargo xtask ministry-jury ballot verify \
  --commit artifacts/ministry/policy_jury_commit_ada.json \
  --reveal artifacts/ministry/policy_jury_reveal_ada.json
```

- `ballot commit` emits a `PolicyJuryBallotCommitV1` JSON payload.  When
  `--out` is omitted the command prints the commitment to stdout.  If
  `--reveal-out` is supplied the tool also writes the matching
  `PolicyJuryBallotRevealV1`, reusing the provided nonce and applying the
  optional `--revealed-at` timestamp (defaults to `--committed-at` or the
  current time).
- `--nonce-hex` accepts any even-length hex string ≥ 16 bytes.  When omitted the
  helper generates a 32-byte nonce using `OsRng`, making it easy to script
  juror workflows without custom randomness plumbing.
- `--choice` is case-insensitive and accepts `approve`, `reject`, or `abstain`.

`ballot verify` cross-checks the commitment/reveal pair via
`PolicyJuryBallotCommitV1::verify_reveal`, guaranteeing that the round id,
proposal id, juror id, nonce, and vote choice all align before the reveal is
admitted to Torii.  The helper exits with a non-zero status when validation
fails, making it safe to wire into CI or local juror portals.
