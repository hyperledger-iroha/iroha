package org.hyperledger.iroha.android.address;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.IdentifierJsonParser;
import org.hyperledger.iroha.android.client.IdentifierNormalization;
import org.hyperledger.iroha.android.client.IdentifierPolicySummary;
import org.hyperledger.iroha.android.client.RamLfeJsonParser;
import org.hyperledger.iroha.android.client.RamLfeProgramPolicySummary;
import org.hyperledger.iroha.android.crypto.Ed25519PublicKeyAdmission;
import org.hyperledger.iroha.android.crypto.MlDsaPublicKey;
import org.hyperledger.iroha.android.crypto.MlDsaPublicKeyAdmission;
import org.hyperledger.iroha.android.testing.SimpleJson;
import org.hyperledger.iroha.android.tx.MultisigSignature;
import org.hyperledger.iroha.norito.Varint;

public final class AccountAddressTests {

  private static final String FIXTURE_RELATIVE = "fixtures/account/address_vectors.json";
  private static final String ED25519_ADMISSION_FIXTURE_RELATIVE =
      "fixtures/crypto/ed25519_public_key_admission_v1.json";
  private static final String VALID_ED25519_PUBLIC_KEY_HEX =
      "3B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29";
  private static final String VALID_ED25519_PUBLIC_KEY_LITERAL =
      "ed0120" + VALID_ED25519_PUBLIC_KEY_HEX;

  private AccountAddressTests() {}

