import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import assert from "node:assert/strict";

import { PRIVACY_PROTOCOL_IDS_V1 as SOURCE_IDS } from "../src/privacyCapabilities.js";
import { PRIVACY_PROTOCOL_IDS_V1 as DIST_IDS } from "../dist/privacyCapabilities.js";

const REPO_ROOT = fileURLToPath(new URL("../../..", import.meta.url));
const EXPECTED_IDS = [
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
];

function source(path) {
  return readFileSync(`${REPO_ROOT}/${path}`, "utf8");
}

test("JavaScript, Python, and the Rust data model pin one canonical protocol order", () => {
  assert.deepEqual(SOURCE_IDS, EXPECTED_IDS);
  assert.deepEqual(DIST_IDS, EXPECTED_IDS);

  const python = source("python/iroha_python/src/iroha_python/privacy_catalog.py");
  const rust = source("crates/iroha_data_model/src/privacy.rs");
  for (const id of EXPECTED_IDS) {
    assert.match(python, new RegExp(`"${id}"`));
    assert.match(rust, new RegExp(`"${id}"`));
  }
});

test("native SDK capability archives use the typed snapshot instead of free-form rows", () => {
  for (const path of [
    "crates/connect_norito_bridge/src/lib.rs",
    "crates/iroha_js_host/src/lib.rs",
    "python/iroha_python/iroha_python_rs/src/lib.rs",
  ]) {
    const rust = source(path);
    assert.match(rust, /PrivacyCapabilitySnapshotV1/);
    assert.match(rust, /PrivacyProtocolIdV1::ALL/);
    assert.doesNotMatch(rust, /struct PrivacyAlgorithmEntry/);
    assert.doesNotMatch(rust, /struct PrivacyCapabilitiesV1/);
  }
});

test("retired identifiers remain rejection fixtures, never accepted catalog rows", () => {
  const retired = [
    "zkat-policy-private-auth-v1",
    "silent-threshold-anoncred-v0",
    "sis-hints-anoncred-pq-v0",
    "sis-with-hints",
    "penumbra-masp-v1",
    "aztec-private-rollup-v1",
    "zk-ams-recursive-admission-v0",
    "zk-x509-onchain-identity-v0",
    "jindo-lattice-pcs-zk-v0",
    "miden-stark-note-v1",
  ];
  for (const id of retired) {
    assert.equal(SOURCE_IDS.includes(id), false);
    assert.equal(DIST_IDS.includes(id), false);
  }
});
