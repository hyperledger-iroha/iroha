import { test } from "node:test";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { ToriiClient } from "../src/toriiClient.js";
import { AccountAddress } from "../src/address.js";
import {
  canonicalRequestSignatureMessage,
  normalizeAccountId,
  signEd25519,
} from "../src/index.js";

const BASE_URL = "https://localhost:8080";
const SORA_I105_DISCRIMINANT = 0x2f1;
const SAMPLE_ACCOUNT_SIGNATORY =
  "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
const CANONICAL_AUTH_ALIAS = "alice-1@wonderland";

function sampleAccountId() {
  const address = AccountAddress.fromAccount({
    publicKey: Buffer.from(SAMPLE_ACCOUNT_SIGNATORY.slice(6), "hex"),
  });
  return normalizeAccountId(
    address.toI105(SORA_I105_DISCRIMINANT),
    "toriiClientVpn.sampleAccountId",
  );
}

const SAMPLE_ACCOUNT_ID = sampleAccountId();

const SAMPLE_VPN_HELPER_TICKET_HEX = `5356504e48543100${"00".repeat(656)}`;
const SAMPLE_VPN_RELAY_ID_HEX =
  "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

function sampleVpnTrustPayload(spki = "ab".repeat(32)) {
  return {
    relay_id_hex: SAMPLE_VPN_RELAY_ID_HEX,
    descriptor_commit_hex: "cd".repeat(32),
    tls_server_name: "relay.example",
    relay_tls_spki_sha256_hex: spki,
    relay_certificate_sha256_hex: "ef".repeat(32),
    directory_snapshot_digest_hex: "42".repeat(32),
  };
}

function sampleVpnTrustModel(spki = "ab".repeat(32)) {
  return {
    relayIdHex: SAMPLE_VPN_RELAY_ID_HEX,
    descriptorCommitHex: "cd".repeat(32),
    tlsServerName: "relay.example",
    relayTlsSpkiSha256Hex: spki,
    relayCertificateSha256Hex: "ef".repeat(32),
    directorySnapshotDigestHex: "42".repeat(32),
  };
}

function sampleVpnProfilePayload() {
  return {
    available: true,
    relay_endpoint: "/dns/torii.exit.example/udp/9443/quic",
    supported_exit_classes: ["standard", "low-latency", "high-security"],
    default_exit_class: "standard",
    lease_secs: 600,
    dns_push_interval_secs: 90,
    meter_family: "soranet.vpn.standard",
    route_pushes: [],
    excluded_routes: [],
    dns_servers: ["1.1.1.1"],
    tunnel_addresses: ["10.208.0.2/32"],
    mtu_bytes: 1280,
    display_billing_label: "standard",
    operator_account_id: SAMPLE_ACCOUNT_ID,
    lease_fee: "1000000.25",
    settlement_grace_secs: 120,
    flow_label_bits: 24,
    padding_budget_ms: 80,
    ...sampleVpnTrustPayload("ac".repeat(32)),
  };
}

function sampleVpnSessionPayload(helperTicketHex = SAMPLE_VPN_HELPER_TICKET_HEX) {
  const sessionId = "ab".repeat(32);
  const quoteId = "cd".repeat(32);
  return {
    session_id: sessionId,
    account_id: SAMPLE_ACCOUNT_ID,
    exit_class: "standard",
    relay_endpoint: "/dns/torii.exit.example/udp/9443/quic",
    lease_secs: 600,
    expires_at_ms: 1_700_000_000_000,
    connected_at_ms: 1_699_999_400_000,
    meter_family: "soranet.vpn.standard",
    quote_id: quoteId,
    payment_reference: quoteId,
    payment_tx_hash: "ef".repeat(32),
    fee_asset_id: "xor#universal.universal",
    escrow_account_id: SAMPLE_ACCOUNT_ID,
    operator_account_id: SAMPLE_ACCOUNT_ID,
    lease_fee: "1000000.25",
    flow_label_bits: 24,
    padding_budget_ms: 80,
    ...sampleVpnTrustPayload("ac".repeat(32)),
    route_pushes: [],
    excluded_routes: [],
    dns_servers: ["1.1.1.1"],
    tunnel_addresses: ["10.208.0.2/32"],
    mtu_bytes: 1280,
    helper_ticket_hex: helperTicketHex,
    bytes_in: 0,
    bytes_out: 0,
    status: "active",
  };
}

function sampleVpnQuotePayload() {
  const profile = sampleVpnProfilePayload();
  const instruction = {
    wire_id: "OpenVpnLeaseEscrow",
    payload_hex: "abcd",
  };
  return {
    quote_id: "cd".repeat(32),
    lease_id_hex: "ab".repeat(32),
    session_id_hex: "ef".repeat(16),
    payment_reference: "vpn-payment-reference",
    account_id: SAMPLE_ACCOUNT_ID,
    exit_class: "standard",
    relay_endpoint: profile.relay_endpoint,
    lease_secs: profile.lease_secs,
    quote_expires_at_ms: 1_700_000_000_000,
    fee_asset_id: "xor#universal.universal",
    escrow_account_id: SAMPLE_ACCOUNT_ID,
    operator_account_id: profile.operator_account_id,
    lease_fee: profile.lease_fee,
    route_pushes: profile.route_pushes,
    excluded_routes: profile.excluded_routes,
    dns_servers: profile.dns_servers,
    tunnel_addresses: profile.tunnel_addresses,
    mtu_bytes: profile.mtu_bytes,
    meter_family: profile.meter_family,
    flow_label_bits: profile.flow_label_bits,
    padding_budget_ms: profile.padding_budget_ms,
    ...sampleVpnTrustPayload(profile.relay_tls_spki_sha256_hex),
    metering_public_key_hex: "12".repeat(32),
    open_lease_instruction: instruction,
  };
}

