import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { ed25519, x25519 } from "@noble/curves/ed25519";
import { chacha20poly1305 } from "@noble/ciphers/chacha";
import { blake2b } from "@noble/hashes/blake2b";
import { hkdf } from "@noble/hashes/hkdf";
import { sha256 } from "@noble/hashes/sha2";

import {
  ConnectApprovalRejectedError,
  ConnectSessionClosedError,
  ConnectSignRequestError,
  NetworkId,
  TORII_CANONICAL_REQUEST_DOMAIN_TAG,
  buildConnectTokenProtocol,
  buildConnectWebSocketUrl,
  createConnectAppSession,
  createConnectCanonicalRequestAuth,
  createConnectSessionPreview,
  deleteConnectSession,
  openConnectWebSocket,
  registerConnectSession,
  resolveConnectLaunchUri,
  resolveConnectLaunchUriForProtocol,
  rewriteConnectUriProtocol,
  toBase64Url,
  toHex,
} from "../src/connect.browser.js";
import { AccountAddress } from "../src/address.js";
import { networkIdToNoritoJson } from "../src/networkIdNoritoJson.js";
import {
  buildCanonicalJsonRequest,
  canonicalRequestSignatureMessage,
} from "../src/canonicalRequest.js";
import { NexusAppClient } from "../src/nexusApp.js";

const connectVectors = JSON.parse(
  readFileSync(new URL("../../../fixtures/connect/session_vectors.json", import.meta.url), "utf8"),
);

class RecordingWebSocket {
  constructor(url, protocols) {
    this.url = url;
    this.protocols = protocols;
    this.sent = [];
    this.listeners = new Map();
    this.readyState = 0;
    RecordingWebSocket.instances.push(this);
  }

  addEventListener(name, handler) {
    const handlers = this.listeners.get(name) ?? [];
    handlers.push(handler);
    this.listeners.set(name, handlers);
  }

  send(data) {
    this.sent.push(Buffer.from(data));
  }

  open() {
    this.readyState = 1;
    this.emit("open", {});
  }

  receive(data) {
    this.emit("message", { data });
  }

  triggerError() {
    this.emit("error", {});
  }

  close() {
    this.readyState = 3;
    this.emit("close", {});
  }

  emit(name, payload) {
    for (const handler of this.listeners.get(name) ?? []) {
      handler(payload);
    }
  }
}
RecordingWebSocket.instances = [];
RecordingWebSocket.OPEN = 1;

const encoder = new TextEncoder();
const CONNECT_SALT_PREFIX = encoder.encode("iroha-connect|salt|");
const CONNECT_AAD_PREFIX = encoder.encode("connect:v1");
const CONNECT_K_APP = encoder.encode("iroha-connect|k_app");
const CONNECT_K_WALLET = encoder.encode("iroha-connect|k_wallet");
const X25519_HKDF_SALT = encoder.encode("iroha:x25519:hkdf:v1");
const X25519_HKDF_INFO = encoder.encode("iroha:x25519:session-key");
const APPROVE_DOMAIN = encoder.encode("iroha-connect|approve|v1");
const RELAY_AUTH_DOMAIN = encoder.encode("iroha-connect|relay-auth|v1");
const CONNECT_ENVELOPE_TYPE_NAME = "iroha_torii_shared::connect::EnvelopeV1";
const CANONICAL_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));
const CANONICAL_NETWORK_ID_JSON = networkIdToNoritoJson(CANONICAL_NETWORK_ID);
const REGISTER_TOKEN_APP = "A".repeat(43);
const REGISTER_TOKEN_WALLET = "B".repeat(43);
const REGISTER_TOKEN_MANAGEMENT = "C".repeat(43);
const REGISTER_TOKEN_RELAY = "D".repeat(43);

function u16(value) {
  const out = Buffer.alloc(2);
  out.writeUInt16LE(value, 0);
  return out;
}

function u32(value) {
  const out = Buffer.alloc(4);
  out.writeUInt32LE(value, 0);
  return out;
}

function u64(value) {
  const out = Buffer.alloc(8);
  out.writeBigUInt64LE(BigInt(value), 0);
  return out;
}

function lenPrefixed(bytes) {
  return Buffer.concat([u64(bytes.length), Buffer.from(bytes)]);
}

function encodeString(value) {
  return lenPrefixed(Buffer.from(value, "utf8"));
}

function encodeBytes(value) {
  return lenPrefixed(Buffer.from(value));
}

function encodeStruct(fields) {
  return Buffer.concat(fields.map((field) => lenPrefixed(field)));
}

function encodeDir(value) {
  return u32(value);
}

function encodeRole(value) {
  return u32(value);
}

function encodeWalletSignature(signature) {
  return encodeStruct([Buffer.from([signature.algorithm]), encodeBytes(signature.signature)]);
}

function taggedApproveField(tag, value) {
  const tagBytes = Buffer.from(tag, "utf8");
  return Buffer.concat([u16(tagBytes.length), tagBytes, u64(value.length), Buffer.from(value)]);
}

function relayAuthHash(preview, relayToken) {
  return Buffer.from(sha256(Buffer.concat([
    Buffer.from(RELAY_AUTH_DOMAIN),
    Buffer.from(preview.sidBytes),
    Buffer.from(relayToken, "utf8"),
  ])));
}

function tokenAuthHash(kind, sidBytes, token) {
  return Buffer.from(sha256(Buffer.concat([
    Buffer.from("iroha-connect|token-auth|v1", "utf8"),
    Buffer.from(kind, "utf8"),
    Buffer.from(sidBytes),
    Buffer.from(token, "utf8"),
  ])));
}

