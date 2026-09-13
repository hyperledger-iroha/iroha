/** Complete response-shape fixture; this does not attest proof signer trust. */
export function sorafsAliasProjectionFixture(boundBy, manifestHex = "a".repeat(64), name = "docs") {
  return {
    alias: `sora/${name}`, namespace: "sora", name,
    manifest_digest_hex: manifestHex, bound_by: boundBy, bound_epoch: 10, expiry_epoch: 99,
    proof_b64: "cHJvb2Y=", cache_state: "fresh", status_label: "fresh", cache_rotation_due: false,
    cache_age_seconds: 12, proof_generated_at_unix: 1, proof_expires_at_unix: 100,
    proof_expires_in_seconds: 87, policy_positive_ttl_secs: 60, policy_refresh_window_secs: 30,
    policy_hard_expiry_secs: 120, policy_rotation_max_age_secs: 600,
    policy_successor_grace_secs: 10, policy_governance_grace_secs: 5,
    cache_decision: "serve", cache_reasons: [],
    cache_evaluation: {
      decision: "serve", reasons: [], ttl_expires_at: "1970-01-01T00:01:40Z", ttl_expires_at_unix: 100,
      serve_until: null, serve_until_unix: null,
      successor: {
        exists: false, head_hex: manifestHex, approved: false, approved_at: null,
        approved_at_unix: null, depth_to_head: 0, anomalies: [],
      },
      governance: {
        ref_ids: [], revoked: false, frozen: false, rotated: false,
        flags: { revoked: false, frozen: false, rotated: false },
        effective_at: null, effective_at_unix: null,
      },
      policy_successor_grace_secs: 10, policy_governance_grace_secs: 5,
    },
    lineage: {
      successor_of_hex: null, head_hex: manifestHex, depth_to_head: 0, is_head: true,
      superseded_by: null, immediate_successor: null, anomalies: [],
    },
  };
}

/** Complete attestation object retained by the alias inventory endpoint. */
export function sorafsAliasAttestationFixture() {
  return { block_height: 1, block_hash_hex: "ab".repeat(32), chain_id: "fixture-chain" };
}
