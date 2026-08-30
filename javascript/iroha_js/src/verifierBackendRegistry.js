/**
 * Closed verifier-registry v1 capability model shared with native Rust.
 *
 * Registry labels select concrete verifier configurations. They are not
 * aliases for the two generic OpenVerify engine tags encoded by Norito.
 */

/** Generic OpenVerify engines in canonical Norito order. */
export const OPEN_VERIFY_BACKEND_TAGS_V1 = /* @__PURE__ */ Object.freeze([
  "halo2-ipa-pasta",
  "stark",
]);

/** Exact verifier-registry v1 labels in native dispatch order. */
export const VERIFIER_BACKEND_REGISTRY_LABELS_V1 = /* @__PURE__ */ Object.freeze([
  "halo2/ipa",
  "halo2/pasta/kaigi-roster-v1",
  "halo2/pasta/kaigi-usage-v1",
  "halo2/pasta/ivm-execution-v1",
  "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
  "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
  "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
  "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
  "stark/fri",
  "stark/fri/sha256-goldilocks",
  "stark/fri/poseidon2-goldilocks",
  "stark/fri/sha256_goldilocks.v1",
]);

const ENGINE_BY_REGISTRY_LABEL_V1 = /* @__PURE__ */ new Map([
  ["halo2/ipa", "halo2-ipa-pasta"],
  ["halo2/pasta/kaigi-roster-v1", "halo2-ipa-pasta"],
  ["halo2/pasta/kaigi-usage-v1", "halo2-ipa-pasta"],
  ["halo2/pasta/ivm-execution-v1", "halo2-ipa-pasta"],
  ["halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3", "halo2-ipa-pasta"],
  ["halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3", "halo2-ipa-pasta"],
  ["halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3", "halo2-ipa-pasta"],
  ["halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4", "halo2-ipa-pasta"],
  ["stark/fri", "stark"],
  ["stark/fri/sha256-goldilocks", "stark"],
  ["stark/fri/poseidon2-goldilocks", "stark"],
  ["stark/fri/sha256_goldilocks.v1", "stark"],
]);

/** Ordered verifier labels and their explicit generic OpenVerify engine bindings. */
export const VERIFIER_BACKEND_REGISTRY_BINDINGS_V1 =
  /* @__PURE__ */ Object.freeze(
    /* @__PURE__ */ Array.from(
      ENGINE_BY_REGISTRY_LABEL_V1,
      ([label, engine]) => Object.freeze({ label, engine }),
    ),
  );

/**
 * Resolve one exact verifier-registry label to its generic OpenVerify engine.
 * Unknown, padded, normalized, or confusable values fail closed with `null`.
 *
 * @param {unknown} label
 * @returns {"halo2-ipa-pasta" | "stark" | null}
 */
export function verifierBackendRegistryTagV1(label) {
  if (typeof label !== "string") return null;
  return ENGINE_BY_REGISTRY_LABEL_V1.get(label) ?? null;
}

/** Return whether a value is one exact verifier-registry v1 label. */
export function isVerifierBackendRegistryLabelV1(label) {
  return typeof label === "string"
    && VERIFIER_BACKEND_REGISTRY_LABELS_V1.includes(label);
}

/**
 * Require one exact verifier-registry v1 label and return it unchanged.
 *
 * @param {unknown} label
 * @param {string} [context]
 * @returns {string}
 */
export function requireVerifierBackendRegistryLabelV1(label, context = "backend") {
  if (!isVerifierBackendRegistryLabelV1(label)) {
    throw new TypeError(
      `${context} uses unsupported verifier-registry label ${String(label)}`,
    );
  }
  return label;
}