test("Connect session vector fixture matches browser crypto helpers", () => {
  const networkId = NetworkId.parse(connectVectors.network_id);
  assert.equal(Buffer.from(networkId.toBytes()).toString("hex"), connectVectors.network_id_hex);
  const appPublicKey = Buffer.from(connectVectors.app_pk_hex, "hex");
  const nonce = Buffer.from(connectVectors.nonce_hex, "hex");
  const sidBytes = Buffer.from(connectVectors.sid_hex, "hex");
  assert.equal(
    Buffer.from(blake2b(Buffer.concat([
      Buffer.from("iroha-connect|sid|", "utf8"),
      Buffer.from(networkId.toBytes()),
      appPublicKey,
      nonce,
    ]), { dkLen: 32 })).toString("hex"),
    connectVectors.sid_hex,
  );
  const preview = {
    networkId,
    sidBytes,
    appKeyPair: { publicKey: appPublicKey },
  };
  assert.equal(
    relayAuthHash(preview, connectVectors.tokens.relay).toString("hex"),
    connectVectors.relay_auth_hash_hex,
  );
  assert.equal(
    tokenAuthHash("app", sidBytes, connectVectors.tokens.app).toString("hex"),
    connectVectors.token_hashes.app,
  );
  assert.equal(
    tokenAuthHash("wallet", sidBytes, connectVectors.tokens.wallet).toString("hex"),
    connectVectors.token_hashes.wallet,
  );
  assert.equal(
    tokenAuthHash("management", sidBytes, connectVectors.tokens.management).toString("hex"),
    connectVectors.token_hashes.management,
  );
  const approval = connectVectors.approval;
  const preimage = approvalPreimage(
    preview,
    Buffer.from(approval.wallet_pk_hex, "hex"),
    approval.account_id,
    connectVectors.tokens.relay,
  );
  assert.equal(preimage.toString("hex"), approval.approve_preimage_hex);
  assert.equal(
    ed25519.verify(
      Buffer.from(approval.signature_hex, "hex"),
      preimage,
      Buffer.from(approval.account_public_key_hex, "hex"),
    ),
    true,
  );
});

test("Connect browser wallet signature encoder validates algorithm labels before byte encoding", () => {
  for (const target of ["../src/connect.browser.js", "../dist/connect.browser.js"]) {
    const source = readFileSync(new URL(target, import.meta.url), "utf8");
    assert.match(source, /normalizeWalletSignatureAlgorithmTag/);
    assert.match(source, /algorithm !== algorithm\.trim\(\)/);
    assert.match(source, /must not contain surrounding whitespace/);
    assert.doesNotMatch(source, /const normalized = algorithm\.trim\(\)/);
    assert.doesNotMatch(source, /Uint8Array\.of\(signature\.algorithm\)/);
  }
});

function approvalPreimage(preview, walletPublicKey, accountId, relayToken) {
  const constraints = encodeStruct([Buffer.from(preview.networkId.toBytes())]);
  return Buffer.concat([
    taggedApproveField("domain", APPROVE_DOMAIN),
    taggedApproveField("network_id", preview.networkId.toBytes()),
    taggedApproveField("constraints", blake2b(constraints, { dkLen: 32 })),
    taggedApproveField("sid", preview.sidBytes),
    taggedApproveField("app_pk", preview.appKeyPair.publicKey),
    taggedApproveField("wallet_pk", walletPublicKey),
    taggedApproveField("account_id", Buffer.from(accountId, "utf8")),
    taggedApproveField("relay_auth", relayAuthHash(preview, relayToken)),
  ]);
}

function encodeApproveControl(preview, walletPublicKey, accountId, accountPrivateKey, relayToken) {
  const signature = ed25519.sign(
    approvalPreimage(preview, walletPublicKey, accountId, relayToken),
    accountPrivateKey,
  );
  const body = encodeStruct([
    Buffer.from(walletPublicKey),
    encodeString(accountId),
    Buffer.from([0]),
    Buffer.from([0]),
    encodeWalletSignature({
      algorithm: 0,
      signature,
    }),
  ]);
  return Buffer.concat([u32(1), u64(body.length), body]);
}

function encodeRejectControl(codeId, reason, code = 401) {
  const body = encodeStruct([
    u16(code),
    encodeString(codeId),
    encodeString(reason),
  ]);
  return Buffer.concat([u32(2), u64(body.length), body]);
}

function encodeFrame({ sidBytes, dir, seq, kind }) {
  return encodeStruct([Buffer.from(sidBytes), encodeDir(dir), u64(seq), kind]);
}

function encodeControlFrame({ sidBytes, dir, seq, control }) {
  return encodeFrame({
    sidBytes,
    dir,
    seq,
    kind: Buffer.concat([u32(0), u64(control.length), control]),
  });
}

function noritoSchemaHash(typeName) {
  return Buffer.from(sha256(Buffer.concat([
    Buffer.from("norito:v1:type-name\0", "utf8"),
    Buffer.from(typeName, "utf8"),
  ]))).subarray(0, 16);
}

function crc64Table() {
  const poly = 0xc96c5795d7870f42n;
  const table = new Array(256);
  for (let byte = 0; byte < 256; byte += 1) {
    let crc = BigInt(byte);
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc & 1n) === 1n ? (crc >> 1n) ^ poly : crc >> 1n;
    }
    table[byte] = BigInt.asUintN(64, crc);
  }
  return table;
}

const CRC64_TABLE = crc64Table();

function crc64Ecma(bytes) {
  let crc = (1n << 64n) - 1n;
  for (const byte of bytes) {
    const index = Number((crc ^ BigInt(byte)) & 0xffn);
    crc = CRC64_TABLE[index] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ ((1n << 64n) - 1n));
}

function frameNorito(typeName, payload) {
  return Buffer.concat([
    Buffer.from("NRT0", "ascii"),
    Buffer.from([0, 0]),
    noritoSchemaHash(typeName),
    Buffer.from([0]),
    u64(payload.length),
    u64(crc64Ecma(payload)),
    Buffer.from([0]),
    payload,
  ]);
}

function deriveKeys(preview, walletPrivateKey) {
  const shared = x25519.getSharedSecret(walletPrivateKey, preview.appKeyPair.publicKey);
  const sessionKey = hkdf(sha256, shared, X25519_HKDF_SALT, X25519_HKDF_INFO, 32);
  const salt = blake2b(Buffer.concat([CONNECT_SALT_PREFIX, Buffer.from(preview.sidBytes)]), { dkLen: 32 });
  return {
    appKey: hkdf(sha256, sessionKey, salt, CONNECT_K_APP, 32),
    walletKey: hkdf(sha256, sessionKey, salt, CONNECT_K_WALLET, 32),
  };
}

