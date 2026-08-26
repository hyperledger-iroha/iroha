import { test } from "node:test";
import assert from "node:assert/strict";
import {
  LocalSigningContext,
  ToriiClient as BaseToriiClient,
} from "../src/toriiClient.js";
import { AccountAddress } from "../src/address.js";
import { KotodamaQuantity, NumericV1 } from "../src/numericV1.js";
import { blake2b256 } from "../src/blake2b.js";
import { NetworkId } from "../src/networkId.js";

const BASE_URL = "https://localhost:8080";
const SAMPLE_ACCOUNT_ID = AccountAddress.fromAccount({ publicKey: Buffer.from(
    "EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245",
    "hex",
  ),
}).toI105();
const OTHER_ACCOUNT_ID = "mallory@universal";
const LOCAL_SIGNING_CONTEXT = new LocalSigningContext(
  NetworkId.fromBytes(Buffer.alloc(32, 0xa5)),
);
const CANONICAL_AUTH = Object.freeze({
  accountId: SAMPLE_ACCOUNT_ID,
  privateKey: Buffer.alloc(32, 0x31),
});
const MUTATION_OPTIONS = Object.freeze({ canonicalAuth: CANONICAL_AUTH });

class ToriiClient extends BaseToriiClient {
  constructor(baseUrl, options = {}) {
    super(baseUrl, { localSigningContext: LOCAL_SIGNING_CONTEXT, ...options });
  }
}

function createResponse({ status, jsonData = {}, arrayData, textBody, headers }) {
  const resolvedHeaders = headers ?? { "content-type": "application/json" };
  const body = arrayData ?? textBody ?? JSON.stringify(jsonData ?? {});
  return new Response(body, { status, headers: resolvedHeaders });
}

function asUrl(input) {
  return typeof input === "string" ? new URL(input) : new URL(input.toString());
}

function transactionDraft(extra = {}) {
  const payload = Buffer.from([1, 2, 3]);
  const signingMessage = Buffer.from(blake2b256(payload));
  signingMessage[signingMessage.length - 1] |= 1;
  return {
    submitted: false,
    transaction_payload_b64: payload.toString("base64"),
    signing_message_b64: signingMessage.toString("base64"),
    ...extra,
  };
}

