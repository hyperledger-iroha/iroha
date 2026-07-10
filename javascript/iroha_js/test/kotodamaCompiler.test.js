import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import test from "node:test";

import {
  KotodamaCompilerClient,
  compileKotodamaProgram,
} from "../src/kotodamaCompiler/index.js";
import {
  compileKotodamaProgram as compileKotodamaInBrowser,
} from "../src/kotodamaCompiler/browser.js";
import { compileKotodamaWithNativeBinding } from "../src/kotodamaCompiler/nativeBridge.js";
import { blake2b256 } from "../src/blake2b.js";

const SERVICE_ARTIFACT = Uint8Array.from([1, 2, 3]);
const CONTRACT_HASH_DOMAIN = new TextEncoder().encode("iroha:ivm:contract-artifact:v1\0");
const SERVICE_HASH_INPUT = new Uint8Array(
  CONTRACT_HASH_DOMAIN.length + SERVICE_ARTIFACT.length,
);
SERVICE_HASH_INPUT.set(CONTRACT_HASH_DOMAIN);
SERVICE_HASH_INPUT.set(SERVICE_ARTIFACT, CONTRACT_HASH_DOMAIN.length);
const SERVICE_CODE_HASH = Array.from(
  blake2b256(SERVICE_HASH_INPUT),
  (byte) => byte.toString(16).padStart(2, "0"),
).join("");
const SERVICE_ABI_HASH = "22".repeat(32);

const SERVICE_OUTPUT = {
  artifactBytes: [...SERVICE_ARTIFACT],
  manifestJson: JSON.stringify({
    compiler_fingerprint: "kotodama_lang/test",
    code_hash: `hash:${SERVICE_CODE_HASH.toUpperCase()}`,
    abi_hash: `hash:${SERVICE_ABI_HASH.toUpperCase()}`,
    entrypoints: [],
  }),
  codeHash: SERVICE_CODE_HASH,
  abiHash: SERVICE_ABI_HASH,
  sourceMapJson: JSON.stringify({
    sidecar_version: 1,
    kind: "source-map",
    artifact_hash: SERVICE_CODE_HASH,
    entries: [{ function_name: "ping", pc_start: 0, pc_end: 4 }],
  }),
  budgetReportJson: JSON.stringify({
    sidecar_version: 1,
    kind: "budget",
    artifact_hash: SERVICE_CODE_HASH,
    entries: [{ function_name: "ping", bytecode_words: 1 }],
  }),
};

const SERVICE_SUCCESS = {
  ok: true,
  output: SERVICE_OUTPUT,
  diagnosticsJson: null,
};

const SERVICE_DIAGNOSTICS = [
  {
    code: "K1001",
    severity: "error",
    phase: "parse",
    message: "expected parameter name",
    primary_span: {
      source: "契約/送金.ko",
      start: { line: 2, column: 9 },
      end: { line: 2, column: 10 },
      byte_range: { start: 20, end: 24 },
    },
    labels: [
      {
        span: {
          source: "契約/送金.ko",
          start: { line: 2, column: 3 },
          end: { line: 2, column: 7 },
          byte_range: { start: 12, end: 16 },
        },
        message: "while parsing this entrypoint",
      },
    ],
    notes: ["the preceding 🙂 occupies one Unicode display column"],
    help: "write name: Type",
    fix: {
      span: {
        source: "契約/送金.ko",
        start: { line: 2, column: 9 },
        end: { line: 2, column: 9 },
        byte_range: { start: 20, end: 20 },
      },
      replacement: "amount: i64",
    },
  },
  {
    code: "K2002",
    severity: "error",
    phase: "semantic",
    message: "unknown name `missing`",
    primary_span: {
      source: "契約/送金.ko",
      start: { line: 4, column: 5 },
      end: { line: 4, column: 12 },
      byte_range: { start: 48, end: 55 },
    },
    labels: [],
    notes: [],
    help: null,
    fix: null,
  },
];

const SERVICE_FAILURE = {
  ok: false,
  output: null,
  diagnosticsJson: JSON.stringify(SERVICE_DIAGNOSTICS),
};

function jsonResponse(value, init = {}) {
  return new Response(JSON.stringify(value), {
    status: init.status ?? 200,
    headers: { "content-type": "application/json", ...init.headers },
  });
}

