import assert from "node:assert/strict";
import { test as baseTest } from "node:test";

import { deriveGatewayHosts } from "../src/soradns.js";
import { makeNativeTest } from "./helpers/native.js";

const test = makeNativeTest(baseTest, { require: "soradnsDeriveGatewayHosts" });

test("deriveGatewayHosts returns deterministic gateway hosts", () => {
  const derived = deriveGatewayHosts("docs.sora");
  assert.equal(derived.normalizedName, "docs.sora");
  assert.ok(derived.canonicalHost.endsWith(".gw.sora.id"));
  assert.ok(derived.prettyHost.endsWith(".gw.sora.name"));
  assert.ok(Array.isArray(derived.hostPatterns));
  assert.ok(derived.hostPatterns.includes(derived.canonicalHost));
  assert.ok(derived.matchesHost(derived.canonicalHost));
});
