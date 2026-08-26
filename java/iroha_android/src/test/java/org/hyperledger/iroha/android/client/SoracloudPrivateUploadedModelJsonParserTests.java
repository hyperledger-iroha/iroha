package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.hyperledger.iroha.android.util.HashLiteral;

public final class SoracloudPrivateUploadedModelJsonParserTests {

  private SoracloudPrivateUploadedModelJsonParserTests() {}

  public static void main(final String[] args) {
    parsesReceiptSubmittedPrivateExecuteResponseAndDurableReceipt();
    parsesEveryUncommittedFirstReleaseSubmissionPhase();
    modelsDefensivelyCopyManifestDigests();
    modelsDeeplySnapshotStatus();
    parsesCommittedReplayWithExplicitNullTransactionHash();
    parsesPrivateReceiptListPaginationMetadata();
    boundedReceiptListAcceptsRequiredNullMetadata();
    rejectsReceiptListMissingRequiredNullableFields();
    rejectsNonCanonicalOrContradictoryReceiptCountMetadata();
    rejectsContradictoryReceiptPaginationRelationships();
    rejectsRetiredInstructionSurfaceAndInvalidSubmissionState();
    rejectsMalformedStatusEnvelope();
    rejectsMissingProductionReceiptEvidence();
    rejectsNonCanonicalReceiptNetworkIdentity();
    rejectsMalformedValidatorAndOutputRecipient();
    rejectsLeadingOrTrailingWhitespaceWithoutNormalization();
    enforcesExactServiceName();
    rejectsMismatchedResponseOutputArtifact();
    rejectsNegativeReceiptPaginationMetadata();
    rejectsReceiptPaginationMetadataAboveU32();
    rejectsInvalidReceiptArtifactAndSequenceFields();
    parsesFullUnsignedReceiptCoordinates();
    rejectsInvalidManifestDigests();
    rejectsNonCanonicalHashFieldsAndFingerprintMismatch();
    rejectsBlankReceiptIdentityFields();
    rejectsReceiptListEntriesWithoutPositiveLedgerCoordinates();
    rejectsNonCanonicalReceiptListOrderAndDuplicates();
    directConstructorsEnforceCanonicalContract();
    System.out.println("[IrohaAndroid] SoracloudPrivateUploadedModelJsonParserTests passed.");
  }

  private static void parsesReceiptSubmittedPrivateExecuteResponseAndDurableReceipt() {
    final SoracloudPrivateUploadedModelExecuteResponse response =
        SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(bytes(executeResponseJson()));

    assert response.schemaVersion() == 1L : "schema version";
    assert Long.valueOf(1L).equals(response.status().get("schema_version"))
        : "status schema";
    assert "portal".equals(
        ((Map<?, ?>) response.status().get("bundle")).get("service_name")) : "status bundle";
    assert "artifact-1".equals(
        ((Map<?, ?>) response.status().get("artifact")).get("artifact_id")) : "status artifact";
    assert SoracloudPrivateUploadedModelSubmissionPhase.RECEIPT_SUBMITTED
        == response.submissionPhase() : "submission phase";
    assert TRANSACTION_HASH.equals(response.transactionHash()) : "transaction hash";
    assert NETWORK_ID.equals(response.receipt().networkId()) : "network id";
    assert RECEIPT_ID.equals(response.receipt().receiptId()) : "receipt id";
    assert "portal".equals(response.receipt().serviceName()) : "service";
    assert "2026.1".equals(response.receipt().serviceVersion()) : "service version";
    assert "decrypt-upload-1".equals(response.receipt().decryptionRequestId())
        : "decryption request";
    assert response.receipt().attestingValidator().laneId() == 0L : "validator lane";
    assert VALIDATOR_ACCOUNT_ID.equals(
        response.receipt().attestingValidator().validatorAccountId()) : "validator account";
    assert VALIDATOR_PEER_ID.equals(response.receipt().attestingValidator().peerId())
        : "validator peer";
    assert "input".equals(response.receipt().inputArtifact().artifactRole()) : "input role";
    assert "output".equals(response.receipt().outputArtifact().artifactRole()) : "output role";
    assert response.receipt().outputReplicationOrderId().length == 32
        : "output replication order id width";
    assert response.receipt().inputArtifact().sorafsRootCid().size() == 36 : "root CID width";
    assert response.receipt().inputArtifact().sorafsRootCid().subList(0, 4)
        .equals(Arrays.asList(1, 113, 31, 32)) : "root CID framing";
    assert Arrays.equals(
        filledDigest(0x11), response.receipt().modelManifestDigest()) : "model manifest digest";
    assert Arrays.equals(
        OUTPUT_REPLICATION_ORDER_ID, response.receipt().outputReplicationOrderId())
        : "output replication order id";
    assert (response.receipt().outputReplicationOrderId()[0] & 0x80) == 0x80
        : "automatic replication-order namespace tag";
    assert Arrays.equals(
        filledDigest(0x33), response.outputArtifact().sorafsManifestDigest())
        : "response output";
    assert "recipient-key".equals(response.receipt().outputRecipient().keyId()) : "recipient";
    assert response.receipt().outputRecipient().publicKeyBytes().length == 32 : "recipient key";
    assert BigInteger.ZERO.equals(response.receipt().emittedSequence())
        : "submission sequence sentinel";
    assert BigInteger.ZERO.equals(response.receipt().authorizationClaimBlockHeight())
        : "submission authorization height sentinel";
    assert BigInteger.ZERO.equals(response.receipt().authorizationClaimEpoch())
        : "submission authorization epoch sentinel";
    assert BigInteger.ZERO.equals(response.receipt().emittedBlockHeight())
        : "submission height sentinel";
    assert BigInteger.ZERO.equals(response.receipt().emittedEpoch())
        : "submission epoch sentinel";
  }

  private static void modelsDefensivelyCopyManifestDigests() {
    final SoracloudPrivateUploadedModelExecuteResponse response =
        SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(bytes(executeResponseJson()));

    final byte[] artifactSource = filledDigest(0x44);
    final SoracloudPrivateModelArtifactRef artifact =
        artifactWithManifestDigest(response.outputArtifact(), artifactSource);
    artifactSource[0] = 0;
    assert artifact.sorafsManifestDigest()[0] == (byte) 0x44
        : "artifact constructor must copy manifest digest";
    final byte[] artifactView = artifact.sorafsManifestDigest();
    artifactView[1] = 0;
    assert artifact.sorafsManifestDigest()[1] == (byte) 0x44
        : "artifact getter must copy manifest digest";

    final byte[] receiptSource = filledDigest(0x55);
    final SoracloudPrivateUploadedModelExecutionReceipt receipt =
        receiptWithManifestDigest(response.receipt(), receiptSource);
    receiptSource[0] = 0;
    assert receipt.modelManifestDigest()[0] == (byte) 0x55
        : "receipt constructor must copy manifest digest";
    final byte[] receiptView = receipt.modelManifestDigest();
    receiptView[1] = 0;
    assert receipt.modelManifestDigest()[1] == (byte) 0x55
        : "receipt getter must copy manifest digest";

    final byte[] replicationOrderSource = OUTPUT_REPLICATION_ORDER_ID.clone();
    final SoracloudPrivateUploadedModelExecutionReceipt receiptWithReplicationOrder =
        receiptWithReplicationOrder(response.receipt(), replicationOrderSource);
    replicationOrderSource[0] = 0;
    assert receiptWithReplicationOrder.outputReplicationOrderId()[0]
        == OUTPUT_REPLICATION_ORDER_ID[0]
        : "receipt constructor must copy output replication order id";
    final byte[] replicationOrderView =
        receiptWithReplicationOrder.outputReplicationOrderId();
    replicationOrderView[1] = 0;
    assert receiptWithReplicationOrder.outputReplicationOrderId()[1]
        == OUTPUT_REPLICATION_ORDER_ID[1]
        : "receipt getter must copy output replication order id";

    assertThrows(
        () -> artifactWithManifestDigest(response.outputArtifact(), new byte[31]),
        "expected short artifact manifest digest rejection");
    assertThrows(
        () -> artifactWithManifestDigest(response.outputArtifact(), null),
        "expected null artifact manifest digest rejection");
    assertThrows(
        () -> receiptWithManifestDigest(response.receipt(), new byte[33]),
        "expected long receipt manifest digest rejection");
    assertThrows(
        () -> receiptWithManifestDigest(response.receipt(), null),
        "expected null receipt manifest digest rejection");
    assertThrows(
        () -> receiptWithReplicationOrder(response.receipt(), new byte[31]),
        "expected short output replication order rejection");
    assertThrows(
        () -> receiptWithReplicationOrder(
            response.receipt(), mismatchingOutputReplicationOrderId()),
        "expected mismatched output replication order rejection");
  }

