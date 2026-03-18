import { test } from "node:test";
import assert from "node:assert/strict";
import { ToriiClient } from "../src/toriiClient.js";
import { AccountAddress } from "../src/address.js";

const BASE_URL = "https://localhost:8080";
const SAMPLE_ACCOUNT_ID = AccountAddress.fromAccount({
  domain: "wonderland",
  publicKey: Buffer.from(
    "EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245",
    "hex",
  ),
}).toI105();

function createResponse({ status, jsonData = {}, arrayData, textBody, headers }) {
  const resolvedHeaders = headers ?? { "content-type": "application/json" };
  return {
    status,
    json: async () => jsonData,
    arrayBuffer: async () => {
      if (arrayData instanceof ArrayBuffer) {
        return arrayData;
      }
      if (ArrayBuffer.isView(arrayData)) {
        return arrayData.buffer.slice(
          arrayData.byteOffset,
          arrayData.byteOffset + arrayData.byteLength,
        );
      }
      return new TextEncoder().encode(textBody ?? JSON.stringify(jsonData ?? {})).buffer;
    },
    text: async () =>
      typeof textBody === "string" ? textBody : JSON.stringify(jsonData ?? {}),
    headers: {
      get(name) {
        if (!resolvedHeaders) {
          return null;
        }
        const normalized = name.toLowerCase();
        for (const [key, value] of Object.entries(resolvedHeaders)) {
          if (key.toLowerCase() === normalized) {
            return value;
          }
        }
        return null;
      },
    },
  };
}

function asUrl(input) {
  return typeof input === "string" ? new URL(input) : new URL(input.toString());
}

test("subscription plan and create endpoints send normalized payloads", async () => {
  const captured = [];
  const responses = new Map([
    [
      "/v1/subscriptions/plans",
      { ok: true, plan_id: "plan#subs", tx_hash_hex: "tx-plan" },
    ],
    [
      "/v1/subscriptions",
      {
        ok: true,
        subscription_id: "sub-1$subscriptions",
        billing_trigger_id: "sub-bill",
        usage_trigger_id: "sub-usage",
        first_charge_ms: 1728000000,
        tx_hash_hex: "tx-sub",
      },
    ],
  ]);
  const fetchImpl = async (url, init = {}) => {
    const parsed = asUrl(url);
    captured.push({ path: parsed.pathname, init });
    const jsonData = responses.get(parsed.pathname);
    return createResponse({ status: 200, jsonData });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });

  const planRequest = {
    authority: SAMPLE_ACCOUNT_ID,
    privateKey: "ed25519:deadbeef",
    planId: "plan#subs",
    plan: { provider: SAMPLE_ACCOUNT_ID, pricing: { kind: "fixed" } },
  };
  const planResponse = await client.createSubscriptionPlan(planRequest);
  assert.equal(planResponse.plan_id, "plan#subs");
  const planBody = JSON.parse(captured[0].init.body);
  assert.equal(planBody.authority, SAMPLE_ACCOUNT_ID);
  assert.equal(planBody.private_key, "ed25519:deadbeef");
  assert.equal(planBody.plan_id, "plan#subs");
  assert.deepEqual(planBody.plan, planRequest.plan);

  const subscriptionRequest = {
    authority: SAMPLE_ACCOUNT_ID,
    privateKey: "ed25519:cafebabe",
    subscriptionId: "sub-1$subscriptions",
    planId: "plan#subs",
    billingTriggerId: "sub-bill",
    usageTriggerId: "sub-usage",
    firstChargeMs: 1728000000,
    grantUsageToProvider: true,
  };
  const subscriptionResponse = await client.createSubscription(subscriptionRequest);
  assert.equal(subscriptionResponse.subscription_id, "sub-1$subscriptions");
  const subscriptionBody = JSON.parse(captured[1].init.body);
  assert.equal(subscriptionBody.authority, SAMPLE_ACCOUNT_ID);
  assert.equal(subscriptionBody.private_key, "ed25519:cafebabe");
  assert.equal(subscriptionBody.subscription_id, "sub-1$subscriptions");
  assert.equal(subscriptionBody.plan_id, "plan#subs");
  assert.equal(subscriptionBody.billing_trigger_id, "sub-bill");
  assert.equal(subscriptionBody.usage_trigger_id, "sub-usage");
  assert.equal(subscriptionBody.first_charge_ms, 1728000000);
  assert.equal(subscriptionBody.grant_usage_to_provider, true);
});

