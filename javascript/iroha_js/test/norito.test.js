import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  noritoEncodeInstruction,
  noritoDecodeInstruction,
  noritoEncodeMultisigProposeRequest,
} from "../src/norito.js";
import { __resetNativeStateForTests } from "../src/native.js";
import { makeNativeTest, noritoRequiredMethods } from "./helpers/native.js";

const test = makeNativeTest(baseTest, { require: noritoRequiredMethods });
const ACCOUNT_ID = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";

const REGISTER_DOMAIN = {
  Register: {
    Domain: {
      id: "wonderland.sora",
      logo: null,
      metadata: {
        key: "value",
      },
    },
  },
};

const REGISTER_ACCOUNT = {
  Register: {
    Account: {
      id: ACCOUNT_ID,
      label: null,
      uaid: null,
      opaque_ids: [],
      metadata: { nickname: "alice" },
    },
  },
};

const REGISTER_ASSET = {
  Register: {
    AssetDefinition: {
      id: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
      name: "",
      description: null,
      alias: null,
      logo: null,
      metadata: {},
      mintable: "Infinitely",
      spec: { scale: null },
      balance_scope_policy: "Global",
      confidential_policy: {
        mode: "TransparentOnly",
        vk_set_hash: null,
        poseidon_params_id: null,
        pedersen_params_id: null,
        pending_transition: null,
      },
    },
  },
};

const REGISTER_ASSET_WITH_POLICY = {
  Register: {
    AssetDefinition: {
      id: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
      name: "demo",
      description: "Demo settlement PoC asset",
      alias: "demo#settlement.main",
      logo: "sorafs://bafybeigdyrztk/logo/demo.png",
      metadata: {
        purpose: "poc",
      },
      mintable: "Limited(5)",
      spec: { scale: 2 },
      balance_scope_policy: "DataspaceRestricted",
      confidential_policy: {
        mode: "Convertible",
        vk_set_hash:
          "hash:1111111111111111111111111111111111111111111111111111111111111111#4667",
        poseidon_params_id: 7,
        pedersen_params_id: 3,
        pending_transition: {
          new_mode: "ShieldedOnly",
          effective_height: 64,
          previous_mode: "Convertible",
          transition_id:
            "hash:5555555555555555555555555555555555555555555555555555555555555555#2B05",
          conversion_window: 5,
        },
      },
    },
  },
};

const REGISTER_ASSET_HIDDEN_POOL = {
  zk: {
    RegisterAssetHiddenZkPool: {
      pool_id: "boi-private-is-pool",
      storage_asset: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
      asset_set_root: Array.from({ length: 32 }, (_, index) => index + 1),
      vk_transfer: {
        backend: "halo2/ipa",
        name: "asset-hidden-transfer-v1",
      },
    },
  },
};

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..");

function loadInstructionFixture(name) {
  const fixturePath = path.join(repoRoot, "fixtures", "norito_instructions", name);
  const raw = fs.readFileSync(fixturePath, "utf8");
  return JSON.parse(raw);
}

function loadInstructionBytes(name) {
  const fixture = loadInstructionFixture(name);
  return Buffer.from(fixture.instruction, "base64");
}

function loadAssetIdFromFixture(name) {
  const decoded = noritoDecodeInstruction(loadInstructionBytes(name));
  const destination = decoded?.Mint?.Asset?.destination ?? decoded?.Burn?.Asset?.destination;
  if (typeof destination !== "string" || !destination.includes("#")) {
    throw new Error(`fixture ${name} did not decode to canonical public AssetId literal`);
  }
  return destination;
}

function withMissingNativeBinding(callback) {
  const previousNativeDir = process.env.IROHA_JS_NATIVE_DIR;
  process.env.IROHA_JS_NATIVE_DIR = "/definitely/missing/iroha-js-native";
  __resetNativeStateForTests();
  try {
    return callback();
  } finally {
    if (previousNativeDir === undefined) {
      delete process.env.IROHA_JS_NATIVE_DIR;
    } else {
      process.env.IROHA_JS_NATIVE_DIR = previousNativeDir;
    }
    __resetNativeStateForTests();
  }
}

