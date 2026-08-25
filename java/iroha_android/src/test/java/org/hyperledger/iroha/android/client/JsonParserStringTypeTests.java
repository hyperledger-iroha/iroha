package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.junit.Test;

/** Regression tests for strict JSON string wire types. */
public final class JsonParserStringTypeTests {
  private static final String VALID_PUBLIC_KEY =
      "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29";
  private static final String RECEIPT_CURSOR = repeated('A', 114);

  @Test
  public void identifierParserRejectsNonStringRequiredAndOptionalFields() {
    final String canonical = identifierPolicyJson();
    assertEquals(
        "retail policy",
        IdentifierJsonParser.parsePolicyList(bytes(canonical)).items().get(0).note());

    assertRejects(
        "identifier policy list.items[0].policy_id",
        () ->
            IdentifierJsonParser.parsePolicyList(
                bytes(canonical.replace("\"policy_id\":\"phone#retail\"", "\"policy_id\":7"))));
    assertRejects(
        "identifier policy list.items[0].program_id",
        () ->
            IdentifierJsonParser.parsePolicyList(
                bytes(
                    canonical.replace(
                        "\"policy_id\":\"phone#retail\",",
                        "\"policy_id\":\"phone#retail\",\"program_id\":7,"))));
    assertRejects(
        "identifier policy list.items[0].note",
        () ->
            IdentifierJsonParser.parsePolicyList(
                bytes(canonical.replace("\"note\":\"retail policy\"", "\"note\":false"))));
  }

  @Test
  public void identifierParserRejectsMissingOrMalformedActive() {
    final String canonical = identifierPolicyJson();
    assertFalse(
        IdentifierJsonParser.parsePolicyList(
                bytes(canonical.replace("\"active\":true", "\"active\":false")))
            .items()
            .get(0)
            .active());

    assertRejects(
        "identifier policy list.items[0].active",
        () ->
            IdentifierJsonParser.parsePolicyList(
                bytes(canonical.replace("\"active\":true", "\"active\":\"true\""))));
    assertRejects(
        "identifier policy list.items[0].active",
        () ->
            IdentifierJsonParser.parsePolicyList(
                bytes(canonical.replace("\"active\":true,", ""))));
  }

  @Test
  public void ramLfeParserRejectsNonStringRequiredAndOptionalFields() {
    final String canonicalPolicy = ramLfePolicyJson();
    assertEquals(
        "retail policy",
        RamLfeJsonParser.parsePolicyList(bytes(canonicalPolicy)).items().get(0).note());
    assertEquals(
        "verification failed",
        RamLfeJsonParser.parseReceiptVerifyResponse(
                bytes(ramLfeVerifyJson("\"verification failed\"")))
            .error());

    assertRejects(
        "ram-lfe program policy list.items[0].program_id",
        () ->
            RamLfeJsonParser.parsePolicyList(
                bytes(canonicalPolicy.replace("\"program_id\":\"lookup\"", "\"program_id\":7"))));
    assertRejects(
        "ram-lfe program policy list.items[0].note",
        () ->
            RamLfeJsonParser.parsePolicyList(
                bytes(canonicalPolicy.replace("\"note\":\"retail policy\"", "\"note\":false"))));
    assertRejects(
        "ram-lfe receipt verify response.error",
        () -> RamLfeJsonParser.parseReceiptVerifyResponse(bytes(ramLfeVerifyJson("7"))));
  }

  @Test
  public void ramLfeParserRejectsMissingOrMalformedRequiredBooleans() {
    final String canonicalPolicy = ramLfePolicyJson();
    assertFalse(
        RamLfeJsonParser.parsePolicyList(
                bytes(canonicalPolicy.replace("\"active\":true", "\"active\":false")))
            .items()
            .get(0)
            .active());

    assertRejects(
        "ram-lfe program policy list.items[0].active",
        () ->
            RamLfeJsonParser.parsePolicyList(
                bytes(canonicalPolicy.replace("\"active\":true", "\"active\":1"))));
    assertRejects(
        "ram-lfe program policy list.items[0].active",
        () ->
            RamLfeJsonParser.parsePolicyList(
                bytes(canonicalPolicy.replace("\"active\":true,", ""))));

    final String canonicalVerify = ramLfeVerifyJson("null");
    assertFalse(RamLfeJsonParser.parseReceiptVerifyResponse(bytes(canonicalVerify)).valid());
    assertRejects(
        "ram-lfe receipt verify response.valid",
        () ->
            RamLfeJsonParser.parseReceiptVerifyResponse(
                bytes(canonicalVerify.replace("\"valid\":false", "\"valid\":0"))));
    assertRejects(
        "ram-lfe receipt verify response.valid",
        () ->
            RamLfeJsonParser.parseReceiptVerifyResponse(
                bytes(canonicalVerify.replace("\"valid\":false,", ""))));

    final String optionalNull =
        canonicalVerify.replace(
            "\"error\":", "\"output_hash_matches\":null,\"error\":");
    assertNull(
        RamLfeJsonParser.parseReceiptVerifyResponse(bytes(optionalNull)).outputHashMatches());
    assertRejects(
        "ram-lfe receipt verify response.output_hash_matches",
        () ->
            RamLfeJsonParser.parseReceiptVerifyResponse(
                bytes(
                    optionalNull.replace(
                        "\"output_hash_matches\":null", "\"output_hash_matches\":0"))));
  }

