import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ed25519 } from "@noble/curves/ed25519";
import { AccountAddress } from "../src/address.js";
import { blake2b256 } from "../src/blake2b.js";
import { getNativeBinding } from "../src/native.js";
import { NetworkId } from "../src/networkId.js";
import {
  BrowserTransactionCodecError,
  browserSignedTransactionHashHex,
  browserTransactionPayloadHashHex,
  buildBrowserTransferPayload,
  finalizeBrowserSignedTransaction,
  validateBrowserTransferSignable,
} from "../src/transactionCodec.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..", "..");
const FIXTURE_PATH = path.join(
  REPO_ROOT,
  "fixtures/norito_rpc/iroha_compact_hash_vector.properties",
);
const PRIVATE_KEY = Buffer.from(
  "CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53",
  "hex",
);
const PUBLIC_KEY = Buffer.from(ed25519.getPublicKey(PRIVATE_KEY));
const AUTHORITY = AccountAddress.fromAccount({
  algorithm: "ed25519",
  publicKey: PUBLIC_KEY,
}).toI105();
const DESTINATION_PUBLIC_KEY = Buffer.from(
  "641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201",
  "hex",
);
const DESTINATION = AccountAddress.fromAccount({
  algorithm: "ed25519",
  publicKey: DESTINATION_PUBLIC_KEY,
}).toI105();
const ASSET_DEFINITION = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const SOURCE_ASSET = `${ASSET_DEFINITION}#${AUTHORITY}`;
const NETWORK_ID = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const FOREIGN_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0x55));
const EXPECTED_COMPACT_FIXTURE_KEYS = new Set([
  "schema.version",
  "source.fixture",
  "versioned.bytes",
  "versioned.sha256",
  "bare.bytes",
  "compact.length.hex",
  "canonical.prefix.hex",
  "canonical.hash",
  "payload.prehash",
  "versioned.base64",
]);

