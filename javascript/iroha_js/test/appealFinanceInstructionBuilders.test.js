import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import {
  buildCancelAssetLockInstruction,
  buildSetAssetTransferAvailabilityInstruction,
  CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1,
} from "../src/instructionBuilders.js";
import { blake2b256 } from "../src/blake2b.js";
import {
  noritoDecodeInstruction,
  noritoEncodeInstruction,
} from "../src/norito.js";
import { AccountAddress } from "../src/address.js";
import {
  makeNativeTest,
  nativeBinding,
  noritoRequiredMethods,
} from "./helpers/native.js";
import {
  assertNativeAndPureInstructionParity,
  normalizedHashHex,
  toByteArray,
  withPureJsInstructionCodec,
} from "./helpers/instructionCodec.js";

const test = makeNativeTest(baseTest, { require: noritoRequiredMethods });
const SORA_I105_DISCRIMINANT = 0x2f1;
const ACCOUNT_SIGNATORY =
  "ED0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
const ACCOUNT_PUBLIC_KEY = Buffer.from(ACCOUNT_SIGNATORY.slice(6), "hex");
const ACCOUNT_ID = AccountAddress.fromAccount({
  publicKey: ACCOUNT_PUBLIC_KEY,
}).toI105(SORA_I105_DISCRIMINANT);
const ASSET_DEFINITION_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const CANCEL_ASSET_LOCK_ESCROW_ID =
  "hash:996264C84790C64086AAB0EF693A1D33EC18FC0B1C1229774C461A00939A6687#F2BD";

baseTest("buildCancelAssetLockInstruction emits the exact two-field V1 payload", () => {
  const instruction = buildCancelAssetLockInstruction({
    lockId: "merchant-lock-001",
    expectedRemainingAmount: "1500",
  });
  assert.deepEqual(instruction, {
    CancelAssetLock: {
      escrow_id: CANCEL_ASSET_LOCK_ESCROW_ID,
      expected_remaining_amount: "1500",
    },
  });
  assert.equal(
    instruction.CancelAssetLock.escrow_id,
    normalizedHashHex(blake2b256(Buffer.from("merchant-lock-001", "utf8"))),
  );
});

baseTest("buildSetAssetTransferAvailabilityInstruction emits exact CAS state", () => {
  assert.deepEqual(
    buildSetAssetTransferAvailabilityInstruction({
      accountId: ACCOUNT_ID,
      assetDefinitionId: ASSET_DEFINITION_ID,
      expectedRevision: 7,
      incoming: "Disabled",
      outgoing: "Enabled",
      reason: "suspend incoming retail transfers",
    }),
    {
      SetAssetTransferAvailability: {
        account_id: ACCOUNT_ID,
        asset_definition_id: ASSET_DEFINITION_ID,
        expected_revision: "7",
        incoming: "Disabled",
        outgoing: "Enabled",
        reason: "suspend incoming retail transfers",
      },
    },
  );
  assert.equal(
    buildSetAssetTransferAvailabilityInstruction({
      accountId: ACCOUNT_ID,
      assetDefinitionId: ASSET_DEFINITION_ID,
      expectedRevision: 0n,
      incoming: "Enabled",
      outgoing: "Enabled",
    }).SetAssetTransferAvailability.reason,
    null,
  );
});

baseTest("asset availability builder rejects ambiguous or noncanonical input", () => {
  const valid = {
    accountId: ACCOUNT_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    expectedRevision: 0,
    incoming: "Enabled",
    outgoing: "Disabled",
  };
  for (const [field, value] of [
    ["incoming", "enabled"],
    ["outgoing", "Frozen"],
    ["expectedRevision", -1],
    ["reason", ""],
    ["reason", " padded"],
    ["reason", "line\u000abreached"],
    ["reason", "ר".repeat(257)],
    ["accountId", ` ${ACCOUNT_ID}`],
    ["assetDefinitionId", `${ASSET_DEFINITION_ID} `],
  ]) {
    assert.throws(
      () =>
        buildSetAssetTransferAvailabilityInstruction({
          ...valid,
          [field]: value,
        }),
      undefined,
      `accepted invalid ${field}`,
    );
  }
  assert.throws(
    () =>
      buildSetAssetTransferAvailabilityInstruction({
        ...valid,
        expected_revision: 0,
      }),
    /not supported/u,
  );
});

