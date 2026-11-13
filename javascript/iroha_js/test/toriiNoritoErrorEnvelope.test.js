// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

// Routes that negotiate `application/x-norito` receive Torii's failures as one
// canonical binary `ErrorEnvelope` frame. Each route family must surface the
// envelope's code and exact message after one dispatch, and keep its previous
// error for a body that is not such an envelope.
import assert from "node:assert/strict";
import test from "node:test";

import { AccountAddress } from "../src/address.js";
import { NetworkId } from "../src/networkId.js";
import { NoritoRpcClient, NoritoRpcError } from "../src/noritoRpcClient.js";
import { fetchPrivacyExact12CapabilityManifestTransportV1 } from "../src/privacyCapabilityTransport.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import { LocalSigningContext, ToriiClient, ToriiHttpError } from "../src/toriiClient.js";
import { isIdempotentSubmissionRejection } from "../src/toriiErrors.js";
import { TORII_TEST_NATIVE_BINDING } from "../src/toriiTestHooks.js";
import {
  errorEnvelopeContract,
  errorEnvelopeDetailsContract,
  errorEnvelopeFrame,
  jsonErrorEnvelope,
} from "./helpers/toriiErrorEnvelope.js";

const NORITO = "application/x-norito";

function noritoReply(body, status, headers = {}) {
  return new Response(body, { status, headers: { "Content-Type": NORITO, ...headers } });
}

// A flipped checksum byte leaves a frame that is not a canonical envelope.
function corrupted(frame) {
  const bytes = Buffer.from(frame);
  bytes[31] ^= 1;
  return bytes;
}

function typedEnvelope(status, code, message) {
  return (error) => {
    assert.ok(error instanceof ToriiHttpError, String(error));
    assert.equal(error.status, status);
    assert.equal(error.code, code);
    assert.equal(error.envelopeCode, code);
    assert.equal(error.rejectCode, null);
    assert.equal(error.errorMessage, message);
    assert.equal(error.details, null);
    assert.equal(error.bodyText, null);
    assert.equal(error.bodyJson, null);
    assert.ok(error.message.endsWith(`: ${code} — ${message}`), error.message);
    return true;
  };
}

// The unchanged failure for a binary body: no code or message can be read.
function opaqueStatus(status) {
  return (error) => {
    assert.ok(error instanceof ToriiHttpError, String(error));
    assert.equal(error.status, status);
    assert.equal(error.code, null);
    assert.equal(error.errorMessage, null);
    assert.equal(error.bodyText, null);
    return true;
  };
}

test("Norito error envelope builder reproduces every Rust-authored ErrorEnvelope frame", () => {
  const frames = errorEnvelopeContract.responses.filter(({ media_type: type }) => type === NORITO);
  assert.equal(frames.length, 7);
  for (const { code, body_hex: hex } of frames) {
    assert.deepEqual(errorEnvelopeFrame(code, code), Buffer.from(hex, "hex"));
  }
  const json = errorEnvelopeContract.responses.find(({ media_type: type }) => type === "application/json");
  assert.deepEqual(jsonErrorEnvelope(json.code, json.code), Buffer.from(json.body_utf8));
});

test("SoraFS PoR reads surface a Norito error envelope once and keep other bodies opaque", async () => {
  const code = "sorafs_por_report_not_found";
  const message = "no PoR report is published for 2026-W05";
  const reads = [
    (client) => client.getSorafsPorStatus(),
    (client) => client.exportSorafsPorStatus(),
    (client) => client.getSorafsPorWeeklyReport("2026-W05"),
  ];
  for (const read of reads) {
    for (const [body, check] of [
      [errorEnvelopeFrame(code, message), typedEnvelope(404, code, message)],
      [corrupted(errorEnvelopeFrame(code, message)), opaqueStatus(404)],
    ]) {
      let calls = 0;
      const client = new ToriiClient("https://torii.invalid", {
        fetchImpl: async (_url, init) => {
          calls += 1;
          assert.equal(new Headers(init.headers).get("accept"), NORITO);
          return noritoReply(body, 404);
        },
      });
      await assert.rejects(read(client), check);
      assert.equal(calls, 1);
    }
  }
});

