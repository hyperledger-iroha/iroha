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
    i105PrefixMismatchThrows();
    i105RejectsInvalidCharacters();
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
    final Map<String, Object> i105 = asMap(encodings.get("i105"), caseId + ".encodings.i105", caseId);
    final int prefix = asNumber(i105.get("prefix"), caseId + ".encodings.i105.prefix", caseId).intValue();
    final String i105String = asString(i105.get("string"), caseId + ".encodings.i105.string", caseId);
    final String defaultI105 = asString(encodings.get("i105_default"), caseId + ".encodings.i105_default", caseId);

    final AccountAddress canonical = AccountAddress.fromCanonicalHex(canonicalHex);
    final byte[] canonicalBytes = canonical.canonicalBytes();

    final AccountAddress.ParseResult i105Parsed = AccountAddress.parseEncoded(i105String, prefix);
    assert Arrays.equals(i105Parsed.address.canonicalBytes(), canonicalBytes)
        : caseId + ": i105 parse canonical mismatch";

    final AccountAddress decoded = AccountAddress.fromI105(defaultI105, null);
    assert Arrays.equals(decoded.canonicalBytes(), canonicalBytes)
        : caseId + ": i105_default canonical mismatch";

    final AccountAddress.ParseResult parsedDefault = AccountAddress.parseEncoded(defaultI105, null);
    assert parsedDefault.format == AccountAddress.Format.I105
        : caseId + ": i105_default parse format mismatch";
    assert Arrays.equals(parsedDefault.address.canonicalBytes(), canonicalBytes)
        : caseId + ": i105_default parse canonical mismatch";

    final String reencodedI105 = canonical.toI105(prefix);
    assert reencodedI105.equals(i105String) : caseId + ": i105 re-encode mismatch";
    assert reencodedI105.equals(defaultI105) : caseId + ": i105_default re-encode mismatch";
    assert canonical.canonicalHex().equalsIgnoreCase(canonicalHex)
        : caseId + ": canonical hex re-encode mismatch";

    final AccountAddress.DisplayFormats formats = canonical.displayFormats(prefix);
    assert formats.i105.equals(i105String) : caseId + ": displayFormats i105 mismatch";
    assert formats.discriminant == prefix : caseId + ": displayFormats discriminant mismatch";
    assert formats.i105Warning.equals(AccountAddress.i105WarningMessage())
        : caseId + ": displayFormats warning mismatch";

    final AccountAddress.DisplayFormats defaultFormats = canonical.displayFormats();
    assert defaultFormats.discriminant == AccountAddress.DEFAULT_I105_DISCRIMINANT
        : caseId + ": default displayFormats discriminant mismatch";
    assert defaultFormats.i105.equals(defaultI105)
        : caseId + ": default displayFormats i105 mismatch";
    assert defaultFormats.i105Warning.equals(AccountAddress.i105WarningMessage())
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
      case "i105":
        expectError(caseId, expected, () -> AccountAddress.parseEncoded(input, expectedPrefix));
        break;
      case "i105_default":
        expectError(caseId, expected, () -> AccountAddress.fromI105(input, null));
        expectError(caseId, expected, () -> AccountAddress.parseEncoded(input, null));
        break;
      case "canonical_hex":
        expectError(caseId, expected, () -> AccountAddress.parseAny(input, null));
        expectError(caseId, expected, () -> AccountAddress.parseEncoded(input, null));
        break;
      default:
        throw new IllegalStateException(caseId + ": unsupported negative format " + format);
    }
  }

  private static void goldenVectorsRoundTrip() throws Exception {
    final byte[] key = new byte[32];
    final AccountAddress address = AccountAddress.fromAccount("default", key, "ed25519");

    final String canonical = address.canonicalHex();
    final String i105 = address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);

    assert canonical.equals("0x020001200000000000000000000000000000000000000000000000000000000000000000")
        : "canonical encoding mismatch";
    assert i105.equals("6cmzPVPX4PK3NiYvG2FdPC5E9YVfkCYUXJCBpxzL71j1gsHxMkpCify")
        : "I105 encoding mismatch";

    final AccountAddress.ParseResult i105Parsed =
        AccountAddress.parseEncoded(i105, AccountAddress.DEFAULT_I105_DISCRIMINANT);
    assert i105Parsed.format == AccountAddress.Format.I105 : "expected I105 format";
    assert Arrays.equals(address.canonicalBytes(), i105Parsed.address.canonicalBytes())
        : "I105 round-trip mismatch";
  }

  private static void i105PrefixMismatchThrows() throws Exception {
    final byte[] key = new byte[32];
    Arrays.fill(key, (byte) 1);
    final AccountAddress address = AccountAddress.fromAccount("default", key, "ed25519");
    final String i105 = address.toI105(5);
    boolean threw = false;
    try {
      AccountAddress.parseEncoded(i105, 9);
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

  private static void i105RejectsInvalidCharacters() {
    boolean threw = false;
    try {
      AccountAddress.fromI105("invalid", null);
    } catch (final AccountAddress.AccountAddressException ex) {
      threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.INVALID_I105_CHAR;
    }
    assert threw : "I105 parsing should reject invalid non-I105 symbols";
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
    final AccountAddress roundTripped = AccountAddress.fromI105(address.toI105(1), 1);
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
        return message.contains("unexpected I105 discriminant")
            && (expectedPrefix == null || message.contains(expectedPrefix.toString()))
            && (foundPrefix == null || message.contains(foundPrefix.toString()));
      case "InvalidI105Char":
        final Object invalidChar = expected.get("char");
        return message.contains("invalid I105 alphabet symbol")
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