baseTest("pure JS codec roundtrips directional asset availability", () => {
  const instruction = buildSetAssetTransferAvailabilityInstruction({
    accountId: ACCOUNT_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    expectedRevision: 3,
    incoming: "Disabled",
    outgoing: "Enabled",
    reason: "operator review",
  });
  withPureJsInstructionCodec(() => {
    const encoded = noritoEncodeInstruction(instruction);
    assert.deepEqual(noritoDecodeInstruction(encoded), instruction);
  });
});

baseTest("asset availability preserves the complete u64 revision domain", () => {
  const instruction = buildSetAssetTransferAvailabilityInstruction({
    accountId: ACCOUNT_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    expectedRevision: 0xffff_ffff_ffff_ffffn,
    incoming: "Enabled",
    outgoing: "Disabled",
  });
  assert.equal(
    instruction.SetAssetTransferAvailability.expected_revision,
    "18446744073709551615",
  );
  withPureJsInstructionCodec(() => {
    const encoded = noritoEncodeInstruction(instruction);
    assert.deepEqual(noritoDecodeInstruction(encoded), instruction);
  });
  assert.throws(
    () =>
      buildSetAssetTransferAvailabilityInstruction({
        accountId: ACCOUNT_ID,
        assetDefinitionId: ASSET_DEFINITION_ID,
        expectedRevision: 0x1_0000_0000_0000_0000n,
        incoming: "Enabled",
        outgoing: "Disabled",
      }),
    /unsigned 64-bit/u,
  );
});

baseTest("pure JS codec rejects noncanonical availability reasons", () => {
  const base = buildSetAssetTransferAvailabilityInstruction({
    accountId: ACCOUNT_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    expectedRevision: 0,
    incoming: "Disabled",
    outgoing: "Enabled",
  });
  withPureJsInstructionCodec(() => {
    for (const reason of ["line\u000abreached", "ר".repeat(257)]) {
      assert.throws(
        () =>
          noritoEncodeInstruction({
            SetAssetTransferAvailability: {
              ...base.SetAssetTransferAvailability,
              reason,
            },
          }),
        undefined,
      );
    }
  });
});

test("native and pure JS codecs byte-match for asset availability", () => {
  const instruction = buildSetAssetTransferAvailabilityInstruction({
    accountId: ACCOUNT_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    expectedRevision: 3,
    incoming: "Disabled",
    outgoing: "Enabled",
    reason: "operator review",
  });
  assertNativeAndPureInstructionParity(
    instruction,
    "SetAssetTransferAvailability",
  );
});

baseTest("buildCancelAssetLockInstruction rejects legacy and ambiguous inputs", () => {
  assert.throws(
    () => buildCancelAssetLockInstruction({ lockId: "merchant-lock-001" }),
    /expectedRemainingAmount/,
  );
  assert.throws(
    () =>
      buildCancelAssetLockInstruction({
        lockId: "merchant-lock-001",
        expectedRemainingAmount: "1",
        expected_remaining_amount: "1",
      }),
    /not supported/,
  );
  assert.throws(
    () =>
      buildCancelAssetLockInstruction({
        lockId: "",
        expectedRemainingAmount: "1",
      }),
    /non-empty string/,
  );
  assert.throws(
    () =>
      buildCancelAssetLockInstruction({
        lockId: " merchant-lock-001",
        expectedRemainingAmount: "1",
      }),
    /surrounding whitespace/,
  );
  for (const lockId of ["\uFEFFmerchant-lock-001", "merchant-lock-001\uFEFF"]) {
    assert.throws(
      () =>
        buildCancelAssetLockInstruction({
          lockId,
          expectedRemainingAmount: "1",
        }),
      /surrounding whitespace/,
    );
  }
  for (const lockId of ["\ud800", "\udfff", "merchant\ud800lock"]) {
    assert.throws(
      () =>
        buildCancelAssetLockInstruction({
          lockId,
          expectedRemainingAmount: "1",
        }),
      /unpaired UTF-16 surrogates/u,
    );
  }
  for (const expectedRemainingAmount of [0n, "0", "-1", "01", "1.0", "+1", 1]) {
    assert.throws(
      () =>
        buildCancelAssetLockInstruction({
          lockId: "merchant-lock-001",
          expectedRemainingAmount,
        }),
      undefined,
      `accepted invalid expected remaining amount ${String(expectedRemainingAmount)}`,
    );
  }
});