  @SuppressWarnings("unchecked")
  private static void modelsDeeplySnapshotStatus() {
    final SoracloudPrivateUploadedModelExecuteResponse parsed =
        SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(bytes(executeResponseJson()));
    final Map<String, Object> bundle =
        new LinkedHashMap<>((Map<String, Object>) parsed.status().get("bundle"));
    final List<Object> modalities =
        new ArrayList<>((List<Object>) bundle.get("modalities"));
    bundle.put("modalities", modalities);
    final Map<String, Object> artifact =
        new LinkedHashMap<>((Map<String, Object>) parsed.status().get("artifact"));
    final Map<String, Object> status = new LinkedHashMap<>();
    status.put("schema_version", Long.valueOf(1L));
    status.put("bundle", bundle);
    status.put("artifact", artifact);

    final SoracloudPrivateUploadedModelExecuteResponse response =
        new SoracloudPrivateUploadedModelExecuteResponse(
            1L,
            status,
            parsed.submissionPhase(),
            parsed.transactionHash(),
            parsed.receipt(),
            parsed.outputArtifact());
    modalities.add("image");
    bundle.put("service_name", "mutated");
    artifact.clear();
    status.put("legacy", Boolean.TRUE);

    final Map<?, ?> snapshotBundle = (Map<?, ?>) response.status().get("bundle");
    assert "portal".equals(snapshotBundle.get("service_name")) : "status object snapshot";
    assert ((List<?>) snapshotBundle.get("modalities")).size() == 1 : "status list snapshot";
    assert "artifact-1".equals(
        ((Map<?, ?>) response.status().get("artifact")).get("artifact_id"))
        : "status artifact snapshot";
    assertThrows(
        () -> response.status().put("legacy", Boolean.TRUE),
        "expected immutable status map");
    assertThrows(
        () -> ((Map<String, Object>) response.status().get("bundle")).put("legacy", true),
        "expected immutable nested status map");
    assertThrows(
        () -> ((List<Object>) snapshotBundle.get("modalities")).add("image"),
        "expected immutable nested status list");

    final Map<String, Object> cyclicBundle = new LinkedHashMap<>();
    cyclicBundle.put("self", cyclicBundle);
    final Map<String, Object> cyclicStatus = new LinkedHashMap<>();
    cyclicStatus.put("schema_version", Long.valueOf(1L));
    cyclicStatus.put("bundle", cyclicBundle);
    cyclicStatus.put("artifact", null);
    assertThrows(
        () -> new SoracloudPrivateUploadedModelExecuteResponse(
            1L,
            cyclicStatus,
            parsed.submissionPhase(),
            parsed.transactionHash(),
            parsed.receipt(),
            parsed.outputArtifact()),
        "expected cyclic status rejection");

    final Map<String, Object> nonfiniteBundle = new LinkedHashMap<>();
    nonfiniteBundle.put("score", Double.NaN);
    final Map<String, Object> nonfiniteStatus = new LinkedHashMap<>();
    nonfiniteStatus.put("schema_version", Long.valueOf(1L));
    nonfiniteStatus.put("bundle", nonfiniteBundle);
    nonfiniteStatus.put("artifact", null);
    assertThrows(
        () -> new SoracloudPrivateUploadedModelExecuteResponse(
            1L,
            nonfiniteStatus,
            parsed.submissionPhase(),
            parsed.transactionHash(),
            parsed.receipt(),
            parsed.outputArtifact()),
        "expected non-finite status number rejection");
  }

  private static void parsesCommittedReplayWithExplicitNullTransactionHash() {
    final SoracloudPrivateUploadedModelExecuteResponse response =
        SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(
                executeResponseJson()
                    .replace(
                        "\"submission_phase\":\"receipt_submitted\"",
                        "\"submission_phase\":\"committed\"")
                    .replace(
                        "\"transaction_hash\":\"" + TRANSACTION_HASH + "\"",
                        "\"transaction_hash\":null")
                    .replace(
                        "\"authorization_claim_block_height\":0",
                        "\"authorization_claim_block_height\":499")
                    .replace(
                        "\"authorization_claim_epoch\":0",
                        "\"authorization_claim_epoch\":1699999900")
                    .replace("\"emitted_sequence\":0", "\"emitted_sequence\":17")
                    .replace("\"emitted_block_height\":0", "\"emitted_block_height\":501")
                    .replace("\"emitted_epoch\":0", "\"emitted_epoch\":1700000000")));

