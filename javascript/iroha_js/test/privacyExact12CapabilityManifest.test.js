import { after, test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import * as productionPrivacyCapabilities from "../src/privacyCapabilities.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import {
  LocalSigningContext,
  ToriiClient,
} from "../src/toriiClient.js";
import { NetworkId } from "../src/networkId.js";

const TEST_NATIVE_BINDING = Symbol.for("iroha.test.exact12.native-binding");
const TEST_CANONICAL_AUTH = Object.freeze({
  accountId: "privacy-test@fixture.test",
  privateKey: Buffer.alloc(32, 0x0b),
});
const TEST_LOCAL_SIGNING_CONTEXT = new LocalSigningContext(
  NetworkId.fromBytes(Buffer.alloc(32, 0xa5)), 753,
);
const SUBJECT_ROOT = mkdtempSync(join(tmpdir(), "iroha-exact12-native-authority-"));
writeFileSync(join(SUBJECT_ROOT, "package.json"), '{"type":"module"}\n');

function writeSubjectFile(directory, name, contents) {
  const path = join(directory, name);
  writeFileSync(path, contents, { encoding: "utf8", flag: "wx" });
}

async function loadSubject(name, nativeModuleSource) {
  const directory = join(SUBJECT_ROOT, name);
  const source = readFileSync(
    new URL("../src/privacyCapabilities.js", import.meta.url),
    "utf8",
  );
  mkdirSync(directory, { mode: 0o700 });
  writeSubjectFile(directory, "privacyCapabilities.js", source);
  writeSubjectFile(directory, "native.js", nativeModuleSource);
  for (const dependency of [
    "privacyCapabilityAdmission",
    "privacyCapabilityTransport",
    "strictLosslessJson",
    "networkId",
  ]) {
    const target = new URL(`../src/${dependency}.js`, import.meta.url).href;
    writeSubjectFile(
      directory,
      `${dependency}.js`,
      `export * from ${JSON.stringify(target)};\n`,
    );
  }
  return import(`${pathToFileURL(join(directory, "privacyCapabilities.js")).href}?${name}`);
}

const authenticatedTestSubject = await loadSubject(
  "authenticated-test-loader",
  `const key = Symbol.for("iroha.test.exact12.native-binding");
export function getNativeBinding() {
  const binding = globalThis[key];
  if (binding === undefined) throw new Error("test native binding is absent");
  return binding;
}
`,
);
const browserSubject = await loadSubject(
  "browser-loader",
  `export { getNativeBinding } from ${JSON.stringify(
    new URL("../src/native.browser.js", import.meta.url).href,
  )};\n`,
);

after(() => rmSync(SUBJECT_ROOT, { recursive: true, force: true }));

const {
  PRIVACY_PROTOCOL_IDS_V1,
  PrivacyExact12CapabilityManifestError,
  PrivacyExact12CapabilityManifestV1,
  decodePrivacyExact12CapabilityManifestV1,
  getPrivacyExact12CapabilityManifestV1,
  requirePrivacyExact12CapabilityAdmissionV1,
} = authenticatedTestSubject;

const ARCHIVE = Uint8Array.from([0x4e, 0x52, 0x54, 0x30, 1, 2, 3, 4]);
const CATALOG = Uint8Array.from([0x4e, 0x52, 0x54, 0x30, 9, 8, 7, 6]);
const ACTIVE_PROTOCOL = "anonymous-pgc-k-out-of-n-v1";
const OPERATION_TUPLES = Object.freeze([
  ["zk_ace_authorization_action_v1", "authorization_action", 0],
  ["anonymous_pgc_payment_action_v1", "payment_action", 6],
  ["verange_range_proof_v1", "component", 1],
  ["zk_ams_admission_and_provisioning_v1", "admission_action", 2],
  ["vega_credential_presentation_v1", "presentation_action", 2],
  ["zk_x509_identity_presentation_v1", "presentation_action", 2],
  ["jindo_polynomial_evaluation_v1", "component", 0],
  ["bootle_lantern_credential_presentation_v1", "presentation_action", 2],
  ["orchard_note_action_v1", "note_action", 7],
  ["fcmp_membership_payment_v1", "payment_action", 2],
  ["ivm_private_note_action_v1", "note_action", 7],
  ["pq_masp_note_action_v1", "note_action", 31],
]);
const BINDING_TUPLES = Object.freeze([
  ["stark-fri-sha3-384-goldilocks-v1", "native-goldilocks-sha3-384-stark-fri-v1"],
  ["anonymous-pgc-p256", "native-anonymous-pgc-p256"],
  ["iroha-verange-p256", "native-verange-p256"],
  ["zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512", "native-zk-ams-masked-relaxed-spartan-t256-ristretto255"],
  ["vega-neutron-nova-spartan-hyrax-t256", "native-vega"],
  ["stark-fri-sha3-384-goldilocks-v1", "native-goldilocks-sha3-384-stark-fri-v1"],
  ["jindo-polynomial-commitment", "native-jindo"],
  ["lantern-lnp22-module-linear-norm", "native-lantern-lnp22"],
  ["halo2-ipa-pasta", "native-halo2-orchard"],
  ["fcmp-plus-plus-curve-tree-bulletproofs", "native-fcmp-plus-plus"],
  ["stark-fri-sha3-384-goldilocks-v1", "native-goldilocks-sha3-384-stark-fri-v1"],
  ["stark-fri-sha3-384-goldilocks-v1", "native-goldilocks-sha3-384-stark-fri-v1"],
]);

