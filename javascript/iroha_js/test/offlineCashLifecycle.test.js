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

test("offline cash configuration snapshot requires cached issuer key and ABI", () => {
  assert.equal(
    assertOfflineCashConfigurationSnapshotUsable(
      {
        chainId: "00000042",
        assetDefinitionId: "pkr#sbp",
        offlinePaymentsEnabled: true,
        issuerPublicKeyBase64: "issuer-key",
        nativeBridgeAbiVersion: 7,
        createdAtMs: 100,
        expiresAtMs: 1_000,
      },
      { nowMs: 999, requiredNativeBridgeAbiVersion: 7 },
    ),
    true,
  );

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
    " issuer-key",
    "issuer-key ",
    "issuer key",
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
        issuerPublicKeyBase64: "issuer-key",
        nativeBridgeAbiVersion: 7,
      }),
    error =>
      error instanceof OfflineCashConfigurationSnapshotError &&
      error.code === "offline_payments_disabled",
  );

  assert.throws(
    () =>
      assertOfflineCashConfigurationSnapshotUsable({
        chainId: "00000042",
        assetDefinitionId: "pkr#sbp",
        offlinePaymentsEnabled: "false",
        issuerPublicKeyBase64: "issuer-key",
        nativeBridgeAbiVersion: 7,
      }),
    error =>
      error instanceof OfflineCashConfigurationSnapshotError &&
      error.code === "offline_payments_disabled",
  );

  assert.throws(
    () =>
      assertOfflineCashConfigurationSnapshotUsable(
        {
          chainId: "00000042",
          assetDefinitionId: "pkr#sbp",
          offlinePaymentsEnabled: true,
          issuerPublicKeyBase64: "issuer-key",
          nativeBridgeAbiVersion: 6,
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
          issuerPublicKeyBase64: "issuer-key",
          nativeBridgeAbiVersion: "7",
        },
        { nowMs: 200, requiredNativeBridgeAbiVersion: 7 },
      ),
    error =>
      error instanceof OfflineCashConfigurationSnapshotError && error.code === "malformed_snapshot",
  );

  assert.throws(
    () =>
      assertOfflineCashConfigurationSnapshotUsable(
        {
          chainId: "00000042",
          assetDefinitionId: "pkr#sbp",
          offlinePaymentsEnabled: true,
          issuerPublicKeyBase64: "issuer-key",
          nativeBridgeAbiVersion: 7,
        },
        { nowMs: 200, requiredNativeBridgeAbiVersion: "7" },
      ),
    error =>
      error instanceof OfflineCashConfigurationSnapshotError && error.code === "malformed_snapshot",
  );

  assert.throws(
    () =>
      assertOfflineCashConfigurationSnapshotUsable(
        {
          chainId: "00000042",
          assetDefinitionId: "pkr#sbp",
          offlinePaymentsEnabled: true,
          issuerPublicKeyBase64: "issuer-key",
          nativeBridgeAbiVersion: 7,
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
          issuerPublicKeyBase64: "issuer-key",
          nativeBridgeAbiVersion: 7,
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
