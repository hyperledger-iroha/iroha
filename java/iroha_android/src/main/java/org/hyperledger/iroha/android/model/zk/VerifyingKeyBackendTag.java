package org.hyperledger.iroha.android.model.zk;

import java.util.Locale;
import java.util.Set;

/**
 * Backend identifiers for verifying key records.
 *
 * <p>Matches the Norito enums exposed by {@code iroha_data_model::zk::BackendTag}.
 */
public enum VerifyingKeyBackendTag {
  HALO2_IPA_PASTA("halo2-ipa-pasta"),
  HALO2_BN254("halo2-bn254"),
  GROTH16("groth16"),
  STARK("stark"),
  UNSUPPORTED("unsupported"),
  HALO2_IPA_ORCHARD("halo2-ipa-orchard"),
  GROTH16_BLS12_377("groth16-bls12-377"),
  FCMP_PLUS_PLUS_CURVE_TREE("fcmp-plus-plus-curve-tree"),
  LATTICE_PCS_SIS("lattice-pcs-sis"),
  MIDEN_STARK("miden-stark"),
  AZTEC_PLONKISH_PRIVATE_KERNEL("aztec-plonkish-private-kernel"),
  PQ_MASP_STARK_FRI("pq-masp-stark-fri"),
  ANONYMOUS_PGC("anonymous-pgc"),
  VERANGE("verange"),
  ZKAT("zkat"),
  RECURSIVE_ANONYMOUS_ADMISSION("recursive-anonymous-admission"),
  VEGA_EXISTING_CREDENTIAL_ZK("vega-existing-credential-zk"),
  SILENT_THRESHOLD_ANONCRED("silent-threshold-anoncred"),
  ZK_X509("zk-x509"),
  SIS_WITH_HINTS("sis-with-hints");

  private final String noritoValue;

  VerifyingKeyBackendTag(final String noritoValue) {
    this.noritoValue = noritoValue;
  }

  /** Returns the Norito string used in encoded instruction arguments. */
  public String noritoValue() {
    return noritoValue;
  }

  /** Returns whether this backend is cataloged but still pending production gates. */
  public boolean isPendingProductionBackend() {
    switch (this) {
      case HALO2_IPA_ORCHARD:
      case GROTH16_BLS12_377:
      case FCMP_PLUS_PLUS_CURVE_TREE:
      case LATTICE_PCS_SIS:
      case MIDEN_STARK:
      case AZTEC_PLONKISH_PRIVATE_KERNEL:
      case PQ_MASP_STARK_FRI:
      case ANONYMOUS_PGC:
      case VERANGE:
      case ZKAT:
      case RECURSIVE_ANONYMOUS_ADMISSION:
      case VEGA_EXISTING_CREDENTIAL_ZK:
      case SILENT_THRESHOLD_ANONCRED:
      case ZK_X509:
      case SIS_WITH_HINTS:
        return true;
      default:
        return false;
    }
  }

  /** Parses a Norito backend string into the corresponding enum value. */
  public static VerifyingKeyBackendTag parse(final String value) {
    if (value == null) {
      throw new IllegalArgumentException("backend tag must not be null");
    }
    final String normalized = trimWhitespace(value).toLowerCase(Locale.ROOT);
    for (final VerifyingKeyBackendTag tag : values()) {
      if (tag.noritoValue.equals(normalized)) {
        return tag;
      }
    }
    throw new IllegalArgumentException("unsupported backend tag: " + value);
  }