test("SoraFS billing statement surfaces a Norito error envelope once", async () => {
  const code = "sorafs_billing_checkpoint_mismatch";
  const message = "statement was published under a different checkpoint";
  const canonicalAuth = { accountId: "billing-reader@sora", privateKey: Buffer.alloc(32, 29) };
  for (const [body, check] of [
    [errorEnvelopeFrame(code, message), typedEnvelope(409, code, message)],
    [corrupted(errorEnvelopeFrame(code, message)), opaqueStatus(409)],
  ]) {
    let calls = 0;
    const client = new ToriiClient("https://torii.example", {
      localSigningContext: new LocalSigningContext(NetworkId.fromBytes(Buffer.alloc(32, 0xa5)), 753),
      fetchImpl: async () => {
        calls += 1;
        return noritoReply(body, 409);
      },
    });
    await assert.rejects(
      client.getSorafsBillingStatement("22".repeat(32), "11".repeat(32), { canonicalAuth }),
      check,
    );
    assert.equal(calls, 1);
  }
});

test("transaction submission surfaces a Norito error envelope once", async () => {
  const code = "invalid_transaction_payload";
  const message = "signed transaction bytes are not one canonical VersionedSignedTransaction";
  for (const [body, check] of [
    [errorEnvelopeFrame(code, message), typedEnvelope(400, code, message)],
    [corrupted(errorEnvelopeFrame(code, message)), opaqueStatus(400)],
  ]) {
    let calls = 0;
    const client = new ToriiClient("https://torii.invalid", {
      maxRetries: 3,
      retryMethods: ["POST"],
      [TORII_TEST_NATIVE_BINDING]: { encodeSignedTransactionVersioned: (payload) => Buffer.from(payload) },
      fetchImpl: async (url, init) => {
        calls += 1;
        assert.equal(url, "https://torii.invalid/v1/pipeline/transactions");
        assert.equal(new Headers(init.headers).get("accept"), "application/x-norito, application/json");
        return noritoReply(body, 400);
      },
    });
    client._ensureDataModelValidation = async () => {};
    await assert.rejects(client.submitTransaction(Uint8Array.of(1, 0xde, 0xad)), check);
    assert.equal(calls, 1);
  }
});

// Torii's headers for a queue rejection or a 429/503: the reject code when the
// details name one, and Retry-After whenever they carry a retry delay.
function toriiHeadersFor(entry, { rejectCodeHeader = true } = {}) {
  const headers = {};
  if (rejectCodeHeader && entry.details?.reject_code !== undefined) {
    headers["x-iroha-reject-code"] = entry.details.reject_code;
  }
  if (entry.details?.retry_after_seconds !== undefined) {
    headers["Retry-After"] = String(entry.details.retry_after_seconds);
  }
  return headers;
}

function submissionClient(reply) {
  const state = { calls: 0 };
  state.client = new ToriiClient("https://torii.invalid", {
    [TORII_TEST_NATIVE_BINDING]: { encodeSignedTransactionVersioned: (payload) => Buffer.from(payload) },
    fetchImpl: async (url, init) => {
      state.calls += 1;
      assert.equal(url, "https://torii.invalid/v1/pipeline/transactions");
      assert.equal(new Headers(init.headers).get("accept"), "application/x-norito, application/json");
      return reply();
    },
  });
  state.client._ensureDataModelValidation = async () => {};
  return state;
}

test("transaction submission exposes queue rejection and retry details from Norito and JSON envelopes", async () => {
  const rows = ["queue_full", "admission_deferred", "already_committed", "queue_nexus_fee_rejected",
    "transaction_admission_deferred"].map((name) => errorEnvelopeDetailsContract.decoded.find((row) => row.name === name));
  for (const entry of rows) {
    const rejectCode = entry.details.reject_code ?? null;
    for (const format of ["norito", "json"]) {
      const body = format === "norito" ? Buffer.from(entry.norito_hex, "hex") : Buffer.from(entry.json_utf8);
      const contentType = format === "norito" ? NORITO : "application/json; charset=utf-8";
      // The details alone carry the reject code, so the header is optional.
      for (const rejectCodeHeader of [true, false]) {
        const state = submissionClient(() => new Response(body, {
          status: entry.status,
          headers: { "Content-Type": contentType, ...toriiHeadersFor(entry, { rejectCodeHeader }) },
        }));
        await assert.rejects(state.client.submitTransaction(Uint8Array.of(1, 0xde, 0xad)), (error) => {
          assert.ok(error instanceof ToriiHttpError, String(error));
          assert.equal(error.status, entry.status);
          assert.equal(error.envelopeCode, entry.code, entry.name);
          assert.equal(error.rejectCode, rejectCode, entry.name);
          assert.equal(error.code, rejectCode ?? entry.code, entry.name);
          assert.equal(error.errorMessage, entry.message);
          assert.deepEqual(error.details, entry.details);
          assert.equal(error.details.retry_after_seconds, entry.details.retry_after_seconds);
          if (format === "json") {
            assert.equal(error.bodyText, entry.json_utf8);
            assert.deepEqual(error.bodyJson, { code: entry.code, message: entry.message, details: entry.details });
          } else {
            assert.equal(error.bodyText, null);
            assert.equal(error.bodyJson, null);
          }
          assert.equal(isIdempotentSubmissionRejection(error), entry.code === "already_committed");
          return true;
        });
        assert.equal(state.calls, 1);
      }
    }
  }
  // Details carrying a structured observation stay opaque, as before.
  const opaque = errorEnvelopeDetailsContract.undecoded[0];
  const state = submissionClient(() => noritoReply(Buffer.from(opaque.norito_hex, "hex"), opaque.status));
  await assert.rejects(state.client.submitTransaction(Uint8Array.of(1, 0xde, 0xad)), (error) => {
    assert.ok(error instanceof ToriiHttpError);
    assert.equal(error.status, opaque.status);
    assert.equal(error.code, null);
    assert.equal(error.envelopeCode, null);
    assert.equal(error.errorMessage, null);
    assert.equal(error.details, null);
    return true;
  });
});

