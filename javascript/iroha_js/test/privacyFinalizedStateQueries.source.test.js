import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const clientSource = readFileSync(
  new URL("../src/toriiClient.js", import.meta.url),
  "utf8",
);
const nativeSource = readFileSync(
  new URL(
    "../../../crates/iroha_js_host/src/authenticated_privacy_state_query.rs",
    import.meta.url,
  ),
  "utf8",
);
const declarations = readFileSync(
  new URL("../index.d.ts", import.meta.url),
  "utf8",
);

test("IDs 97-104 use one closed native signed-query and response boundary", () => {
  for (const method of [
    "getPrivacyZkAceReplayNullifierV1",
    "getPrivacyProofManagedPoolStateV1",
    "getPrivacyOrchardPoolStateV1",
    "getPrivacyOrchardNullifierV1",
    "getPrivacyAnonymousPgcPoolStateV1",
    "getPrivacyZkAmsAdmissionV1",
    "getPrivacyZkAmsProvisionV1",
    "getPrivacyZkX509CertificateNullifierV1",
  ]) {
    assert.match(clientSource, new RegExp(`async ${method}\\(`, "u"));
    assert.match(declarations, new RegExp(`${method}\\(`, "u"));
  }
  assert.match(clientSource, /privacyBuildFinalizedStateQueryV1/u);
  assert.match(clientSource, /privacyInspectFinalizedStateQueryResponseV1/u);
  assert.match(clientSource, /"\/v1\/query"/u);
  assert.match(clientSource, /canonicalAuth: normalized\.canonicalAuth/u);
  assert.match(clientSource, /exactNetworkId: normalized\.networkId/u);
  assert.match(clientSource, /disableRetries: true/u);
  assert.match(clientSource, /response\.status === 404/u);
});

test("native state inspection is canonical, typed, finalized, and request-bound", () => {
  for (const query of [
    "FindPrivacyZkAceReplayNullifierV1",
    "FindPrivacyProofManagedPoolStateV1",
    "FindPrivacyOrchardPoolStateV1",
    "FindPrivacyOrchardNullifierV1",
    "FindPrivacyAnonymousPgcPoolStateV1",
    "FindPrivacyZkAmsAdmissionV1",
    "FindPrivacyZkAmsProvisionV1",
    "FindPrivacyZkX509CertificateNullifierV1",
  ]) {
    assert.match(nativeSource, new RegExp(query, "u"));
  }
  assert.match(nativeSource, /build_signed_request_v1/u);
  assert.match(nativeSource, /decode_canonical_with_limits/u);
  assert.match(nativeSource, /canonical != response/u);
  assert.match(nativeSource, /view\.network_id != expected_network_id/u);
  assert.match(nativeSource, /view\.validate\(\)/u);
  assert.match(nativeSource, /stringify_projection_numbers/u);
  assert.doesNotMatch(nativeSource, /QueryRequest::Start/u);
});

test("state-query bindings reject aliases, zero material, and open protocol choices", () => {
  assert.match(nativeSource, /privacy state-query discriminant is outside the closed union/u);
  assert.match(nativeSource, /proof-managed state query protocol is outside its closed union/u);
  assert.match(nativeSource, /must not be all zero/u);
  assert.match(nativeSource, /must contain exactly \{expected\} bytes/u);
  assert.match(clientSource, /PRIVACY_PROOF_MANAGED_QUERY_PROTOCOL_INDEX_V1/u);
  assert.match(clientSource, /exactNonzeroFixed32V1/u);
});