function readU64Length(buffer, offset, label) {
  assert.ok(offset + 8 <= buffer.length, `${label} length prefix is in bounds`);
  const value = buffer.readBigUInt64LE(offset);
  assert.ok(value <= BigInt(Number.MAX_SAFE_INTEGER), `${label} length fits JS number`);
  return { length: Number(value), bytes: 8 };
}

function readCompactLength(buffer, offset, label) {
  let value = 0n;
  let shift = 0n;
  let cursor = offset;
  for (; cursor < buffer.length; cursor += 1) {
    const byte = buffer[cursor];
    value |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) {
      assert.ok(value <= BigInt(Number.MAX_SAFE_INTEGER), `${label} length fits JS number`);
      return { length: Number(value), bytes: cursor + 1 - offset };
    }
    shift += 7n;
  }
  assert.fail(`${label} compact length prefix is unterminated`);
}

function readNoritoFieldPayload(buffer, offset, label, compactLength) {
  const { length, bytes } = compactLength
    ? readCompactLength(buffer, offset, label)
    : readU64Length(buffer, offset, label);
  const start = offset + bytes;
  const end = start + length;
  assert.ok(end <= buffer.length, `${label} payload is in bounds`);
  return { payload: buffer.subarray(start, end), offset: end };
}

function noritoFramePayload(body, label) {
  const buffer = Buffer.from(body);
  assert.equal(buffer.subarray(0, 4).toString("ascii"), "NRT0");
  const { length: payloadLength } = readU64Length(buffer, 23, `${label}.payloadLength`);
  assert.equal(buffer.length, 40 + payloadLength);
  return {
    flags: buffer[39],
    payload: buffer.subarray(40),
  };
}

test("noritoEncodeInstruction returns canonical bytes", () => {
  const encoded = noritoEncodeInstruction(REGISTER_DOMAIN);
  assert.ok(Buffer.isBuffer(encoded));
  assert.ok(encoded.length > 32);
});

test("noritoDecodeInstruction round-trips instruction JSON", () => {
  const encoded = noritoEncodeInstruction(REGISTER_DOMAIN);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, REGISTER_DOMAIN);
});

test("norito encode/decode supports account registration", () => {
  const encoded = noritoEncodeInstruction(REGISTER_ACCOUNT);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, REGISTER_ACCOUNT);
});

test("norito encode/decode supports asset definition registration", () => {
  const encoded = noritoEncodeInstruction(REGISTER_ASSET);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, REGISTER_ASSET);
});

baseTest("pure JS Norito codec supports asset definition registration without native binding", () => {
  withMissingNativeBinding(() => {
    const encoded = noritoEncodeInstruction(REGISTER_ASSET);
    const decoded = noritoDecodeInstruction(encoded);
    assert.deepEqual(decoded, REGISTER_ASSET);
  });
});

baseTest("pure JS Norito asset definition codec preserves policy fields", () => {
  withMissingNativeBinding(() => {
    const encoded = noritoEncodeInstruction(REGISTER_ASSET_WITH_POLICY);
    const decoded = noritoDecodeInstruction(encoded);
    assert.deepEqual(decoded, REGISTER_ASSET_WITH_POLICY);
  });
});

test("native Norito decoder accepts pure JS asset definition frames", () => {
  const encoded = withMissingNativeBinding(() =>
    noritoEncodeInstruction(REGISTER_ASSET_WITH_POLICY),
  );
  assert.deepEqual(noritoDecodeInstruction(encoded), REGISTER_ASSET_WITH_POLICY);
});

