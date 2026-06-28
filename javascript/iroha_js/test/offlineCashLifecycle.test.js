import assert from "node:assert/strict";
import test from "node:test";

import {
  KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME,
  KAGEMUSHA_REDEEM_RECURSIVE_INSTRUCTION_WIRE_NAME,
  KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME,
  OfflineCashConfigurationSnapshotError,
  OfflineCashLifecycleController,
  assertOfflineCashConfigurationSnapshotUsable,
  offlineCashAvailableTransportKinds,
} from "../src/index.js";

const ISSUER_PUBLIC_KEY_BASE64 = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";
const ISSUER_PUBLIC_KEY_BASE64URL = "__________________________________________8";
const SHORT_ISSUER_PUBLIC_KEY_BASE64 = "q6urq6urq6urq6urq6urq6urq6urq6urq6urq6urqw";
const LONG_ISSUER_PUBLIC_KEY_BASE64 = "zc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3N";

test("offline cash transport availability hides unsupported NFC", () => {
  assert.deepEqual(
    offlineCashAvailableTransportKinds({
      qrStreaming: true,
      nfc: { supported: false, reason: "missing HCE entitlement" },
      nearby: true,
    }),
    ["qr", "nearby"],
  );
  assert.deepEqual(
    offlineCashAvailableTransportKinds({ qrStreaming: true, nfc: true, nearby: false }),
    ["qr", "nfc"],
  );
  assert.deepEqual(
    offlineCashAvailableTransportKinds({
      qrStreaming: true,
      nfc: { supported: "true" },
      nearby: true,
    }),
    ["qr", "nearby"],
  );
});

test("offline cash lifecycle syncs pending audit receipts before load", async () => {
  const events = [];
  const controller = new OfflineCashLifecycleController({
    wallet: {
      async load(assetDefinitionId, amount) {
        events.push(`load:${assetDefinitionId}:${amount}`);
        return { ok: true };
      },
    },
    auditReceiptSynchronizer: {
      async hasPendingAuditReceipts() {
        events.push("hasPending");
        return true;
      },
      async syncPendingAuditReceipts() {
        events.push("sync");
      },
    },
  });

  assert.deepEqual(await controller.load("pkr#sbp", "10"), { ok: true });
  assert.deepEqual(events, ["hasPending", "sync", "load:pkr#sbp:10"]);
});

test("offline cash lifecycle does not load when audit receipt sync fails", async () => {
  const events = [];
  const controller = new OfflineCashLifecycleController({
    wallet: {
      async load() {
        events.push("load");
        return { ok: true };
      },
    },
    auditReceiptSynchronizer: {
      async hasPendingAuditReceipts() {
        events.push("hasPending");
        return true;
      },
      async syncPendingAuditReceipts() {
        events.push("sync");
        throw new Error("audit sync failed");
      },
    },
  });

  await assert.rejects(() => controller.load("pkr#sbp", "10"), /audit sync failed/);
  assert.deepEqual(events, ["hasPending", "sync"]);
});

