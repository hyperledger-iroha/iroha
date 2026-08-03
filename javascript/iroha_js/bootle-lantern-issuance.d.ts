export const BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1:
  "/v1/privacy/bootle-lantern/issuance/authorize";
export const BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1:
  "/v1/privacy/bootle-lantern/issuance/issue";
export const BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1: "application/x-norito";
export const BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1: 320;
export const BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1: 71896;
export const BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1: 3176;
export const BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1: 4096;
export const BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1: 512;

/** Opaque, bounded bearer credential whose retained bytes can be overwritten. */
export class BootleLanternIssuanceCredentialV1 {
  constructor(secret: Uint8Array);
  static fromOpaqueBytes(secret: Uint8Array): BootleLanternIssuanceCredentialV1;
  static fromCanonicalBase64Url(encoded: string): BootleLanternIssuanceCredentialV1;
  destroy(): void;
  toString(): string;
}

/** Fail-closed transport or response validation failure. */
export class BootleLanternIssuanceClientErrorV1 extends Error {
  readonly status: number | null;
  readonly code: string | null;
  readonly retryAfterSeconds: number | null;
}

export type BootleLanternIssuanceFetchV1 = (
  input: string | URL,
  init: RequestInit,
) => Promise<Response>;

/** Exact, single-attempt client for first-release native blind issuance. */
export class BootleLanternIssuanceClientV1 {
  constructor(options: {
    baseUrl: string | URL;
    fetch?: BootleLanternIssuanceFetchV1;
  });
  authorize(credential: BootleLanternIssuanceCredentialV1): Promise<Uint8Array>;
  issue(
    credential: BootleLanternIssuanceCredentialV1,
    canonicalRequest: Uint8Array,
  ): Promise<Uint8Array>;
}