  /** Classifies catalog and verifier labels without enabling pending production backends. */
  public static VerifyingKeyBackendTag fromCatalogLabel(final String raw) {
    final String label = raw == null ? "" : trimWhitespace(raw).toLowerCase(Locale.ROOT);
    if (label.isEmpty()) {
      return UNSUPPORTED;
    }
    if (hasNonAscii(label)) {
      return UNSUPPORTED;
    }
    final String compact = compactAscii(label);

    if ("unsupported".equals(label) || "unsupported".equals(compact)) {
      return UNSUPPORTED;
    }
    if (compact.contains("pqmasp") || compact.contains("postquantummasp")) {
      return PQ_MASP_STARK_FRI;
    }
    if (compact.contains("anonymouspgc") || compact.contains("pgckoutofn")) {
      return ANONYMOUS_PGC;
    }
    if (compact.contains("verange")) {
      return VERANGE;
    }
    if (compact.contains("zkat") || compact.contains("policyprivateauthenticator")) {
      return ZKAT;
    }
    if (compact.contains("zkams") || compact.contains("recursiveanonymousadmission")) {
      return RECURSIVE_ANONYMOUS_ADMISSION;
    }
    if (compact.contains("vega") || compact.contains("existingcredentialzk")) {
      return VEGA_EXISTING_CREDENTIAL_ZK;
    }
    if (compact.contains("silentthreshold")
        || compact.contains("thresholdanonymouscredential")) {
      return SILENT_THRESHOLD_ANONCRED;
    }
    if (compact.contains("zkx509") || compact.contains("x509") || compact.contains("zkvmx509")) {
      return ZK_X509;
    }
    if (compact.contains("siswithhints")
        || compact.contains("sishints")
        || compact.contains("latticeanonymouscredentials")) {
      return SIS_WITH_HINTS;
    }
    if (compact.contains("orchard") || compact.contains("zcashorchard")) {
      return HALO2_IPA_ORCHARD;
    }
    if (compact.contains("penumbra")
        || compact.contains("masp")
        || compact.contains("bls12377")
        || compact.contains("decaf377")) {
      return GROTH16_BLS12_377;
    }
    if (compact.contains("fcmp") || compact.contains("monero") || compact.contains("curvetree")) {
      return FCMP_PLUS_PLUS_CURVE_TREE;
    }
    if (compact.contains("lattice") || compact.contains("pcssis") || compact.contains("jindo")) {
      return LATTICE_PCS_SIS;
    }
    if (compact.contains("miden")) {
      return MIDEN_STARK;
    }
    if (compact.contains("aztec")) {
      return AZTEC_PLONKISH_PRIVATE_KERNEL;
    }
    if (compact.contains("halo2") && compact.contains("bn254")) {
      return HALO2_BN254;
    }
    if (compact.contains("groth16")) {
      return GROTH16;
    }
    if (compact.contains("stark")) {
      return STARK;
    }
    if ("halo2ipa".equals(compact)
        || "halo2ipapasta".equals(compact)
        || "halo2pasta".equals(compact)
        || (compact.contains("halo2")
            && (compact.contains("ipa") || compact.contains("pasta")))) {
      return HALO2_IPA_PASTA;
    }

    return UNSUPPORTED;
  }

  public static boolean isPendingProductionBackendLabel(final String raw) {
    return fromCatalogLabel(raw).isPendingProductionBackend();
  }

  public static boolean isProductionVerifyBackendLabel(final String raw) {
    if (raw == null) {
      return false;
    }
    final String backend = raw;
    if (trimWhitespace(backend).isEmpty()
        || !trimWhitespace(backend).equals(backend)
        || !isPortableVerifierBackendLabel(backend)
        || isPendingProductionBackendLabel(backend)
        || isProductionClaimBackendLabel(backend)
        || isTrustedSetupBackendLabel(backend)
        || isDeveloperOnlyBackendLabel(backend)) {
      return false;
    }
    return "halo2/ipa".equals(backend)
        || isStarkFriProductionBackendLabel(backend)
        || isNativeHalo2PastaProductionBackendLabel(backend);
  }

  public static String requireProductionVerifyBackendLabel(
      final String raw, final String context) {
    if (raw == null || trimWhitespace(raw).isEmpty()) {
      throw new IllegalArgumentException(context + " must not be blank");
    }
    final String backend = raw;
    if (!isProductionVerifyBackendLabel(backend)) {
      throw new IllegalArgumentException(
          context + " uses unsupported production verifier backend " + backend);
    }
    return backend;
  }

  private static final Set<String> PRODUCTION_NATIVE_HALO2_PASTA_BACKENDS =
      Set.of(
          "halo2/pasta/kaigi-roster-v1",
          "halo2/pasta/kaigi-usage-v1",
          "halo2/pasta/ivm-overlay-bind",
          "halo2/pasta/ivm-execution-v1",
          "halo2/pasta/offline-note-recursive",
          "halo2/pasta/kagemusha-folded-v1",
          "halo2/pasta/kagemusha-recursive-aggregation-v1",
          "halo2/pasta/kagemusha-recursive-spend-lineage-v1",
          "halo2/pasta/anon-transfer-2x2-merkle16-poseidon-diversified",
          "halo2/pasta/anon-unshield-merkle16-poseidon-diversified",
          "halo2/pasta/anon-unshield-2in-1change-merkle16-poseidon-diversified");

  private static final Set<String> STARK_FRI_PRODUCTION_BACKENDS =
      Set.of(
          "stark/fri",
          "stark/fri/sha256-goldilocks",
          "stark/fri/poseidon2-goldilocks",
          "stark/fri/sha256_goldilocks.v1");

  private static final Set<String> TRUSTED_SETUP_BACKEND_SEGMENTS =
      Set.of(
          "groth16",
          "kzg",
          "bn254",
          "bn256",
          "bls12",
          "srs",
          "crs",
          "ptau",
          "ceremony",
          "powersoftau");

