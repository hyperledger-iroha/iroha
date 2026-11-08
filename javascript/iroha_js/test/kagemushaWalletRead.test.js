import assert from "node:assert/strict";
import test from "node:test";
import { createPublicKey, verify } from "node:crypto";
import { canonicalRequestSignatureMessage } from "../src/canonicalRequest.js";
import { ToriiClient, LocalSigningContext } from "../src/toriiClient.js";
import { AccountAddress } from "../src/address.js";
import { NetworkId } from "../src/networkId.js";
import { kagemushaWalletReadSelection } from "../src/kagemushaWalletRead.js";

const network = NetworkId.fromBytes(new Uint8Array(32).fill(0xab));
const key = Buffer.from("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60", "hex");
const publicKey = Buffer.from("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a", "hex");
const load = { schemeId: "11".repeat(32), walletId: "22".repeat(32), requestId: "33".repeat(32) };
const ledger = { schemeId: load.schemeId, walletId: load.walletId, entrypointHash: "ab".repeat(32) };
const cases = [
  ["LoadIssuance", load, `loads/${load.requestId}`, 65_536],
  ["LoadFinality", load, `loads/${load.requestId}/finality`, 262_144],
  ["LoadEpoch", { ...load, boundary: "2" }, `loads/${load.requestId}/epochs/2`, 262_144],
  ["LedgerFinality", ledger, `ledger/${ledger.entrypointHash}/finality`, 1_327_104],
  ["LedgerEpoch", { ...ledger, boundary: "18446744073709551615" }, `ledger/${ledger.entrypointHash}/epochs/18446744073709551615`, 262_144],
];
const original = Uint8Array.of(1, 4, 7, 9); // Transport only; deliberately not a verified proof.
function auth() { return { canonicalAuth: { accountId: AccountAddress.fromAccount({ publicKey }).toI105(753), privateKey: key } }; }
function client(fetchImpl, opts = {}) {
  return new ToriiClient("https://wallet.invalid", { localSigningContext: new LocalSigningContext(network, 753), fetchImpl, maxRetries: 3,
    defaultHeaders: { Authorization: "must-not-leak", "X-Iroha-Witness": "must-not-leak" }, ...opts });
}
function response(bytes = original, headers = {}) { return new Response(bytes, { headers: { "Content-Type": "application/x-norito", ...headers } }); }

test("current wallet originals use five exact authenticated, non-replayed private routes", async () => {
  for (const [name, selection, suffix] of cases) {
    let calls = 0;
    const opts = auth();
    const c = client(async (url, init) => {
      calls += 1;
      assert.equal(url, `https://wallet.invalid/v1/kagemusha/${load.schemeId}/wallets/${load.walletId}/${suffix}`);
      assert.equal(init.method, "GET"); assert.equal(init.body, undefined);
      assert.equal(init.redirect, "error"); assert.equal(init.credentials, "omit"); assert.equal(init.cache, "no-store");
      const h = new Headers(init.headers);
      assert.equal(h.get("accept"), "application/x-norito"); assert.equal(h.get("accept-encoding"), "identity");
      assert.equal(h.get("authorization"), null); assert.equal(h.get("x-iroha-witness"), null);
      assert.equal(h.get("x-iroha-account"), AccountAddress.fromAccount({ publicKey }).canonicalHex());
      const message = canonicalRequestSignatureMessage({ networkId: network, method: "GET", path: new URL(url).pathname, timestampMs: Number(h.get("x-iroha-timestamp-ms")), nonce: h.get("x-iroha-nonce") });
      assert.ok(verify(null, message, createPublicKey({ key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), publicKey]), format: "der", type: "spki" }), Buffer.from(h.get("x-iroha-signature"), "base64")));
      assert.ok(h.get("x-iroha-signature")); assert.ok(h.get("x-iroha-nonce"));
      return response(original, { "Content-Length": String(original.length) });
    });
    const result = await c[`getKagemushaWallet${name}OriginalV1`](selection, opts);
    assert.deepEqual(result.selection, selection); assert.equal(result.networkId, network);
    assert.equal(result.accountId, opts.canonicalAuth.accountId); assert.deepEqual(result.canonicalResponseOriginal, original);
    assert.equal(calls, 1); assert.equal(Object.hasOwn(result, "verified"), false);
  }
});