baseTest("buildCancelAssetLockInstruction bounds the exact UTF-8 lock-id preimage", () => {
  const exactBound = "🔒".repeat(1_024);
  assert.equal(Buffer.byteLength(exactBound, "utf8"), 4_096);
  assert.equal(CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1, 4_096);
  assert.doesNotThrow(() =>
    buildCancelAssetLockInstruction({
      lockId: exactBound,
      expectedRemainingAmount: "1",
    }),
  );

  const overBound = `${exactBound}a`;
  assert.equal(Buffer.byteLength(overBound, "utf8"), 4_097);
  assert.throws(
    () =>
      buildCancelAssetLockInstruction({
        lockId: overBound,
        expectedRemainingAmount: "1",
      }),
    /at most 4096 UTF-8 bytes/u,
  );
});

baseTest("pure JS codec roundtrips CancelAssetLock and rejects the legacy shape", () => {
  withPureJsInstructionCodec(() => {
    const instruction = buildCancelAssetLockInstruction({
      lockId: "merchant-lock-001",
      expectedRemainingAmount: "1.25",
    });
    const encoded = noritoEncodeInstruction(instruction);
    assert.deepEqual(noritoDecodeInstruction(encoded), instruction);

    assert.throws(
      () =>
        noritoEncodeInstruction({
          CancelAssetLock: { escrow_id: instruction.CancelAssetLock.escrow_id },
        }),
      /expected_remaining_amount is required/,
    );
    for (const expected_remaining_amount of ["0", "01", "1.0"]) {
      assert.throws(
        () =>
          noritoEncodeInstruction({
            CancelAssetLock: {
              escrow_id: instruction.CancelAssetLock.escrow_id,
              expected_remaining_amount,
            },
          }),
        undefined,
        `pure JS codec accepted ${expected_remaining_amount}`,
      );
    }
    for (const escrow_id of [
      CANCEL_ASSET_LOCK_ESCROW_ID.slice(5, 69),
      CANCEL_ASSET_LOCK_ESCROW_ID.replace(
        /^hash:([0-9A-F]+)#/u,
        (_, body) => `hash:${body.toLowerCase()}#`,
      ),
      CANCEL_ASSET_LOCK_ESCROW_ID.toLowerCase(),
    ]) {
      assert.throws(
        () =>
          noritoEncodeInstruction({
            CancelAssetLock: {
              ...instruction.CancelAssetLock,
              escrow_id,
            },
          }),
        /canonical uppercase hash/u,
      );
    }
  });
});

test("native and pure JS codecs byte-match and cross-decode CancelAssetLock V1", () => {
  const instruction = buildCancelAssetLockInstruction({
    lockId: "merchant-lock-001",
    expectedRemainingAmount: "1.25",
  });
  assert.equal(
    instruction.CancelAssetLock.escrow_id,
    CANCEL_ASSET_LOCK_ESCROW_ID,
  );

  const pureEncoded = withPureJsInstructionCodec(() =>
    noritoEncodeInstruction(instruction),
  );
  const nativeEncoded = nativeBinding.noritoEncodeInstruction(
    JSON.stringify(instruction),
  );
  assert.deepEqual(toByteArray(pureEncoded), toByteArray(nativeEncoded));

  assert.deepEqual(
    JSON.parse(nativeBinding.noritoDecodeInstruction(pureEncoded)),
    instruction,
  );
  assert.deepEqual(
    withPureJsInstructionCodec(() =>
      noritoDecodeInstruction(nativeEncoded),
    ),
    instruction,
  );

  assert.throws(
    () =>
      nativeBinding.noritoEncodeInstruction(
        JSON.stringify({
          CancelAssetLock: {
            escrow_id: instruction.CancelAssetLock.escrow_id,
          },
        }),
      ),
    /missing field/,
  );
  assert.throws(
    () =>
      nativeBinding.noritoEncodeInstruction(
        JSON.stringify({
          CancelAssetLock: {
            escrow_id: instruction.CancelAssetLock.escrow_id,
            expected_remaining_amount: "0",
          },
        }),
      ),
    /must be positive/,
  );
  for (const escrowId of [
    instruction.CancelAssetLock.escrow_id.slice(5, 69),
    instruction.CancelAssetLock.escrow_id.toLowerCase(),
  ]) {
    assert.throws(
      () =>
        nativeBinding.noritoEncodeInstruction(
          JSON.stringify({
            CancelAssetLock: {
              ...instruction.CancelAssetLock,
              escrow_id: escrowId,
            },
          }),
        ),
      /canonical|hash:|uppercase|checksum/u,
    );
  }
});
