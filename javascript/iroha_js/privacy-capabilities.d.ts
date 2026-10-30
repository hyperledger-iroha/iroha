import type { CanonicalRequestAuth, NetworkId } from "./index.js";

export type PrivacyProtocolIdV1 =
  | "zk-ace-pq-authorization-v1"
  | "anonymous-pgc-k-out-of-n-v1"
  | "verange-transparent-range-v1"
  | "iroha-zk-ams-v1"
  | "vega-existing-credential-zk-v1"
  | "iroha-zk-x509-stark-p256-v1"
  | "iroha-jindo-polynomial-commitment-v1"
  | "iroha-bootle-lantern-anoncred-v1"
  | "orchard-halo2-actions-v1"
  | "monero-fcmp-plus-plus-v1"
  | "iroha-ivm-private-note-stark-v1"
  | "pq-masp-stark-v1";
export type PrivacyProofSystemIdV1 =
  | "stark-fri-sha3-384-goldilocks-v1"
  | "anonymous-pgc-p256"
  | "iroha-verange-p256"
  | "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512"
  | "vega-neutron-nova-spartan-hyrax-t256"
  | "jindo-polynomial-commitment"
  | "lantern-lnp22-module-linear-norm"
  | "halo2-ipa-pasta"
  | "fcmp-plus-plus-curve-tree-bulletproofs";
export type PrivacyEngineIdV1 =
  | "native-goldilocks-sha3-384-stark-fri-v1"
  | "native-anonymous-pgc-p256"
  | "native-verange-p256"
  | "native-zk-ams-masked-relaxed-spartan-t256-ristretto255"
  | "native-vega"
  | "native-jindo"
  | "native-lantern-lnp22"
  | "native-halo2-orchard"
  | "native-fcmp-plus-plus";
export type PrivacyTaggedUnitV1<
  Key extends string,
  Value extends string,
  Content extends string = "value",
> = Readonly<Record<Key, Value> & Record<Content, null>>;
export type PrivacyProtocolTagV1 = PrivacyTaggedUnitV1<
  "protocol",
  PrivacyProtocolIdV1
>;
export type PrivacyProofSystemTagV1 = PrivacyTaggedUnitV1<
  "proof_system",
  PrivacyProofSystemIdV1
