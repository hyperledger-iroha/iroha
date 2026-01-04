import assert from "node:assert/strict";
import test from "node:test";
import {
  ToriiClient,
  ValidationError,
  ValidationErrorCode,
  buildConnectWebSocketUrl,
  openConnectWebSocket,
} from "../src/index.js";

const SID_HEX = "aa".repeat(32);
const TOKEN = "secret-token";

class RecordingWebSocket {
  constructor(url, protocols, options) {
    this.url = url;
    this.protocols = protocols;
    this.options = options;
    RecordingWebSocket.instances.push(this);
  }
}
RecordingWebSocket.instances = [];

class DualArgRecordingWebSocket {
  constructor(url, options) {
    this.url = url;
    this.protocols = undefined;
    this.options = options;
    DualArgRecordingWebSocket.instances.push(this);
  }
}
DualArgRecordingWebSocket.instances = [];

test("buildConnectWebSocketUrl normalizes schemes and appends query", () => {
  const url = buildConnectWebSocketUrl("https://torii.example", {
    sid: SID_HEX,
    role: "wallet",
    token: TOKEN,
  });
  assert.equal(
    url,
    `wss://torii.example/v1/connect/ws?sid=${SID_HEX}&role=wallet`,
  );

  const httpUrl = buildConnectWebSocketUrl("http://127.0.0.1:8080", {
    sid: SID_HEX,
    role: "app",
    token: "app-token",
    allowInsecure: true,
  });
  assert.equal(
    httpUrl,
    `ws://127.0.0.1:8080/v1/connect/ws?sid=${SID_HEX}&role=app`,
  );
  assert(!httpUrl.includes("token="));
});

test("buildConnectWebSocketUrl rejects token query parameters", () => {
  assert.throws(
    () =>
      buildConnectWebSocketUrl("https://torii.example/v1/connect/ws?token=example", {
        sid: SID_HEX,
        role: "wallet",
        token: TOKEN,
      }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.path, "buildConnectWebSocketUrl.token");
      assert.match(error.message, /must not include token query parameters|token is forbidden/);
      return true;
    },
  );
});

test("buildConnectWebSocketUrl removes unrelated query params to keep URLs referrer-safe", () => {
  const url = buildConnectWebSocketUrl("https://torii.example/v1/connect/ws?debug=true", {
    sid: SID_HEX,
    role: "app",
    token: TOKEN,
  });
  assert.equal(
    url,
    `wss://torii.example/v1/connect/ws?sid=${SID_HEX}&role=app`,
  );
});


test("buildConnectWebSocketUrl rejects insecure protocols unless allowInsecure is set", () => {
  assert.throws(() =>
    buildConnectWebSocketUrl("http://torii.example", {
      sid: SID_HEX,
      role: "wallet",
      token: TOKEN,
    }),
  );
  const url = buildConnectWebSocketUrl("http://torii.example", {
    sid: SID_HEX,
    role: "wallet",
    token: TOKEN,
    allowInsecure: true,
  });
  assert.equal(url, `ws://torii.example/v1/connect/ws?sid=${SID_HEX}&role=wallet`);
});

test("buildConnectWebSocketUrl rejects endpoint host overrides", () => {
  assert.throws(
    () =>
      buildConnectWebSocketUrl("https://torii.example", {
        sid: SID_HEX,
        role: "wallet",
        token: TOKEN,
        endpointPath: "wss://other.example/v1/connect/ws",
      }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.path, "buildConnectWebSocketUrl.endpointPath");
      assert.match(error.message, /host .* must match baseUrl host/i);
      return true;
    },
  );
});

test("buildConnectWebSocketUrl rejects endpoint protocol mismatches", () => {
  assert.throws(
    () =>
      buildConnectWebSocketUrl("https://torii.example", {
        sid: SID_HEX,
        role: "wallet",
        token: TOKEN,
        endpointPath: "ws://torii.example/v1/connect/ws",
      }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.path, "buildConnectWebSocketUrl.endpointPath");
      assert.match(error.message, /protocol must match baseUrl protocol/i);
      return true;
    },
  );
});

test("buildConnectWebSocketUrl rejects missing tokens", () => {
  assert.throws(
    () =>
      buildConnectWebSocketUrl("https://torii.example", {
        sid: SID_HEX,
        role: "wallet",
      }),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
      assert.equal(error.path, "buildConnectWebSocketUrl.token");
      return true;
    },
  );
});

test("buildConnectWebSocketUrl rejects non-object options", () => {
  assert.throws(
    () => buildConnectWebSocketUrl("https://torii.example", null),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_OBJECT);
      assert.equal(error.path, "buildConnectWebSocketUrl.options");
      assert.match(error.message, /buildConnectWebSocketUrl options must be an object/);
      return true;
    },
  );
});

