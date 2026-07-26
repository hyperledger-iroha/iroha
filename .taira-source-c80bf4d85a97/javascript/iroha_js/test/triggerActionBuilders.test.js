import { test as baseTest } from "node:test";
import assert from "node:assert/strict";

import {
  ToriiClient,
  buildMintTriggerRepetitionsInstruction,
  buildPrecommitTriggerAction,
  buildTimeTriggerAction,
} from "../src/index.js";
import { AccountAddress } from "../src/address.js";
import { makeNativeTest } from "./helpers/native.js";

const BASE_URL = "https://example.test";
const CANONICAL_AUTHORITY = AccountAddress.fromAccount({ publicKey: Buffer.from(
    "CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
    "hex",
  ),
}).toI105();
const test = makeNativeTest(baseTest);

function okJsonResponse(body = {}) {
  return new Response(JSON.stringify(body), {
    status: 202,
    headers: { "content-type": "application/json" },
  });
}

test("buildTimeTriggerAction encodes Norito payload", () => {
  const action = buildTimeTriggerAction({
    authority: CANONICAL_AUTHORITY,
    instructions: [
      buildMintTriggerRepetitionsInstruction({
        triggerId: "apps::demo",
        repetitions: 1,
      }),
    ],
    startTimestampMs: Date.now() + 1_000,
    periodMs: 5_000,
    repeats: 2,
    metadata: { label: "demo" },
  });
  assert.equal(typeof action, "string");
  assert.ok(action.length > 0);
});

test("registerTrigger accepts base64 trigger action", async (t) => {
  let captured;
  const fetchImpl = async (_url, init) => {
    captured = JSON.parse(init.body);
    return okJsonResponse({ ok: true });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const action = buildPrecommitTriggerAction({
    authority: CANONICAL_AUTHORITY,
    instructions: [
      buildMintTriggerRepetitionsInstruction({
        triggerId: "apps::guardian",
        repetitions: 1,
      }),
    ],
  });
  await client.registerTrigger({
    id: "apps::guardian_refill",
    namespace: "apps",
    action,
  });
  assert.ok(captured);
  assert.equal(typeof captured.action, "string");
  assert.equal(captured.action, action);
  t.diagnostic(`encoded trigger payload length=${action.length}`);
});
