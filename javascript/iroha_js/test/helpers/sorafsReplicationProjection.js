/** Exact inventory projection built from the maintained native replication fixture. */
import { readFileSync } from "node:fs";
const nativeFixture = JSON.parse(readFileSync(new URL("../../../../fixtures/sorafs_manifest/replication_order/order_v1.json", import.meta.url), "utf8"));
export function sorafsReplicationProjectionFixture(owner) {
  const order = {
    version: nativeFixture.schema_version,
    order_id_hex: nativeFixture.order_id_hex,
    manifest_cid_b64: Buffer.from(nativeFixture.manifest_cid_hex, "hex").toString("base64"),
    manifest_digest_hex: nativeFixture.manifest_digest_hex,
    chunking_profile: "sorafs.sf1@1.0.0",
    target_replicas: nativeFixture.target_replicas,
    assignments: structuredClone(nativeFixture.assignments),
    issued_at: nativeFixture.issued_at, deadline_at: nativeFixture.deadline_at,
    sla: structuredClone(nativeFixture.sla), metadata: structuredClone(nativeFixture.metadata),
  };
  return {
    order_id_hex: order.order_id_hex, manifest_digest_hex: order.manifest_digest_hex,
    issued_by: owner, issued_epoch: order.issued_at, deadline_epoch: order.deadline_at,
    status: { state: "pending" }, canonical_order_b64: Buffer.from(nativeFixture.norito_bytes_hex, "hex").toString("base64"),
    assignment_revision: 1, order,
    provider_completions: [{
      provider_hex: order.assignments[0].provider_id_hex, completed_by: owner,
      completion_epoch: order.issued_at + 10, assignment_revision: 1,
      completion_authority: { provider_owner: owner, signer_policy: {
        policy_id_hex: "31".repeat(32), revision: 2, predecessor_digest_hex: "32".repeat(32), policy_digest_hex: "33".repeat(32),
      } },
      finalized_anchor: { height: 51, block_hash_hex: "42".repeat(32) },
    }],
    providers: order.assignments.map(item => item.provider_id_hex),
  };
}
export function sorafsReplicationAttestationFixture() {
  return { block_height: 51, block_hash_hex: "42".repeat(32), chain_id: "fixture-chain" };
}
