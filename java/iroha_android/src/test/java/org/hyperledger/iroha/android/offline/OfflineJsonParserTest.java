package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Locale;

public final class OfflineJsonParserTest {
  private static final String CANONICAL_ASSET_DEFINITION_ID =
      "7EAD8EFYUx1aVKZPUU1fyKvr8dF1";
  private static final String CANONICAL_ASSET_JSON = "\"" + CANONICAL_ASSET_DEFINITION_ID + "\"";

  private OfflineJsonParserTest() {}

  public static void main(final String[] args) {
    parsesOfflineReadiness();
    parsesExpectedUnavailableReadiness();
    ignoresUnknownOfflineReadinessMembers();
    rejectsMalformedUtf8WithoutReplacement();
    acceptsOnlyJsonWhitespaceAroundTheDocument();
    rejectsNonCanonicalOfflineReadiness();
    validatesReadinessTextAsBoundedUnicodeScalars();
    parsesOfflineTransfers();
    rejectsOfflineTransferMalformedIntegerFields();
    canonicalizesJson();
    System.out.println("[IrohaAndroid] OfflineJsonParserTest passed.");
  }

  private static void parsesOfflineReadiness() {
    final String json =
        """
        {
          "asset_definition_id": "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
          "asset_scale": 9,
          "evaluated_block_height": 18446744073709551615,
          "evaluated_block_hash": "abababababababababababababababababababababababababababababababab",
          "active_transfer_verifier": {
            "id": {"backend": "halo2/ipa", "name": "offline-transfer"},
            "version": 7,
            "circuit_id": "confidential-transfer-v2",
            "commitment": "4444444444444444444444444444444444444444444444444444444444444444",
            "public_inputs_schema_hash": "5555555555555555555555555555555555555555555555555555555555555555",
            "max_proof_bytes": 4096,
            "activation_height": 1,
            "withdrawal_height": null
          },
          "active_topup_shield_verifier": {
            "id": {"backend": "halo2/ipa", "name": "topup-shield-v2"},
            "version": 3,
            "circuit_id": "topup-shield-v2",
            "commitment": "6666666666666666666666666666666666666666666666666666666666666666",
            "public_inputs_schema_hash": "7777777777777777777777777777777777777777777777777777777777777777",
            "max_proof_bytes": 8192,
            "activation_height": 1,
            "withdrawal_height": null
          },
          "ready": false,
          "blockers": [
            {"code": "offline_disabled", "message": "Offline transfers are disabled"}
          ]
        }
        """;
    final OfflineReadiness readiness =
        OfflineJsonParser.parseOfflineReadiness(json.getBytes(StandardCharsets.UTF_8));
    assert CANONICAL_ASSET_DEFINITION_ID.equals(readiness.assetDefinitionId());
    assert Long.valueOf(9).equals(readiness.assetScale());
    assert new BigInteger("18446744073709551615").equals(readiness.evaluatedBlockHeight());
    assert "abababababababababababababababababababababababababababababababab"
        .equals(readiness.evaluatedBlockHash());
    assert "halo2/ipa".equals(readiness.activeTransferVerifier().id().backend());
    assert readiness.activeTransferVerifier().maxProofBytes() == 4096L;
    assert "topup-shield-v2".equals(readiness.activeTopUpShieldVerifier().id().name());
    assert readiness.activeTopUpShieldVerifier().maxProofBytes() == 8192L;
    assert !readiness.ready();
    assert readiness.blockers().size() == 1;
    assert "offline_disabled".equals(readiness.blockers().get(0).code());
    assert "Offline transfers are disabled".equals(readiness.blockers().get(0).message());
  }

  private static void ignoresUnknownOfflineReadinessMembers() {
    final OfflineReadiness readiness =
        parseOfflineReadiness(
            readinessBody(
                "\"future_top_level\": {\"ignored\": true, \"ratio\": 1.25},",
                CANONICAL_ASSET_JSON,
                "7",
                "false",
                "[{\"code\":\"2fa_required\",\"message\":\"no\",\"future_detail\":7}]"));
    assert readiness.blockers().size() == 1;
    assert "2fa_required".equals(readiness.blockers().get(0).code());
  }

  private static void parsesExpectedUnavailableReadiness() {
    final OfflineReadiness readiness =
        parseOfflineReadiness(
            readinessBody(
                "",
                CANONICAL_ASSET_JSON,
                "29",
                "7",
                activeTransferVerifier(),
                "false",
                "[{\"code\":\"asset_scale_unsupported\",\"message\":\"unsupported\"}]"));
    assert Long.valueOf(29).equals(readiness.assetScale());
    assert readiness.activeTransferVerifier() != null;

    final OfflineReadiness topUpUnavailable =
        parseOfflineReadiness(
            readinessBody(
                "",
                CANONICAL_ASSET_JSON,
                "9",
                "7",
                activeTransferVerifier(),
                "null",
                "false",
                "[{\"code\":\"topup_shield_verifier_unavailable\",\"message\":\"unavailable\"}]"));
    assert topUpUnavailable.activeTopUpShieldVerifier() == null;
    assert "topup_shield_verifier_unavailable"
        .equals(topUpUnavailable.blockers().get(0).code());
  }

