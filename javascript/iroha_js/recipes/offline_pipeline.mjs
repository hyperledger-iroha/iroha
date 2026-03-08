#!/usr/bin/env node
/**
 * Offline transaction signing and replay recipe.
 *
 * This script:
 * 1. Builds a signed transaction using the native Norito helpers.
 * 2. Packages it into an offline envelope and writes JSON to disk.
 * 3. Replays the stored envelope against Torii (default: an in-process mock).
 *
 * Environment variables:
 * - TORII_BASE_URL — Torii base URL for replay (default: mock server).
 * - OFFLINE_PIPELINE_OUT_DIR — directory for persisted envelopes (default: artifacts/js/offline_pipeline).
 * - OFFLINE_PIPELINE_USE_MOCK — set to "0" to disable the mock Torii server.
 * - OFFLINE_PIPELINE_SKIP_REPLAY — set to "1" to build/parse/write only and skip replay.
 */
import { createServer } from "node:http";
import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import {
  AccountAddress,
  ToriiClient,
  buildRegisterDomainInstruction,
  buildOfflineEnvelope,
  buildTransaction,
  hashSignedTransaction,
  parseOfflineEnvelope,
  publicKeyFromPrivate,
  replayOfflineEnvelope,
  serializeOfflineEnvelope,
  writeOfflineEnvelopeFile,
} from "@iroha/iroha-js";

const PRIVATE_KEY = Buffer.alloc(32, 0x11);
const AUTHORITY_DOMAIN = process.env.OFFLINE_PIPELINE_AUTHORITY_DOMAIN ?? "wonderland";
const AUTHORITY_ID =
  process.env.OFFLINE_PIPELINE_AUTHORITY ??
  AccountAddress.fromAccount({
    domain: AUTHORITY_DOMAIN,
    publicKey: publicKeyFromPrivate(PRIVATE_KEY),
  }).toIH58();
const OUT_DIR = process.env.OFFLINE_PIPELINE_OUT_DIR ?? path.join("artifacts", "js", "offline_pipeline");
const USE_MOCK = (process.env.OFFLINE_PIPELINE_USE_MOCK ?? "1") !== "0";
const SKIP_REPLAY = (process.env.OFFLINE_PIPELINE_SKIP_REPLAY ?? "0") === "1";
const DOMAIN_ID = `offline_demo_${Date.now().toString(16)}`;

function buildEnvelope() {
  const { signedTransaction, hash } = buildTransaction({
    chainId: "offline-demo",
    authority: AUTHORITY_ID,
    instructions: [
      buildRegisterDomainInstruction({
        domainId: DOMAIN_ID,
        metadata: { source: "offline-pipeline" },
      }),
    ],
    metadata: { purpose: "offline-demo" },
    privateKey: PRIVATE_KEY,
  });
  return buildOfflineEnvelope({
    signedTransaction,
    keyAlias: "alice-key",
    metadata: { purpose: "offline-demo" },
    hashHex: hash.toString("hex"),
  });
}

async function ensureDir(dir) {
  await fs.mkdir(dir, { recursive: true });
}

function startMockTorii() {
  const posted = [];
  let seenHash = null;
  const server = createServer(async (req, res) => {
    const url = new URL(req.url, `http://${req.headers.host}`);
    if (req.method === "POST" && url.pathname === "/v1/pipeline/transactions") {
      const chunks = [];
      for await (const chunk of req) {
        chunks.push(chunk);
      }
      posted.push(Buffer.concat(chunks));
      res.statusCode = 202;
      res.end();
      return;
    }
    if (req.method === "GET" && url.pathname === "/v1/pipeline/transactions/status") {
      seenHash = url.searchParams.get("hash");
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ status: "Committed" }));
      return;
    }
    res.statusCode = 404;
    res.end();
  });
  return new Promise((resolve, reject) => {
    server.listen(0, "127.0.0.1", (err) => {
      if (err) {
        reject(err);
        return;
      }
      const address = server.address();
      resolve({
        baseUrl: `http://127.0.0.1:${address.port}`,
        server,
        posted,
        getSeenHash: () => seenHash,
      });
    });
  });
}

async function main() {
  await ensureDir(OUT_DIR);
  const envelope = buildEnvelope();
  const serialized = serializeOfflineEnvelope(envelope);
  const jsonPath = path.join(OUT_DIR, "envelope.json");
  await writeOfflineEnvelopeFile(jsonPath, envelope);
  console.log("Stored envelope at", jsonPath);

  const parsed = parseOfflineEnvelope(serialized);
  console.log("Envelope hash:", parsed.hashHex);
  console.log("Envelope metadata:", parsed.metadata);
  console.log("Envelope schema:", parsed.schemaName);
  if (SKIP_REPLAY) {
    console.log("OFFLINE_PIPELINE_SKIP_REPLAY=1; skipping replay stage.");
    return;
  }

  let mock = null;
  let baseUrl = process.env.TORII_BASE_URL;
  if (USE_MOCK || !baseUrl) {
    mock = await startMockTorii(parsed.hashHex);
    baseUrl = mock.baseUrl;
    console.log("Using mock Torii at", baseUrl);
  } else {
    console.log("Using provided Torii base URL", baseUrl);
  }

  try {
    const client = new ToriiClient(baseUrl);
    await replayOfflineEnvelope(client, parsed, { intervalMs: 50, timeoutMs: 5_000 });
    console.log("Replay succeeded (hash:", parsed.hashHex, ")");
    if (mock) {
      console.log("Posted bytes:", mock.posted[0]?.length ?? 0);
      console.log("Hash seen by server:", mock.getSeenHash());
      console.log("Hash check:", hashSignedTransaction(mock.posted[0]) === parsed.hashHex ? "ok" : "mismatch");
    }
  } finally {
    if (mock) {
      await new Promise((resolve, reject) =>
        mock.server.close((err) => (err ? reject(err) : resolve())),
      );
    }
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