function tagged(protocol) {
  return { protocol, value: null };
}

function consensusPolicy() {
  return {
    current_limits: {
      max_actions_per_transaction: 1,
      max_actions_per_block: 2,
      max_proof_bytes_per_action: 9 * 1024 * 1024,
      max_action_bytes: 9 * 1024 * 1024 + 256 * 1024 + 512,
      max_privacy_bytes_per_transaction: 9 * 1024 * 1024 + 256 * 1024 + 512,
      max_privacy_bytes_per_block: 16 * 1024 * 1024 - 64 * 1024,
      max_statement_and_encrypted_output_bytes_per_transaction: 256 * 1024,
      max_nullifiers_per_action: 8,
      max_commitments_per_action: 8,
      retained_root_count: 2048,
    },
    pending_update: null,
  };
}

function availablePgcProfile() {
  const binding = releaseBinding(ACTIVE_PROTOCOL, 1);
  return {
    ...binding,
    protocol_limits: {
      protocol: ACTIVE_PROTOCOL,
      limits: { max_anonymity_set_size: 64, max_recipient_count: 8 },
    },
  };
}

function releaseBinding(protocolId, index) {
  const [proofSystem, engine] = BINDING_TUPLES[index];
  return {
    protocol_id: tagged(protocolId),
    proof_system_id: { proof_system: proofSystem, value: null },
    engine_id: { engine, value: null },
    parameter_id: Array(32).fill(index + 1),
    parameter_digest: Array(32).fill(index + 21),
    verifier_digest: Array(32).fill(index + 41),
    statement_schema_digest: Array(32).fill(index + 61),
    engine_manifest_digest: Array(32).fill(index + 81),
  };
}

function manifestPayload() {
  const unavailable = {
    status: "unavailable",
    value: { reason: "engine-unavailable", detail: null },
  };
  const profile = availablePgcProfile();
  return {
    version: 1,
    committed_height: 42,
    consensus_policy: consensusPolicy(),
    network_id: TEST_LOCAL_SIGNING_CONTEXT.networkId.literal,
    protocols: PRIVACY_PROTOCOL_IDS_V1.map((protocolId, index) => {
      const active = protocolId === ACTIVE_PROTOCOL;
      const compiledProfile = active
        ? { status: "available", value: profile }
        : structuredClone(unavailable);
      const policy = active ? {
        protocol_id: tagged(protocolId),
        current_limits: structuredClone(profile.protocol_limits),
        pending_update: null,
      } : null;
      const [operationSchema, executionMode, featureMask] = OPERATION_TUPLES[index];
      return {
        protocol_id: tagged(protocolId),
        operation_schema: { operation_schema: operationSchema, value: null },
        execution_mode: { execution_mode: executionMode, value: null },
        privacy_feature_mask: featureMask,
        compiled_profile: compiledProfile,
        readiness: active
          ? { readiness: "available", detail: null }
          : {
              readiness: "unavailable",
              detail: {
                reason: "compiled-profile",
                detail: structuredClone(unavailable.value),
              },
            },
        policy,
      };
    }),
    manifest_digest: Array(32).fill(0xa5),
  };
}

function sameBytes(left, right) {
  return Buffer.from(left).equals(Buffer.from(right));
}


const TEST_MANIFEST_URL = "https://privacy.example.test/v1/privacy/capabilities";
function manifestResponse(body = ARCHIVE, init = {}, metadata = {}) {
  const response = new Response(body, {
    status: 200,
    headers: { "content-type": "application/x-norito" },
    ...init,
  });
  return {
    body: response.body, headers: response.headers, status: response.status,
    statusText: response.statusText,
    url: metadata.url ?? TEST_MANIFEST_URL,
    redirected: metadata.redirected ?? false,
  };
}
async function fetchTestManifest() {
  return getPrivacyExact12CapabilityManifestV1(new ToriiClient("https://privacy.example.test", {
    localSigningContext: TEST_LOCAL_SIGNING_CONTEXT,
    fetchImpl: async () => manifestResponse(),
  }), { canonicalAuth: TEST_CANONICAL_AUTH });
}

function fakeNative(payload = manifestPayload(), overrides = {}) {
  return {
    connectNoritoBridgeAbiVersion: () => 33,
    privacyCompiledProfileCatalogV1: () => Uint8Array.from(CATALOG),
    privacyValidateCompiledProfileCatalogV1: (bytes) =>
      sameBytes(bytes, CATALOG) ? 0 : 8,
    privacyValidateExact12CapabilityManifestV1: (bytes) =>
      sameBytes(bytes, ARCHIVE) ? 0 : 7,
    privacyExact12CapabilityManifestJsonV1: (bytes) => {
      if (!sameBytes(bytes, ARCHIVE)) throw new Error("noncanonical archive");
      return JSON.stringify(payload);
    },
    privacyRequireExact12CapabilityTupleV1: (bytes, protocolId, expectedNetworkId) => {
      if (!sameBytes(bytes, ARCHIVE) || protocolId !== ACTIVE_PROTOCOL
          || !sameBytes(expectedNetworkId, TEST_LOCAL_SIGNING_CONTEXT.networkId.toBytes())) {
        throw new Error("tuple mismatch");
      }
      return true;
    },
    ...overrides,
  };
}

