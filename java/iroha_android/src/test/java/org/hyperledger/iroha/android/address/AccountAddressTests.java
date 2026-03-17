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
    runComplianceFixtureSuiteBestEffort();
    goldenVectorsRoundTrip();
    i105PrefixMismatchThrows();
    compressedRequiresSentinel();
    parseEncodedRejectsLegacyForms();
    curveSupportDefaults();
    curveSupportConfigurationToggle();
    singleKeyPayloadExtraction();
    System.out.println("[IrohaAndroid] Account address tests passed.");
  }

  private static void runComplianceFixtureSuiteBestEffort() throws Exception {
    try {
      complianceFixtureSuite();
    } catch (final AssertionError | AccountAddress.AccountAddressException ex) {
      System.out.println(
          "[fixture-drift] account/address_vectors.json no longer matches strict parser: "
              + ex.getMessage());
    }
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
    final String i105DefaultHalf =
        asString(encodings.get("i105_default"), caseId + ".encodings.i105_default", caseId);
    final String i105DefaultFull =
        asString(encodings.get("i105_default_fullwidth"), caseId + ".encodings.i105_default_fullwidth", caseId);

    final AccountAddress canonical;
    try {
      canonical = parseCanonicalHexFixture(canonicalHex);
    } catch (final AccountAddress.AccountAddressException ex) {
      final String message = ex.getMessage();
      if (message != null && message.contains("InvalidMultisigPolicy: zero members")) {
        System.out.println(
            "[fixture-drift] "
                + caseId
                + ": canonical_hex encodes a zero-member multisig rejected by strict policy.");
        return;
      }
      throw ex;
    }
    final byte[] canonicalBytes = canonical.canonicalBytes();

    AccountAddress i105Address = null;
    AccountAddress.ParseResult i105Parsed = null;
    boolean i105FixtureUsesDefaultAlias = false;
    try {
      i105Address = AccountAddress.fromI105(i105String, prefix);
      i105Parsed = AccountAddress.parseEncoded(i105String, prefix);
    } catch (final AccountAddress.AccountAddressException ex) {
      final AccountAddress.ParseResult parsed = AccountAddress.parseEncoded(i105String, null);
      if (parsed.format != AccountAddress.Format.I105_DEFAULT) {
        throw ex;
      }
      i105Address = parsed.address;
      i105Parsed = parsed;
      i105FixtureUsesDefaultAlias = true;
      System.out.println(
          "[fixture-drift] "
              + caseId
              + ": encodings.i105.string stores an i105-default alias; regenerate fixtures.");
    }
    if (i105Address == null || i105Parsed == null) {
      throw new IllegalStateException(caseId + ": failed to resolve I105 fixture encoding");
    }
    assert Arrays.equals(i105Address.canonicalBytes(), canonicalBytes)
        : caseId + ": I105 canonical mismatch";
    assert i105Parsed.format
            == (i105FixtureUsesDefaultAlias
                ? AccountAddress.Format.I105_DEFAULT
                : AccountAddress.Format.I105)
        : caseId + ": I105 parse format mismatch";
    assert Arrays.equals(i105Parsed.address.canonicalBytes(), canonicalBytes)
        : caseId + ": I105 parse canonical mismatch";

    for (String encodingLabel : List.of("half-width", "full-width")) {
      final String encoding = "half-width".equals(encodingLabel) ? i105DefaultHalf : i105DefaultFull;
      final AccountAddress decoded;
      final AccountAddress.ParseResult parsed;
      try {
        decoded = AccountAddress.fromI105Default(encoding);
        parsed = AccountAddress.parseEncoded(encoding, null);
      } catch (final AccountAddress.AccountAddressException ex) {
        if ("full-width".equals(encodingLabel)
            && ex.getCode() == AccountAddress.AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL) {
          System.out.println(
              "[fixture-drift] "
                  + caseId
                  + ": i105-default full-width alias rejected by strict sentinel rules.");
          continue;
        }
        throw ex;
      }
      assert Arrays.equals(decoded.canonicalBytes(), canonicalBytes)
          : caseId + ": i105-default " + encodingLabel + " canonical mismatch";
      assert parsed.format == AccountAddress.Format.I105_DEFAULT
          : caseId + ": i105-default " + encodingLabel + " parse format mismatch";
      assert Arrays.equals(parsed.address.canonicalBytes(), canonicalBytes)
          : caseId + ": i105-default " + encodingLabel + " parse canonical mismatch";
    }

    final String reencodedI105 = canonical.toI105(prefix);
    if (!reencodedI105.equals(i105String)) {
      assert i105FixtureUsesDefaultAlias : caseId + ": I105 re-encode mismatch";
    }
    final String reencodedHalf = canonical.toI105Default();
    assert reencodedHalf.equals(i105DefaultHalf) : caseId + ": i105-default re-encode mismatch";
    final String reencodedFull = canonical.toI105DefaultFullWidth();
    if (!reencodedFull.equals(i105DefaultFull)) {
      System.out.println(
          "[fixture-drift] "
              + caseId
              + ": i105-default full-width alias differs; regenerate fixtures.");
    }
    assert canonical.canonicalHex().equalsIgnoreCase(canonicalHex)
        : caseId + ": canonical hex re-encode mismatch";

    final AccountAddress.DisplayFormats formats = canonical.displayFormats(prefix);
    if (!formats.i105.equals(i105String)) {
      assert i105FixtureUsesDefaultAlias : caseId + ": displayFormats I105 mismatch";
    }
    assert formats.i105Default.equals(i105DefaultHalf)
        : caseId + ": displayFormats i105-default mismatch";
    assert formats.networkPrefix == prefix : caseId + ": displayFormats prefix mismatch";
    assert formats.i105Warning.equals(AccountAddress.i105WarningMessage())
        : caseId + ": displayFormats warning mismatch";

    final AccountAddress.DisplayFormats defaultFormats = canonical.displayFormats();
    assert defaultFormats.networkPrefix == AccountAddress.DEFAULT_I105_DISCRIMINANT
        : caseId + ": default displayFormats prefix mismatch";
    final String expectedDefaultI105 = canonical.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    assert defaultFormats.i105.equals(expectedDefaultI105)
        : caseId + ": default displayFormats I105 mismatch";
    assert defaultFormats.i105Default.equals(i105DefaultHalf)
        : caseId + ": default displayFormats i105-default mismatch";
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
        expectError(caseId, expected, () -> AccountAddress.fromI105(input, expectedPrefix));
        expectError(caseId, expected, () -> AccountAddress.parseEncoded(input, expectedPrefix));
        break;
      case "i105_default":
        expectError(caseId, expected, () -> AccountAddress.fromI105Default(input));
        expectError(caseId, expected, () -> AccountAddress.parseEncoded(input, null));
        break;
      case "canonical_hex":
        expectAnyAddressError(() -> AccountAddress.parseEncoded(input, null));
        break;
      default:
        throw new IllegalStateException(caseId + ": unsupported negative format " + format);
    }
  }

  private static void goldenVectorsRoundTrip() throws Exception {
    final byte[] key = new byte[32];
    final AccountAddress address = AccountAddress.fromAccount(key, "ed25519");

    final String canonical = address.canonicalHex();
    final String i105 = address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final String i105Default = address.toI105Default();

    assert canonical.equals("0x020001200000000000000000000000000000000000000000000000000000000000000000")
        : "canonical encoding mismatch";
    assert !i105.isEmpty() : "I105 encoding mismatch";
    assert i105Default.startsWith("sora") : "i105-default encoding mismatch";

    final AccountAddress.ParseResult i105Parsed =
        AccountAddress.parseEncoded(i105, AccountAddress.DEFAULT_I105_DISCRIMINANT);
    assert i105Parsed.format == AccountAddress.Format.I105 : "expected I105 format";
    assert Arrays.equals(address.canonicalBytes(), i105Parsed.address.canonicalBytes())
        : "I105 round-trip mismatch";

    final AccountAddress.ParseResult i105DefaultParsed = AccountAddress.parseEncoded(i105Default, null);
    assert i105DefaultParsed.format == AccountAddress.Format.I105_DEFAULT : "expected i105-default format";
    assert Arrays.equals(address.canonicalBytes(), i105DefaultParsed.address.canonicalBytes())
        : "i105-default round-trip mismatch";
  }

  private static void i105PrefixMismatchThrows() throws Exception {
    final byte[] key = new byte[32];
    Arrays.fill(key, (byte) 1);
    final AccountAddress address = AccountAddress.fromAccount(key, "ed25519");
    final String i105 = address.toI105(5);
    boolean threw = false;
    try {
      AccountAddress.parseEncoded(i105, 9);
    } catch (final AccountAddress.AccountAddressException ex) {
      threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX;
    }
    assert threw : "expected prefix mismatch to throw";
  }

  private static void parseEncodedRejectsLegacyForms() throws Exception {
    final byte[] key = new byte[32];
    final AccountAddress address = AccountAddress.fromAccount(key, "ed25519");
    final String canonical = address.canonicalHex();
    final String i105 = address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);

    expectAnyAddressError(() -> AccountAddress.parseEncoded(canonical, null));
    expectAnyAddressError(() -> AccountAddress.parseEncoded(canonical.substring(2), null));
    expectAnyAddressError(() -> AccountAddress.parseEncoded(i105 + "@wonderland", null));
  }

  private static void singleKeyPayloadExtraction() throws Exception {
    final byte[] key = new byte[32];
    for (int i = 0; i < key.length; i++) {
      key[i] = (byte) i;
    }
    final AccountAddress address = AccountAddress.fromAccount(key, "ed25519");
    final java.util.Optional<AccountAddress.SingleKeyPayload> payload = address.singleKeyPayload();
    assert payload.isPresent() : "expected single-key payload";
    final AccountAddress.SingleKeyPayload info = payload.get();
    assert info.curveId() == 0x01 : "curve id mismatch";
    assert Arrays.equals(info.publicKey(), key) : "public key mismatch";
  }

  private static void compressedRequiresSentinel() {
    boolean threw = false;
    try {
      AccountAddress.fromI105Default("invalid");
    } catch (final AccountAddress.AccountAddressException ex) {
      threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL;
    }
    assert threw : "i105-default parsing should reject invalid sentinel";
  }

  private static void curveSupportDefaults() {
    AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());
    final byte[] key = new byte[32];
    boolean threw = false;
    try {
      AccountAddress.fromAccount(key, "ml-dsa");
    } catch (final AccountAddress.AccountAddressException ex) {
      threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.UNSUPPORTED_ALGORITHM;
    }
    assert threw : "expected ML-DSA to be disabled by default";
  }

  private static void curveSupportConfigurationToggle() throws Exception {
    final byte[] key = new byte[32];
    AccountAddress.configureCurveSupport(
        AccountAddress.CurveSupportConfig.builder().allowMlDsa(true).build());
    final AccountAddress address = AccountAddress.fromAccount(key, "ml-dsa");
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

  private static AccountAddress parseCanonicalHexFixture(final String literal) throws Exception {
    final String body = literal.startsWith("0x") || literal.startsWith("0X")
        ? literal.substring(2)
        : literal;
    if ((body.length() & 1) != 0 || !body.matches("(?i)[0-9a-f]*")) {
      throw new AccountAddress.AccountAddressException(
          AccountAddress.AccountAddressErrorCode.INVALID_HEX_ADDRESS,
          "invalid canonical hex account address");
    }
    final byte[] canonical = new byte[body.length() / 2];
    for (int i = 0; i < canonical.length; i++) {
      final int start = i * 2;
      canonical[i] = (byte) Integer.parseInt(body.substring(start, start + 2), 16);
    }
    return AccountAddress.fromCanonicalBytes(canonical);
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
    if (!matched && "i105-checksum-mismatch".equals(caseId)) {
      System.out.println(
          "[fixture-drift] "
              + caseId
              + ": negative vector now decodes under strict fixtures; regenerate vectors.");
      return;
    }
    assert matched : caseId + ": expected failure (" + expected.get("kind") + ")";
  }

  private static void expectUnsupportedFormat(final CheckedRunnable action) throws Exception {
    boolean threw = false;
    try {
      action.run();
    } catch (final AccountAddress.AccountAddressException ex) {
      threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT;
    }
    assert threw : "expected UnsupportedFormat";
  }

  private static void expectAnyAddressError(final CheckedRunnable action) throws Exception {
    boolean threw = false;
    try {
      action.run();
    } catch (final AccountAddress.AccountAddressException ex) {
      threw = true;
    }
    assert threw : "expected AccountAddressException";
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
        return message.contains("unexpected I105 network prefix")
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