>;
export type PrivacyEngineTagV1 = PrivacyTaggedUnitV1<
  "engine",
  PrivacyEngineIdV1
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
export interface PrivacyConsensusPolicyUpdateV1 {
  readonly scheduled_at_height: PrivacyU64V1;
  readonly effective_at_height: PrivacyU64V1;
  readonly next_limits: PrivacyConsensusLimitsV1;
}
export interface PrivacyConsensusPolicyV1 {
  readonly current_limits: PrivacyConsensusLimitsV1;
  readonly pending_update: PrivacyConsensusPolicyUpdateV1 | null;
}
export interface PrivacyProtocolLimitsV1 {
  readonly protocol: PrivacyProtocolIdV1;
  readonly limits: Readonly<Record<string, number>> | null;
}
export interface PrivacyCompiledProfileBindingsV1 {
  readonly protocol_id: PrivacyProtocolTagV1;
  readonly proof_system_id: PrivacyProofSystemTagV1;
  readonly engine_id: PrivacyEngineTagV1;
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
            reason: "engine-unavailable" | "profile-initialization-failed" | "joint-security-reduction-incomplete";
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
export interface PrivacyProtocolLimitsUpdateV1 {
  readonly scheduled_at_height: PrivacyU64V1;
  readonly effective_at_height: PrivacyU64V1;
  readonly next_limits: PrivacyProtocolLimitsV1;
}
/** Optional committed resource policy; absence uses the selected native defaults. */
export interface PrivacyProtocolPolicyV1 {
  readonly protocol_id: PrivacyProtocolTagV1;
  readonly current_limits: PrivacyProtocolLimitsV1;
  readonly pending_update: PrivacyProtocolLimitsUpdateV1 | null;
}
/** Closed canonical order required for the Exact12 manifest rows. */
export const PRIVACY_PROTOCOL_IDS_V1: readonly PrivacyProtocolIdV1[];

export interface PrivacyExact12CapabilityManifestRequestOptions {
  signal?: AbortSignal;
  canonicalAuth: CanonicalRequestAuth;
}

/**
 * Minimal public shape for the Node/N-API Torii client. Runtime admission also
 * requires the package-private canonical archive transport capability.
 */
export interface PrivacyExact12CapabilityManifestNodeClientV1 {
  getNodeCapabilities(
    options: PrivacyExact12CapabilityManifestRequestOptions,
  ): Promise<unknown>;
}

export type PrivacyOperationSchemaV1 =
  | "zk_ace_authorization_action_v1"
  | "anonymous_pgc_payment_action_v1"
  | "verange_range_proof_v1"
  | "zk_ams_admission_and_provisioning_v1"
  | "vega_credential_presentation_v1"
  | "zk_x509_identity_presentation_v1"
  | "jindo_polynomial_evaluation_v1"
  | "bootle_lantern_credential_presentation_v1"
  | "orchard_note_action_v1"
  | "fcmp_membership_payment_v1"
  | "ivm_private_note_action_v1"
  | "pq_masp_note_action_v1";
export type PrivacyExecutionModeV1 =
  | "authorization_action"
  | "payment_action"
  | "component"
  | "admission_action"
  | "presentation_action"
  | "note_action";
export type PrivacyCapabilityReadinessV1 =
  | Readonly<{ readiness: "available"; detail: null }>
  | Readonly<{
      readiness: "unavailable";
      detail:
        | Readonly<{
            reason: "compiled-profile";
            detail: Extract<
              PrivacyCompiledProfileResultV1,
              { status: "unavailable" }
            >["value"];
          }>;
    }>;
export interface PrivacyExact12CapabilityRowV1 {
  readonly protocol_id: PrivacyProtocolTagV1;
  readonly operation_schema: Readonly<{
    operation_schema: PrivacyOperationSchemaV1;
    value: null;
  }>;
  readonly execution_mode: Readonly<{
    execution_mode: PrivacyExecutionModeV1;
    value: null;
  }>;
  readonly privacy_feature_mask: number;
  readonly compiled_profile: PrivacyCompiledProfileResultV1;
  readonly readiness: PrivacyCapabilityReadinessV1;
  readonly policy: PrivacyProtocolPolicyV1 | null;
}

/** Native-validated immutable view of Torii's canonical Exact12 bytes. */
export declare class PrivacyExact12CapabilityManifestV1 {
  private constructor();
  readonly version: 1;
  readonly committed_height: PrivacyU64V1;
  readonly consensus_policy: PrivacyConsensusPolicyV1;
  readonly network_id: NetworkId;
  readonly protocols: readonly PrivacyExact12CapabilityRowV1[];
  readonly manifest_digest: PrivacyFixed32BytesV1;
  canonicalBytes(): Uint8Array;
}

export interface PrivacyExact12CapabilityAdmissionV1 {
  readonly manifest_digest: PrivacyFixed32BytesV1;
  readonly network_id: PrivacyFixed32BytesV1;
  readonly torii_origin: string;
  readonly committed_height: PrivacyU64V1;
  readonly protocol_id: PrivacyProtocolIdV1;
  readonly operation_schema: PrivacyOperationSchemaV1;
  readonly execution_mode: PrivacyExecutionModeV1;
  readonly privacy_feature_mask: number;
  readonly readiness: "available";
  readonly compiled_profile: PrivacyCompiledProfileBindingsV1 &
    Readonly<{ protocol_limits: PrivacyProtocolLimitsV1 }>;
}

export const PRIVACY_EXACT12_CAPABILITY_MANIFEST_VERSION_V1: 1;
export const PRIVACY_EXACT12_CAPABILITY_MANIFEST_MAX_BYTES_V1: 262144;

/** Local native catalog bytes; never sufficient to authorize network use. */
export function compiledProfileCatalogV1(): Uint8Array;
/** Inspection-only decoding; archived bytes never grant admission. */
export function decodePrivacyExact12CapabilityManifestV1(
  canonicalArchive: ArrayBufferView | ArrayBuffer,
): PrivacyExact12CapabilityManifestV1;
/** Authenticated HTTPS transport with immutable LocalSigningContext; custom fetch implementations are trusted transport dependencies. */
export function getPrivacyExact12CapabilityManifestV1(
  client: PrivacyExact12CapabilityManifestNodeClientV1,
  options: PrivacyExact12CapabilityManifestRequestOptions,
): Promise<PrivacyExact12CapabilityManifestV1>;
export function requirePrivacyExact12CapabilityTupleV1(
  manifest: PrivacyExact12CapabilityManifestV1,
  protocolId: PrivacyProtocolIdV1,
): PrivacyExact12CapabilityAdmissionV1;
export function requirePrivacyExact12CapabilityAdmissionV1(
  manifest: PrivacyExact12CapabilityManifestV1,
  protocolId: PrivacyProtocolIdV1,
): PrivacyExact12CapabilityAdmissionV1;
/** @internal Authenticated original manifest bytes for a native construction owner. */
export function privacyExact12AuthenticatedConstructionArchiveV1(
  manifest: PrivacyExact12CapabilityManifestV1,
  expectedNetworkId: NetworkId,
): Uint8Array;

export declare class PrivacyExact12CapabilityManifestError extends TypeError {
  readonly path: string;
}
