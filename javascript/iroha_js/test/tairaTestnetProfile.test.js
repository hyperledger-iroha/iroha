import assert from "node:assert/strict";
import test from "node:test";

import { NetworkId } from "../src/networkId.js";
import {
  TAIRA_TESTNET_PROFILE,
  createTairaLocalSigningContext,
} from "../src/tairaTestnetProfile.js";

const NETWORK_ID = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);

test("Taira profile exposes exact public deployment metadata", () => {
  assert.deepEqual(TAIRA_TESTNET_PROFILE, {
    toriiBaseUrl: "https://taira.sora.org",
    chainId: "fc56984b-2be7-431d-840e-21514d1883f0",
    i105Discriminant: 369,
    offlineCashAssetDefinitionId: "7ZepsJTHCVLKsrFFNZGSRGZgvBhv",
    offlineCashAssetAlias: "ds#boi.is",
    offlineCashAssetScale: 2,
    xorAssetDefinitionId: "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
    xorAssetAlias: "xor#universal",
    xorAssetScale: 9,
  });
  assert.equal(Object.isFrozen(TAIRA_TESTNET_PROFILE), true);
});

test("Taira signing context requires and preserves the deployed NetworkId", () => {
  const context = createTairaLocalSigningContext(NETWORK_ID);
  assert.equal(context.networkId, NETWORK_ID);
  assert.equal(context.chainDiscriminant, 369);
  assert.throws(
    () => createTairaLocalSigningContext("fc56984b-2be7-431d-840e-21514d1883f0"),
    /NetworkId/u,
  );
});