baseTest("pure JS Norito codec supports asset-hidden pool registration without native binding", () => {
  withMissingNativeBinding(() => {
    const encoded = noritoEncodeInstruction(REGISTER_ASSET_HIDDEN_POOL);
    const decoded = noritoDecodeInstruction(encoded);
    assert.deepEqual(decoded, REGISTER_ASSET_HIDDEN_POOL);
  });
});

test("native Norito decoder accepts pure JS asset-hidden pool registration frames", () => {
  const encoded = withMissingNativeBinding(() =>
    noritoEncodeInstruction(REGISTER_ASSET_HIDDEN_POOL),
  );
  assert.deepEqual(noritoDecodeInstruction(encoded), REGISTER_ASSET_HIDDEN_POOL);
});

baseTest("pure JS Norito asset definition codec rejects adversarial fields", () => {
  const withAssetPatch = (patch) => ({
    Register: {
      AssetDefinition: {
        ...REGISTER_ASSET.Register.AssetDefinition,
        ...patch,
      },
    },
  });
  withMissingNativeBinding(() => {
    assert.throws(
      () => noritoEncodeInstruction(withAssetPatch({ mintable: "Limited(0)" })),
      /positive unsigned 32-bit integer/,
    );
    assert.throws(
      () =>
        noritoEncodeInstruction(
          withAssetPatch({ balance_scope_policy: "ObserverScoped" }),
        ),
      /Global or DataspaceRestricted/,
    );
    assert.throws(
      () =>
        noritoEncodeInstruction(
          withAssetPatch({
            confidential_policy: {
              ...REGISTER_ASSET.Register.AssetDefinition.confidential_policy,
              mode: "Mixed",
            },
          }),
        ),
      /TransparentOnly, ShieldedOnly, or Convertible/,
    );
    assert.throws(
      () => noritoEncodeInstruction(withAssetPatch({ logo: "https://example.invalid/logo.png" })),
      /sorafs:\/\/ URI/,
    );
    assert.throws(
      () =>
        noritoEncodeInstruction(
          withAssetPatch({ id: "62Fk4FPcMuLvW5QjDGNF2a4jAmxM" }),
        ),
      /checksum is invalid/,
    );
  });
});

test("norito encode/decode supports mint asset instructions", () => {
  const instruction = {
    Mint: {
      Asset: {
        object: "42",
        destination: loadAssetIdFromFixture("mint_asset_numeric.json"),
      },
    },
  };
  const encoded = noritoEncodeInstruction(instruction);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, instruction);
});

test("norito encode/decode supports transfer asset instructions", () => {
  const instruction = {
    Transfer: {
      Asset: {
        source: loadAssetIdFromFixture("mint_asset_numeric.json"),
        object: "10",
        destination: ACCOUNT_ID,
      },
    },
  };
  const encoded = noritoEncodeInstruction(instruction);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, instruction);
});

baseTest("noritoEncodeInstruction uses the pure JS codec for supported instruction JSON", () => {
  const instruction = {
    Transfer: {
      Asset: {
        source: loadAssetIdFromFixture("mint_asset_numeric.json"),
        object: "7",
        destination: ACCOUNT_ID,
      },
    },
  };
  let encoded;
  withMissingNativeBinding(() => {
    encoded = Buffer.from(noritoEncodeInstruction(instruction));
  });
  assert.ok(encoded.length > 32);
  assert.deepEqual(noritoDecodeInstruction(encoded), instruction);
});