async function withNative(native, callback) {
  const previous = globalThis[TEST_NATIVE_BINDING];
  globalThis[TEST_NATIVE_BINDING] = native;
  try {
    return await callback();
  } finally {
    if (previous === undefined) {
      delete globalThis[TEST_NATIVE_BINDING];
    } else {
      globalThis[TEST_NATIVE_BINDING] = previous;
    }
  }
}

test("mutable global bindings cannot authorize Exact12 native admission", () => {
  let fakeCalls = 0;
  const fake = fakeNative(manifestPayload(), {
    privacyCompiledProfileCatalogV1: () => {
      fakeCalls += 1;
      return Uint8Array.from(CATALOG);
    },
  });
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = fake;
  try {
    try {
      productionPrivacyCapabilities.compiledProfileCatalogV1();
    } catch (error) {
      assert.ok(error instanceof Error);
    }
    assert.equal(fakeCalls, 0, "mutable global binding was consulted");
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("browser Exact12 exports fail closed even when a fake global binding exists", () => {
  let fakeCalls = 0;
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = fakeNative(manifestPayload(), {
    privacyCompiledProfileCatalogV1: () => {
      fakeCalls += 1;
      return Uint8Array.from(CATALOG);
    },
  });
  try {
    assert.throws(
      () => browserSubject.compiledProfileCatalogV1(),
      /no browser or mock fallback is permitted/u,
    );
    assert.equal(fakeCalls, 0, "browser facade consulted a mutable global binding");
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
});

test("canonical decoder preserves immutable bytes and the closed Exact12 mapping", async () => {
  await withNative(fakeNative(), () => {
    const input = Uint8Array.from(ARCHIVE);
    const manifest = decodePrivacyExact12CapabilityManifestV1(input);
    input[0] ^= 0xff;
    assert.ok(manifest instanceof PrivacyExact12CapabilityManifestV1);
    assert.equal(manifest.version, 1);
    assert.equal(manifest.committed_height, 42n);
    assert.equal(manifest.network_id.literal, TEST_LOCAL_SIGNING_CONTEXT.networkId.literal);
    assert.equal(Object.hasOwn(manifest, "qualification"), false);
    const networkBytes = manifest.network_id.toBytes();
    networkBytes[0] ^= 1;
    assert.equal(manifest.network_id.literal, TEST_LOCAL_SIGNING_CONTEXT.networkId.literal);
    assert.deepEqual(manifest.canonicalBytes(), ARCHIVE);
    const returned = manifest.canonicalBytes();
    returned[0] ^= 0xff;
    assert.deepEqual(manifest.canonicalBytes(), ARCHIVE);
    assert.equal(Object.isFrozen(manifest), true);
    assert.equal(Object.isFrozen(manifest.protocols), true);
    assert.deepEqual(
      manifest.protocols.map((row) => [
        row.protocol_id.protocol,
        row.operation_schema.operation_schema,
        row.execution_mode.execution_mode,
        row.privacy_feature_mask,
      ]),
      PRIVACY_PROTOCOL_IDS_V1.map((protocol, index) => [
        protocol,
        ...OPERATION_TUPLES[index],
      ]),
    );
    assert.deepEqual(Object.keys(manifest.protocols[0]), [
      "protocol_id",
      "operation_schema",
      "execution_mode",
      "privacy_feature_mask",
      "compiled_profile",
      "readiness",
      "policy",
    ]);
    assert.deepEqual(manifest.protocols[0].readiness, {
      readiness: "unavailable",
      detail: {
        reason: "compiled-profile",
        detail: { reason: "engine-unavailable", detail: null },
      },
    });
    assert.deepEqual(manifest.protocols[1].readiness, {
      readiness: "available",
      detail: null,
    });
  });
});

test("privacy consensus maxima match Rust transport fixtures and reject one over", async () => {
  const limits = JSON.parse(readFileSync(new URL(
    "../../../fixtures/zk/signed_attachments_v1/vectors.json", import.meta.url,
  ), "utf8")).privacy_frame_limits;
  await withNative(fakeNative(), () => {
    const policy = decodePrivacyExact12CapabilityManifestV1(ARCHIVE).consensus_policy.current_limits;
    assert.equal(policy.max_proof_bytes_per_action, 9_437_184);
    assert.equal(policy.max_action_bytes, limits.envelope_bytes);
    assert.equal(policy.max_privacy_bytes_per_transaction, limits.envelope_bytes);
    assert.equal(policy.max_privacy_bytes_per_block, limits.block_privacy_bytes);
  });
  for (const key of ["max_action_bytes", "max_privacy_bytes_per_transaction", "max_privacy_bytes_per_block"]) {
    const payload = manifestPayload();
    payload.consensus_policy.current_limits[key] += 1;
    await withNative(fakeNative(payload), () => {
      assert.throws(() => decodePrivacyExact12CapabilityManifestV1(ARCHIVE), /hard maximum/u);
    });
  }
});

test("only authenticated transport grants admission; archived available rows remain inspection-only", async () => {
  await withNative(fakeNative(manifestPayload()), async () => {
    const archived = decodePrivacyExact12CapabilityManifestV1(ARCHIVE);
    assert.throws(() => requirePrivacyExact12CapabilityAdmissionV1(archived, ACTIVE_PROTOCOL), /authenticated Torii-origin/u);
    const manifest = await fetchTestManifest();
    assert.equal(Object.isFrozen(manifest.network_id), true);
    assert.equal(Object.hasOwn(manifest, "qualification"), false);
    assert.deepEqual(manifest.protocols[1].readiness, {
      readiness: "available",
      detail: null,
    });
    const admission = requirePrivacyExact12CapabilityAdmissionV1(
      manifest,
      ACTIVE_PROTOCOL,
    );
    assert.equal(admission.readiness, "available");
    assert.equal(admission.protocol_id, ACTIVE_PROTOCOL);
    assert.deepEqual(admission.network_id, Array.from(TEST_LOCAL_SIGNING_CONTEXT.networkId.toBytes()));
    assert.equal(admission.torii_origin, "https://privacy.example.test");
    const replayedArchive = decodePrivacyExact12CapabilityManifestV1(manifest.canonicalBytes());
    assert.throws(() => requirePrivacyExact12CapabilityAdmissionV1(replayedArchive, ACTIVE_PROTOCOL), /authenticated Torii-origin/u);
  });
});

test("committed policy protocol drift rejects before native admission", async () => {
  const payload = manifestPayload();
  payload.protocols[1].policy.protocol_id = tagged(PRIVACY_PROTOCOL_IDS_V1[2]);
  let admissionCalls = 0;
  await withNative(fakeNative(payload, {
    privacyRequireExact12CapabilityTupleV1: () => {
      admissionCalls += 1;
      return true;
    },
  }), async () => {
    await assert.rejects(fetchTestManifest(), /row protocol/u);
    assert.equal(admissionCalls, 0);
  });
});

test("native canonical status rejects truncation, suffixes, and text shells", async () => {
  await withNative(fakeNative(), () => {
    for (const hostile of [
      ARCHIVE.slice(0, -1),
      Uint8Array.from([...ARCHIVE, 0]),
      new TextEncoder().encode("digest-shell"),
    ]) {
      assert.throws(
        () => decodePrivacyExact12CapabilityManifestV1(hostile),
        PrivacyExact12CapabilityManifestError,
      );
    }
    assert.throws(
      () => decodePrivacyExact12CapabilityManifestV1("NRT0"),
      /must be canonical Norito bytes/u,
    );
  });
});

test("projection substitutions fail even behind a lying JSON projection helper", async () => {
  const cases = [
    (value) => { value.protocols.reverse(); },
    (value) => { value.protocols[0].operation_schema.operation_schema = "pq_masp_note_action_v1"; },
    (value) => { value.protocols[0].privacy_feature_mask = 31; },
    (value) => { value.protocols[0].readiness = { readiness: "available", detail: null }; },
    (value) => { value.protocols[0].readiness = { readiness: "available-experimental", detail: null }; },
    (value) => { value.protocols[0].readiness = { readiness: "production-qualified", detail: null }; },
    (value) => {
      value.protocols[0].readiness = {
        readiness: "unavailable",
        detail: { reason: "engine-unavailable", detail: null },
      };
    },
    (value) => { value.protocols[0].activation_state = { activation_state: "not-registered", detail: null }; },
    (value) => { value.protocols[6].limitation = null; },
    (value) => {
      value.protocols[1].policy.assurance = {
        assurance: "experimental",
        value: null,
      };
      value.protocols[1].policy.production_qualification = null;
    },
    (value) => { value.manifest_digest = Array(32).fill(0); },
    (value) => { value.unknown = true; },
  ];
  for (const mutate of cases) {
    const payload = manifestPayload();
    mutate(payload);
    await withNative(fakeNative(payload), () => {
      assert.throws(
        () => decodePrivacyExact12CapabilityManifestV1(ARCHIVE),
        /Exact12 capability manifest/u,
      );
    });
  }
});

test("retired registration and lifecycle readiness cannot disable a compiled profile", async () => {
  for (const reason of ["not-registered", "proposed", "suspended", "retired"]) {
    const payload = manifestPayload();
    payload.protocols[1].policy = null;
    payload.protocols[1].readiness = { readiness: "unavailable", detail: { reason, detail: null } };
    await withNative(fakeNative(payload), () => {
      assert.throws(() => decodePrivacyExact12CapabilityManifestV1(ARCHIVE), /evidence-derived available/u);
    });
  }
});

test("pending policy update accepts next block and rejects invalid snapshot heights", async () => {
  for (const kind of ["consensus", "protocol"]) {
    for (const [scheduled, effective, committed, accepted] of [
      [42, 43, 42, true],
      [42, 44, 42, true],
      ["18446744073709551614", "18446744073709551615", "18446744073709551614", true],
      [0, 43, 42, false],
      [42, 42, 42, false],
      [42, 41, 42, false],
      [43, 44, 42, false],
      [42, 43, 43, false],
      ["18446744073709551615", "18446744073709551615", "18446744073709551615", false],
      ["18446744073709551615", "18446744073709551616", "18446744073709551615", false],
    ]) {
      const payload = manifestPayload();
      payload.committed_height = committed;
      const pending = { scheduled_at_height: scheduled, effective_at_height: effective };
      if (kind === "consensus") {
        pending.next_limits = { ...payload.consensus_policy.current_limits, retained_root_count: 1024 };
        payload.consensus_policy.pending_update = pending;
      } else {
        const row = payload.protocols[1];
        pending.next_limits = {
          protocol: ACTIVE_PROTOCOL,
          limits: { max_anonymity_set_size: 32, max_recipient_count: 8 },
        };
        row.policy.pending_update = pending;
      }
      // Native JSON carries uint64 values as integer tokens, never quoted strings.
      const native = fakeNative(payload, {
        privacyExact12CapabilityManifestJsonV1: () =>
          JSON.stringify(payload).replace(
            /"(committed_height|scheduled_at_height|effective_at_height)":"([0-9]+)"/gu,
            '"$1":$2',
          ),
      });
      await withNative(native, () => {
        if (accepted) {
          const manifest = decodePrivacyExact12CapabilityManifestV1(ARCHIVE);
          const actual = kind === "consensus"
            ? manifest.consensus_policy.pending_update
            : manifest.protocols[1].policy.pending_update;
          assert.equal(actual.effective_at_height, BigInt(effective));
          assert.throws(() => requirePrivacyExact12CapabilityAdmissionV1(manifest, ACTIVE_PROTOCOL));
        } else {
          assert.throws(() => decodePrivacyExact12CapabilityManifestV1(ARCHIVE), PrivacyExact12CapabilityManifestError);
        }
      });
    }
  }
});

test("compiled availability uses defaults without a registration at every committed height", async () => {
  for (const committedHeight of [1, 42, 4_000]) {
    const payload = manifestPayload();
    payload.committed_height = committedHeight;
    payload.protocols[1].policy = null;
    await withNative(fakeNative(payload), async () => {
      const manifest = await fetchTestManifest();
      assert.equal(manifest.protocols[1].policy, null);
      assert.equal(requirePrivacyExact12CapabilityAdmissionV1(manifest, ACTIVE_PROTOCOL).readiness, "available");
    });
  }
});

test("ordinary policies preserve unavailable relations and reject retired activation layouts", async () => {
  const payload = manifestPayload();
  payload.protocols[5].policy = {
    protocol_id: tagged(PRIVACY_PROTOCOL_IDS_V1[5]),
    current_limits: { protocol: PRIVACY_PROTOCOL_IDS_V1[5], limits: null },
    pending_update: null,
  };
  await withNative(fakeNative(payload), async () => {
    const manifest = await fetchTestManifest();
    assert.equal(manifest.protocols[5].readiness.readiness, "unavailable");
    assert.throws(() => requirePrivacyExact12CapabilityAdmissionV1(manifest, PRIVACY_PROTOCOL_IDS_V1[5]), /not available/u);
  });
  for (const mutate of [
    (p) => { p.protocols[1].activation = null; },
    (p) => { p.protocols[1].policy.lifecycle = { state: "active", record: { proposed_at_height: 1, activated_at_height: 1, state_since_height: 1 } }; },
    (p) => { p.protocols[1].policy.pending_protocol_limits_tightening = null; },
    (p) => { p.consensus_policy.pending_tightening = null; },
  ]) {
    const retired = manifestPayload(); mutate(retired);
    await withNative(fakeNative(retired), () => {
      assert.throws(() => decodePrivacyExact12CapabilityManifestV1(ARCHIVE), PrivacyExact12CapabilityManifestError);
    });
  }
});

test("ordinary updates accept bounded increases and decreases but reject no-ops and ceilings", async () => {
  for (const increase of [false, true]) {
    const payload = manifestPayload();
    payload.consensus_policy.current_limits.retained_root_count = increase ? 1024 : 2048;
    payload.consensus_policy.pending_update = {
      scheduled_at_height: 42, effective_at_height: 43,
      next_limits: { ...payload.consensus_policy.current_limits, retained_root_count: increase ? 2048 : 1024 },
    };
    const policy = payload.protocols[1].policy;
    policy.current_limits.limits.max_recipient_count = increase ? 4 : 8;
    policy.pending_update = {
      scheduled_at_height: 42, effective_at_height: 43,
      next_limits: { protocol: ACTIVE_PROTOCOL, limits: { max_anonymity_set_size: 64, max_recipient_count: increase ? 8 : 4 } },
    };
    await withNative(fakeNative(payload), () => {
      const manifest = decodePrivacyExact12CapabilityManifestV1(ARCHIVE);
      assert.equal(manifest.protocols[1].policy.pending_update.next_limits.limits.max_recipient_count, increase ? 8 : 4);
    });
  }
  for (const kind of ["consensus", "protocol"]) {
    for (const invalid of ["no-op", "ceiling", "protocol", "zero"]) {
      const payload = manifestPayload();
      const policy = kind === "consensus" ? payload.consensus_policy : payload.protocols[1].policy;
      const next = structuredClone(policy.current_limits);
      if (invalid === "ceiling") {
        if (kind === "consensus") next.retained_root_count = 2049;
        else next.limits.max_recipient_count = 9;
      } else if (invalid === "zero") {
        if (kind === "consensus") next.retained_root_count = 0;
        else next.limits.max_recipient_count = 0;
      } else if (invalid === "protocol") {
        if (kind === "consensus") next.max_actions_per_transaction = 2;
        else next.protocol = PRIVACY_PROTOCOL_IDS_V1[2];
      }
      policy.pending_update = { scheduled_at_height: 42, effective_at_height: 43, next_limits: next };
      await withNative(fakeNative(payload), () => {
        assert.throws(() => decodePrivacyExact12CapabilityManifestV1(ARCHIVE), PrivacyExact12CapabilityManifestError);
      });
    }
  }
});

test("selected active protocol admits without a global qualification receipt", async () => {
  let nativeAdmissionCalls = 0;
  await withNative(fakeNative(manifestPayload(), {
    privacyRequireExact12CapabilityTupleV1: (_archive, protocolId, network) => {
      nativeAdmissionCalls += 1;
      assert.equal(protocolId, ACTIVE_PROTOCOL);
      assert.deepEqual(network, TEST_LOCAL_SIGNING_CONTEXT.networkId.toBytes());
      return true;
    },
  }), async () => {
    const manifest = await fetchTestManifest();
    assert.equal(manifest.protocols.filter((row) => row.readiness.readiness === "unavailable").length, 11);
    const admission = requirePrivacyExact12CapabilityAdmissionV1(manifest, ACTIVE_PROTOCOL);
    assert.equal(admission.readiness, "available");
    assert.equal(nativeAdmissionCalls, 1);
    for (const retired of [
      "jindo-lattice-pcs-zk-v0",
      "sis-with-hints",
      "zk-ams-recursive-admission-v0",
    ]) {
      assert.throws(
        () => requirePrivacyExact12CapabilityAdmissionV1(manifest, retired),
        /retained Exact12 identifier/u,
      );
    }
    assert.equal(nativeAdmissionCalls, 1);
  });
});

test("legacy snapshot exports and caller-created shells are rejected", async () => {
  for (const name of [
    "getPrivacyCapabilitiesV1",
    "parsePrivacyCapabilitySnapshotV1",
    "PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1",
    "PrivacyCapabilitySnapshotError",
  ]) {
    assert.equal(Object.hasOwn(authenticatedTestSubject, name), false, name);
  }
  await withNative(fakeNative(), () => {
    assert.throws(
      () => new PrivacyExact12CapabilityManifestV1(),
      /no public constructor/u,
    );
    assert.throws(
      () => requirePrivacyExact12CapabilityAdmissionV1({
        manifest_digest: Array(32).fill(0xa5),
      }, ACTIVE_PROTOCOL),
      /authenticated Torii-origin/u,
    );
  });
});

test("N-API Torii fetch requests exact bounded Norito and browser fallback is absent", async () => {
  await withNative(fakeNative(), async () => {
    const calls = [];
    const node = new ToriiClient("https://privacy.example.test", {
      localSigningContext: TEST_LOCAL_SIGNING_CONTEXT,
      fetchImpl: async (url, init) => {
        calls.push({ url: String(url), init });
        return manifestResponse();
      },
    });
    const manifest = await getPrivacyExact12CapabilityManifestV1(node, {
      canonicalAuth: TEST_CANONICAL_AUTH,
    });
    assert.equal(manifest.committed_height, 42n);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].url, "https://privacy.example.test/v1/privacy/capabilities");
    assert.equal(new Headers(calls[0].init.headers).get("accept"), "application/x-norito");
    assert.equal(calls[0].init.redirect, "error");
    assert.equal(calls[0].init.cache, "no-store");
    assert.equal(new Headers(calls[0].init.headers).get("cache-control"), "no-store");

    const legacyJsonNode = new ToriiClient("https://privacy.example.test", {
      localSigningContext: TEST_LOCAL_SIGNING_CONTEXT,
      fetchImpl: async () => manifestResponse(JSON.stringify({
        version: 1,
        committed_height: 42,
        consensus_policy: consensusPolicy(),
        protocols: [],
      }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    });
    await assert.rejects(
      getPrivacyExact12CapabilityManifestV1(legacyJsonNode, {
        canonicalAuth: TEST_CANONICAL_AUTH,
      }),
      /must use exactly application\/x-norito/u,
    );

    let browserCalls = 0;
    const browser = new ToriiBrowserClient("https://privacy.example.test", {
      fetchImpl: async () => {
        browserCalls += 1;
        throw new Error("must not fetch");
      },
    });
    await assert.rejects(
      getPrivacyExact12CapabilityManifestV1(browser),
      /browser and mock transports cannot authorize privacy/u,
    );
    assert.equal(browserCalls, 0);
  });
});


test("forged symbol transports, other-network archives and insecure response origins reject", async () => {
  await withNative(fakeNative(manifestPayload()), async () => {
    let forgedCalls = 0;
    const forged = {};
    for (const key of Object.getOwnPropertySymbols(ToriiClient.prototype)) {
      forged[key] = async () => { forgedCalls += 1; return ARCHIVE; };
    }
    await assert.rejects(getPrivacyExact12CapabilityManifestV1(forged, { canonicalAuth: TEST_CANONICAL_AUTH }), /N-API Torii client/u);
    assert.equal(forgedCalls, 0);
    for (const [baseUrl, context, metadata, error] of [
      ["http://privacy.example.test", TEST_LOCAL_SIGNING_CONTEXT, {}, /HTTPS/u],
      ["https://privacy.example.test", undefined, {}, /NetworkId|networkId/u],
      ["https://privacy.example.test", new LocalSigningContext(NetworkId.fromBytes(Buffer.alloc(32, 0xd1)), 753), {}, /different network/u],
      ["https://privacy.example.test", TEST_LOCAL_SIGNING_CONTEXT, {url: "https://other.example.test/v1/privacy/capabilities"}, /exact authenticated URL/u],
      ["https://privacy.example.test", TEST_LOCAL_SIGNING_CONTEXT, {redirected: true}, /exact authenticated URL/u],
    ]) {
      const client = new ToriiClient(baseUrl, {localSigningContext: context, fetchImpl: async () => manifestResponse(ARCHIVE, {}, metadata)});
      await assert.rejects(getPrivacyExact12CapabilityManifestV1(client, {canonicalAuth: TEST_CANONICAL_AUTH}), error);
    }
  });
});


test("transport origin never substitutes for native tuple or network validation", async () => {
  for (const result of [false, "true", null]) {
    await withNative(fakeNative(manifestPayload(), {
      privacyRequireExact12CapabilityTupleV1: () => result,
    }), async () => {
      const manifest = await fetchTestManifest();
      assert.throws(() => requirePrivacyExact12CapabilityAdmissionV1(manifest, ACTIVE_PROTOCOL), /sole success value/u);
    });
  }
  await withNative(fakeNative(manifestPayload(), {
    privacyRequireExact12CapabilityTupleV1: () => { throw new Error("native network or tuple mismatch"); },
  }), async () => {
    const manifest = await fetchTestManifest();
    assert.throws(() => requirePrivacyExact12CapabilityAdmissionV1(manifest, ACTIVE_PROTOCOL), /native/u);
  });
});

test("unreadable transport origin is rejected without invoking getters and cancels its body", async () => {
  await withNative(fakeNative(), async () => {
    let getterCalls = 0;
    let cancellations = 0;
    const response = manifestResponse();
    response.body = new ReadableStream({ cancel() { cancellations += 1; } });
    Object.defineProperty(response, "url", {
      get() { getterCalls += 1; throw new Error("untrusted origin getter"); },
    });
    const client = new ToriiClient("https://privacy.example.test", {
      localSigningContext: TEST_LOCAL_SIGNING_CONTEXT,
      fetchImpl: async () => response,
    });
    await assert.rejects(getPrivacyExact12CapabilityManifestV1(client, {
      canonicalAuth: TEST_CANONICAL_AUTH,
    }), /URL must be an own data property/u);
    assert.equal(getterCalls, 0);
    assert.equal(cancellations, 1);
  });
});

test("Exact12 transport ignores mutable client and static normalization overrides", async () => {
  await withNative(fakeNative(manifestPayload()), async () => {
    let fetchCalls = 0;
    let overrideCalls = 0;
    const client = new ToriiClient("https://privacy.example.test", {
      localSigningContext: TEST_LOCAL_SIGNING_CONTEXT,
      fetchImpl: async (_url, init) => {
        fetchCalls += 1;
        assert.ok(Object.keys(init.headers).some((name) => name.toLowerCase() === "x-iroha-signature"));
        return manifestResponse();
      },
    });
    const failOverride = () => { overrideCalls += 1; throw new Error("mutable client override reached"); };
    for (const name of ["_request", "_createHeaders", "_expectStatus", "_getHeader", "_readBoundedResponseBytes"]) {
      client[name] = failOverride;
    }
    const originalAuth = ToriiClient._normalizeCanonicalAuth;
    const originalKey = ToriiClient._normalizePrivateKey;
    ToriiClient._normalizeCanonicalAuth = failOverride;
    ToriiClient._normalizePrivateKey = failOverride;
    try {
      const manifest = await getPrivacyExact12CapabilityManifestV1(client, {
        canonicalAuth: TEST_CANONICAL_AUTH,
      });
      requirePrivacyExact12CapabilityAdmissionV1(manifest, ACTIVE_PROTOCOL);
      assert.equal(fetchCalls, 1);
      assert.equal(overrideCalls, 0);
    } finally {
      ToriiClient._normalizeCanonicalAuth = originalAuth;
      ToriiClient._normalizePrivateKey = originalKey;
    }
  });
});


test("selected protocol binding rejects another proof suite", async () => {
  for (const [key, field, label] of [
    ["proof_system_id", "proof_system", "stark-fri-sha3-384-goldilocks-v1"],
    ["engine_id", "engine", "native-goldilocks-sha3-384-stark-fri-v1"],
  ]) {
    const payload = manifestPayload();
    payload.protocols[1].compiled_profile.value[key][field] = label;
    await withNative(fakeNative(payload), () => {
      assert.throws(() => decodePrivacyExact12CapabilityManifestV1(ARCHIVE), PrivacyExact12CapabilityManifestError);
    });
  }
});

test("retired global qualification layout and readiness are rejected", async () => {
  for (const mutate of [
    (payload) => { payload.qualification = null; },
    (payload) => { delete payload.network_id; },
    (payload) => { payload.protocols[1].readiness = { readiness: "production-qualified", detail: null }; },
    (payload) => { payload.protocols[1].readiness = { readiness: "unavailable", detail: { reason: "missing-production-qualification", detail: null } }; },
  ]) {
    const payload = manifestPayload();
    mutate(payload);
    await withNative(fakeNative(payload), () => {
      assert.throws(() => decodePrivacyExact12CapabilityManifestV1(ARCHIVE), PrivacyExact12CapabilityManifestError);
    });
  }
});

test("network identity requires the exact canonical marked hash literal", async () => {
  for (const value of [null, "fixture-chain", Array(32).fill(0xa5),
    TEST_LOCAL_SIGNING_CONTEXT.networkId.literal.toLowerCase(),
    TEST_LOCAL_SIGNING_CONTEXT.networkId.literal.slice(0, -1) + "X"]) {
    const payload = manifestPayload();
    payload.network_id = value;
    await withNative(fakeNative(payload), () => {
      assert.throws(() => decodePrivacyExact12CapabilityManifestV1(ARCHIVE), PrivacyExact12CapabilityManifestError);
    });
  }
});

test("protocol sequence accepts exactly twelve rows and rejects both adjacent counts", async () => {
  for (const count of [11, 13]) {
    const payload = manifestPayload();
    payload.protocols = count === 11 ? payload.protocols.slice(0, 11) : [...payload.protocols, payload.protocols[0]];
    await withNative(fakeNative(payload), () => {
      assert.throws(() => decodePrivacyExact12CapabilityManifestV1(ARCHIVE), /exactly the 12 canonical protocol rows/u);
    });
  }
});

test("unfinished X509 joint security reduction rejects only its selected operation", async () => {
  const payload = manifestPayload();
  const row = payload.protocols[5];
  const reason = { reason: "joint-security-reduction-incomplete", detail: null };
  row.compiled_profile = { status: "unavailable", value: structuredClone(reason) };
  row.readiness = { readiness: "unavailable", detail: { reason: "compiled-profile", detail: structuredClone(reason) } };
  const calls = [];
  await withNative(fakeNative(payload, {
    privacyRequireExact12CapabilityTupleV1: (_archive, protocolId) => { calls.push(protocolId); return true; },
  }), async () => {
    const manifest = await fetchTestManifest();
    assert.deepEqual(manifest.protocols[5].compiled_profile.value, reason);
    assert.throws(() => requirePrivacyExact12CapabilityAdmissionV1(manifest, PRIVACY_PROTOCOL_IDS_V1[5]), /joint-security-reduction-incomplete/u);
    requirePrivacyExact12CapabilityAdmissionV1(manifest, ACTIVE_PROTOCOL);
    assert.deepEqual(calls, [ACTIVE_PROTOCOL]);
  });
  row.compiled_profile.value.detail = {};
  await withNative(fakeNative(payload), () => {
    assert.throws(() => decodePrivacyExact12CapabilityManifestV1(ARCHIVE), /null detail/u);
  });
});

test("an unrelated remote tuple mismatch leaves the matching selected protocol usable", async () => {
  const payload = manifestPayload();
  const row = payload.protocols[6];
  const profile = {
    ...releaseBinding(PRIVACY_PROTOCOL_IDS_V1[6], 6),
    protocol_limits: { protocol: PRIVACY_PROTOCOL_IDS_V1[6], limits: { max_polynomial_count: 4 } },
  };
  profile.parameter_digest[0] ^= 1;
  row.compiled_profile = { status: "available", value: structuredClone(profile) };
  row.policy = null;
  row.readiness = { readiness: "available", detail: null };
  const calls = [];
  await withNative(fakeNative(payload, {
    privacyRequireExact12CapabilityTupleV1: (_archive, protocolId) => {
      calls.push(protocolId);
      if (protocolId !== ACTIVE_PROTOCOL) throw new Error("selected native tuple mismatch");
      return true;
    },
  }), async () => {
    const manifest = await fetchTestManifest();
    requirePrivacyExact12CapabilityAdmissionV1(manifest, ACTIVE_PROTOCOL);
    assert.deepEqual(calls, [ACTIVE_PROTOCOL]);
    assert.throws(() => requirePrivacyExact12CapabilityAdmissionV1(manifest, PRIVACY_PROTOCOL_IDS_V1[6]), (error) => {
      assert.ok(error instanceof PrivacyExact12CapabilityManifestError);
      assert.equal(error.cause?.message, "selected native tuple mismatch");
      return true;
    });
    assert.deepEqual(calls, [ACTIVE_PROTOCOL, PRIVACY_PROTOCOL_IDS_V1[6]]);
  });
});


test("selected native admission never requires an unrelated local catalog lookup", async () => {
  let calls = 0;
  await withNative(fakeNative(manifestPayload(), {
    privacyCompiledProfileCatalogV1: () => { throw new Error("unrelated catalog must not be queried"); },
    privacyRequireExact12CapabilityTupleV1: () => { calls += 1; return true; },
  }), async () => {
    const manifest = await fetchTestManifest();
    assert.equal(requirePrivacyExact12CapabilityAdmissionV1(manifest, ACTIVE_PROTOCOL).readiness, "available");
    assert.equal(calls, 1);
  });
});

test("privacy admission rejects retired32 and future34 before any catalog call", async () => {
  for (const version of [32, 34]) {
    let catalogCalls = 0;
    await withNative(fakeNative(manifestPayload(), {
      connectNoritoBridgeAbiVersion: () => version,
      privacyCompiledProfileCatalogV1() { catalogCalls += 1; return CATALOG; },
    }), () => {
      assert.throws(() => decodePrivacyExact12CapabilityManifestV1(ARCHIVE), /requires exact ABI33/u);
      assert.equal(catalogCalls, 0);
    });
  }
});
