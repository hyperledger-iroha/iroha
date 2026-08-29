package org.hyperledger.iroha.android.model.zk;

import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.Locale;
import java.util.Set;

/**
 * Low-level proof engines encoded by {@code iroha_data_model::zk::BackendTag}.
 *
 * <p>Privacy protocols and verifier profiles remain separate catalog labels
 * and never become Norito enum variants.
 */
public enum VerifyingKeyBackendTag {
  HALO2_IPA_PASTA("halo2-ipa-pasta"),
  STARK("stark");

  private final String noritoValue;

  VerifyingKeyBackendTag(final String noritoValue) {
    this.noritoValue = noritoValue;
  }

  /** Returns the exact canonical Norito label. */
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

  /** Exact verifier-registry v1 labels admitted by native Rust dispatch. */
  public static final Set<String> VERIFIER_BACKEND_REGISTRY_LABELS_V1 =
      immutableSet(
          "halo2/ipa",
          "halo2/pasta/kaigi-roster-v1",
          "halo2/pasta/kaigi-usage-v1",
          "halo2/pasta/ivm-execution-v1",
          "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
          "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
          "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
          "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
          "stark/fri/poseidon-x7-goldilocks-6x64-v1");

  private static final Set<String> STARK_FRI_PRODUCTION_BACKENDS =
      immutableSet("stark/fri/poseidon-x7-goldilocks-6x64-v1");

  private static final Set<String> PRODUCTION_NATIVE_HALO2_PASTA_BACKENDS =
      immutableSet(
          "halo2/pasta/kaigi-roster-v1",
          "halo2/pasta/kaigi-usage-v1",
          "halo2/pasta/ivm-execution-v1",
          "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
          "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
          "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
          "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4");

  private static final Set<String> TRUSTED_SETUP_BACKEND_SEGMENTS =
      immutableSet(
          "groth16", "kzg", "bn254", "bn256", "bls12", "srs", "crs",
          "ptau", "ceremony", "powersoftau");

  private static final Set<String> TRUSTED_SETUP_COMPACT_TOKENS =
      immutableSet(
          "groth16", "kzg", "bn254", "bn256", "bls12381", "bls12",
          "srs", "crs", "ptau", "ceremony", "trustedsetup",
          "structuredreferencestring", "universalsrs", "powersoftau");

  private static final String[] PRODUCTION_CLAIM_BACKEND_FRAGMENTS = {
    "productionready", "productionhardened", "productionenabled",
    "productionapproved", "productioncertified", "productionclaim",
    "claimedproduction", "mainnetready", "mainnetcomplete", "mainnetclaim",
    "claimedmainnet", "mainnetcertified", "mainnetapproved", "mainnetrelease",
    "auditedproduction", "externallyaudited", "thirdpartyaudited",
    "boiaudited", "auditedmainnet", "externalaudit", "auditpassed",
    "auditapproved", "auditsignoff", "auditclaim", "claimedaudit",
    "securityreviewpassed", "securityauditpassed", "securityaudited",
    "externalsecurityreview", "certifiedproduction", "certifiedmainnet",
    "releaseready", "releaseapproved", "releasecertified"
  };

  /** Resolves one exact registry label to its low-level proof engine. */
  public static VerifyingKeyBackendTag verifierBackendRegistryTagV1(final String label) {
    if ("halo2/ipa".equals(label) || PRODUCTION_NATIVE_HALO2_PASTA_BACKENDS.contains(label)) {
      return HALO2_IPA_PASTA;
    }
    if (STARK_FRI_PRODUCTION_BACKENDS.contains(label)) {
      return STARK;
    }
    return null;
  }

  /** Returns true only for an exact registry-v1 label. */
  public static boolean isVerifierBackendRegistryLabelV1(final String label) {
    return verifierBackendRegistryTagV1(label) != null;
  }

  /** Requires one exact registry-v1 label and returns it unchanged. */
  public static String requireVerifierBackendRegistryLabelV1(
      final String label, final String context) {
    if (!isVerifierBackendRegistryLabelV1(label)) {
      throw new IllegalArgumentException(
          context + " uses unsupported verifier-registry label " + label);
    }
    return label;
  }

  /** Human-facing catalog classification separate from the wire enum. */
  public enum CatalogBackendTag {
    PRODUCTION,
    UNSUPPORTED
  }

  /** Classifies only exact production labels; aliases are unsupported. */
  public static CatalogBackendTag fromCatalogLabel(final String raw) {
    final String label = raw == null ? "" : raw;
    if (label.isEmpty() || hasNonAscii(label)) {
      return CatalogBackendTag.UNSUPPORTED;
    }
    if (VERIFIER_BACKEND_REGISTRY_LABELS_V1.contains(label)
        || HALO2_IPA_PASTA.noritoValue.equals(label)
        || STARK.noritoValue.equals(label)) {
      return CatalogBackendTag.PRODUCTION;
    }
    return CatalogBackendTag.UNSUPPORTED;
  }

