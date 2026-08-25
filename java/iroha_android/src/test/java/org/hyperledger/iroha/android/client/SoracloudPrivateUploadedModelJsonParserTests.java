package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;

public final class SoracloudPrivateUploadedModelJsonParserTests {

  private SoracloudPrivateUploadedModelJsonParserTests() {}

  public static void main(final String[] args) {
    parsesSubmittedPrivateExecuteResponseAndDurableReceipt();
    parsesCommittedReplayWithExplicitNullTransactionHash();
    parsesPrivateReceiptListPaginationMetadata();
    boundedReceiptListLeavesTotalAbsent();
    rejectsRetiredInstructionSurfaceAndInvalidSubmissionState();
    rejectsMissingProductionReceiptEvidence();
    rejectsNonCanonicalReceiptNetworkIdentity();
    rejectsMalformedValidatorAndOutputRecipient();
    rejectsMismatchedResponseOutputArtifact();
    rejectsNegativeReceiptPaginationMetadata();
    rejectsInvalidReceiptArtifactAndSequenceFields();
    rejectsBlankReceiptIdentityFields();
    System.out.println("[IrohaAndroid] SoracloudPrivateUploadedModelJsonParserTests passed.");
  }

  private static void parsesSubmittedPrivateExecuteResponseAndDurableReceipt() {
    final SoracloudPrivateUploadedModelExecuteResponse response =
        SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(bytes(executeResponseJson()));

    assert response.schemaVersion() == 1L : "schema version";
    assert "finalized".equals(response.status().get("status")) : "status";
    assert "submitted".equals(response.submissionStatus()) : "submission status";
    assert "transaction-hash".equals(response.transactionHash()) : "transaction hash";
    assert NETWORK_ID.equals(response.receipt().networkId()) : "network id";
    assert "receipt-1".equals(response.receipt().receiptId()) : "receipt id";
    assert "portal".equals(response.receipt().serviceName()) : "service";
    assert "2026.1".equals(response.receipt().serviceVersion()) : "service version";
    assert "decrypt-upload-1".equals(response.receipt().decryptionRequestId())
        : "decryption request";
    assert response.receipt().attestingValidator().laneId() == 0L : "validator lane";
    assert "validator@public".equals(
        response.receipt().attestingValidator().validatorAccountId()) : "validator account";
    assert "peer-1".equals(response.receipt().attestingValidator().peerId()) : "validator peer";
    assert "input".equals(response.receipt().inputArtifact().artifactRole()) : "input role";
    assert "output".equals(response.receipt().outputArtifact().artifactRole()) : "output role";
    assert response.receipt().inputArtifact().sorafsRootCid().size() == 36 : "root CID width";
    assert response.receipt().inputArtifact().sorafsRootCid().subList(0, 4)
        .equals(java.util.Arrays.asList(1, 113, 31, 32)) : "root CID framing";
    assert "output-manifest".equals(response.outputArtifact().sorafsManifestDigest())
        : "response output";
    assert "recipient-key".equals(response.receipt().outputRecipient().keyId()) : "recipient";
    assert response.receipt().outputRecipient().publicKeyBytes().length == 32 : "recipient key";
    assert response.receipt().emittedSequence() == 0L : "submission sequence sentinel";
    assert response.receipt().emittedBlockHeight() == 0L : "submission height sentinel";
  }

  private static void parsesCommittedReplayWithExplicitNullTransactionHash() {
    final SoracloudPrivateUploadedModelExecuteResponse response =
        SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(
                executeResponseJson()
                    .replace("\"submission_status\":\"submitted\"", "\"submission_status\":\"committed\"")
                    .replace("\"transaction_hash\":\"transaction-hash\"", "\"transaction_hash\":null")
                    .replace("\"emitted_sequence\":0", "\"emitted_sequence\":17")
                    .replace("\"emitted_block_height\":0", "\"emitted_block_height\":501")));