baseTest("native multisig proposal DTO embeds pure JS instructions with compact inner frames", () => {
  const sourceAssetId = loadAssetIdFromFixture("mint_asset_numeric.json");
  const instruction = {
    Transfer: {
      Asset: {
        source: sourceAssetId,
        object: "7",
        destination: ACCOUNT_ID,
      },
    },
  };
  const request = {
    multisig_account_alias: "cbdc@hbl.sbp",
    signer_account_id: ACCOUNT_ID,
    fee_sponsor: "sponsor@sbp",
    instructions: [instruction],
  };
  const nativeBody = Buffer.from(noritoEncodeMultisigProposeRequest(request));
  const body = withMissingNativeBinding(() =>
    Buffer.from(noritoEncodeMultisigProposeRequest(request)),
  );
  assert.deepEqual(body, nativeBody);

  const outer = noritoFramePayload(body, "MultisigProposeDto");
  const outerUsesCompactLengths = (outer.flags & 0x02) !== 0;
  assert.equal(outerUsesCompactLengths, true);
  let offset = 0;
  for (const fieldName of [
    "multisig_account_id",
    "multisig_account_alias",
    "signer_account_id",
    "private_key",
    "public_key_hex",
    "signature_b64",
    "creation_time_ms",
    "fee_sponsor",
  ]) {
    offset = readNoritoFieldPayload(
      outer.payload,
      offset,
      `MultisigProposeDto.${fieldName}`,
      outerUsesCompactLengths,
    ).offset;
  }
  const instructions = readNoritoFieldPayload(
    outer.payload,
    offset,
    "MultisigProposeDto.instructions",
    outerUsesCompactLengths,
  );
  const count = readU64Length(instructions.payload, 0, "MultisigProposeDto.instructions.count");
  assert.equal(count.length, 1);
  const firstInstruction = readNoritoFieldPayload(
    instructions.payload,
    count.bytes,
    "MultisigProposeDto.instructions[0]",
    outerUsesCompactLengths,
  );
  const wireId = readNoritoFieldPayload(
    firstInstruction.payload,
    0,
    "MultisigProposeDto.instructions[0].wire_id",
    outerUsesCompactLengths,
  );
  const wireIdValue = readNoritoFieldPayload(
    wireId.payload,
    0,
    "MultisigProposeDto.instructions[0].wire_id.value",
    outerUsesCompactLengths,
  );
  assert.equal(wireIdValue.payload.toString("utf8"), "iroha.transfer");
  const embeddedFrameField = readNoritoFieldPayload(
    firstInstruction.payload,
    wireId.offset,
    "MultisigProposeDto.instructions[0].payload",
    outerUsesCompactLengths,
  );
  const embeddedFrame = readNoritoFieldPayload(
    embeddedFrameField.payload,
    0,
    "MultisigProposeDto.instructions[0].payload.frame",
    false,
  );
  const inner = noritoFramePayload(
    embeddedFrame.payload,
    "MultisigProposeDto.instructions[0].payload.frame",
  );
  assert.equal((inner.flags & 0x02) !== 0, true);
});

test("native multisig proposal DTO preserves native instruction frames without JS schema entries", () => {
  const request = {
    multisig_account_alias: "cbdc@hbl.sbp",
    signer_account_id: ACCOUNT_ID,
    instructions: [
      {
        Unregister: {
          Domain: "wonderland.sora",
        },
      },
    ],
  };

  const body = Buffer.from(noritoEncodeMultisigProposeRequest(request));
  assert.ok(body.length > 32);
});

baseTest("noritoEncodeInstruction requires native binding for unsupported instruction JSON", () => {
  const instruction = {
    Log: {
      level: "INFO",
      message: "unsupported by the pure JS fallback",
    },
  };
  withMissingNativeBinding(() => {
    assert.throws(
      () => noritoEncodeInstruction(instruction),
      /Native binding required/,
    );
  });
});

baseTest("noritoDecodeInstruction requires native binding for canonical bytes", () => {
  const bytes = loadInstructionBytes("mint_asset_numeric.json");
  withMissingNativeBinding(() => {
    assert.throws(
      () => noritoDecodeInstruction(bytes),
      /Native binding required/,
    );
  });
});

