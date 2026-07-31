#!/usr/bin/env node
/**
 * Deterministically regenerate the shared compact signed-transaction vector.
 *
 * The vector deliberately uses only the browser codec and a fixed Ed25519 key,
 * so regeneration does not depend on a native build or platform-specific ABI.
 */
import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { ed25519 } from "@noble/curves/ed25519";

import { AccountAddress } from "../src/address.js";
import {
  browserSignedTransactionHashHex,
  browserTransactionPayloadHashHex,
  buildBrowserTransferPayload,
  finalizeBrowserSignedTransaction,
} from "../src/transactionCodec.js";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const PACKAGE_ROOT = path.resolve(path.dirname(SCRIPT_PATH), "..");
const REPO_ROOT = path.resolve(PACKAGE_ROOT, "..", "..");
const SOURCE_ROOT = path.join(PACKAGE_ROOT, "src");
const TRANSACTION_CODEC_PATH = path.join(SOURCE_ROOT, "transactionCodec.js");
const SOURCE_BUNDLE_DOMAIN = "iroha-js-compact-vector-source-bundle-v1";
export const COMPACT_HASH_VECTOR_PATH = path.join(
  REPO_ROOT,
  "fixtures/norito_rpc/iroha_compact_hash_vector.properties",
);

const PRIVATE_KEY = Buffer.from(
  "CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53",
  "hex",
);
const DESTINATION_PUBLIC_KEY = Buffer.from(
  "641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201",
  "hex",
);
const ASSET_DEFINITION = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const PROPERTY_ORDER = Object.freeze([
  "schema.version",
  "source.tag",
  "source.bundle.sha256",
  "reference",
  "versioned.bytes",
  "versioned.sha256",
  "bare.bytes",
  "compact.length.hex",
  "canonical.prefix.hex",
  "canonical.hash",
  "payload.prehash",
  "versioned.base64",
]);

function compactLength(value) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError("compact length input must be a non-negative safe integer");
  }
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