test("Exact12 capability transport surfaces a Norito error envelope once", async () => {
  const code = "privacy_capabilities_forbidden";
  const message = "the signing account may not read privacy capabilities";
  const url = "https://privacy.example.test/v1/privacy/capabilities";
  for (const [body, check] of [
    [errorEnvelopeFrame(code, message), typedEnvelope(403, code, message)],
    [corrupted(errorEnvelopeFrame(code, message)), opaqueStatus(403)],
  ]) {
    let calls = 0;
    const client = new ToriiClient("https://privacy.example.test", {
      localSigningContext: new LocalSigningContext(NetworkId.fromBytes(Buffer.alloc(32, 0xa5)), 753),
      fetchImpl: async () => {
        calls += 1;
        const response = noritoReply(body, 403);
        return {
          body: response.body, headers: response.headers, status: response.status,
          statusText: response.statusText, url, redirected: false,
        };
      },
    });
    await assert.rejects(
      fetchPrivacyExact12CapabilityManifestTransportV1(client, {
        canonicalAuth: { accountId: "privacy-test@fixture.test", privateKey: Buffer.alloc(32, 0x0b) },
      }),
      check,
    );
    assert.equal(calls, 1);
  }
});

test("Kagemusha wallet originals surface a Norito error envelope within the byte bound", async () => {
  const network = NetworkId.fromBytes(new Uint8Array(32).fill(0xab));
  const publicKey = Buffer.from("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a", "hex");
  const canonicalAuth = {
    accountId: AccountAddress.fromAccount({ publicKey }).toI105(753),
    privateKey: Buffer.from("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60", "hex"),
  };
  const load = { schemeId: "11".repeat(32), walletId: "22".repeat(32), requestId: "33".repeat(32) };
  const code = "kagemusha_wallet_load_not_found";
  const message = "no load issuance is recorded for this request";
  const client = (reply) => {
    const state = { calls: 0 };
    state.client = new ToriiClient("https://wallet.invalid", {
      localSigningContext: new LocalSigningContext(network, 753),
      maxRetries: 3,
      fetchImpl: async () => {
        state.calls += 1;
        return reply();
      },
    });
    return state;
  };
  const typed = client(() => noritoReply(errorEnvelopeFrame(code, message), 404));
  await assert.rejects(
    typed.client.getKagemushaWalletLoadIssuanceOriginalV1(load, { canonicalAuth }),
    typedEnvelope(404, code, message),
  );
  assert.equal(typed.calls, 1);
  // Bytes that are not an envelope keep the strict text failure, and the
  // route's 64 KiB error bound still applies before any classification.
  for (const body of [corrupted(errorEnvelopeFrame(code, message)), Buffer.alloc(64 * 1024 + 1)]) {
    const other = client(() => noritoReply(body, 404));
    await assert.rejects(
      other.client.getKagemushaWalletLoadIssuanceOriginalV1(load, { canonicalAuth }),
      /Kagemusha wallet original error response (?:body must be strict UTF-8|exceeds its 65536-byte size bound)/u,
    );
    assert.equal(other.calls, 1);
  }
});