    assert SoracloudPrivateUploadedModelSubmissionPhase.COMMITTED
        == response.submissionPhase() : "committed replay";
    assert response.transactionHash() == null : "committed transaction hash";
    assert BigInteger.valueOf(17L).equals(response.receipt().emittedSequence())
        : "committed sequence";
    assert BigInteger.valueOf(501L).equals(response.receipt().emittedBlockHeight())
        : "committed height";
    assert BigInteger.valueOf(1_700_000_000L).equals(response.receipt().emittedEpoch())
        : "committed epoch";
  }

  private static void parsesEveryUncommittedFirstReleaseSubmissionPhase() {
    final SoracloudPrivateUploadedModelExecuteResponse awaiting =
        SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(
                executeResponseJson()
                    .replace(
                        "\"submission_phase\":\"receipt_submitted\"",
                        "\"submission_phase\":\"awaiting_output_durability\"")
                    .replace(
                        "\"transaction_hash\":\"" + TRANSACTION_HASH + "\"",
                        "\"transaction_hash\":null")));
    assert SoracloudPrivateUploadedModelSubmissionPhase.AWAITING_OUTPUT_DURABILITY
        == awaiting.submissionPhase() : "awaiting output durability";
    assert awaiting.transactionHash() == null : "awaiting transaction hash";

    final SoracloudPrivateUploadedModelExecuteResponse prepareSubmitted =
        SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(
                executeResponseJson()
                    .replace(
                        "\"submission_phase\":\"receipt_submitted\"",
                        "\"submission_phase\":\"prepare_submitted\"")));
    assert SoracloudPrivateUploadedModelSubmissionPhase.PREPARE_SUBMITTED
        == prepareSubmitted.submissionPhase() : "prepare submitted";
    assert TRANSACTION_HASH.equals(prepareSubmitted.transactionHash())
        : "prepare transaction hash";
  }

  private static void parsesPrivateReceiptListPaginationMetadata() {
    final String json = "{"
        + "\"schema_version\":1,"
        + "\"receipts\":["
        + receiptJson()
            .replace(
                "\"authorization_claim_block_height\":0",
                "\"authorization_claim_block_height\":499")
            .replace(
                "\"authorization_claim_epoch\":0",
                "\"authorization_claim_epoch\":1699999900")
            .replace("\"emitted_sequence\":0", "\"emitted_sequence\":17")
            .replace("\"emitted_block_height\":0", "\"emitted_block_height\":501")
            .replace("\"emitted_epoch\":0", "\"emitted_epoch\":1700000000")
        + "],"
        + "\"total\":3,"
        + "\"returned_items\":1,"
        + "\"remaining_items\":2,"
        + "\"has_more\":true,"
        + "\"count_mode\":\"exact\","
        + "\"continue_cursor\":\"" + RECEIPT_CURSOR + "\""
        + "}";
    final SoracloudPrivateUploadedModelReceiptListResponse response =
        SoracloudPrivateUploadedModelJsonParser.parseReceiptList(bytes(json));

    assert response.receipts().size() == 1 : "receipt count";
    assert Long.valueOf(3L).equals(response.total()) : "total";
    assert response.returnedItems() == 1L : "returned";
    assert response.remainingItems() == 2L : "remaining";
    assert "exact".equals(response.countMode()) : "count mode";
    assert RECEIPT_CURSOR.equals(response.continueCursor()) : "continue cursor";
    assert "2026.1".equals(response.receipts().get(0).serviceVersion())
        : "list receipt service version";
  }

  private static void boundedReceiptListAcceptsRequiredNullMetadata() {
    final SoracloudPrivateUploadedModelReceiptListResponse response =
        SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes("{"
                + "\"schema_version\":1,"
                + "\"receipts\":[],"
                + "\"total\":null,"
                + "\"returned_items\":0,"
                + "\"remaining_items\":null,"
                + "\"has_more\":false,"
                + "\"count_mode\":\"bounded\","
                + "\"continue_cursor\":null"
                + "}"));

    assert response.total() == null : "bounded total null";
    assert response.remainingItems() == null : "bounded remaining count null";
    assert !response.hasMore() : "has more";
    assert response.continueCursor() == null : "continue cursor null";
  }

  private static void rejectsReceiptListMissingRequiredNullableFields() {
    final String canonical = "{"
        + "\"schema_version\":1,"
        + "\"receipts\":[],"
        + "\"total\":null,"
        + "\"returned_items\":0,"
        + "\"remaining_items\":null,"
        + "\"has_more\":false,"
        + "\"count_mode\":\"bounded\","
        + "\"continue_cursor\":null"
        + "}";
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(canonical.replace("\"total\":null,", ""))),
        "expected missing total key rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(canonical.replace(",\"continue_cursor\":null", ""))),
        "expected missing continue_cursor key rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(canonical.replace("\"remaining_items\":null,", ""))),
        "expected missing remaining_items key rejection");
  }

  private static void rejectsNonCanonicalOrContradictoryReceiptCountMetadata() {
    final String exact = receiptListJson("0", "0", "0");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(exact.replace("\"count_mode\":\"exact\"", "\"count_mode\":\"EXACT\""))),
        "expected uppercase count mode alias rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(exact.replace("\"count_mode\":\"exact\"", "\"count_mode\":\"full\""))),
        "expected unknown count mode rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(receiptListJson("null", "0", "0"))),
        "expected exact count mode with null total rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(exact.replace("\"count_mode\":\"exact\"", "\"count_mode\":\"bounded\""))),
        "expected bounded count mode with non-null total rejection");
  }

  private static void rejectsContradictoryReceiptPaginationRelationships() {
    assertThrows(
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                bytes(receiptListJson("1", "1", "0"))),
        "expected returned_items/receipts mismatch rejection");
    assertThrows(
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                bytes(
                    withMore(receiptListJson("1", "1", "1"))
                        .replace(
                            "\"receipts\":[]",
                            "\"receipts\":[" + committedReceiptJson(RECEIPT_ID, 1L, 1L) + "]"))),
        "expected exact total below current suffix rejection");
    assertThrows(
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                bytes(receiptListJson("1", "0", "1"))),
        "expected false has_more with remaining items rejection");
    assertThrows(
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                bytes(
                    withMore(receiptListJson("0", "0", "0")))),
        "expected true has_more without remaining items rejection");

    final SoracloudPrivateUploadedModelReceiptListResponse saturated =
        SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(
                withMore(receiptListJson("4294967295", "0", "4294967295"))));
    assert saturated.total().longValue() == 4_294_967_295L : "saturated total";
  }

  private static void rejectsRetiredInstructionSurfaceAndInvalidSubmissionState() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"output_artifact\":", "\"tx_instructions\":[],\"output_artifact\":"))),
        "expected retired tx_instructions rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"submission_phase\"", "\"submission_status\""))),
        "expected retired submission_status rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                "\"submission_phase\":\"receipt_submitted\"",
                "\"submission_phase\":\"pending\""))),
        "expected unknown submission phase rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                "\"transaction_hash\":\"" + TRANSACTION_HASH + "\"",
                "\"transaction_hash\":null"))),
        "expected receipt-submitted response hash rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                "\"submission_phase\":\"receipt_submitted\"",
                "\"submission_phase\":\"awaiting_output_durability\""))),
        "expected awaiting response non-null hash rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(
                executeResponseJson()
                    .replace(
                        "\"submission_phase\":\"receipt_submitted\"",
                        "\"submission_phase\":\"prepare_submitted\"")
                    .replace(
                        "\"transaction_hash\":\"" + TRANSACTION_HASH + "\"",
                        "\"transaction_hash\":null"))),
        "expected prepare-submitted response hash rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                "\"submission_phase\":\"receipt_submitted\"",
                "\"submission_phase\":\"committed\""))),
        "expected committed response non-null hash rejection");
  }

  private static void rejectsMalformedStatusEnvelope() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                statusJson(), statusJson().replaceFirst(
                    "\\{\"schema_version\":1", "{\"schema_version\":1,\"legacy\":true")))),
        "expected unknown status field rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                statusJson(), statusJson().replace("\"bundle\":", "\"legacy_bundle\":")))),
        "expected missing status bundle rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(statusJson(), statusJson().replace(
                "\"schema_version\":1", "\"schema_version\":2")))),
        "expected invalid status schema rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                statusJson(),
                statusJson().replace(
                    "\"artifact\":" + artifactStatusJson(), "\"artifact\":\"legacy\"")))),
        "expected non-object status artifact rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                statusJson(),
                statusJson().replace(
                    "\"family\":\"decoder-only\"",
                    "\"family\":\"decoder-only\",\"legacy\":true")))),
        "expected unknown bundle field rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                statusJson(),
                statusJson().replace(
                    "\"artifact_id\":\"artifact-1\"",
                    "\"artifact_id\":\"artifact-1\",\"legacy\":true")))),
        "expected unknown artifact field rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                statusJson(), statusJson().replace("\"modalities\":[\"text\"]", "\"modalities\":\"text\"")))),
        "expected invalid bundle field type rejection");
    final String[][] mismatches = {
        {"\"service_name\":\"portal\"", "\"service_name\":\"other\""},
        {"\"model_id\":\"upload-1\"", "\"model_id\":\"upload-2\""},
        {"\"weight_version\":\"v1\"", "\"weight_version\":\"v2\""},
        {"\"bundle_root\":\"" + MODEL_BUNDLE_ROOT + "\"",
            "\"bundle_root\":\"" + TRANSACTION_HASH + "\""},
        {"\"sorafs_manifest_digest\":" + fixedBytesJson(0x11, 32),
            "\"sorafs_manifest_digest\":" + fixedBytesJson(0x12, 32)}
    };
    for (final String[] mismatch : mismatches) {
      final String mismatchedStatus = statusJson().replace(mismatch[0], mismatch[1]);
      assertThrows(
          () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
              bytes(executeResponseJson().replace(statusJson(), mismatchedStatus))),
          "expected status/receipt binding mismatch rejection");
    }
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
            bytes(executeResponseJson().replace(
                "\"output_replication_order_id\":"
                    + fixedBytesJson(OUTPUT_REPLICATION_ORDER_ID)
                    + ",",
                ""))),
        "expected missing output_replication_order_id rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(",\"emitted_block_height\":0", ""))),
        "expected missing emitted_block_height rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(",\"emitted_epoch\":0", ""))),
        "expected missing emitted_epoch rejection");
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
            bytes(executeResponseJson().replace(VALIDATOR_ACCOUNT_ID, "validator@public"))),
        "expected non-I105 validator account rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                VALIDATOR_PEER_ID, "ed25519:" + VALIDATOR_PEER_ID))),
        "expected prefixed peer alias rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(VALIDATOR_PEER_ID, OTHER_VALIDATOR_PEER_ID))),
        "expected validator account and peer mismatch rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                VALIDATOR_ACCOUNT_ID, MULTISIG_VALIDATOR_ACCOUNT_ID))),
        "expected multisig validator account rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(PUBLIC_KEY_BASE64, "not-base64"))),
        "expected malformed recipient key rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(PUBLIC_KEY_BASE64, ZERO_X25519_KEY_BASE64))),
        "expected low-order recipient key rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("X25519HkdfSha256", "UnknownKem"))),
        "expected unsupported KEM rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"value\":null", "\"value\":{}"))),
        "expected non-unit suite value rejection");
  }

  private static void rejectsLeadingOrTrailingWhitespaceWithoutNormalization() {
    final String[][] paddedFields = {
        {
          "\"submission_phase\":\"receipt_submitted\"",
          "\"submission_phase\":\"receipt_submitted \""
        },
        {"\"service_version\":\"2026.1\"", "\"service_version\":\" 2026.1\""},
        {
          "\"validator_account_id\":\"" + VALIDATOR_ACCOUNT_ID + "\"",
          "\"validator_account_id\":\" " + VALIDATOR_ACCOUNT_ID + "\""
        },
        {
          "\"peer_id\":\"" + VALIDATOR_PEER_ID + "\"",
          "\"peer_id\":\"" + VALIDATOR_PEER_ID + " \""
        }
    };
    for (final String[] paddedField : paddedFields) {
      assertThrows(
          () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
              bytes(executeResponseJson().replace(paddedField[0], paddedField[1]))),
          "expected padded canonical string rejection");
    }
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(receiptListJson("0", "0", "0")
                .replace("\"count_mode\":\"exact\"", "\"count_mode\":\" exact\""))),
        "expected padded count mode rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(receiptListJson("0", "0", "0")
                .replace("\"continue_cursor\":null", "\"continue_cursor\":\" next\""))),
        "expected padded cursor rejection");
    assertThrows(
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                bytes(
                    executeResponseJson()
                        .replace(
                            "\"service_version\":\"2026.1\"",
                            "\"service_version\":\"2026\\n1\""))),
        "expected embedded control character rejection");
  }

  private static void enforcesExactServiceName() {
    final String composedServiceName = "caf\u00e9";
    final String decomposedServiceName = "cafe\u0301";
    final SoracloudPrivateUploadedModelExecuteResponse composed =
        SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(
                executeResponseJson()
                    .replace(
                        "\"service_name\":\"portal\"",
                        "\"service_name\":\"" + composedServiceName + "\"")));
    assert composedServiceName.equals(composed.receipt().serviceName())
        : "canonical NFC service name";
    assertThrows(
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                bytes(
                    executeResponseJson()
                        .replace(
                            "\"service_name\":\"portal\"",
                            "\"service_name\":\"" + decomposedServiceName + "\""))),
        "expected non-NFC service name rejection");
    assertThrows(
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                bytes(
                    executeResponseJson()
                        .replace(
                            "\"service_name\":\"portal\"",
                            "\"service_name\":\"portal#alias\""))),
        "expected forbidden Iroha Name character rejection");
  }

  private static void rejectsMismatchedResponseOutputArtifact() {
    final String canonical = executeResponseJson();
    final String target = fixedBytesJson(0x33, 32);
    final int responseOutput = canonical.lastIndexOf(target);
    final String mismatched = canonical.substring(0, responseOutput)
        + fixedBytesJson(0x34, 32)
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

  private static void rejectsReceiptPaginationMetadataAboveU32() {
    final String u32MaxPlusOne = "4294967296";
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(receiptListJson(u32MaxPlusOne, "0", "0"))),
        "expected total above u32 rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(receiptListJson("0", u32MaxPlusOne, "0"))),
        "expected returned_items above u32 rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(receiptListJson("0", "0", u32MaxPlusOne))),
        "expected remaining_items above u32 rejection");
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
            bytes(executeResponseJson().replace(
                "\"ciphertext_bytes\":64", "\"ciphertext_bytes\":75497473"))),
        "expected ciphertext_bytes above 72 MiB rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                "\"authorization_claim_block_height\":0,", ""))),
        "expected missing authorization_claim_block_height rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                "\"authorization_claim_epoch\":0", "\"authorization_claim_epoch\":-1"))),
        "expected negative authorization_claim_epoch rejection");
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
            bytes(executeResponseJson().replace("\"emitted_epoch\":0", "\"emitted_epoch\":-1"))),
        "expected negative emitted_epoch rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"emitted_sequence\":0", "\"emitted_sequence\":1"))),
        "expected mixed zero and positive ledger coordinates rejection");
    assertThrows(
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                bytes(
                    executeResponseJson()
                        .replace(
                            "\"submission_phase\":\"receipt_submitted\"",
                            "\"submission_phase\":\"committed\"")
                        .replace(
                            "\"transaction_hash\":\"" + TRANSACTION_HASH + "\"",
                            "\"transaction_hash\":null")
                        .replace(
                            "\"authorization_claim_block_height\":0",
                            "\"authorization_claim_block_height\":502")
                        .replace(
                            "\"authorization_claim_epoch\":0",
                            "\"authorization_claim_epoch\":1700000001")
                        .replace("\"emitted_sequence\":0", "\"emitted_sequence\":17")
                        .replace(
                            "\"emitted_block_height\":0",
                            "\"emitted_block_height\":501")
                        .replace(
                            "\"emitted_epoch\":0", "\"emitted_epoch\":1700000000"))),
        "expected emission-before-authorization rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                "\"emitted_sequence\":0", "\"emitted_sequence\":18446744073709551616"))),
        "expected emitted_sequence above u64 rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                "\"emitted_sequence\":0", "\"emitted_sequence\":1.0"))),
        "expected non-integer emitted_sequence rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"artifact_role\":\"input\"", "\"artifact_role\":\"output\""))),
        "expected swapped artifact role rejection");
  }

  private static void parsesFullUnsignedReceiptCoordinates() {
    final String u64Max = SoracloudPrivateModelValidation.U64_MAX.toString();
    final SoracloudPrivateUploadedModelExecuteResponse response =
        SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(
                executeResponseJson()
                    .replace(
                        "\"submission_phase\":\"receipt_submitted\"",
                        "\"submission_phase\":\"committed\"")
                    .replace(
                        "\"transaction_hash\":\"" + TRANSACTION_HASH + "\"",
                        "\"transaction_hash\":null")
                    .replace(
                        "\"authorization_claim_block_height\":0",
                        "\"authorization_claim_block_height\":" + u64Max)
                    .replace(
                        "\"authorization_claim_epoch\":0",
                        "\"authorization_claim_epoch\":" + u64Max)
                    .replace("\"emitted_sequence\":0", "\"emitted_sequence\":" + u64Max)
                    .replace(
                        "\"emitted_block_height\":0",
                        "\"emitted_block_height\":" + u64Max)
                    .replace("\"emitted_epoch\":0", "\"emitted_epoch\":" + u64Max)));

    assert SoracloudPrivateModelValidation.U64_MAX.equals(
        response.receipt().emittedSequence()) : "maximum u64 sequence";
    assert SoracloudPrivateModelValidation.U64_MAX.equals(
        response.receipt().authorizationClaimBlockHeight()) : "maximum u64 claim height";
    assert SoracloudPrivateModelValidation.U64_MAX.equals(
        response.receipt().authorizationClaimEpoch()) : "maximum u64 claim epoch";
    assert SoracloudPrivateModelValidation.U64_MAX.equals(
        response.receipt().emittedBlockHeight()) : "maximum u64 height";
    assert SoracloudPrivateModelValidation.U64_MAX.equals(
        response.receipt().emittedEpoch()) : "maximum u64 epoch";
  }

  private static void rejectsInvalidManifestDigests() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                fixedBytesJson(0x11, 32), fixedBytesJson(0x11, 31)))),
        "expected short model manifest digest rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                fixedBytesJson(0x22, 32), fixedBytesJson(0x22, 33)))),
        "expected long artifact manifest digest rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                fixedBytesJson(0x33, 32), fixedBytesJson(256, 32)))),
        "expected out-of-range manifest digest byte rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                fixedBytesJson(0x11, 32), "\"model-manifest\""))),
        "expected string manifest digest rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                fixedBytesJson(0x11, 32), fixedBytesJson(0x11, 32).replaceFirst("17", "17.0")))),
        "expected non-integer manifest digest byte rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                fixedBytesJson(OUTPUT_REPLICATION_ORDER_ID),
                fixedBytesJson(Arrays.copyOf(OUTPUT_REPLICATION_ORDER_ID, 31))))),
        "expected short output replication order rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                fixedBytesJson(OUTPUT_REPLICATION_ORDER_ID),
                fixedBytesJson(mismatchingOutputReplicationOrderId())))),
        "expected mismatched output replication order rejection");
  }

  private static void rejectsNonCanonicalHashFieldsAndFingerprintMismatch() {
    final String[][] hashFields = {
        {"transaction_hash", TRANSACTION_HASH},
        {"receipt_id", RECEIPT_ID},
        {"model_bundle_root", MODEL_BUNDLE_ROOT},
        {"artifact_hash", INPUT_ARTIFACT_HASH},
        {"artifact_hash", OUTPUT_ARTIFACT_HASH},
        {"input_commitment", INPUT_COMMITMENT},
        {"output_commitment", OUTPUT_COMMITMENT},
        {"request_commitment", REQUEST_COMMITMENT},
        {"result_commitment", RESULT_COMMITMENT},
        {"public_key_fingerprint", PUBLIC_KEY_FINGERPRINT}
    };
    for (final String[] hashField : hashFields) {
      assertHashFieldRejected(
          hashField[0],
          hashField[1],
          "not-a-hash",
          "expected non-canonical " + hashField[0] + " rejection");
    }
    assertHashFieldRejected(
        "receipt_id",
        RECEIPT_ID,
        RECEIPT_ID.toLowerCase(java.util.Locale.ROOT),
        "expected lowercase receipt_id alias rejection");
    assertHashFieldRejected(
        "receipt_id",
        RECEIPT_ID,
        tamperChecksum(RECEIPT_ID),
        "expected receipt_id checksum rejection");
    assertHashFieldRejected(
        "receipt_id",
        RECEIPT_ID,
        UNMARKED_HASH_LITERAL,
        "expected unmarked receipt_id rejection");
    assertHashFieldRejected(
        "receipt_id",
        RECEIPT_ID,
        ZERO_PREHASH_SENTINEL,
        "expected zero-prehash receipt_id rejection");
    assertHashFieldRejected(
        "public_key_fingerprint",
        PUBLIC_KEY_FINGERPRINT,
        RESULT_COMMITMENT,
        "expected public key fingerprint mismatch rejection");
  }

  private static void rejectsBlankReceiptIdentityFields() {
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace(
                "\"receipt_id\":\"" + RECEIPT_ID + "\"", "\"receipt_id\":\"   \""))),
        "expected blank receipt_id rejection");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(executeResponseJson().replace("\"policy_id\":\"policy-1\"", "\"policy_id\":\"\""))),
        "expected blank policy_id rejection");
  }

  private static void rejectsReceiptListEntriesWithoutPositiveLedgerCoordinates() {
    final String zeroCoordinates = receiptJson();
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(receiptListJson("1", "1", "0")
                .replace("\"receipts\":[]", "\"receipts\":[" + zeroCoordinates + "]"))),
        "expected zero-coordinate receipt-list entry rejection");

    final String positiveSequenceOnly =
        receiptJson().replace("\"emitted_sequence\":0", "\"emitted_sequence\":1");
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            bytes(receiptListJson("1", "1", "0")
                .replace(
                    "\"receipts\":[]", "\"receipts\":[" + positiveSequenceOnly + "]"))),
        "expected mixed-coordinate receipt-list entry rejection");
  }

  private static void rejectsNonCanonicalReceiptListOrderAndDuplicates() {
    final String first = committedReceiptJson(RECEIPT_ID, 1L, 101L);
    final String second = committedReceiptJson(MODEL_BUNDLE_ROOT, 2L, 102L);
    assertThrows(
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                bytes(
                    receiptListJson("2", "2", "0")
                        .replace("\"receipts\":[]", "\"receipts\":[" + second + "," + first + "]"))),
        "expected descending receipt sequence rejection");
    assertThrows(
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                bytes(
                    receiptListJson("2", "2", "0")
                        .replace("\"receipts\":[]", "\"receipts\":[" + first + "," + first + "]"))),
        "expected duplicate receipt rejection");

    final String higherId = committedReceiptJson(MODEL_BUNDLE_ROOT, 1L, 101L);
    assertThrows(
        () ->
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                bytes(
                    receiptListJson("2", "2", "0")
                        .replace("\"receipts\":[]", "\"receipts\":[" + higherId + "," + first + "]"))),
        "expected descending same-sequence receipt_id rejection");
  }

  private static void directConstructorsEnforceCanonicalContract() {
    final SoracloudPrivateUploadedModelExecuteResponse parsed =
        SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(bytes(executeResponseJson()));
    final SoracloudPrivateUploadedModelExecutionReceipt receipt = parsed.receipt();
    final SoracloudPrivateModelArtifactRef output = parsed.outputArtifact();

    assertThrows(
        () -> new SoracloudPrivateModelArtifactRef(
            2L,
            output.sorafsManifestDigest(),
            output.sorafsRootCid(),
            output.artifactHash(),
            output.ciphertextBytes(),
            output.artifactRole()),
        "expected direct artifact schema rejection");
    assertThrows(
        () -> new SoracloudPrivateModelArtifactRef(
            1L,
            output.sorafsManifestDigest(),
            output.sorafsRootCid(),
            output.artifactHash(),
            75_497_473L,
            output.artifactRole()),
        "expected direct artifact size rejection");
    assertThrows(
        () ->
            new SoracloudPrivateModelArtifactRef(
                1L,
                output.sorafsManifestDigest(),
                output.sorafsRootCid(),
                output.artifactHash(),
                output.ciphertextBytes(),
                "plaintext"),
        "expected direct artifact role rejection");
    assertThrows(
        () ->
            new SoracloudPrivateModelArtifactRef(
                1L,
                output.sorafsManifestDigest(),
                output.sorafsRootCid(),
                ZERO_PREHASH_SENTINEL,
                output.ciphertextBytes(),
                output.artifactRole()),
        "expected direct zero-prehash artifact hash rejection");
    assertThrows(
        () -> new SoracloudRuntimeDeterministicValidatorHost(
            0L, VALIDATOR_ACCOUNT_ID, OTHER_VALIDATOR_PEER_ID),
        "expected direct validator identity mismatch rejection");
    assertThrows(
        () -> new SoracloudUploadedModelEncryptionRecipient(
            1L,
            "recipient-key",
            1L,
            "X25519HkdfSha256",
            "Aes256Gcm",
            PUBLIC_KEY_BASE64,
            RESULT_COMMITMENT),
        "expected direct recipient fingerprint mismatch rejection");
    assertThrows(
        () -> receiptWithCoordinates(
            receipt, BigInteger.ONE, BigInteger.ZERO, BigInteger.ZERO),
        "expected direct receipt mixed coordinates rejection");
    assertThrows(
        () -> receiptWithCoordinates(
            receipt,
            SoracloudPrivateModelValidation.U64_MAX.add(BigInteger.ONE),
            SoracloudPrivateModelValidation.U64_MAX,
            SoracloudPrivateModelValidation.U64_MAX),
        "expected direct receipt coordinate above u64 rejection");
    assertThrows(
        () -> receiptWithServiceName(receipt, " portal"),
        "expected direct receipt padded string rejection");
    assertThrows(
        () -> receiptWithServiceName(receipt, "cafe\u0301"),
        "expected direct receipt non-NFC service name rejection");
    assertThrows(
        () -> receiptWithSelectors(receipt, receipt.serviceVersion(), "model id", "v1"),
        "expected direct receipt noncanonical model id rejection");
    assertThrows(
        () -> receiptWithSelectors(receipt, receipt.serviceVersion(), "model-id", "v/1"),
        "expected direct receipt noncanonical weight version rejection");
    assertThrows(
        () -> receiptWithSelectors(receipt, repeated('v', 257), "model-id", "v1"),
        "expected direct receipt oversized service version rejection");
    final SoracloudPrivateUploadedModelExecuteResponse awaitingDurability =
        new SoracloudPrivateUploadedModelExecuteResponse(
            1L,
            parsed.status(),
            SoracloudPrivateUploadedModelSubmissionPhase.RECEIPT_SUBMITTED,
            null,
            receipt,
            output);
    assert awaitingDurability.transactionHash() == null
        : "direct awaiting-durability response hash";
    assertThrows(
        () -> new SoracloudPrivateUploadedModelReceiptListResponse(
            1L,
            Collections.emptyList(),
            null,
            0L,
            0L,
            false,
            "exact",
            null),
        "expected direct exact list without total rejection");
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
        + "\"count_mode\":\"exact\","
        + "\"continue_cursor\":null"
        + "}";
  }

  private static String withMore(final String json) {
    return json
        .replace("\"has_more\":false", "\"has_more\":true")
        .replace("\"continue_cursor\":null", "\"continue_cursor\":\"" + RECEIPT_CURSOR + "\"");
  }

  private static String committedReceiptJson(
      final String receiptId, final long emittedSequence, final long emittedBlockHeight) {
    return receiptJson()
        .replace("\"receipt_id\":\"" + RECEIPT_ID + "\"", "\"receipt_id\":\"" + receiptId + "\"")
        .replace(
            "\"authorization_claim_block_height\":0",
            "\"authorization_claim_block_height\":" + emittedBlockHeight)
        .replace(
            "\"authorization_claim_epoch\":0",
            "\"authorization_claim_epoch\":" + emittedBlockHeight)
        .replace("\"emitted_sequence\":0", "\"emitted_sequence\":" + emittedSequence)
        .replace(
            "\"emitted_block_height\":0",
            "\"emitted_block_height\":" + emittedBlockHeight)
        .replace("\"emitted_epoch\":0", "\"emitted_epoch\":" + emittedBlockHeight);
  }

  private static String executeResponseJson() {
    return "{"
        + "\"schema_version\":1,"
        + "\"status\":" + statusJson() + ","
        + "\"submission_phase\":\"receipt_submitted\","
        + "\"transaction_hash\":\"" + TRANSACTION_HASH + "\","
        + "\"receipt\":" + receiptJson() + ","
        + "\"output_artifact\":" + outputArtifactJson()
        + "}";
  }

  private static String statusJson() {
    return "{"
        + "\"schema_version\":1,"
        + "\"bundle\":{"
        + "\"schema_version\":1,"
        + "\"service_name\":\"portal\","
        + "\"model_id\":\"upload-1\","
        + "\"weight_version\":\"v1\","
        + "\"family\":\"decoder-only\","
        + "\"modalities\":[\"text\"],"
        + "\"plaintext_root\":\"" + RESULT_COMMITMENT + "\","
        + "\"runtime_format\":{\"runtime_format\":\"DeterministicQuantizedCpuV1\",\"value\":null},"
        + "\"bundle_root\":\"" + MODEL_BUNDLE_ROOT + "\","
        + "\"sorafs_manifest_digest\":" + fixedBytesJson(0x11, 32) + ","
        + "\"chunk_count\":1,"
        + "\"plaintext_bytes\":32,"
        + "\"ciphertext_bytes\":48,"
        + "\"chunk_manifest_root\":\"" + RESULT_COMMITMENT + "\","
        + "\"upload_recipient\":{"
        + "\"schema_version\":1,"
        + "\"key_id\":\"bundle-key\","
        + "\"key_version\":1,"
        + "\"kem\":{\"kem\":\"X25519HkdfSha256\",\"value\":null},"
        + "\"aead\":{\"aead\":\"Aes256Gcm\",\"value\":null},"
        + "\"public_key_bytes\":\"" + PUBLIC_KEY_BASE64 + "\","
        + "\"public_key_fingerprint\":\"" + PUBLIC_KEY_FINGERPRINT + "\""
        + "},"
        + "\"wrapped_bundle_key\":{"
        + "\"schema_version\":1,"
        + "\"recipient_key_id\":\"bundle-key\","
        + "\"recipient_key_version\":1,"
        + "\"kem\":{\"kem\":\"X25519HkdfSha256\",\"value\":null},"
        + "\"aead\":{\"aead\":\"Aes256Gcm\",\"value\":null},"
        + "\"ephemeral_public_key\":\"" + PUBLIC_KEY_BASE64 + "\","
        + "\"nonce\":\"" + WRAPPED_NONCE_BASE64 + "\","
        + "\"wrapped_key_ciphertext\":\"" + WRAPPED_KEY_CIPHERTEXT_BASE64 + "\","
        + "\"ciphertext_hash\":\"" + WRAPPED_KEY_CIPHERTEXT_HASH + "\","
        + "\"aad_digest\":\"" + OUTPUT_COMMITMENT + "\""
        + "},"
        + "\"pricing_policy\":{\"storage_price\":\"1\"},"
        + "\"decryption_policy_ref\":\"policy-1\""
        + "},"
        + "\"artifact\":" + artifactStatusJson()
        + "}";
  }

  private static String artifactStatusJson() {
    return "{"
        + "\"service_name\":\"portal\","
        + "\"model_name\":\"portal_model\","
        + "\"artifact_id\":\"artifact-1\","
        + "\"training_job_id\":\"upload-1\","
        + "\"weight_version\":\"v1\","
        + "\"weight_artifact_hash\":\"" + INPUT_ARTIFACT_HASH + "\","
        + "\"dataset_ref\":\"dataset:upload-1\","
        + "\"training_config_hash\":\"" + INPUT_COMMITMENT + "\","
        + "\"reproducibility_hash\":\"" + OUTPUT_COMMITMENT + "\","
        + "\"provenance_attestation_hash\":\"" + REQUEST_COMMITMENT + "\","
        + "\"registered_sequence\":1,"
        + "\"consumed_by_version\":\"v1\","
        + "\"chunk_manifest_root\":\"" + RESULT_COMMITMENT + "\""
        + "}";
  }

  private static String receiptJson() {
    return "{"
        + "\"schema_version\":1,"
        + "\"network_id\":\"" + NETWORK_ID + "\","
        + "\"receipt_id\":\"" + RECEIPT_ID + "\","
        + "\"service_name\":\"portal\","
        + "\"service_version\":\"2026.1\","
        + "\"model_id\":\"upload-1\","
        + "\"weight_version\":\"v1\","
        + "\"runtime_version\":\"soracloud.quantized-cpu.v1\","
        + "\"model_manifest_digest\":" + fixedBytesJson(0x11, 32) + ","
        + "\"model_bundle_root\":\"" + MODEL_BUNDLE_ROOT + "\","
        + "\"policy_id\":\"policy-1\","
        + "\"decryption_request_id\":\"decrypt-upload-1\","
        + attestingValidatorField()
        + "\"input_artifact\":{"
        + "\"schema_version\":1,"
        + "\"sorafs_manifest_digest\":" + fixedBytesJson(0x22, 32) + ","
        + "\"sorafs_root_cid\":" + ROOT_CID_JSON + ","
        + "\"artifact_hash\":\"" + INPUT_ARTIFACT_HASH + "\","
        + "\"ciphertext_bytes\":64,"
        + "\"artifact_role\":\"input\""
        + "},"
        + "\"output_artifact\":" + outputArtifactJson() + ","
        + "\"output_replication_order_id\":"
        + fixedBytesJson(OUTPUT_REPLICATION_ORDER_ID)
        + ","
        + "\"input_commitment\":\"" + INPUT_COMMITMENT + "\","
        + "\"output_commitment\":\"" + OUTPUT_COMMITMENT + "\","
        + outputRecipientField()
        + "\"request_commitment\":\"" + REQUEST_COMMITMENT + "\","
        + "\"result_commitment\":\"" + RESULT_COMMITMENT + "\","
        + "\"authorization_claim_block_height\":0,"
        + "\"authorization_claim_epoch\":0,"
        + "\"emitted_sequence\":0,"
        + "\"emitted_block_height\":0,"
        + "\"emitted_epoch\":0"
        + "}";
  }

  private static String attestingValidatorField() {
    return "\"attesting_validator\":{"
        + "\"lane_id\":0,"
        + "\"validator_account_id\":\"" + VALIDATOR_ACCOUNT_ID + "\","
        + "\"peer_id\":\"" + VALIDATOR_PEER_ID + "\""
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
        + "\"public_key_fingerprint\":\"" + PUBLIC_KEY_FINGERPRINT + "\""
        + "},";
  }

  private static String outputArtifactJson() {
    return "{"
        + "\"schema_version\":1,"
        + "\"sorafs_manifest_digest\":" + fixedBytesJson(0x33, 32) + ","
        + "\"sorafs_root_cid\":" + ROOT_CID_JSON + ","
        + "\"artifact_hash\":\"" + OUTPUT_ARTIFACT_HASH + "\","
        + "\"ciphertext_bytes\":96,"
        + "\"artifact_role\":\"output\""
        + "}";
  }

  private static byte[] bytes(final String json) {
    return json.getBytes(StandardCharsets.UTF_8);
  }

  private static byte[] filledDigest(final int value) {
    final byte[] digest = new byte[32];
    Arrays.fill(digest, (byte) value);
    return digest;
  }

  private static byte[] filledBytes(final int value, final int size) {
    final byte[] bytes = new byte[size];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static String fixedBytesJson(final int value, final int size) {
    final StringBuilder json = new StringBuilder(size * 4 + 2).append('[');
    for (int index = 0; index < size; index++) {
      if (index != 0) {
        json.append(',');
      }
      json.append(value);
    }
    return json.append(']').toString();
  }

  private static String fixedBytesJson(final byte[] values) {
    final StringBuilder json = new StringBuilder(values.length * 4 + 2).append('[');
    for (int index = 0; index < values.length; index++) {
      if (index != 0) {
        json.append(',');
      }
      json.append(values[index] & 0xff);
    }
    return json.append(']').toString();
  }

  private static byte[] mismatchingOutputReplicationOrderId() {
    final byte[] mismatching = OUTPUT_REPLICATION_ORDER_ID.clone();
    mismatching[mismatching.length - 1] ^= 1;
    return mismatching;
  }

  private static SoracloudPrivateModelArtifactRef artifactWithManifestDigest(
      final SoracloudPrivateModelArtifactRef template, final byte[] manifestDigest) {
    return new SoracloudPrivateModelArtifactRef(
        template.schemaVersion(),
        manifestDigest,
        template.sorafsRootCid(),
        template.artifactHash(),
        template.ciphertextBytes(),
        template.artifactRole());
  }

  private static SoracloudPrivateUploadedModelExecutionReceipt receiptWithManifestDigest(
      final SoracloudPrivateUploadedModelExecutionReceipt template,
      final byte[] manifestDigest) {
    return copyReceipt(
        template,
        manifestDigest,
        template.serviceName(),
        template.emittedSequence(),
        template.emittedBlockHeight(),
        template.emittedEpoch());
  }

  private static SoracloudPrivateUploadedModelExecutionReceipt receiptWithReplicationOrder(
      final SoracloudPrivateUploadedModelExecutionReceipt template,
      final byte[] outputReplicationOrderId) {
    return new SoracloudPrivateUploadedModelExecutionReceipt(
        template.schemaVersion(),
        template.networkId(),
        template.receiptId(),
        template.serviceName(),
        template.serviceVersion(),
        template.modelId(),
        template.weightVersion(),
        template.runtimeVersion(),
        template.modelManifestDigest(),
        template.modelBundleRoot(),
        template.policyId(),
        template.decryptionRequestId(),
        template.attestingValidator(),
        template.inputArtifact(),
        template.outputArtifact(),
        outputReplicationOrderId,
        template.inputCommitment(),
        template.outputCommitment(),
        template.outputRecipient(),
        template.requestCommitment(),
        template.resultCommitment(),
        template.authorizationClaimBlockHeight(),
        template.authorizationClaimEpoch(),
        template.emittedSequence(),
        template.emittedBlockHeight(),
        template.emittedEpoch());
  }

  private static SoracloudPrivateUploadedModelExecutionReceipt receiptWithCoordinates(
      final SoracloudPrivateUploadedModelExecutionReceipt template,
      final BigInteger emittedSequence,
      final BigInteger emittedBlockHeight,
      final BigInteger emittedEpoch) {
    return copyReceipt(
        template,
        template.modelManifestDigest(),
        template.serviceName(),
        emittedSequence,
        emittedBlockHeight,
        emittedEpoch);
  }

  private static SoracloudPrivateUploadedModelExecutionReceipt receiptWithServiceName(
      final SoracloudPrivateUploadedModelExecutionReceipt template,
      final String serviceName) {
    return copyReceipt(
        template,
        template.modelManifestDigest(),
        serviceName,
        template.emittedSequence(),
        template.emittedBlockHeight(),
        template.emittedEpoch());
  }

  private static SoracloudPrivateUploadedModelExecutionReceipt receiptWithSelectors(
      final SoracloudPrivateUploadedModelExecutionReceipt template,
      final String serviceVersion,
      final String modelId,
      final String weightVersion) {
    return new SoracloudPrivateUploadedModelExecutionReceipt(
        template.schemaVersion(),
        template.networkId(),
        template.receiptId(),
        template.serviceName(),
        serviceVersion,
        modelId,
        weightVersion,
        template.runtimeVersion(),
        template.modelManifestDigest(),
        template.modelBundleRoot(),
        template.policyId(),
        template.decryptionRequestId(),
        template.attestingValidator(),
        template.inputArtifact(),
        template.outputArtifact(),
        template.outputReplicationOrderId(),
        template.inputCommitment(),
        template.outputCommitment(),
        template.outputRecipient(),
        template.requestCommitment(),
        template.resultCommitment(),
        template.authorizationClaimBlockHeight(),
        template.authorizationClaimEpoch(),
        template.emittedSequence(),
        template.emittedBlockHeight(),
        template.emittedEpoch());
  }

  private static SoracloudPrivateUploadedModelExecutionReceipt copyReceipt(
      final SoracloudPrivateUploadedModelExecutionReceipt template,
      final byte[] manifestDigest,
      final String serviceName,
      final BigInteger emittedSequence,
      final BigInteger emittedBlockHeight,
      final BigInteger emittedEpoch) {
    return new SoracloudPrivateUploadedModelExecutionReceipt(
        template.schemaVersion(),
        template.networkId(),
        template.receiptId(),
        serviceName,
        template.serviceVersion(),
        template.modelId(),
        template.weightVersion(),
        template.runtimeVersion(),
        manifestDigest,
        template.modelBundleRoot(),
        template.policyId(),
        template.decryptionRequestId(),
        template.attestingValidator(),
        template.inputArtifact(),
        template.outputArtifact(),
        template.outputReplicationOrderId(),
        template.inputCommitment(),
        template.outputCommitment(),
        template.outputRecipient(),
        template.requestCommitment(),
        template.resultCommitment(),
        template.authorizationClaimBlockHeight(),
        template.authorizationClaimEpoch(),
        emittedSequence,
        emittedBlockHeight,
        emittedEpoch);
  }

  private static void assertThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final RuntimeException expected) {
      return;
    }
    throw new AssertionError(message);
  }

  private static String repeated(final char value, final int count) {
    final char[] chars = new char[count];
    Arrays.fill(chars, value);
    return new String(chars);
  }

  private static void assertHashFieldRejected(
      final String field,
      final String canonicalHash,
      final String invalidHash,
      final String message) {
    final String canonicalField = "\"" + field + "\":\"" + canonicalHash + "\"";
    final String response = executeResponseJson();
    if (!response.contains(canonicalField)) {
      throw new AssertionError("missing canonical fixture field " + field);
    }
    assertThrows(
        () -> SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            bytes(response.replace(
                canonicalField, "\"" + field + "\":\"" + invalidHash + "\""))),
        message);
  }

  private static String tamperChecksum(final String literal) {
    final char last = literal.charAt(literal.length() - 1);
    return literal.substring(0, literal.length() - 1) + (last == '0' ? '1' : '0');
  }

  private static String validatorAccountId(final byte[] publicKey) {
    try {
      return AccountAddress.fromAccount(publicKey, "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException error) {
      throw new ExceptionInInitializerError(error);
    }
  }

  private static String multisigValidatorAccountId(final byte[] publicKey) {
    try {
      return AccountAddress.fromMultisigPolicy(
              AccountAddress.MultisigPolicyPayload.of(
                  1,
                  1,
                  Collections.singletonList(
                      AccountAddress.MultisigMemberPayload.of(0x01, 1, publicKey))))
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException error) {
      throw new ExceptionInInitializerError(error);
    }
  }

  private static byte[] x25519PublicKey() {
    final byte[] privateKey = new byte[32];
    for (int index = 0; index < privateKey.length; index++) {
      privateKey[index] = (byte) (index + 1);
    }
    return new X25519PrivateKeyParameters(privateKey, 0).generatePublicKey().getEncoded();
  }

  private static final byte[] VALIDATOR_PUBLIC_KEY = TestEd25519Keys.publicKey(0x30);
  private static final String VALIDATOR_ACCOUNT_ID = validatorAccountId(VALIDATOR_PUBLIC_KEY);
  private static final String VALIDATOR_PEER_ID =
      PublicKeyCodec.encodePublicKeyMultihash(0x01, VALIDATOR_PUBLIC_KEY);
  private static final String OTHER_VALIDATOR_PEER_ID =
      PublicKeyCodec.encodePublicKeyMultihash(0x01, TestEd25519Keys.publicKey(0x31));
  private static final String MULTISIG_VALIDATOR_ACCOUNT_ID =
      multisigValidatorAccountId(VALIDATOR_PUBLIC_KEY);
  private static final byte[] PUBLIC_KEY_BYTES = x25519PublicKey();
  private static final String PUBLIC_KEY_BASE64 =
      Base64.getEncoder().encodeToString(PUBLIC_KEY_BYTES);
  private static final String ZERO_X25519_KEY_BASE64 =
      Base64.getEncoder().encodeToString(new byte[32]);
  private static final String ZERO_PREHASH_SENTINEL =
      "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E";
  private static final String TRANSACTION_HASH =
      "hash:4141414141414141414141414141414141414141414141414141414141414141#7023";
  private static final String RECEIPT_ID =
      "hash:4343434343434343434343434343434343434343434343434343434343434343#AAA5";
  private static final String MODEL_BUNDLE_ROOT =
      "hash:4545454545454545454545454545454545454545454545454545454545454545#D50E";
  private static final String INPUT_ARTIFACT_HASH =
      "hash:4747474747474747474747474747474747474747474747474747474747474747#0F88";
  private static final String OUTPUT_ARTIFACT_HASH =
      "hash:4949494949494949494949494949494949494949494949494949494949494949#2A58";
  private static final String INPUT_COMMITMENT =
      "hash:4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B4B#F947";
  private static final String OUTPUT_COMMITMENT =
      "hash:4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D4D#86EC";
  private static final String REQUEST_COMMITMENT =
      "hash:4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F4F#5C6A";
  private static final String RESULT_COMMITMENT =
      "hash:5151515151515151515151515151515151515151515151515151515151515151#8E28";
  private static final byte[] OUTPUT_REPLICATION_ORDER_ID = {
      (byte) 223, 84, (byte) 153, 93, (byte) 189, (byte) 208, 15, 57,
      18, (byte) 144, 6, (byte) 143, 35, 114, 49, (byte) 183,
      (byte) 235, (byte) 169, (byte) 151, 26, 48, (byte) 191, (byte) 231, (byte) 173,
      2, (byte) 235, (byte) 241, 47, (byte) 189, 13, 37, 69
  };
  private static final String WRAPPED_NONCE_BASE64 =
      Base64.getEncoder().encodeToString(filledBytes(0x0b, 12));
  private static final byte[] WRAPPED_KEY_CIPHERTEXT = filledBytes(0x0c, 48);
  private static final String WRAPPED_KEY_CIPHERTEXT_BASE64 =
      Base64.getEncoder().encodeToString(WRAPPED_KEY_CIPHERTEXT);
  private static final String WRAPPED_KEY_CIPHERTEXT_HASH =
      HashLiteral.canonicalize(IrohaHash.prehash(WRAPPED_KEY_CIPHERTEXT));
  private static final String PUBLIC_KEY_FINGERPRINT =
      HashLiteral.canonicalize(IrohaHash.prehash(PUBLIC_KEY_BYTES));
  private static final String RECEIPT_CURSOR = repeated('A', 114);
  private static final String UNMARKED_HASH_LITERAL =
      "hash:0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A#86CD";
  private static final String NETWORK_ID =
      "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
  private static final String ROOT_CID_JSON =
      "[1,113,31,32,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32]";
  private static final String ZERO_DIGEST_ROOT_CID_JSON =
      "[1,113,31,32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]";
}
