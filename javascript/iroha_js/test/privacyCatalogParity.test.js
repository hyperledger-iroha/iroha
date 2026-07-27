import { test } from "node:test";
import assert from "node:assert/strict";

import * as sourceApi from "../src/index.js";
import * as distApi from "../dist/index.js";
import {
  PRIVACY_PROTOCOL_IDS_V1 as SOURCE_IDS,
  PrivacyCapabilitySnapshotError as SourceSnapshotError,
  parsePrivacyCapabilitySnapshotV1 as parseSourceSnapshot,
} from "../src/privacyCapabilities.js";
import {
  PRIVACY_PROTOCOL_IDS_V1 as DIST_IDS,
  PrivacyCapabilitySnapshotError as DistSnapshotError,
  parsePrivacyCapabilitySnapshotV1 as parseDistSnapshot,
} from "../dist/privacyCapabilities.js";

const EXPECTED_PROTOCOL_IDS = Object.freeze([
  "zk-ace-pq-authorization-v0",
  "anonymous-pgc-k-out-of-n-v1",
  "verange-transparent-range-v1",
  "iroha-zk-ams-v1",
  "vega-existing-credential-zk-v0",
  "iroha-zk-x509-stark-p256-v0",
  "iroha-jindo-polynomial-commitment-v0",
  "iroha-bootle-lantern-anoncred-v1",
  "orchard-halo2-actions-v1",
  "monero-fcmp-plus-plus-v1",
  "iroha-ivm-private-note-stark-v1",
  "pq-masp-stark-v0",
]);

const RETIRED_EXPORTS = Object.freeze([
  "getPrivacyAlgorithmDescriptor",
  "getPrivacyAlgorithmDescriptors",
  "getPrivacyCapabilities",
  "getPrivacyCriteria",
  "validatePrivacyAlgorithmDescriptor",
  "buildPrivacyProofEnvelope",
  "buildZkAtPolicyProofV1",
  "buildZkAtDevProofFixture",
  "buildSilentThresholdCredentialShowingProofV0",
  "buildSilentThresholdCredentialDevProofFixture",
  "buildSisHintsAnonymousCredentialProofV0",
  "buildSisHintsCredentialDevProofFixture",
  "buildPenumbraSpendProofV1",
  "buildPenumbraOutputProofV1",
  "buildAztecPrivateKernelProofV1",
  "buildMidenStarkTransactionProofV1",
]);

function snapshot() {
  return {
    version: 1,
    committed_height: 1,
    consensus_policy: {
      current_limits: {
        max_actions_per_transaction: 1,
        max_actions_per_block: 2,
        max_proof_bytes_per_action: 8 * 1024 * 1024,
        max_action_bytes: 8 * 1024 * 1024,
        max_privacy_bytes_per_transaction: 8 * 1024 * 1024,
        max_privacy_bytes_per_block: 16 * 1024 * 1024,
        max_statement_and_encrypted_output_bytes_per_transaction: 256 * 1024,
        max_nullifiers_per_action: 8,
        max_commitments_per_action: 8,
        retained_root_count: 2048,
      },
      pending_tightening: null,
    },
    protocols: EXPECTED_PROTOCOL_IDS.map((protocol) => ({
      protocol_id: { protocol, value: null },
      compiled_profile: {
        status: "unavailable",
        value: { reason: "engine-unavailable", detail: null },
      },
      activation: null,
    })),
  };
}

test("source and checked dist expose exactly the canonical 12 protocol ids", () => {
  assert.deepEqual(SOURCE_IDS, EXPECTED_PROTOCOL_IDS);
  assert.deepEqual(DIST_IDS, EXPECTED_PROTOCOL_IDS);
  assert.equal(new Set(SOURCE_IDS).size, 12);
  assert.equal(new Set(DIST_IDS).size, 12);
});

test("source and checked dist parse the same closed capability snapshot", () => {
  assert.deepEqual(parseSourceSnapshot(snapshot()), parseDistSnapshot(snapshot()));
});

test("aliases, unknown protocol ids, and unknown fields fail closed in both builds", () => {
  const hostileCases = [
    (value) => {
      value.protocols[3].protocol_id.protocol = "zk-ams-recursive-admission-v0";
    },
    (value) => {
      value.protocols[5].protocol_id.protocol = "zk-x509-onchain-identity-v0";
    },
    (value) => {
      value.protocols[0].protocol_id.protocol = "ZK-ACE";
    },
    (value) => {
      value.protocols[0].unexpected = true;
    },
    (value) => {
      value.consensus_policy.current_limits.unexpected = 1;
    },
  ];
  for (const mutate of hostileCases) {
    const sourceValue = snapshot();
    mutate(sourceValue);
    assert.throws(() => parseSourceSnapshot(sourceValue), SourceSnapshotError);
    const distValue = snapshot();
    mutate(distValue);
    assert.throws(() => parseDistSnapshot(distValue), DistSnapshotError);
  }
});

test("retired catalog and research-builder exports are absent", () => {
  for (const name of RETIRED_EXPORTS) {
    assert.equal(Object.hasOwn(sourceApi, name), false, `source export ${name}`);
    assert.equal(Object.hasOwn(distApi, name), false, `dist export ${name}`);
  }
});
