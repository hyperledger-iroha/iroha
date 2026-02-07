---
lang: ka
direction: ltr
source: docs/source/sorafs/signing_ceremony.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 474e9259f3ff5bc814c371ce46a6382577af736da3e59a2d9e5b014f9ea2b994
source_last_modified: "2026-01-05T09:28:12.079783+00:00"
translation_last_reviewed: 2026-02-07
---

# Sora Parliament Fixture Approval (SF-1b)

The manual "council signing ceremony" has been retired. The SoraFS chunker
fixtures are now ratified exclusively through the **Sora Parliament**, the
multi-body sortition-based DAO that governs the Nexus network. Parliament
members bond XOR to gain Sora citizen status, are randomly assigned to
specialised panels, and cast on-chain votes that approve, reject, or roll back
chunker fixture releases.

offline process and how developers interact with it.

## Parliament Overview

- **Citizenship**: Participants lock the required XOR bond to enrol as Sora
  citizens and become eligible for sortition.
- **Panels**: Parliament splits responsibilities across rotating panels
  (Infrastructure, Moderation, Treasury, etc.). The Infrastructure Panel now
  handles SoraFS fixture approvals.
- **Sortition & Rotation**: Panel seats are re-drawn at the cadence defined in
  the Parliament constitution so no single group can monopolise approvals.

## Fixture Approval Flow

1. **Proposal Submission**
   - Tooling WG uploads the candidate `manifest_blake3.json` bundle and fixture
     diff to the SoraFS on-chain registry (`sorafs.fixtureProposal` call).
   - The proposal stores the BLAKE3 digest, version metadata, and change notes.
2. **Review & Voting**
   - The Infrastructure Panel receives the proposal assignment via the
     Parliament task queue.
   - Panel members inspect CI artifacts, run parity tests, and cast weighted
     votes on-chain.
3. **Finalisation**
   - Once the on-chain threshold is met, the Nexus runtime emits an approval
     event containing the canonical manifest digest and the Merkle commitment to
     the fixture payload.
   - The event is mirrored into the SoraFS registry so clients can fetch the
     latest Parliament-approved manifest.
4. **Distribution**
   - CLI helpers (`cargo xtask sorafs-fetch-fixture`) pull the approved manifest
     directly from the Nexus RPC endpoint. The local repository keeps the
     fixture JSON/TS/Go constants in sync by re-running `export_vectors` and
     validating the digest against the on-chain record.

## Developer Workflow

- Regenerate fixtures with `cargo run -p sorafs_chunker --bin export_vectors`.
- Use the Parliament fetch helper from `xtask` to download the approved envelope,
  verify council signatures, and refresh the local fixtures. Point
  `--signatures` at the Parliament-published envelope; the helper resolves the
  accompanying manifest, recomputes the BLAKE3 digest, and enforces the
  canonical `sorafs.sf1@1.0.0` chunk profile.

  ```
  cargo xtask sorafs-fetch-fixture \
    --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
    --out fixtures/sorafs_chunker
  ```

  Pass `--manifest` if the manifest lives at a different URL or path. The
  command refuses unsigned envelopes unless `--allow-unsigned` is provided for
  local smoke runs.
  When validating the published manifest against a staging gateway, point
  `sorafs-fetch` at the Torii host instead of local payloads:

  ```
  sorafs-fetch \
    --plan=fixtures/chunk_fetch_specs.json \
    --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
    --gateway-manifest-id=<manifest_id_hex> \
    --gateway-chunker-handle=sorafs.sf1@1.0.0 \
    --json-out=reports/staging_gateway.json
  ```

- Local CI no longer requires a `signer.json` roster. Instead,
  `ci/check_sorafs_fixtures.sh` compares the repository state with the latest
  on-chain commitment and fails if they diverge.

## Data Availability Review Packet

When the Infrastructure or Moderation panels evaluate Data Availability
manifests (DA-10), they must attach a governance packet so Parliament can trace
the vote back to the signed Norito artefact.【docs/source/governance_playbook.md:24】

1. Fetch the retention policy snapshot (`torii.da_ingest.replication_policy`) and
   confirm the manifest’s `RetentionPolicy` matches the enforced class (see
   `docs/source/da/replication_policy.md`). Store the CLI output or CI artefact.
2. Record the manifest digest, governance tag, blob class, and retention values
   inside `docs/examples/da_manifest_review_template.md`. Attach the filled
   template, the signed Norito payload (`.to`), and any subsidy/takedown ticket
   references to the Parliament docket.
3. Include moderation/compliance context when the request originated from a
   takedown or appeal so the Governance DAG links the manifest hash with the
   escalation record (`docs/source/sorafs_gateway_compliance_plan.md`,
   `docs/source/sorafs_moderation_panel_plan.md`).
4. During rollbacks, reference the prior packet and delta the retention or
   governance fields so auditors can confirm no silent parameter drift occurred.

## Governance Notes

- The Parliament constitution governs quorum, rotation, and escalation; no
  crate-level configuration is needed.
- Emergency rollbacks are triggered via the Parliament "moderation" panel. The
  infrastructure panel submits a revert proposal referencing the previous
  manifest digest, which replaces the release once approved.
- Historical approvals remain available in the SoraFS registry for forensic
  replay.

## FAQ

- **Where did `signer.json` go?**  
  It was removed. All signer attribution is handled by the Sora Parliament
  contract; individual hardware tokens are no longer tracked in-repo.

- **Do we still require local Ed25519 signatures?**  
  The Parliament runtime stores approvals as on-chain artifacts. `manifest_signatures.json`
  stays in the repository only as developer fixtures and must match the on-chain
  digest recorded in the latest approval event.

- **How do teams monitor approvals?**  
  Subscribe to the `ParliamentFixtureApproved` event or query the registry via
  the Nexus RPC to retrieve the current manifest digest and panel roll call.