test("selectors reject aliases, old fields, noncanonical hashes and epoch boundaries before fetch", () => {
  for (const bad of ["00".repeat(32), "AB".repeat(32), "ab", ` ${load.schemeId}`, "../secret"]) {
    assert.throws(() => kagemushaWalletReadSelection("load", { ...load, schemeId: bad }));
  }
  assert.throws(() => kagemushaWalletReadSelection("load", { ...load, top_up: true }));
  assert.throws(() => kagemushaWalletReadSelection("ledgerFinality", { ...ledger, entrypointHash: "aa".repeat(32) }), /marker/);
  for (const boundary of [0, 2, "0", "1", "02", "-2", "18446744073709551616", "2.0"]) {
    assert.throws(() => kagemushaWalletReadSelection("loadEpoch", { ...load, boundary }));
  }
});

test("private evidence requires local exact-network context, explicit I105 signer and HTTPS", async () => {
  let calls = 0;
  const fetchImpl = async () => { calls += 1; return response(); };
  await assert.rejects(new ToriiClient("https://wallet.invalid", { fetchImpl }).getKagemushaWalletLoadIssuanceOriginalV1(load, auth()), /localSigningContext/);
  await assert.rejects(client(fetchImpl).getKagemushaWalletLoadIssuanceOriginalV1(load, {}), /canonicalAuth/);
  await assert.rejects(client(fetchImpl).getKagemushaWalletLoadIssuanceOriginalV1(load, { canonicalAuth: { ...auth().canonicalAuth, accountId: "alice@wonderland" } }));
  await assert.rejects(client(fetchImpl, { localSigningContext: new LocalSigningContext(network, 0) }).getKagemushaWalletLoadIssuanceOriginalV1(load, auth()), /exact canonical/);
  for (const url of ["http://wallet.invalid", "https://wallet.invalid/v1", "https://wallet.invalid?x=1", "https://x:y@wallet.invalid"]) {
    await assert.rejects(new ToriiClient(url, { localSigningContext: new LocalSigningContext(network, 753), fetchImpl, allowInsecure: true }).getKagemushaWalletLoadIssuanceOriginalV1(load, auth()));
  }
  assert.equal(calls, 0);
});

test("originals reject wrong media, compression, empty, truncated, oversize and redirected responses", async () => {
  const responses = [
    () => response(original, { "Content-Type": "application/json" }),
    () => response(original, { "Content-Type": "application/x-norito; charset=utf-8" }),
    () => response(original, { "Content-Encoding": "gzip" }),
    () => response(new Uint8Array()),
    () => response(original, { "Content-Length": "5" }),
    () => response(original, { "Content-Length": "04" }),
    () => response(new Uint8Array(65_537)),
    () => { const r = response(); Object.defineProperty(r, "url", { value: "https://evil.invalid" }); return r; },
    () => { const r = response(); Object.defineProperty(r, "redirected", { value: true }); return r; },
  ];
  for (const make of responses) await assert.rejects(client(async () => make()).getKagemushaWalletLoadIssuanceOriginalV1(load, auth()));
  for (const [name, selection, , bound] of cases) {
    await assert.rejects(client(async () => response(original, { "Content-Length": String(bound + 1) }))[`getKagemushaWallet${name}OriginalV1`](selection, auth()));
  }
});

test("404 and 503 remain errors without retries or a fabricated settled state", async () => {
  for (const status of [404, 503]) {
    let calls = 0;
    const c = client(async () => { calls += 1; return new Response('{"code":"kagemusha_ledger_finality_unavailable"}', { status, headers: { "Content-Type": "application/json" } }); });
    await assert.rejects(c.getKagemushaWalletLedgerFinalityOriginalV1(ledger, auth()), (error) => error.status === status);
    assert.equal(calls, 1);
  }
});

test("abort and original deadline cover headers and stalled response bodies", async () => {
  const abort = new AbortController(); abort.abort();
  let called = false;
  await assert.rejects(client(async () => { called = true; return response(); }).getKagemushaWalletLoadIssuanceOriginalV1(load, { ...auth(), signal: abort.signal }));
  assert.equal(called, false);
  let cancelled = false;
  const body = new ReadableStream({ cancel() { cancelled = true; } });
  await assert.rejects(client(async () => response(body), { timeoutMs: 25 }).getKagemushaWalletLoadIssuanceOriginalV1(load, auth()));
  assert.equal(cancelled, true);
});