test("subscription list endpoints build query params and normalize responses", async () => {
  const captured = [];
  const fetchImpl = async (url) => {
    const parsed = asUrl(url);
    captured.push(parsed);
    if (parsed.pathname === "/v1/subscriptions/plans") {
      return createResponse({
        status: 200,
        jsonData: {
          items: [{ plan_id: "plan#subs", plan: { provider: SAMPLE_ACCOUNT_ID } }],
          total: 1,
        },
      });
    }
    return createResponse({
      status: 200,
      jsonData: {
        items: [
          {
            subscription_id: "sub-1$subscriptions",
            subscription: { status: { status: "active", value: null } },
            invoice: null,
            plan: { provider: SAMPLE_ACCOUNT_ID },
          },
        ],
        total: 1,
      },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });

  const plans = await client.listSubscriptionPlans({
    provider: SAMPLE_ACCOUNT_ID,
    limit: 5,
    offset: 2,
  });
  assert.equal(plans.items.length, 1);
  assert.equal(plans.items[0].plan_id, "plan#subs");
  const planUrl = captured[0];
  assert.equal(planUrl.pathname, "/v1/subscriptions/plans");
  assert.equal(planUrl.searchParams.get("provider"), SAMPLE_ACCOUNT_ID);
  assert.equal(planUrl.searchParams.get("limit"), "5");
  assert.equal(planUrl.searchParams.get("offset"), "2");

  const subs = await client.listSubscriptions({
    ownedBy: SAMPLE_ACCOUNT_ID,
    provider: SAMPLE_ACCOUNT_ID,
    status: "past_due",
    limit: 1,
    offset: 0,
  });
  assert.equal(subs.items.length, 1);
  assert.equal(subs.items[0].subscription_id, "sub-1$subscriptions");
  const subUrl = captured[1];
  assert.equal(subUrl.pathname, "/v1/subscriptions");
  assert.equal(subUrl.searchParams.get("owned_by"), SAMPLE_ACCOUNT_ID);
  assert.equal(subUrl.searchParams.get("provider"), SAMPLE_ACCOUNT_ID);
  assert.equal(subUrl.searchParams.get("status"), "past_due");
  assert.equal(subUrl.searchParams.get("limit"), "1");
  assert.equal(subUrl.searchParams.get("offset"), "0");
});

test("subscription action endpoints send normalized payloads", async () => {
  const captured = new Map();
  const fetchImpl = async (url, init = {}) => {
    const parsed = asUrl(url);
    captured.set(parsed.pathname, JSON.parse(init.body));
    return createResponse({
      status: 200,
      jsonData: {
        ok: true,
        subscription_id: "sub-1$subscriptions",
        tx_hash_hex: "tx-action",
      },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const subscriptionId = "sub-1$subscriptions";
  const encodedId = encodeURIComponent(subscriptionId);
  const actionRequest = {
    authority: SAMPLE_ACCOUNT_ID,
    privateKey: "ed25519:deadbeef",
  };

  await client.pauseSubscription(subscriptionId, actionRequest);
  await client.resumeSubscription(subscriptionId, {
    ...actionRequest,
    chargeAtMs: 1728000000,
  });
  await client.cancelSubscription(subscriptionId, actionRequest);
  await client.keepSubscription(subscriptionId, actionRequest);
  await client.chargeSubscriptionNow(subscriptionId, {
    ...actionRequest,
    chargeAtMs: 1730000000,
  });
  await client.recordSubscriptionUsage(subscriptionId, {
    authority: SAMPLE_ACCOUNT_ID,
    privateKey: "ed25519:deadbeef",
    unitKey: "compute_ms",
    delta: "12.5",
    usageTriggerId: "sub-usage",
  });

  const pauseBody = captured.get(`/v1/subscriptions/${encodedId}/pause`);
  assert.equal(pauseBody.authority, SAMPLE_ACCOUNT_ID);
  assert.equal(pauseBody.private_key, "ed25519:deadbeef");
  assert.ok(!("charge_at_ms" in pauseBody));

  const resumeBody = captured.get(`/v1/subscriptions/${encodedId}/resume`);
  assert.equal(resumeBody.charge_at_ms, 1728000000);

  const cancelBody = captured.get(`/v1/subscriptions/${encodedId}/cancel`);
  assert.ok(!("charge_at_ms" in cancelBody));

  const keepBody = captured.get(`/v1/subscriptions/${encodedId}/keep`);
  assert.ok(!("charge_at_ms" in keepBody));

  const chargeBody = captured.get(`/v1/subscriptions/${encodedId}/charge-now`);
  assert.equal(chargeBody.charge_at_ms, 1730000000);

  const usageBody = captured.get(`/v1/subscriptions/${encodedId}/usage`);
  assert.equal(usageBody.unit_key, "compute_ms");
  assert.equal(usageBody.delta, "12.5");
  assert.equal(usageBody.usage_trigger_id, "sub-usage");
});

test("getSubscription returns null on 404", async () => {
  const fetchImpl = async () => createResponse({ status: 404 });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getSubscription("sub-404$subscriptions");
  assert.equal(result, null);
});
