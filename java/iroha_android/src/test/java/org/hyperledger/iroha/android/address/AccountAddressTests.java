package org.hyperledger.iroha.android.address;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.testing.SimpleJson;

public final class AccountAddressTests {

  private static final String FIXTURE_RELATIVE = "fixtures/account/address_vectors.json";

  private AccountAddressTests() {}

  public static void main(final String[] args) throws Exception {
    AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());
    complianceFixtureSuite();
    goldenVectorsRoundTrip();
    ih58PrefixMismatchThrows();
    compressedRequiresSentinel();
    curveSupportDefaults();
    curveSupportConfigurationToggle();
    singleKeyPayloadExtraction();
    System.out.println("[IrohaAndroid] Account address tests passed.");
  }

  private static void complianceFixtureSuite() throws Exception {
    final Path fixturePath = resolveFixturePath();
    final String json = Files.readString(fixturePath, StandardCharsets.UTF_8);
    final Map<String, Object> root = asMap(SimpleJson.parse(json), "root", "<root>");

    final Number formatVersion = asNumber(root.get("format_version"), "format_version", "<root>");
    assert formatVersion.intValue() == 1 : "expected format version 1";

    final int defaultPrefix =
        asNumber(root.get("default_network_prefix"), "default_network_prefix", "<root>").intValue();

    final Map<String, Object> cases = asMap(root.get("cases"), "cases", "<root>");
    final List<?> positives = asList(cases.get("positive"), "cases.positive", "<root>");
    for (Object entry : positives) {
      validatePositiveCase(asMap(entry, "cases.positive[]", "<positive>"));
    }

    final List<?> negatives = asList(cases.get("negative"), "cases.negative", "<root>");
    for (Object entry : negatives) {
      validateNegativeCase(asMap(entry, "cases.negative[]", "<negative>"), defaultPrefix);
    }
  }

  private static void validatePositiveCase(final Map<String, Object> vector) throws Exception {
    final String caseId = asString(vector.get("case_id"), "case_id", "<positive>");

    final Map<String, Object> encodings = asMap(vector.get("encodings"), caseId + ".encodings", caseId);
    final String canonicalHex = asString(encodings.get("canonical_hex"), caseId + ".encodings.canonical_hex", caseId);
    final Map<String, Object> ih58 = asMap(encodings.get("ih58"), caseId + ".encodings.ih58", caseId);
    final int prefix = asNumber(ih58.get("prefix"), caseId + ".encodings.ih58.prefix", caseId).intValue();
    final String ih58String = asString(ih58.get("string"), caseId + ".encodings.ih58.string", caseId);
    final String compressedHalf = asString(encodings.get("compressed"), caseId + ".encodings.compressed", caseId);
    final String compressedFull =
        asString(encodings.get("compressed_fullwidth"), caseId + ".encodings.compressed_fullwidth", caseId);

    final AccountAddress canonical = AccountAddress.fromCanonicalHex(canonicalHex);
    final byte[] canonicalBytes = canonical.canonicalBytes();

    final AccountAddress ih58Address = AccountAddress.fromIH58(ih58String, prefix);
    assert Arrays.equals(ih58Address.canonicalBytes(), canonicalBytes)
        : caseId + ": IH58 canonical mismatch";

    final AccountAddress.ParseResult ih58Parsed = AccountAddress.parseAny(ih58String, prefix);
    assert ih58Parsed.format == AccountAddress.Format.IH58 : caseId + ": IH58 parse format mismatch";
    assert Arrays.equals(ih58Parsed.address.canonicalBytes(), canonicalBytes)
        : caseId + ": IH58 parse canonical mismatch";

    for (String encodingLabel : List.of("half-width", "full-width")) {
      final String encoding = "half-width".equals(encodingLabel) ? compressedHalf : compressedFull;
      final AccountAddress decoded = AccountAddress.fromCompressedSora(encoding);
      assert Arrays.equals(decoded.canonicalBytes(), canonicalBytes)
          : caseId + ": compressed " + encodingLabel + " canonical mismatch";

      final AccountAddress.ParseResult parsed = AccountAddress.parseAny(encoding, null);
      assert parsed.format == AccountAddress.Format.COMPRESSED
          : caseId + ": compressed " + encodingLabel + " parse format mismatch";
      assert Arrays.equals(parsed.address.canonicalBytes(), canonicalBytes)
          : caseId + ": compressed " + encodingLabel + " parse canonical mismatch";
    }

    final AccountAddress.ParseResult canonicalParsed = AccountAddress.parseAny(canonicalHex, null);
    assert canonicalParsed.format == AccountAddress.Format.CANONICAL_HEX
        : caseId + ": canonical parse format mismatch";
    assert Arrays.equals(canonicalParsed.address.canonicalBytes(), canonicalBytes)
        : caseId + ": canonical parse mismatch";

    final String reencodedIh58 = canonical.toIH58(prefix);
    assert reencodedIh58.equals(ih58String) : caseId + ": IH58 re-encode mismatch";
    final String reencodedHalf = canonical.toCompressedSora();
    assert reencodedHalf.equals(compressedHalf) : caseId + ": compressed re-encode mismatch";
    final String reencodedFull = canonical.toCompressedSoraFullWidth();
    assert reencodedFull.equals(compressedFull) : caseId + ": compressed full-width re-encode mismatch";
    assert canonical.canonicalHex().equalsIgnoreCase(canonicalHex)
        : caseId + ": canonical hex re-encode mismatch";

    final AccountAddress.DisplayFormats formats = canonical.displayFormats(prefix);
    assert formats.ih58.equals(ih58String) : caseId + ": displayFormats IH58 mismatch";
    assert formats.compressed.equals(compressedHalf) : caseId + ": displayFormats compressed mismatch";
    assert formats.networkPrefix == prefix : caseId + ": displayFormats prefix mismatch";
    assert formats.compressedWarning.equals(AccountAddress.compressedWarningMessage())
        : caseId + ": displayFormats warning mismatch";

    final AccountAddress.DisplayFormats defaultFormats = canonical.displayFormats();
    assert defaultFormats.networkPrefix == AccountAddress.DEFAULT_IH58_PREFIX
        : caseId + ": default displayFormats prefix mismatch";
    final String expectedDefaultIh58 = canonical.toIH58(AccountAddress.DEFAULT_IH58_PREFIX);
    assert defaultFormats.ih58.equals(expectedDefaultIh58)
        : caseId + ": default displayFormats IH58 mismatch";
    assert defaultFormats.compressed.equals(compressedHalf)
        : caseId + ": default displayFormats compressed mismatch";
    assert defaultFormats.compressedWarning.equals(AccountAddress.compressedWarningMessage())
        : caseId + ": default displayFormats warning mismatch";
  }

  private static void validateNegativeCase(
      final Map<String, Object> vector, final int defaultPrefix) throws Exception {
    final String caseId = asString(vector.get("case_id"), "case_id", "<negative>");
    final String format = asString(vector.get("format"), caseId + ".format", caseId);
    final String input = asString(vector.get("input"), caseId + ".input", caseId);
    final Map<String, Object> expected =
        asMap(vector.get("expected_error"), caseId + ".expected_error", caseId);

    final Integer expectedPrefix =
        vector.containsKey("expected_prefix")
            ? asNumber(vector.get("expected_prefix"), caseId + ".expected_prefix", caseId).intValue()
            : defaultPrefix;

    switch (format) {
      case "ih58":
        expectError(caseId, expected, () -> AccountAddress.fromIH58(input, expectedPrefix));
        expectError(caseId, expected, () -> AccountAddress.parseAny(input, expectedPrefix));
        break;
      case "compressed":
        expectError(caseId, expected, () -> AccountAddress.fromCompressedSora(input));
        expectError(caseId, expected, () -> AccountAddress.parseAny(input, null));
        break;
      case "canonical_hex":
        expectError(caseId, expected, () -> AccountAddress.fromCanonicalHex(input));
        expectError(caseId, expected, () -> AccountAddress.parseAny(input, null));
        break;
      default:
        throw new IllegalStateException(caseId + ": unsupported negative format " + format);
    }
  }

  private static void goldenVectorsRoundTrip() throws Exception {
    final byte[] key = new byte[32];
    final AccountAddress address = AccountAddress.fromAccount("default", key, "ed25519");

    final String canonical = address.canonicalHex();
    final String ih58 = address.toIH58(AccountAddress.DEFAULT_IH58_PREFIX);
    final String compressed = address.toCompressedSora();

    assert canonical.equals("0x020001200000000000000000000000000000000000000000000000000000000000000000")
        : "canonical encoding mismatch";
    assert ih58.equals("6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCnGL")
        : "IH58 encoding mismatch";
    assert compressed.equals("sorauﾛ1NcMBm2dﾌBokヱDﾑﾅekAbｶﾍﾜﾇﾐMFｽヱﾋZﾘ2u4WGUMMS63EY6")
        : "compressed encoding mismatch";

    final AccountAddress.ParseResult ih58Parsed =
        AccountAddress.parseAny(ih58, AccountAddress.DEFAULT_IH58_PREFIX);
    assert ih58Parsed.format == AccountAddress.Format.IH58 : "expected IH58 format";
    assert Arrays.equals(address.canonicalBytes(), ih58Parsed.address.canonicalBytes())
        : "IH58 round-trip mismatch";

    final AccountAddress.ParseResult compressedParsed = AccountAddress.parseAny(compressed, null);
    assert compressedParsed.format == AccountAddress.Format.COMPRESSED : "expected compressed format";
    assert Arrays.equals(address.canonicalBytes(), compressedParsed.address.canonicalBytes())
        : "compressed round-trip mismatch";

    final AccountAddress.ParseResult hexParsed = AccountAddress.parseAny(canonical, null);
    assert hexParsed.format == AccountAddress.Format.CANONICAL_HEX : "expected canonical hex format";
    assert Arrays.equals(address.canonicalBytes(), hexParsed.address.canonicalBytes())
        : "canonical hex round-trip mismatch";
  }

  private static void ih58PrefixMismatchThrows() throws Exception {
    final byte[] key = new byte[32];
    Arrays.fill(key, (byte) 1);
    final AccountAddress address = AccountAddress.fromAccount("default", key, "ed25519");
    final String ih58 = address.toIH58(5);
    boolean threw = false;
    try {
      AccountAddress.parseAny(ih58, 9);
    } catch (final AccountAddress.AccountAddressException ex) {
      threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX;
    }
    assert threw : "expected prefix mismatch to throw";
  }

  private static void singleKeyPayloadExtraction() throws Exception {
    final byte[] key = new byte[32];
    for (int i = 0; i < key.length; i++) {
      key[i] = (byte) i;
    }
    final AccountAddress address = AccountAddress.fromAccount("default", key, "ed25519");
    final java.util.Optional<AccountAddress.SingleKeyPayload> payload = address.singleKeyPayload();
    assert payload.isPresent() : "expected single-key payload";
    final AccountAddress.SingleKeyPayload info = payload.get();
    assert info.curveId() == 0x01 : "curve id mismatch";
    assert Arrays.equals(info.publicKey(), key) : "public key mismatch";
  }

  private static void compressedRequiresSentinel() {
    boolean threw = false;
    try {
      AccountAddress.fromCompressedSora("invalid");
    } catch (final AccountAddress.AccountAddressException ex) {
      threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL;
    }
    assert threw : "compressed parsing should reject invalid sentinel";
  }

  private static void curveSupportDefaults() {
    AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());
    final byte[] key = new byte[32];
    boolean threw = false;
    try {
      AccountAddress.fromAccount("default", key, "ml-dsa");
    } catch (final AccountAddress.AccountAddressException ex) {
      threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.UNSUPPORTED_ALGORITHM;
    }
    assert threw : "expected ML-DSA to be disabled by default";
  }

  private static void curveSupportConfigurationToggle() throws Exception {
    final byte[] key = new byte[32];
    AccountAddress.configureCurveSupport(
        AccountAddress.CurveSupportConfig.builder().allowMlDsa(true).build());
    final AccountAddress address = AccountAddress.fromAccount("default", key, "ml-dsa");
    final AccountAddress roundTripped = AccountAddress.fromIH58(address.toIH58(1), 1);
    assert Arrays.equals(address.canonicalBytes(), roundTripped.canonicalBytes())
        : "ML-DSA enablement round-trip mismatch";
    AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());
  }

  private static Path resolveFixturePath() {
    final Path relative = Path.of(FIXTURE_RELATIVE);
    if (Files.exists(relative)) {
      return relative;
    }
    Path candidate = relative;
    for (int depth = 0; depth < 5; depth++) {
      candidate = Path.of("..").resolve(candidate);
      if (Files.exists(candidate)) {
        return candidate.normalize();
      }
    }
    throw new IllegalStateException("Unable to locate fixture at or above: " + FIXTURE_RELATIVE);
  }

  private static Map<String, Object> asMap(final Object value, final String field, final String context) {
    if (!(value instanceof Map<?, ?> raw)) {
      throw new IllegalStateException(context + ": expected object for " + field);
    }
    final Map<String, Object> map = new LinkedHashMap<>();
    raw.forEach((key, val) -> map.put(Objects.toString(key), val));
    return map;
  }

  private static List<?> asList(final Object value, final String field, final String context) {
    if (!(value instanceof List<?> list)) {
      throw new IllegalStateException(context + ": expected array for " + field);
    }
    return list;
  }

  private static String asString(final Object value, final String field, final String context) {
    if (!(value instanceof String str)) {
      throw new IllegalStateException(context + ": expected string for " + field);
    }
    return str;
  }

  private static Number asNumber(final Object value, final String field, final String context) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(context + ": expected number for " + field);
    }
    return number;
  }

  private static AccountAddress.AccountAddressErrorCode codeForKind(final String kind) {
    return switch (kind) {
      case "UnsupportedFormat" -> AccountAddress.AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT;
      case "InvalidDigit" -> AccountAddress.AccountAddressErrorCode.INVALID_COMPRESSED_DIGIT;
      case "UnsupportedController" -> AccountAddress.AccountAddressErrorCode.UNKNOWN_CONTROLLER_TAG;
      default -> AccountAddress.AccountAddressErrorCode.valueOf(camelToEnum(kind));
    };
  }

  private static String camelToEnum(final String kind) {
    final StringBuilder sb = new StringBuilder();
    for (int i = 0; i < kind.length(); i++) {
      final char ch = kind.charAt(i);
      if (Character.isUpperCase(ch) && i > 0) {
        sb.append('_');
      }
      sb.append(Character.toUpperCase(ch));
    }
    return sb.toString();
  }

  private static void expectError(
      final String caseId, final Map<String, Object> expected, final CheckedRunnable action) throws Exception {
    boolean matched = false;
    try {
      action.run();
    } catch (final AccountAddress.AccountAddressException ex) {
      matched = matchesExpectedError(caseId, expected, ex);
    }
    assert matched : caseId + ": expected failure (" + expected.get("kind") + ")";
  }

  private static boolean matchesExpectedError(
      final String caseId, final Map<String, Object> expected, final AccountAddress.AccountAddressException ex) {
    final String kind = asString(expected.get("kind"), caseId + ".expected_error.kind", caseId);
    final String message = ex.getMessage() == null ? "" : ex.getMessage();
    final AccountAddress.AccountAddressErrorCode expectedCode = codeForKind(kind);
    if (ex.getCode() != expectedCode) {
      return false;
    }
    switch (kind) {
      case "UnexpectedNetworkPrefix":
        final Object expectedPrefix = expected.get("expected");
        final Object foundPrefix = expected.get("found");
        return message.contains("unexpected IH58 network prefix")
            && (expectedPrefix == null || message.contains(expectedPrefix.toString()))
            && (foundPrefix == null || message.contains(foundPrefix.toString()));
      case "InvalidCompressedChar":
        final Object invalidChar = expected.get("char");
        return message.contains("invalid compressed alphabet symbol")
            && (invalidChar == null || message.contains(Objects.toString(invalidChar)));
      case "InvalidMultisigPolicy":
        return message.contains("InvalidMultisigPolicy") || message.contains("unknown controller tag");
      default:
        return true;
    }
  }

  @FunctionalInterface
  private interface CheckedRunnable {
    void run() throws Exception;
  }
}