function aad(preview, dir, seq) {
  return Buffer.concat([
    CONNECT_AAD_PREFIX,
    Buffer.from(preview.sidBytes),
    Buffer.from([dir]),
    u64(seq),
    Buffer.from([1]),
  ]);
}

function nonce(seq) {
  return Buffer.concat([Buffer.alloc(4), u64(seq)]);
}

function encodeSignResultOk(preview, keys, seq, signature, algorithm = 0) {
  const payload = Buffer.concat([
    u32(3),
    lenPrefixed(
      encodeWalletSignature({
        algorithm,
        signature,
      }),
    ),
  ]);
  const envelope = frameNorito(CONNECT_ENVELOPE_TYPE_NAME, encodeStruct([u64(seq), payload]));
  const ciphertext = Buffer.from(
    chacha20poly1305(keys.walletKey, nonce(seq), aad(preview, 1, seq)).encrypt(envelope),
  );
  const cipherStruct = encodeStruct([encodeDir(1), encodeBytes(ciphertext)]);
  return encodeFrame({
    sidBytes: preview.sidBytes,
    dir: 1,
    seq,
    kind: Buffer.concat([u32(1), u64(cipherStruct.length), cipherStruct]),
  });
}

function encodeSignResultErr(preview, keys, seq, code, message) {
  const payload = Buffer.concat([
    u32(4),
    lenPrefixed(encodeString(code)),
    lenPrefixed(encodeString(message)),
  ]);
  const envelope = frameNorito(CONNECT_ENVELOPE_TYPE_NAME, encodeStruct([u64(seq), payload]));
  const ciphertext = Buffer.from(
    chacha20poly1305(keys.walletKey, nonce(seq), aad(preview, 1, seq)).encrypt(envelope),
  );
  const cipherStruct = encodeStruct([encodeDir(1), encodeBytes(ciphertext)]);
  return encodeFrame({
    sidBytes: preview.sidBytes,
    dir: 1,
    seq,
    kind: Buffer.concat([u32(1), u64(cipherStruct.length), cipherStruct]),
  });
}

function encodeEncryptedClose(preview, keys, seq, reason) {
  const control = Buffer.concat([
    u32(0),
    lenPrefixed(
      Buffer.concat([
        u32(0),
        lenPrefixed(encodeRole(1)),
        lenPrefixed(u16(1001)),
        lenPrefixed(encodeString(reason)),
        lenPrefixed(Buffer.from([0])),
      ]),
    ),
  ]);
  const envelope = frameNorito(CONNECT_ENVELOPE_TYPE_NAME, encodeStruct([u64(seq), control]));
  const ciphertext = Buffer.from(
    chacha20poly1305(keys.walletKey, nonce(seq), aad(preview, 1, seq)).encrypt(envelope),
  );
  const cipherStruct = encodeStruct([encodeDir(1), encodeBytes(ciphertext)]);
  return encodeFrame({
    sidBytes: preview.sidBytes,
    dir: 1,
    seq,
    kind: Buffer.concat([u32(1), u64(cipherStruct.length), cipherStruct]),
  });
}

function decodeAppSignRequest(preview, keys, frameBytes) {
  let offset = 0;
  const readLen = () => {
    const length = Number(frameBytes.readBigUInt64LE(offset));
    offset += 8;
    return length;
  };
  const sidLength = readLen();
  offset += sidLength;
  const dirLength = readLen();
  offset += dirLength;
  const seqLength = readLen();
  const seq = Number(frameBytes.readBigUInt64LE(offset));
  offset += seqLength;
  const kindLength = readLen();
  const kindTag = frameBytes.readUInt32LE(offset);
  assert.equal(kindTag, 1);
  const kindBodyLength = Number(frameBytes.readBigUInt64LE(offset + 4));
  const kindBody = frameBytes.subarray(offset + 12, offset + 12 + kindBodyLength);
  const dirFieldLength = Number(kindBody.readBigUInt64LE(0));
  const aeadFieldLength = Number(kindBody.readBigUInt64LE(8 + dirFieldLength));
  const aeadField = kindBody.subarray(16 + dirFieldLength, 16 + dirFieldLength + aeadFieldLength);
  const aeadLength = Number(aeadField.readBigUInt64LE(0));
  const ciphertext = aeadField.subarray(8, 8 + aeadLength);
  const plaintext = Buffer.from(
    chacha20poly1305(keys.appKey, nonce(seq), aad(preview, 0, seq)).decrypt(ciphertext),
  );
  const payload = plaintext.subarray(40);
  let envOffset = 0;
  const envSeqLength = Number(payload.readBigUInt64LE(envOffset));
  envOffset += 8;
  const envSeq = Number(payload.readBigUInt64LE(envOffset));
  envOffset += envSeqLength;
  const payloadLength = Number(payload.readBigUInt64LE(envOffset));
  envOffset += 8;
  const payloadBytes = payload.subarray(envOffset, envOffset + payloadLength);
  const payloadTag = payloadBytes.readUInt32LE(0);
  let payloadOffset = 4;
  const readPayloadField = () => {
    const fieldLength = Number(payloadBytes.readBigUInt64LE(payloadOffset));
    payloadOffset += 8;
    const field = payloadBytes.subarray(payloadOffset, payloadOffset + fieldLength);
    payloadOffset += fieldLength;
    const valueLength = Number(field.readBigUInt64LE(0));
    return field.subarray(8, 8 + valueLength);
  };
  if (payloadTag === 1) {
    return {
      kind: "raw",
      seq: envSeq,
      domainTag: readPayloadField().toString("utf8"),
      bytes: readPayloadField(),
      kindLength,
    };
  }
  assert.equal(payloadTag, 2);
  return {
    kind: "transaction",
    seq: envSeq,
    txBytes: readPayloadField(),
    kindLength,
  };
}