  @Test
  public void soracloudParserRejectsNonStringRequiredAndOptionalFields() {
    final SoracloudPrivateUploadedModelReceiptListResponse canonical =
        SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(soracloudReceiptListJson("\"exact\"", "\"" + RECEIPT_CURSOR + "\"")));
    assertEquals("exact", canonical.countMode());
    assertEquals(RECEIPT_CURSOR, canonical.continueCursor());

    assertRejects(
        "soracloud private receipt list.count_mode",
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                bytes(soracloudReceiptListJson("7", "null"))));
    assertRejects(
        "soracloud private receipt list.continue_cursor",
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                bytes(soracloudReceiptListJson("\"exact\"", "7"))));
  }

  @Test
  public void soracloudParserRejectsMissingOrMalformedHasMore() {
    final String canonical = soracloudReceiptListJson("\"exact\"", "null");
    assertFalse(
        SoracloudPrivateUploadedModelJsonParser.parseReceiptList(bytes(canonical)).hasMore());

    assertRejects(
        "soracloud private receipt list.has_more",
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                bytes(canonical.replace("\"has_more\":false", "\"has_more\":0"))));
    assertRejects(
        "soracloud private receipt list.has_more",
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                bytes(canonical.replace("\"has_more\":false,", ""))));
  }

  private static String identifierPolicyJson() {
    return "{"
        + "\"total\":1,"
        + "\"items\":[{"
        + "\"policy_id\":\"phone#retail\","
        + "\"owner\":\"owner\","
        + "\"active\":true,"
        + "\"normalization\":\"phone_e164\","
        + "\"resolver_public_key\":\""
        + VALID_PUBLIC_KEY
        + "\","
        + "\"backend\":\"bfv-affine-sha3-256-v1\","
        + "\"note\":\"retail policy\""
        + "}]}";
  }

  private static String ramLfePolicyJson() {
    return "{"
        + "\"total\":1,"
        + "\"items\":[{"
        + "\"program_id\":\"lookup\","
        + "\"owner\":\"owner\","
        + "\"active\":true,"
        + "\"resolver_public_key\":\""
        + VALID_PUBLIC_KEY
        + "\","
        + "\"backend\":\"bfv-programmed-sha3-256-v1\","
        + "\"verification_mode\":\"signed\","
        + "\"note\":\"retail policy\""
        + "}]}";
  }

  private static String ramLfeVerifyJson(final String error) {
    return "{"
        + "\"valid\":false,"
        + "\"program_id\":\"lookup\","
        + "\"backend\":\"bfv-programmed-sha3-256-v1\","
        + "\"verification_mode\":\"signed\","
        + "\"output_hash\":\""
        + "11".repeat(32)
        + "\","
        + "\"associated_data_hash\":\""
        + "22".repeat(32)
        + "\","
        + "\"error\":"
        + error
        + "}";
  }

  private static String soracloudReceiptListJson(
      final String countMode, final String continueCursor) {
    final boolean hasMore = !"null".equals(continueCursor);
    return "{"
        + "\"schema_version\":1,"
        + "\"receipts\":[],"
        + "\"total\":"
        + (hasMore ? 1 : 0)
        + ","
        + "\"returned_items\":0,"
        + "\"remaining_items\":"
        + (hasMore ? 1 : 0)
        + ","
        + "\"has_more\":"
        + hasMore
        + ","
        + "\"count_mode\":"
        + countMode
        + ","
        + "\"continue_cursor\":"
        + continueCursor
        + "}";
  }

  private static String repeated(final char value, final int count) {
    final char[] chars = new char[count];
    Arrays.fill(chars, value);
    return new String(chars);
  }

  private static byte[] bytes(final String json) {
    return json.getBytes(StandardCharsets.UTF_8);
  }

  private static void assertRejects(final String path, final Runnable parse) {
    try {
      parse.run();
    } catch (final IllegalStateException expected) {
      if (expected.getMessage() == null || !expected.getMessage().contains(path)) {
        throw new AssertionError("expected rejection to identify " + path, expected);
      }
      return;
    }
    throw new AssertionError("expected malformed-present or missing scalar field rejection");
  }
}
