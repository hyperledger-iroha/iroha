import type { Buffer } from "buffer";
import type { NetworkId, SignedTransactionResult } from "./index.js";
import type { BrowserFeePayment, BrowserTransactionMetadataValue } from "./transaction-codec.js";
import type { PrivacyExact12CapabilityManifestV1 } from "./privacy-capabilities.js";

export type PrivacyX509BytesV1 = ArrayBuffer | ArrayBufferView;

/** One original direct action. No mixed executable, alternate authority or caller-derived intent. */
export interface PrivacyX509DirectTransactionInputV1 {
  readonly manifest: PrivacyExact12CapabilityManifestV1;
  readonly networkId: NetworkId;
  readonly requestNorito: PrivacyX509BytesV1;
  readonly authority: string;
  readonly feePayment: BrowserFeePayment;
  /** Required exact safe integer; preparation never supplies the current clock implicitly. */
  readonly creationTimeMs: number;
  /** Required positive safe integer, frozen before proving; no implicit lifetime. */
  readonly ttlMs: number;
  readonly nonce?: number | null;
  readonly metadata?: Readonly<Record<string, BrowserTransactionMetadataValue>> | string | null;
}

/** Native-backed owner of the original request and transaction context; copies cannot replace it. */
export class PrivacyX509DirectTransactionV1 {
  private constructor();
  readonly materializedStatementNorito: Buffer;
  readonly orderedIntentDigest: Buffer;
  readonly actionStatementNorito: Buffer;
  readonly actionStatementDigest: Buffer;
  /** Only an exact 32-byte Ed25519 private key controlling the retained authority is accepted. */
  sign(rawProof: PrivacyX509BytesV1, privateKey: PrivacyX509BytesV1): SignedTransactionResult;
}

/** Requires authenticated Torii origin and actual native selected-profile availability. */
export function preparePrivacyX509DirectTransactionV1(
  input: PrivacyX509DirectTransactionInputV1,
): PrivacyX509DirectTransactionV1;

/** Prove the exact retained statement, then complete and sign its one original occurrence. */
export function buildPrivacyX509DirectTransactionV1(
  input: PrivacyX509DirectTransactionInputV1,
  options: {
    prove: (materializedStatementNorito: Buffer) => PrivacyX509BytesV1 | Promise<PrivacyX509BytesV1>;
    privateKey: PrivacyX509BytesV1;
  },
): Promise<SignedTransactionResult>;
