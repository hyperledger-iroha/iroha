import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { ToriiClient } from "../src/toriiClient.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import { stringifyStrictLosslessIntegerJson } from "../src/strictLosslessJson.js";
import { SCCP_CAPABILITIES_MAX_BYTES, SCCP_REGISTRY_MAX_BYTES, SCCP_RECENT_MAX_BYTES } from "../src/sccpRead.js";

// Native [u8; N] JSON uses uppercase; recent_cursor's formatter uses lowercase.
const HASH = "AB".repeat(32);
const CURSOR_HASH = "ab".repeat(32);
const NETWORK = { network: "ethereum_mainnet", profile: null };
const ACCOUNT = JSON.parse(readFileSync(new URL("./fixtures/bond-v1-codec.json", import.meta.url), "utf8")).accounts.holder;
const U64_MAX = 18446744073709551615n;
const U128_MAX = (1n << 128n) - 1n;
// Field names and enum tags follow iroha_sccp/src/api/{generation,views}.rs,
// api.rs and iroha_data_model/src/sccp/{registry,deployment,inbound,outbound,params}.rs.
function capabilities() {
  return {
    network_id: HASH, consensus_instance: HASH,
    finality: { x_layout_id: "11".repeat(16), dst_sig: "22".repeat(43), result_tag: "33".repeat(24), result_body_tag: "44".repeat(29) },
    current_generation: { generation: 1, committee_root: HASH, start_height: 2, deadline_ms: U64_MAX },
    coalition_slashable_floor: "100000.125", liability_clock: U64_MAX, forgery_max_generation: null,
    pause_panel_ids: { primary: HASH, backup: HASH }, parliament_binding_ballot_available: true,
    reconciliation_available: false,
    parameters: {
      enabled: false, committee_validity_ms: 1000, committee_heartbeat_ms: 100,
      forgery_evidence_grace_ms: 10, destination_progress_cap_ms: 20, outbound_ttl_ms: 2000,
      min_outbound_amount: "1", inbound_self_claim_fee: "0", max_exempt_transactions_per_block: 8,
      fast_pause_panel_seats: 5, fast_pause_panel_term_ms: 10, fast_pause_seat_accept_window_ms: 2,
      fast_pause_endorse_window_ms: 3, fast_pause_renew_lead_ms: 4, fast_pause_hold_ms: 5,
    },
    committed_height: U64_MAX,
    light_client_profiles: [{ network: NETWORK, version: 1, profile_hash: HASH, activation_height: null, proposal_id: null, supported_until_ms: null }],
    chain_id: "test-chain", profiles: [{ network: NETWORK, supported_until_ms: null }],
    profiles_policy_hash: HASH, limits: { recent_messages: 50, outbound_page: 256, controls_page: 64, rotations_page: 16 },
    path_templates: ["/v1/sccp/capabilities", "/v1/sccp/registry", "/v1/sccp/messages/recent"],
  };
}
function registry() {
  return [{
    network: NETWORK, route_id: "taira_eth_xor", escrow: ACCOUNT, stranded: "0",
    revisions: { "1": {
      revision: 1, deployment: { family: "evm", deployment: { address: "12".repeat(20), runtime_code_hash: HASH } },
      destination_word: HASH, initial_committee: { generation: 1, committee_root: HASH, start_height: 1, until_ms: U64_MAX },
      activation: { activation: "paused", detail: null }, destination_frozen: false, destination_paused: true,
      next_control_nonce: U64_MAX, max_wrapped_supply: U128_MAX.toString(), liability: "1000", next_outbound_nonce: 1,
      registered_at_height: 1, forgery_floor: 0, quarantine: { certificate_id: HASH, since_height: 5 }, ever_activated: true,
    } },
  }];
}
function page(direction = "outbound") {
  const detail = direction === "outbound" ? {
    message_id: HASH,
    record: {
      network: NETWORK, revision: 1, nonce: U64_MAX, height: U64_MAX, commitment_index: 0,
      deadline_ms: U64_MAX, sender: ACCOUNT, amount: U128_MAX.toString(), payload: "AgGquw==", leaf: HASH,
      status: { status: "recorded", detail: null },
    },
    state: { state: "recorded", detail: { deadline_ms: U64_MAX, provable: false } },
  } : {
    message_id: HASH,
    record: {
      network: NETWORK, revision: 1, payload: "AgGquw==", fee_due: "0", proven_at_height: U64_MAX,
      source_locator: { source_height: U64_MAX, block_hash: HASH, index_in_block: 0 },
      status: { status: "released", detail: { height: U64_MAX, payout: U128_MAX } },
    },
  };
  return { messages: [{ direction, detail }], next_before: direction === "outbound" ? "1:0" : `1:${CURSOR_HASH}` };
}
function response(value, headers = {}) {
  return new Response(stringifyStrictLosslessIntegerJson(value, "fixture"), {
    headers: { "content-type": "application/json", ...headers },
  });
}