function makePreview() {
  const appPrivateKey = new Uint8Array(32).fill(0x33);
  return createConnectSessionPreview({
    networkId: CANONICAL_NETWORK_ID,
    node: "https://taira.sora.org",
    nonce: new Uint8Array(16).fill(0x11),
    appKeyPair: {
      publicKey: x25519.getPublicKey(appPrivateKey),
      privateKey: appPrivateKey,
    },
  });
}

function makeAccount() {
  const privateKey = new Uint8Array(32).fill(0x77);
  const publicKey = ed25519.getPublicKey(privateKey);
  const accountId = AccountAddress.fromAccount({ publicKey, algorithm: "ed25519" }).toI105();
  return { accountId, privateKey, publicKey };
}

async function createApprovedTestSession({ permissions = null } = {}) {
  RecordingWebSocket.instances.length = 0;
  const preview = makePreview();
  const account = makeAccount();
  const relayToken = "relay-token";
  const walletPrivateKey = new Uint8Array(32).fill(0x55);
  const walletPublicKey = x25519.getPublicKey(walletPrivateKey);
  const keys = deriveKeys(preview, walletPrivateKey);
  const session = createConnectAppSession({
    baseUrl: "https://taira.sora.org",
    preview,
    session: {
      sid: preview.sidBase64Url,
      token_app: "token-app",
      token_relay: relayToken,
    },
    permissions,
    webSocketImpl: RecordingWebSocket,
  });
  const socket = RecordingWebSocket.instances[0];
  socket.open();
  socket.receive(
    encodeControlFrame({
      sidBytes: preview.sidBytes,
      dir: 1,
      seq: 1,
      control: encodeApproveControl(
        preview,
        walletPublicKey,
        account.accountId,
        account.privateKey,
        relayToken,
      ),
    }),
  );
  await session.waitForApproval();
  return { account, keys, preview, session, socket };
}

test("createConnectSessionPreview is deterministic with fixed nonce and keypair", () => {
  const options = {
    networkId: CANONICAL_NETWORK_ID,
    node: "https://taira.sora.org",
    nonce: new Uint8Array(16).fill(0x11),
    appKeyPair: {
      publicKey: new Uint8Array(32).fill(0x22),
      privateKey: new Uint8Array(32).fill(0x33),
    },
  };

  const first = createConnectSessionPreview(options);
  const second = createConnectSessionPreview(options);

  assert.equal(first.sidBase64Url, second.sidBase64Url);
  assert.equal(first.sidBase64Url.includes("="), false);
  assert.equal(toHex(first.nonce), "11".repeat(16));
  const walletUri = new URL(first.walletUri);
  const appUri = new URL(first.appUri);
  assert.equal(walletUri.searchParams.get("network_id"), CANONICAL_NETWORK_ID.toString());
  assert.equal(walletUri.searchParams.get("app_pk"), toBase64Url(first.appKeyPair.publicKey));
  assert.equal(walletUri.searchParams.get("nonce"), toBase64Url(first.nonce));
  assert.equal(walletUri.searchParams.get("role"), "wallet");
  assert.equal(appUri.searchParams.get("role"), "app");
  assert.equal(
    first.wsUrl,
    `wss://taira.sora.org/v1/connect/ws?sid=${first.sidBase64Url}&role=app`,
  );
});

test("createConnectSessionPreview rejects coercible non-byte array entries", () => {
  const appKeyPair = {
    publicKey: new Uint8Array(32).fill(0x22),
    privateKey: new Uint8Array(32).fill(0x33),
  };
  for (const entry of ["1", true, null]) {
    const nonce = new Array(16).fill(0x11);
    nonce[0] = entry;
    assert.throws(
      () => createConnectSessionPreview({ networkId: CANONICAL_NETWORK_ID, nonce, appKeyPair }),
      (error) => error instanceof TypeError && /nonce\[0\] must be a byte/.test(error.message),
    );
  }
});

test("buildConnectWebSocketUrl switches schemes for secure and insecure Torii urls", () => {
  assert.equal(
    buildConnectWebSocketUrl("https://taira.sora.org", "sid123", "app"),
    "wss://taira.sora.org/v1/connect/ws?sid=sid123&role=app",
  );
  assert.equal(
    buildConnectWebSocketUrl("http://127.0.0.1:8080", "sid123", "wallet"),
    "ws://127.0.0.1:8080/v1/connect/ws?sid=sid123&role=wallet",
  );
});