test("ToriiClient openConnectWebSocket delegates to provided implementation", () => {
  RecordingWebSocket.instances.length = 0;
  const client = new ToriiClient("https://torii.devnet.example");
  const socket = client.openConnectWebSocket({
    sid: SID_HEX,
    role: "app",
    token: "preview-token",
    protocols: ["iroha-connect"],
    websocketOptions: { headers: { "x-debug": "1" } },
    WebSocketImpl: RecordingWebSocket,
  });
  assert(socket instanceof RecordingWebSocket);
  assert.equal(RecordingWebSocket.instances.length, 1);
  const record = RecordingWebSocket.instances[0];
  assert.equal(
    record.url,
    `wss://torii.devnet.example/v1/connect/ws?sid=${SID_HEX}&role=app`,
  );
  assert.deepEqual(record.protocols, ["iroha-connect"]);
  assert.deepEqual(record.options, {
    headers: { Authorization: "Bearer preview-token", "x-debug": "1" },
  });
});

test("openConnectWebSocket falls back to second-argument options when no protocols", () => {
  DualArgRecordingWebSocket.instances.length = 0;
  openConnectWebSocket({
    baseUrl: "https://torii.integration.example",
    sid: SID_HEX,
    role: "wallet",
    token: "wallet-token",
    websocketOptions: { rejectUnauthorized: false },
    WebSocketImpl: DualArgRecordingWebSocket,
  });
  assert.equal(DualArgRecordingWebSocket.instances.length, 1);
  const record = DualArgRecordingWebSocket.instances[0];
  assert.equal(
    record.url,
    `wss://torii.integration.example/v1/connect/ws?sid=${SID_HEX}&role=wallet`,
  );
  assert.deepEqual(record.options, {
    headers: { Authorization: "Bearer wallet-token" },
    rejectUnauthorized: false,
  });
});

test("openConnectWebSocket enforces option shapes", () => {
  assert.throws(
    () => openConnectWebSocket("not an object"),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_OBJECT);
      assert.equal(error.path, "openConnectWebSocket.options");
      assert.match(error.message, /openConnectWebSocket options must be an object/);
      return true;
    },
  );
});

test("ToriiClient.openConnectWebSocket enforces option shapes before merging", () => {
  const client = new ToriiClient("https://torii.devnet.example");
  assert.throws(
    () => client.openConnectWebSocket("invalid"),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_OBJECT);
      assert.equal(error.path, "ToriiClient.openConnectWebSocket.options");
      assert.match(error.message, /ToriiClient\.openConnectWebSocket options must be an object/);
      return true;
    },
  );
});

function decodeProtocolToken(protocol) {
  const encoded = protocol.replace(/^iroha-connect\.token\.v1\./, "");
  return Buffer.from(encoded.replace(/-/g, "+").replace(/_/g, "/"), "base64").toString("utf8");
}

test("openConnectWebSocket injects Sec-WebSocket-Protocol when headers are unavailable", () => {
  const originalWindow = globalThis.window;
  class BrowserLikeWebSocket {
    constructor(url, protocols) {
      this.url = url;
      this.protocols = protocols;
      BrowserLikeWebSocket.instances.push(this);
    }
  }
  BrowserLikeWebSocket.instances = [];
  globalThis.window = { WebSocket: BrowserLikeWebSocket };
  try {
    openConnectWebSocket({
      baseUrl: "https://torii-browser.example",
      sid: SID_HEX,
      role: "app",
      token: "browser-token",
      WebSocketImpl: BrowserLikeWebSocket,
    });
    assert.equal(BrowserLikeWebSocket.instances.length, 1);
    const protocols = BrowserLikeWebSocket.instances[0].protocols;
    assert(Array.isArray(protocols));
    assert.match(protocols[0], /^iroha-connect\.token\.v1\./);
    assert.equal(decodeProtocolToken(protocols[0]), "browser-token");
    assert.equal(
      BrowserLikeWebSocket.instances[0].url,
      `wss://torii-browser.example/v1/connect/ws?sid=${SID_HEX}&role=app`,
    );
  } finally {
    globalThis.window = originalWindow;
  }
});


test("ToriiClient.openConnectWebSocket inherits allowInsecure from the client", () => {
  DualArgRecordingWebSocket.instances.length = 0;
  const client = new ToriiClient("http://torii.devnet.example", {
    allowInsecure: true,
  });
  const socket = client.openConnectWebSocket({
    sid: SID_HEX,
    role: "app",
    token: TOKEN,
    WebSocketImpl: DualArgRecordingWebSocket,
  });
  assert(socket instanceof DualArgRecordingWebSocket);
  assert.equal(
    DualArgRecordingWebSocket.instances[0].url,
    `ws://torii.devnet.example/v1/connect/ws?sid=${SID_HEX}&role=app`,
  );
});

test("openConnectWebSocket emits telemetry when allowInsecure is used", () => {
  DualArgRecordingWebSocket.instances.length = 0;
  const events = [];
  openConnectWebSocket({
    baseUrl: "http://torii.example",
    sid: SID_HEX,
    role: "wallet",
    token: TOKEN,
    allowInsecure: true,
    WebSocketImpl: DualArgRecordingWebSocket,
    insecureTransportTelemetryHook: (event) => events.push(event),
  });
  assert.equal(events.length, 1);
  const event = events[0];
  assert.equal(event.client, "connect-ws");
  assert.equal(event.host, "torii.example");
  assert.equal(event.protocol, "ws:");
  assert(event.allowInsecure);
  assert(event.hasCredentials);
  assert.equal(event.pathIsAbsolute, false);
  assert(Number.isFinite(event.timestampMs));
});
