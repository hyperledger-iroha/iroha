import { LocalSigningContext } from "./toriiClient.js";

/** Stable, non-secret public metadata for the SORA Taira testnet. */
export const TAIRA_TESTNET_PROFILE = Object.freeze({
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

/**
 * Bind Taira's public address profile to the caller-supplied deployed genesis identity.
 * Taira resets can change NetworkId, so this helper never guesses it from a chain label.
 *
 * @param {import("./networkId.js").NetworkId} deployedNetworkId
 * @returns {LocalSigningContext}
 */
export function createTairaLocalSigningContext(deployedNetworkId) {
  return new LocalSigningContext(
    deployedNetworkId,
    TAIRA_TESTNET_PROFILE.i105Discriminant,
  );
}
