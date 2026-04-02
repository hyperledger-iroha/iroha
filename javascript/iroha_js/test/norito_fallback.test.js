import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { AccountAddress } from "../src/address.js";
import { getNativeBinding } from "../src/native.js";

const ACCOUNT_ID = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB";
const EXECUTE_TRIGGER = {
  ExecuteTrigger: {
    trigger: "staged_mint_request_hbl",
    args: {
      action: "create",
      request_id: "mr1",
    },
  },
};
const EXECUTE_TRIGGER_HEX =
  "4e525430000073c28483bfb728cd73c28483bfb728cd00ca0000000000000077927a80b9602eac001d00000000000000150000000000000069726f68612e657865637574655f747269676765729d0000000000000095000000000000004e5254300000f92cdee1227ffa12f92cdee1227ffa12006d00000000000000eeaab5739f49073d0027000000000000001f0000000000000017000000000000007374616765645f6d696e745f726571756573745f68626c36000000000000002e0000000000000026000000000000007b22616374696f6e223a22637265617465222c22726571756573745f6964223a226d7231227d";
const MULTISIG_CANONICAL_HEX =
  "0x0a010100030003010001002068f4b6017d0f876a55c80a82b8388a54aad264d367269e2de8be079c935b5f9601000100207ea0e3bd52e207c9d3b0eba65c0704e66fca2d8e165a175218b174fc4160e4130100020020884b8857f4eaa1613c61504db34d4beaf346517a0e31de3cddd4d9b4201d9d0b";
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..");
const nativeBinding = getNativeBinding();

function hexToBytes(hex) {
  return Buffer.from(hex.replace(/^0x/i, ""), "hex");
}

function loadInstructionBytes(name) {
  const fixturePath = path.join(repoRoot, "fixtures", "norito_instructions", name);
  const raw = JSON.parse(fs.readFileSync(fixturePath, "utf8"));
  return Buffer.from(raw.instruction, "base64");
}

function withDisabledNative(fn) {
  const previous = process.env.IROHA_JS_DISABLE_NATIVE;
  process.env.IROHA_JS_DISABLE_NATIVE = "1";
  return Promise.resolve()
    .then(() => fn())
    .finally(() => {
      if (previous === undefined) {
        delete process.env.IROHA_JS_DISABLE_NATIVE;
      } else {
        process.env.IROHA_JS_DISABLE_NATIVE = previous;
      }
    });
}

test("JS-only norito fallback emits canonical ExecuteTrigger bytes", async () => {
  await withDisabledNative(async () => {
    const mod = await import("../src/norito.js?js-fallback-execute-trigger");
    const encoded = mod.noritoEncodeInstruction(EXECUTE_TRIGGER);
    assert.equal(encoded.toString("hex"), EXECUTE_TRIGGER_HEX);
    assert.deepEqual(mod.noritoDecodeInstruction(encoded), EXECUTE_TRIGGER);
  });
});

test("JS-only norito fallback matches canonical mint/burn fixtures", async () => {
  await withDisabledNative(async () => {
    const mod = await import("../src/norito.js?js-fallback-fixtures");
    for (const name of [
      "mint_asset_numeric.json",
      "burn_asset_numeric.json",
      "burn_asset_fractional.json",
      "burn_trigger_repetitions.json",
    ]) {
      const bytes = loadInstructionBytes(name);
      const nativeCanonical = nativeBinding.noritoEncodeInstruction(
        nativeBinding.noritoDecodeInstruction(bytes),
      );
      const decoded = mod.noritoDecodeInstruction(bytes);
      const reencoded = mod.noritoEncodeInstruction(decoded);
      assert.equal(reencoded.toString("hex"), nativeCanonical.toString("hex"), name);
    }
  });
});

test("JS-only norito fallback round-trips transfer asset instructions", async () => {
  await withDisabledNative(async () => {
    const mod = await import("../src/norito.js?js-fallback-transfer");
    const instruction = {
      Transfer: {
        Asset: {
          source: mod.noritoDecodeInstruction(loadInstructionBytes("mint_asset_numeric.json")).Mint
            .Asset.destination,
          object: "10",
          destination: ACCOUNT_ID,
        },
      },
    };
    const encoded = mod.noritoEncodeInstruction(instruction);
    assert.deepEqual(mod.noritoDecodeInstruction(encoded), instruction);
  });
});

test("JS-only norito fallback matches native bytes for a multisig-controlled account id", async () => {
  const multisigAccountId = AccountAddress.fromCanonicalBytes(
    hexToBytes(MULTISIG_CANONICAL_HEX),
  ).toI105();
  const instruction = {
    Transfer: {
      Asset: {
        source: JSON.parse(nativeBinding.noritoDecodeInstruction(loadInstructionBytes("mint_asset_numeric.json"))).Mint
          .Asset.destination,
        object: "7",
        destination: multisigAccountId,
      },
    },
  };
  const nativeEncoded = nativeBinding.noritoEncodeInstruction(JSON.stringify(instruction));

  await withDisabledNative(async () => {
    const jsMod = await import("../src/norito.js?js-fallback-multisig-transfer");
    const jsEncoded = jsMod.noritoEncodeInstruction(instruction);
    assert.equal(jsEncoded.toString("hex"), nativeEncoded.toString("hex"));
    assert.deepEqual(jsMod.noritoDecodeInstruction(jsEncoded), instruction);
  });
});

test("norito encode accepts base64 payloads in JS mode", async () => {
  await withDisabledNative(async () => {
    const mod = await import("../src/norito.js?js-fallback-base64");
    const payload = loadInstructionBytes("mint_asset_numeric.json");
    const base64 = payload.toString("base64");
    const encoded = mod.noritoEncodeInstruction(base64);
    assert.equal(encoded.toString("base64"), base64);
  });
});

test("norito encode rejects unsupported instruction shapes in JS mode", async () => {
  await withDisabledNative(async () => {
    const mod = await import("../src/norito.js?js-fallback-unsupported");
    assert.throws(
      () =>
        mod.noritoEncodeInstruction({
          Unregister: {
            Domain: {
              object: "wonderland",
            },
          },
        }),
      /Pure JS Norito encoding supports/,
    );
  });
});

test("norito decode surfaces hint when bytes are not canonical Norito in JS mode", async () => {
  await withDisabledNative(async () => {
    const mod = await import("../src/norito.js?js-fallback-error");
    assert.throws(
      () => mod.noritoDecodeInstruction(Buffer.from([0xde, 0xad])),
      /Run `npm run build:native`/,
    );
  });
});
