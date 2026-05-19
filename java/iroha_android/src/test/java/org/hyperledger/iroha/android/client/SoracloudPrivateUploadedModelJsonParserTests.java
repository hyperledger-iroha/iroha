package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.List;

public final class SoracloudPrivateUploadedModelJsonParserTests {

  private SoracloudPrivateUploadedModelJsonParserTests() {}

  public static void main(final String[] args) {
    parsesPrivateExecuteResponseAndReceiptInstruction();
    parsesPrivateReceiptListPaginationMetadata();
    rejectsMissingOrMalformedReceiptInstruction();
    boundedReceiptListLeavesTotalAbsent();
    rejectsNegativeReceiptPaginationMetadata();
    rejectsNegativeReceiptArtifactAndSequenceFields();
    rejectsBlankReceiptIdentityFields();
    System.out.println("[IrohaAndroid] SoracloudPrivateUploadedModelJsonParserTests passed.");
  }

  private static void parsesPrivateExecuteResponseAndReceiptInstruction() {
    final SoracloudPrivateUploadedModelExecuteResponse response =
        SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(bytes(executeResponseJson()));

    assert response.schemaVersion() == 1L : "schema version";
    assert "finalized".equals(response.status().get("status")) : "status";
    assert "receipt-1".equals(response.receipt().receiptId()) : "receipt id";
    assert "portal".equals(response.receipt().serviceName()) : "service";
    assert "input".equals(response.receipt().inputArtifact().artifactRole()) : "input role";
    assert "output".equals(response.receipt().outputArtifact().artifactRole()) : "output role";
    assert response.receipt().emittedSequence() == 17L : "sequence";
    assert SoracloudPrivateUploadedModelJsonParser.PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID
        .equals(response.receiptInstruction().wireId()) : "receipt instruction wire id";
    assert "0a0b0c".equals(response.receiptInstruction().payloadHex()) : "receipt payload";
  }

  private static void parsesPrivateReceiptListPaginationMetadata() {
    final String json = "{"
        + "\"schema_version\":1,"
        + "\"receipts\":[" + receiptJson() + "],"
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
  }

  private static void rejectsMissingOrMalformedReceiptInstruction() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.privateUploadedModelReceiptInstruction(
            List.of(new SoracloudTxInstruction("other", "0a"))),
        "expected missing receipt instruction rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.privateUploadedModelReceiptInstruction(
            List.of(new SoracloudTxInstruction(
                SoracloudPrivateUploadedModelJsonParser.PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID,
                "zz"))),
        "expected malformed receipt instruction rejection");
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

  private static void rejectsNegativeReceiptArtifactAndSequenceFields() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"ciphertext_bytes\":64", "\"ciphertext_bytes\":-1"))),
        "expected negative ciphertext_bytes rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"emitted_sequence\":17", "\"emitted_sequence\":-1"))),
        "expected negative emitted_sequence rejection");
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
        + "\"receipt\":" + receiptJson() + ","
        + "\"tx_instructions\":[{"
        + "\"wire_id\":\""
        + SoracloudPrivateUploadedModelJsonParser.PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID
        + "\","
        + "\"payload_hex\":\"0a0b0c\""
        + "}]"
        + "}";
  }

  private static String receiptJson() {
    return "{"
        + "\"schema_version\":1,"
        + "\"receipt_id\":\"receipt-1\","
        + "\"service_name\":\"portal\","
        + "\"model_id\":\"upload-1\","
        + "\"weight_version\":\"v1\","
        + "\"runtime_version\":\"soracloud.private.quantized_cpu.v1\","
        + "\"model_manifest_digest\":\"model-manifest\","
        + "\"model_bundle_root\":\"bundle-root\","
        + "\"policy_id\":\"policy-1\","
        + "\"input_artifact\":{"
        + "\"schema_version\":1,"
        + "\"sorafs_manifest_digest\":\"input-manifest\","
        + "\"artifact_hash\":\"input-artifact\","
        + "\"ciphertext_bytes\":64,"
        + "\"artifact_role\":\"input\""
        + "},"
        + "\"output_artifact\":{"
        + "\"schema_version\":1,"
        + "\"sorafs_manifest_digest\":\"output-manifest\","
        + "\"artifact_hash\":\"output-artifact\","
        + "\"ciphertext_bytes\":96,"
        + "\"artifact_role\":\"output\""
        + "},"
        + "\"input_commitment\":\"input-commitment\","
        + "\"output_commitment\":\"output-commitment\","
        + "\"request_commitment\":\"request-commitment\","
        + "\"result_commitment\":\"result-commitment\","
        + "\"emitted_sequence\":17"
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
}
