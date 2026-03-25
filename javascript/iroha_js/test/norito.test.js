import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { noritoEncodeInstruction, noritoDecodeInstruction } from "../src/norito.js";
import { makeNativeTest, noritoRequiredMethods } from "./helpers/native.js";

const test = makeNativeTest(baseTest, { require: noritoRequiredMethods });
const ACCOUNT_ID = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";

const REGISTER_DOMAIN = {
  Register: {
    Domain: {
      id: "wonderland",
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
      logo: null,
      metadata: {},
      mintable: "Infinitely",
      spec: { scale: null },
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

test("noritoDecodeInstruction keeps canonical public asset ids without @domain rewrites", () => {
  const bytes = loadInstructionBytes("mint_asset_numeric.json");
  const decoded = noritoDecodeInstruction(bytes);
  const assetId = decoded?.Mint?.Asset?.destination;
  assert.equal(typeof assetId, "string");
  assert.equal(assetId.includes("#"), true);
  assert.equal(assetId.includes("@"), false);
});

test("noritoDecodeInstruction preserves nested public asset identifiers", () => {
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