function localImportPaths(source, file) {
  if (/\bimport\s*\(/u.test(source)) {
    throw new Error(
      `dynamic imports are not allowed in the compact-vector source closure: ${file}`,
    );
  }
  const imports = [];
  const pattern = /(?:\bfrom\s*|\bimport\s*)["'](\.[^"']+)["']/gu;
  for (const match of source.matchAll(pattern)) {
    const resolved = path.resolve(path.dirname(file), match[1]);
    if (
      resolved !== SOURCE_ROOT &&
      !resolved.startsWith(`${SOURCE_ROOT}${path.sep}`)
    ) {
      throw new Error(`local import escapes the source root: ${match[1]} in ${file}`);
    }
    imports.push(resolved);
  }
  return imports;
}

function requireBoundSourceFile(file, readFileSync, lstatSync) {
  const metadata = lstatSync(file);
  if (metadata.isSymbolicLink() || !metadata.isFile()) {
    throw new Error(`bound source must be a regular non-symlink file: ${file}`);
  }
  return Buffer.from(readFileSync(file));
}

export function compactHashVectorSourceBundleSha256({
  readFileSync = fs.readFileSync,
  lstatSync = fs.lstatSync,
} = {}) {
  const contents = new Map();
  const pending = [TRANSACTION_CODEC_PATH];
  while (pending.length > 0) {
    const file = pending.pop();
    if (contents.has(file)) continue;
    const bytes = requireBoundSourceFile(file, readFileSync, lstatSync);
    contents.set(file, bytes);
    const source = bytes.toString("utf8");
    if (Buffer.from(source, "utf8").compare(bytes) !== 0) {
      throw new Error(`bound JavaScript source is not valid UTF-8: ${file}`);
    }
    for (const imported of localImportPaths(source, file)) {
      pending.push(imported);
    }
  }
  for (const file of [
    SCRIPT_PATH,
    path.join(PACKAGE_ROOT, "package.json"),
    path.join(PACKAGE_ROOT, "package-lock.json"),
  ]) {
    contents.set(file, requireBoundSourceFile(file, readFileSync, lstatSync));
  }

  const hash = createHash("sha256");
  hash.update(`${SOURCE_BUNDLE_DOMAIN}\0`, "utf8");
  const entries = [...contents].map(([file, bytes]) => {
    const relative = path.relative(PACKAGE_ROOT, file).split(path.sep).join("/");
    if (!relative || relative.startsWith("../")) {
      throw new Error(`bound source path is outside the package root: ${file}`);
    }
    return { relative, bytes };
  });
  entries.sort((left, right) =>
    left.relative < right.relative ? -1 : left.relative > right.relative ? 1 : 0,
  );
  for (const { relative, bytes } of entries) {
    hash.update(`${relative}\0${bytes.length}\0`, "utf8");
    hash.update(bytes);
    hash.update("\0", "utf8");
  }
  return hash.digest("hex");
}

export function compactHashVectorInput() {
  const publicKey = Buffer.from(ed25519.getPublicKey(PRIVATE_KEY));
  const authority = AccountAddress.fromAccount({
    algorithm: "ed25519",
    publicKey,
  }).toI105();
  const destination = AccountAddress.fromAccount({
    algorithm: "ed25519",
    publicKey: DESTINATION_PUBLIC_KEY,
  }).toI105();
  return {
    privateKey: Buffer.from(PRIVATE_KEY),
    publicKey,
    transfer: {
      chainId: "test-chain",
      authority,
      sourceAssetHoldingId: `${ASSET_DEFINITION}#${authority}`,
      quantity: "1.25",
      destinationAccountId: destination,
      feePayment: { payer: "authority", chargeLimits: [] },
      metadata: { memo: "browser", nested: [true, null, { order: 2 }] },
      creationTimeMs: 1_700_000_000_000,
      ttlMs: 5_000,
      nonce: 42,
    },
  };
}

export function createCompactHashVector(options = {}) {
  const sourceBundleSha256 =
    options.sourceBundleSha256 ?? compactHashVectorSourceBundleSha256();
  if (!/^[0-9a-f]{64}$/u.test(sourceBundleSha256)) {
    throw new Error("source bundle SHA-256 must be 64 lowercase hexadecimal characters");
  }
  const { privateKey, publicKey, transfer } = compactHashVectorInput();
  const payload = buildBrowserTransferPayload(transfer);
  const payloadHashHex = browserTransactionPayloadHashHex(payload);
  const signature = Buffer.from(
    ed25519.sign(Buffer.from(payloadHashHex, "hex"), privateKey),
  );
  const finalized = finalizeBrowserSignedTransaction(
    {
      payloadBytes: payload,
      payloadHashHex,
      authority: transfer.authority,
      signingPublicKey: publicKey,
      signatureAlgorithm: "ed25519",
    },
    { algorithm: "ed25519", signature },
    publicKey,
  );
  const versioned = Buffer.from(finalized.signedTransaction);
  if (versioned[0] !== 1) {
    throw new Error(`unexpected signed transaction version: ${versioned[0]}`);
  }
  const bare = versioned.subarray(1);
  const encodedLength = compactLength(payload.length);
  const canonical = Buffer.concat([Buffer.alloc(4), encodedLength, payload]);
  const canonicalHash = browserSignedTransactionHashHex(versioned);
  if (canonicalHash !== finalized.hashHex) {
    throw new Error("browser finalizer and compact entrypoint hashes disagree");
  }

  return Object.freeze({
    "schema.version": "2",
    "source.tag": "abi21-browser-codec-source-bundle-v1",
    "source.bundle.sha256": sourceBundleSha256,
    reference: "deterministic-browser-transfer-codec-vector-v1",
    "versioned.bytes": String(versioned.length),
    "versioned.sha256": createHash("sha256").update(versioned).digest("hex"),
    "bare.bytes": String(bare.length),
    "compact.length.hex": encodedLength.toString("hex"),
    "canonical.prefix.hex": canonical
      .subarray(0, 4 + encodedLength.length)
      .toString("hex"),
    "canonical.hash": canonicalHash,
    "payload.prehash": payloadHashHex,
    "versioned.base64": versioned.toString("base64"),
  });
}

export function renderCompactHashVector(options = {}) {
  const vector = createCompactHashVector(options);
  const lines = PROPERTY_ORDER.map((key) => {
    const value = vector[key];
    if (typeof value !== "string" || value.length === 0 || /[\r\n]/u.test(value)) {
      throw new Error(`compact vector property ${key} is not a safe non-empty value`);
    }
    return `${key}=${value}`;
  });
  return `${lines.join("\n")}\n`;
}

function requireRegularNonSymlink(file) {
  const metadata = fs.lstatSync(file);
  if (metadata.isSymbolicLink() || !metadata.isFile()) {
    throw new Error(`compact vector path must be a regular non-symlink file: ${file}`);
  }
}

export function checkCompactHashVectorFile(
  file = COMPACT_HASH_VECTOR_PATH,
  expected = renderCompactHashVector(),
) {
  requireRegularNonSymlink(file);
  const actual = fs.readFileSync(file, "utf8");
  if (actual !== expected) {
    const expectedHash = createHash("sha256").update(expected).digest("hex");
    const actualHash = createHash("sha256").update(actual).digest("hex");
    throw new Error(
      `compact vector drift at ${file}: expected sha256 ${expectedHash}, got ${actualHash}`,
    );
  }
}

export function writeCompactHashVectorFile(
  file = COMPACT_HASH_VECTOR_PATH,
  contents = renderCompactHashVector(),
) {
  const parent = path.dirname(file);
  fs.mkdirSync(parent, { recursive: true });
  if (fs.existsSync(file)) requireRegularNonSymlink(file);
  const temporary = path.join(
    parent,
    `.${path.basename(file)}.${process.pid}.${Date.now()}.tmp`,
  );
  let descriptor;
  try {
    descriptor = fs.openSync(temporary, "wx", 0o644);
    fs.writeFileSync(descriptor, contents, "utf8");
    fs.fsyncSync(descriptor);
    fs.closeSync(descriptor);
    descriptor = undefined;
    fs.renameSync(temporary, file);
    const directoryDescriptor = fs.openSync(parent, "r");
    try {
      fs.fsyncSync(directoryDescriptor);
    } finally {
      fs.closeSync(directoryDescriptor);
    }
  } finally {
    if (descriptor !== undefined) fs.closeSync(descriptor);
    fs.rmSync(temporary, { force: true });
  }
}

export function parseCompactHashVectorArguments(argv) {
  let check = false;
  let output = COMPACT_HASH_VECTOR_PATH;
  let outputSeen = false;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--check") {
      if (check) throw new Error("--check may only be specified once");
      check = true;
    } else if (argument === "--output") {
      if (outputSeen) throw new Error("--output may only be specified once");
      const value = argv[index + 1];
      if (value === undefined || value.startsWith("--")) {
        throw new Error("--output requires a path");
      }
      output = path.resolve(value);
      outputSeen = true;
      index += 1;
    } else if (argument === "--help" || argument === "-h") {
      return { help: true, check: false, output };
    } else {
      throw new Error(`unknown argument: ${argument}`);
    }
  }
  return { help: false, check, output };
}

function main(argv) {
  const options = parseCompactHashVectorArguments(argv);
  if (options.help) {
    console.log(
      "Usage: regenerate-compact-hash-vector.mjs [--check] [--output PATH]",
    );
    return;
  }
  if (options.check) {
    checkCompactHashVectorFile(options.output);
    console.log(`[compact-vector] verified ${options.output}`);
  } else {
    writeCompactHashVectorFile(options.output);
    checkCompactHashVectorFile(options.output);
    console.log(`[compact-vector] wrote ${options.output}`);
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === SCRIPT_PATH) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(`[compact-vector] ${error instanceof Error ? error.message : error}`);
    process.exitCode = 1;
  }
}
