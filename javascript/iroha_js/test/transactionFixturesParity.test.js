import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { blake2b256 } from "../src/blake2b.js";
import { parseStrictLosslessIntegerJson } from "../src/strictLosslessJson.js";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..");

function loadJsonRelative(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  return parseStrictLosslessIntegerJson(
    fs.readFileSync(absolutePath, "utf8"),
    relativePath,
  );
}

const sourceFixtureFields = new Set([
  "authority",
  "chain",
  "creation_time_ms",
  "name",
  "nonce",
  "payload",
  "payload_base64",
  "payload_hash",
  "signed_base64",
  "signed_hash",
  "time_to_live_ms",
]);
const payloadFields = new Set([
  "authority",
  "chain",
  "creation_time_ms",
  "executable",
  "fee_payment",
  "metadata",
  "nonce",
  "time_to_live_ms",
]);
const executableVariants = new Set([
  "Batch",
  "ContractCall",
  "Instructions",
  "Ivm",
]);
const instructionFields = new Set(["payload_base64", "wire_name"]);
const contractCallFields = new Set([
  "arguments",
  "contract_address",
  "entrypoint",
  "expected_code_hash",
]);
const manifestFields = new Set(["fixtures"]);
const manifestFixtureFields = new Set([
  "authority",
  "chain",
  "creation_time_ms",
  "encoded_file",
  "encoded_len",
  "name",
  "nonce",
  "payload_base64",
  "payload_hash",
  "signed_base64",
  "signed_hash",
  "signed_len",
  "time_to_live_ms",
]);

const canonicalManifest = loadJsonRelative(
  "fixtures/norito_rpc/transaction_fixtures.manifest.json",
);
const sourcePayloadFixtures = loadJsonRelative(
  "fixtures/norito_rpc/transaction_payloads.json",
);

function decodeCanonicalBase64(value, context) {
  if (
    typeof value !== "string" ||
    value.length % 4 !== 0 ||
    !/^[A-Za-z0-9+/]*={0,2}$/u.test(value)
  ) {
    throw new Error(`${context} is invalid base64`);
  }
  const decoded = Buffer.from(value, "base64");
  if (decoded.toString("base64") !== value) {
    throw new Error(`${context} is non-canonical base64`);
  }
  return decoded;
}