test("offline cash configuration snapshot requires cached identity, time, issuer key, and ABI", () => {
  assert.equal(
    assertOfflineCashConfigurationSnapshotUsable(
      {
        chainId: "00000042",
        assetDefinitionId: "pkr#sbp",
        offlinePaymentsEnabled: true,
        issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
        nativeBridgeAbiVersion: 7,
        artifactSetId: "artifact-set",
        circuitId: "kagemusha-recursive-compact-v1",
        createdAtMs: 100,
        expiresAtMs: 1_000,
      },
      { nowMs: 999, requiredNativeBridgeAbiVersion: 7 },
    ),
    true,
  );
  assert.equal(
    assertOfflineCashConfigurationSnapshotUsable(
      {
        chainId: "00000042",
        assetDefinitionId: "pkr#sbp",
        offlinePaymentsEnabled: true,
        issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64URL,
        nativeBridgeAbiVersion: 7,
        artifactSetId: "artifact-set",
        circuitId: "kagemusha-recursive-compact-v1",
        createdAtMs: 100,
        expiresAtMs: 1_000,
      },
      { nowMs: 999, requiredNativeBridgeAbiVersion: 7 },
    ),
    true,
  );

  for (const [fieldName, value] of [
    ["chainId", ""],
    ["chainId", " 00000042"],
    ["chainId", "00000042\n"],
    ["chainId", true],
    ["assetDefinitionId", ""],
    ["assetDefinitionId", "pkr sbp"],
    ["assetDefinitionId", "pkr#sbp\u2603"],
    ["artifactSetId", "artifact set"],
    ["circuitId", "kagemusha-recursive-compact-v1\n"],
  ]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable(
          {
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
            nativeBridgeAbiVersion: 7,
            artifactSetId: "artifact-set",
            circuitId: "kagemusha-recursive-compact-v1",
            createdAtMs: 100,
            [fieldName]: value,
          },
          { nowMs: 200, requiredNativeBridgeAbiVersion: 7 },
        ),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "malformed_snapshot" &&
        error.message.includes(fieldName),
    );
  }

  for (const [fieldName, value] of [
    ["createdAtMs", undefined],
    ["createdAtMs", -1],
    ["createdAtMs", 100.5],
    ["createdAtMs", Number.MAX_SAFE_INTEGER + 1],
    ["createdAtMs", true],
    ["expiresAtMs", -1],
    ["expiresAtMs", 100.5],
    ["expiresAtMs", Number.MAX_SAFE_INTEGER + 1],
    ["expiresAtMs", true],
    ["expiresAtMs", 100],
  ]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable(
          {
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
            nativeBridgeAbiVersion: 7,
            createdAtMs: 100,
            expiresAtMs: 1_000,
            [fieldName]: value,
          },
          { nowMs: 200, requiredNativeBridgeAbiVersion: 7 },
        ),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "malformed_snapshot" &&
        error.message.includes(fieldName),
    );
  }

  for (const nowMs of [-1, 999.5, Number.MAX_SAFE_INTEGER + 1, true]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable(
          {
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
            nativeBridgeAbiVersion: 7,
            createdAtMs: 100,
          },
          { nowMs, requiredNativeBridgeAbiVersion: 7 },
        ),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "malformed_snapshot" &&
        error.message.includes("nowMs"),
    );
  }

  assert.throws(
    () =>
      assertOfflineCashConfigurationSnapshotUsable({
        chainId: "00000042",
        assetDefinitionId: "pkr#sbp",
        offlinePaymentsEnabled: true,
        issuerPublicKeyBase64: " ",
        nativeBridgeAbiVersion: 7,
        createdAtMs: 100,
      }),
    error =>
      error instanceof OfflineCashConfigurationSnapshotError &&
      error.code === "missing_issuer_public_key",
  );

  for (const issuerPublicKeyBase64 of [
    "",
    ` ${ISSUER_PUBLIC_KEY_BASE64}`,
    `${ISSUER_PUBLIC_KEY_BASE64} `,
    "not base64",
    "!!!!",
    `${ISSUER_PUBLIC_KEY_BASE64}=`,
    SHORT_ISSUER_PUBLIC_KEY_BASE64,
    LONG_ISSUER_PUBLIC_KEY_BASE64,
    "issuer-key\n",
    "issuer-key\u2603",
  ]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable({
          chainId: "00000042",
          assetDefinitionId: "pkr#sbp",
          offlinePaymentsEnabled: true,
          issuerPublicKeyBase64,
          nativeBridgeAbiVersion: 7,
          createdAtMs: 100,
        }),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "missing_issuer_public_key",
    );
  }

  assert.throws(
    () =>
      assertOfflineCashConfigurationSnapshotUsable({
        chainId: "00000042",
        assetDefinitionId: "pkr#sbp",
        offlinePaymentsEnabled: false,
        issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
        nativeBridgeAbiVersion: 7,
        createdAtMs: 100,
      }),
    error =>
      error instanceof OfflineCashConfigurationSnapshotError &&
      error.code === "offline_payments_disabled",
  );

  for (const offlinePaymentsEnabled of ["false", "true", 1]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable({
          chainId: "00000042",
          assetDefinitionId: "pkr#sbp",
          offlinePaymentsEnabled,
          issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
          nativeBridgeAbiVersion: 7,
          createdAtMs: 100,
        }),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "offline_payments_disabled",
    );
  }

  assert.throws(
    () =>
      assertOfflineCashConfigurationSnapshotUsable(
        {
          chainId: "00000042",
          assetDefinitionId: "pkr#sbp",
          offlinePaymentsEnabled: true,
          issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
          nativeBridgeAbiVersion: 6,
          createdAtMs: 100,
        },
        { nowMs: 200, requiredNativeBridgeAbiVersion: 7 },
      ),
    error =>
      error instanceof OfflineCashConfigurationSnapshotError &&
      error.code === "unsupported_native_bridge_abi",
  );

  assert.throws(
    () =>
      assertOfflineCashConfigurationSnapshotUsable(
        {
          chainId: "00000042",
          assetDefinitionId: "pkr#sbp",
          offlinePaymentsEnabled: true,
          issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
          nativeBridgeAbiVersion: "7",
          createdAtMs: 100,
        },
        { nowMs: 200, requiredNativeBridgeAbiVersion: 7 },
      ),
    error =>
      error instanceof OfflineCashConfigurationSnapshotError && error.code === "malformed_snapshot",
  );

  for (const nativeBridgeAbiVersion of [0, -1, 7.5]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable(
          {
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
            nativeBridgeAbiVersion,
            createdAtMs: 100,
          },
          { nowMs: 200, requiredNativeBridgeAbiVersion: 7 },
        ),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "malformed_snapshot",
    );
  }

  assert.throws(
    () =>
      assertOfflineCashConfigurationSnapshotUsable(
        {
          chainId: "00000042",
          assetDefinitionId: "pkr#sbp",
          offlinePaymentsEnabled: true,
          issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
          nativeBridgeAbiVersion: 7,
          createdAtMs: 100,
        },
        { nowMs: 200, requiredNativeBridgeAbiVersion: "7" },
      ),
    error =>
      error instanceof OfflineCashConfigurationSnapshotError && error.code === "malformed_snapshot",
  );

  for (const requiredNativeBridgeAbiVersion of [0, -1, 7.5]) {
    assert.throws(
      () =>
        assertOfflineCashConfigurationSnapshotUsable(
          {
            chainId: "00000042",
            assetDefinitionId: "pkr#sbp",
            offlinePaymentsEnabled: true,
            issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
            nativeBridgeAbiVersion: 7,
            createdAtMs: 100,
          },
          { nowMs: 200, requiredNativeBridgeAbiVersion },
        ),
      error =>
        error instanceof OfflineCashConfigurationSnapshotError &&
        error.code === "malformed_snapshot",
    );
  }

  assert.throws(
    () =>
      assertOfflineCashConfigurationSnapshotUsable(
        {
          chainId: "00000042",
          assetDefinitionId: "pkr#sbp",
          offlinePaymentsEnabled: true,
          issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
          nativeBridgeAbiVersion: 7,
          createdAtMs: 100,
          expiresAtMs: 1_000,
        },
        { nowMs: 1_000, requiredNativeBridgeAbiVersion: 7 },
      ),
    error => error instanceof OfflineCashConfigurationSnapshotError && error.code === "expired",
  );

  assert.throws(
    () =>
      assertOfflineCashConfigurationSnapshotUsable(
        {
          chainId: "00000042",
          assetDefinitionId: "pkr#sbp",
          offlinePaymentsEnabled: true,
          issuerPublicKeyBase64: ISSUER_PUBLIC_KEY_BASE64,
          nativeBridgeAbiVersion: 7,
          createdAtMs: 100,
          expiresAtMs: "1000",
        },
        { nowMs: 999, requiredNativeBridgeAbiVersion: 7 },
      ),
    error =>
      error instanceof OfflineCashConfigurationSnapshotError && error.code === "malformed_snapshot",
  );
});

test("kagemusha wire name constants are canonical", () => {
  assert.equal(
    KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME,
    "iroha_data_model::isi::offline::KagemushaTransfer",
  );
  assert.equal(
    KAGEMUSHA_REDEEM_RECURSIVE_INSTRUCTION_WIRE_NAME,
    "iroha_data_model::isi::offline::RedeemKagemushaRecursive",
  );
  assert.equal(
    KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME,
    "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1",
  );
});
