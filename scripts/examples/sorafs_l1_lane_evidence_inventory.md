# L1 lane inventory detached-signature example

The commands below show the public workflow. `SIGNER_SIGNATURE_HEX` is returned
by the independently administered authenticated software-signing service after
it signs `signing.payload` with role `l1-lane-evidence-inventory`. No private key
is passed to this tool or stored beside these artifacts.

```bash
summary_args=(
  --summary ai_prescreen=/evidence/ai_prescreen.json
  --summary appeal_finance=/evidence/appeal_finance.json
  --summary gateway_compliance=/evidence/gateway_compliance.json
  --summary gateway_load=/evidence/gateway_load.json
  --summary governance_dag=/evidence/governance_dag.json
  --summary hedging_billing=/evidence/hedging_billing.json
  --summary moderation_panel=/evidence/moderation_panel.json
  --summary orderbook=/evidence/orderbook.json
  --summary pdp=/evidence/pdp.json
  --summary pop_credentials=/evidence/pop_credentials.json
  --summary por=/evidence/por.json
  --summary potr=/evidence/potr.json
  --summary reference_sdk_release=/evidence/reference_sdk_release.json
  --summary repair=/evidence/repair.json
  --summary reputation=/evidence/reputation.json
  --summary reserve_rent=/evidence/reserve_rent.json
  --summary transparency=/evidence/transparency.json
)

trust_args=(
  --deployment-id sorafs-taira-qualification-2026-08
  --environment production
  --now-unix 1800000000
  --verification-public-key-hex "$SIGNER_PUBLIC_KEY_HEX"
  --service-id sorafs-l1-lane-inventory-signer-a
  --administrator-id sorafs-l1-lane-inventory-admin-b
  --key-revision 7
  --policy-revision 11
  --policy-digest-sha256 "$SIGNER_POLICY_SHA256"
  --expected-topology-qualification-summary-sha256 "$TOPOLOGY_SUMMARY_SHA256"
  --expected-topology-manifest-sha256 "$TOPOLOGY_MANIFEST_SHA256"
  --expected-topology-canonical-manifest-sha256 "$CANONICAL_TOPOLOGY_MANIFEST_SHA256"
  --expected-validator-ids-sha256 "$TAIRA_VALIDATOR_IDS_SHA256"
)

python3 scripts/sorafs_l1_lane_evidence_inventory.py prepare \
  "${summary_args[@]}" "${trust_args[@]}" \
  --generated-at-unix 1800000000 \
  --prepared-out prepared-inventory.json \
  --signing-payload-out signing.payload

# Submit signing.payload to the authenticated external software signer here.

python3 scripts/sorafs_l1_lane_evidence_inventory.py finalize \
  "${summary_args[@]}" "${trust_args[@]}" \
  --prepared prepared-inventory.json \
  --signature-hex "$SIGNER_SIGNATURE_HEX" \
  --inventory-out signed-inventory.json

python3 scripts/sorafs_l1_lane_evidence_inventory.py verify \
  "${summary_args[@]}" "${trust_args[@]}" \
  --inventory signed-inventory.json \
  --verification-out inventory-verification.json
```

Run `verify` a second time with the same arguments and compare the two output
files byte-for-byte. A changed clock or changed summary is a new review and must
not be relabeled as the same replay.

The finalized inventory and complete public trust tuple are mandatory inputs to
both phases of `build_sorafs_foundational_prerequisite.py`. Both phases also
receive the same exact ordered 17 `--lane-summary GATE=PATH` values. The
production-readiness collection runner receives the inventory and trust tuple
once, independently replays those 17 paths in both aggregate runs, and binds the
inventory SHA-256 into its payload-free aggregate without changing the 17/17
summary counts. Its deterministic input set is exactly 22 files: topology
summary and envelope, resilience summary, signed inventory, foundational
envelope, and the 17 lane summaries.
