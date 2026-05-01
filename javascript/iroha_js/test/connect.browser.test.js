import assert from "node:assert/strict";
import test from "node:test";
import { ed25519, x25519 } from "@noble/curves/ed25519";
import { chacha20poly1305 } from "@noble/ciphers/chacha";
import { blake2b } from "@noble/hashes/blake2b";
import { hkdf } from "@noble/hashes/hkdf";
import { sha256 } from "@noble/hashes/sha2";

import {
  ConnectApprovalRejectedError,
  ConnectSessionClosedError,
  ConnectSignRequestError,
  buildConnectTokenProtocol,
  buildConnectWebSocketUrl,
  createConnectAppSession,
  createConnectSessionPreview,
  deleteConnectSession,
  openConnectWebSocket,
  registerConnectSession,
  resolveConnectLaunchUri,
  resolveConnectLaunchUriForProtocol,
  rewriteConnectUriProtocol,
  toHex,
} from "../src/connect.browser.js";
import { AccountAddress } from "../src/address.js";

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

function approvalPreimage(preview, walletPublicKey, accountId, relayToken) {
  return Buffer.concat([
    taggedApproveField("domain", APPROVE_DOMAIN),
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
  let hash = 0xcbf29ce484222325n;
  for (const byte of Buffer.from(typeName, "utf8")) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  const low = u64(hash);
  return Buffer.concat([low, low]);
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

function encodeSignResultOk(preview, keys, seq, signature) {
  const payload = Buffer.concat([
    u32(3),
    lenPrefixed(
      encodeWalletSignature({
        algorithm: 0,
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
  assert.equal(payloadBytes.readUInt32LE(0), 2);
  const txFieldLength = Number(payloadBytes.readBigUInt64LE(4));
  const txField = payloadBytes.subarray(12, 12 + txFieldLength);
  const txLength = Number(txField.readBigUInt64LE(0));
  return {
    seq: envSeq,
    txBytes: txField.subarray(8, 8 + txLength),
    kindLength,
  };
}

function makePreview() {
  const appPrivateKey = new Uint8Array(32).fill(0x33);
  return createConnectSessionPreview({
    chainId: "alpha-net",
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

test("createConnectSessionPreview is deterministic with fixed nonce and keypair", () => {
  const options = {
    chainId: "alpha-net",
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
  assert.equal(first.walletUri, `iroha://connect?sid=${first.sidBase64Url}&chain_id=alpha-net&v=1&role=wallet&node=https%3A%2F%2Ftaira.sora.org`);
  assert.equal(first.appUri, `iroha://connect?sid=${first.sidBase64Url}&chain_id=alpha-net&v=1&role=app&node=https%3A%2F%2Ftaira.sora.org`);
  assert.equal(
    first.wsUrl,
    `wss://taira.sora.org/v1/connect/ws?sid=${first.sidBase64Url}&role=app`,
  );
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

test("registerConnectSession posts sid and node directly to Torii", async () => {
  const calls = [];
  const response = await registerConnectSession("https://taira.sora.org", "sid123", {
    node: "https://taira.sora.org",
    fetchImpl: async (url, init) => {
      calls.push({ url: String(url), init });
      return new Response(
        JSON.stringify({
          sid: "sid123",
          wallet_uri: "iroha://connect?sid=sid123&role=wallet&token=wallet-token",
          app_uri: "iroha://connect?sid=sid123&role=app&token=app-token",
          token_app: "app-token",
          token_wallet: "wallet-token",
          token_management: "management-token",
          token_relay: "relay-token",
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
  assert.equal(calls[0].init.body, JSON.stringify({ sid: "sid123", node: "https://taira.sora.org" }));
  assert.equal(response.token_app, "app-token");
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