  /** Returns true only for an exact, portable production verifier label. */
  public static boolean isProductionVerifyBackendLabel(final String raw) {
    if (raw == null) {
      return false;
    }
    final String backend = raw;
    if (trimWhitespace(backend).isEmpty()
        || !trimWhitespace(backend).equals(backend)
        || !isPortableVerifierBackendLabel(backend)
        || isProductionClaimBackendLabel(backend)
        || isTrustedSetupBackendLabel(backend)
        || isDeveloperOnlyBackendLabel(backend)) {
      return false;
    }
    return "halo2/ipa".equals(backend)
        || STARK_FRI_PRODUCTION_BACKENDS.contains(backend)
        || PRODUCTION_NATIVE_HALO2_PASTA_BACKENDS.contains(backend);
  }

  /** Requires one exact production verifier label and returns it unchanged. */
  public static String requireProductionVerifyBackendLabel(
      final String raw, final String context) {
    if (raw == null || trimWhitespace(raw).isEmpty()) {
      throw new IllegalArgumentException(context + " must not be blank");
    }
    final String backend = raw;
    if (!trimWhitespace(backend).equals(backend)) {
      throw new IllegalArgumentException(context + " must not contain surrounding whitespace");
    }
    if (!isProductionVerifyBackendLabel(backend)) {
      throw new IllegalArgumentException(
          context + " uses unsupported production verifier backend " + backend);
    }
    return backend;
  }

  private static boolean isProductionClaimBackendLabel(final String raw) {
    final String compact = compactAscii(raw.toLowerCase(Locale.ROOT));
    for (final String fragment : PRODUCTION_CLAIM_BACKEND_FRAGMENTS) {
      if (compact.contains(fragment)) {
        return true;
      }
    }
    return false;
  }

  private static boolean isTrustedSetupBackendLabel(final String raw) {
    final String label = raw.toLowerCase(Locale.ROOT);
    final String compact = compactAscii(label);
    for (final String segment : label.split("[^a-z0-9]+")) {
      if (TRUSTED_SETUP_BACKEND_SEGMENTS.contains(segment)) {
        return true;
      }
    }
    for (final String token : TRUSTED_SETUP_COMPACT_TOKENS) {
      if (compact.contains(token)) {
        return true;
      }
    }
    return false;
  }

  private static boolean isDeveloperOnlyBackendLabel(final String raw) {
    final String label = raw.toLowerCase(Locale.ROOT);
    final String compact = compactAscii(label);
    for (final String fragment :
        new String[] {
          "notforproduction", "notproduction", "notproductionready", "notready",
          "replacebeforeproduction", "replacebeforemainnet", "draftonly"
        }) {
      if (compact.contains(fragment)) {
        return true;
      }
    }
    final StringBuilder letterRun = new StringBuilder();
    for (final String token : label.split("[^a-z0-9]+")) {
      if (token.isEmpty()) {
        continue;
      }
      if (isDeveloperOnlyBackendRun(token)) {
        return true;
      }
      if (token.length() == 1) {
        letterRun.append(token);
      } else {
        if (isDeveloperOnlyBackendRun(letterRun.toString())) {
          return true;
        }
        letterRun.setLength(0);
      }
    }
    return isDeveloperOnlyBackendRun(letterRun.toString());
  }

  private static boolean isDeveloperOnlyBackendRun(final String value) {
    return value.contains("debug")
        || value.contains("mock")
        || value.contains("fixture")
        || value.contains("dev")
        || value.contains("todo")
        || value.contains("draft")
        || value.contains("pending")
        || value.contains("replace")
        || "test".equals(value)
        || "dummy".equals(value)
        || "fake".equals(value)
        || "stub".equals(value)
        || "sample".equals(value)
        || "placeholder".equals(value);
  }

  private static boolean isPortableVerifierBackendLabel(final String value) {
    if (value.isEmpty()
        || !isLowerAsciiAlphanumeric(value.charAt(0))
        || !isLowerAsciiAlphanumeric(value.charAt(value.length() - 1))) {
      return false;
    }
    for (int index = 0; index < value.length(); index++) {
      final char ch = value.charAt(index);
      if (!isLowerAsciiAlphanumeric(ch)
          && ch != '/'
          && ch != ':'
          && ch != '.'
          && ch != '_'
          && ch != '-') {
        return false;
      }
    }
    for (final String unsafe :
        new String[] {"//", "::", "..", "/:", ":/", "/.", "./", ":.", ".:"}) {
      if (value.contains(unsafe)) {
        return false;
      }
    }
    return true;
  }

  private static boolean isLowerAsciiAlphanumeric(final char ch) {
    return ch >= '0' && ch <= '9' || ch >= 'a' && ch <= 'z';
  }

  private static boolean hasNonAscii(final String value) {
    for (int index = 0; index < value.length(); index++) {
      if (value.charAt(index) > 0x7F) {
        return true;
      }
    }
    return false;
  }

  static String trimWhitespace(final String value) {
    int start = 0;
    int end = value.length();
    while (start < end && Character.isWhitespace(value.charAt(start))) {
      start++;
    }
    while (end > start && Character.isWhitespace(value.charAt(end - 1))) {
      end--;
    }
    return value.substring(start, end);
  }

  private static String compactAscii(final String value) {
    final StringBuilder compact = new StringBuilder(value.length());
    for (int index = 0; index < value.length(); index++) {
      final char ch = value.charAt(index);
      if (isLowerAsciiAlphanumeric(ch)) {
        compact.append(ch);
      }
    }
    return compact.toString();
  }

  private static Set<String> immutableSet(final String... values) {
    return Collections.unmodifiableSet(new LinkedHashSet<>(Arrays.asList(values)));
  }
}
