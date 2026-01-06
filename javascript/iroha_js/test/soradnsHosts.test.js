import assert from "node:assert/strict";
import { test as baseTest } from "node:test";

import {
  deriveSoradnsGatewayHosts,
  hostPatternsCoverDerivedHosts,
  canonicalGatewaySuffix,
  canonicalGatewayWildcard,
} from "../src/index.js";
import { makeNativeTest } from "./helpers/native.js";

const test = makeNativeTest(baseTest, { require: "soradnsDeriveGatewayHosts" });

test("derives deterministic gateway hosts", () => {
  const bindings = deriveSoradnsGatewayHosts("App.Dao.Sora");
  assert.equal(bindings.normalizedName, "app.dao.sora");
  assert.match(
    bindings.canonicalLabel,
    /^[a-z2-7]{52}$/,
    "canonical label uses lowercase base32",
  );
  assert.ok(
    bindings.canonicalHost.endsWith(`.${canonicalGatewaySuffix()}`),
    "canonical host uses the expected suffix",
  );
  assert.equal(bindings.prettyHost, "app.dao.sora.gw.sora.name");
  assert.equal(bindings.canonicalWildcard, canonicalGatewayWildcard());
  assert.equal(bindings.hostPatterns.length, 3);
  assert.ok(bindings.matchesHost(bindings.canonicalHost));
  assert.ok(bindings.matchesHost(bindings.prettyHost.toUpperCase()));
  assert.strictEqual(bindings.matchesHost("example.com"), false);
  assert.ok(hostPatternsCoverDerivedHosts(bindings.hostPatterns, bindings));
});

test("host pattern coverage detects missing entries", () => {
  const bindings = deriveSoradnsGatewayHosts("docs.sora");
  const partialPatterns = [bindings.canonicalHost, bindings.prettyHost];
  assert.strictEqual(
    hostPatternsCoverDerivedHosts(partialPatterns, bindings),
    false,
  );
});
