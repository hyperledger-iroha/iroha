export type PrivacyProtocolIdV1 =
  | "zk-ace-pq-authorization-v0"
  | "anonymous-pgc-k-out-of-n-v1"
  | "verange-transparent-range-v1"
  | "iroha-zk-ams-v1"
  | "vega-existing-credential-zk-v0"
  | "iroha-zk-x509-stark-p256-v0"
  | "iroha-jindo-polynomial-commitment-v0"
  | "iroha-bootle-lantern-anoncred-v1"
  | "orchard-halo2-actions-v1"
  | "monero-fcmp-plus-plus-v1"
  | "iroha-ivm-private-note-stark-v1"
  | "pq-masp-stark-v0";
export type PrivacyTaggedUnitV1<
  Key extends string,
  Value extends string,
  Content extends string = "value",
> = Readonly<Record<Key, Value> & Record<Content, null>>;
export type PrivacyProtocolTagV1 = PrivacyTaggedUnitV1<
  "protocol",
  PrivacyProtocolIdV1
>;
export type PrivacyFixed32BytesV1 = readonly number[];
/** Exact unsigned 64-bit protocol integer decoded without IEEE-754 rounding. */
export type PrivacyU64V1 = bigint;
export interface PrivacyConsensusLimitsV1 {
  readonly max_actions_per_transaction: number;
  readonly max_actions_per_block: number;
  readonly max_proof_bytes_per_action: number;
  readonly max_action_bytes: number;
  readonly max_privacy_bytes_per_transaction: number;
  readonly max_privacy_bytes_per_block: number;
  readonly max_statement_and_encrypted_output_bytes_per_transaction: number;
  readonly max_nullifiers_per_action: number;
  readonly max_commitments_per_action: number;
  readonly retained_root_count: number;
}
export interface PrivacyConsensusPolicyTighteningV1 {
  readonly scheduled_at_height: PrivacyU64V1;
  readonly effective_at_height: PrivacyU64V1;
  readonly next_limits: PrivacyConsensusLimitsV1;
}
export interface PrivacyConsensusPolicyV1 {
  readonly current_limits: PrivacyConsensusLimitsV1;
  readonly pending_tightening: PrivacyConsensusPolicyTighteningV1 | null;
}
export interface PrivacyProtocolLimitsV1 {
  readonly protocol: PrivacyProtocolIdV1;
  readonly limits: Readonly<Record<string, number>> | null;
}
export interface PrivacyCompiledProfileBindingsV1 {
  readonly protocol_id: PrivacyProtocolTagV1;
  readonly proof_system_id: Readonly<{ proof_system: string; value: null }>;
  readonly engine_id: Readonly<{ engine: string; value: null }>;
  readonly parameter_id: PrivacyFixed32BytesV1;
  readonly parameter_digest: PrivacyFixed32BytesV1;
  readonly verifier_digest: PrivacyFixed32BytesV1;
  readonly statement_schema_digest: PrivacyFixed32BytesV1;
  readonly engine_manifest_digest: PrivacyFixed32BytesV1;
}
export type PrivacyCompiledProfileResultV1 =
  | Readonly<{
      status: "available";
      value: PrivacyCompiledProfileBindingsV1 &
        Readonly<{ protocol_limits: PrivacyProtocolLimitsV1 }>;
    }>
  | Readonly<{
      status: "unavailable";
      value:
        | Readonly<{
            reason: "engine-unavailable" | "profile-initialization-failed";
            detail: null;
          }>
        | Readonly<{
            reason: "statement-schema-invalid";
            detail: Readonly<{
              schema_error:
                | "conflicting-stable-type-id"
                | "missing-type-reference";
              detail: null;
            }>;
          }>;
    }>;
export type PrivacyProtocolLifecycleV1 =
  | Readonly<{
      state: "proposed";
      record: Readonly<{
        proposed_at_height: PrivacyU64V1;
        activate_at_height: PrivacyU64V1;
      }>;
    }>
  | Readonly<{
      state: "active" | "suspended";
      record: Readonly<{
        proposed_at_height: PrivacyU64V1;
        activated_at_height: PrivacyU64V1;
        state_since_height: PrivacyU64V1;
      }>;
    }>
  | Readonly<{
      state: "retired";
      record: Readonly<{
        proposed_at_height: PrivacyU64V1;
        activated_at_height: PrivacyU64V1 | null;
        state_since_height: PrivacyU64V1;
      }>;
    }>;
export interface PrivacyProtocolActivationRecordV1
  extends PrivacyCompiledProfileBindingsV1 {
  readonly lifecycle: PrivacyProtocolLifecycleV1;
  readonly protocol_limits: PrivacyProtocolLimitsV1;
  readonly pending_protocol_limits_tightening: Readonly<{
    scheduled_at_height: PrivacyU64V1;
    effective_at_height: PrivacyU64V1;
    next_limits: PrivacyProtocolLimitsV1;
  }> | null;
  readonly assurance: Readonly<{ assurance: "experimental"; value: null }>;
}
export interface PrivacyCapabilityRowV1 {
  readonly protocol_id: PrivacyProtocolTagV1;
  readonly compiled_profile: PrivacyCompiledProfileResultV1;
  readonly activation: PrivacyProtocolActivationRecordV1 | null;
}
export interface PrivacyCapabilitySnapshotV1 {
  readonly version: 1;
  readonly committed_height: PrivacyU64V1;
  readonly consensus_policy: PrivacyConsensusPolicyV1;
  readonly protocols: readonly PrivacyCapabilityRowV1[];
}

/** The only accepted authoritative privacy-capability snapshot version. */
export const PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1: 1;
/** Closed canonical order required for `PrivacyCapabilitySnapshotV1.protocols`. */
export const PRIVACY_PROTOCOL_IDS_V1: readonly PrivacyProtocolIdV1[];

/** Error raised when a privacy-capability response cannot be trusted. */
export declare class PrivacyCapabilitySnapshotError extends TypeError {
  readonly path: string;
}

/** Parse the exact, fail-closed Torii `PrivacyCapabilitySnapshotV1` JSON shape. */
export function parsePrivacyCapabilitySnapshotV1(
  payload: unknown,
): PrivacyCapabilitySnapshotV1;

export interface PrivacyCapabilitiesNodeRequestOptions {
  signal?: AbortSignal;
}

export interface PrivacyCapabilitiesBrowserRequestOptions
  extends PrivacyCapabilitiesNodeRequestOptions {
  headers?: Record<string, string>;
  successStatuses?: ReadonlyArray<number>;
}

/**
 * Minimal public shape shared by configured package clients. Runtime admission
 * additionally requires the package-private transport capability.
 */
export interface PrivacyCapabilitiesNodeClientV1 {
  getNodeCapabilities(options?: {
    signal?: AbortSignal;
  }): Promise<unknown>;
}

export interface PrivacyCapabilitiesBrowserClientV1
  extends PrivacyCapabilitiesNodeClientV1 {
  readonly baseUrl: string;
}

/**
 * Fetch and validate the committed snapshot through the supplied configured
 * package client. The privacy policy parser remains outside base entry graphs.
 */
export function getPrivacyCapabilitiesV1(
  client: PrivacyCapabilitiesBrowserClientV1,
  options?: PrivacyCapabilitiesBrowserRequestOptions,
): Promise<PrivacyCapabilitySnapshotV1>;
export function getPrivacyCapabilitiesV1(
  client: PrivacyCapabilitiesNodeClientV1,
  options?: PrivacyCapabilitiesNodeRequestOptions,
): Promise<PrivacyCapabilitySnapshotV1>;
