package org.hyperledger.iroha.android.model.zk;

import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.Set;

/**
 * Low-level proof engines encoded by {@code iroha_data_model::zk::BackendTag}.
 *
 * <p>Protocol profiles are deliberately not enum variants. They are selected by one exact label
 * from {@link #VERIFIER_BACKEND_REGISTRY_LABELS_V1}.
 */
public enum VerifyingKeyBackendTag {
  HALO2_IPA_PASTA("halo2-ipa-pasta"),
  STARK("stark");

  /**
   * Exact native verifier configurations admitted by registry v1.
   *
   * <p>Equality is byte-for-byte. Callers must not trim, case-fold, infer a family, or accept
   * aliases.
   */
  public static final Set<String> VERIFIER_BACKEND_REGISTRY_LABELS_V1 =
      Collections.unmodifiableSet(
          new LinkedHashSet<>(
              Arrays.asList(
                  "halo2/ipa",
                  "halo2/pasta/kaigi-roster-v1",
                  "halo2/pasta/kaigi-usage-v1",
                  "halo2/pasta/ivm-overlay-bind",
                  "halo2/pasta/ivm-execution-v1",
                  "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
                  "halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2",
                  "halo2/pasta/kagemusha-recursive-spend-step-ep-two-parent-operation-protocol-v2",
                  "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
                  "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
                  "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
                  "stark/fri",
                  "stark/fri/sha256-goldilocks",
                  "stark/fri/poseidon2-goldilocks",
                  "stark/fri/sha256_goldilocks.v1")));

  private final String noritoValue;

  VerifyingKeyBackendTag(final String noritoValue) {
    this.noritoValue = noritoValue;
  }

  /** Returns the exact Norito string used in encoded instruction arguments. */
  public String noritoValue() {
    return noritoValue;
  }

  /** Parses one exact canonical Norito engine label. */
  public static VerifyingKeyBackendTag parse(final String value) {
    if (HALO2_IPA_PASTA.noritoValue.equals(value)) {
      return HALO2_IPA_PASTA;
    }
    if (STARK.noritoValue.equals(value)) {
      return STARK;
    }
    throw new IllegalArgumentException("unsupported backend tag: " + value);
  }

  /** Resolves one exact registry label to its low-level proof engine. */
  public static VerifyingKeyBackendTag verifierBackendRegistryTagV1(final String label) {
    if (label == null) {
      return null;
    }
    switch (label) {
      case "halo2/ipa":
      case "halo2/pasta/kaigi-roster-v1":
      case "halo2/pasta/kaigi-usage-v1":
      case "halo2/pasta/ivm-overlay-bind":
      case "halo2/pasta/ivm-execution-v1":
      case "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3":
      case "halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2":
      case "halo2/pasta/kagemusha-recursive-spend-step-ep-two-parent-operation-protocol-v2":
      case "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3":
      case "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3":
      case "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4":
        return HALO2_IPA_PASTA;
      case "stark/fri":
      case "stark/fri/sha256-goldilocks":
      case "stark/fri/poseidon2-goldilocks":
      case "stark/fri/sha256_goldilocks.v1":
        return STARK;
      default:
        return null;
    }
  }

  /** Returns true only for an exact registry-v1 label. */
  public static boolean isVerifierBackendRegistryLabelV1(final String raw) {
    return verifierBackendRegistryTagV1(raw) != null;
  }

  /** Requires an exact registry-v1 label and returns it unchanged. */
  public static String requireVerifierBackendRegistryLabelV1(
      final String raw, final String context) {
    if (!isVerifierBackendRegistryLabelV1(raw)) {
      throw new IllegalArgumentException(
          context + " uses unsupported verifier-registry label " + raw);
    }
    return raw;
  }
}