test("subscription plan and create endpoints send normalized payloads", async () => {
  const captured = [];
  const responses = new Map([
    [
      "/v1/subscriptions/plans",
      transactionDraft({ plan_id: "plan#subs" }),
    ],
    [
      "/v1/subscriptions",
      {
        version: 1,
        authority: SAMPLE_ACCOUNT_ID,
        action: "create",
        subscription_id: "sub-1$subscriptions",
        plan_id: "plan#subs",
        billing_trigger_id: "sub-bill",
        usage_trigger_id: "sub-usage",
        first_charge_ms: 1728000000,
        provider_usage_grant_included: true,
        resulting_subscription: {},
        tx_instructions: [
          { wire_id: "register_subscription", payload_hex: "00" },
          { wire_id: "register_billing_trigger", payload_hex: "01" },
        ],
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
    planId: "plan#subs",
    plan: { provider: SAMPLE_ACCOUNT_ID, pricing: { kind: "fixed" } },
  };
  const planResponse = await client.createSubscriptionPlan(planRequest, MUTATION_OPTIONS);
  assert.equal(planResponse.plan_id, "plan#subs");
  const planBody = JSON.parse(captured[0].init.body);
  assert.equal(planBody.authority, SAMPLE_ACCOUNT_ID);
  assert.equal(captured[0].init.redirect, "error");
  assert.equal("private_key" in planBody, false);
  assert.equal(planBody.plan_id, "plan#subs");
  assert.deepEqual(planBody.plan, planRequest.plan);

  const subscriptionRequest = {
    authority: SAMPLE_ACCOUNT_ID,
    subscriptionId: "sub-1$subscriptions",
    planId: "plan#subs",
    billingTriggerId: "sub-bill",
    usageTriggerId: "sub-usage",
    firstChargeMs: 1728000000,
    grantUsageToProvider: true,
  };
  const subscriptionResponse = await client.createSubscription(subscriptionRequest, MUTATION_OPTIONS);
  assert.equal(subscriptionResponse.subscription_id, "sub-1$subscriptions");
  const subscriptionBody = JSON.parse(captured[1].init.body);
  assert.equal(subscriptionBody.authority, SAMPLE_ACCOUNT_ID);
  assert.equal("private_key" in subscriptionBody, false);
  assert.equal(subscriptionBody.subscription_id, "sub-1$subscriptions");
  assert.equal(subscriptionBody.plan_id, "plan#subs");
  assert.equal(subscriptionBody.billing_trigger_id, "sub-bill");
  assert.equal(subscriptionBody.usage_trigger_id, "sub-usage");
  assert.equal(subscriptionBody.first_charge_ms, 1728000000);
  assert.equal(subscriptionBody.grant_usage_to_provider, true);
  assert.equal(captured[1].init.redirect, "error");
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
    if (parsed.pathname.endsWith("/usage")) {
      return createResponse({
        status: 200,
        jsonData: transactionDraft({
          subscription_id: "sub-1$subscriptions",
        }),
      });
    }
    const suffix = parsed.pathname.split("/").at(-1);
    const action = suffix === "charge-now" ? "charge_now" : suffix;
    return createResponse({
      status: 200,
      jsonData: {
        version: 1,
        authority: SAMPLE_ACCOUNT_ID,
        action,
        subscription_id: "sub-1$subscriptions",
        details: {
          billing_trigger_id: "sub-bill",
          billing_trigger_operation: "none",
          effective_charge_ms: null,
          cancel_mode: null,
          resulting_subscription: {},
        },
        tx_instructions: [{ wire_id: "set_subscription", payload_hex: "00" }],
      },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const subscriptionId = "sub-1$subscriptions";
  const encodedId = encodeURIComponent(subscriptionId);
  const actionRequest = { authority: SAMPLE_ACCOUNT_ID };

  await client.pauseSubscription(subscriptionId, actionRequest, MUTATION_OPTIONS);
  await client.resumeSubscription(subscriptionId, {
    ...actionRequest,
    chargeAtMs: 1728000000,
  }, MUTATION_OPTIONS);
  await client.cancelSubscription(subscriptionId, {
    ...actionRequest,
    cancelMode: "immediate",
  }, MUTATION_OPTIONS);
  await client.keepSubscription(subscriptionId, actionRequest, MUTATION_OPTIONS);
  await client.chargeSubscriptionNow(subscriptionId, {
    ...actionRequest,
    chargeAtMs: 1730000000,
  }, MUTATION_OPTIONS);
  await client.recordSubscriptionUsage(subscriptionId, {
    authority: SAMPLE_ACCOUNT_ID,
    unitKey: "compute_ms",
    delta: "12.5",
    usageTriggerId: "sub-usage",
  }, MUTATION_OPTIONS);

  const pauseBody = captured.get(`/v1/subscriptions/${encodedId}/pause`);
  assert.equal(pauseBody.authority, SAMPLE_ACCOUNT_ID);
  assert.equal("private_key" in pauseBody, false);
  assert.ok(!("charge_at_ms" in pauseBody));

  const resumeBody = captured.get(`/v1/subscriptions/${encodedId}/resume`);
  assert.equal(resumeBody.charge_at_ms, 1728000000);

  const cancelBody = captured.get(`/v1/subscriptions/${encodedId}/cancel`);
  assert.ok(!("charge_at_ms" in cancelBody));
  assert.deepEqual(cancelBody.cancel_mode, { mode: "immediate", value: null });

  const keepBody = captured.get(`/v1/subscriptions/${encodedId}/keep`);
  assert.ok(!("charge_at_ms" in keepBody));

  const chargeBody = captured.get(`/v1/subscriptions/${encodedId}/charge-now`);
  assert.equal(chargeBody.charge_at_ms, 1730000000);

  const usageBody = captured.get(`/v1/subscriptions/${encodedId}/usage`);
  assert.equal(usageBody.unit_key, "compute_ms");
  assert.equal("private_key" in usageBody, false);
  assert.equal(usageBody.delta, "12.5");
  assert.equal(usageBody.usage_trigger_id, "sub-usage");
});

test("subscription usage delta uses the canonical Quantity boundary", async () => {
  const submittedDeltas = [];
  const fetchImpl = async (_url, init = {}) => {
    submittedDeltas.push(JSON.parse(init.body).delta);
    return createResponse({
      status: 200,
      jsonData: transactionDraft({
        subscription_id: "sub-1$subscriptions",
      }),
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const request = {
    authority: SAMPLE_ACCOUNT_ID,
    unitKey: "compute_ms",
  };
  const scale28 = `0.${"0".repeat(27)}1`;
  const canonical = [
    ["0", "0"],
    ["12.5", "12.5"],
    [NumericV1.INT_MAX.toString(), NumericV1.INT_MAX.toString()],
    [scale28, scale28],
    [42n, "42"],
    [new KotodamaQuantity("7.25"), "7.25"],
  ];

  for (const [delta, expected] of canonical) {
    await client.recordSubscriptionUsage("sub-1$subscriptions", {
      ...request,
      delta,
    }, MUTATION_OPTIONS);
    assert.equal(submittedDeltas.at(-1), expected);
  }
  assert.equal(submittedDeltas.length, canonical.length);

  const invalid = [
    0,
    1,
    1.25,
    "+1",
    "01",
    "00",
    "00.1",
    "1.0",
    "1.20",
    "0.0",
    "-0",
    "-1",
    (NumericV1.INT_MAX + 1n).toString(),
    `0.${"0".repeat(28)}1`,
  ];
  for (const delta of invalid) {
    await assert.rejects(
      client.recordSubscriptionUsage("sub-1$subscriptions", {
        ...request,
        delta,
      }, MUTATION_OPTIONS),
      undefined,
      `subscription usage accepted noncanonical Quantity input ${String(delta)}`,
    );
  }
  assert.equal(
    submittedDeltas.length,
    canonical.length,
    "invalid Quantity inputs must fail before the request is sent",
  );
});

test("subscription mutations require bound canonical auth and never retry", async () => {
  let calls = 0;
  const client = new ToriiClient(BASE_URL, {
    maxRetries: 3,
    fetchImpl: async () => {
      calls += 1;
      return createResponse({ status: 503 });
    },
  });
  const request = {
    authority: SAMPLE_ACCOUNT_ID,
    planId: "plan#subs",
    plan: { provider: SAMPLE_ACCOUNT_ID, pricing: { kind: "fixed" } },
  };

  await assert.rejects(
    () => client.createSubscriptionPlan(request),
    /canonicalAuth is required/,
  );
  await assert.rejects(
    () => client.createSubscriptionPlan(request, {
      canonicalAuth: { ...CANONICAL_AUTH, accountId: OTHER_ACCOUNT_ID },
    }),
    /canonicalAuth\.accountId must equal payload\.authority/,
  );
  assert.equal(calls, 0, "missing or mismatched authority must fail before dispatch");

  await assert.rejects(() => client.createSubscriptionPlan(request, MUTATION_OPTIONS));
  assert.equal(calls, 1, "canonical mutation requests must never be replayed");
});

test("subscription plan and usage drafts reject inline private-key fields", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => {
      throw new Error("fetch must not run for a secret-bearing request");
    },
  });
  await assert.rejects(
    () =>
      client.createSubscriptionPlan({
        authority: SAMPLE_ACCOUNT_ID,
        privateKey: "secret",
        planId: "plan#subs",
        plan: { provider: SAMPLE_ACCOUNT_ID, pricing: { kind: "fixed" } },
      }),
    /does not accept private-key fields/,
  );
  await assert.rejects(
    () =>
      client.recordSubscriptionUsage("sub-1$subscriptions", {
        authority: SAMPLE_ACCOUNT_ID,
        private_key_hex: "11".repeat(32),
        unitKey: "compute_ms",
        delta: "1",
      }),
    /does not accept private-key fields/,
  );
});

test("getSubscription returns null on 404", async () => {
  const fetchImpl = async () => createResponse({ status: 404 });
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const result = await client.getSubscription("sub-404$subscriptions");
  assert.equal(result, null);
});