function successfulFetch(calls, value = SERVICE_SUCCESS) {
  return async (url, init) => {
    calls.push({ url, init });
    return jsonResponse(value);
  };
}

test("JavaScript ships only adapters to the canonical Rust compiler", () => {
  const expectedFiles = [
    "browser.js",
    "client.js",
    "index.js",
    "nativeBridge.js",
    "normalize.js",
  ];
  for (const directory of ["../src/kotodamaCompiler/", "../dist/kotodamaCompiler/"]) {
    const actualFiles = readdirSync(new URL(directory, import.meta.url), {
      withFileTypes: true,
    })
      .filter((entry) => entry.isFile())
      .map((entry) => entry.name)
      .sort();
    assert.deepEqual(
      actualFiles,
      expectedFiles,
      `${directory} must not contain an independent JavaScript compiler`,
    );
  }
  for (const file of expectedFiles) {
    assert.equal(
      readFileSync(new URL(`../src/kotodamaCompiler/${file}`, import.meta.url), "utf8"),
      readFileSync(new URL(`../dist/kotodamaCompiler/${file}`, import.meta.url), "utf8"),
      `dist/kotodamaCompiler/${file} must exactly match src`,
    );
  }
});

test("TypeScript exposes only the bounded source-name and ZK request policy", () => {
  const declarations = readFileSync(
    new URL("../kotodama-compiler.d.ts", import.meta.url),
    "utf8",
  );
  const requestStart = declarations.indexOf(
    "export interface KotodamaCompilerRequestOptions",
  );
  const requestEnd = declarations.indexOf(
    "export interface KotodamaCompilerOutput",
    requestStart,
  );
  assert.ok(requestStart >= 0 && requestEnd > requestStart);
  const requestDeclarations = declarations.slice(requestStart, requestEnd);
  assert.match(requestDeclarations, /sourceName\?: string;/u);
  assert.match(requestDeclarations, /zk\?: boolean;/u);
  assert.match(requestDeclarations, /source: string;[\s\S]*zk: boolean;/u);
  assert.doesNotMatch(
    requestDeclarations,
    /abiVersion|forceVector|embedDebug|forceZk|testMode/u,
  );
  assert.match(
    declarations,
    /compile\(\s*source: string,\s*options\?: KotodamaCompilerRequestOptions,/u,
  );
});

test("Node delegates asynchronously to iroha_js_host exactly once", async () => {
  const source = "seiyaku Demo { view fn ping() -> i64 { return 1; } }";
  const options = { sourceName: "contracts/demo.ko", zk: true };
  let finishCompilation;
  const nativeCompletion = new Promise((resolve) => {
    finishCompilation = resolve;
  });
  const calls = [];
  const native = {
    compileKotodama(receivedSource) {
      calls.push(receivedSource);
      return nativeCompletion;
    },
  };

  let settled = false;
  const resultPromise = compileKotodamaWithNativeBinding(native, source, options);
  resultPromise.then(() => {
    settled = true;
  });
  assert.ok(resultPromise instanceof Promise);
  assert.deepEqual(calls, [{ source, sourceName: "contracts/demo.ko", zk: true }]);
  await Promise.resolve();
  assert.equal(settled, false, "the adapter must await the asynchronous native task");

  finishCompilation(SERVICE_SUCCESS);
  const result = await resultPromise;
  assert.equal(result.ok, true);
  assert.deepEqual([...result.output.artifactBytes], [1, 2, 3]);
  assert.deepEqual(
    calls,
    [{ source, sourceName: "contracts/demo.ko", zk: true }],
    "one SDK request must perform one native compilation",
  );

  await assert.rejects(
    compileKotodamaWithNativeBinding({}, source),
    /native binding is missing compileKotodama/,
  );
});

test("iroha_js_host keeps compilation off the Node event-loop thread", () => {
  const hostSource = readFileSync(
    new URL("../../../crates/iroha_js_host/src/lib.rs", import.meta.url),
    "utf8",
  );
  assert.match(
    hostSource,
    /#\[napi\(js_name = "compileKotodama"\)\]\s*pub async fn compile_kotodama/u,
  );
  assert.match(
    hostSource,
    /pub async fn compile_kotodama[\s\S]*?tokio::task::spawn_blocking/u,
  );
  assert.match(
    hostSource,
    /pub struct JsKotodamaCompileRequest[\s\S]*?pub source: String,[\s\S]*?pub source_name: Option<String>,[\s\S]*?pub zk: bool,/u,
  );
  assert.match(
    hostSource,
    /CompilerOptions\s*\{[\s\S]*?force_zk: request\.zk,[\s\S]*?\.build\(/u,
  );
  assert.match(
    hostSource,
    /source_name: request\.source_name\.as_deref\(\)/u,
  );
});

test("browser condition precedes the Node import condition", () => {
  const packageJson = JSON.parse(
    readFileSync(new URL("../package.json", import.meta.url), "utf8"),
  );
  assert.deepEqual(Object.keys(packageJson.exports["./kotodama-compiler"]), [
    "types",
    "browser",
    "import",
  ]);
  assert.equal(
    packageJson.exports["./kotodama-compiler"].browser,
    "./dist/kotodamaCompiler/browser.js",
  );
});

test("browser compiler client uses the explicit Rust service and normalizes output", async () => {
  const calls = [];
  const client = new KotodamaCompilerClient("https://compiler.example/", {
    fetchImpl: successfulFetch(calls),
  });
  const result = await client.compile(
    "seiyaku Demo { view fn ping() -> i64 { return 1; } }",
    { sourceName: "contracts/demo.ko", zk: true },
  );

  assert.equal(result.ok, true);
  assert.deepEqual([...result.output.artifactBytes], [1, 2, 3]);
  assert.equal(result.output.codeHashHex, SERVICE_OUTPUT.codeHash);
  assert.equal(result.output.abiHashHex, SERVICE_OUTPUT.abiHash);
  assert.equal(result.output.compilerFingerprint, "kotodama_lang/test");
  assert.equal(result.output.sourceMap[0].function_name, "ping");
  assert.equal(result.output.budgetReport[0].bytecode_words, 1);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://compiler.example/v1/kotodama/compile");
  assert.equal(calls[0].init.method, "POST");
  assert.equal(calls[0].init.headers.accept, "application/json");
  assert.deepEqual(JSON.parse(calls[0].init.body), {
    source: "seiyaku Demo { view fn ping() -> i64 { return 1; } }",
    sourceName: "contracts/demo.ko",
    zk: true,
  });
});

test("compiler adapters reject oversized UTF-8 source before native or network dispatch", async () => {
  const calls = [];
  const client = new KotodamaCompilerClient("https://compiler.example", {
    fetchImpl: successfulFetch(calls),
  });
  const maxSourceBytes = 1024 * 1024;

  const boundary = await client.compile("a".repeat(maxSourceBytes));
  assert.equal(boundary.ok, true);
  assert.equal(calls.length, 1, "the exact V1 source-byte limit remains admissible");

  await assert.rejects(
    client.compile("a".repeat(maxSourceBytes + 1)),
    /exceeds the 1048576-byte V1 limit/,
  );
  await assert.rejects(
    client.compile(`${"a".repeat(maxSourceBytes - 1)}🙂`),
    /exceeds the 1048576-byte V1 limit/,
  );
  await assert.rejects(
    client.compile("\ud800"),
    /must contain valid Unicode scalar values/,
  );
  assert.equal(calls.length, 1, "oversized source must not reach the compiler service");

  await assert.rejects(
    compileKotodamaProgram("a".repeat(maxSourceBytes + 1)),
    /exceeds the 1048576-byte V1 limit/,
  );
});

test("compiler requests bound sourceName and expose only the canonical ZK selector", async () => {
  const calls = [];
  const client = new KotodamaCompilerClient("https://compiler.example", {
    fetchImpl: successfulFetch(calls),
  });

  await client.compile("seiyaku Demo {}", { sourceName: "契約/送金.ko", zk: true });
  assert.deepEqual(JSON.parse(calls[0].init.body), {
    source: "seiyaku Demo {}",
    sourceName: "契約/送金.ko",
    zk: true,
  });

  for (const options of [
    { sourceName: "" },
    { sourceName: "contracts/demo\nleak.ko" },
    { sourceName: "x".repeat(4097) },
    { sourceName: "\ud800" },
    { zk: "true" },
    { forceZk: true },
  ]) {
    await assert.rejects(client.compile("seiyaku Demo {}", options));
  }
  assert.equal(calls.length, 1, "invalid compiler policy must fail before network dispatch");
});

test("compiler failures preserve every canonical semantic diagnostic field", async () => {
  const client = new KotodamaCompilerClient("https://compiler.example", {
    fetchImpl: async () => jsonResponse(SERVICE_FAILURE),
  });
  const result = await client.compile("seiyaku Demo { 🙂 }");

  assert.deepEqual(result, { ok: false, diagnostics: SERVICE_DIAGNOSTICS });
  assert.equal(result.diagnostics.length, 2);
  assert.deepEqual(result.diagnostics[0].primary_span, {
    source: "契約/送金.ko",
    start: { line: 2, column: 9 },
    end: { line: 2, column: 10 },
    byte_range: { start: 20, end: 24 },
  });
  assert.deepEqual(result.diagnostics[0].labels, SERVICE_DIAGNOSTICS[0].labels);
  assert.deepEqual(result.diagnostics[0].notes, SERVICE_DIAGNOSTICS[0].notes);
  assert.equal(result.diagnostics[0].help, "write name: Type");
  assert.deepEqual(result.diagnostics[0].fix, SERVICE_DIAGNOSTICS[0].fix);
});

test("compiler sidecars must match the deployable artifact hash", async () => {
  const invalid = {
    ...SERVICE_SUCCESS,
    output: {
      ...SERVICE_OUTPUT,
      sourceMapJson: JSON.stringify({
        sidecar_version: 1,
        kind: "source-map",
        artifact_hash: "ff".repeat(32),
        entries: [],
      }),
    },
  };
  const client = new KotodamaCompilerClient("https://compiler.example", {
    fetchImpl: async () => jsonResponse(invalid),
  });
  await assert.rejects(
    client.compile("seiyaku Demo {}"),
    /invalid or mismatched source-map sidecar/,
  );
});

test("compiler wire output cannot substitute bytes behind a claimed hash", async () => {
  const client = new KotodamaCompilerClient("https://compiler.example", {
    fetchImpl: async () =>
      jsonResponse({
        ...SERVICE_SUCCESS,
        output: { ...SERVICE_OUTPUT, artifactBytes: [1, 2, 4] },
      }),
  });
  await assert.rejects(
    client.compile("seiyaku Demo {}"),
    /artifact bytes do not match codeHash/,
  );
});

test("compile helper accepts an explicit browser compiler service", async () => {
  const calls = [];
  const result = await compileKotodamaProgram("seiyaku Demo {}", {
    compilerUrl: "https://compiler.example",
    fetchImpl: successfulFetch(calls),
    sourceName: "contracts/node-service.ko",
    zk: true,
  });
  assert.equal(result.ok, true);
  assert.deepEqual([...result.output.artifactBytes], [1, 2, 3]);
  assert.equal(calls.length, 1);
  assert.deepEqual(JSON.parse(calls[0].init.body), {
    source: "seiyaku Demo {}",
    sourceName: "contracts/node-service.ko",
    zk: true,
  });
});

test("browser entrypoint forwards the bounded request to its compiler service", async () => {
  const calls = [];
  const result = await compileKotodamaInBrowser("seiyaku Demo {}", {
    compilerUrl: "https://compiler.example",
    fetchImpl: successfulFetch(calls),
    sourceName: "contracts/browser-service.ko",
    zk: true,
  });
  assert.equal(result.ok, true);
  assert.deepEqual(JSON.parse(calls[0].init.body), {
    source: "seiyaku Demo {}",
    sourceName: "contracts/browser-service.ko",
    zk: true,
  });
});

test("malformed service JSON and malformed envelopes fail closed", async () => {
  const malformedJson = new KotodamaCompilerClient("https://compiler.example", {
    fetchImpl: async () => new Response("{", { status: 200 }),
  });
  await assert.rejects(
    malformedJson.compile("seiyaku Demo {}"),
    /returned malformed JSON/,
  );

  const legacyOutput = new KotodamaCompilerClient("https://compiler.example", {
    fetchImpl: async () => jsonResponse(SERVICE_OUTPUT),
  });
  await assert.rejects(
    legacyOutput.compile("seiyaku Demo {}"),
    /result contains an unknown field/,
  );

  const malformedDiagnostic = structuredClone(SERVICE_FAILURE);
  const diagnostics = JSON.parse(malformedDiagnostic.diagnosticsJson);
  diagnostics[0].primary_span.start.column = 0;
  malformedDiagnostic.diagnosticsJson = JSON.stringify(diagnostics);
  const invalidFields = new KotodamaCompilerClient("https://compiler.example", {
    fetchImpl: async () => jsonResponse(malformedDiagnostic),
  });
  await assert.rejects(
    invalidFields.compile("seiyaku Demo {}"),
    /one-based safe-integer line and column/,
  );
});

test("compiler response and HTTP error bodies are bounded before reading", async () => {
  const oversizedResult = new KotodamaCompilerClient("https://compiler.example", {
    fetchImpl: async () =>
      new Response("{}", {
        status: 200,
        headers: { "content-length": String(16 * 1024 * 1024 + 1) },
      }),
  });
  await assert.rejects(
    oversizedResult.compile("seiyaku Demo {}"),
    /exceeds the 16777216-byte response limit/,
  );

  let streamCancelled = false;
  let emittedChunks = 0;
  const chunk = new Uint8Array(1024 * 1024);
  const oversizedStream = new KotodamaCompilerClient("https://compiler.example", {
    fetchImpl: async () =>
      new Response(
        new ReadableStream({
          pull(controller) {
            controller.enqueue(chunk);
            emittedChunks += 1;
          },
          cancel() {
            streamCancelled = true;
          },
        }),
        { status: 200 },
      ),
  });
  await assert.rejects(
    oversizedStream.compile("seiyaku Demo {}"),
    /exceeds the 16777216-byte response limit/,
  );
  assert.ok(
    emittedChunks >= 17 && emittedChunks <= 18,
    "stream backpressure may queue at most one chunk beyond the rejected chunk",
  );
  assert.equal(streamCancelled, true, "the reader must cancel an oversized live stream");

  const oversizedError = new KotodamaCompilerClient("https://compiler.example", {
    fetchImpl: async () =>
      new Response("failure", {
        status: 500,
        headers: { "content-length": String(64 * 1024 + 1) },
      }),
  });
  await assert.rejects(
    oversizedError.compile("seiyaku Demo {}"),
    /exceeds the 65536-byte response limit/,
  );
});

test("compiler service transport failures surface bounded status details", async () => {
  const client = new KotodamaCompilerClient("https://compiler.example", {
    fetchImpl: async () => new Response("K9000: unavailable", { status: 503 }),
  });
  await assert.rejects(
    client.compile("seiyaku Demo {}"),
    /Kotodama compiler service failed \(503\): K9000: unavailable/,
  );
});

test("browser entrypoint refuses implicit offline compilation", async () => {
  await assert.rejects(
    compileKotodamaInBrowser("seiyaku Demo {}"),
    /requires compilerUrl; offline compilation is unsupported/,
  );
});

test("retired compiler policy options fail closed", async () => {
  for (const options of [
    { abiVersion: 1 },
    { forceVector: true },
    { forceZk: true },
    { embedDebug: true },
    { mode: "test" },
  ]) {
    await assert.rejects(
      compileKotodamaInBrowser("seiyaku Demo {}", options),
      /unknown Kotodama compiler option/,
    );
  }
});

test("compiler transport configuration rejects ambiguous or credential-bearing URLs", async () => {
  for (const options of [
    { compilerUrl: "" },
    { compilerUrl: "compiler.example" },
    { compilerUrl: "http://compiler.example" },
    { compilerUrl: "https://user:secret@compiler.example" },
    { compilerUrl: "https://compiler.example?target=other" },
    { compilerUrl: "https://compiler.example#other" },
    { compilerUrl: "https://compiler.example", fetchImpl: true },
  ]) {
    await assert.rejects(compileKotodamaProgram("seiyaku Demo {}", options), TypeError);
  }
});

test("loopback development compiler services may use HTTP", async () => {
  for (const compilerUrl of [
    "http://localhost:8080",
    "http://worker.localhost:8080",
    "http://127.0.0.1:8080",
    "http://[::1]:8080",
  ]) {
    const calls = [];
    const result = await compileKotodamaProgram("seiyaku Demo {}", {
      compilerUrl,
      fetchImpl: successfulFetch(calls),
    });
    assert.equal(result.ok, true);
    assert.equal(calls.length, 1);
  }
});