baseTest("noritoEncodeInstruction passes pre-encoded payloads through without native binding", () => {
  const payload = Buffer.from([1, 2, 3, 4]);
  withMissingNativeBinding(() => {
    assert.strictEqual(noritoEncodeInstruction(payload), payload);
    assert.deepEqual(noritoEncodeInstruction(payload.toString("base64")), payload);
    assert.deepEqual(noritoEncodeInstruction(`0x${payload.toString("hex")}`), payload);
  });
});

test("norito encode/decode supports ExecuteTrigger instructions", () => {
  const instruction = {
    ExecuteTrigger: {
      trigger: "mint_request_hbl",
      args: {
        action: "create",
        request_id: "mr1",
      },
    },
  };
  const encoded = noritoEncodeInstruction(instruction);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, instruction);
});

test("noritoDecodeInstruction keeps canonical asset-holding ids without @domain rewrites", () => {
  const bytes = loadInstructionBytes("mint_asset_numeric.json");
  const decoded = noritoDecodeInstruction(bytes);
  const assetId = decoded?.Mint?.Asset?.destination;
  assert.equal(typeof assetId, "string");
  assert.equal(assetId.includes("#"), true);
  assert.equal(assetId.includes("@"), false);
});

test("noritoDecodeInstruction preserves nested asset-holding identifiers", () => {
  const bytes = loadInstructionBytes("burn_asset_numeric.json");
  const decoded = noritoDecodeInstruction(bytes);
  const assetId = decoded?.Burn?.Asset?.destination;
  assert.equal(typeof assetId, "string");
  assert.equal(assetId.includes("#"), true);
  assert.equal(assetId.includes("@"), false);
});

test("noritoDecodeInstruction can return raw JSON string", () => {
  const encoded = noritoEncodeInstruction(REGISTER_DOMAIN);
  const json = noritoDecodeInstruction(encoded, { parseJson: false });
  assert.equal(typeof json, "string");
  const parsed = JSON.parse(json);
  assert.deepEqual(parsed, REGISTER_DOMAIN);
});

test("burn asset fixture matches canonical Norito bytes", () => {
  const bytes = loadInstructionBytes("burn_asset_numeric.json");
  const instruction = noritoDecodeInstruction(bytes);
  const expectedHex = bytes.toString("hex");
  assert.equal(typeof expectedHex, "string");
  const encoded = noritoEncodeInstruction(instruction);
  assert.ok(Buffer.isBuffer(encoded));
  const encodedHex = encoded.toString("hex");
  assert.equal(encodedHex, expectedHex);
});

test("burn asset fractional fixture matches canonical Norito bytes", () => {
  const bytes = loadInstructionBytes("burn_asset_fractional.json");
  const instruction = noritoDecodeInstruction(bytes);
  const expectedHex = bytes.toString("hex");
  assert.equal(typeof expectedHex, "string");
  const encoded = noritoEncodeInstruction(instruction);
  assert.ok(Buffer.isBuffer(encoded));
  const encodedHex = encoded.toString("hex");
  assert.equal(encodedHex, expectedHex);
});

test("mint asset fixture matches canonical Norito bytes", () => {
  const bytes = loadInstructionBytes("mint_asset_numeric.json");
  const instruction = noritoDecodeInstruction(bytes);
  const expectedHex = bytes.toString("hex");
  assert.equal(typeof expectedHex, "string");
  const encoded = noritoEncodeInstruction(instruction);
  assert.ok(Buffer.isBuffer(encoded));
  const encodedHex = encoded.toString("hex");
  assert.equal(encodedHex, expectedHex);
});

test("burn trigger fixture matches canonical Norito bytes", () => {
  const bytes = loadInstructionBytes("burn_trigger_repetitions.json");
  const instruction = noritoDecodeInstruction(bytes);
  const expectedHex = bytes.toString("hex");
  assert.equal(typeof expectedHex, "string");
  const encoded = noritoEncodeInstruction(instruction);
  assert.ok(Buffer.isBuffer(encoded));
  const encodedHex = encoded.toString("hex");
  assert.equal(encodedHex, expectedHex);
});