  private static void rejectsMalformedUtf8WithoutReplacement() {
    final byte[] payload =
        readinessBody("", CANONICAL_ASSET_JSON, "7", "true", "[]")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] marker = CANONICAL_ASSET_DEFINITION_ID.getBytes(StandardCharsets.US_ASCII);
    final int offset = findSubsequence(payload, marker);
    if (offset < 0) {
      throw new AssertionError("readiness asset marker missing from test payload");
    }
    payload[offset] = (byte) 0xc3;
    expectIllegalState(
        () -> OfflineJsonParser.parseOfflineReadiness(payload),
        "Offline JSON payload must be valid UTF-8");
  }

  private static void acceptsOnlyJsonWhitespaceAroundTheDocument() {
    final String canonical = readinessBody("", CANONICAL_ASSET_JSON, "7", "true", "[]");
    final OfflineReadiness readiness = parseOfflineReadiness("\t\n" + canonical + "\r ");
    assert CANONICAL_ASSET_DEFINITION_ID.equals(readiness.assetDefinitionId());
    expectIllegalState(
        () -> parseOfflineReadiness("\u0000" + canonical),
        "Invalid number: expected digit");
  }

  private static void rejectsNonCanonicalOfflineReadiness() {
    for (final MalformedReadinessCase item : malformedReadinessCases()) {
      expectIllegalState(() -> parseOfflineReadiness(item.body), item.message);
    }
  }

  private static void validatesReadinessTextAsBoundedUnicodeScalars() {
    final String boundary = "x".repeat(1023) + "😀";
    final OfflineReadinessBlocker blocker = new OfflineReadinessBlocker("blocked", boundary);
    assert blocker.message().codePointCount(0, blocker.message().length()) == 1024;
    expectIllegalState(
        () -> new OfflineReadinessBlocker("blocked", "x".repeat(1024) + "😀"),
        "must not exceed 1024 Unicode characters");
    expectIllegalState(
        () -> new OfflineReadinessBlocker("blocked", "line\u0001break"),
        "must be exact non-empty text");
    expectIllegalState(
        () -> new OfflineVerifierId(new String(new char[] {'\uD800'}), "transfer"),
        "must contain well-formed Unicode");
  }


  private static void parsesOfflineTransfers() {
    final String json =
        """
        {
          "items": [
            {
              "bundle_id_hex": "0xabc123",
              "controller_id": "payer",
              "controller_display": "Payer",
              "receiver_id": "payee",
              "receiver_display": "Payee",
              "deposit_account_id": "deposit",
              "deposit_account_display": "Deposit",
              "asset_id": "pkr#paynet",
              "total_amount": "500",
              "claimed_delta": "500",
              "status": "PENDING",
              "receipt_count": 1,
              "recorded_at_ms": 1767648180000,
              "recorded_at_height": 12345,
              "receipt_summaries": [
                {
                  "sender_id": "payer",
                  "receiver_id": "payee",
                  "amount": "500",
                  "asset_id": "pkr#paynet",
                  "status": "PENDING"
                }
              ]
            }
          ],
          "total": 1
        }
        """;
    final OfflineTransferList transfers =
        OfflineJsonParser.parseTransfers(json.getBytes(StandardCharsets.UTF_8));
    assert transfers.total() == 1L;
    assert transfers.items().size() == 1;
    final OfflineTransferList.OfflineTransferItem item = transfers.items().get(0);
    assert "0xabc123".equals(item.bundleIdHex());
    assert "Payee".equals(item.receiverDisplay());
    assert "Deposit".equals(item.depositAccountDisplay());
    assert item.receiptCount() == 1L;
    assert item.firstReceiptSummary().isPresent();
    assert "payer".equals(item.firstReceiptSummary().get().senderId());
    assert "pkr#paynet".equals(item.toJsonMap().get("asset_id"));
  }

  private static void rejectsOfflineTransferMalformedIntegerFields() {
    for (final MalformedTransferIntegerCase item : malformedTransferIntegerCases()) {
      expectIllegalState(() -> parseOfflineTransfers(item.body), item.message);
    }
  }

  private static void canonicalizesJson() {
    final String encoded =
        OfflineJsonParser.canonicalJson("{\"b\":2,\"a\":1}".getBytes(StandardCharsets.UTF_8));
    assert "{\"a\":1,\"b\":2}".equals(encoded) : "canonical JSON mismatch: " + encoded;
  }

  private static OfflineReadiness parseOfflineReadiness(final String json) {
    return OfflineJsonParser.parseOfflineReadiness(json.getBytes(StandardCharsets.UTF_8));
  }


  private static OfflineTransferList parseOfflineTransfers(final String json) {
    return OfflineJsonParser.parseTransfers(json.getBytes(StandardCharsets.UTF_8));
  }

  private record MalformedReadinessCase(String body, String message) {}

  private record MalformedTransferIntegerCase(String body, String message) {}

  private static MalformedReadinessCase[] malformedReadinessCases() {
    return new MalformedReadinessCase[] {
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "\"7\"", "true", "[]"),
          "evaluated_block_height must be a JSON integer number"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "-1", "true", "[]"),
          "evaluated_block_height must fit in an unsigned 64-bit integer"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "18446744073709551616", "true", "[]"),
          "evaluated_block_height must fit in an unsigned 64-bit integer"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "1e1", "true", "[]"),
          "evaluated_block_height must be a JSON integer number"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "-1", "7", activeTransferVerifier(),
              "true", "[]"),
          "asset_scale must fit in an unsigned 64-bit integer"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "4294967296", "7",
              activeTransferVerifier(), "true", "[]"),
          "asset_scale must fit in an unsigned 32-bit integer"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "29", "7", activeTransferVerifier(),
              "true", "[]"),
          "asset_scale_unsupported must be present exactly when assetScale exceeds 28"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "null", "7", activeTransferVerifier(),
              "false", "[{\"code\":\"blocked\",\"message\":\"no\"}]"),
          "asset_scale_unavailable must be present exactly when assetScale is null"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "9", "7", "null", "true", "[]"),
          "transfer_verifier_unavailable must be present exactly when no active verifier is reported"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "9", "7", activeTransferVerifier(),
              "null", "true", "[]"),
          "topup_shield_verifier_unavailable must be present exactly when no active top-up shield verifier is reported"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "9", "7",
              activeTransferVerifier().replace("\"max_proof_bytes\": 4096", "\"max_proof_bytes\": 0"),
              "true", "[]"),
          "maxProofBytes must fit in a positive unsigned 32-bit integer"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "9", "7",
              activeTransferVerifier().replace("\"activation_height\": 1", "\"activation_height\": 8"),
              "true", "[]"),
          "active_transfer_verifier must be active at evaluated_block_height"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "9", "7", activeTransferVerifier(),
              activeTopUpShieldVerifier().replace(
                  "\"max_proof_bytes\": 8192", "\"max_proof_bytes\": 0"),
              "true", "[]"),
          "maxProofBytes must fit in a positive unsigned 32-bit integer"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "9", "7", activeTransferVerifier(),
              activeTopUpShieldVerifier().replace(
                  "\"activation_height\": 1", "\"activation_height\": 8"),
              "true", "[]"),
          "active_topup_shield_verifier must be active at evaluated_block_height"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "9", "7", activeTransferVerifier(),
              activeTopUpShieldVerifier(), "false",
              "[{\"code\":\"topup_shield_verifier_unavailable\",\"message\":\"no\"}]"),
          "topup_shield_verifier_unavailable must be present exactly when no active top-up shield verifier is reported"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "7", "true", "[]")
              .replace("\"active_topup_shield_verifier\": {",
                  "\"future_topup_shield_verifier\": {"),
          "root.active_topup_shield_verifier is required"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "7", "1", "[]"),
          "ready must be a boolean"),
      new MalformedReadinessCase(
          readinessBody("", "\" xor#wonderland\"", "7", "true", "[]"),
          "asset_definition_id must be an exact non-empty string"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "7", "true",
              "[{\"code\":\"blocked\",\"message\":1}]"),
          "blockers[0].message must be a string"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "7", "false",
              "[{\"code\":\"Bad-Code\",\"message\":\"no\"}]"),
          "code must be a 1-64 character lowercase stable identifier"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "7", "false", "[]"),
          "ready must be true exactly when blockers is empty"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "7", "true",
              "[{\"code\":\"blocked\",\"message\":\"no\"}]"),
          "ready must be true exactly when blockers is empty"),
      new MalformedReadinessCase(
          readinessBody("", CANONICAL_ASSET_JSON, "7", "false",
              "[{\"code\":\"blocked\",\"message\":\"one\"},{\"code\":\"blocked\",\"message\":\"two\"}]"),
          "blockers must not repeat blocker codes")
    };
  }

  private static MalformedTransferIntegerCase[] malformedTransferIntegerCases() {
    return new MalformedTransferIntegerCase[] {
      new MalformedTransferIntegerCase(
          transferListBody("\"1\"", "1", "1", "1"),
          "total must be a JSON integer number"),
      new MalformedTransferIntegerCase(
          transferListBody("\"\"", "1", "1", "1"),
          "total must be a JSON integer number"),
      new MalformedTransferIntegerCase(
          transferListBody("1.5", "1", "1", "1"),
          "total must be an integer"),
      new MalformedTransferIntegerCase(
          transferListBody("9223372036854775808", "1", "1", "1"),
          "total must fit in signed 64-bit range"),
      new MalformedTransferIntegerCase(
          transferListBody("1", "\"1\"", "1", "1"),
          "items[0].receipt_count must be a JSON integer number"),
      new MalformedTransferIntegerCase(
          transferListBody("1", "1", "1.5", "1"),
          "items[0].recorded_at_ms must be an integer"),
      new MalformedTransferIntegerCase(
          transferListBody("1", "1", "1", "\"\""),
          "items[0].recorded_at_height must be a JSON integer number")
    };
  }

  private static String transferListBody(
      final String total,
      final String receiptCount,
      final String recordedAtMs,
      final String recordedAtHeight) {
    return String.format(
        Locale.ROOT,
        """
        {
          "items": [
            {
              "receipt_count": %s,
              "recorded_at_ms": %s,
              "recorded_at_height": %s
            }
          ],
          "total": %s
        }
        """,
        receiptCount,
        recordedAtMs,
        recordedAtHeight,
        total);
  }

  private static String readinessBody(
      final String extra,
      final String assetDefinitionId,
      final String evaluatedBlockHeight,
      final String ready,
      final String blockers) {
    return readinessBody(
        extra,
        assetDefinitionId,
        "9",
        evaluatedBlockHeight,
        activeTransferVerifier(),
        ready,
        blockers);
  }

  private static String readinessBody(
      final String extra,
      final String assetDefinitionId,
      final String assetScale,
      final String evaluatedBlockHeight,
      final String activeTransferVerifier,
      final String ready,
      final String blockers) {
    return readinessBody(
        extra,
        assetDefinitionId,
        assetScale,
        evaluatedBlockHeight,
        activeTransferVerifier,
        activeTopUpShieldVerifier(),
        ready,
        blockers);
  }

  private static String readinessBody(
      final String extra,
      final String assetDefinitionId,
      final String assetScale,
      final String evaluatedBlockHeight,
      final String activeTransferVerifier,
      final String activeTopUpShieldVerifier,
      final String ready,
      final String blockers) {
    return String.format(
        Locale.ROOT,
        """
        {
          %s
          "asset_definition_id": %s,
          "asset_scale": %s,
          "evaluated_block_height": %s,
          "evaluated_block_hash": "abababababababababababababababababababababababababababababababab",
          "active_transfer_verifier": %s,
          "active_topup_shield_verifier": %s,
          "ready": %s,
          "blockers": %s
        }
        """,
        extra,
        assetDefinitionId,
        assetScale,
        evaluatedBlockHeight,
        activeTransferVerifier,
        activeTopUpShieldVerifier,
        ready,
        blockers);
  }

  private static String activeTransferVerifier() {
    return """
        {
          "id": {"backend": "halo2/ipa", "name": "offline-transfer"},
          "version": 7,
          "circuit_id": "confidential-transfer-v2",
          "commitment": "4444444444444444444444444444444444444444444444444444444444444444",
          "public_inputs_schema_hash": "5555555555555555555555555555555555555555555555555555555555555555",
          "max_proof_bytes": 4096,
          "activation_height": 1,
          "withdrawal_height": null
        }
        """;
  }

  private static String activeTopUpShieldVerifier() {
    return """
        {
          "id": {"backend": "halo2/ipa", "name": "topup-shield-v2"},
          "version": 3,
          "circuit_id": "topup-shield-v2",
          "commitment": "6666666666666666666666666666666666666666666666666666666666666666",
          "public_inputs_schema_hash": "7777777777777777777777777777777777777777777777777777777777777777",
          "max_proof_bytes": 8192,
          "activation_height": 1,
          "withdrawal_height": null
        }
        """;
  }

  private static int findSubsequence(final byte[] haystack, final byte[] needle) {
    outer:
    for (int offset = 0; offset <= haystack.length - needle.length; offset++) {
      for (int index = 0; index < needle.length; index++) {
        if (haystack[offset + index] != needle[index]) {
          continue outer;
        }
      }
      return offset;
    }
    return -1;
  }

  private static void expectIllegalState(
      final Runnable action, final String expectedMessageFragment) {
    try {
      action.run();
    } catch (final IllegalStateException | IllegalArgumentException ex) {
      assert ex.getMessage().contains(expectedMessageFragment)
          : "unexpected message: " + ex.getMessage();
      return;
    }
    throw new AssertionError("Expected IllegalStateException");
  }
}