test("executed block wire surfaces a Norito error envelope in the node and browser clients", async () => {
  const code = "ledger_block_not_found";
  const message = "no finalized block exists at height 7";
  const reads = [
    (fetchImpl) => new ToriiClient("https://torii.invalid", { fetchImpl }).getLedgerExecutedBlockWire(7),
    (fetchImpl) => new ToriiBrowserClient("https://torii.invalid", { fetchImpl }).getLedgerExecutedBlockWire(7),
    (fetchImpl) => new ToriiBrowserClient("https://torii.invalid", { fetchImpl })
      .getLedgerBlockProof(7, "ab".repeat(32)),
  ];
  for (const read of reads) {
    let calls = 0;
    await assert.rejects(read(async (_url, init) => {
      calls += 1;
      assert.equal(new Headers(init.headers).get("accept"), NORITO);
      return noritoReply(errorEnvelopeFrame(code, message), 404);
    }), typedEnvelope(404, code, message));
    assert.equal(calls, 1);
  }
  // The browser keeps its lossy text rendering for a body that is not an envelope.
  const other = corrupted(errorEnvelopeFrame(code, message));
  await assert.rejects(reads[1](async () => noritoReply(other, 404)), (error) => {
    assert.ok(error instanceof ToriiHttpError);
    assert.equal(error.status, 404);
    assert.equal(error.code, null);
    assert.equal(error.bodyText, new TextDecoder().decode(other));
    return true;
  });
  await assert.rejects(reads[0](async () => noritoReply(other, 404)), opaqueStatus(404));
  // A throttled read keeps the details Torii attaches to every 429.
  const throttled = errorEnvelopeDetailsContract.decoded.find((row) => row.name === "proof_rate_limited");
  for (const read of reads) {
    await assert.rejects(
      read(async () => noritoReply(Buffer.from(throttled.norito_hex, "hex"), 429, toriiHeadersFor(throttled))),
      (error) => {
        assert.ok(error instanceof ToriiHttpError);
        assert.equal(error.status, 429);
        assert.equal(error.code, throttled.code);
        assert.equal(error.envelopeCode, throttled.code);
        assert.equal(error.errorMessage, throttled.message);
        assert.deepEqual(error.details, { retry_after_seconds: 3, endpoint: "/v1/zk/proofs" });
        return true;
      },
    );
  }
});

test("Norito RPC errors expose a Torii error envelope's code and message", async () => {
  const code = "norito_rpc_unknown_route";
  const message = "no Norito RPC handler is registered for this path";
  const frame = errorEnvelopeFrame(code, message);
  let calls = 0;
  const client = new NoritoRpcClient("https://torii.invalid", {
    fetchImpl: async () => {
      calls += 1;
      return noritoReply(frame, 404);
    },
  });
  await assert.rejects(client.call("/v1/norito-rpc", Uint8Array.of(1)), (error) => {
    assert.ok(error instanceof NoritoRpcError);
    assert.equal(error.status, 404);
    assert.equal(error.code, code);
    assert.equal(error.errorMessage, message);
    assert.equal(error.body, new TextDecoder().decode(frame));
    assert.equal(error.details, null);
    assert.equal(error.message, `Norito RPC request failed with status 404: ${code} — ${message}`);
    return true;
  });
  assert.equal(calls, 1);
  const deferred = errorEnvelopeDetailsContract.decoded.find((row) => row.name === "transaction_admission_deferred");
  const unavailable = new NoritoRpcClient("https://torii.invalid", {
    fetchImpl: async () => noritoReply(Buffer.from(deferred.norito_hex, "hex"), 503, toriiHeadersFor(deferred)),
  });
  await assert.rejects(unavailable.call("/v1/norito-rpc", Uint8Array.of(1)), (error) => {
    assert.ok(error instanceof NoritoRpcError);
    assert.equal(error.status, 503);
    assert.equal(error.code, deferred.code);
    assert.equal(error.errorMessage, deferred.message);
    assert.deepEqual(error.details, { retry_after_seconds: 1 });
    return true;
  });
  const other = corrupted(frame);
  const plain = new NoritoRpcClient("https://torii.invalid", { fetchImpl: async () => noritoReply(other, 404) });
  await assert.rejects(plain.call("/v1/norito-rpc", Uint8Array.of(1)), (error) => {
    assert.ok(error instanceof NoritoRpcError);
    assert.equal(error.code, null);
    assert.equal(error.errorMessage, null);
    assert.equal(error.details, null);
    assert.equal(error.body, new TextDecoder().decode(other));
    assert.equal(error.message, "Norito RPC request failed with status 404");
    return true;
  });
});
