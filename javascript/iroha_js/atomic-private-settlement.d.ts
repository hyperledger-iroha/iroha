/** Authentication class for one atomic-private-settlement V1 route. */
export type AtomicPrivateSettlementAuthTagV1 =
  | "SPONSOR"
  | "ROLE_IDENTITY"
  | "PUBLIC";

/** Closed authentication catalog used by the prepared-request operations. */
export const AtomicPrivateSettlementAuthV1: Readonly<{
  SPONSOR: "SPONSOR";
  ROLE_IDENTITY: "ROLE_IDENTITY";
  PUBLIC: "PUBLIC";
}>;

/** Immutable route descriptor for a native-prepared settlement request. */
export interface AtomicPrivateSettlementOperationDescriptorV1 {
  readonly path: string;
  readonly auth: AtomicPrivateSettlementAuthTagV1;
  readonly topLevelFields: readonly string[];
  readonly maximumRequestBytes: number;
}

/** Closed mutation-route catalog for atomic private settlement V1. */
export const AtomicPrivateSettlementOperationV1: Readonly<{
  AVAILABILITY_SHARE: AtomicPrivateSettlementOperationDescriptorV1;
  PREPARE_VOTE: AtomicPrivateSettlementOperationDescriptorV1;
  COMMIT_VOTE: AtomicPrivateSettlementOperationDescriptorV1;
  PHASE_CERTIFICATE: AtomicPrivateSettlementOperationDescriptorV1;
  LEG_UPLOAD: AtomicPrivateSettlementOperationDescriptorV1;
  AUDIT_APPROVAL: AtomicPrivateSettlementOperationDescriptorV1;
  BUNDLE_SUBMIT: AtomicPrivateSettlementOperationDescriptorV1;
}>;

/** Exact marked 32-byte Iroha hash used in settlement paths and responses. */
export class AtomicPrivateSettlementIdentifierV1 {
  constructor(value: string);
  readonly pathComponent: string;
  readonly jsonLiteral: string;
  toString(): string;
}

/** Bounded operation-tagged JSON object produced by a native coordinator. */
export class AtomicPrivateSettlementPreparedRequestV1 {
  constructor(
    operation: AtomicPrivateSettlementOperationDescriptorV1,
    nativePreparedJson: string | ArrayBuffer | ArrayBufferView,
  );
  readonly operation: AtomicPrivateSettlementOperationDescriptorV1;
  bytes(): Uint8Array;
  close(): void;
  toString(): string;
}

/** Opaque bounded response suitable for native wallet or auditor decoding. */
export class AtomicPrivateSettlementJsonResponseV1 {
  constructor(route: string, body: Uint8Array);
  readonly route: string;
  bytes(): Uint8Array;
  close(): void;
  toString(): string;
}

/** Redacted exact-route transport or response-validation failure. */
export class AtomicPrivateSettlementToriiErrorV1 extends Error {}

/** Exact authentication input passed to a signing header provider. */
export interface AtomicPrivateSettlementHeaderRequestV1 {
  readonly method: "GET" | "POST";
  readonly path: string;
  readonly url: string;
  readonly body: Uint8Array;
}

/** Exact header quartet returned by a sponsor, validator, or auditor signer. */
export type AtomicPrivateSettlementHeaderProviderV1 = (
  request: AtomicPrivateSettlementHeaderRequestV1,
) => Readonly<Record<string, string>> | Promise<Readonly<Record<string, string>>>;

/** Fetch-compatible single-attempt transport injection. */
export type AtomicPrivateSettlementFetchV1 = (
  input: string | URL,
  init: RequestInit,
) => Promise<Response>;

export interface AtomicPrivateSettlementSponsorOptionsV1 {
  readonly sponsorHeaderProvider?: AtomicPrivateSettlementHeaderProviderV1;
  readonly signal?: AbortSignal;
}

export interface AtomicPrivateSettlementRoleOptionsV1 {
  readonly roleHeaderProvider: AtomicPrivateSettlementHeaderProviderV1;
  readonly signal?: AbortSignal;
}

export interface AtomicPrivateSettlementPublicOptionsV1 {
  readonly signal?: AbortSignal;
}

/**
 * Witness-free exact-route client for prepared-leg, audit, coordination, and
 * redacted public query workflows.
 */
export class AtomicPrivateSettlementToriiClientV1 {
  constructor(
    baseUrl: string | URL,
    options?: {
      readonly fetchImpl?: AtomicPrivateSettlementFetchV1;
      readonly sponsorHeaderProvider?: AtomicPrivateSettlementHeaderProviderV1;
    },
  );
  requestAvailabilityShare(
    request: AtomicPrivateSettlementPreparedRequestV1,
    options?: AtomicPrivateSettlementSponsorOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
  requestPrepareVote(
    request: AtomicPrivateSettlementPreparedRequestV1,
    options?: AtomicPrivateSettlementSponsorOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
  requestCommitVote(
    request: AtomicPrivateSettlementPreparedRequestV1,
    options?: AtomicPrivateSettlementSponsorOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
  persistPhaseCertificate(
    request: AtomicPrivateSettlementPreparedRequestV1,
    options?: AtomicPrivateSettlementSponsorOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
  uploadLeg(
    request: AtomicPrivateSettlementPreparedRequestV1,
    options?: AtomicPrivateSettlementSponsorOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
  submitAuditApproval(
    payloadDigest: string | AtomicPrivateSettlementIdentifierV1,
    request: AtomicPrivateSettlementPreparedRequestV1,
    options: AtomicPrivateSettlementRoleOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
  submitBundle(
    request: AtomicPrivateSettlementPreparedRequestV1,
    options?: AtomicPrivateSettlementSponsorOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
  getLegStatus(
    payloadDigest: string | AtomicPrivateSettlementIdentifierV1,
    options?: AtomicPrivateSettlementSponsorOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
  getPhaseCertificates(
    payloadDigest: string | AtomicPrivateSettlementIdentifierV1,
    options?: AtomicPrivateSettlementSponsorOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
  getCommitteeProof(
    payloadDigest: string | AtomicPrivateSettlementIdentifierV1,
    options: AtomicPrivateSettlementRoleOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
  getAuditorCapsule(
    payloadDigest: string | AtomicPrivateSettlementIdentifierV1,
    options: AtomicPrivateSettlementRoleOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
  getBundleStatus(
    bundleId: string | AtomicPrivateSettlementIdentifierV1,
    options?: AtomicPrivateSettlementPublicOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
  getBundleReceipt(
    bundleId: string | AtomicPrivateSettlementIdentifierV1,
    options?: AtomicPrivateSettlementPublicOptionsV1,
  ): Promise<AtomicPrivateSettlementJsonResponseV1>;
}
