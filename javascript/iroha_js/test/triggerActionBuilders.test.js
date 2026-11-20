import { test as baseTest } from "node:test";
import assert from "node:assert/strict";

import {
  buildMintTriggerRepetitionsInstruction,
  buildPrecommitTriggerAction,
  buildTimeTriggerAction,
} from "../src/index.js";
import { AccountAddress } from "../src/address.js";
import { makeNativeTest } from "./helpers/native.js";

const CANONICAL_AUTHORITY = AccountAddress.fromAccount({ publicKey: Buffer.from(
    "CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
    "hex",
  ),
}).toI105();
const test = makeNativeTest(baseTest);

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

test("buildPrecommitTriggerAction encodes canonical base64 Norito", (t) => {
  const action = buildPrecommitTriggerAction({
    authority: CANONICAL_AUTHORITY,
    instructions: [
      buildMintTriggerRepetitionsInstruction({
        triggerId: "apps::guardian",
        repetitions: 1,
      }),
    ],
  });
  assert.equal(typeof action, "string");
  assert.ok(action.length > 0);
  // The action is canonical padded base64 of the Norito `Action` frame.
  assert.equal(Buffer.from(action, "base64").toString("base64"), action);
  t.diagnostic(`encoded trigger payload length=${action.length}`);
});