function decodeCanonicalFixtureBase64(value, context) {
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

function parseProperties(contents) {
  const entries = new Map();
  for (const line of contents.split(/\r?\n/u)) {
    if (!line || line.startsWith("#")) continue;
    const separator = line.indexOf("=");
    assert.ok(separator > 0 && separator < line.length - 1, `malformed property line: ${line}`);
    const key = line.slice(0, separator);
    const value = line.slice(separator + 1);
    assert.ok(!entries.has(key), `duplicate property key: ${key}`);
    entries.set(key, value);
  }
  assert.deepEqual(
    [...entries.keys()].sort(),
    [...EXPECTED_COMPACT_FIXTURE_KEYS].sort(),
    "compact fixture property keys must match the required set",
  );
  decodeCanonicalFixtureBase64(entries.get("versioned.base64"), "versioned.base64");
  return Object.fromEntries(entries);
}

function properties(file) {
  return parseProperties(fs.readFileSync(file, "utf8"));
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

function field(value) {
  return Buffer.concat([compactLength(value.length), value]);
}

function struct(fields) {
  return Buffer.concat(fields.map(field));
}

function stringArchive(value) {
  return field(Buffer.from(value, "utf8"));
}

function u64(value) {
  const output = Buffer.alloc(8);
  output.writeBigUInt64LE(BigInt(value));
  return output;
}

function u32(value) {
  const output = Buffer.alloc(4);
  output.writeUInt32LE(Number(value));
  return output;
}

const CRC64_MASK = 0xffff_ffff_ffff_ffffn;
const CRC64_REFLECTED_POLY = 0xc96c_5795_d787_0f42n;
const CRC64_TABLE = Array.from({ length: 256 }, (_, index) => {
  let value = BigInt(index);
  for (let bit = 0; bit < 8; bit += 1) {
    value =
      (value & 1n) === 0n
        ? value >> 1n
        : (value >> 1n) ^ CRC64_REFLECTED_POLY;
  }
  return value;
});

function crc64(payload) {
  let value = CRC64_MASK;
  for (const byte of payload) {
    value = CRC64_TABLE[Number((value ^ BigInt(byte)) & 0xffn)] ^ (value >> 8n);
  }
  return BigInt.asUintN(64, value ^ CRC64_MASK);
}

function metadataArchive(entries) {
  const encoded = entries.map(([key, json]) =>
    struct([stringArchive(key), struct([stringArchive(json)])]),
  );
  return Buffer.concat([u64(encoded.length), ...encoded.map(field)]);
}

function readField(input, offset) {
  let length = 0n;
  let shift = 0n;
  let cursor = offset;
  for (;;) {
    assert.ok(cursor < input.length, "test fixture field length is truncated");
    const byte = input[cursor];
    cursor += 1;
    length |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) break;
    shift += 7n;
    assert.ok(shift < 70n, "test fixture field length is invalid");
  }
  assert.ok(length <= BigInt(Number.MAX_SAFE_INTEGER));
  const end = cursor + Number(length);
  assert.ok(end <= input.length, "test fixture field is truncated");
  return { value: input.subarray(cursor, end), next: end };
}

function replacePayloadMetadata(payload, archive) {
  return replacePayloadField(payload, 8, archive);
}

function replacePayloadField(payload, fieldIndex, archive) {
  const fields = [];
  let offset = 0;
  for (let index = 0; index < 10; index += 1) {
    const decoded = readField(payload, offset);
    fields.push(decoded.value);
    offset = decoded.next;
  }
  assert.equal(offset, payload.length, "test payload must contain exactly ten fields");
  fields[fieldIndex] = archive;
  return struct(fields);
}

function replaceTransferExecutable(payload, { wireId, numericArchive } = {}) {
  let topLevelOffset = 0;
  let executableBytes;
  for (let index = 0; index <= 3; index += 1) {
    const decoded = readField(payload, topLevelOffset);
    topLevelOffset = decoded.next;
    if (index === 3) executableBytes = decoded.value;
  }
  assert.ok(executableBytes);
  assert.equal(executableBytes.readUInt32LE(0), 0);
  const instructionVectorField = readField(executableBytes, 4);
  assert.equal(instructionVectorField.next, executableBytes.length);
  const instructionVector = instructionVectorField.value;
  assert.equal(instructionVector.readBigUInt64LE(0), 1n);
  const instructionField = readField(instructionVector, 8);
  assert.equal(instructionField.next, instructionVector.length);
  const instruction = instructionField.value;
  const wireField = readField(instruction, 0);
  const frameContainerField = readField(instruction, wireField.next);
  assert.equal(frameContainerField.next, instruction.length);

  const encodedWireId = wireId === undefined ? wireField.value : stringArchive(wireId);
  let frameContainer = frameContainerField.value;
  if (numericArchive !== undefined) {
    assert.ok(frameContainer.length >= 8);
    const frameLength = frameContainer.readBigUInt64LE(0);
    assert.equal(frameLength, BigInt(frameContainer.length - 8));
    const frame = frameContainer.subarray(8);
    assert.equal(frame.subarray(0, 4).toString("ascii"), "NRT0");
    assert.equal(frame.length, 40 + Number(frame.readBigUInt64LE(23)));
    const transferPayload = frame.subarray(40);
    assert.equal(transferPayload.readUInt32LE(0), 2);
    const bodyField = readField(transferPayload, 4);
    assert.equal(bodyField.next, transferPayload.length);
    const bodyFields = [];
    let bodyOffset = 0;
    for (let index = 0; index < 3; index += 1) {
      const decoded = readField(bodyField.value, bodyOffset);
      bodyFields.push(decoded.value);
      bodyOffset = decoded.next;
    }
    assert.equal(bodyOffset, bodyField.value.length);
    bodyFields[1] = numericArchive;
    const rewrittenTransferPayload = Buffer.concat([
      u32(2),
      field(struct(bodyFields)),
    ]);
    const header = Buffer.from(frame.subarray(0, 40));
    header.writeBigUInt64LE(BigInt(rewrittenTransferPayload.length), 23);
    header.writeBigUInt64LE(crc64(rewrittenTransferPayload), 31);
    const rewrittenFrame = Buffer.concat([header, rewrittenTransferPayload]);
    frameContainer = Buffer.concat([u64(rewrittenFrame.length), rewrittenFrame]);
  }

  const rewrittenInstruction = Buffer.concat([
    field(encodedWireId),
    field(frameContainer),
  ]);
  const rewrittenVector = Buffer.concat([u64(1), field(rewrittenInstruction)]);
  const rewrittenExecutable = Buffer.concat([u32(0), field(rewrittenVector)]);
  return replacePayloadField(payload, 3, rewrittenExecutable);
}

function replaceSignedPayload(versioned, payload) {
  const fields = [];
  let offset = 1;
  for (let index = 0; index < 3; index += 1) {
    const decoded = readField(versioned, offset);
    fields.push(decoded.value);
    offset = decoded.next;
  }
  assert.equal(offset, versioned.length, "test transaction must contain exactly three fields");
  fields[1] = payload;
  return Buffer.concat([Buffer.of(1), struct(fields)]);
}

function sampleInput(overrides = {}) {
  return {
    networkId: NETWORK_ID,
    authority: AUTHORITY,
    sourceAssetHoldingId: SOURCE_ASSET,
    quantity: "1.25",
    destinationAccountId: DESTINATION,
    feePayment: { payer: "authority", chargeLimits: [] },
    metadata: { memo: "browser", nested: [true, null, { order: 2 }] },
    creationTimeMs: 1_700_000_000_000,
    ttlMs: 5_000,
    nonce: 42,
    ...overrides,
  };
}

function nativeBuild(input) {
  const native = getNativeBinding();
  assert.equal(typeof native.buildTransferAssetPayload, "function");
  return native.buildTransferAssetPayload(
    input.networkId.toBytes(),
    input.authority,
    input.sourceAssetHoldingId ?? input.sourceAssetId,
    String(input.quantity),
    input.destinationAccountId,
    JSON.stringify({
      payer: "authority",
      value: { charge_limits: [], gas_limit: null },
    }),
    input.metadata == null
      ? null
      : typeof input.metadata === "string"
        ? input.metadata
        : JSON.stringify(input.metadata),
    input.creationTimeMs ?? null,
    input.ttlMs ?? null,
    input.nonce ?? null,
  );
}

function signPayload(payload) {
  const hashHex = browserTransactionPayloadHashHex(payload);
  return {
    hashHex,
    signature: Buffer.from(ed25519.sign(Buffer.from(hashHex, "hex"), PRIVATE_KEY)),
  };
}

function mixedTorsionSignature(message) {
  const privateKey = PRIVATE_KEY;
  const extended = ed25519.utils.getExtendedPublicKey(privateKey);
  const orderTwoTorsion = ed25519.Point.fromHex(
    "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
  );
  const publicKey = Buffer.from(extended.point.toBytes());
  const scalarFromDigest = (digest) => {
    let scalar = 0n;
    for (let index = digest.length - 1; index >= 0; index -= 1) {
      scalar = (scalar << 8n) | BigInt(digest[index]);
    }
    return scalar % ed25519.CURVE.n;
  };
  const hashToScalar = (...parts) =>
    scalarFromDigest(
      createHash("sha512")
        .update(Buffer.concat(parts.map((part) => Buffer.from(part))))
        .digest(),
    );
  const nonce = hashToScalar(extended.prefix, message);
  const encodedNonce = Buffer.from(
    ed25519.Point.BASE.multiply(nonce).add(orderTwoTorsion).toBytes(),
  );
  const challenge = hashToScalar(encodedNonce, publicKey, message);
  let response = (nonce + challenge * extended.scalar) % ed25519.CURVE.n;
  const encodedResponse = Buffer.alloc(32);
  for (let index = 0; index < encodedResponse.length; index += 1) {
    encodedResponse[index] = Number(response & 0xffn);
    response >>= 8n;
  }
  return {
    publicKey,
    signature: Buffer.concat([encodedNonce, encodedResponse]),
  };
}

function expectCodecError(action, code, context = "codec error") {
  assert.throws(action, (error) => {
    assert.ok(error instanceof BrowserTransactionCodecError, context);
    assert.equal(error.code, code, context);
    return true;
  });
}

test("browser transfer payload is byte-for-byte native Rust canonical", () => {
  for (const input of [
    sampleInput(),
    sampleInput({ metadata: {}, quantity: "1", ttlMs: null, nonce: null }),
    sampleInput({ metadata: '{"__proto__":{"safe":true}}', quantity: "2" }),
    sampleInput({
      metadata: { "\u{e000}": "bmp", "😀": { "\u{e001}": 1, "😁": 2 } },
      quantity: "3",
    }),
    sampleInput({
      metadata: { z: [true, null, { b: 2, a: "é" }], a: "-1.25" },
      quantity: "0.0000000000000000000000000001",
      ttlMs: 0,
      nonce: 0xffff_ffff,
    }),
  ]) {
    const browserPayload = buildBrowserTransferPayload(input);
    const native = nativeBuild(input);
    assert.deepEqual(browserPayload, Buffer.from(native.payloadBytes));
    assert.equal(
      browserTransactionPayloadHashHex(browserPayload),
      Buffer.from(native.payloadHash).toString("hex"),
    );
    assert.equal({}.safe, undefined, "metadata normalization must not mutate prototypes");
  }
});

test("browser payload pins canonical TransactionDomain::Network wire and rejects domain aliases", () => {
  const payload = buildBrowserTransferPayload(sampleInput());
  const domain = readField(payload, 0);
  const expectedNetworkBytes = Buffer.from(NETWORK_ID.toBytes());
  assert.deepEqual(
    domain.value,
    Buffer.concat([u32(0), field(expectedNetworkBytes)]),
  );
  assert.equal(domain.value.readUInt32LE(0), 0);
  assert.equal(domain.value[4], 32);
  assert.deepEqual(domain.value.subarray(5), expectedNetworkBytes);

  for (const alias of ["chain", "chainId", "chain_id"]) {
    expectCodecError(
      () => buildBrowserTransferPayload(sampleInput({ [alias]: "retired" })),
      "invalid_input",
      alias,
    );
  }
  for (const invalid of [
    "test-chain",
    NETWORK_ID.toBytes(),
    { literal: NETWORK_ID.literal, toBytes: () => NETWORK_ID.toBytes() },
  ]) {
    expectCodecError(
      () => buildBrowserTransferPayload(sampleInput({ networkId: invalid })),
      "invalid_input",
    );
  }

  const replaceDomain = (archive) =>
    Buffer.concat([field(archive), payload.subarray(domain.next)]);
  const validate = (payloadBytes) =>
    validateBrowserTransferSignable({
      networkId: NETWORK_ID,
      payloadBytes,
      authority: AUTHORITY,
      signingPublicKey: PUBLIC_KEY,
    });
  expectCodecError(() => validate(replaceDomain(u32(1))), "unsupported_payload");
  expectCodecError(() => validate(replaceDomain(u32(2))), "unsupported_payload");
  expectCodecError(
    () => validate(replaceDomain(Buffer.concat([u32(0), field(Buffer.alloc(32, 2))]))),
    "malformed_payload",
  );
  expectCodecError(
    () => validate(replaceDomain(Buffer.concat([u32(0), field(Buffer.alloc(31, 1))]))),
    "malformed_payload",
  );
});

test("browser payload requires signature-bound QueuePlan admission", () => {
  const payload = buildBrowserTransferPayload(sampleInput());
  let offset = 0;
  for (let index = 0; index <= 7; index += 1) {
    const fieldValue = readField(payload, offset);
    offset = fieldValue.next;
    if (index === 7) {
      assert.deepEqual(fieldValue.value, u32(1));
    }
  }

  expectCodecError(
    () =>
      validateBrowserTransferSignable({
        networkId: NETWORK_ID,
        payloadBytes: replacePayloadField(payload, 7, u32(0)),
        authority: AUTHORITY,
        signingPublicKey: PUBLIC_KEY,
      }),
    "unsupported_payload",
  );
});

test("browser finalizer matches the native N-API bytes and entrypoint hash", () => {
  const input = sampleInput();
  const payload = buildBrowserTransferPayload(input);
  const { hashHex, signature } = signPayload(payload);
  const signable = {
    networkId: NETWORK_ID,
    payloadBytes: payload,
    payloadHashHex: hashHex,
    authority: AUTHORITY,
    signingPublicKey: PUBLIC_KEY,
    signatureAlgorithm: "ed25519",
  };
  const browser = finalizeBrowserSignedTransaction(
    signable,
    { algorithm: "ed25519", signature },
    PUBLIC_KEY,
  );
  const nativeBinding = getNativeBinding();
  assert.equal(typeof nativeBinding.finalizeSignedTransaction, "function");
  const native = nativeBinding.finalizeSignedTransaction({
    networkId: NETWORK_ID.toBytes(),
    payloadBytes: payload,
    payloadHashHex: hashHex,
    signature,
    publicKey: PUBLIC_KEY,
    authority: AUTHORITY,
  });

  assert.deepEqual(browser.signedTransaction, Buffer.from(native.signedTransaction));
  assert.deepEqual(browser.hash, Buffer.from(native.hash));
  assert.equal(browser.hashHex, Buffer.from(native.hash).toString("hex"));
  assert.equal(browserSignedTransactionHashHex(browser.signedTransaction), browser.hashHex);

  const differentlyAuthorized = Buffer.from(browser.signedTransaction);
  const signatureField = readField(differentlyAuthorized.subarray(1), 0);
  differentlyAuthorized[signatureField.next] ^= 1;
  assert.notDeepEqual(differentlyAuthorized, browser.signedTransaction);
  assert.equal(
    browserSignedTransactionHashHex(differentlyAuthorized),
    browser.hashHex,
    "authorization proof bytes must not change transaction intent identity",
  );

  const legacyOuterAttachment = Buffer.concat([
    browser.signedTransaction,
    field(Buffer.of(0)),
  ]);
  expectCodecError(
    () => browserSignedTransactionHashHex(legacyOuterAttachment),
    "malformed_signed_transaction",
  );
});

test("browser signed hash matches the shared compact Android and native Rust golden", () => {
  const fixture = properties(FIXTURE_PATH);
  assert.equal(fixture["schema.version"], "2");
  assert.equal(fixture["source.fixture"], "transfer_asset");
  const versioned = decodeCanonicalFixtureBase64(
    fixture["versioned.base64"],
    "versioned.base64",
  );
  assert.equal(versioned.length, Number(fixture["versioned.bytes"]));
  assert.equal(
    createHash("sha256").update(versioned).digest("hex"),
    fixture["versioned.sha256"],
  );
  assert.equal(versioned[0], 1);
  const bare = versioned.subarray(1);
  assert.equal(bare.length, Number(fixture["bare.bytes"]));
  const signature = readField(bare, 0);
  const payload = readField(bare, signature.next);
  assert.equal(
    browserTransactionPayloadHashHex(payload.value),
    fixture["payload.prehash"],
  );

  const canonical = Buffer.concat([
    Buffer.alloc(4),
    Buffer.from(fixture["compact.length.hex"], "hex"),
    payload.value,
  ]);
  assert.equal(
    canonical.subarray(0, 6).toString("hex"),
    fixture["canonical.prefix.hex"],
  );
  const directHash = Buffer.from(blake2b256(canonical));
  directHash[directHash.length - 1] |= 1;
  assert.equal(directHash.toString("hex"), fixture["canonical.hash"]);
  assert.equal(browserSignedTransactionHashHex(versioned), fixture["canonical.hash"]);
  assert.equal(
    Buffer.from(getNativeBinding().hashSignedTransaction(versioned)).toString("hex"),
    fixture["canonical.hash"],
  );
});

test("compact golden parser rejects duplicate keys and base64 aliases", () => {
  const contents = fs.readFileSync(FIXTURE_PATH, "utf8");
  assert.throws(
    () => parseProperties(`${contents}\ncanonical.hash=duplicate\n`),
    /duplicate property key: canonical\.hash/,
  );
  for (const encoded of ["YQ!!", "Y Q==", "YQ=", "YQ===", "YR=="]) {
    assert.throws(
      () => decodeCanonicalFixtureBase64(encoded, "versioned.base64"),
      /(?:invalid|non-canonical) base64/,
    );
  }
});

test("browser payload binds canonical authority and sponsor fee payment intents", () => {
  const chargeLimits = [
    {
      kind: "nexus",
      assetDefinitionId: ASSET_DEFINITION,
      maxAmount: "2.5",
    },
    {
      kind: "pipelineGas",
      assetDefinitionId: ASSET_DEFINITION,
      maxAmount: "4",
    },
  ];
  const authorityPayload = buildBrowserTransferPayload(
    sampleInput({
      feePayment: {
        payer: "authority",
        chargeLimits,
        gasLimit: 50_000,
      },
    }),
  );
  assert.match(browserTransactionPayloadHashHex(authorityPayload), /^[0-9a-f]{64}$/u);

  const sponsorPayload = buildBrowserTransferPayload(
    sampleInput({
      feePayment: {
        payer: "sponsor",
        programId: `${AUTHORITY}/retail/transfers`,
        programRevision: 3,
        chargeLimits,
        gasLimit: 50_000,
      },
    }),
  );
  assert.notDeepEqual(sponsorPayload, authorityPayload);
  assert.match(browserTransactionPayloadHashHex(sponsorPayload), /^[0-9a-f]{64}$/u);

  expectCodecError(
    () => buildBrowserTransferPayload(sampleInput({ feePayment: undefined })),
    "invalid_input",
  );
  expectCodecError(
    () =>
      buildBrowserTransferPayload(
        sampleInput({
          feePayment: {
            payer: "authority",
            chargeLimits: [...chargeLimits].reverse(),
          },
        }),
      ),
    "invalid_fee_payment",
  );
  expectCodecError(
    () =>
      buildBrowserTransferPayload(
        sampleInput({
          feePayment: {
            payer: "sponsor",
            programId: `${AUTHORITY}/default`,
            programRevision: 0,
            chargeLimits,
          },
        }),
      ),
    "invalid_fee_payment",
  );
  expectCodecError(
    () =>
      buildBrowserTransferPayload(
        sampleInput({ metadata: { fee_sponsor: AUTHORITY } }),
      ),
    "invalid_fee_payment",
  );
});

test("browser builder rejects ambiguous, non-canonical, and hostile inputs", () => {
  expectCodecError(
    () => buildBrowserTransferPayload({ ...sampleInput(), unexpected: true }),
    "invalid_input",
  );
  expectCodecError(
    () =>
      buildBrowserTransferPayload(
        Object.create({ inherited: true }, Object.getOwnPropertyDescriptors(sampleInput())),
      ),
    "invalid_input",
  );
  expectCodecError(
    () => buildBrowserTransferPayload(sampleInput({ networkPrefix: 0, chainDiscriminant: 0 })),
    "invalid_input",
  );
  expectCodecError(
    () => buildBrowserTransferPayload(sampleInput({ nonce: 0 })),
    "invalid_integer",
  );
  for (const quantity of [0, 1, -1, "01", "1.0", "1e3", Number.NaN]) {
    expectCodecError(
      () => buildBrowserTransferPayload(sampleInput({ quantity })),
      "invalid_quantity",
    );
  }
  expectCodecError(
    () =>
      buildBrowserTransferPayload(
        sampleInput({ sourceAssetHoldingId: `${ASSET_DEFINITION}#${DESTINATION}` }),
      ),
    "authority_mismatch",
  );
  expectCodecError(
    () =>
      buildBrowserTransferPayload(
        sampleInput({ sourceAssetHoldingId: `12Fk4FPcMuLvW5QjDGNF2a4jAmjM#${AUTHORITY}` }),
      ),
    "invalid_asset",
  );
  const cyclic = {};
  cyclic.self = cyclic;
  expectCodecError(
    () => buildBrowserTransferPayload(sampleInput({ metadata: cyclic })),
    "invalid_metadata",
  );
});

test("browser snapshots Proxy data descriptors without invoking get traps", () => {
  const input = sampleInput();
  const expectedPayload = buildBrowserTransferPayload(input);
  let inputGets = 0;
  const proxiedInput = new Proxy(input, {
    get(target, property, receiver) {
      inputGets += 1;
      if (property === "networkId") return "descriptor-get-mismatch";
      target.quantity = "999";
      return Reflect.get(target, property, receiver);
    },
  });
  assert.deepEqual(buildBrowserTransferPayload(proxiedInput), expectedPayload);
  assert.equal(inputGets, 0, "transfer input get traps must not run");
  assert.equal(input.quantity, "1.25", "get-trap mutation must not run");

  const { hashHex, signature } = signPayload(expectedPayload);
  const signable = {
    networkId: NETWORK_ID,
    payloadBytes: expectedPayload,
    payloadHashHex: hashHex,
    authority: AUTHORITY,
  };
  let signableGets = 0;
  const proxiedSignable = new Proxy(signable, {
    get(target, property, receiver) {
      signableGets += 1;
      if (property === "payloadBytes") return Buffer.alloc(expectedPayload.length);
      target.authority = DESTINATION;
      return Reflect.get(target, property, receiver);
    },
  });
  let signatureGets = 0;
  const signatureObject = { algorithm: "ed25519", signature };
  const proxiedSignature = new Proxy(signatureObject, {
    get(target, property, receiver) {
      signatureGets += 1;
      if (property === "signature") return Buffer.alloc(64);
      target.algorithm = "unsupported";
      return Reflect.get(target, property, receiver);
    },
  });
  const expected = finalizeBrowserSignedTransaction(signable, signature, PUBLIC_KEY);
  const actual = finalizeBrowserSignedTransaction(
    proxiedSignable,
    proxiedSignature,
    PUBLIC_KEY,
  );
  assert.deepEqual(actual.signedTransaction, expected.signedTransaction);
  assert.equal(actual.hashHex, expected.hashHex);
  assert.equal(signableGets, 0, "signable get traps must not run");
  assert.equal(signatureGets, 0, "signature get traps must not run");
  assert.equal(signable.authority, AUTHORITY, "signable get-trap mutation must not run");
  assert.equal(
    signatureObject.algorithm,
    "ed25519",
    "signature get-trap mutation must not run",
  );

  const unstableTarget = sampleInput();
  const unstable = new Proxy(unstableTarget, {
    ownKeys(target) {
      return Reflect.ownKeys(target);
    },
    getOwnPropertyDescriptor(target, property) {
      if (property === "networkId") {
        delete target.networkId;
        return undefined;
      }
      return Reflect.getOwnPropertyDescriptor(target, property);
    },
  });
  expectCodecError(() => buildBrowserTransferPayload(unstable), "invalid_input");
});

test("browser metadata numbers are safe integers and match native Rust at boundaries", () => {
  for (const value of [0, -0, 1, -1, Number.MAX_SAFE_INTEGER, Number.MIN_SAFE_INTEGER]) {
    const input = sampleInput({ metadata: { value } });
    assert.deepEqual(buildBrowserTransferPayload(input), Buffer.from(nativeBuild(input).payloadBytes));
  }
  for (const value of [
    0.5,
    1e-6,
    1e20,
    1e21,
    Number.MAX_VALUE,
    Number.MAX_SAFE_INTEGER + 1,
    Number.NaN,
    Number.POSITIVE_INFINITY,
  ]) {
    expectCodecError(
      () => buildBrowserTransferPayload(sampleInput({ metadata: { value } })),
      "invalid_metadata",
    );
  }
});

test("browser metadata JSON strings must already be exact and canonical", () => {
  const canonical = '{"a":[1,true],"z":"1.25"}';
  const canonicalInput = sampleInput({ metadata: canonical });
  assert.deepEqual(
    buildBrowserTransferPayload(canonicalInput),
    Buffer.from(nativeBuild(canonicalInput).payloadBytes),
  );

  const controlValue = "\b\f\u0000\u001f\n\r\t";
  const controlObjectInput = sampleInput({ metadata: { v: controlValue } });
  assert.deepEqual(
    buildBrowserTransferPayload(controlObjectInput),
    Buffer.from(nativeBuild(controlObjectInput).payloadBytes),
    "stored Metadata Json must use Rust plain control escaping",
  );
  const canonicalControls =
    '{"v":"\\u0008\\u000C\\u0000\\u001F\\n\\r\\t"}';
  const canonicalControlInput = sampleInput({ metadata: canonicalControls });
  assert.deepEqual(
    buildBrowserTransferPayload(canonicalControlInput),
    Buffer.from(nativeBuild(canonicalControlInput).payloadBytes),
  );
  const javascriptControlAlias = JSON.stringify({ v: controlValue });
  assert.notEqual(javascriptControlAlias, canonicalControls);
  expectCodecError(
    () => buildBrowserTransferPayload(sampleInput({ metadata: javascriptControlAlias })),
    "invalid_metadata",
  );

  for (const metadata of [
    '{"value":1.0}',
    '{"value":1e0}',
    '{"value":-0}',
    '{"value":1,"value":2}',
    '{"z":1,"a":2}',
    '{ "value": 1 }',
  ]) {
    expectCodecError(
      () => buildBrowserTransferPayload(sampleInput({ metadata })),
      "invalid_metadata",
    );
  }
});

test("browser metadata rejects sparse accessor prototyped custom and huge arrays", () => {
  const sparse = [];
  sparse.length = 1;
  let accessorReads = 0;
  const accessor = [];
  Object.defineProperty(accessor, "0", {
    enumerable: true,
    configurable: true,
    get() {
      accessorReads += 1;
      return "secret";
    },
  });
  accessor.length = 1;
  const prototyped = [1];
  Object.setPrototypeOf(prototyped, Object.create(Array.prototype));
  const custom = [1];
  custom.extra = 2;
  const huge = [];
  huge.length = 0xffff_ffff;

  for (const value of [sparse, accessor, prototyped, custom]) {
    expectCodecError(
      () => buildBrowserTransferPayload(sampleInput({ metadata: { value } })),
      "invalid_metadata",
    );
  }
  expectCodecError(
    () => buildBrowserTransferPayload(sampleInput({ metadata: { value: huge } })),
    "bounds_exceeded",
  );
  assert.equal(accessorReads, 0, "metadata accessors must never be invoked");
});

test("browser metadata preserves prototype-shaped keys and enforces Rust Unicode rules", () => {
  const prototypeKeys = JSON.parse(
    '{"__proto__":{"safe":true},"constructor":"kept","prototype":"kept"}',
  );
  const prototypeInput = sampleInput({ metadata: prototypeKeys });
  assert.deepEqual(
    buildBrowserTransferPayload(prototypeInput),
    Buffer.from(nativeBuild(prototypeInput).payloadBytes),
  );
  assert.equal({}.safe, undefined);

  const nelMetadata = { [`bad\u0085key`]: 1 };
  expectCodecError(
    () => buildBrowserTransferPayload(sampleInput({ metadata: nelMetadata })),
    "invalid_metadata",
  );
  assert.throws(() => nativeBuild(sampleInput({ metadata: nelMetadata })));
  expectCodecError(
    () => buildBrowserTransferPayload(sampleInput({ metadata: { ["k".repeat(256)]: 1 } })),
    "invalid_metadata",
  );
  expectCodecError(
    () => buildBrowserTransferPayload(sampleInput({ metadata: { value: "x".repeat(65_537) } })),
    "bounds_exceeded",
  );
});

test("browser rejects lone surrogates while preserving scalar and noncharacter parity", () => {
  for (const surrogate of ["\ud800", "\udfff"]) {
    expectCodecError(
      () => buildBrowserTransferPayload(sampleInput({ metadata: { value: surrogate } })),
      "invalid_metadata",
    );
    expectCodecError(
      () =>
        buildBrowserTransferPayload(
          sampleInput({ metadata: JSON.parse(`{"value":"${surrogate}"}`) }),
        ),
      "invalid_metadata",
    );
    expectCodecError(
      () => buildBrowserTransferPayload(sampleInput({ metadata: { [`key${surrogate}`]: 1 } })),
      "invalid_metadata",
    );
    assert.throws(() => nativeBuild(sampleInput({ metadata: { value: surrogate } })));
  }

  const scalarInput = sampleInput({
    metadata: { "emoji😀": "paired😀", noncharacters: "\ufdd0\u{10ffff}" },
  });
  assert.deepEqual(
    buildBrowserTransferPayload(scalarInput),
    Buffer.from(nativeBuild(scalarInput).payloadBytes),
  );
});

test("browser numeric parsing rejects multi-megabyte decimal strings before conversion", () => {
  const huge = "9".repeat(2_000_000);
  for (const [field, code] of [
    ["networkPrefix", "bounds_exceeded"],
    ["creationTimeMs", "bounds_exceeded"],
    ["ttlMs", "bounds_exceeded"],
    ["nonce", "bounds_exceeded"],
    ["quantity", "bounds_exceeded"],
  ]) {
    expectCodecError(
      () => buildBrowserTransferPayload(sampleInput({ [field]: huge })),
      code,
    );
  }
});

test("browser rejects non-canonical scaled Numeric archives with trailing zeros", () => {
  const canonicalPayload = buildBrowserTransferPayload(
    sampleInput({ quantity: "1.2" }),
  );
  const nonCanonicalNumeric = struct([
    Buffer.concat([u32(1), Buffer.of(120)]),
    u32(2),
  ]);
  const nonCanonicalPayload = replaceTransferExecutable(canonicalPayload, {
    numericArchive: nonCanonicalNumeric,
  });
  const nonCanonicalHash = browserTransactionPayloadHashHex(nonCanonicalPayload);
  const nonCanonicalSignature = signPayload(nonCanonicalPayload).signature;
  const signable = {
    networkId: NETWORK_ID,
    payloadBytes: nonCanonicalPayload,
    payloadHashHex: nonCanonicalHash,
    authority: AUTHORITY,
    signingPublicKey: PUBLIC_KEY,
    signatureAlgorithm: "ed25519",
  };
  expectCodecError(
    () => validateBrowserTransferSignable(signable),
    "malformed_payload",
  );
  expectCodecError(
    () =>
      finalizeBrowserSignedTransaction(
        signable,
        nonCanonicalSignature,
        PUBLIC_KEY,
      ),
    "malformed_payload",
  );

  const canonicalSignature = signPayload(canonicalPayload).signature;
  const canonicalSigned = finalizeBrowserSignedTransaction(
    {
      networkId: NETWORK_ID,
      payloadBytes: canonicalPayload,
      payloadHashHex: browserTransactionPayloadHashHex(canonicalPayload),
      authority: AUTHORITY,
      signingPublicKey: PUBLIC_KEY,
    },
    canonicalSignature,
    PUBLIC_KEY,
  ).signedTransaction;
  expectCodecError(
    () =>
      browserSignedTransactionHashHex(
        replaceSignedPayload(canonicalSigned, nonCanonicalPayload),
      ),
    "malformed_signed_transaction",
  );

  assert.doesNotThrow(() =>
    buildBrowserTransferPayload(sampleInput({ quantity: "10" })),
  );
});

test("browser signed payload validation enforces byte caps before decoding or bigint work", () => {
  const maximumQuantity = ((1n << 511n) - 1n).toString();
  const maximumInput = sampleInput({ quantity: maximumQuantity });
  assert.deepEqual(
    buildBrowserTransferPayload(maximumInput),
    Buffer.from(nativeBuild(maximumInput).payloadBytes),
    "the canonical 64-byte signed-positive boundary must remain valid",
  );
  expectCodecError(
    () => buildBrowserTransferPayload(sampleInput({ quantity: (1n << 511n).toString() })),
    "bounds_exceeded",
  );

  const payload = buildBrowserTransferPayload(sampleInput());
  const { hashHex, signature } = signPayload(payload);
  const baseline = finalizeBrowserSignedTransaction(
    {
      networkId: NETWORK_ID,
      payloadBytes: payload,
      payloadHashHex: hashHex,
      authority: AUTHORITY,
    },
    signature,
    PUBLIC_KEY,
  ).signedTransaction;
  const oversizedMantissaBytes = 64 * 1024;
  const oversizedNumeric = struct([
    Buffer.concat([
      u32(oversizedMantissaBytes),
      Buffer.alloc(oversizedMantissaBytes, 1),
    ]),
    u32(0),
  ]);
  const oversizedJson = JSON.stringify("x".repeat(65_535));
  assert.equal(Buffer.byteLength(oversizedJson), 65_537);

  for (const [context, mutatedPayload] of [
    [
      "chain ID",
      replacePayloadField(
        payload,
        0,
        struct([stringArchive("c".repeat(1_025))]),
      ),
    ],
    ["wire ID", replaceTransferExecutable(payload, { wireId: "w".repeat(15) })],
    [
      "metadata key",
      replacePayloadMetadata(payload, metadataArchive([["k".repeat(256), "1"]])),
    ],
    [
      "metadata JSON",
      replacePayloadMetadata(payload, metadataArchive([["json", oversizedJson]])),
    ],
    [
      "numeric mantissa",
      replaceTransferExecutable(payload, { numericArchive: oversizedNumeric }),
    ],
  ]) {
    const mutated = replaceSignedPayload(baseline, mutatedPayload);
    expectCodecError(
      () => browserSignedTransactionHashHex(mutated),
      "bounds_exceeded",
      context,
    );
    assert.ok(mutated.length < 1024 * 1024, `${context} fixture must stay bounded`);
  }
});

test("browser byte ingress copies ArrayBuffer and SAB views and bounds before copying", () => {
  const payload = buildBrowserTransferPayload(sampleInput());
  const { hashHex, signature } = signPayload(payload);
  const expected = finalizeBrowserSignedTransaction(
    {
      networkId: NETWORK_ID,
      payloadBytes: payload,
      payloadHashHex: hashHex,
      authority: AUTHORITY,
    },
    signature,
    PUBLIC_KEY,
  );
  const reentrantPayload = Uint8Array.from(payload);
  let reentrantMutation = false;
  const reentrantSignature = new Proxy(
    { algorithm: "ed25519", signature: Buffer.from(signature) },
    {
      get(target, property, receiver) {
        if (property === "algorithm" && !reentrantMutation) {
          reentrantPayload[0] ^= 0xff;
          reentrantMutation = true;
        }
        return Reflect.get(target, property, receiver);
      },
    },
  );
  const reentrantFinalized = finalizeBrowserSignedTransaction(
    {
      networkId: NETWORK_ID,
      payloadBytes: reentrantPayload,
      payloadHashHex: hashHex,
      authority: AUTHORITY,
    },
    reentrantSignature,
    PUBLIC_KEY,
  );
  assert.equal(reentrantMutation, false);
  assert.deepEqual(Buffer.from(reentrantPayload), payload);
  assert.deepEqual(reentrantFinalized.signedTransaction, expected.signedTransaction);
  assert.equal(reentrantFinalized.hashHex, expected.hashHex);

  const sharedPayload = new SharedArrayBuffer(payload.length + 8);
  const sharedPayloadView = new Uint8Array(sharedPayload, 4, payload.length);
  sharedPayloadView.set(payload);
  const sharedSignature = new SharedArrayBuffer(64);
  const sharedSignatureView = new Uint8Array(sharedSignature);
  sharedSignatureView.set(signature);
  const sharedKey = new SharedArrayBuffer(32);
  const sharedKeyView = new Uint8Array(sharedKey);
  sharedKeyView.set(PUBLIC_KEY);

  const finalized = finalizeBrowserSignedTransaction(
    {
      networkId: NETWORK_ID,
      payloadBytes: sharedPayloadView,
      payloadHashHex: hashHex,
      authority: AUTHORITY,
      signingPublicKey: sharedKeyView,
    },
    sharedSignatureView,
    sharedKeyView,
  );
  const outputSnapshot = Buffer.from(finalized.signedTransaction);
  sharedPayloadView.fill(0);
  sharedSignatureView.fill(0);
  sharedKeyView.fill(0);
  assert.deepEqual(finalized.signedTransaction, outputSnapshot);
  assert.equal(
    browserTransactionPayloadHashHex(payload.buffer.slice(payload.byteOffset, payload.byteOffset + payload.length)),
    hashHex,
  );

  expectCodecError(
    () => browserTransactionPayloadHashHex(new ArrayBuffer(1024 * 1024 + 1)),
    "bounds_exceeded",
  );
  expectCodecError(
    () => finalizeBrowserSignedTransaction(
      { networkId: NETWORK_ID, payloadBytes: payload, authority: AUTHORITY },
      new ArrayBuffer(65),
      PUBLIC_KEY,
    ),
    "bounds_exceeded",
  );
  expectCodecError(
    () => finalizeBrowserSignedTransaction(
      { networkId: NETWORK_ID, payloadBytes: payload, authority: AUTHORITY },
      signature,
      new ArrayBuffer(33),
    ),
    "bounds_exceeded",
  );
});

test("browser strict Ed25519 rejects the mixed-torsion signature accepted by cofactored verify", () => {
  const payload = buildBrowserTransferPayload(sampleInput());
  const payloadHashHex = browserTransactionPayloadHashHex(payload);
  const { publicKey, signature } = mixedTorsionSignature(
    Buffer.from(payloadHashHex, "hex"),
  );
  assert.equal(
    ed25519.verify(signature, Buffer.from(payloadHashHex, "hex"), publicKey, {
      zip215: false,
    }),
    true,
    "reviewer PoC must exercise noble's cofactored acceptance",
  );
  const signable = {
    networkId: NETWORK_ID,
    payloadBytes: payload,
    payloadHashHex,
    authority: AUTHORITY,
    signingPublicKey: publicKey,
  };
  expectCodecError(
    () => finalizeBrowserSignedTransaction(signable, signature, publicKey),
    "invalid_signature",
  );
  assert.throws(() =>
    getNativeBinding().finalizeSignedTransaction({
      networkId: NETWORK_ID.toBytes(),
      payloadBytes: payload,
      payloadHashHex,
      signature,
      publicKey,
      authority: AUTHORITY,
    }),
  );
});

test("browser finalizer fails closed on contradictory signer and payload state", () => {
  const payload = buildBrowserTransferPayload(sampleInput());
  const { hashHex, signature } = signPayload(payload);
  const signable = {
    networkId: NETWORK_ID,
    payloadBytes: payload,
    payloadHashHex: hashHex,
    authority: AUTHORITY,
    signingPublicKey: PUBLIC_KEY,
    signatureAlgorithm: "ed25519",
  };
  const otherKey = Buffer.from(DESTINATION_PUBLIC_KEY);

  assert.equal(
    validateBrowserTransferSignable({
      ...signable,
      signatureAlgorithm: "0",
    }).signatureAlgorithm,
    "ed25519",
    "the validator must retain the canonical string-zero algorithm alias",
  );
  expectCodecError(
    () =>
      finalizeBrowserSignedTransaction(
        { ...signable, signatureAlgorithm: "0" },
        signature,
        PUBLIC_KEY,
      ),
    "unsupported_algorithm",
  );

  expectCodecError(
    () =>
      finalizeBrowserSignedTransaction(
        { ...signable, networkId: FOREIGN_NETWORK_ID },
        signature,
        PUBLIC_KEY,
      ),
    "network_id_mismatch",
  );
  expectCodecError(
    () =>
      finalizeBrowserSignedTransaction(
        { ...signable, payloadHashHex: "00".repeat(32) },
        signature,
        PUBLIC_KEY,
      ),
    "payload_hash_mismatch",
  );
  expectCodecError(
    () =>
      finalizeBrowserSignedTransaction(
        { ...signable, signingPublicKey: otherKey },
        signature,
        PUBLIC_KEY,
      ),
    "authority_mismatch",
  );
  expectCodecError(
    () => finalizeBrowserSignedTransaction(signable, signature, otherKey),
    "authority_mismatch",
  );
  const badSignature = Buffer.from(signature);
  badSignature[0] ^= 0x80;
  expectCodecError(
    () => finalizeBrowserSignedTransaction(signable, badSignature, PUBLIC_KEY),
    "invalid_signature",
  );
  expectCodecError(
    () =>
      finalizeBrowserSignedTransaction(
        { ...signable, signatureAlgorithm: "mldsa" },
        signature,
        PUBLIC_KEY,
      ),
    "unsupported_algorithm",
  );

  const overlong = Buffer.concat([
    Buffer.of(payload[0] | 0x80, 0),
    payload.subarray(1),
  ]);
  expectCodecError(
    () =>
      finalizeBrowserSignedTransaction(
        { ...signable, payloadBytes: overlong, payloadHashHex: undefined },
        signature,
        PUBLIC_KEY,
      ),
    "malformed_payload",
  );

  for (const [json, code] of [
    [`${"[".repeat(40)}0${"]".repeat(40)}`, "bounds_exceeded"],
    [`[${new Array(4_097).fill("0").join(",")}]`, "bounds_exceeded"],
    [JSON.stringify("x".repeat(65_536)), "bounds_exceeded"],
  ]) {
    const hostilePayload = replacePayloadMetadata(
      payload,
      metadataArchive([["hostile", json]]),
    );
    const hostileSignature = signPayload(hostilePayload);
    expectCodecError(
      () =>
        finalizeBrowserSignedTransaction(
          {
            ...signable,
            payloadBytes: hostilePayload,
            payloadHashHex: hostileSignature.hashHex,
          },
          hostileSignature.signature,
          PUBLIC_KEY,
        ),
      code,
    );
  }

  const aggregatePayload = replacePayloadMetadata(
    payload,
    metadataArchive([
      ["first", JSON.stringify("x".repeat(33_000))],
      ["second", JSON.stringify("y".repeat(33_000))],
    ]),
  );
  const aggregateSignature = signPayload(aggregatePayload);
  expectCodecError(
    () =>
      finalizeBrowserSignedTransaction(
        {
          ...signable,
          payloadBytes: aggregatePayload,
          payloadHashHex: aggregateSignature.hashHex,
        },
        aggregateSignature.signature,
        PUBLIC_KEY,
      ),
    "bounds_exceeded",
  );

  const utf16SortedPayload = replacePayloadMetadata(
    payload,
    metadataArchive([
      ["😀", "0"],
      ["\u{e000}", "0"],
    ]),
  );
  const utf16SortedSignature = signPayload(utf16SortedPayload);
  expectCodecError(
    () =>
      finalizeBrowserSignedTransaction(
        {
          ...signable,
          payloadBytes: utf16SortedPayload,
          payloadHashHex: utf16SortedSignature.hashHex,
        },
        utf16SortedSignature.signature,
        PUBLIC_KEY,
      ),
    "malformed_payload",
  );
});

test("browser hash rejects wrong versions, trailing data, and overlong field lengths", () => {
  const payload = buildBrowserTransferPayload(sampleInput());
  const { hashHex, signature } = signPayload(payload);
  const finalized = finalizeBrowserSignedTransaction(
    {
      networkId: NETWORK_ID,
      payloadBytes: payload,
      payloadHashHex: hashHex,
      authority: AUTHORITY,
      signingPublicKey: PUBLIC_KEY,
      signatureAlgorithm: "ed25519",
    },
    signature,
    PUBLIC_KEY,
  );

  expectCodecError(
    () => browserSignedTransactionHashHex(Buffer.alloc(0)),
    "malformed_signed_transaction",
  );
  const wrongVersion = Buffer.from(finalized.signedTransaction);
  wrongVersion[0] = 2;
  expectCodecError(
    () => browserSignedTransactionHashHex(wrongVersion),
    "malformed_signed_transaction",
  );
  expectCodecError(
    () =>
      browserSignedTransactionHashHex(
        Buffer.concat([finalized.signedTransaction, Buffer.of(0)]),
      ),
    "malformed_payload",
  );
  expectCodecError(
    () =>
      browserSignedTransactionHashHex(
        replaceSignedPayload(finalized.signedTransaction, Buffer.of(0)),
      ),
    "malformed_signed_transaction",
  );

  const versioned = finalized.signedTransaction;
  assert.equal(versioned[1], 0x8a);
  assert.equal(versioned[2], 0x01);
  const overlong = Buffer.concat([
    versioned.subarray(0, 2),
    Buffer.of(0x81, 0x00),
    versioned.subarray(3),
  ]);
  expectCodecError(
    () => browserSignedTransactionHashHex(overlong),
    "malformed_payload",
  );
});