function sampleVpnReceiptPayload() {
  const session = sampleVpnSessionPayload();
  return {
    session_id: session.session_id,
    account_id: session.account_id,
    exit_class: session.exit_class,
    relay_endpoint: session.relay_endpoint,
    meter_family: session.meter_family,
    connected_at_ms: session.connected_at_ms,
    disconnected_at_ms: session.connected_at_ms + 60_000,
    duration_ms: 60_000,
    bytes_in: 1024,
    bytes_out: 2048,
    status: "disconnected",
    receipt_source: "torii",
    quote_id: session.quote_id,
    payment_tx_hash: session.payment_tx_hash,
    fee_asset_id: session.fee_asset_id,
    escrow_account_id: session.escrow_account_id,
    operator_account_id: session.operator_account_id,
    lease_fee: session.lease_fee,
    earned_fee: "0",
    refunded_fee: session.lease_fee,
    lease_id_hex: session.quote_id,
    settle_lease_instruction: null,
  };
}

async function parseVpnTestResponse(kind, payload) {
  const status = kind === "quote" || kind === "session-create" ? 201 : 200;
  const fetchImpl = async () =>
    createResponse({
      status,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    });
  const client = new ToriiClient(BASE_URL, {
    fetchImpl,
  });
  const canonicalAuth = {
    accountId: CANONICAL_AUTH_ALIAS,
    privateKey: Buffer.alloc(32, 12),
  };
  switch (kind) {
    case "profile":
      return client.getVpnProfile();
    case "quote":
      return client.createVpnQuote(
        { meteringPublicKeyHex: "12".repeat(32) },
        { canonicalAuth },
      );
    case "session":
      return client.getVpnSession("ab".repeat(32), { canonicalAuth });
    case "receipt":
      return client.deleteVpnSession("ab".repeat(32), { canonicalAuth });
    case "list":
      return client.listVpnReceipts({ canonicalAuth });
    default:
      throw new Error(`unsupported VPN test response kind: ${kind}`);
  }
}
function createResponse({ status, jsonData = {}, arrayData, textBody, headers }) {
  const responseText =
    typeof textBody === "string" ? textBody : JSON.stringify(jsonData ?? {});
  const bodyBytes =
    arrayData instanceof ArrayBuffer
      ? new Uint8Array(arrayData)
      : ArrayBuffer.isView(arrayData)
        ? new Uint8Array(
            arrayData.buffer,
            arrayData.byteOffset,
            arrayData.byteLength,
          )
        : new TextEncoder().encode(responseText);
  return {
    status,
    json: async () => jsonData,
    arrayBuffer: async () => {
      if (arrayData instanceof ArrayBuffer) {
        return arrayData;
      }
      if (ArrayBuffer.isView(arrayData)) {
        return arrayData.buffer.slice(arrayData.byteOffset, arrayData.byteOffset + arrayData.byteLength);
      }
      return bodyBytes.buffer.slice(
        bodyBytes.byteOffset,
        bodyBytes.byteOffset + bodyBytes.byteLength,
      );
    },
    text: async () => responseText,
    body: new ReadableStream({
      start(controller) {
        if (bodyBytes.byteLength > 0) controller.enqueue(bodyBytes);
        controller.close();
      },
    }),
    headers: {
      get(name) {
        if (!headers) {
          return null;
        }
        const normalized = name.toLowerCase();
        for (const [key, value] of Object.entries(headers)) {
          if (key.toLowerCase() === normalized) {
            return value;
          }
        }
        return null;
      },
    },
  };
}

test("getVpnProfile normalizes payloads and tolerates missing control plane", async () => {
  let callCount = 0;
  const fetchImpl = async (url, init = {}) => {
    callCount += 1;
    assert.equal(url, `${BASE_URL}/v1/vpn/profile`);
    assert.equal(init.method, "GET");
    assert.equal(init.headers.Accept, "application/json");
    assert.equal(init.redirect, "error");
    if (callCount === 1) {
      return createResponse({
        status: 200,
        jsonData: {
          available: true,
          relay_endpoint: "/dns/torii.exit.example/udp/9443/quic",
          supported_exit_classes: ["standard", "low-latency", "high-security"],
          default_exit_class: "standard",
          lease_secs: 600,
          dns_push_interval_secs: 90,
          meter_family: "soranet.vpn.standard",
          route_pushes: ["0.0.0.0/0", "::/0"],
          excluded_routes: ["127.0.0.0/8"],
          dns_servers: ["1.1.1.1", "2606:4700:4700::1111"],
          tunnel_addresses: ["10.208.0.2/32", "fd53:7261:6574::2/128"],
          mtu_bytes: 1280,
          display_billing_label: "standard · soranet.vpn.standard · 1000000.25 XOR",
          operator_account_id: SAMPLE_ACCOUNT_ID,
          lease_fee: "1000000.25",
          settlement_grace_secs: 120,
          flow_label_bits: 24,
          padding_budget_ms: 80,
          ...sampleVpnTrustPayload("11".repeat(32)),
        },
        headers: { "content-type": "application/json" },
      });
    }
    return createResponse({
      status: 404,
      jsonData: {},
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });

  const profile = await client.getVpnProfile();
  assert.deepEqual(profile, {
    available: true,
    relayEndpoint: "/dns/torii.exit.example/udp/9443/quic",
    supportedExitClasses: ["standard", "low-latency", "high-security"],
    defaultExitClass: "standard",
    leaseSecs: 600,
    dnsPushIntervalSecs: 90,
    meterFamily: "soranet.vpn.standard",
    routePushes: ["0.0.0.0/0", "::/0"],
    excludedRoutes: ["127.0.0.0/8"],
    dnsServers: ["1.1.1.1", "2606:4700:4700::1111"],
    tunnelAddresses: ["10.208.0.2/32", "fd53:7261:6574::2/128"],
    mtuBytes: 1280,
    displayBillingLabel: "standard · soranet.vpn.standard · 1000000.25 XOR",
    operatorAccountId: SAMPLE_ACCOUNT_ID,
    leaseFee: "1000000.25",
    settlementGraceSecs: 120,
    flowLabelBits: 24,
    paddingBudgetMs: 80,
    ...sampleVpnTrustModel("11".repeat(32)),
  });
  const missing = await client.getVpnProfile();
  assert.equal(missing, null);
});

test("VPN requests reject insecure transport before dispatch", async () => {
  let dispatched = false;
  const client = new ToriiClient("http://torii.example", {
    fetchImpl: async () => {
      dispatched = true;
      throw new Error("insecure VPN request reached dispatch");
    },
  });

  await assert.rejects(
    () => client.getVpnProfile(),
    /require an HTTPS Torii base URL/u,
  );
  assert.equal(dispatched, false);
});

test("unavailable VPN profile accepts only the explicit empty trust tuple", async () => {
  const payload = {
    ...sampleVpnProfilePayload(),
    available: false,
    relay_endpoint: "",
    relay_id_hex: "",
    descriptor_commit_hex: "",
    tls_server_name: "",
    relay_tls_spki_sha256_hex: "",
    relay_certificate_sha256_hex: "",
    directory_snapshot_digest_hex: "",
  };
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({
      status: 200,
      jsonData: payload,
      headers: { "content-type": "application/json" },
    }),
  });

  const profile = await client.getVpnProfile();

  assert.equal(profile.available, false);
  assert.equal(profile.relayEndpoint, "");
  assert.equal(profile.relayIdHex, "");
});

