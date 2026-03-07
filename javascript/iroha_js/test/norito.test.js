import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { noritoEncodeInstruction, noritoDecodeInstruction } from "../src/norito.js";
import { makeNativeTest, noritoRequiredMethods } from "./helpers/native.js";

const test = makeNativeTest(baseTest, { require: noritoRequiredMethods });
const ACCOUNT_ID = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
const CANONICAL_ACCOUNT_EXPECTED =
  "6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9";

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
      id: "rose#wonderland",
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

const MINT_ASSET = {
  Mint: {
    Asset: {
      object: "42",
      destination: `rose##${ACCOUNT_ID}`,
    },
  },
};

const TRANSFER_ASSET = {
  Transfer: {
    Asset: {
      source: `rose##${ACCOUNT_ID}`,
      object: "10",
      destination: ACCOUNT_ID,
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
  const encoded = noritoEncodeInstruction(MINT_ASSET);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, MINT_ASSET);
});

test("norito encode/decode supports transfer asset instructions", () => {
  const encoded = noritoEncodeInstruction(TRANSFER_ASSET);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(decoded, TRANSFER_ASSET);
});

test("noritoDecodeInstruction strips decoded @domain suffix from account ids", () => {
  const fixture = loadInstructionFixture("mint_asset_numeric.json");
  const bytes = Buffer.from(fixture.instruction, "base64");
  const decoded = noritoDecodeInstruction(bytes);
  const accountId = decoded?.Mint?.Asset?.destination?.split?.("##")?.[1];
  assert.equal(accountId, CANONICAL_ACCOUNT_EXPECTED);
  assert.equal(accountId.includes("@"), false);
});

test("noritoDecodeInstruction canonicalizes nested asset account identifiers", () => {
  const fixture = loadInstructionFixture("burn_asset_numeric.json");
  const bytes = Buffer.from(fixture.instruction, "base64");
  const decoded = noritoDecodeInstruction(bytes);
  assert.equal(decoded.Burn.Asset.destination, `rose##${CANONICAL_ACCOUNT_EXPECTED}`);
});

test("noritoDecodeInstruction can return raw JSON string", () => {
  const encoded = noritoEncodeInstruction(REGISTER_DOMAIN);
  const json = noritoDecodeInstruction(encoded, { parseJson: false });
  assert.equal(typeof json, "string");
  const parsed = JSON.parse(json);
  assert.deepEqual(parsed, REGISTER_DOMAIN);
});

test("burn asset fixture matches canonical Norito bytes", () => {
  const fixture = loadInstructionFixture("burn_asset_numeric.json");
  const bytes = Buffer.from(fixture.instruction, "base64");
  const instruction = noritoDecodeInstruction(bytes);
  const expectedHex = bytes.toString("hex");
  assert.equal(typeof expectedHex, "string");
  const encoded = noritoEncodeInstruction(instruction);
  assert.ok(Buffer.isBuffer(encoded));
  const encodedHex = encoded.toString("hex");
  assert.equal(encodedHex, expectedHex);
});

test("burn asset fractional fixture matches canonical Norito bytes", () => {
  const fixture = loadInstructionFixture("burn_asset_fractional.json");
  const bytes = Buffer.from(fixture.instruction, "base64");
  const instruction = noritoDecodeInstruction(bytes);
  const expectedHex = bytes.toString("hex");
  assert.equal(typeof expectedHex, "string");
  const encoded = noritoEncodeInstruction(instruction);
  assert.ok(Buffer.isBuffer(encoded));
  const encodedHex = encoded.toString("hex");
  assert.equal(encodedHex, expectedHex);
});

test("mint asset fixture matches canonical Norito bytes", () => {
  const fixture = loadInstructionFixture("mint_asset_numeric.json");
  const bytes = Buffer.from(fixture.instruction, "base64");
  const instruction = noritoDecodeInstruction(bytes);
  const expectedHex = bytes.toString("hex");
  assert.equal(typeof expectedHex, "string");
  const encoded = noritoEncodeInstruction(instruction);
  assert.ok(Buffer.isBuffer(encoded));
  const encodedHex = encoded.toString("hex");
  assert.equal(encodedHex, expectedHex);
});

test("burn trigger fixture matches canonical Norito bytes", () => {
  const fixture = loadInstructionFixture("burn_trigger_repetitions.json");
  const bytes = Buffer.from(fixture.instruction, "base64");
  const instruction = noritoDecodeInstruction(bytes);
  const expectedHex = bytes.toString("hex");
  assert.equal(typeof expectedHex, "string");
  const encoded = noritoEncodeInstruction(instruction);
  assert.ok(Buffer.isBuffer(encoded));
  const encodedHex = encoded.toString("hex");
  assert.equal(encodedHex, expectedHex);
});
