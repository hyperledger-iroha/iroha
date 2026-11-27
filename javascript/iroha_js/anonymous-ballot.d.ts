/** Dedicated Torii transport for anonymous Parliament ballot envelopes and whole public records. */
export class AnonymousBallotClientV1 {
  constructor(baseUrl: string | URL, options?: { allowLocalHttp?: boolean });
  submitAnonymousBallot(envelope: Uint8Array, signal?: AbortSignal): Promise<void>;
  getAnonymousBallotBoard(scope: Uint8Array | string, signal?: AbortSignal): Promise<Uint8Array>;
  getAnonymousParliamentAttempt(attempt: Uint8Array | string, signal?: AbortSignal): Promise<Uint8Array>;
}
export class AnonymousBallotRefusal extends Error {
  readonly status: number;
  readonly code: string | null;
  readonly retryAfterSeconds: number | null;
}
