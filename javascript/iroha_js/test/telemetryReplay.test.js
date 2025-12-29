import { test } from "node:test";
import assert from "node:assert/strict";
import { captureSumeragiTelemetrySnapshot, appendSumeragiTelemetrySnapshot } from "../src/telemetryReplay.js";

function makeTelemetry() {
  return {
    availability: { collectors: [] },
    qc_latency_ms: [],
    rbc_backlog: { lanes: [] },
    vrf: {
      buckets: [],
      late_reveals: [],
      summary: null,
    },
  };
}

test("captureSumeragiTelemetrySnapshot respects overrides", async () => {
  const signal = new AbortController().signal;
  const telemetry = makeTelemetry();
  let called = false;
  const client = {
    async getSumeragiTelemetryTyped(options) {
      called = true;
      assert.equal(options.signal, signal);
      return telemetry;
    },
  };
  const timestamp = 1_726_000_000_000;
  const snapshot = await captureSumeragiTelemetrySnapshot(client, {
    signal,
    timestamp,
  });
  assert.ok(called, "client should be invoked");
  assert.equal(snapshot.capturedAtUnixMs, timestamp);
  assert.equal(snapshot.capturedAtIso, new Date(timestamp).toISOString());
  assert.equal(snapshot.telemetry, telemetry);
});

function createFakeFs() {
  const ops = [];
  return {
    ops,
    async mkdir(path, options) {
      ops.push({ kind: "mkdir", path, options });
    },
    async appendFile(path, data) {
      ops.push({ kind: "appendFile", path, data });
    },
  };
}

test("appendSumeragiTelemetrySnapshot writes ndjson records", async () => {
  const telemetry = makeTelemetry();
  const client = {
    async getSumeragiTelemetryTyped() {
      return telemetry;
    },
  };
  const fakeFs = createFakeFs();
  const outputPath = "/tmp/sumeragi/telemetry.ndjson";
  const timestamp = 1_726_000_123_456;
  const snapshot = await appendSumeragiTelemetrySnapshot(client, outputPath, {
    timestamp,
    fs: fakeFs,
  });
  assert.equal(snapshot.capturedAtUnixMs, timestamp);
  assert.equal(snapshot.telemetry, telemetry);
  assert.equal(fakeFs.ops.length, 2);
  assert.deepEqual(fakeFs.ops[0], {
    kind: "mkdir",
    path: "/tmp/sumeragi",
    options: { recursive: true },
  });
  const appendOp = fakeFs.ops[1];
  assert.equal(appendOp.kind, "appendFile");
  assert.equal(appendOp.path, outputPath);
  assert.ok(appendOp.data.endsWith("\n"));
  const parsed = JSON.parse(appendOp.data.trim());
  assert.equal(parsed.capturedAtUnixMs, timestamp);
  assert.deepEqual(parsed.telemetry, telemetry);
});

test("appendSumeragiTelemetrySnapshot enforces output path", async () => {
  const client = {
    async getSumeragiTelemetryTyped() {
      return makeTelemetry();
    },
  };
  await assert.rejects(
    () => appendSumeragiTelemetrySnapshot(client, ""),
    /outputPath must be a non-empty string/,
  );
});

test("captureSumeragiTelemetrySnapshot validates options payload", async () => {
  const telemetry = makeTelemetry();
  const client = {
    async getSumeragiTelemetryTyped() {
      return telemetry;
    },
  };
  await assert.rejects(
    () => captureSumeragiTelemetrySnapshot(client, "invalid"),
    /captureSumeragiTelemetrySnapshot options must be a plain object/,
  );
  await assert.rejects(
    () => captureSumeragiTelemetrySnapshot(client, { signal: {} }),
    /captureSumeragiTelemetrySnapshot options\.signal must be an AbortSignal/,
  );
  await assert.rejects(
    () => captureSumeragiTelemetrySnapshot(client, { timestamp: "soon" }),
    /captureSumeragiTelemetrySnapshot options\.timestamp must be a finite number/,
  );
  await assert.rejects(
    () => captureSumeragiTelemetrySnapshot(client, { timestamp: 1, unknown: true }),
    /captureSumeragiTelemetrySnapshot options contains unsupported fields: unknown/,
  );
});

test("appendSumeragiTelemetrySnapshot validates fs overrides", async () => {
  const telemetry = makeTelemetry();
  const client = {
    async getSumeragiTelemetryTyped() {
      return telemetry;
    },
  };
  await assert.rejects(
    () =>
      appendSumeragiTelemetrySnapshot(client, "/tmp/out.ndjson", {
        fs: "bad",
      }),
    /appendSumeragiTelemetrySnapshot options\.fs must be a plain object/,
  );
  await assert.rejects(
    () =>
      appendSumeragiTelemetrySnapshot(client, "/tmp/out.ndjson", {
        fs: { mkdir: "noop" },
      }),
    /appendSumeragiTelemetrySnapshot options\.fs\.mkdir must be a function/,
  );
  await assert.rejects(
    () =>
      appendSumeragiTelemetrySnapshot(client, "/tmp/out.ndjson", {
        fs: { appendFile: 123 },
      }),
    /appendSumeragiTelemetrySnapshot options\.fs\.appendFile must be a function/,
  );
  await assert.rejects(
    () =>
      appendSumeragiTelemetrySnapshot(client, "/tmp/out.ndjson", {
        fs: { mkdir: async () => {}, appendFile: async () => {} },
        extra: true,
      }),
    /appendSumeragiTelemetrySnapshot options contains unsupported fields: extra/,
  );
});