test("available VPN profile rejects malformed trust tuple values", async () => {
  const invalidValues = [
    ["relay_id_hex", "00".repeat(32)],
    ["descriptor_commit_hex", "00".repeat(32)],
    ["descriptor_commit_hex", `0x${"cd".repeat(32)}`],
    ["tls_server_name", "Relay.Example"],
    ["tls_server_name", "-relay.example"],
    ["relay_endpoint", "/dns4/Relay.Example/udp/443/quic"],
    ["relay_endpoint", "/dns4/relay.example/udp/0443/quic"],
    ["relay_endpoint", "/dns4/relay.example/tcp/443/quic"],
  ];
  for (const [field, value] of invalidValues) {
    const payload = { ...sampleVpnProfilePayload(), [field]: value };
    const client = new ToriiClient(BASE_URL, {
      fetchImpl: async () => createResponse({
        status: 200,
        jsonData: payload,
        headers: { "content-type": "application/json" },
      }),
    });

    await assert.rejects(() => client.getVpnProfile(), undefined, field);
  }
});

test("getVpnProfile rejects noncanonical exact fee quantities", async () => {
  for (const leaseFee of [1000000, "01", "1.0", "-1"]) {
    const client = new ToriiClient(BASE_URL, {
      fetchImpl: async () =>
        createResponse({
          status: 200,
          jsonData: {
            available: true,
            relay_endpoint: "/dns/torii.exit.example/udp/9443/quic",
            supported_exit_classes: ["standard", "low-latency", "high-security"],
            default_exit_class: "standard",
            lease_secs: 600,
            dns_push_interval_secs: 90,
            meter_family: "soranet.vpn.standard",
            route_pushes: [],
            excluded_routes: [],
            dns_servers: ["1.1.1.1"],
            tunnel_addresses: ["10.208.0.2/32"],
            mtu_bytes: 1280,
            display_billing_label: "standard",
            operator_account_id: SAMPLE_ACCOUNT_ID,
            lease_fee: leaseFee,
            settlement_grace_secs: 120,
            flow_label_bits: 24,
            padding_budget_ms: 80,
            ...sampleVpnTrustPayload(),
          },
          headers: { "content-type": "application/json" },
        }),
    });

    await assert.rejects(() => client.getVpnProfile(), /lease_fee/);
  }
});

test("getVpnProfile requires dns_push_interval_secs of at least 30", async () => {
  const invalidProfiles = [
    ["missing", (payload) => delete payload.dns_push_interval_secs],
    ["below minimum", (payload) => { payload.dns_push_interval_secs = 29; }],
  ];
  for (const [caseName, mutate] of invalidProfiles) {
    const payload = sampleVpnProfilePayload();
    mutate(payload);
    const client = new ToriiClient(BASE_URL, {
      fetchImpl: async () =>
        createResponse({
          status: 200,
          jsonData: payload,
          headers: { "content-type": "application/json" },
        }),
    });

    await assert.rejects(
      () => client.getVpnProfile(),
      /dns_push_interval_secs/u,
      caseName,
    );
  }
});

test("VPN requests reject unknown fields before dispatch", async () => {
  const canonicalAuth = {
    accountId: CANONICAL_AUTH_ALIAS,
    privateKey: Buffer.alloc(32, 3),
  };
  let dispatched = false;
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      dispatched = true;
      throw new Error("request with unknown fields reached dispatch");
    },
  });
  const cases = [
    () =>
      client.createVpnQuote(
        { meteringPublicKeyHex: "ab".repeat(32), unexpected: true },
        { canonicalAuth },
      ),
    () =>
      client.createVpnSession(
        {
          quoteId: "cd".repeat(32),
          paymentTxHash: "ef".repeat(32),
          meteringPublicKeyHex: "ab".repeat(32),
          unexpected: true,
        },
        { canonicalAuth },
      ),
    () =>
      client.submitVpnReceipt(
        { relayReceiptHex: "abcd", clientVoucherHex: "beef", unexpected: true },
        { canonicalAuth },
      ),
  ];

  for (const invoke of cases) {
    await assert.rejects(invoke, /contains unsupported fields: unexpected/u);
  }
  await assert.rejects(
    () =>
      client.createVpnSession(
        {
          quoteId: `0x${"cd".repeat(32)}`,
          paymentTxHash: "ef".repeat(32),
          meteringPublicKeyHex: "ab".repeat(32),
        },
        { canonicalAuth },
      ),
    /quoteId must be an exact lowercase 32-byte hex string/u,
  );
  await assert.rejects(
    () =>
      client.createVpnQuote(
        { exitClass: "fastest", meteringPublicKeyHex: "ab".repeat(32) },
        { canonicalAuth },
      ),
    /exitClass must be one of/u,
  );
  await assert.rejects(
    () =>
      client.createVpnSession(
        {
          exitClass: "fastest",
          quoteId: "cd".repeat(32),
          paymentTxHash: "ef".repeat(32),
          meteringPublicKeyHex: "ab".repeat(32),
        },
        { canonicalAuth },
      ),
    /exitClass must be one of/u,
  );
  assert.equal(dispatched, false);
});