function requireRecord(value, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${context} must be an object`);
  }
}

function requireExactFields(record, expected, context) {
  requireRecord(record, context);
  const actual = new Set(Object.keys(record));
  const missing = [...expected].filter((field) => !actual.has(field)).sort();
  const unexpected = [...actual]
    .filter((field) => !expected.has(field))
    .sort();
  if (missing.length !== 0 || unexpected.length !== 0) {
    throw new Error(
      `${context} has invalid fields: missing=${JSON.stringify(missing)}, unexpected=${JSON.stringify(unexpected)}`,
    );
  }
}

function validateTransactionMetadata(record, context) {
  if (typeof record.chain !== "string" || record.chain.length === 0) {
    throw new Error(`${context}.chain must be a non-empty string`);
  }
  if (typeof record.authority !== "string" || record.authority.length === 0) {
    throw new Error(`${context}.authority must be a non-empty string`);
  }
  if (
    !Number.isSafeInteger(record.creation_time_ms) ||
    record.creation_time_ms < 0
  ) {
    throw new Error(`${context}.creation_time_ms must be non-negative`);
  }
  requirePositiveFixtureTtl(record, context);
  if (
    record.nonce !== null &&
    (!Number.isSafeInteger(record.nonce) ||
      record.nonce < 1 ||
      record.nonce > 0xffff_ffff)
  ) {
    throw new Error(`${context}.nonce must be null or a non-zero u32`);
  }
}

function validateExecutable(executable, context) {
  requireRecord(executable, context);
  const variants = Object.keys(executable);
  if (variants.length !== 1) {
    throw new Error(`${context} must contain exactly one executable variant`);
  }
  const variant = variants[0];
  if (!executableVariants.has(variant)) {
    throw new Error(`${context} has unknown variant '${variant}'`);
  }
  const body = executable[variant];
  if (variant === "Ivm") {
    decodeCanonicalBase64(body, `${context}.Ivm`);
    return;
  }
  if (variant === "Instructions") {
    if (!Array.isArray(body)) {
      throw new Error(`${context}.Instructions must be an array`);
    }
    for (const [index, instruction] of body.entries()) {
      validateInstruction(instruction, `${context}.Instructions[${index}]`);
    }
    return;
  }
  if (variant === "ContractCall") {
    validateContractCall(body, `${context}.ContractCall`);
    return;
  }
  if (!Array.isArray(body)) {
    throw new Error(`${context}.Batch must be an array`);
  }
  if (body.length === 0) {
    throw new Error(`${context}.Batch must contain at least one item`);
  }
  for (const [index, item] of body.entries()) {
    const itemContext = `${context}.Batch[${index}]`;
    requireRecord(item, itemContext);
    const itemVariants = Object.keys(item);
    if (itemVariants.length !== 1) {
      throw new Error(`${itemContext} must contain exactly one variant`);
    }
    const itemVariant = itemVariants[0];
    if (itemVariant === "Instruction") {
      validateInstruction(item.Instruction, `${itemContext}.Instruction`);
    } else if (itemVariant === "ContractCall") {
      validateContractCall(item.ContractCall, `${itemContext}.ContractCall`);
    } else {
      throw new Error(`${itemContext} has unknown variant '${itemVariant}'`);
    }
  }
}

function validateInstruction(instruction, context) {
  requireExactFields(instruction, instructionFields, context);
  if (
    typeof instruction.wire_name !== "string" ||
    instruction.wire_name.length === 0
  ) {
    throw new Error(`${context}.wire_name must be a non-empty string`);
  }
  const payload = decodeCanonicalBase64(
    instruction.payload_base64,
    `${context}.payload_base64`,
  );
  if (payload.length === 0) {
    throw new Error(`${context}.payload_base64 must encode non-empty bytes`);
  }
}

function validateContractCall(contractCall, context) {
  requireExactFields(contractCall, contractCallFields, context);
  for (const field of [
    "contract_address",
    "expected_code_hash",
    "entrypoint",
  ]) {
    if (
      typeof contractCall[field] !== "string" ||
      contractCall[field].length === 0
    ) {
      throw new Error(`${context}.${field} must be a non-empty string`);
    }
  }
  if (contractCall.arguments === null) {
    return;
  }
  if (
    !Array.isArray(contractCall.arguments) ||
    contractCall.arguments.some(
      (byte) => !Number.isInteger(byte) || byte < 0 || byte > 0xff,
    )
  ) {
    throw new Error(`${context}.arguments must be null or an array of bytes`);
  }
}

function validateSourceFixtureSchema(fixture, context) {
  requireRecord(fixture, context);
  if (Object.prototype.hasOwnProperty.call(fixture, "encoded")) {
    throw new Error(`${context}: encoded alias is retired; use payload_base64`);
  }
  requireExactFields(fixture, sourceFixtureFields, context);
  if (typeof fixture.name !== "string" || fixture.name.length === 0) {
    throw new Error(`${context}.name must be a non-empty string`);
  }
  validateTransactionMetadata(fixture, context);
  requireExactFields(fixture.payload, payloadFields, `${context}.payload`);
  validateTransactionMetadata(fixture.payload, `${context}.payload`);
  validateExecutable(fixture.payload.executable, `${context}.payload.executable`);
  requireRecord(fixture.payload.fee_payment, `${context}.payload.fee_payment`);
  requireRecord(fixture.payload.metadata, `${context}.payload.metadata`);
  for (const field of [
    "authority",
    "chain",
    "creation_time_ms",
    "nonce",
    "time_to_live_ms",
  ]) {
    if (fixture.payload[field] !== fixture[field]) {
      throw new Error(`${context}: payload.${field} must match ${field}`);
    }
  }
}

function validateEncodedFile(name, encodedFile, context) {
  const expected = `${name}.norito`;
  if (encodedFile !== expected) {
    throw new Error(`${context}.encoded_file must be exactly '${expected}'`);
  }
  if (
    name.length === 0 ||
    name === "." ||
    name === ".." ||
    name.includes("/") ||
    name.includes("\\") ||
    path.basename(encodedFile) !== encodedFile
  ) {
    throw new Error(`${context}.encoded_file must not traverse directories`);
  }
}

function validateManifestFixtureSchema(fixture, context) {
  requireExactFields(fixture, manifestFixtureFields, context);
  if (typeof fixture.name !== "string") {
    throw new Error(`${context}.name must be a string`);
  }
  if (typeof fixture.encoded_file !== "string") {
    throw new Error(`${context}.encoded_file must be a string`);
  }
  validateEncodedFile(fixture.name, fixture.encoded_file, context);
  validateTransactionMetadata(fixture, context);
  for (const [field, value] of [
    ["encoded_len", fixture.encoded_len],
    ["creation_time_ms", fixture.creation_time_ms],
    ["signed_len", fixture.signed_len],
  ]) {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new Error(`${context}.${field} must be a non-negative integer`);
    }
  }
}

function selectFixture(fixtures, name) {
  const match = fixtures.find((fixture) => fixture?.name === name);
  if (!match) {
    throw new Error(`Fixture '${name}' is missing`);
  }
  return match;
}

function requirePositiveFixtureTtl(record, context) {
  if (!Object.prototype.hasOwnProperty.call(record, "time_to_live_ms")) {
    throw new Error(`${context}.time_to_live_ms is required`);
  }
  const ttl = record.time_to_live_ms;
  if (!Number.isSafeInteger(ttl)) {
    throw new Error(`${context}.time_to_live_ms must be an integer`);
  }
  if (ttl <= 0) {
    throw new Error(`${context}.time_to_live_ms must be positive`);
  }
  return ttl;
}

function assertUniqueFixtureIdentities(
  fixtures,
  {
    requireEncodedFile = false,
    requirePayload = false,
    rejectEncodedAlias = false,
    schema = null,
  } = {},
) {
  assert.ok(Array.isArray(fixtures), "fixture collection must be an array");
  const names = new Set();
  const encodedFiles = new Set();
  const payloadHashes = new Set();
  const payloadBytesValues = new Set();
  const signedHashes = new Set();
  const signedBytesValues = new Set();
  for (const [index, fixture] of fixtures.entries()) {
    if (schema === "source") {
      validateSourceFixtureSchema(fixture, `source fixture[${index}]`);
    } else if (schema === "manifest") {
      validateManifestFixtureSchema(fixture, `manifest fixture[${index}]`);
    }
    assert.equal(
      typeof fixture?.name,
      "string",
      "fixture name must be a string",
    );
    assert.ok(
      !names.has(fixture.name),
      `duplicate fixture name: ${fixture.name}`,
    );
    names.add(fixture.name);
    if (rejectEncodedAlias) {
      assert.ok(
        !Object.prototype.hasOwnProperty.call(fixture, "encoded"),
        `${fixture.name}: encoded alias is retired; use payload_base64`,
      );
    }
    if (requirePayload) {
      assert.ok(
        fixture.payload !== null &&
          typeof fixture.payload === "object" &&
          !Array.isArray(fixture.payload),
        `${fixture.name}.payload must be an object`,
      );
    }
    const descriptorTtl = requirePositiveFixtureTtl(fixture, fixture.name);
    if (fixture.payload != null) {
      const payloadTtl = requirePositiveFixtureTtl(
        fixture.payload,
        `${fixture.name}.payload`,
      );
      if (payloadTtl !== descriptorTtl) {
        throw new Error(
          `${fixture.name}: top-level and payload time_to_live_ms values must match`,
        );
      }
    }
    if (requireEncodedFile) {
      assert.equal(
        typeof fixture.encoded_file,
        "string",
        `${fixture.name}: encoded_file must be a string`,
      );
      assert.ok(
        !encodedFiles.has(fixture.encoded_file),
        `duplicate fixture encoded_file: ${fixture.encoded_file}`,
      );
      encodedFiles.add(fixture.encoded_file);
    }
    assert.equal(
      typeof fixture.payload_hash,
      "string",
      `${fixture.name}: payload_hash must be a string`,
    );
    assert.equal(
      typeof fixture.payload_base64,
      "string",
      `${fixture.name}: payload_base64 must be a string`,
    );
    assert.equal(
      typeof fixture.signed_hash,
      "string",
      `${fixture.name}: signed_hash must be a string`,
    );
    assert.equal(
      typeof fixture.signed_base64,
      "string",
      `${fixture.name}: signed_base64 must be a string`,
    );
    assert.ok(
      !payloadHashes.has(fixture.payload_hash),
      `duplicate fixture payload_hash: ${fixture.payload_hash}`,
    );
    payloadHashes.add(fixture.payload_hash);
    const payloadBytes = decodeCanonicalBase64(
      fixture.payload_base64,
      `${fixture.name}.payload_base64`,
    );
    const payloadIdentity = payloadBytes.toString("hex");
    assert.ok(
      !payloadBytesValues.has(payloadIdentity),
      `duplicate fixture payload bytes: ${fixture.name}`,
    );
    payloadBytesValues.add(payloadIdentity);
    assert.ok(
      !signedHashes.has(fixture.signed_hash),
      `duplicate fixture signed_hash: ${fixture.signed_hash}`,
    );
    signedHashes.add(fixture.signed_hash);
    const signedBytes = decodeCanonicalBase64(
      fixture.signed_base64,
      `${fixture.name}.signed_base64`,
    );
    const signedIdentity = signedBytes.toString("hex");
    assert.ok(
      !signedBytesValues.has(signedIdentity),
      `duplicate fixture signed bytes: ${fixture.name}`,
    );
    signedBytesValues.add(signedIdentity);
  }
}

function validateLoadedFixtureCollections() {
  requireExactFields(canonicalManifest, manifestFields, "fixture manifest");
  assertUniqueFixtureIdentities(sourcePayloadFixtures, {
    requirePayload: true,
    rejectEncodedAlias: true,
    schema: "source",
  });
  assertUniqueFixtureIdentities(canonicalManifest.fixtures, {
    requireEncodedFile: true,
    schema: "manifest",
  });
  assert.deepEqual(
    sourcePayloadFixtures.map(({ name }) => name).sort(),
    canonicalManifest.fixtures.map(({ name }) => name).sort(),
    "source payloads and manifest must contain exactly the same fixture names",
  );
}

validateLoadedFixtureCollections();

function irohaHashHex(bytes) {
  const digest = Buffer.from(blake2b256(bytes));
  if (digest.length > 0) {
    digest[digest.length - 1] |= 1;
  }
  return digest.toString("hex");
}

function compactLength(value) {
  let remaining = BigInt(value);
  const output = [];
  do {
    let byte = Number(remaining & 0x7fn);
    remaining >>= 7n;
    if (remaining !== 0n) byte |= 0x80;
    output.push(byte);
  } while (remaining !== 0n);
  return Buffer.from(output);
}

function externalTransactionEntrypointHashHex(canonicalPayload) {
  return irohaHashHex(
    Buffer.concat([
      Buffer.alloc(4),
      compactLength(canonicalPayload.length),
      canonicalPayload,
    ]),
  );
}

function makeSourceFixture(name = "alpha") {
  const common = {
    authority: "sorau-example",
    chain: "00000002",
    creation_time_ms: 1,
    nonce: null,
    time_to_live_ms: 100_000,
  };
  return {
    ...common,
    name,
    payload: {
      ...common,
      executable: { Instructions: [] },
      fee_payment: {
        payer: "authority",
        value: { charge_limits: [] },
      },
      metadata: {},
    },
    payload_base64: "AA==",
    payload_hash: "payload-hash",
    signed_base64: "AQ==",
    signed_hash: "signed-hash",
  };
}

function makeManifestFixture(name = "alpha") {
  return {
    authority: "sorau-example",
    chain: "00000002",
    creation_time_ms: 1,
    encoded_file: `${name}.norito`,
    encoded_len: 1,
    name,
    nonce: null,
    payload_base64: "AA==",
    payload_hash: "payload-hash",
    signed_base64: "AQ==",
    signed_hash: "signed-hash",
    signed_len: 1,
    time_to_live_ms: 100_000,
  };
}

test("strict native JSON parsing rejects duplicate object keys", () => {
  assert.throws(
    () =>
      parseStrictLosslessIntegerJson(
        '{"name":"first","na\\u006de":"second"}',
        "duplicate fixture",
      ),
    /duplicate object key "name"/,
  );
  const parsed = parseStrictLosslessIntegerJson(
    '{"nested":{"name":"first"}}',
    "valid fixture",
  );
  assert.equal(parsed.nested.name, "first");
});

test("source descriptors require exact fields and one executable variant", () => {
  const fixture = makeSourceFixture();
  assert.doesNotThrow(() => validateSourceFixtureSchema(fixture, fixture.name));

  assert.throws(
    () =>
      validateSourceFixtureSchema(
        { ...fixture, unexpected: true },
        fixture.name,
      ),
    /unexpected=\["unexpected"\]/,
  );
  assert.throws(
    () =>
      validateSourceFixtureSchema(
        {
          ...fixture,
          payload: { ...fixture.payload, unexpected: true },
        },
        fixture.name,
      ),
    /unexpected=\["unexpected"\]/,
  );
  assert.throws(
    () =>
      validateSourceFixtureSchema(
        {
          ...fixture,
          payload: {
            ...fixture.payload,
            executable: { Instructions: [], ContractCall: {} },
          },
        },
        fixture.name,
      ),
    /exactly one executable variant/,
  );

  const directCall = {
    ...fixture,
    payload: {
      ...fixture.payload,
      executable: {
        ContractCall: {
          contract_address: "tairac1example",
          expected_code_hash: "hash:example",
          entrypoint: "main",
          arguments: [],
        },
      },
    },
  };
  assert.doesNotThrow(() =>
    validateSourceFixtureSchema(directCall, directCall.name),
  );
});

test("executable variant bodies use the exact first-release schema", () => {
  const instruction = {
    wire_name: "iroha.test",
    payload_base64: "AQ==",
  };
  const contractCall = {
    contract_address: "tairac1example",
    expected_code_hash: "hash:example",
    entrypoint: "main",
    arguments: [0, 255],
  };
  for (const executable of [
    { Ivm: "AQ==" },
    { Instructions: [instruction] },
    { ContractCall: { ...contractCall, arguments: null } },
    {
      Batch: [
        { Instruction: instruction },
        { ContractCall: contractCall },
      ],
    },
  ]) {
    assert.doesNotThrow(() => validateExecutable(executable, "executable"));
  }

  for (const [executable, diagnostic] of [
    [{ Ivm: 1 }, /Ivm is invalid base64/],
    [{ Ivm: "YR==" }, /Ivm is non-canonical base64/],
    [{ Instructions: {} }, /Instructions must be an array/],
    [
      { Instructions: [{ ...instruction, unexpected: true }] },
      /unexpected=\["unexpected"\]/,
    ],
    [
      { Instructions: [{ ...instruction, wire_name: "" }] },
      /wire_name must be a non-empty string/,
    ],
    [
      { Instructions: [{ ...instruction, payload_base64: "" }] },
      /payload_base64 must encode non-empty bytes/,
    ],
    [
      { Instructions: [{ ...instruction, payload_base64: "YR==" }] },
      /payload_base64 is non-canonical base64/,
    ],
    [
      { ContractCall: { ...contractCall, unexpected: true } },
      /unexpected=\["unexpected"\]/,
    ],
    [
      {
        ContractCall: {
          contract_address: contractCall.contract_address,
          entrypoint: contractCall.entrypoint,
          arguments: null,
        },
      },
      /missing=\["expected_code_hash"\]/,
    ],
    [
      { ContractCall: { ...contractCall, arguments: [256] } },
      /arguments must be null or an array of bytes/,
    ],
    [{ Batch: [] }, /Batch must contain at least one item/],
    [{ Batch: {} }, /Batch must be an array/],
    [
      {
        Batch: [
          {
            Instruction: instruction,
            ContractCall: contractCall,
          },
        ],
      },
      /must contain exactly one variant/,
    ],
    [
      { Batch: [{ Instruction: { ...instruction, unexpected: true } }] },
      /unexpected=\["unexpected"\]/,
    ],
  ]) {
    assert.throws(() => validateExecutable(executable, "executable"), diagnostic);
  }
});

test("manifest requires exact fields and canonical encoded filenames", () => {
  const fixture = makeManifestFixture();
  assert.doesNotThrow(() =>
    validateManifestFixtureSchema(fixture, fixture.name),
  );
  assert.throws(
    () =>
      validateManifestFixtureSchema(
        { ...fixture, unexpected: true },
        fixture.name,
      ),
    /unexpected=\["unexpected"\]/,
  );
  assert.throws(
    () =>
      validateManifestFixtureSchema(
        { ...fixture, encoded_file: "renamed.norito" },
        fixture.name,
      ),
    /must be exactly 'alpha\.norito'/,
  );
  assert.throws(
    () => {
      const traversing = makeManifestFixture("../alpha");
      validateManifestFixtureSchema(traversing, traversing.name);
    },
    /must not traverse directories/,
  );
  assert.throws(
    () =>
      requireExactFields(
        { fixtures: [], unexpected: true },
        manifestFields,
        "manifest",
      ),
    /unexpected=\["unexpected"\]/,
  );
});

test("fixture collections reject duplicate names and encoded files before lookup", () => {
  const fixture = {
    name: "first",
    time_to_live_ms: 100_000,
    encoded_file: "first.norito",
    payload_hash: "payload-hash",
    payload_base64: "AA==",
    signed_hash: "signed-hash",
    signed_base64: "AQ==",
  };
  assert.throws(
    () => assertUniqueFixtureIdentities([fixture, { ...fixture }]),
    /duplicate fixture name: first/,
  );
  assert.throws(
    () =>
      assertUniqueFixtureIdentities(
        [
          { ...fixture, encoded_file: "shared.norito" },
          { ...fixture, name: "second", encoded_file: "shared.norito" },
        ],
        { requireEncodedFile: true },
      ),
    /duplicate fixture encoded_file: shared\.norito/,
  );
  assert.throws(
    () =>
      assertUniqueFixtureIdentities(
        [
          fixture,
          {
            ...fixture,
            name: "renamed-clone",
            encoded_file: "renamed-clone.norito",
          },
        ],
        { requireEncodedFile: true },
      ),
    /duplicate fixture payload_hash: payload-hash/,
  );
});

test("fixture descriptors require explicit matching positive TTL", () => {
  const fixture = {
    name: "ttl-fixture",
    time_to_live_ms: 100_000,
    payload: { time_to_live_ms: 100_000 },
    payload_hash: "payload-hash",
    payload_base64: "AA==",
    signed_hash: "signed-hash",
    signed_base64: "AQ==",
  };
  assert.doesNotThrow(() => assertUniqueFixtureIdentities([fixture]));

  for (const [invalid, diagnostic] of [
    [null, /time_to_live_ms must be an integer/],
    [0, /time_to_live_ms must be positive/],
  ]) {
    assert.throws(
      () =>
        assertUniqueFixtureIdentities([
          { ...fixture, time_to_live_ms: invalid },
        ]),
      diagnostic,
    );
    assert.throws(
      () =>
        assertUniqueFixtureIdentities([
          { ...fixture, payload: { time_to_live_ms: invalid } },
        ]),
      diagnostic,
    );
  }

  const missingDescriptorTtl = { ...fixture };
  delete missingDescriptorTtl.time_to_live_ms;
  assert.throws(
    () => assertUniqueFixtureIdentities([missingDescriptorTtl]),
    /time_to_live_ms is required/,
  );
  const missingPayloadTtl = { ...fixture, payload: {} };
  assert.throws(
    () => assertUniqueFixtureIdentities([missingPayloadTtl]),
    /time_to_live_ms is required/,
  );
  assert.throws(
    () =>
      assertUniqueFixtureIdentities([
        { ...fixture, payload: { time_to_live_ms: 99_999 } },
      ]),
    /time_to_live_ms values must match/,
  );
});

test("source fixture descriptors require payload fields without encoded alias", () => {
  const fixture = {
    name: "source-fixture",
    time_to_live_ms: 100_000,
    payload: { time_to_live_ms: 100_000 },
    payload_hash: "payload-hash",
    payload_base64: "AA==",
    signed_hash: "signed-hash",
    signed_base64: "AQ==",
  };
  const options = { requirePayload: true, rejectEncodedAlias: true };

  assert.doesNotThrow(() => assertUniqueFixtureIdentities([fixture], options));

  const missingPayload = { ...fixture };
  delete missingPayload.payload;
  assert.throws(
    () => assertUniqueFixtureIdentities([missingPayload], options),
    /payload must be an object/,
  );

  const missingPayloadBase64 = { ...fixture };
  delete missingPayloadBase64.payload_base64;
  assert.throws(
    () => assertUniqueFixtureIdentities([missingPayloadBase64], options),
    /payload_base64 must be a string/,
  );

  assert.throws(
    () =>
      assertUniqueFixtureIdentities(
        [{ ...fixture, encoded: fixture.payload_base64 }],
        options,
      ),
    /encoded alias is retired/,
  );
});

test("fixture collections reject invalid and non-canonical base64", () => {
  for (const encoded of ["YQ!!", "Y Q==", "YQ=", "YQ===", "YR=="]) {
    assert.throws(
      () => decodeCanonicalBase64(encoded, "adversarial fixture"),
      /(?:invalid|non-canonical) base64/,
    );
  }
});

test("fixture base64 hashes match manifest", () => {
  for (const fixture of canonicalManifest.fixtures) {
    const payloadBytes = decodeCanonicalBase64(
      fixture.payload_base64,
      `${fixture.name}.payload_base64`,
    );
    assert.equal(
      payloadBytes.length,
      fixture.encoded_len,
      `${fixture.name}: payload length mismatch`,
    );
    assert.equal(
      irohaHashHex(payloadBytes),
      fixture.payload_hash,
      `${fixture.name}: payload hash mismatch`,
    );

    const signedBytes = decodeCanonicalBase64(
      fixture.signed_base64,
      `${fixture.name}.signed_base64`,
    );
    assert.equal(
      signedBytes.length,
      fixture.signed_len,
      `${fixture.name}: signed length mismatch`,
    );
    assert.equal(
      externalTransactionEntrypointHashHex(payloadBytes),
      fixture.signed_hash,
      `${fixture.name}: signed hash mismatch`,
    );
    assert.notEqual(
      irohaHashHex(signedBytes),
      fixture.signed_hash,
      `${fixture.name}: raw signed bytes must not alias compact External hash`,
    );
  }
});

test("source payload metadata matches canonical manifest metadata", () => {
  for (const fixture of canonicalManifest.fixtures) {
    const sourceFixture = selectFixture(sourcePayloadFixtures, fixture.name);
    assert.equal(
      sourceFixture.payload_base64,
      fixture.payload_base64,
      `${fixture.name}: source payload base64 drifted from manifest`,
    );
    assert.equal(
      sourceFixture.signed_base64,
      fixture.signed_base64,
      `${fixture.name}: signed base64 drifted from manifest`,
    );

    const payload = sourceFixture.payload;
    assert.ok(
      payload,
      `${fixture.name}: source fixture is missing payload metadata`,
    );
    assert.equal(
      payload.chain,
      fixture.chain,
      `${fixture.name}: chain mismatch`,
    );
    assert.equal(
      payload.authority,
      fixture.authority,
      `${fixture.name}: authority mismatch`,
    );
    assert.equal(
      payload.creation_time_ms,
      fixture.creation_time_ms,
      `${fixture.name}: creation_time_ms mismatch`,
    );
    assert.equal(
      payload.time_to_live_ms,
      fixture.time_to_live_ms,
      `${fixture.name}: time_to_live_ms mismatch`,
    );
    assert.equal(
      payload.nonce ?? null,
      fixture.nonce ?? null,
      `${fixture.name}: nonce mismatch`,
    );
  }
});

test("burn_asset fixture retains the expected burn wire payload", () => {
  const burnFixture = selectFixture(sourcePayloadFixtures, "burn_asset");
  const instructions = burnFixture.payload?.executable?.Instructions;
  assert.ok(
    Array.isArray(instructions),
    "burn_asset payload must carry wire instructions",
  );
  assert.equal(
    instructions.length,
    1,
    "burn_asset fixture should contain exactly one instruction",
  );
  assert.equal(instructions[0].wire_name, "iroha.burn");
  assert.equal(typeof instructions[0].payload_base64, "string");
  assert.ok(
    instructions[0].payload_base64.length > 0,
    "burn_asset wire payload must not be empty",
  );
});
