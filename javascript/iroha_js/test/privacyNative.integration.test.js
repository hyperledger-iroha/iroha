import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  PRIVACY_COMPILED_PROFILE_CATALOG_VALIDATION_STATUS_V1,
  PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
  isPrivacyNativeAvailable,
  privacyCompiledProfileCatalogV1,
} from "../src/crypto.js";
import { getNativeBinding } from "../src/native.js";
import { decodePrivacyExact12CapabilityManifestV1 } from "../src/privacyCapabilities.js";

test("the real addon projection of Core's manifest reaches the managed parser", () => {
  const archive = Buffer.from(
    readFileSync(
      new URL("../../../fixtures/privacy/exact12_capability_manifest_v1.norito.b64", import.meta.url),
      "utf8",
    ).trim(),
    "base64",
  );
  assert.equal(getNativeBinding().privacyValidateExact12CapabilityManifestV1(archive), 0);
  const manifest = decodePrivacyExact12CapabilityManifestV1(archive);
  const { current_limits: current, pending_update: pending } = manifest.consensus_policy;
  assert.equal(current.retained_root_count, 2048);
  assert.equal(pending.next_limits.retained_root_count, 1024);
  const note = manifest.protocols[10];
  assert.equal(note.protocol_id.protocol, "iroha-ivm-private-note-stark-v1");
  assert.equal(note.privacy_feature_mask, 7);
  assert.equal(manifest.manifest_digest.length, 32);
});


test("authenticated N-API ABI33 executes the canonical privacy catalog contract", () => {
  assert.equal(globalThis.__IROHA_NATIVE_BINDING__, undefined);

  const native = getNativeBinding();
  assert.equal(
    native.connectNoritoBridgeAbiVersion(),
    PRIVACY_REQUIRED_BRIDGE_ABI_VERSION,
  );
  assert.equal(typeof native.privacyCompiledProfileCatalogV1, "function");
  assert.equal(
    typeof native.privacyValidateCompiledProfileCatalogV1,
    "function",
  );
  for (const method of [
    "privacyValidateExact12CapabilityManifestV1",
    "privacyExact12CapabilityManifestJsonV1",
    "privacyRequireExact12CapabilityTupleV1",
  ]) {
    assert.equal(typeof native[method], "function", method);
  }
  assert.notEqual(
    native.privacyValidateExact12CapabilityManifestV1(Buffer.alloc(0)),
    0,
  );
  assert.notEqual(
    native.privacyValidateExact12CapabilityManifestV1(
      Buffer.from("caller-provided-digest-shell"),
    ),
    0,
  );
  assert.equal(isPrivacyNativeAvailable(), true);

  const direct = Buffer.from(native.privacyCompiledProfileCatalogV1());
  const publicArchive = privacyCompiledProfileCatalogV1();
  assert.ok(direct.length > 0);
  assert.deepEqual(publicArchive, direct);
  assert.equal(
    native.privacyValidateCompiledProfileCatalogV1(direct),
    PRIVACY_COMPILED_PROFILE_CATALOG_VALIDATION_STATUS_V1.VALID,
  );
  assert.deepEqual(privacyCompiledProfileCatalogV1(), direct);

  const hostile = [
    direct.subarray(0, direct.length - 1),
    direct.subarray(1),
    Buffer.concat([direct, Buffer.from([0])]),
  ];
  for (const index of new Set([0, Math.floor(direct.length / 2), direct.length - 1])) {
    const mutated = Buffer.from(direct);
    mutated[index] ^= 0x80;
    hostile.push(mutated);
  }
  for (const archive of hostile) {
    assert.notEqual(
      native.privacyValidateCompiledProfileCatalogV1(archive),
      PRIVACY_COMPILED_PROFILE_CATALOG_VALIDATION_STATUS_V1.VALID,
    );
  }

  publicArchive.fill(0);
  assert.deepEqual(privacyCompiledProfileCatalogV1(), direct);
});
