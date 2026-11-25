# Data Availability Manifest Governance Packet (Template)

Use this template when Parliament panels review DA manifests for subsidies,
takedowns, or retention changes (roadmap DA-10). Copy the Markdown into the
governance ticket, fill in the placeholders, and attach the completed file
alongside the signed Norito payloads and CI artefacts referenced below.

```markdown
## Manifest Metadata
- Manifest name / version: <string>
- Blob class & governance tag: <taikai_segment · da.taikai.live>
- DA manifest digest (BLAKE3 of canonical Norito bytes): `<manifest_hash>`
- Original payload digest (BLAKE3): `<blob_hash>`
- Signed DA receipt and source location: <artefact reference>
- Torii policy snapshot ID: `<unix timestamp or git sha>`

## Commitment and Receipt Verification
- Torii storage ticket from the operator-signed receipt: `<ticket>`
- Trusted receipt-signing public key used by the producer at submission: `<operator_key>`
- Fetch the canonical manifest: `iroha app da get-blob --storage-ticket "$ticket" --output-dir "$evidence_dir"`
- Obtain the commitment for the receipt's manifest digest: `iroha --output-format json app da commitments-prove --manifest-hash "$manifest_hash" > "$evidence_dir/commitment.json"`
- Extract the membership proof: `jq -e '.proof' "$evidence_dir/commitment.json" > "$evidence_dir/proof.json"`
- Verify against a trusted node on the selected network: `iroha --config "$verifier_config" --output-format json app da commitments-verify --proof-json "$evidence_dir/proof.json" | jq -e '.valid == true'`
- Compare the canonical manifest file's BLAKE3 digest and ticket with the verified commitment. Record the certified block height and independent checkpoint reference: `<height, checkpoint>`
- Panel signatures or vote evidence, when required: <governance record>
- Verification timestamp (UTC): `<timestamp>`

## Retention Verification
| Field | Expected (policy) | Observed (manifest) | Evidence |
|-------|-------------------|---------------------|----------|
| Hot retention (seconds) | <e.g., 86400> | <value> | `<torii.da_ingest.replication_policy dump | CI link>` |
| Cold retention (seconds) | <e.g., 1209600> | <value> |  |
| Required replicas | <value> | <value> |  |
| Storage class | <hot / warm / cold> | <value> |  |
| Governance tag | <da.taikai.live> | <value> |  |

## Context
- Request type: <Subsidy | Takedown | Manifest rotation | Emergency freeze>
- Origin ticket / compliance reference: <link or ID>
- Subsidy / rent impact: <expected XOR change or “n/a”>
- Moderation appeal link (if any): <case_id or link>

## Decision Summary
- Panel: <Infrastructure | Moderation | Treasury>
- Vote tally: `<for>/<against>/<abstain>` (quorum `<threshold>` met?)
- Activation / rollback height or window: `<block/slot range>`
- Follow-up actions:
  - [ ] Notify Treasury / rent ops
  - [ ] Update transparency report (`TransparencyReportV1`)
  - [ ] Schedule buffer audit

## Escalation & Reporting
- Escalation track: <Subsidy | Compliance | Emergency Freeze>
- Transparency report link / ID (if updated): <`TransparencyReportV1` CID>
- Proof-token bundle or ComplianceUpdate reference: <path or ticket ID>
- Rent / reserve ledger delta (if applicable): <`ReserveSummaryV1` snapshot link>
- Telemetry snapshot URL(s): <Grafana permalink or artefact ID>
- Notes for Parliament minutes: <summary of deadlines / obligations>

## Attachments
- [ ] Canonical DA manifest (`.norito`), operator-signed receipt and verified commitment proof
- [ ] JSON summary / CI artefact proving retention values
- [ ] Proof token or compliance packet (for takedowns)
- [ ] Buffer telemetry snapshot (`iroha_settlement_buffer_xor`)
```

Archive every completed packet under the Governance DAG entry for the vote so
subsequent reviews can reference the manifest digest without repeating the full
ceremony.【specs/governance_playbook.md:24】