    assert "committed".equals(response.submissionStatus()) : "committed replay";
    assert response.transactionHash() == null : "committed transaction hash";
    assert response.receipt().emittedSequence() == 17L : "committed sequence";
    assert response.receipt().emittedBlockHeight() == 501L : "committed height";
  }

  private static void parsesPrivateReceiptListPaginationMetadata() {
    final String json = "{"
        + "\"schema_version\":1,"
        + "\"receipts\":["
        + receiptJson()
            .replace("\"emitted_sequence\":0", "\"emitted_sequence\":17")
            .replace("\"emitted_block_height\":0", "\"emitted_block_height\":501")
        + "],"
        + "\"total\":3,"
        + "\"returned_items\":1,"
        + "\"remaining_items\":2,"
        + "\"has_more\":true,"
        + "\"count_mode\":\"exact\","
        + "\"continue_cursor\":null"
        + "}";
    final SoracloudPrivateUploadedModelReceiptListResponse response =
        SoracloudPrivateUploadedModelJsonParser.parseReceiptList(bytes(json));

    assert response.receipts().size() == 1 : "receipt count";
    assert Long.valueOf(3L).equals(response.total()) : "total";
    assert response.returnedItems() == 1L : "returned";
    assert response.remainingItems() == 2L : "remaining";
    assert "exact".equals(response.countMode()) : "count mode";
    assert response.continueCursor() == null : "continue cursor";
    assert "2026.1".equals(response.receipts().get(0).serviceVersion())
        : "list receipt service version";
  }

  private static void boundedReceiptListLeavesTotalAbsent() {
    final SoracloudPrivateUploadedModelReceiptListResponse response =
        SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes("{"
                + "\"schema_version\":1,"
                + "\"receipts\":[],"
                + "\"returned_items\":0,"
                + "\"remaining_items\":0,"
                + "\"has_more\":false,"
                + "\"count_mode\":\"bounded\""
                + "}"));

    assert response.total() == null : "bounded total absent";
    assert !response.hasMore() : "has more";
  }

  private static void rejectsRetiredInstructionSurfaceAndInvalidSubmissionState() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"output_artifact\":", "\"tx_instructions\":[],\"output_artifact\":"))),
        "expected retired tx_instructions rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"submission_status\":\"submitted\"", "\"submission_status\":\"pending\""))),
        "expected unknown submission status rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"transaction_hash\":\"transaction-hash\"", "\"transaction_hash\":null"))),
        "expected submitted response hash rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"submission_status\":\"submitted\"", "\"submission_status\":\"committed\""))),
        "expected committed response non-null hash rejection");
  }

  private static void rejectsMissingProductionReceiptEvidence() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"network_id\":\"" + NETWORK_ID + "\",", ""))),
        "expected missing network_id rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"service_version\":\"2026.1\",", ""))),
        "expected missing service_version rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"decryption_request_id\":\"decrypt-upload-1\",", ""))),
        "expected missing decryption_request_id rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(attestingValidatorField(), ""))),
        "expected missing attesting_validator rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(outputRecipientField(), ""))),
        "expected missing output_recipient rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(",\"emitted_block_height\":0", ""))),
        "expected missing emitted_block_height rejection");
  }

  private static void rejectsNonCanonicalReceiptNetworkIdentity() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                "\"network_id\":\"" + NETWORK_ID + "\"", "\"network_id\":7"))),
        "expected non-string network_id rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                NETWORK_ID, NETWORK_ID.toLowerCase(java.util.Locale.ROOT)))),
        "expected non-canonical network_id spelling rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(NETWORK_ID, NETWORK_ID.replace("#A2F0", "#A2F1")))),
        "expected invalid network_id checksum rejection");
  }

  private static void rejectsMalformedValidatorAndOutputRecipient() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"lane_id\":0", "\"lane_id\":-1"))),
        "expected invalid lane rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(PUBLIC_KEY_BASE64, "not-base64"))),
        "expected malformed recipient key rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("X25519HkdfSha256", "UnknownKem"))),
        "expected unsupported KEM rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"value\":null", "\"value\":{}"))),
        "expected non-unit suite value rejection");
  }

  private static void rejectsMismatchedResponseOutputArtifact() {
    final String canonical = executeResponseJson();
    final String target = "\"output-manifest\"";
    final int responseOutput = canonical.lastIndexOf(target);
    final String mismatched = canonical.substring(0, responseOutput)
        + "\"different-output-manifest\""
        + canonical.substring(responseOutput + target.length());
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(bytes(mismatched)),
        "expected response/receipt output mismatch rejection");
  }

  private static void rejectsNegativeReceiptPaginationMetadata() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(receiptListJson("-1", "0", "0"))),
        "expected negative total rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(receiptListJson("0", "-1", "0"))),
        "expected negative returned_items rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(receiptListJson("0", "0", "-1"))),
        "expected negative remaining_items rejection");
  }

  private static void rejectsInvalidReceiptArtifactAndSequenceFields() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                "\"sorafs_root_cid\":" + ROOT_CID_JSON + ",", ""))),
        "expected missing sorafs_root_cid rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(ROOT_CID_JSON, "[1,113,31,32,1]"))),
        "expected short sorafs_root_cid rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                ROOT_CID_JSON, ROOT_CID_JSON.replaceFirst("\\[1,113", "[2,113")))),
        "expected malformed sorafs_root_cid prefix rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(ROOT_CID_JSON, ZERO_DIGEST_ROOT_CID_JSON))),
        "expected zero sorafs_root_cid digest rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                ROOT_CID_JSON, ROOT_CID_JSON.replaceFirst(",1,2", ",1.0,2")))),
        "expected non-integer sorafs_root_cid byte rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"ciphertext_bytes\":64", "\"ciphertext_bytes\":0"))),
        "expected zero ciphertext_bytes rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"emitted_sequence\":0", "\"emitted_sequence\":-1"))),
        "expected negative emitted_sequence rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"emitted_block_height\":0", "\"emitted_block_height\":-1"))),
        "expected negative emitted_block_height rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"artifact_role\":\"input\"", "\"artifact_role\":\"output\""))),
        "expected swapped artifact role rejection");
  }

  private static void rejectsBlankReceiptIdentityFields() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"receipt_id\":\"receipt-1\"", "\"receipt_id\":\"   \""))),
        "expected blank receipt_id rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"policy_id\":\"policy-1\"", "\"policy_id\":\"\""))),
        "expected blank policy_id rejection");
  }

  private static String receiptListJson(
      final String total, final String returnedItems, final String remainingItems) {
    return "{"
        + "\"schema_version\":1,"
        + "\"receipts\":[],"
        + "\"total\":" + total + ","
        + "\"returned_items\":" + returnedItems + ","
        + "\"remaining_items\":" + remainingItems + ","
        + "\"has_more\":false,"
        + "\"count_mode\":\"exact\""
        + "}";
  }

  private static String executeResponseJson() {
    return "{"
        + "\"schema_version\":1,"
        + "\"status\":{\"status\":\"finalized\",\"service_name\":\"portal\"},"
        + "\"submission_status\":\"submitted\","
        + "\"transaction_hash\":\"transaction-hash\","
        + "\"receipt\":" + receiptJson() + ","
        + "\"output_artifact\":" + outputArtifactJson()
        + "}";
  }

  private static String receiptJson() {
    return "{"
        + "\"schema_version\":1,"
        + "\"network_id\":\"" + NETWORK_ID + "\","
        + "\"receipt_id\":\"receipt-1\","
        + "\"service_name\":\"portal\","
        + "\"service_version\":\"2026.1\","
        + "\"model_id\":\"upload-1\","
        + "\"weight_version\":\"v1\","
        + "\"runtime_version\":\"soracloud.quantized-cpu.v1\","
        + "\"model_manifest_digest\":\"model-manifest\","
        + "\"model_bundle_root\":\"bundle-root\","
        + "\"policy_id\":\"policy-1\","
        + "\"decryption_request_id\":\"decrypt-upload-1\","
        + attestingValidatorField()
        + "\"input_artifact\":{"
        + "\"schema_version\":1,"
        + "\"sorafs_manifest_digest\":\"input-manifest\","
        + "\"sorafs_root_cid\":" + ROOT_CID_JSON + ","
        + "\"artifact_hash\":\"input-artifact\","
        + "\"ciphertext_bytes\":64,"
        + "\"artifact_role\":\"input\""
        + "},"
        + "\"output_artifact\":" + outputArtifactJson() + ","
        + "\"input_commitment\":\"input-commitment\","
        + "\"output_commitment\":\"output-commitment\","
        + outputRecipientField()
        + "\"request_commitment\":\"request-commitment\","
        + "\"result_commitment\":\"result-commitment\","
        + "\"emitted_sequence\":0,"
        + "\"emitted_block_height\":0"
        + "}";
  }

  private static String attestingValidatorField() {
    return "\"attesting_validator\":{"
        + "\"lane_id\":0,"
        + "\"validator_account_id\":\"validator@public\","
        + "\"peer_id\":\"peer-1\""
        + "},";
  }

  private static String outputRecipientField() {
    return "\"output_recipient\":{"
        + "\"schema_version\":1,"
        + "\"key_id\":\"recipient-key\","
        + "\"key_version\":1,"
        + "\"kem\":{\"kem\":\"X25519HkdfSha256\",\"value\":null},"
        + "\"aead\":{\"aead\":\"Aes256Gcm\",\"value\":null},"
        + "\"public_key_bytes\":\"" + PUBLIC_KEY_BASE64 + "\","
        + "\"public_key_fingerprint\":\"recipient-fingerprint\""
        + "},";
  }

  private static String outputArtifactJson() {
    return "{"
        + "\"schema_version\":1,"
        + "\"sorafs_manifest_digest\":\"output-manifest\","
        + "\"sorafs_root_cid\":" + ROOT_CID_JSON + ","
        + "\"artifact_hash\":\"output-artifact\","
        + "\"ciphertext_bytes\":96,"
        + "\"artifact_role\":\"output\""
        + "}";
  }

  private static byte[] bytes(final String json) {
    return json.getBytes(StandardCharsets.UTF_8);
  }

  private static void assertThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final RuntimeException expected) {
      return;
    }
    throw new AssertionError(message);
  }

  private static final String PUBLIC_KEY_BASE64 =
      "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  private static final String NETWORK_ID =
      "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
  private static final String ROOT_CID_JSON =
      "[1,113,31,32,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32]";
  private static final String ZERO_DIGEST_ROOT_CID_JSON =
      "[1,113,31,32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]";
}