  private static final Set<String> TRUSTED_SETUP_COMPACT_TOKENS =
      Set.of(
          "groth16",
          "kzg",
          "bn254",
          "bn256",
          "bls12381",
          "bls12",
          "srs",
          "crs",
          "ptau",
          "ceremony",
          "trustedsetup",
          "structuredreferencestring",
          "universalsrs",
          "powersoftau");

  private static final String[] PRODUCTION_CLAIM_BACKEND_FRAGMENTS = {
    "productionready",
    "productionhardened",
    "productionenabled",
    "productionapproved",
    "productioncertified",
    "productionclaim",
    "claimedproduction",
    "mainnetready",
    "mainnetcomplete",
    "mainnetclaim",
    "claimedmainnet",
    "auditedproduction",
    "externallyaudited",
    "auditpassed",
    "auditapproved",
    "auditsignoff",
    "auditclaim",
    "claimedaudit",
    "securityreviewpassed"
  };

  private static boolean isTrustedSetupBackendLabel(final String raw) {
    final String label = trimWhitespace(raw).toLowerCase(Locale.ROOT);
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
    return "groth16".equals(label)
        || label.startsWith("groth16/")
        || "kzg".equals(label)
        || label.startsWith("kzg/")
        || "bn254".equals(label)
        || "bn256".equals(label)
        || "bls12_381".equals(label)
        || "bls12-381".equals(label)
        || "halo2/bn254".equals(label)
        || label.startsWith("halo2/bn254/")
        || label.contains("/bn254")
        || label.contains(":bn254")
        || label.contains("/bn256")
        || label.contains(":bn256")
        || label.contains("/bls12")
        || label.contains(":bls12")
        || "halo2/kzg".equals(label)
        || label.startsWith("halo2/kzg/")
        || label.contains("/kzg")
        || label.contains(":kzg");
  }

  private static boolean isDeveloperOnlyBackendLabel(final String raw) {
    final String label = trimWhitespace(raw).toLowerCase(Locale.ROOT);
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
        continue;
      }
      if (isDeveloperOnlyBackendRun(letterRun.toString())) {
        return true;
      }
      letterRun.setLength(0);
    }
    return isDeveloperOnlyBackendRun(letterRun.toString());
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

  private static boolean isDeveloperOnlyBackendRun(final String value) {
    return value.contains("debug")
        || value.contains("mock")
        || value.contains("fixture")
        || value.contains("dev")
        || value.equals("test")
        || value.equals("dummy")
        || value.equals("fake")
        || value.equals("stub")
        || value.equals("sample")
        || value.equals("placeholder");
  }

  private static boolean isStarkFriProductionBackendLabel(final String backend) {
    return STARK_FRI_PRODUCTION_BACKENDS.contains(backend);
  }

  private static boolean isNativeHalo2PastaProductionBackendLabel(final String backend) {
    final String normalized = normalizeNativeHalo2PastaBackendLabel(backend);
    return normalized != null && PRODUCTION_NATIVE_HALO2_PASTA_BACKENDS.contains(normalized);
  }

  private static boolean isPortableVerifierBackendLabel(final String value) {
    for (int index = 0; index < value.length(); index++) {
      final char ch = value.charAt(index);
      final boolean allowed =
          (ch >= '0' && ch <= '9')
              || (ch >= 'A' && ch <= 'Z')
              || (ch >= 'a' && ch <= 'z')
              || ch == '/'
              || ch == ':'
              || ch == '.'
              || ch == '_'
              || ch == '-';
      if (!allowed) {
        return false;
      }
    }
    return true;
  }

  private static String normalizeNativeHalo2PastaBackendLabel(final String raw) {
    final String backend = raw;
    if (backend.isEmpty() || !trimWhitespace(backend).equals(backend)) {
      return null;
    }
    final String[][] prefixes = {
      {"halo2/pasta/ipa/", "halo2/pasta/"},
      {"halo2/pasta/", "halo2/pasta/"},
      {"halo2/ipa::", "halo2/pasta/"},
      {"halo2/ipa:", "halo2/pasta/"},
      {"halo2/ipa/", "halo2/pasta/"}
    };
    for (final String[] entry : prefixes) {
      final String prefix = entry[0];
      if (backend.startsWith(prefix)) {
        final String rest = backend.substring(prefix.length());
        return rest.isEmpty() ? null : entry[1] + rest;
      }
    }
    return null;
  }

  private static String compactAscii(final String value) {
    final StringBuilder builder = new StringBuilder(value.length());
    for (int index = 0; index < value.length(); index++) {
      final char ch = value.charAt(index);
      if ((ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'z')) {
        builder.append(ch);
      }
    }
    return builder.toString();
  }

  private static boolean hasNonAscii(final String value) {
    for (int index = 0; index < value.length(); index++) {
      if (value.charAt(index) > 0x7F) {
        return true;
      }
    }
    return false;
  }

  private static String trimWhitespace(final String value) {
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
}