  public static void main(final String[] args) throws Exception {
    AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());
    strictEd25519AdmissionFixtureSuite();
    complianceFixtureSuite();
    goldenVectorsRoundTrip();
    mixedI105LiteralRoundTrip();
    selectorPrefixedCanonicalBytesAreRejected();
    i105PrefixMismatchThrows();
    i105RejectsFullwidthSentinel();
    i105RejectsNonCanonicalFullwidthKana();
    i105RejectsInvalidCharacters();
    curveSupportDefaults();
    curveSupportConfigurationToggle();
    protocolMlDsa65AliasesAreExactAndCaseInsensitive();
    mlDsa65KeyShapeAndWireMatrix();
    fullCryptoCurveRegistry();
    curveAlgorithmAliasesRejectBlankAndPaddedLabels();
    curveAlgorithmAliasesRejectControlsAndUnicodeConfusables();
    longGostLabelsAcceptedWhenEnabled();
    singleKeyPayloadExtraction();
    System.out.println("[IrohaAndroid] Account address tests passed.");
  }

  private static void complianceFixtureSuite() throws Exception {
    final Path fixturePath = resolveFixturePath(FIXTURE_RELATIVE);
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

  private static void strictEd25519AdmissionFixtureSuite() throws Exception {
    final Path fixturePath = resolveFixturePath(ED25519_ADMISSION_FIXTURE_RELATIVE);
    final String json = new String(Files.readAllBytes(fixturePath), StandardCharsets.UTF_8);
    final Map<String, Object> root = asMap(SimpleJson.parse(json), "root", "<root>");
    assert asNumber(root.get("schema_version"), "schema_version", "<root>").intValue() == 1;

    final List<?> vectors = asList(root.get("vectors"), "vectors", "<root>");
    for (final Object entry : vectors) {
      validateStrictEd25519AdmissionVector(asMap(entry, "vectors[]", "<vector>"));
    }
    validateOutputOpeningPublicKeyJsonTypes();

    final byte[] secpKey = new byte[33];
    for (int i = 0; i < secpKey.length; i++) {
      secpKey[i] = (byte) (i + 1);
    }
    final String secpLiteral = PublicKeyCodec.encodePublicKeyMultihash(0x04, secpKey);
    final PublicKeyCodec.PublicKeyPayload decodedSecp =
        PublicKeyCodec.decodePublicKeyLiteral("secp256k1:" + secpLiteral);
    assert decodedSecp != null && decodedSecp.curveId() == 0x04
        : "canonical secp256k1 prefix must match its multihash";
    assert PublicKeyCodec.decodePublicKeyLiteral("ed25519:" + secpLiteral) == null
        : "mismatched canonical prefix must be rejected";
  }

  private static void validateStrictEd25519AdmissionVector(final Map<String, Object> vector)
      throws Exception {
    final String name = asString(vector.get("name"), "name", "<vector>");
    final boolean valid = asBoolean(vector.get("valid"), "valid", name);
    final byte[] key = decodeHex(asString(vector.get("key_hex"), "key_hex", name));
    final byte[] canonical =
        decodeHex(asString(vector.get("single_canonical_hex"), "single_canonical_hex", name));
    final boolean hasCanonicalI105 = vector.containsKey("single_i105");
    final boolean hasMalformedI105 = vector.containsKey("malformed_i105");
    assert hasCanonicalI105 ^ hasMalformedI105
        : name + ": exactly one I105 fixture field is required";
    assert !hasMalformedI105 || !valid : name + ": malformed I105 is negative-test-only";
    final String i105 =
        asString(
            vector.get(hasCanonicalI105 ? "single_i105" : "malformed_i105"),
            hasCanonicalI105 ? "single_i105" : "malformed_i105",
            name);

    assert Ed25519PublicKeyAdmission.isValid(key) == valid : name + ": helper result mismatch";

    final String literal = rawEd25519Literal(key);
    final byte[] compact = new byte[1 + key.length];
    System.arraycopy(key, 0, compact, 1, key.length);
    final PublicKeyCodec.PublicKeyPayload decodedLiteral =
        PublicKeyCodec.decodePublicKeyLiteral(literal);
    final PublicKeyCodec.PublicKeyPayload decodedCompact =
        PublicKeyCodec.decodeCompactPublicKeyPayload(compact);

    if (valid) {
      assert literal.equals(PublicKeyCodec.encodePublicKeyMultihash(0x01, key))
          : name + ": multihash encoding mismatch";
      assert Arrays.equals(compact, PublicKeyCodec.compactPublicKeyPayload(0x01, key))
          : name + ": compact encoding mismatch";
      assert PublicKeyCodec.decodePublicKeyLiteral("ed25519:" + literal) != null
          : name + ": canonical algorithm prefix rejected";
      assert PublicKeyCodec.decodePublicKeyLiteral("garbage:" + literal) == null
          : name + ": unknown algorithm prefix accepted";
      assert PublicKeyCodec.decodePublicKeyLiteral("secp256k1:" + literal) == null
          : name + ": mismatched algorithm prefix accepted";
      assert PublicKeyCodec.decodePublicKeyLiteral("ed25519:ed25519:" + literal) == null
          : name + ": multiple algorithm prefixes accepted";
      assert PublicKeyCodec.decodePublicKeyLiteral("ED25519:" + literal) == null
          : name + ": noncanonical algorithm prefix accepted";
      assert PublicKeyCodec.decodePublicKeyLiteral(" " + literal) == null
          : name + ": leading whitespace accepted";
      assert PublicKeyCodec.decodePublicKeyLiteral(literal + " ") == null
          : name + ": trailing whitespace accepted";
      assert PublicKeyCodec.decodePublicKeyLiteral("\u00A0" + literal) == null
          : name + ": leading Unicode whitespace accepted";
      assert Arrays.equals(
          compact,
          MultisigSignature.fromCurveId(0x01, key, new byte[64]).publicKeyNoritoPayload())
          : name + ": multisig signature public-key encoding mismatch";
      assert decodedLiteral != null : name + ": valid multihash key rejected";
      assert decodedCompact != null : name + ": valid compact key rejected";
      assert Arrays.equals(key, decodedLiteral.keyBytes()) : name + ": multihash key mismatch";
      assert Arrays.equals(key, decodedCompact.keyBytes()) : name + ": compact key mismatch";
      assert Arrays.equals(
          canonical, AccountAddress.fromAccount(key, "ed25519").canonicalBytes());
      assert Arrays.equals(canonical, AccountAddress.fromCanonicalBytes(canonical).canonicalBytes());
      assert Arrays.equals(
          canonical,
          AccountAddress.fromI105(i105, AccountAddress.DEFAULT_I105_DISCRIMINANT)
              .canonicalBytes());
      AccountAddress.parseEncodedIgnoringCurveSupport(
          i105, AccountAddress.DEFAULT_I105_DISCRIMINANT);
      AccountAddress.fromMultisigPolicy(multisigPolicy(key));
      AccountAddress.fromCanonicalBytes(multisigCanonical(key));
      identifierPolicy(literal);
      ramLfePolicy(literal);
      IdentifierJsonParser.parsePolicyList(identifierPolicyJson(literal));
      RamLfeJsonParser.parsePolicyList(ramLfePolicyJson(literal));
    } else {
      expectInvalidPublicKeyEncoding(
          name, () -> PublicKeyCodec.encodePublicKeyMultihash(0x01, key));
      expectInvalidPublicKeyEncoding(
          name, () -> PublicKeyCodec.compactPublicKeyPayload(0x01, key));
      expectInvalidPublicKeyEncoding(
          name, () -> MultisigSignature.fromCurveId(0x01, key, new byte[64]));
      assert decodedLiteral == null : name + ": invalid multihash key accepted";
      assert decodedCompact == null : name + ": invalid compact key accepted";
      expectInvalidPublicKey(name, () -> AccountAddress.fromAccount(key, "ed25519"));
      expectInvalidPublicKey(name, () -> AccountAddress.fromCanonicalBytes(canonical));
      final AccountAddress.AccountAddressErrorCode expectedI105Error =
          hasMalformedI105
              ? AccountAddress.AccountAddressErrorCode.CHECKSUM_MISMATCH
              : AccountAddress.AccountAddressErrorCode.INVALID_PUBLIC_KEY;
      expectAddressError(
          name,
          expectedI105Error,
          () -> AccountAddress.fromI105(i105, AccountAddress.DEFAULT_I105_DISCRIMINANT));
      expectAddressError(
          name,
          expectedI105Error,
          () ->
              AccountAddress.parseEncodedIgnoringCurveSupport(
                  i105, AccountAddress.DEFAULT_I105_DISCRIMINANT));
      expectInvalidPublicKey(name, () -> AccountAddress.fromMultisigPolicy(multisigPolicy(key)));
      expectInvalidPublicKey(name, () -> AccountAddress.fromCanonicalBytes(multisigCanonical(key)));
      expectInvalidPublicKeyLiteral(name, () -> identifierPolicy(literal));
      expectInvalidPublicKeyLiteral(name, () -> ramLfePolicy(literal));
      expectInvalidPublicKeyLiteral(
          name, () -> identifierPolicy(VALID_ED25519_PUBLIC_KEY_LITERAL, literal));
      expectInvalidPublicKeyLiteral(
          name, () -> ramLfePolicy(VALID_ED25519_PUBLIC_KEY_LITERAL, literal));
      expectInvalidPolicyJson(
          name, () -> IdentifierJsonParser.parsePolicyList(identifierPolicyJson(literal)));
      expectInvalidPolicyJson(
          name,
          () ->
              IdentifierJsonParser.parsePolicyList(
                  identifierPolicyJson(VALID_ED25519_PUBLIC_KEY_LITERAL, literal)));
      expectInvalidPolicyJson(
          name, () -> RamLfeJsonParser.parsePolicyList(ramLfePolicyJson(literal)));
      expectInvalidPolicyJson(
          name,
          () ->
              RamLfeJsonParser.parsePolicyList(
                  ramLfePolicyJson(VALID_ED25519_PUBLIC_KEY_LITERAL, literal)));
    }
  }

  private static AccountAddress.MultisigPolicyPayload multisigPolicy(final byte[] publicKey) {
    return AccountAddress.MultisigPolicyPayload.of(
        1,
        1,
        Collections.singletonList(AccountAddress.MultisigMemberPayload.of(0x01, 1, publicKey)));
  }

  private static byte[] multisigCanonical(final byte[] publicKey) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(0x0A);
    out.write(0x01);
    out.write(0x01);
    out.write(0x00);
    out.write(0x01);
    out.write(0x00);
    out.write(0x01);
    out.write(0x01);
    out.write(0x00);
    out.write(0x01);
    out.write((publicKey.length >>> 8) & 0xFF);
    out.write(publicKey.length & 0xFF);
    out.write(publicKey, 0, publicKey.length);
    return out.toByteArray();
  }

  private static IdentifierPolicySummary identifierPolicy(final String publicKeyLiteral) {
    return identifierPolicy(publicKeyLiteral, publicKeyLiteral);
  }

  private static IdentifierPolicySummary identifierPolicy(
      final String resolverPublicKeyLiteral, final String outputOpeningPublicKeyLiteral) {
    return new IdentifierPolicySummary(
        "key-admission#fixture",
        "key_admission_fixture",
        "owner",
        true,
        IdentifierNormalization.EXACT,
        resolverPublicKeyLiteral,
        outputOpeningPublicKeyLiteral,
        "signed",
        null,
        null,
        null,
        null,
        null);
  }

  private static RamLfeProgramPolicySummary ramLfePolicy(final String publicKeyLiteral) {
    return ramLfePolicy(publicKeyLiteral, publicKeyLiteral);
  }

  private static RamLfeProgramPolicySummary ramLfePolicy(
      final String resolverPublicKeyLiteral, final String outputOpeningPublicKeyLiteral) {
    return new RamLfeProgramPolicySummary(
        "key_admission_fixture",
        "owner",
        true,
        resolverPublicKeyLiteral,
        outputOpeningPublicKeyLiteral,
        "signed",
        "signed",
        null,
        null,
        null,
        null,
        null);
  }

  private static byte[] identifierPolicyJson(final String publicKeyLiteral) {
    return identifierPolicyJson(publicKeyLiteral, publicKeyLiteral);
  }

  private static byte[] identifierPolicyJson(
      final String resolverPublicKeyLiteral, final String outputOpeningPublicKeyLiteral) {
    return identifierPolicyJsonWithOutputValue(
        resolverPublicKeyLiteral, "\"" + outputOpeningPublicKeyLiteral + "\"");
  }

  private static byte[] identifierPolicyJsonWithOutputValue(
      final String resolverPublicKeyLiteral, final String outputOpeningPublicKeyJson) {
    return ("{\"items\":[{\"policy_id\":\"key-admission#fixture\",\"owner\":\"owner\","
            + "\"active\":true,\"normalization\":\"exact\",\"resolver_public_key\":\""
            + resolverPublicKeyLiteral
            + "\",\"output_opening_public_key\":"
            + outputOpeningPublicKeyJson
            + ",\"backend\":\"signed\"}]}")
        .getBytes(StandardCharsets.UTF_8);
  }

  private static byte[] ramLfePolicyJson(final String publicKeyLiteral) {
    return ramLfePolicyJson(publicKeyLiteral, publicKeyLiteral);
  }

  private static byte[] ramLfePolicyJson(
      final String resolverPublicKeyLiteral, final String outputOpeningPublicKeyLiteral) {
    return ramLfePolicyJsonWithOutputValue(
        resolverPublicKeyLiteral, "\"" + outputOpeningPublicKeyLiteral + "\"");
  }

  private static byte[] ramLfePolicyJsonWithOutputValue(
      final String resolverPublicKeyLiteral, final String outputOpeningPublicKeyJson) {
    return ("{\"items\":[{\"program_id\":\"key_admission_fixture\",\"owner\":\"owner\","
            + "\"active\":true,\"resolver_public_key\":\""
            + resolverPublicKeyLiteral
            + "\",\"output_opening_public_key\":"
            + outputOpeningPublicKeyJson
            + ",\"backend\":\"signed\",\"verification_mode\":\"signed\"}]}")
        .getBytes(StandardCharsets.UTF_8);
  }

  private static void validateOutputOpeningPublicKeyJsonTypes() throws Exception {
    final String identifierWithoutOutput =
        new String(
                identifierPolicyJson(VALID_ED25519_PUBLIC_KEY_LITERAL), StandardCharsets.UTF_8)
            .replace(
                ",\"output_opening_public_key\":\""
                    + VALID_ED25519_PUBLIC_KEY_LITERAL
                    + "\"",
                "");
    final IdentifierPolicySummary identifierPolicy =
        IdentifierJsonParser.parsePolicyList(
                identifierWithoutOutput.getBytes(StandardCharsets.UTF_8))
            .items()
            .get(0);
    assert VALID_ED25519_PUBLIC_KEY_LITERAL.equals(identifierPolicy.outputOpeningPublicKey())
        : "missing identifier output-opening key must inherit the resolver key";

    final String ramLfeWithoutOutput =
        new String(ramLfePolicyJson(VALID_ED25519_PUBLIC_KEY_LITERAL), StandardCharsets.UTF_8)
            .replace(
                ",\"output_opening_public_key\":\""
                    + VALID_ED25519_PUBLIC_KEY_LITERAL
                    + "\"",
                "");
    final RamLfeProgramPolicySummary ramLfePolicy =
        RamLfeJsonParser.parsePolicyList(ramLfeWithoutOutput.getBytes(StandardCharsets.UTF_8))
            .items()
            .get(0);
    assert VALID_ED25519_PUBLIC_KEY_LITERAL.equals(ramLfePolicy.outputOpeningPublicKey())
        : "missing RAM-LFE output-opening key must inherit the resolver key";

    for (final String invalidJsonValue : new String[] {"null", "true"}) {
      expectInvalidOutputOpeningPolicyJson(
          "identifier output opening " + invalidJsonValue,
          () ->
              IdentifierJsonParser.parsePolicyList(
                  identifierPolicyJsonWithOutputValue(
                      VALID_ED25519_PUBLIC_KEY_LITERAL, invalidJsonValue)));
      expectInvalidOutputOpeningPolicyJson(
          "RAM-LFE output opening " + invalidJsonValue,
          () ->
              RamLfeJsonParser.parsePolicyList(
                  ramLfePolicyJsonWithOutputValue(
                      VALID_ED25519_PUBLIC_KEY_LITERAL, invalidJsonValue)));
    }
  }

  private static void expectInvalidPublicKey(final String name, final CheckedRunnable action)
      throws Exception {
    expectAddressError(name, AccountAddress.AccountAddressErrorCode.INVALID_PUBLIC_KEY, action);
  }

  private static void expectAddressError(
      final String name,
      final AccountAddress.AccountAddressErrorCode expected,
      final CheckedRunnable action)
      throws Exception {
    try {
      action.run();
    } catch (final AccountAddress.AccountAddressException ex) {
      assert ex.getCode() == expected
          : name + ": unexpected error " + ex.getCode();
      return;
    }
    throw new AssertionError(name + ": expected account-address error " + expected);
  }

  private static void expectInvalidPublicKeyEncoding(
      final String name, final CheckedRunnable action) throws Exception {
    try {
      action.run();
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("invalid Ed25519 public key")
          : name + ": unexpected encoding error " + ex.getMessage();
      return;
    }
    throw new AssertionError(name + ": invalid Ed25519 public key was encoded");
  }

  private static void expectInvalidMlDsaKey(
      final String name, final CheckedRunnable action) throws Exception {
    try {
      action.run();
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("ML-DSA-65")
          : name + ": unexpected ML-DSA key error " + ex.getMessage();
      return;
    }
    throw new AssertionError(name + ": invalid ML-DSA-65 public key was accepted");
  }

  private static void expectInvalidPublicKeyLiteral(
      final String name, final CheckedRunnable action) throws Exception {
    try {
      action.run();
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("not a valid public key literal")
          : name + ": unexpected policy error " + ex.getMessage();
      return;
    }
    throw new AssertionError(name + ": invalid Ed25519 policy key was accepted");
  }

  private static void expectInvalidPolicyJson(
      final String name, final CheckedRunnable action) throws Exception {
    try {
      action.run();
    } catch (final IllegalStateException ex) {
      assert ex.getMessage().contains("must be a valid public key literal")
          : name + ": unexpected policy JSON error " + ex.getMessage();
      return;
    }
    throw new AssertionError(name + ": invalid Ed25519 policy JSON key was accepted");
  }

  private static void expectInvalidOutputOpeningPolicyJson(
      final String name, final CheckedRunnable action) throws Exception {
    try {
      action.run();
    } catch (final IllegalStateException ex) {
      assert ex.getMessage().contains("output_opening_public_key")
          : name + ": unexpected policy JSON error " + ex.getMessage();
      return;
    }
    throw new AssertionError(name + ": invalid output-opening policy JSON key was accepted");
  }

  private static void validatePositiveCase(final Map<String, Object> vector) throws Exception {
    final String caseId = asString(vector.get("case_id"), "case_id", "<positive>");

    final Map<String, Object> encodings = asMap(vector.get("encodings"), caseId + ".encodings", caseId);
    final String canonicalHex = asString(encodings.get("canonical_hex"), caseId + ".encodings.canonical_hex", caseId);
    final Map<String, Object> i105 = asMap(encodings.get("i105"), caseId + ".encodings.i105", caseId);
    final int prefix = asNumber(i105.get("prefix"), caseId + ".encodings.i105.prefix", caseId).intValue();
    final String i105String = asString(i105.get("string"), caseId + ".encodings.i105.string", caseId);

    final AccountAddress i105Parsed = AccountAddress.parseEncoded(i105String, prefix);
    final byte[] canonicalBytes = i105Parsed.canonicalBytes();
    assert i105Parsed.canonicalHex().equalsIgnoreCase(canonicalHex)
        : caseId + ": canonical hex mismatch";

    final String reencodedI105 = i105Parsed.toI105(prefix);
    assert reencodedI105.equals(i105String) : caseId + ": i105 re-encode mismatch";
    assert i105Parsed.canonicalHex().equalsIgnoreCase(canonicalHex)
        : caseId + ": canonical hex re-encode mismatch";

    final AccountAddress.DisplayFormats formats = i105Parsed.displayFormats(prefix);
    assert formats.i105.equals(i105String) : caseId + ": displayFormats i105 mismatch";
    assert formats.discriminant == prefix : caseId + ": displayFormats discriminant mismatch";
    assert formats.i105Warning.equals(AccountAddress.i105WarningMessage())
        : caseId + ": displayFormats warning mismatch";

    final AccountAddress.DisplayFormats defaultFormats = i105Parsed.displayFormats();
    assert defaultFormats.discriminant == AccountAddress.DEFAULT_I105_DISCRIMINANT
        : caseId + ": default displayFormats discriminant mismatch";
    assert defaultFormats.i105.startsWith("sora")
        : caseId + ": default displayFormats should render the Sora sentinel";
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
      case "canonical_hex":
        expectError(caseId, expected, () -> AccountAddress.parseEncoded(input, null));
        break;
      default:
        throw new IllegalStateException(caseId + ": unsupported negative format " + format);
    }
  }

  private static void goldenVectorsRoundTrip() throws Exception {
    final byte[] key = validEd25519Key();
    final AccountAddress address = AccountAddress.fromAccount(key, "ed25519");

    final String canonical = address.canonicalHex();
    final String i105 = address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);

    assert canonical.equals("0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29")
        : "canonical encoding mismatch";
    assert i105.equals("sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE")
        : "i105 encoding mismatch";

    final AccountAddress i105Parsed =
        AccountAddress.parseEncoded(i105, AccountAddress.DEFAULT_I105_DISCRIMINANT);
    assert Arrays.equals(address.canonicalBytes(), i105Parsed.canonicalBytes())
        : "i105 round-trip mismatch";
  }

  private static void mixedI105LiteralRoundTrip() throws Exception {
    final String literal =
        "sorauﾛ1PﾜdﾎｼﾋﾉNｸdﾁﾑkiﾇ3ｵﾓaPBQDTｲKqｼqｵrﾗｶwSQ1ﾌﾅQU61Y7";
    final AccountAddress address =
        AccountAddress.fromI105(literal, AccountAddress.DEFAULT_I105_DISCRIMINANT);
    assert address.canonicalHex().equals(
            "0x02000120bc717326224e4b4119298e7b1db8133cb27d6cdf6b3e04d75a6d27b29a34c1cf")
        : "ambiguous literal canonical mismatch";
    assert address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT).equals(literal)
        : "ambiguous literal round-trip mismatch";
  }

  private static void selectorPrefixedCanonicalBytesAreRejected() throws Exception {
    final byte[] canonical =
        AccountAddress.fromAccount(validEd25519Key(), "ed25519").canonicalBytes();
    final byte[] selectorPrefixed = new byte[canonical.length + 13];
    selectorPrefixed[0] = canonical[0];
    selectorPrefixed[1] = 0x01;
    for (int i = 0; i < 12; i++) {
      selectorPrefixed[i + 2] = (byte) (i + 1);
    }
    System.arraycopy(canonical, 1, selectorPrefixed, 14, canonical.length - 1);

    try {
      AccountAddress.fromCanonicalBytes(selectorPrefixed);
    } catch (final AccountAddress.AccountAddressException expected) {
      return;
    }
    throw new AssertionError("retired domain-selector prefix was accepted");
  }

  private static void i105PrefixMismatchThrows() throws Exception {
    final byte[] key = validEd25519Key();
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

  private static void i105RejectsFullwidthSentinel() throws Exception {
    final byte[] key = validEd25519Key();
    final AccountAddress address = AccountAddress.fromAccount(key, "ed25519");
    final String canonical = address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final String noncanonical = canonical.replaceFirst("sora", "ｓｏｒａ");

    boolean parseThrew = false;
    try {
      AccountAddress.parseEncoded(noncanonical, AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      parseThrew = ex.getCode() == AccountAddress.AccountAddressErrorCode.MISSING_I105_SENTINEL;
    }
    assert parseThrew : "fullwidth sentinel literal must be rejected";

    boolean fromI105Threw = false;
    try {
      AccountAddress.fromI105(noncanonical, AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      fromI105Threw =
          ex.getCode() == AccountAddress.AccountAddressErrorCode.MISSING_I105_SENTINEL;
    }
    assert fromI105Threw : "fullwidth sentinel literal must be rejected by fromI105";
  }

  private static void i105RejectsNonCanonicalFullwidthKana() throws Exception {
    final String canonical =
        "sorauﾛ1PﾜdﾎｼﾋﾉNｸdﾁﾑkiﾇ3ｵﾓaPBQDTｲKqｼqｵrﾗｶwSQ1ﾌﾅQU61Y7";
    final String nonCanonical = canonical.replaceFirst("ﾛ", "ロ");
    boolean threw = false;
    try {
      AccountAddress.fromI105(nonCanonical, AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.INVALID_I105_CHAR;
    }
    assert threw : "non-canonical fullwidth kana literal must be rejected";
  }

  private static void singleKeyPayloadExtraction() throws Exception {
    final byte[] key = validEd25519Key();
    final AccountAddress address = AccountAddress.fromAccount(key, "ed25519");
    final java.util.Optional<AccountAddress.SingleKeyPayload> payload = address.singleKeyPayload();
    assert payload.isPresent() : "expected single-key payload";
    final AccountAddress.SingleKeyPayload info = payload.get();
    assert info.curveId() == 0x01 : "curve id mismatch";
    assert Arrays.equals(info.publicKey(), key) : "public key mismatch";
  }

  private static void i105RejectsInvalidCharacters() {
    final byte[] key = validEd25519Key();
    final String literal;
    try {
      literal =
          AccountAddress.fromAccount(key, "ed25519")
              .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new AssertionError("failed to build canonical I105 literal", ex);
    }
    final String malformed = literal.substring(0, literal.length() - 1) + "!";
    boolean threw = false;
    try {
      AccountAddress.fromI105(malformed, null);
    } catch (final AccountAddress.AccountAddressException ex) {
      threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.INVALID_I105_CHAR;
    }
    assert threw : "i105 parsing should reject invalid non-i105 symbols";
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
    final byte[] key = nonzeroMlDsaKey(MlDsaPublicKeyAdmission.PUBLIC_KEY_LENGTH);
    AccountAddress.configureCurveSupport(
        AccountAddress.CurveSupportConfig.builder().allowMlDsa(true).build());
    final AccountAddress address = AccountAddress.fromAccount(key, "ml-dsa");
    final AccountAddress roundTripped = AccountAddress.fromI105(address.toI105(1), 1);
    assert Arrays.equals(address.canonicalBytes(), roundTripped.canonicalBytes())
        : "ML-DSA enablement round-trip mismatch";
    AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());
  }

  private static void protocolMlDsa65AliasesAreExactAndCaseInsensitive() throws Exception {
    final byte[] key = nonzeroMlDsaKey(MlDsaPublicKeyAdmission.PUBLIC_KEY_LENGTH);
    AccountAddress.configureCurveSupport(
        AccountAddress.CurveSupportConfig.builder().allowMlDsa(true).build());
    try {
      for (final String algorithm :
          new String[] {
            "ml-dsa",
            "mldsa",
            "ml_dsa",
            "mldsa65",
            "MLDSA65",
            "ml-dsa-65",
            "ML-DSA-65",
            "ml_dsa_65",
            "ML_DSA_65",
            "ml_dsa-65",
            "ML_DSA-65",
          }) {
        final AccountAddress address = AccountAddress.fromAccount(key, algorithm);
        assert address
                .singleKeyPayload()
                .orElseThrow(() -> new IllegalStateException("single-key payload missing"))
                .curveId()
            == 0x02
            : "expected ML-DSA curve for " + algorithm;
      }

      for (final String algorithm :
          new String[] {
            "mldsa44",
            "ml-dsa-44",
            "ml_dsa_44",
            "ml_dsa-44",
            "mldsa87",
            "ml-dsa-87",
            "ml_dsa_87",
            "ml_dsa-87",
            "ml-dsa-\uFF16\uFF15",
            "ml\uFF0Ddsa-65",
          }) {
        boolean threw = false;
        try {
          AccountAddress.fromAccount(key, algorithm);
        } catch (final AccountAddress.AccountAddressException ex) {
          threw =
              ex.getCode() == AccountAddress.AccountAddressErrorCode.UNSUPPORTED_ALGORITHM;
        }
        assert threw : "expected unsupported ML-DSA alias for " + algorithm;
      }
    } finally {
      AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());
    }
  }

  private static void mlDsa65KeyShapeAndWireMatrix() throws Exception {
    final byte[] valid = nonzeroMlDsaKey(MlDsaPublicKeyAdmission.PUBLIC_KEY_LENGTH);
    assert MlDsaPublicKeyAdmission.isValid(valid) : "valid ML-DSA-65 key rejected";
    assert Arrays.equals(valid, new MlDsaPublicKey(valid).getEncoded())
        : "ML-DSA public-key wrapper changed valid key bytes";

    AccountAddress.configureCurveSupport(
        AccountAddress.CurveSupportConfig.builder().allowMlDsa(true).build());
    try {
      final AccountAddress address = AccountAddress.fromAccount(valid, "ml-dsa-65");
      final byte[] canonical = address.canonicalBytes();
      assert (canonical[0] & 0xFF) == 0x02
          : "single-key construction must emit the canonical class-0 header";
      assert (canonical[1] & 0xFF) == 0x02
          : "ML-DSA-65 single-key address must use the extended controller tag";
      assert (canonical[2] & 0xFF) == 0x02 : "ML-DSA-65 curve id mismatch";
      assert (((canonical[3] & 0xFF) << 8) | (canonical[4] & 0xFF)) == valid.length
          : "ML-DSA-65 extended length mismatch";
      final AccountAddress canonicalRoundTrip = AccountAddress.fromCanonicalBytes(canonical);
      assert Arrays.equals(canonical, canonicalRoundTrip.canonicalBytes())
          : "extended canonical address did not round-trip";
      assert Arrays.equals(
              valid,
              canonicalRoundTrip
                  .singleKeyPayload()
                  .orElseThrow(() -> new IllegalStateException("single-key payload missing"))
                  .publicKey())
          : "extended canonical address changed ML-DSA-65 key bytes";
      final byte[] mismatchedExtendedClass = Arrays.copyOf(canonical, canonical.length);
      mismatchedExtendedClass[0] = 0x0A;
      expectAddressError(
          "extended single-key controller under multisig address class",
          AccountAddress.AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
          () -> AccountAddress.fromCanonicalBytes(mismatchedExtendedClass));

      final byte[] shortSingleCanonical =
          AccountAddress.fromAccount(validEd25519Key(), "ed25519").canonicalBytes();
      shortSingleCanonical[0] = 0x0A;
      expectAddressError(
          "short single-key controller under multisig address class",
          AccountAddress.AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
          () -> AccountAddress.fromCanonicalBytes(shortSingleCanonical));

      final String i105 = address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
      final AccountAddress i105RoundTrip =
          AccountAddress.fromI105(i105, AccountAddress.DEFAULT_I105_DISCRIMINANT);
      assert Arrays.equals(canonical, i105RoundTrip.canonicalBytes())
          : "extended ML-DSA-65 I105 address did not round-trip";
      assert i105.equals(i105RoundTrip.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT))
          : "extended ML-DSA-65 I105 address was not canonical";

      final String literal = PublicKeyCodec.encodePublicKeyMultihash(0x02, valid);
      final PublicKeyCodec.PublicKeyPayload literalPayload =
          Objects.requireNonNull(PublicKeyCodec.decodePublicKeyLiteral("ml-dsa:" + literal));
      assert literalPayload.curveId() == 0x02;
      assert Arrays.equals(valid, literalPayload.keyBytes());
      final byte[] compact = PublicKeyCodec.compactPublicKeyPayload(0x02, valid);
      final PublicKeyCodec.PublicKeyPayload compactPayload =
          Objects.requireNonNull(PublicKeyCodec.decodeCompactPublicKeyPayload(compact));
      assert compact[0] == 4 : "ML-DSA compact algorithm tag mismatch";
      assert Arrays.equals(valid, compactPayload.keyBytes());

      final MultisigSignature directSignature =
          MultisigSignature.fromCurveId(0x02, valid, new byte[] {0x01});
      final MultisigSignature literalSignature =
          MultisigSignature.fromPublicKeyLiteral(literal, new byte[] {0x02});
      assert Arrays.equals(valid, directSignature.publicKey());
      assert Arrays.equals(valid, literalSignature.publicKey());

      final AccountAddress multisig =
          AccountAddress.fromMultisigPolicy(mlDsaMultisigPolicy(valid));
      assert (multisig.canonicalBytes()[0] & 0xFF) == 0x0A
          : "multisig construction must emit the canonical class-1 header";
      final AccountAddress decodedMultisig =
          AccountAddress.fromCanonicalBytes(multisig.canonicalBytes());
      assert Arrays.equals(multisig.canonicalBytes(), decodedMultisig.canonicalBytes())
          : "ML-DSA multisig policy did not round-trip";
      assert Arrays.equals(
              valid,
              decodedMultisig
                  .multisigPolicyPayload()
                  .orElseThrow(() -> new IllegalStateException("multisig payload missing"))
                  .members()
                  .get(0)
                  .publicKey())
          : "ML-DSA multisig policy changed member key bytes";
      final byte[] mismatchedMultisigClass = multisig.canonicalBytes();
      mismatchedMultisigClass[0] = 0x02;
      expectAddressError(
          "multisig controller under single-key address class",
          AccountAddress.AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
          () -> AccountAddress.fromCanonicalBytes(mismatchedMultisigClass));

      final String[] invalidNames = {
        "empty", "32", "1312", "1951", "1953", "2592", "all-zero"
      };
      final byte[][] invalidKeys = {
        new byte[0],
        nonzeroMlDsaKey(32),
        nonzeroMlDsaKey(1312),
        nonzeroMlDsaKey(1951),
        nonzeroMlDsaKey(1953),
        nonzeroMlDsaKey(2592),
        new byte[MlDsaPublicKeyAdmission.PUBLIC_KEY_LENGTH],
      };
      for (int index = 0; index < invalidKeys.length; index++) {
        final String name = "ML-DSA-65 " + invalidNames[index];
        final byte[] invalid = invalidKeys[index];
        assert !MlDsaPublicKeyAdmission.isValid(invalid) : name + ": validator accepted key";
        expectInvalidMlDsaKey(name + " wrapper", () -> new MlDsaPublicKey(invalid));
        expectInvalidPublicKey(name + " address", () -> AccountAddress.fromAccount(invalid, "ml-dsa"));
        expectInvalidPublicKey(
            name + " multisig construction",
            () -> AccountAddress.fromMultisigPolicy(mlDsaMultisigPolicy(invalid)));

        final int canonicalTag = invalid.length <= 0xFF ? 0x00 : 0x02;
        expectInvalidPublicKey(
            name + " canonical decode",
            () -> AccountAddress.fromCanonicalBytes(singleMlDsaCanonical(canonicalTag, invalid)));
        expectInvalidPublicKey(
            name + " multisig decode",
            () -> AccountAddress.fromCanonicalBytes(multisigMlDsaCanonical(invalid)));

        expectInvalidMlDsaKey(
            name + " literal encode",
            () -> PublicKeyCodec.encodePublicKeyMultihash(0x02, invalid));
        expectInvalidMlDsaKey(
            name + " compact encode",
            () -> PublicKeyCodec.compactPublicKeyPayload(0x02, invalid));
        assert PublicKeyCodec.decodePublicKeyLiteral(rawMlDsaLiteral(invalid)) == null
            : name + ": literal decoder accepted key";
        assert PublicKeyCodec.decodeCompactPublicKeyPayload(rawMlDsaCompact(invalid)) == null
            : name + ": compact decoder accepted key";
        expectInvalidMlDsaKey(
            name + " multisig signature",
            () -> MultisigSignature.fromCurveId(0x02, invalid, new byte[] {0x01}));
      }

      for (final int shortExtendedLength : new int[] {0, 32, 255}) {
        expectAddressError(
            "short extended ML-DSA key length " + shortExtendedLength,
            AccountAddress.AccountAddressErrorCode.INVALID_LENGTH,
            () ->
                AccountAddress.fromCanonicalBytes(
                    singleMlDsaCanonical(
                        0x02, nonzeroMlDsaKey(shortExtendedLength))));
      }
      expectInvalidPublicKey(
          "long ML-DSA key under short controller tag",
          () -> AccountAddress.fromCanonicalBytes(singleMlDsaCanonical(0x00, valid)));
    } finally {
      AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());
    }
  }

  private static void fullCryptoCurveRegistry() throws Exception {
    assert "secp256k1".equals(PublicKeyCodec.algorithmForCurveId(0x04));
    assert "bls_normal".equals(PublicKeyCodec.algorithmForCurveId(0x03));
    assert "bls_small".equals(PublicKeyCodec.algorithmForCurveId(0x05));

    final byte[] secpKey = new byte[33];
    Arrays.fill(secpKey, (byte) 0x02);
    final AccountAddress secpAddress = AccountAddress.fromAccount(secpKey, "secp256k1");
    assert secpAddress.singleKeyPayload().orElseThrow().curveId() == 0x04;

    final byte[] blsKey = new byte[48];
    Arrays.fill(blsKey, (byte) 0x03);
    boolean threw = false;
    try {
      AccountAddress.fromAccount(blsKey, "bls_normal");
    } catch (final AccountAddress.AccountAddressException ex) {
      threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.UNSUPPORTED_ALGORITHM;
    }
    assert threw : "expected BLS to be disabled by default";

    AccountAddress.configureCurveSupport(
        AccountAddress.CurveSupportConfig.builder().allowBls(true).build());
    final AccountAddress blsAddress = AccountAddress.fromAccount(blsKey, "bls-normal");
    assert blsAddress.singleKeyPayload().orElseThrow().curveId() == 0x03;
    AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());

    final String encoded = PublicKeyCodec.encodePublicKeyMultihash(0x04, secpKey);
    final PublicKeyCodec.PublicKeyPayload decoded =
        Objects.requireNonNull(PublicKeyCodec.decodePublicKeyLiteral(encoded));
    assert decoded.curveId() == 0x04;
    assert Arrays.equals(secpKey, decoded.keyBytes());

    final byte[] compact = PublicKeyCodec.compactPublicKeyPayload(0x04, secpKey);
    assert compact[0] == 1;
    final PublicKeyCodec.PublicKeyPayload decodedCompact =
        Objects.requireNonNull(PublicKeyCodec.decodeCompactPublicKeyPayload(compact));
    assert decodedCompact.curveId() == 0x04;
    assert Arrays.equals(secpKey, decodedCompact.keyBytes());
  }

  private static void curveAlgorithmAliasesRejectControlsAndUnicodeConfusables() {
    final byte[] key = new byte[32];
    Arrays.fill(key, (byte) 0x11);
    for (final String algorithm :
        new String[] {
          "future-curve",
          "ed\t25519",
          "ed\u200B25519",
          "\u0435d25519",
          "ml\uFF0Ddsa",
          "gost256\u0430",
        }) {
      boolean threw = false;
      try {
        AccountAddress.fromAccount(key, algorithm);
      } catch (final AccountAddress.AccountAddressException ex) {
        threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.UNSUPPORTED_ALGORITHM;
      }
      assert threw : "expected unsupported curve algorithm for " + algorithm;
    }
  }

  private static void curveAlgorithmAliasesRejectBlankAndPaddedLabels() {
    final byte[] key = new byte[32];
    Arrays.fill(key, (byte) 0x11);
    for (final String algorithm :
        new String[] {
          "",
          " ",
          " ed25519",
          "ed25519 ",
          "\ted25519",
          "ed25519\n",
          "\u00A0ed25519",
          "ed25519\u00A0",
        }) {
      boolean threw = false;
      try {
        AccountAddress.fromAccount(key, algorithm);
      } catch (final AccountAddress.AccountAddressException ex) {
        threw = ex.getCode() == AccountAddress.AccountAddressErrorCode.UNSUPPORTED_ALGORITHM;
      }
      assert threw : "expected unsupported curve algorithm for " + algorithm;
    }
  }

  private static void longGostLabelsAcceptedWhenEnabled() throws Exception {
    final byte[] key = new byte[64];
    Arrays.fill(key, (byte) 0x0A);
    AccountAddress.configureCurveSupport(
        AccountAddress.CurveSupportConfig.builder().allowGost(true).build());
    final AccountAddress address =
        AccountAddress.fromAccount(key, "gost3410-2012-256-paramset-a");
    assert address.singleKeyPayload().orElseThrow().curveId() == 0x0A;
    AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());
  }

  private static Path resolveFixturePath(final String fixtureRelative) {
    final Path relative = Path.of(fixtureRelative);
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
    throw new IllegalStateException("Unable to locate fixture at or above: " + fixtureRelative);
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

  private static boolean asBoolean(final Object value, final String field, final String context) {
    if (!(value instanceof Boolean)) {
      throw new IllegalStateException(context + ": expected boolean for " + field);
    }
    return ((Boolean) value).booleanValue();
  }

  private static byte[] validEd25519Key() {
    return decodeHex(VALID_ED25519_PUBLIC_KEY_HEX);
  }

  private static byte[] nonzeroMlDsaKey(final int length) {
    final byte[] key = new byte[length];
    Arrays.fill(key, (byte) 0x5A);
    return key;
  }

  private static AccountAddress.MultisigPolicyPayload mlDsaMultisigPolicy(
      final byte[] publicKey) {
    return AccountAddress.MultisigPolicyPayload.of(
        1,
        1,
        Collections.singletonList(
            AccountAddress.MultisigMemberPayload.of(0x02, 1, publicKey)));
  }

  private static byte[] singleMlDsaCanonical(final int tag, final byte[] publicKey) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(0x02);
    out.write(tag);
    out.write(0x02);
    if (tag == 0x02) {
      out.write((publicKey.length >>> 8) & 0xFF);
      out.write(publicKey.length & 0xFF);
    } else if (tag == 0x00) {
      out.write(publicKey.length & 0xFF);
    } else {
      throw new IllegalArgumentException("unsupported single-key controller tag");
    }
    out.write(publicKey, 0, publicKey.length);
    return out.toByteArray();
  }

  private static byte[] multisigMlDsaCanonical(final byte[] publicKey) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(0x0A);
    out.write(0x01);
    out.write(0x01);
    out.write(0x00);
    out.write(0x01);
    out.write(0x00);
    out.write(0x01);
    out.write(0x02);
    out.write(0x00);
    out.write(0x01);
    out.write((publicKey.length >>> 8) & 0xFF);
    out.write(publicKey.length & 0xFF);
    out.write(publicKey, 0, publicKey.length);
    return out.toByteArray();
  }

  private static byte[] rawMlDsaCompact(final byte[] publicKey) {
    final byte[] compact = new byte[1 + publicKey.length];
    compact[0] = 4;
    System.arraycopy(publicKey, 0, compact, 1, publicKey.length);
    return compact;
  }

  private static String rawMlDsaLiteral(final byte[] publicKey) {
    final byte[] code = Varint.encode(0xEEL);
    final byte[] length = Varint.encode(publicKey.length);
    final StringBuilder literal =
        new StringBuilder((code.length + length.length + publicKey.length) * 2);
    appendHex(literal, code);
    appendHex(literal, length);
    appendHex(literal, publicKey);
    return literal.toString();
  }

  private static void appendHex(final StringBuilder output, final byte[] bytes) {
    final char[] digits = "0123456789abcdef".toCharArray();
    for (final byte value : bytes) {
      output.append(digits[(value >>> 4) & 0x0F]);
      output.append(digits[value & 0x0F]);
    }
  }

  private static byte[] decodeHex(final String hex) {
    if ((hex.length() & 1) != 0) {
      throw new IllegalArgumentException("hex string must have even length");
    }
    final byte[] bytes = new byte[hex.length() / 2];
    for (int i = 0; i < bytes.length; i++) {
      bytes[i] = (byte) Integer.parseInt(hex.substring(i * 2, i * 2 + 2), 16);
    }
    return bytes;
  }

  private static String rawEd25519Literal(final byte[] key) {
    final StringBuilder result = new StringBuilder(6 + key.length * 2);
    result.append("ed01").append(String.format("%02x", key.length));
    for (final byte value : key) {
      result.append(String.format("%02X", value & 0xff));
    }
    return result.toString();
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
        return message.contains("unexpected i105 discriminant")
            && (expectedPrefix == null || message.contains(expectedPrefix.toString()))
            && (foundPrefix == null || message.contains(foundPrefix.toString()));
      case "InvalidI105Char":
        final Object invalidChar = expected.get("char");
        return message.contains("invalid i105 alphabet symbol")
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