test("registerConnectSession posts the exact canonical session identity", async () => {
  const calls = [];
  const preview = makePreview();
  const response = await registerConnectSession("https://taira.sora.org", preview, {
    node: "https://taira.sora.org",
    fetchImpl: async (url, init) => {
      calls.push({ url: String(url), init });
      return new Response(
        JSON.stringify({
          sid: preview.sidBase64Url,
          network_id: CANONICAL_NETWORK_ID_JSON,
          app_pk: toBase64Url(preview.appKeyPair.publicKey),
          nonce: toBase64Url(preview.nonce),
          wallet_uri: `${preview.walletUri}&token=${REGISTER_TOKEN_WALLET}&relay=${REGISTER_TOKEN_RELAY}`,
          app_uri: `${preview.appUri}&token=${REGISTER_TOKEN_APP}&relay=${REGISTER_TOKEN_RELAY}`,
          token_app: REGISTER_TOKEN_APP,
          token_wallet: REGISTER_TOKEN_WALLET,
          token_management: REGISTER_TOKEN_MANAGEMENT,
          token_relay: REGISTER_TOKEN_RELAY,
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      );
    },
  });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://taira.sora.org/v1/connect/session");
  assert.equal(calls[0].init.method, "POST");
  assert.deepEqual(JSON.parse(calls[0].init.body), {
    sid: preview.sidBase64Url,
    network_id: CANONICAL_NETWORK_ID_JSON,
    app_pk: toBase64Url(preview.appKeyPair.publicKey),
    nonce: toBase64Url(preview.nonce),
    node: "https://taira.sora.org",
  });
  assert.equal(response.token_app, REGISTER_TOKEN_APP);
  assert.equal(response.network_id, preview.networkId.toString());
  assert.equal(
    new URL(response.wallet_uri).searchParams.get("network_id"),
    preview.networkId.toString(),
  );
});

test("registerConnectSession rejects raw or noncanonical typed NetworkId JSON", async () => {
  const preview = makePreview();
  const badChecksum = `${CANONICAL_NETWORK_ID_JSON.slice(0, -1)}${
    CANONICAL_NETWORK_ID_JSON.endsWith("0") ? "1" : "0"
  }`;
  for (const networkId of [
    preview.networkId.toString(),
    CANONICAL_NETWORK_ID_JSON.toLowerCase(),
    badChecksum,
  ]) {
    const response = {
      sid: preview.sidBase64Url,
      network_id: networkId,
      app_pk: toBase64Url(preview.appKeyPair.publicKey),
      nonce: toBase64Url(preview.nonce),
      wallet_uri: `${preview.walletUri}&token=${REGISTER_TOKEN_WALLET}&relay=${REGISTER_TOKEN_RELAY}`,
      app_uri: `${preview.appUri}&token=${REGISTER_TOKEN_APP}&relay=${REGISTER_TOKEN_RELAY}`,
      token_app: REGISTER_TOKEN_APP,
      token_wallet: REGISTER_TOKEN_WALLET,
      token_management: REGISTER_TOKEN_MANAGEMENT,
      token_relay: REGISTER_TOKEN_RELAY,
    };
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(
      registerConnectSession("https://taira.sora.org", preview, {
        fetchImpl: async () => new Response(JSON.stringify(response), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      }),
      /session\.network_id.*canonical marked Iroha NetworkId|checksum/u,
    );
  }
});

test("registerConnectSession rejects deep-link substitution and replay-shaped duplicates", async () => {
  const preview = makePreview();
  for (const mutate of [
    (response) => {
      response.wallet_uri += `&sid=${preview.sidBase64Url}`;
    },
    (response) => {
      const uri = new URL(response.app_uri);
      uri.searchParams.set("relay", "E".repeat(43));
      response.app_uri = uri.toString();
    },
  ]) {
    const response = {
      sid: preview.sidBase64Url,
      network_id: CANONICAL_NETWORK_ID_JSON,
      app_pk: toBase64Url(preview.appKeyPair.publicKey),
      nonce: toBase64Url(preview.nonce),
      wallet_uri: `${preview.walletUri}&token=${REGISTER_TOKEN_WALLET}&relay=${REGISTER_TOKEN_RELAY}`,
      app_uri: `${preview.appUri}&token=${REGISTER_TOKEN_APP}&relay=${REGISTER_TOKEN_RELAY}`,
      token_app: REGISTER_TOKEN_APP,
      token_wallet: REGISTER_TOKEN_WALLET,
      token_management: REGISTER_TOKEN_MANAGEMENT,
      token_relay: REGISTER_TOKEN_RELAY,
    };
    mutate(response);
    await assert.rejects(
      registerConnectSession("https://taira.sora.org", preview, {
        node: "https://taira.sora.org",
        fetchImpl: async () => new Response(JSON.stringify(response), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      }),
      /(substituted|duplicate)/u,
    );
  }
});

test("deleteConnectSession tolerates missing sessions and uses DELETE", async () => {
  const calls = [];
  await deleteConnectSession("https://taira.sora.org", "sid123", {
    tokenManagement: "management-token",
    fetchImpl: async (url, init) => {
      calls.push({ url: String(url), init });
      return new Response("", { status: 404 });
    },
  });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://taira.sora.org/v1/connect/session/sid123");
  assert.equal(calls[0].init.method, "DELETE");
  assert.equal(calls[0].init.headers.Authorization, "Bearer management-token");
});

test("resolveConnectLaunchUri prefers canonical session deeplinks", () => {
  assert.equal(
    resolveConnectLaunchUri(
      "wallet",
      {
        walletUri: "iroha://connect?sid=preview&role=wallet",
        appUri: "iroha://connect?sid=preview&role=app",
      },
      {
        wallet_uri: "iroha://connect?sid=session&role=wallet&token=wallet-token",
        app_uri: "iroha://connect?sid=session&role=app&token=app-token",
      },
    ),
    "iroha://connect?sid=session&role=wallet&token=wallet-token",
  );
});

test("rewriteConnectUriProtocol swaps the scheme without changing the session payload", () => {
  assert.equal(
    rewriteConnectUriProtocol(
      "iroha://connect?sid=session&role=wallet&token=wallet-token",
    ),
    "irohaconnect://connect?sid=session&role=wallet&token=wallet-token",
  );
  assert.equal(
    rewriteConnectUriProtocol(
      "iroha://connect?sid=session&role=wallet&token=wallet-token",
      "irohaconnect:",
    ),
    "irohaconnect://connect?sid=session&role=wallet&token=wallet-token",
  );
});

test("resolveConnectLaunchUriForProtocol rewrites the selected launch URI", () => {
  assert.equal(
    resolveConnectLaunchUriForProtocol(
      "wallet",
      {
        walletUri: "iroha://connect?sid=preview&role=wallet",
        appUri: "iroha://connect?sid=preview&role=app",
      },
      {
        wallet_uri: "iroha://connect?sid=session&role=wallet&token=wallet-token",
        app_uri: "iroha://connect?sid=session&role=app&token=app-token",
      },
    ),
    "irohaconnect://connect?sid=session&role=wallet&token=wallet-token",
  );
});

test("openConnectWebSocket sends the connect token as the first subprotocol", () => {
  RecordingWebSocket.instances.length = 0;
  const socket = openConnectWebSocket("https://taira.sora.org", "sid123", "token-app", "app", {
    webSocketImpl: RecordingWebSocket,
    protocols: ["iroha-connect"],
  });

  assert(socket instanceof RecordingWebSocket);
  assert.equal(RecordingWebSocket.instances.length, 1);
  assert.equal(
    RecordingWebSocket.instances[0].url,
    "wss://taira.sora.org/v1/connect/ws?sid=sid123&role=app",
  );
  assert.deepEqual(RecordingWebSocket.instances[0].protocols, [
    buildConnectTokenProtocol("token-app"),
    "iroha-connect",
  ]);
});

test("createConnectAppSession handles approval and sign success", async () => {
  RecordingWebSocket.instances.length = 0;
  const preview = makePreview();
  const account = makeAccount();
  const relayToken = "relay-token";
  const walletPrivateKey = new Uint8Array(32).fill(0x55);
  const walletPublicKey = x25519.getPublicKey(walletPrivateKey);
  const approvalSignature = ed25519.sign(
    approvalPreimage(preview, walletPublicKey, account.accountId, relayToken),
    account.privateKey,
  );
  const keys = deriveKeys(preview, walletPrivateKey);
  const session = createConnectAppSession({
    baseUrl: "https://taira.sora.org",
    preview,
    session: {
      sid: preview.sidBase64Url,
      token_app: "token-app",
      token_relay: relayToken,
    },
    webSocketImpl: RecordingWebSocket,
  });
  const socket = RecordingWebSocket.instances[0];
  socket.open();
  assert.equal(socket.sent.length, 1);
  socket.receive(
    encodeControlFrame({
      sidBytes: preview.sidBytes,
      dir: 1,
      seq: 1,
      control: encodeApproveControl(preview, walletPublicKey, account.accountId, account.privateKey, relayToken),
    }),
  );

  const approval = await session.waitForApproval();
  assert.equal(approval.accountId, account.accountId);
  assert.deepEqual(Buffer.from(approval.signingPublicKey), Buffer.from(account.publicKey));
  assert.deepEqual(Buffer.from(approval.walletPublicKey), Buffer.from(walletPublicKey));
  assert.deepEqual(Buffer.from(approval.signature), Buffer.from(approvalSignature));
  assert.equal(Object.isFrozen(approval), true);

  approval.signingPublicKey.fill(0);
  approval.walletPublicKey.fill(0);
  approval.signature.fill(0);
  assert.throws(() => {
    approval.accountId = "mutated-account";
  }, TypeError);
  const secondApproval = await session.waitForApproval();
  assert.notStrictEqual(secondApproval, approval);
  assert.notStrictEqual(secondApproval.signingPublicKey, approval.signingPublicKey);
  assert.notStrictEqual(secondApproval.walletPublicKey, approval.walletPublicKey);
  assert.notStrictEqual(secondApproval.signature, approval.signature);
  assert.equal(secondApproval.accountId, account.accountId);
  assert.deepEqual(
    Buffer.from(secondApproval.signingPublicKey),
    Buffer.from(account.publicKey),
  );
  assert.deepEqual(
    Buffer.from(secondApproval.walletPublicKey),
    Buffer.from(walletPublicKey),
  );
  assert.deepEqual(
    Buffer.from(secondApproval.signature),
    Buffer.from(approvalSignature),
  );
  assert.equal(session.approvedAccountId, account.accountId);

  const nexusApproval = await new NexusAppClient().awaitApproval({
    sid: preview.sidBase64Url,
    appSession: session,
  });
  assert.equal(nexusApproval.accountId, account.accountId);
  assert.deepEqual(nexusApproval.signingPublicKey, Buffer.from(account.publicKey));

  const signPromise = session.signTransaction(Buffer.from([0xaa, 0xbb, 0xcc]));
  await Promise.resolve();
  assert.equal(socket.sent.length, 2);
  const signRequest = decodeAppSignRequest(preview, keys, socket.sent[1]);
  assert.deepEqual([...signRequest.txBytes.values()], [0xaa, 0xbb, 0xcc]);

  const signature = Buffer.alloc(64, 0x77);
  socket.receive(encodeSignResultOk(preview, keys, signRequest.seq, signature));
  const detached = await signPromise;
  assert.deepEqual(Buffer.from(detached), signature);
});

test("createConnectAppSession requests sign_raw permission and signs raw bytes under the exact domain", async () => {
  const { account, keys, preview, session, socket } = await createApprovedTestSession({
    permissions: {
      methods: ["sign_raw"],
      resources: [TORII_CANONICAL_REQUEST_DOMAIN_TAG],
    },
  });
  assert.equal(socket.sent[0].includes(Buffer.from("sign_raw", "utf8")), true);
  assert.equal(
    socket.sent[0].includes(Buffer.from(TORII_CANONICAL_REQUEST_DOMAIN_TAG, "utf8")),
    true,
  );

  const message = Buffer.from("POST\n/v1/multisig/spec\n\nbody-hash\n123\nnonce", "utf8");
  const signPromise = session.signRaw(TORII_CANONICAL_REQUEST_DOMAIN_TAG, message);
  await Promise.resolve();
  const signRequest = decodeAppSignRequest(preview, keys, socket.sent[1]);
  assert.equal(signRequest.kind, "raw");
  assert.equal(signRequest.domainTag, TORII_CANONICAL_REQUEST_DOMAIN_TAG);
  assert.deepEqual(signRequest.bytes, message);

  const signature = ed25519.sign(message, account.privateKey);
  socket.receive(encodeSignResultOk(preview, keys, signRequest.seq, signature));
  assert.deepEqual(Buffer.from(await signPromise), Buffer.from(signature));
});

test("createConnectAppSession rejects a raw signature that is not bound to the approved identity", async () => {
  const { keys, preview, session, socket } = await createApprovedTestSession();
  const message = Buffer.from("canonical request", "utf8");
  const signPromise = session.signRaw(TORII_CANONICAL_REQUEST_DOMAIN_TAG, message);
  await Promise.resolve();
  const signRequest = decodeAppSignRequest(preview, keys, socket.sent[1]);
  const otherPrivateKey = new Uint8Array(32).fill(0x42);
  socket.receive(
    encodeSignResultOk(
      preview,
      keys,
      signRequest.seq,
      ed25519.sign(message, otherPrivateKey),
    ),
  );

  await assert.rejects(signPromise, (error) => {
    assert.ok(error instanceof ConnectSignRequestError);
    assert.equal(error.code, "INVALID_SIGNATURE");
    return true;
  });
});

test("createConnectAppSession shares one stable in-flight gate before approval", async () => {
  RecordingWebSocket.instances.length = 0;
  const preview = makePreview();
  const account = makeAccount();
  const relayToken = "relay-token";
  const walletPrivateKey = new Uint8Array(32).fill(0x55);
  const walletPublicKey = x25519.getPublicKey(walletPrivateKey);
  const keys = deriveKeys(preview, walletPrivateKey);
  const session = createConnectAppSession({
    baseUrl: "https://taira.sora.org",
    preview,
    session: {
      sid: preview.sidBase64Url,
      token_app: "token-app",
      token_relay: relayToken,
    },
    webSocketImpl: RecordingWebSocket,
  });
  const socket = RecordingWebSocket.instances[0];
  socket.open();
  const message = Buffer.from("pending canonical request", "utf8");
  const rawPromise = session.signRaw(TORII_CANONICAL_REQUEST_DOMAIN_TAG, message);
  await assert.rejects(
    session.signTransaction(Buffer.from([0xaa])),
    (error) => {
      assert.ok(error instanceof ConnectSignRequestError);
      assert.equal(error.code, "REQUEST_IN_FLIGHT");
      return true;
    },
  );

  socket.receive(
    encodeControlFrame({
      sidBytes: preview.sidBytes,
      dir: 1,
      seq: 1,
      control: encodeApproveControl(
        preview,
        walletPublicKey,
        account.accountId,
        account.privateKey,
        relayToken,
      ),
    }),
  );
  await session.waitForApproval();
  await Promise.resolve();
  const signRequest = decodeAppSignRequest(preview, keys, socket.sent[1]);
  socket.receive(
    encodeSignResultOk(
      preview,
      keys,
      signRequest.seq,
      ed25519.sign(message, account.privateKey),
    ),
  );
  await rawPromise;
});

test("createConnectAppSession rejects padded raw-sign domain tags before sending", async () => {
  const { session, socket } = await createApprovedTestSession();
  await assert.rejects(
    session.signRaw(` ${TORII_CANONICAL_REQUEST_DOMAIN_TAG}`, Buffer.from([0xaa])),
    /domainTag must not contain surrounding whitespace/u,
  );
  assert.equal(socket.sent.length, 1);
});

test("createConnectCanonicalRequestAuth signs the exact canonical message with the approved identity", async () => {
  const account = makeAccount();
  let captured = null;
  const auth = await createConnectCanonicalRequestAuth({
    async waitForApproval() {
      return { accountId: account.accountId };
    },
    async signRaw(domainTag, bytes) {
      captured = { domainTag, bytes: Uint8Array.from(bytes) };
      return ed25519.sign(bytes, account.privateKey);
    },
  });
  assert.equal(Object.isFrozen(auth), true);
  assert.equal(auth.authAccountId, account.accountId);

  const body = { multisig_account_id: account.accountId };
  const timestampMs = 123456;
  const nonce = "canonical-nonce";
  const request = await buildCanonicalJsonRequest({
    accountId: auth.authAccountId,
    networkId: CANONICAL_NETWORK_ID,
    method: "POST",
    path: "/v1/multisig/spec",
    body,
    sign: auth.sign,
    timestampMs,
    nonce,
  });
  const expectedMessage = canonicalRequestSignatureMessage({
    networkId: CANONICAL_NETWORK_ID,
    method: "POST",
    path: "/v1/multisig/spec",
    body: JSON.stringify(body),
    timestampMs,
    nonce,
  });
  assert.equal(captured.domainTag, TORII_CANONICAL_REQUEST_DOMAIN_TAG);
  assert.deepEqual(Buffer.from(captured.bytes), expectedMessage);
  assert.equal(
    request.headers["X-Iroha-Signature"],
    Buffer.from(ed25519.sign(expectedMessage, account.privateKey)).toString("base64"),
  );
});

test("createConnectCanonicalRequestAuth rejects invalid wallet signatures", async () => {
  const account = makeAccount();
  const auth = await createConnectCanonicalRequestAuth({
    async waitForApproval() {
      return { accountId: account.accountId };
    },
    async signRaw() {
      return new Uint8Array(64);
    },
  });
  await assert.rejects(
    auth.sign({ message: Buffer.from("canonical request", "utf8") }),
    (error) => {
      assert.ok(error instanceof ConnectSignRequestError);
      assert.equal(error.code, "INVALID_SIGNATURE");
      return true;
    },
  );
});

test("createConnectAppSession rejects duplicate approvals without replacing identity", async () => {
  RecordingWebSocket.instances.length = 0;
  const preview = makePreview();
  const account = makeAccount();
  const relayToken = "relay-token";
  const walletPrivateKey = new Uint8Array(32).fill(0x55);
  const walletPublicKey = x25519.getPublicKey(walletPrivateKey);
  const session = createConnectAppSession({
    baseUrl: "https://taira.sora.org",
    preview,
    session: {
      sid: preview.sidBase64Url,
      token_app: "token-app",
      token_relay: relayToken,
    },
    webSocketImpl: RecordingWebSocket,
  });
  const socket = RecordingWebSocket.instances[0];
  socket.open();
  socket.receive(
    encodeControlFrame({
      sidBytes: preview.sidBytes,
      dir: 1,
      seq: 1,
      control: encodeApproveControl(
        preview,
        walletPublicKey,
        account.accountId,
        account.privateKey,
        relayToken,
      ),
    }),
  );
  const first = await session.waitForApproval();

  const replacementPrivateKey = new Uint8Array(32).fill(0x66);
  const replacementPublicKey = ed25519.getPublicKey(replacementPrivateKey);
  const replacementAccountId = AccountAddress.fromAccount({
    publicKey: replacementPublicKey,
    algorithm: "ed25519",
  }).toI105();
  const replacementWalletPrivateKey = new Uint8Array(32).fill(0x44);
  socket.receive(
    encodeControlFrame({
      sidBytes: preview.sidBytes,
      dir: 1,
      seq: 2,
      control: encodeApproveControl(
        preview,
        x25519.getPublicKey(replacementWalletPrivateKey),
        replacementAccountId,
        replacementPrivateKey,
        relayToken,
      ),
    }),
  );
  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(first.accountId, account.accountId);
  assert.equal(session.approvedAccountId, account.accountId);
  assert.equal(socket.readyState, 3);
  await assert.rejects(session.waitForApproval(), (error) => {
    assert.ok(error instanceof ConnectSessionClosedError);
    assert.match(error.message, /more than one wallet approval/u);
    return true;
  });
  await assert.rejects(
    session.signTransaction(Buffer.from([0xaa])),
    /more than one wallet approval/u,
  );
});

test("createConnectAppSession rejects unsupported wallet signature algorithm tags", async () => {
  RecordingWebSocket.instances.length = 0;
  const preview = makePreview();
  const account = makeAccount();
  const relayToken = "relay-token";
  const walletPrivateKey = new Uint8Array(32).fill(0x12);
  const walletPublicKey = x25519.getPublicKey(walletPrivateKey);
  const keys = deriveKeys(preview, walletPrivateKey);
  const session = createConnectAppSession({
    baseUrl: "https://taira.sora.org",
    preview,
    session: {
      sid: preview.sidBase64Url,
      token_app: "token-app",
      token_relay: relayToken,
    },
    webSocketImpl: RecordingWebSocket,
  });
  const socket = RecordingWebSocket.instances[0];
  socket.open();
  socket.receive(
    encodeControlFrame({
      sidBytes: preview.sidBytes,
      dir: 1,
      seq: 1,
      control: encodeApproveControl(preview, walletPublicKey, account.accountId, account.privateKey, relayToken),
    }),
  );

  await session.waitForApproval();
  const signPromise = session.signTransaction(Buffer.from([0xaa]));
  await Promise.resolve();
  const signRequest = decodeAppSignRequest(preview, keys, socket.sent[1]);
  socket.receive(encodeSignResultOk(preview, keys, signRequest.seq, Buffer.alloc(64), 1));

  await assert.rejects(signPromise, (error) => {
    assert.ok(error instanceof ConnectSignRequestError);
    assert.equal(error.code, "UNSUPPORTED_ALGORITHM");
    assert.match(error.message, /unsupported wallet signature algorithm 1/);
    return true;
  });
});

test("createConnectAppSession surfaces wallet rejection", async () => {
  RecordingWebSocket.instances.length = 0;
  const preview = makePreview();
  const session = createConnectAppSession({
    baseUrl: "https://taira.sora.org",
    preview,
    session: {
      sid: preview.sidBase64Url,
      token_app: "token-app",
      token_relay: "relay-token",
    },
    webSocketImpl: RecordingWebSocket,
  });
  const socket = RecordingWebSocket.instances[0];
  socket.open();
  socket.receive(
    encodeControlFrame({
      sidBytes: preview.sidBytes,
      dir: 1,
      seq: 1,
      control: encodeRejectControl("USER_DENIED", "wallet rejected the session"),
    }),
  );

  await assert.rejects(() => session.waitForApproval(), (error) => {
    assert(error instanceof ConnectApprovalRejectedError);
    assert.equal(error.codeId, "USER_DENIED");
    return true;
  });
});

test("createConnectAppSession surfaces wallet sign errors", async () => {
  RecordingWebSocket.instances.length = 0;
  const preview = makePreview();
  const account = makeAccount();
  const relayToken = "relay-token";
  const walletPrivateKey = new Uint8Array(32).fill(0x55);
  const walletPublicKey = x25519.getPublicKey(walletPrivateKey);
  const keys = deriveKeys(preview, walletPrivateKey);
  const session = createConnectAppSession({
    baseUrl: "https://taira.sora.org",
    preview,
    session: {
      sid: preview.sidBase64Url,
      token_app: "token-app",
      token_relay: relayToken,
    },
    webSocketImpl: RecordingWebSocket,
  });
  const socket = RecordingWebSocket.instances[0];
  socket.open();
  socket.receive(
    encodeControlFrame({
      sidBytes: preview.sidBytes,
      dir: 1,
      seq: 1,
      control: encodeApproveControl(preview, walletPublicKey, account.accountId, account.privateKey, relayToken),
    }),
  );
  await session.waitForApproval();

  const signPromise = session.signTransaction(Buffer.from([0xaa]));
  await Promise.resolve();
  const signRequest = decodeAppSignRequest(preview, keys, socket.sent[1]);
  socket.receive(
    encodeSignResultErr(
      preview,
      keys,
      signRequest.seq,
      "USER_DENIED",
      "wallet refused to sign",
    ),
  );

  await assert.rejects(() => signPromise, (error) => {
    assert(error instanceof ConnectSignRequestError);
    assert.equal(error.code, "USER_DENIED");
    return true;
  });
});

test("createConnectAppSession surfaces encrypted close frames", async () => {
  RecordingWebSocket.instances.length = 0;
  const preview = makePreview();
  const account = makeAccount();
  const relayToken = "relay-token";
  const walletPrivateKey = new Uint8Array(32).fill(0x55);
  const walletPublicKey = x25519.getPublicKey(walletPrivateKey);
  const keys = deriveKeys(preview, walletPrivateKey);
  const session = createConnectAppSession({
    baseUrl: "https://taira.sora.org",
    preview,
    session: {
      sid: preview.sidBase64Url,
      token_app: "token-app",
      token_relay: relayToken,
    },
    webSocketImpl: RecordingWebSocket,
  });
  const socket = RecordingWebSocket.instances[0];
  socket.open();
  socket.receive(
    encodeControlFrame({
      sidBytes: preview.sidBytes,
      dir: 1,
      seq: 1,
      control: encodeApproveControl(preview, walletPublicKey, account.accountId, account.privateKey, relayToken),
    }),
  );
  await session.waitForApproval();

  const signPromise = session.signTransaction(Buffer.from([0xaa]));
  await Promise.resolve();
  const signRequest = decodeAppSignRequest(preview, keys, socket.sent[1]);
  socket.receive(encodeEncryptedClose(preview, keys, signRequest.seq, "wallet session closed"));

  await assert.rejects(() => signPromise, (error) => {
    assert(error instanceof ConnectSessionClosedError);
    assert.equal(error.reason, "wallet session closed");
    return true;
  });
});
