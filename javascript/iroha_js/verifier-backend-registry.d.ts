/** Generic OpenVerify engines encoded by Norito. */
export type OpenVerifyBackendTagV1 = "halo2-ipa-pasta" | "stark";

/** Exact verifier-registry labels admitted by native Rust dispatch. */
export type VerifierBackendRegistryLabelV1 =
  | "halo2/ipa"
  | "halo2/pasta/kaigi-roster-v1"
  | "halo2/pasta/kaigi-usage-v1"
  | "halo2/pasta/ivm-execution-v1"
  | "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3"
  | "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3"
  | "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3"
  | "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4"
  | "stark/fri"
  | "stark/fri/sha256-goldilocks"
  | "stark/fri/poseidon2-goldilocks"
  | "stark/fri/sha256_goldilocks.v1";

export interface VerifierBackendRegistryBindingV1 {
  readonly label: VerifierBackendRegistryLabelV1;
  readonly engine: OpenVerifyBackendTagV1;
}

/** Generic OpenVerify engines in canonical Norito order. */
export const OPEN_VERIFY_BACKEND_TAGS_V1: readonly [
  "halo2-ipa-pasta",
  "stark",
];

/** Exact verifier-registry labels in native dispatch order. */
export const VERIFIER_BACKEND_REGISTRY_LABELS_V1:
  readonly VerifierBackendRegistryLabelV1[];

/** Ordered verifier labels and their generic OpenVerify engine bindings. */
export const VERIFIER_BACKEND_REGISTRY_BINDINGS_V1:
  readonly VerifierBackendRegistryBindingV1[];

/** Resolve an exact registry label, returning `null` for every unsupported value. */
export function verifierBackendRegistryTagV1(
  label: unknown,
): OpenVerifyBackendTagV1 | null;

/** Return whether a value is one exact verifier-registry v1 label. */
export function isVerifierBackendRegistryLabelV1(
  label: unknown,
): label is VerifierBackendRegistryLabelV1;

/** Require one exact registry label and return it unchanged. */
export function requireVerifierBackendRegistryLabelV1(
  label: unknown,
  context?: string,
): VerifierBackendRegistryLabelV1;
