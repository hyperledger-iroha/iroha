import assert from "node:assert/strict";
import { test as baseTest } from "node:test";

import {
  deriveGatewayHosts,
  tairaMonPrettyGatewaySuffix,
} from "../src/soradns.js";
import { makeNativeTest } from "./helpers/native.js";

const test = makeNativeTest(baseTest, { require: "soradnsDeriveGatewayHosts" });
const customSuffixTest = makeNativeTest(baseTest, {
  require: [
    "soradnsDeriveGatewayHosts",
    "soradnsDeriveGatewayHostsWithPrettySuffix",
  ],
});

test("deriveGatewayHosts returns deterministic gateway hosts", () => {
  const derived = deriveGatewayHosts("docs.sora");
  assert.equal(derived.normalizedName, "docs.sora");
  assert.ok(derived.canonicalHost.endsWith(".gw.sora.id"));
  assert.ok(derived.prettyHost.endsWith(".gw.sora.name"));
  assert.ok(Array.isArray(derived.hostPatterns));
  assert.ok(derived.hostPatterns.includes(derived.canonicalHost));
  assert.ok(derived.matchesHost(derived.canonicalHost));
});

customSuffixTest("deriveGatewayHosts supports the Taira Mon pretty suffix", () => {
  const derived = deriveGatewayHosts("solswap-indexer.sora", {
    prettySuffix: tairaMonPrettyGatewaySuffix(),
  });
  assert.equal(
    derived.prettyHost,
    "solswap-indexer.sora.mon.taira.sora.org",
  );
  assert.ok(derived.canonicalHost.endsWith(".gw.sora.id"));
  assert.ok(derived.hostPatterns.includes(derived.prettyHost));
  assert.ok(derived.matchesHost("SOLSWAP-INDEXER.SORA.MON.TAIRA.SORA.ORG"));
});
