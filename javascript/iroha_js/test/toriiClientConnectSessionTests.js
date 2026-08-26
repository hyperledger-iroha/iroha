// Exact-identity Connect session Torii client regression registrations.

import { generateConnectSid } from "../src/connectSession.js";
import { computeHashLiteralCrc } from "../src/hashLiteralCrc.js";

export function registerToriiClientConnectSessionTests({
  assert,
  BASE_URL,
  NetworkId,
  ToriiClient,
  VK_SIGNING_NETWORK_ID,
  createResponse,
  test,
}) {
  const appPublicKey = Buffer.alloc(32, 0x22);
  const nonce = Buffer.alloc(16, 0x33);
  const sid = generateConnectSid({
    networkId: VK_SIGNING_NETWORK_ID,
    appPublicKey,
    nonce,
  }).sidBase64Url;
  const tokenApp = "A".repeat(43);
  const tokenWallet = "B".repeat(43);
  const tokenManagement = "C".repeat(43);
  const tokenRelay = "D".repeat(43);

  function markedNetworkId(networkId = VK_SIGNING_NETWORK_ID) {
    const body = networkId.toString().toUpperCase();
    return `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
  }

  function sampleInput(overrides = {}) {
    return {
      sid,
      networkId: VK_SIGNING_NETWORK_ID,
      appPublicKey,
      nonce,
      ...overrides,
    };
  }

  function sampleResponse(overrides = {}, node = "") {
    const responseIdentity = {
      sid,
      network_id: markedNetworkId(),
      app_pk: appPublicKey.toString("base64url"),
      nonce: nonce.toString("base64url"),
    };
    const deepLinkIdentity = {
      ...responseIdentity,
      network_id: VK_SIGNING_NETWORK_ID.toString(),
    };
    const uri = (role, token) => {
      const params = new URLSearchParams({
        ...deepLinkIdentity,
        node,
        v: "1",
        role,
        token,
        relay: tokenRelay,
      });
      return `iroha://connect?${params}`;
    };
    return {
      ...responseIdentity,
      wallet_uri: uri("wallet", tokenWallet),
      app_uri: uri("app", tokenApp),
      token_app: tokenApp,
      token_wallet: tokenWallet,
      token_management: tokenManagement,
      token_relay: tokenRelay,
      ...overrides,
    };
  }

  test("createConnectSession validates sid and posts JSON", async () => {
    let captured;
    const fetchImpl = async (url, init) => {
      captured = { url, init };
      return createResponse({
        status: 200,
        jsonData: sampleResponse({}, "torii"),
        headers: { "content-type": "application/json" },
      });
    };
    const client = new ToriiClient(BASE_URL, { fetchImpl });
    const response = await client.createConnectSession(
      sampleInput({ node: "torii" }),
    );
    assert.equal(response.sid, sid);
    assert.match(response.wallet_uri, /^iroha:\/\/connect\?/u);
    assert.match(response.app_uri, /^iroha:\/\/connect\?/u);
    assert.equal(response.token_app, tokenApp);
    assert.equal(response.token_wallet, tokenWallet);
    assert.equal(response.token_management, tokenManagement);
    assert.equal(response.token_relay, tokenRelay);
    assert.deepEqual(response.extra, {});
    assert.equal(captured.url, `${BASE_URL}/v1/connect/session`);
    assert.equal(captured.init.headers["Content-Type"], "application/json");
    assert.deepEqual(JSON.parse(captured.init.body), {
      sid,
      network_id: markedNetworkId(),
      app_pk: appPublicKey.toString("base64url"),
      nonce: nonce.toString("base64url"),
      node: "torii",
    });
    assert.equal(
      new URL(response.wallet_uri).searchParams.get("network_id"),
      VK_SIGNING_NETWORK_ID.toString(),
    );
  });

  test("createConnectSession rejects malformed responses", async () => {
    const client = new ToriiClient(BASE_URL, {
      fetchImpl: async () =>
        createResponse({
          status: 200,
          jsonData: sampleResponse({ token_wallet: undefined }),
          headers: { "content-type": "application/json" },
        }),
    });
    await assert.rejects(
      () => client.createConnectSession(sampleInput()),
      /token_wallet/i,
    );
  });

  test("createConnectSession accepts base64url sid", async () => {
    let captured;
    const fetchImpl = async (url, init) => {
      captured = { url, init };
      return createResponse({
        status: 200,
        jsonData: sampleResponse(),
        headers: { "content-type": "application/json" },
      });
    };
    const client = new ToriiClient(BASE_URL, { fetchImpl });
    const response = await client.createConnectSession(sampleInput());
    assert.equal(response.sid, sid);
    assert.equal(captured.url, `${BASE_URL}/v1/connect/session`);
    assert.deepEqual(JSON.parse(captured.init.body), {
      sid,
      network_id: markedNetworkId(),
      app_pk: appPublicKey.toString("base64url"),
      nonce: nonce.toString("base64url"),
    });
  });

  test("createConnectSession rejects raw or noncanonical NetworkId response JSON", async () => {
    const canonical = markedNetworkId();
    const badChecksum = `${canonical.slice(0, -1)}${canonical.endsWith("0") ? "1" : "0"}`;
    for (const networkId of [
      VK_SIGNING_NETWORK_ID.toString(),
      canonical.toLowerCase(),
      badChecksum,
    ]) {
      const client = new ToriiClient(BASE_URL, {
        fetchImpl: async () => createResponse({
          status: 200,
          jsonData: sampleResponse({ network_id: networkId }),
          headers: { "content-type": "application/json" },
        }),
      });
      // eslint-disable-next-line no-await-in-loop
      await assert.rejects(
        () => client.createConnectSession(sampleInput()),
        /network_id.*canonical marked Iroha hash literal|invalid checksum/u,
      );
    }
  });

  test("createConnectSession rejects invalid sid values", async () => {
    const client = new ToriiClient(BASE_URL, {
      fetchImpl: async () => {
        throw new Error("should not be called");
      },
    });
    await assert.rejects(
      () => client.createConnectSession(sampleInput({ sid: "not a valid sid" })),
      /32-byte.*(base64url|hex)/i,
    );
  });

  test("createConnectSession rejects a SID derived for another exact network", async () => {
    const otherNetwork = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));
    const wrongSid = generateConnectSid({
      networkId: otherNetwork,
      appPublicKey,
      nonce,
    }).sidBase64Url;
    const client = new ToriiClient(BASE_URL, {
      fetchImpl: async () => {
        throw new Error("should not be called");
      },
    });
    await assert.rejects(
      () => client.createConnectSession(sampleInput({ sid: wrongSid })),
      /exact networkId, appPublicKey, and nonce/,
    );
  });

  test("createConnectSession rejects substituted or duplicate deep-link identity", async () => {
    for (const mutate of [
      (response) => {
        const uri = new URL(response.wallet_uri);
        uri.searchParams.set("app_pk", Buffer.alloc(32, 0x99).toString("base64url"));
        response.wallet_uri = uri.toString();
      },
      (response) => {
        response.wallet_uri += `&sid=${response.sid}`;
      },
      (response) => {
        const uri = new URL(response.app_uri);
        uri.searchParams.set("relay", "E".repeat(43));
        response.app_uri = uri.toString();
      },
    ]) {
      const response = sampleResponse();
      mutate(response);
      const client = new ToriiClient(BASE_URL, {
        fetchImpl: async () => createResponse({
          status: 200,
          jsonData: response,
          headers: { "content-type": "application/json" },
        }),
      });
      await assert.rejects(
        () => client.createConnectSession(sampleInput()),
        /(substituted|duplicate)/u,
      );
    }
  });

  test("deleteConnectSession returns flag based on status", async () => {
    let captured;
    const fetchImpl = async (url, init) => {
      captured = { url, init };
      return createResponse({ status: 204 });
    };
    const client = new ToriiClient(BASE_URL, { fetchImpl });
    const ok = await client.deleteConnectSession({
      sid,
      tokenManagement: "token-management",
    });
    assert.equal(ok, true);
    assert.equal(
      captured.url,
      `${BASE_URL}/v1/connect/session/${encodeURIComponent(sid)}`,
    );
    assert.equal(captured.init.method, "DELETE");
    assert.equal(captured.init.headers.Authorization, "Bearer token-management");
  });

  test("deleteConnectSession returns false for missing session", async () => {
    const client = new ToriiClient(BASE_URL, {
      fetchImpl: async () => createResponse({ status: 404 }),
    });
    const ok = await client.deleteConnectSession({
      sid,
      token_management: "token-management",
    });
    assert.equal(ok, false);
  });
}
