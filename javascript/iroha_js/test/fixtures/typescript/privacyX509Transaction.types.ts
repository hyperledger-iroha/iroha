import { NetworkId, type SignedTransactionResult, type PrivacyX509DirectTransactionInputV1 } from "../../../index.js";
import type { PrivacyExact12CapabilityManifestV1 } from "../../../privacy-capabilities.js";
import {
  PrivacyX509DirectTransactionV1,
  preparePrivacyX509DirectTransactionV1,
  buildPrivacyX509DirectTransactionV1,
} from "../../../privacy-x509.js";

declare const manifest: PrivacyExact12CapabilityManifestV1;
const input: PrivacyX509DirectTransactionInputV1 = {
  manifest, networkId: NetworkId.fromBytes(new Uint8Array(32)),
  authority: "canonical-account", requestNorito: new Uint8Array(1),
  feePayment: { payer: "authority" as const, chargeLimits: [] }, creationTimeMs: 17, ttlMs: 600_000,
};
const prepared = preparePrivacyX509DirectTransactionV1(input);
const signed: SignedTransactionResult = prepared.sign(new Uint8Array(1), new Uint8Array(32));
const pending: Promise<SignedTransactionResult> = buildPrivacyX509DirectTransactionV1(input, {
  prove: async (statement) => statement, privateKey: new Uint8Array(32),
});
void signed; void pending;
// @ts-expect-error There is no public prepared-owner constructor.
new PrivacyX509DirectTransactionV1();
// @ts-expect-error Caller-derived statement digests are not independent input.
preparePrivacyX509DirectTransactionV1({ ...input, actionStatementDigest: new Uint8Array(32) });
// @ts-expect-error Explicit creation time is required before proof generation.
preparePrivacyX509DirectTransactionV1({ manifest, networkId: input.networkId, authority: input.authority,
  requestNorito: input.requestNorito, feePayment: input.feePayment, ttlMs: input.ttlMs });
// @ts-expect-error Inspection/projection objects cannot replace authenticated manifests.
preparePrivacyX509DirectTransactionV1({ ...input, manifest: { readiness: "available" } });

// @ts-expect-error Explicit positive TTL is required before proof generation.
preparePrivacyX509DirectTransactionV1({ manifest, networkId: input.networkId, authority: input.authority,
  requestNorito: input.requestNorito, feePayment: input.feePayment, creationTimeMs: input.creationTimeMs });
