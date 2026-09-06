import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import assert from "node:assert/strict";

import { PRIVACY_PROTOCOL_IDS_V1 as SOURCE_IDS } from "../src/privacyCapabilities.js";
import { PRIVACY_PROTOCOL_IDS_V1 as DIST_IDS } from "../dist/privacyCapabilities.js";

const REPO_ROOT = fileURLToPath(new URL("../../..", import.meta.url));
const EXPECTED_IDS = [
  "zk-ace-pq-authorization-v1",
  "anonymous-pgc-k-out-of-n-v1",
  "verange-transparent-range-v1",
  "iroha-zk-ams-v1",
  "vega-existing-credential-zk-v1",
  "iroha-zk-x509-stark-p256-v1",
  "iroha-jindo-polynomial-commitment-v1",
  "iroha-bootle-lantern-anoncred-v1",
  "orchard-halo2-actions-v1",
  "monero-fcmp-plus-plus-v1",
  "iroha-ivm-private-note-stark-v1",
  "pq-masp-stark-v1",
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

test("native SDK archives expose local compiled profiles without synthesizing readiness", () => {
  for (const path of [
    "crates/connect_norito_bridge/src/lib.rs",
    "crates/iroha_js_host/src/lib.rs",
    "python/iroha_python/iroha_python_rs/src/lib.rs",
  ]) {
    const rust = source(path);
    const productionRust = rust.split(/\n#\[cfg\(test\)\]\nmod tests\s*\{/u, 1)[0];
    assert.match(rust, /PrivacyCompiledProfileCatalogV1/);
    assert.match(rust, /PrivacyProtocolIdV1::ALL/);
    assert.match(rust, /compiled_privacy_profile_catalog_v1/);
    assert.match(
      rust,
      /validate_local_privacy_compiled_profile_catalog_archive_v1/,
    );
    assert.doesNotMatch(
      productionRust,
      /PrivacyConsensusPolicyV1::taira_default\(\)/,
    );
    assert.doesNotMatch(rust, /fn privacy_capabilities\s*\(/);
    assert.doesNotMatch(rust, /pub fn privacy_capabilities_v1\s*\(/);
    assert.doesNotMatch(rust, /name = "privacy_capabilities_v1"/);
    assert.doesNotMatch(rust, /iroha_privacy_capabilities_v1/);
    assert.doesNotMatch(rust, /committed_privacy_capability_snapshot_v1/);
    assert.doesNotMatch(rust, /struct PrivacyAlgorithmEntry/);
    assert.doesNotMatch(rust, /struct PrivacyCapabilitiesV1/);
  }
});

test("only a fresh committed Torii view supplies authoritative privacy readiness", () => {
  const runtime = source("crates/iroha_torii/src/runtime.rs");
  const state = source("crates/iroha_core/src/state.rs");
  assert.match(
    runtime,
    /handle_privacy_capabilities[\s\S]*PrivacyExact12CapabilityManifestV1[\s\S]*privacy_capability_snapshot_v1\(\)[\s\S]*exact12_capability_manifest_v1\(\)/,
  );
  assert.match(
    state,
    /privacy_capability_snapshot_v1[\s\S]*committed_height[\s\S]*world\.privacy_consensus_policy\(\)[\s\S]*world\s*\.privacy_activations\(\)/,
  );

  const javascriptParser = source(
    "javascript/iroha_js/src/privacyCapabilities.js",
  );
  assert.match(javascriptParser, /PrivacyExact12CapabilityManifestV1/);
  assert.match(javascriptParser, /privacyExact12CapabilityManifestTransportV1/);
  assert.doesNotMatch(javascriptParser, /parsePrivacyCapabilitySnapshotV1/);
  assert.doesNotMatch(javascriptParser, /privacyCapabilityTransportV1/);
  for (const client of [
    source("javascript/iroha_js/src/toriiClient.js"),
    source("python/iroha_python/src/iroha_python/client.py"),
  ]) {
    assert.match(client, /\/v1\/privacy\/capabilities/);
  }
  assert.doesNotMatch(
    source("javascript/iroha_js/src/toriiBrowserClient.js"),
    /\/v1\/privacy\/capabilities/,
  );
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