test("VPN session paths normalize hex before signing and reject malformed IDs", async () => {
  const normalizedSessionId = "ab".repeat(32);
  const inputSessionId = `0X${normalizedSessionId.toUpperCase()}`;
  const privateKey = Buffer.alloc(32, 10);
  const captured = [];
  const fetchImpl = async (url, init = {}) => {
    captured.push({ url, init });
    return createResponse({
      status: 404,
      jsonData: {},
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const canonicalAuth = { accountId: CANONICAL_AUTH_ALIAS, privateKey };

  assert.equal(
    await client.getVpnSession(inputSessionId, { canonicalAuth }),
    null,
  );
  assert.equal(
    await client.deleteVpnSession(inputSessionId, { canonicalAuth }),
    null,
  );
  assert.equal(captured.length, 2);
  for (const { url, init } of captured) {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, `/v1/vpn/sessions/${normalizedSessionId}`);
    const message = canonicalRequestSignatureMessage({
      method: init.method,
      path: parsed.pathname,
      query: "",
      body: "",
      timestampMs: Number(init.headers["X-Iroha-Timestamp-Ms"]),
      nonce: init.headers["X-Iroha-Nonce"],
    });
    assert.deepEqual(
      Buffer.from(init.headers["X-Iroha-Signature"], "base64"),
      signEd25519(message, privateKey),
    );
  }

  await assert.rejects(
    () => client.getVpnSession("not-hex", { canonicalAuth }),
    /sessionId must be a 32-byte hex string/u,
  );
  await assert.rejects(
    () => client.deleteVpnSession("ab", { canonicalAuth }),
    /sessionId must be a 32-byte hex string/u,
  );
  assert.equal(captured.length, 2);
});

test("VPN session responses reject unknown fields and noncanonical IDs or hashes", async () => {
  const canonicalAuth = {
    accountId: CANONICAL_AUTH_ALIAS,
    privateKey: Buffer.alloc(32, 4),
  };
  const requestSession = async (mutate) => {
    const payload = sampleVpnSessionPayload();
    mutate(payload);
    const fetchImpl = async () =>
      createResponse({
        status: 200,
        jsonData: payload,
        headers: { "content-type": "application/json" },
      });
    const client = new ToriiClient(BASE_URL, { fetchImpl });
    return client.getVpnSession("ab".repeat(32), { canonicalAuth });
  };
  const invalidResponses = [
    ["unknown field", (payload) => { payload.unexpected = true; }],
    ["prefixed session id", (payload) => { payload.session_id = `0x${payload.session_id}`; }],
    ["uppercase quote id", (payload) => { payload.quote_id = payload.quote_id.toUpperCase(); }],
    ["prefixed payment hash", (payload) => { payload.payment_tx_hash = `0X${payload.payment_tx_hash}`; }],
    ["uppercase SPKI hash", (payload) => {
      payload.relay_tls_spki_sha256_hex = payload.relay_tls_spki_sha256_hex.toUpperCase();
    }],
  ];

  for (const [caseName, mutate] of invalidResponses) {
    await assert.rejects(
      () => requestSession(mutate),
      /contains unsupported fields|exact lowercase 32-byte hex string/u,
      caseName,
    );
  }
});

test("VPN response parsers require every OpenAPI field", async () => {
  const cases = [
    ["profile", sampleVpnProfilePayload(), "relay_tls_spki_sha256_hex"],
    ["quote", sampleVpnQuotePayload(), "open_lease_instruction"],
    ["session", sampleVpnSessionPayload(), "route_pushes"],
    ["receipt", sampleVpnReceiptPayload(), "settle_lease_instruction"],
    ["list", { items: [sampleVpnReceiptPayload()], total: 1 }, "total"],
  ];
  for (const [kind, payload, missingField] of cases) {
    delete payload[missingField];
    await assert.rejects(
      () => parseVpnTestResponse(kind, payload),
      new RegExp(`missing required fields: ${missingField}`, "u"),
      `${kind}.${missingField}`,
    );
  }

  const nested = sampleVpnQuotePayload();
  delete nested.open_lease_instruction.payload_hex;
  await assert.rejects(
    () => parseVpnTestResponse("quote", nested),
    /missing required fields: payload_hex/u,
  );

  const nullArray = sampleVpnSessionPayload();
  nullArray.route_pushes = null;
  await assert.rejects(
    () => parseVpnTestResponse("session", nullArray),
    /route_pushes must be an array/u,
  );
});

test("VPN response parsers reject empty minLength strings", async () => {
  const cases = [
    [
      "profile",
      sampleVpnProfilePayload,
      [
        "relay_endpoint",
        "meter_family",
        "display_billing_label",
        "operator_account_id",
      ],
    ],
    [
      "quote",
      sampleVpnQuotePayload,
      [
        "payment_reference",
        "account_id",
        "relay_endpoint",
        "fee_asset_id",
        "escrow_account_id",
        "operator_account_id",
        "meter_family",
      ],
    ],
    [
      "session",
      sampleVpnSessionPayload,
      [
        "account_id",
        "relay_endpoint",
        "meter_family",
        "payment_reference",
        "fee_asset_id",
        "escrow_account_id",
        "operator_account_id",
      ],
    ],
    [
      "receipt",
      sampleVpnReceiptPayload,
      [
        "account_id",
        "relay_endpoint",
        "meter_family",
        "fee_asset_id",
        "escrow_account_id",
        "operator_account_id",
      ],
    ],
  ];
  for (const [kind, payloadFactory, fields] of cases) {
    for (const field of fields) {
      const payload = payloadFactory();
      payload[field] = "";
      await assert.rejects(
        () => parseVpnTestResponse(kind, payload),
        new RegExp(field, "u"),
        `${kind}.${field}`,
      );
    }
  }

  const instruction = sampleVpnQuotePayload();
  instruction.open_lease_instruction.wire_id = "";
  await assert.rejects(
    () => parseVpnTestResponse("quote", instruction),
    /wire_id/u,
  );
});

test("VPN response parsers enforce OpenAPI enums and bounds", async () => {
  const cases = [
    ["profile exits", "profile", sampleVpnProfilePayload(), (payload) => {
      payload.supported_exit_classes = ["standard", "standard", "high-security"];
    }, "supported_exit_classes"],
    ["profile lease", "profile", sampleVpnProfilePayload(), (payload) => {
      payload.lease_secs = 0;
    }, "lease_secs"],
    ["profile settlement", "profile", sampleVpnProfilePayload(), (payload) => {
      payload.settlement_grace_secs = 0;
    }, "settlement_grace_secs"],
    ["retired quote instruction array", "quote", sampleVpnQuotePayload(), (payload) => {
      payload.tx_instructions = [];
    }, "tx_instructions"],
    ["quote exit", "quote", sampleVpnQuotePayload(), (payload) => {
      payload.exit_class = "fastest";
    }, "exit_class"],
    ["session mtu", "session", sampleVpnSessionPayload(), (payload) => {
      payload.mtu_bytes = 1500;
    }, "mtu_bytes"],
    ["session flow", "session", sampleVpnSessionPayload(), (payload) => {
      payload.flow_label_bits = 20;
    }, "flow_label_bits"],
    ["session padding", "session", sampleVpnSessionPayload(), (payload) => {
      payload.padding_budget_ms = 0;
    }, "padding_budget_ms"],
    ["session status", "session", sampleVpnSessionPayload(), (payload) => {
      payload.status = "connected";
    }, "status"],
    ["receipt status", "receipt", sampleVpnReceiptPayload(), (payload) => {
      payload.status = "active";
    }, "status"],
    ["receipt source", "receipt", sampleVpnReceiptPayload(), (payload) => {
      payload.receipt_source = "client";
    }, "receipt_source"],
    ["retired receipt instruction array", "receipt", sampleVpnReceiptPayload(), (payload) => {
      payload.tx_instructions = [];
    }, "tx_instructions"],
    ["receipt list item count", "list", {
      items: Array.from({ length: 25 }, () => sampleVpnReceiptPayload()),
      total: 24,
    }, () => {}, "items"],
    ["receipt list total", "list", { items: [], total: 25 }, () => {}, "total"],
  ];
  for (const [caseName, kind, payload, mutate, field] of cases) {
    mutate(payload);
    await assert.rejects(
      () => parseVpnTestResponse(kind, payload),
      new RegExp(field, "u"),
      caseName,
    );
  }
});

test("createVpnQuote returns the native lease-open instruction", async () => {
  const canonicalAuth = {
    accountId: CANONICAL_AUTH_ALIAS,
    privateKey: Buffer.alloc(32, 8),
  };
  const quoteId = "22".repeat(32);
  const sessionIdHex = "33".repeat(16);
  const meteringPublicKeyHex = "a4".repeat(32);
  const openInstruction = {
    wire_id: "OpenVpnLeaseEscrow",
    payload_hex: "abcd",
  };
  const fetchImpl = async (url, init = {}) => {
    assert.equal(url, `${BASE_URL}/v1/vpn/quotes`);
    assert.equal(init.method, "POST");
    assert.equal(init.headers.Accept, "application/json");
    assert.equal(init.headers["Content-Type"], "application/json");
    assert.equal(init.headers["X-Iroha-Account"], CANONICAL_AUTH_ALIAS);
    assert.ok(typeof init.headers["X-Iroha-Signature"] === "string");
    assert.deepEqual(JSON.parse(init.body), {
      exit_class: "low-latency",
      metering_public_key_hex: meteringPublicKeyHex,
    });
    return createResponse({
      status: 201,
      jsonData: {
        quote_id: quoteId,
        lease_id_hex: quoteId,
        session_id_hex: sessionIdHex,
        payment_reference: quoteId,
        account_id: SAMPLE_ACCOUNT_ID,
        exit_class: "low-latency",
        relay_endpoint: "/dns/torii.exit.example/udp/9443/quic",
        lease_secs: 600,
        quote_expires_at_ms: 1_700_000_000_000,
        fee_asset_id: "xor#universal.universal",
        escrow_account_id: "vpn_escrow",
        operator_account_id: SAMPLE_ACCOUNT_ID,
        lease_fee: "1000000.25",
        route_pushes: ["0.0.0.0/0"],
        excluded_routes: ["127.0.0.0/8"],
        dns_servers: ["1.1.1.1"],
        tunnel_addresses: ["10.208.0.2/32"],
        mtu_bytes: 1280,
        meter_family: "soranet.vpn.low-latency",
        flow_label_bits: 24,
        padding_budget_ms: 80,
        ...sampleVpnTrustPayload("55".repeat(32)),
        metering_public_key_hex: meteringPublicKeyHex,
        open_lease_instruction: openInstruction,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });

  const quote = await client.createVpnQuote(
    {
      exitClass: "low-latency",
      meteringPublicKeyHex: `0X${meteringPublicKeyHex.toUpperCase()}`,
    },
    { canonicalAuth },
  );

  assert.deepEqual(quote, {
    quoteId,
    leaseIdHex: quoteId,
    sessionIdHex,
    paymentReference: quoteId,
    accountId: SAMPLE_ACCOUNT_ID,
    exitClass: "low-latency",
    relayEndpoint: "/dns/torii.exit.example/udp/9443/quic",
    leaseSecs: 600,
    quoteExpiresAtMs: 1_700_000_000_000,
    feeAssetId: "xor#universal.universal",
    escrowAccountId: "vpn_escrow",
    operatorAccountId: SAMPLE_ACCOUNT_ID,
    leaseFee: "1000000.25",
    routePushes: ["0.0.0.0/0"],
    excludedRoutes: ["127.0.0.0/8"],
    dnsServers: ["1.1.1.1"],
    tunnelAddresses: ["10.208.0.2/32"],
    mtuBytes: 1280,
    meterFamily: "soranet.vpn.low-latency",
    flowLabelBits: 24,
    paddingBudgetMs: 80,
    ...sampleVpnTrustModel("55".repeat(32)),
    meteringPublicKeyHex,
    openLeaseInstruction: { wireId: "OpenVpnLeaseEscrow", payloadHex: "abcd" },
  });
});

test("createVpnSession signs the request and normalizes the response", async () => {
  const canonicalAuth = {
    accountId: CANONICAL_AUTH_ALIAS,
    privateKey: Buffer.alloc(32, 7),
  };
  const quoteId = "66".repeat(32);
  const sessionId = "55".repeat(32);
  const paymentTxHash = "b7".repeat(32);
  const meteringPublicKeyHex = "a8".repeat(32);
  const fetchImpl = async (url, init = {}) => {
    assert.equal(url, `${BASE_URL}/v1/vpn/sessions`);
    assert.equal(init.method, "POST");
    assert.equal(init.headers.Accept, "application/json");
    assert.equal(init.headers["Content-Type"], "application/json");
    assert.equal(init.headers["X-Iroha-Account"], CANONICAL_AUTH_ALIAS);
    assert.ok(typeof init.headers["X-Iroha-Signature"] === "string");
    assert.deepEqual(JSON.parse(init.body), {
      exit_class: "low-latency",
      quote_id: quoteId,
      payment_tx_hash: paymentTxHash,
      metering_public_key_hex: meteringPublicKeyHex,
    });
    return createResponse({
      status: 201,
      jsonData: {
        session_id: sessionId,
        account_id: SAMPLE_ACCOUNT_ID,
        exit_class: "low-latency",
        relay_endpoint: "/dns/torii.exit.example/udp/9443/quic",
        lease_secs: 600,
        expires_at_ms: 1_700_000_000_000,
        connected_at_ms: 1_699_999_400_000,
        meter_family: "soranet.vpn.low-latency",
        quote_id: quoteId,
        payment_reference: quoteId,
        payment_tx_hash: paymentTxHash,
        fee_asset_id: "xor#universal.universal",
        escrow_account_id: "vpn_escrow",
        operator_account_id: SAMPLE_ACCOUNT_ID,
        lease_fee: "1000000.25",
        flow_label_bits: 24,
        padding_budget_ms: 80,
        ...sampleVpnTrustPayload("99".repeat(32)),
        route_pushes: [],
        excluded_routes: [],
        dns_servers: ["1.1.1.1"],
        tunnel_addresses: ["10.208.0.2/32"],
        mtu_bytes: 1280,
        helper_ticket_hex: SAMPLE_VPN_HELPER_TICKET_HEX,
        bytes_in: 123,
        bytes_out: 456,
        status: "active",
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });

  const session = await client.createVpnSession(
    {
      exitClass: "low-latency",
      quoteId,
      paymentTxHash: `0x${paymentTxHash.toUpperCase()}`,
      meteringPublicKeyHex: `0X${meteringPublicKeyHex.toUpperCase()}`,
    },
    { canonicalAuth },
  );

  assert.deepEqual(session, {
    sessionId,
    accountId: SAMPLE_ACCOUNT_ID,
    exitClass: "low-latency",
    relayEndpoint: "/dns/torii.exit.example/udp/9443/quic",
    leaseSecs: 600,
    expiresAtMs: 1_700_000_000_000,
    connectedAtMs: 1_699_999_400_000,
    meterFamily: "soranet.vpn.low-latency",
    quoteId,
    paymentReference: quoteId,
    paymentTxHash,
    feeAssetId: "xor#universal.universal",
    escrowAccountId: "vpn_escrow",
    operatorAccountId: SAMPLE_ACCOUNT_ID,
    leaseFee: "1000000.25",
    flowLabelBits: 24,
    paddingBudgetMs: 80,
    ...sampleVpnTrustModel("99".repeat(32)),
    routePushes: [],
    excludedRoutes: [],
    dnsServers: ["1.1.1.1"],
    tunnelAddresses: ["10.208.0.2/32"],
    mtuBytes: 1280,
    helperTicketHex: SAMPLE_VPN_HELPER_TICKET_HEX,
    bytesIn: 123,
    bytesOut: 456,
    status: "active",
  });
});

test("VPN session responses require an exact lowercase 664-byte helper ticket", async () => {
  const canonicalAuth = {
    accountId: CANONICAL_AUTH_ALIAS,
    privateKey: Buffer.alloc(32, 7),
  };
  const requestSession = async (helperTicketHex) => {
    const fetchImpl = async () =>
      createResponse({
        status: 200,
        jsonData: sampleVpnSessionPayload(helperTicketHex),
        headers: { "content-type": "application/json" },
      });
    const client = new ToriiClient(BASE_URL, { fetchImpl });
    return client.getVpnSession("55".repeat(32), { canonicalAuth });
  };

  const session = await requestSession(SAMPLE_VPN_HELPER_TICKET_HEX);
  assert.equal(session.helperTicketHex, SAMPLE_VPN_HELPER_TICKET_HEX);
  assert.equal(session.helperTicketHex.length, 1328);

  const invalidTickets = [
    ["prefix", `0x${SAMPLE_VPN_HELPER_TICKET_HEX}`],
    ["uppercase", SAMPLE_VPN_HELPER_TICKET_HEX.toUpperCase()],
    ["odd length", SAMPLE_VPN_HELPER_TICKET_HEX.slice(0, -1)],
    ["wrong even length", SAMPLE_VPN_HELPER_TICKET_HEX.slice(0, -2)],
  ];
  for (const [caseName, helperTicketHex] of invalidTickets) {
    await assert.rejects(
      () => requestSession(helperTicketHex),
      /helper_ticket_hex must contain exactly 1328 lowercase hexadecimal characters/u,
      caseName,
    );
  }
});

test("createVpnSession requires canonical auth options", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch should not run");
    },
  });
  await assert.rejects(
    () => client.createVpnSession({ exitClass: "standard" }),
    /createVpnSession options\.canonicalAuth is required/,
  );
});

test("deleteVpnSession returns null when the session is already missing", async () => {
  const requestedSessionId = "13".repeat(32);
  const canonicalAuth = {
    accountId: CANONICAL_AUTH_ALIAS,
    privateKey: Buffer.alloc(32, 9),
  };
  const fetchImpl = async (url, init = {}) => {
    assert.equal(url, `${BASE_URL}/v1/vpn/sessions/${requestedSessionId}`);
    assert.equal(init.method, "DELETE");
    assert.equal(init.headers.Accept, "application/json");
    assert.equal(init.headers["X-Iroha-Account"], CANONICAL_AUTH_ALIAS);
    return createResponse({
      status: 404,
      jsonData: {
        session_id: requestedSessionId,
        status: "not_found",
        disconnected_at_ms: 1_700_000_000_000,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.deleteVpnSession(requestedSessionId, { canonicalAuth });
  assert.equal(result, null);
});

test("getVpnSession and listVpnReceipts normalize authenticated responses", async () => {
  const canonicalAuth = {
    accountId: CANONICAL_AUTH_ALIAS,
    privateKey: Buffer.alloc(32, 5),
  };
  const quoteId = "aa".repeat(32);
  const sessionId = "cc".repeat(32);
  const requestedSessionId = "de".repeat(32);
  const paymentTxHash = "bb".repeat(32);
  let callCount = 0;
  const fetchImpl = async (url, init = {}) => {
    callCount += 1;
    assert.equal(init.headers.Accept, "application/json");
    assert.equal(init.headers["X-Iroha-Account"], CANONICAL_AUTH_ALIAS);
    if (callCount === 1) {
      assert.equal(url, `${BASE_URL}/v1/vpn/sessions/${requestedSessionId}`);
      assert.equal(init.method, "GET");
      return createResponse({
        status: 200,
        jsonData: {
          session_id: sessionId,
          account_id: SAMPLE_ACCOUNT_ID,
          exit_class: "standard",
          relay_endpoint: "/dns/torii.exit.example/udp/9443/quic",
          lease_secs: 600,
          expires_at_ms: 1_700_000_000_000,
          connected_at_ms: 1_699_999_800_000,
          meter_family: "soranet.vpn.standard",
          quote_id: quoteId,
          payment_reference: quoteId,
          payment_tx_hash: paymentTxHash,
          fee_asset_id: "xor#universal.universal",
          escrow_account_id: "vpn_escrow",
          operator_account_id: SAMPLE_ACCOUNT_ID,
          lease_fee: "1000000.25",
          flow_label_bits: 24,
          padding_budget_ms: 80,
          ...sampleVpnTrustPayload("cc".repeat(32)),
          route_pushes: ["0.0.0.0/0"],
          excluded_routes: ["127.0.0.0/8"],
          dns_servers: ["1.1.1.1"],
          tunnel_addresses: ["10.208.0.2/32"],
          mtu_bytes: 1280,
          helper_ticket_hex: SAMPLE_VPN_HELPER_TICKET_HEX,
          bytes_in: 11,
          bytes_out: 22,
          status: "active",
        },
        headers: { "content-type": "application/json" },
      });
    }
    assert.equal(url, `${BASE_URL}/v1/vpn/receipts`);
    assert.equal(init.method, "GET");
    return createResponse({
      status: 200,
      jsonData: {
        items: [
          {
            session_id: sessionId,
            account_id: SAMPLE_ACCOUNT_ID,
            exit_class: "standard",
            relay_endpoint: "/dns/torii.exit.example/udp/9443/quic",
            meter_family: "soranet.vpn.standard",
            connected_at_ms: 1_699_999_800_000,
            disconnected_at_ms: 1_700_000_100_000,
            duration_ms: 300000,
            bytes_in: 11,
            bytes_out: 22,
            status: "disconnected",
            receipt_source: "torii",
            quote_id: quoteId,
            payment_tx_hash: paymentTxHash,
            fee_asset_id: "xor#universal.universal",
            escrow_account_id: "vpn_escrow",
            operator_account_id: SAMPLE_ACCOUNT_ID,
            lease_fee: "1000000.25",
            earned_fee: "0",
            refunded_fee: "1000000.25",
            lease_id_hex: quoteId,
            settle_lease_instruction: null,
          },
        ],
        total: 1,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });

  const session = await client.getVpnSession(requestedSessionId, { canonicalAuth });
  assert.deepEqual(session, {
    sessionId,
    accountId: SAMPLE_ACCOUNT_ID,
    exitClass: "standard",
    relayEndpoint: "/dns/torii.exit.example/udp/9443/quic",
    leaseSecs: 600,
    expiresAtMs: 1_700_000_000_000,
    connectedAtMs: 1_699_999_800_000,
    meterFamily: "soranet.vpn.standard",
    quoteId,
    paymentReference: quoteId,
    paymentTxHash,
    feeAssetId: "xor#universal.universal",
    escrowAccountId: "vpn_escrow",
    operatorAccountId: SAMPLE_ACCOUNT_ID,
    leaseFee: "1000000.25",
    flowLabelBits: 24,
    paddingBudgetMs: 80,
    ...sampleVpnTrustModel("cc".repeat(32)),
    routePushes: ["0.0.0.0/0"],
    excludedRoutes: ["127.0.0.0/8"],
    dnsServers: ["1.1.1.1"],
    tunnelAddresses: ["10.208.0.2/32"],
    mtuBytes: 1280,
    helperTicketHex: SAMPLE_VPN_HELPER_TICKET_HEX,
    bytesIn: 11,
    bytesOut: 22,
    status: "active",
  });

  const receipts = await client.listVpnReceipts({ canonicalAuth });
  assert.deepEqual(receipts, {
    items: [{
      sessionId,
      accountId: SAMPLE_ACCOUNT_ID,
      exitClass: "standard",
      relayEndpoint: "/dns/torii.exit.example/udp/9443/quic",
      meterFamily: "soranet.vpn.standard",
      connectedAtMs: 1_699_999_800_000,
      disconnectedAtMs: 1_700_000_100_000,
      durationMs: 300000,
      bytesIn: 11,
      bytesOut: 22,
      status: "disconnected",
      receiptSource: "torii",
      quoteId,
      paymentTxHash,
      feeAssetId: "xor#universal.universal",
      escrowAccountId: "vpn_escrow",
      operatorAccountId: SAMPLE_ACCOUNT_ID,
      leaseFee: "1000000.25",
      earnedFee: "0",
      refundedFee: "1000000.25",
      leaseIdHex: quoteId,
      settleLeaseInstruction: null,
    }],
    total: 1,
  });
});

test("deleteVpnSession normalizes canonical receipts", async () => {
  const requestedSessionId = "79".repeat(32);
  const canonicalAuth = {
    accountId: CANONICAL_AUTH_ALIAS,
    privateKey: Buffer.alloc(32, 6),
  };
  const quoteId = "dd".repeat(32);
  const sessionId = "ff".repeat(32);
  const paymentTxHash = "ee".repeat(32);
  const fetchImpl = async (url, init = {}) => {
    assert.equal(url, `${BASE_URL}/v1/vpn/sessions/${requestedSessionId}`);
    assert.equal(init.method, "DELETE");
    return createResponse({
      status: 200,
      jsonData: {
        session_id: sessionId,
        account_id: SAMPLE_ACCOUNT_ID,
        exit_class: "high-security",
        relay_endpoint: "/dns/torii.exit.example/udp/9443/quic",
        meter_family: "soranet.vpn.high-security",
        connected_at_ms: 1_699_999_700_000,
        disconnected_at_ms: 1_700_000_000_000,
        duration_ms: 300000,
        bytes_in: 99,
        bytes_out: 33,
        status: "disconnected",
        receipt_source: "torii",
        quote_id: quoteId,
        payment_tx_hash: paymentTxHash,
        fee_asset_id: "xor#universal.universal",
        escrow_account_id: "vpn_escrow",
        operator_account_id: SAMPLE_ACCOUNT_ID,
        lease_fee: "1000000.25",
        earned_fee: "0",
        refunded_fee: "1000000.25",
        lease_id_hex: quoteId,
        settle_lease_instruction: null,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const receipt = await client.deleteVpnSession(requestedSessionId, { canonicalAuth });
  assert.deepEqual(receipt, {
    sessionId,
    accountId: SAMPLE_ACCOUNT_ID,
    exitClass: "high-security",
    relayEndpoint: "/dns/torii.exit.example/udp/9443/quic",
    meterFamily: "soranet.vpn.high-security",
    connectedAtMs: 1_699_999_700_000,
    disconnectedAtMs: 1_700_000_000_000,
    durationMs: 300000,
    bytesIn: 99,
    bytesOut: 33,
    status: "disconnected",
    receiptSource: "torii",
    quoteId,
    paymentTxHash,
    feeAssetId: "xor#universal.universal",
    escrowAccountId: "vpn_escrow",
    operatorAccountId: SAMPLE_ACCOUNT_ID,
    leaseFee: "1000000.25",
    earnedFee: "0",
    refundedFee: "1000000.25",
    leaseIdHex: quoteId,
    settleLeaseInstruction: null,
  });
});

test("submitVpnReceipt posts metering evidence and exposes settlement instructions", async () => {
  const canonicalAuth = {
    accountId: CANONICAL_AUTH_ALIAS,
    privateKey: Buffer.alloc(32, 4),
  };
  const quoteId = "12".repeat(32);
  const sessionId = "56".repeat(32);
  const paymentTxHash = "34".repeat(32);
  const settleInstruction = {
    wire_id: "SettleVpnLease",
    payload_hex: "cafe",
  };
  const fetchImpl = async (url, init = {}) => {
    assert.equal(url, `${BASE_URL}/v1/vpn/receipts`);
    assert.equal(init.method, "POST");
    assert.equal(init.headers.Accept, "application/json");
    assert.equal(init.headers["Content-Type"], "application/json");
    assert.equal(init.headers["X-Iroha-Account"], CANONICAL_AUTH_ALIAS);
    assert.ok(typeof init.headers["X-Iroha-Signature"] === "string");
    assert.deepEqual(JSON.parse(init.body), {
      relay_receipt_hex: "abcd",
      client_voucher_hex: "beef",
      lease_id_hex: quoteId,
    });
    return createResponse({
      status: 201,
      jsonData: {
        session_id: sessionId,
        account_id: SAMPLE_ACCOUNT_ID,
        exit_class: "standard",
        relay_endpoint: "/dns/torii.exit.example/udp/9443/quic",
        meter_family: "soranet.vpn.standard",
        connected_at_ms: 1_699_999_700_000,
        disconnected_at_ms: 1_700_000_000_000,
        duration_ms: 300000,
        bytes_in: 99,
        bytes_out: 33,
        status: "settled",
        receipt_source: "relay",
        quote_id: quoteId,
        payment_tx_hash: paymentTxHash,
        fee_asset_id: "xor#universal.universal",
        escrow_account_id: "vpn_escrow",
        operator_account_id: SAMPLE_ACCOUNT_ID,
        lease_fee: "1000000.25",
        earned_fee: "500000.125",
        refunded_fee: "500000.125",
        lease_id_hex: quoteId,
        settle_lease_instruction: settleInstruction,
      },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });

  const receipt = await client.submitVpnReceipt(
    {
      relayReceiptHex: "0xABCD",
      clientVoucherHex: "0xBEEF",
      leaseIdHex: quoteId,
    },
    { canonicalAuth },
  );

  assert.deepEqual(receipt, {
    sessionId,
    accountId: SAMPLE_ACCOUNT_ID,
    exitClass: "standard",
    relayEndpoint: "/dns/torii.exit.example/udp/9443/quic",
    meterFamily: "soranet.vpn.standard",
    connectedAtMs: 1_699_999_700_000,
    disconnectedAtMs: 1_700_000_000_000,
    durationMs: 300000,
    bytesIn: 99,
    bytesOut: 33,
    status: "settled",
    receiptSource: "relay",
    quoteId,
    paymentTxHash,
    feeAssetId: "xor#universal.universal",
    escrowAccountId: "vpn_escrow",
    operatorAccountId: SAMPLE_ACCOUNT_ID,
    leaseFee: "1000000.25",
    earnedFee: "500000.125",
    refundedFee: "500000.125",
    leaseIdHex: quoteId,
    settleLeaseInstruction: { wireId: "SettleVpnLease", payloadHex: "cafe" },
  });
});