test("browser exposes credential-free capabilities without native account reads", async () => {
  let init;
  const client = new ToriiBrowserClient("https://torii.example", {
    defaultHeaders: { Authorization: "secret", "x-api-token": "secret" },
    fetchImpl: async (_url, request) => { init = request; return response(capabilities()); },
  });
  const result = await client.getSccpCapabilitiesV1();
  assert.equal(result.committed_height, U64_MAX);
  assert.equal(init.credentials, "omit");
  assert.equal(init.redirect, "error");
  assert.equal(new Headers(init.headers).get("authorization"), null);
  assert.equal(new Headers(init.headers).get("x-api-token"), null);
  assert.equal(client.getSccpRegistryV1, undefined);
  assert.equal(client.listSccpRecentMessagesV1, undefined);
  await assert.rejects(() => client.getSccpCapabilitiesV1({ canonicalAuth: {} }));
  const malformed = capabilities(); delete malformed.current_generation;
  await assert.rejects(() => new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => response(malformed),
  }).getSccpCapabilitiesV1());
});

for (const Client of [ToriiClient]) {
  const name = Client.name;
  function client(fetchImpl, extra = {}) {
    return new Client("https://torii.example/prefix", { fetchImpl, ...extra });
  }
  test(`${name} reads strict public capabilities without inherited credentials`, async () => {
    const calls = [];
    const subject = client(async (url, init) => { calls.push({ url: String(url), init }); return response(capabilities()); }, {
      defaultHeaders: { Authorization: "secret", "x-api-token": "secret" },
    });
    const result = await subject.getSccpCapabilitiesV1();
    assert.equal(result.committed_height, U64_MAX);
    assert.equal(result.parameters.min_outbound_amount, "1");
    assert.equal(result.finality.dst_sig.length, 86);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].url, "https://torii.example/prefix/v1/sccp/capabilities");
    assert.equal(calls[0].init.method, "GET");
    assert.equal(calls[0].init.credentials, "omit");
    assert.equal(calls[0].init.redirect, "error");
    const headers = new Headers(calls[0].init.headers);
    assert.equal(headers.get("accept"), "application/json");
    for (const header of ["authorization", "x-api-token", "x-iroha-account", "x-iroha-signature"]) assert.equal(headers.get(header), null);
  });
  test(`${name} retains native route revisions, quantities and tagged statuses`, async () => {
    const result = await client(async () => response(registry())).getSccpRegistryV1();
    assert.equal(result[0].revisions["1"].next_control_nonce, U64_MAX);
    assert.equal(result[0].revisions["1"].max_wrapped_supply, U128_MAX.toString());
    assert.deepEqual(result[0].network, Object.assign(Object.create(null), NETWORK));
    assert.equal(result[0].revisions["1"].activation.activation, "paused");
  });
  for (const direction of ["inbound", "outbound"]) {
    test(`${name} bounds and encodes ${direction} cursor pages and preserves integers`, async () => {
      let url;
      const before = direction === "outbound" ? `${U64_MAX}:1` : `${U64_MAX}:${CURSOR_HASH}`;
      const result = await client(async (value) => { url = new URL(value); return response(page(direction)); })
        .listSccpRecentMessagesV1({ direction, network: "ethereum-mainnet", before, limit: 2 });
      assert.equal(url.pathname, "/prefix/v1/sccp/messages/recent");
      assert.equal(url.searchParams.get("before"), before);
      assert.equal(url.searchParams.get("direction"), direction);
      assert.equal(url.searchParams.get("network"), "ethereum-mainnet");
      assert.equal(url.searchParams.get("limit"), "2");
      const record = result.messages[0].detail.record;
      assert.equal(direction === "outbound" ? record.nonce : record.status.detail.payout, direction === "outbound" ? U64_MAX : U128_MAX);
    });
  }
  test(`${name} admits an empty native scan page with a continuation cursor`, async () => {
    const result = await client(async () => response({ messages: [], next_before: "5:6" })).listSccpRecentMessagesV1();
    assert.equal(result.next_before, "5:6");
  });
  test(`${name} rejects invalid and retired options before fetch`, async () => {
    let calls = 0;
    const subject = client(async () => { calls += 1; return response({}); });
    for (const options of [{ limit: 0 }, { limit: 51 }, { limit: "1" }, { direction: "all" },
      { network: "tron-mainnet" }, { network: "sora-taira" }, { before: "01:2" },
      { before: "1:4294967296" }, { before: "18446744073709551616:1" },
      { direction: "inbound", before: "1:2" }, { direction: "inbound", before: `1:${HASH}` },
      { cursor: "1:2" }, { canonicalAuth: {} }]) {
      await assert.rejects(() => subject.listSccpRecentMessagesV1(options));
    }
    await assert.rejects(() => subject.getSccpCapabilitiesV1({ headers: {} }));
    await assert.rejects(() => subject.getSccpRegistryV1({ network: "ethereum-mainnet" }));
    assert.equal(calls, 0);
  });
  test(`${name} rejects missing, unknown, numeric-string and malformed capability fields`, async () => {
    for (const mutate of [
      (value) => { delete value.parameters; },
      (value) => { value.retired = true; },
      (value) => { value.committed_height = "1"; },
      (value) => { value.network_id = [1, 2]; },
      (value) => { value.network_id = CURSOR_HASH; },
      (value) => { value.consensus_instance = `aB${HASH.slice(2)}`; },
      (value) => { value.finality.dst_sig = HASH; },
      (value) => { value.parameters.enabled = 1; },
      (value) => { value.profiles[0].network = "ethereum-mainnet"; },
      (value) => { value.coalition_slashable_floor = "01.0"; },
    ]) {
      const fixture = structuredClone(capabilities()); mutate(fixture);
      await assert.rejects(() => client(async () => response(fixture)).getSccpCapabilitiesV1());
    }
  });
  test(`${name} rejects wrong registry routes, retired families and noncanonical amounts`, async () => {
    for (const mutate of [
      (value) => { value[0].route_id = "old-eth-route"; },
      (value) => { value[0].escrow = "alice@wonderland"; },
      (value) => { value[0].revisions["1"].revision = 2; },
      (value) => { value[0].revisions["1"].deployment.family = "tron"; },
      (value) => { value[0].revisions["1"].destination_word = CURSOR_HASH; },
      (value) => { value[0].stranded = U128_MAX; },
      (value) => { value[0].stranded = (U128_MAX + 1n).toString(); },
      (value) => { value.push(structuredClone(value[0])); },
    ]) {
      const fixture = structuredClone(registry()); mutate(fixture);
      await assert.rejects(() => client(async () => response(fixture)).getSccpRegistryV1());
    }
  });
  test(`${name} rejects wrong direction, network, page count, payload and enum shapes`, async () => {
    for (const mutate of [
      (value) => { value.messages[0].direction = "unknown"; },
      (value) => { value.messages[0].detail.record.network.network = "bsc_mainnet"; },
      (value) => { value.messages.push(structuredClone(value.messages[0])); },
      (value) => { value.messages[0].detail.record.payload = "AB=="; },
      (value) => { value.messages[0].detail.message_id = CURSOR_HASH; },
      (value) => { value.messages[0].detail.record.status.detail = {}; },
      (value) => { value.next_before = `1:${HASH}`; },
    ]) {
      const fixture = structuredClone(page()); mutate(fixture);
      await assert.rejects(() => client(async () => response(fixture))
        .listSccpRecentMessagesV1({ network: "ethereum-mainnet", limit: 1 }));
    }
    await assert.rejects(() => client(async () => response(page("inbound"))).listSccpRecentMessagesV1());
  });
  test(`${name} requires JSON, valid UTF-8 and rejects duplicate or fractional numbers`, async () => {
    for (const body of ["{\"messages\":[],\"messages\":[],\"next_before\":null}",
      "{\"messages\":[],\"next_before\":1.5}", Uint8Array.of(0xff)]) {
      await assert.rejects(() => client(async () => new Response(body, { headers: { "content-type": "application/json" } })).listSccpRecentMessagesV1());
    }
    await assert.rejects(() => client(async () => response(registry(), { "content-type": "text/html" })).getSccpRegistryV1());
  });
  for (const [method, maximum] of [["getSccpCapabilitiesV1", SCCP_CAPABILITIES_MAX_BYTES],
    ["getSccpRegistryV1", SCCP_REGISTRY_MAX_BYTES], ["listSccpRecentMessagesV1", SCCP_RECENT_MAX_BYTES]]) {
    test(`${name}.${method} cancels an oversized response`, async () => {
      let cancelled = false;
      const body = new ReadableStream({ cancel() { cancelled = true; } });
      await assert.rejects(() => client(async () => new Response(body, {
        headers: { "content-type": "application/json", "content-length": String(maximum + 1) },
      }))[method]());
      assert.equal(cancelled, true);
    });
  }
  test(`${name} preserves HTTP absence/error and abort semantics`, async () => {
    await assert.rejects(() => client(async () => new Response(null, { status: 404 })).getSccpRegistryV1());
    const abort = new AbortController(); abort.abort();
    await assert.rejects(() => client(async (_url, init) => {
      init.signal.throwIfAborted(); return response(registry());
    }).getSccpRegistryV1({ signal: abort.signal }));
  });
}
