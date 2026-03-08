package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Map;

public final class OfflineJsonParserTest {

  private OfflineJsonParserTest() {}

  public static void main(final String[] args) {
    parsesAllowancePayload();
    parsesTransferPayload();
    parsesTransferDetailPayload();
    handlesOptionalAssetId();
    encodesTransferItem();
    extractsReceiptSummary();
    parsesRevocationPayload();
    parsesCertificateIssueResponse();
    parsesSettlementSubmitResponse();
    parsesBundleProofStatusResponse();
    draftJsonOmitsOperator();
    rejectsFractionalTotals();
    rejectsFractionalOptionalTimestamp();
    System.out.println("[IrohaAndroid] OfflineJsonParserTest passed.");
  }

  private static void parsesAllowancePayload() {
    final String json =
        """
        {
          "total": 1,
          "items": [
            {
              "certificate_id_hex": "deadbeef",
              "controller_id": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "controller_display": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "asset_id": "norito:00",
              "registered_at_ms": 1700000000000,
              "expires_at_ms": 1700500000000,
              "policy_expires_at_ms": 1700600000000,
              "refresh_at_ms": 1700400000000,
              "verdict_id_hex": "feedface",
              "attestation_nonce_hex": "1234abcd",
              "remaining_amount": "42",
              "record": { "policy": { "max_tx_value": "10" } }
            }
          ]
        }
        """;
    final OfflineAllowanceList list =
        OfflineJsonParser.parseAllowances(json.getBytes(StandardCharsets.UTF_8));
    assert list.total() == 1 : "total mismatch";
    assert list.items().size() == 1 : "items size mismatch";
    final OfflineAllowanceList.OfflineAllowanceItem item = list.items().get(0);
    assert "deadbeef".equals(item.certificateIdHex()) : "certificate id mismatch";
    assert "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp".equals(item.controllerId()) : "controller mismatch";
    assert "norito:00".equals(item.assetId()) : "asset mismatch";
    assert item.registeredAtMs() == 1_700_000_000_000L : "timestamp mismatch";
    assert item.certificateExpiresAtMs() == 1_700_500_000_000L : "certificate expiry mismatch";
    assert item.policyExpiresAtMs() == 1_700_600_000_000L : "policy expiry mismatch";
    assert item.refreshAtMs() == 1_700_400_000_000L : "refresh mismatch";
    assert "feedface".equals(item.verdictIdHex()) : "verdict mismatch";
    assert "1234abcd".equals(item.attestationNonceHex()) : "nonce mismatch";
    assert "42".equals(item.remainingAmount()) : "remaining mismatch";
    assert item.recordAsMap().containsKey("policy") : "record missing policy";
  }

  private static void parsesTransferPayload() {
    final String json =
        """
        {
          "total": 1,
          "items": [
            {
              "bundle_id_hex": "cafebabe",
              "receiver_id": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "receiver_display": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "deposit_account_id": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "deposit_account_display": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "asset_id": "norito:00",
              "receipt_count": 2,
              "total_amount": "15",
              "claimed_delta": "15",
              "status": "settled",
              "recorded_at_ms": 1700000000000,
              "recorded_at_height": 42,
              "status_transitions": [{"status":"settled","transitioned_at_ms":1700000000000}],
              "platform_policy": "play_integrity",
              "platform_token_snapshot": {
                "policy": "play_integrity",
                "attestation_jws_b64": "token"
              },
              "transfer": { "bundle": "payload" }
            }
          ]
        }
        """;
    final OfflineTransferList list =
        OfflineJsonParser.parseTransfers(json.getBytes(StandardCharsets.UTF_8));
    assert list.total() == 1 : "transfer total mismatch";
    final List<OfflineTransferList.OfflineTransferItem> items = list.items();
    assert items.size() == 1 : "transfer size mismatch";
    final OfflineTransferList.OfflineTransferItem item = items.get(0);
    assert "cafebabe".equals(item.bundleIdHex()) : "bundle id mismatch";
    assert "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp".equals(item.receiverId()) : "receiver mismatch";
    assert item.receiptCount() == 2 : "receipt count mismatch";
    assert "15".equals(item.totalAmount()) : "total amount mismatch";
    assert "15".equals(item.claimedDelta()) : "claimed delta mismatch";
    assert "settled".equals(item.status()) : "status mismatch";
    assert Long.valueOf(1_700_000_000_000L).equals(item.recordedAtMs()) : "recordedAtMs mismatch";
    assert Long.valueOf(42L).equals(item.recordedAtHeight()) : "recordedAtHeight mismatch";
    assert item.statusTransitionsJson() != null : "status transitions missing";
    assert "play_integrity".equals(item.platformPolicy()) : "platform policy mismatch";
    assert item.platformTokenSnapshot() != null : "snapshot missing";
    assert "token".equals(item.platformTokenSnapshot().attestationJwsB64())
        : "snapshot token mismatch";
    assert item.transferAsMap() != null : "transfer map missing";
  }

  private static void parsesTransferDetailPayload() {
    final String json =
        """
        {
          "bundle_id_hex": "deadbeef",
          "receiver_id": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
          "receiver_display": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
          "deposit_account_id": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
          "deposit_account_display": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
          "asset_id": "norito:00",
          "receipt_count": 1,
          "total_amount": "5",
          "claimed_delta": "5",
          "status": "archived",
          "recorded_at_ms": 1700000000000,
          "recorded_at_height": 100,
          "transfer": {}
        }
        """;
    final OfflineTransferList.OfflineTransferItem item =
        OfflineJsonParser.parseTransferItem(json.getBytes(StandardCharsets.UTF_8));
    assert "deadbeef".equals(item.bundleIdHex()) : "detail bundle id mismatch";
    assert "archived".equals(item.status()) : "detail status mismatch";
    assert Long.valueOf(100L).equals(item.recordedAtHeight()) : "detail height mismatch";
  }

  private static void handlesOptionalAssetId() {
    final String json =
        """
        {
          "total": 1,
          "items": [
            {
              "bundle_id_hex": "feedface",
              "receiver_id": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "receiver_display": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "deposit_account_id": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "deposit_account_display": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "receipt_count": 1,
              "total_amount": "5",
              "claimed_delta": "5",
              "transfer": {}
            }
          ]
        }
        """;
    final OfflineTransferList list =
        OfflineJsonParser.parseTransfers(json.getBytes(StandardCharsets.UTF_8));
    final OfflineTransferList.OfflineTransferItem item = list.items().get(0);
    assert item.assetId() == null : "asset should be null when field omitted";
  }

  private static void encodesTransferItem() {
    final OfflineTransferList.OfflineTransferItem.PlatformTokenSnapshot snapshot =
        new OfflineTransferList.OfflineTransferItem.PlatformTokenSnapshot("play_integrity", "token");
    final OfflineTransferList.OfflineTransferItem item =
        new OfflineTransferList.OfflineTransferItem(
            "cafebabe",
            "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
            "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
            "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
            "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
            null,
            2,
            "15",
            "15",
            null,
            snapshot,
            "{\"bundle\":\"payload\"}");
    final Map<String, Object> json = item.toJsonMap();
    assert "cafebabe".equals(json.get("bundle_id_hex")) : "bundle id mismatch";
    assert "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp".equals(json.get("receiver_id")) : "receiver mismatch";
    assert !json.containsKey("asset_id") : "asset_id should be omitted when null";
    assert !json.containsKey("platform_policy") : "platform_policy should be omitted when null";
    final Object snapshotNode = json.get("platform_token_snapshot");
    assert snapshotNode instanceof Map<?, ?> : "snapshot must be a JSON map";
    final Map<?, ?> snapshotMap = (Map<?, ?>) snapshotNode;
    assert "play_integrity".equals(snapshotMap.get("policy")) : "snapshot policy mismatch";
    assert "token".equals(snapshotMap.get("attestation_jws_b64"))
        : "snapshot attestation mismatch";
    final Object transferNode = json.get("transfer");
    assert transferNode instanceof Map<?, ?> : "transfer must be a JSON map";
    final Map<?, ?> transferMap = (Map<?, ?>) transferNode;
    assert "payload".equals(transferMap.get("bundle")) : "transfer payload mismatch";
  }

  private static void extractsReceiptSummary() {
    final String json =
        """
        {
          "total": 1,
          "items": [
            {
              "bundle_id_hex": "c0ffee",
              "receiver_id": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "receiver_display": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "deposit_account_id": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "deposit_account_display": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "asset_id": "norito:00",
              "receipt_count": 1,
              "total_amount": "7.5",
              "claimed_delta": "7.5",
              "transfer": {
                "receipts": [
                  {
                    "tx_id": "offline-tx-1",
                    "from": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
                    "to": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
                    "asset": "norito:00#6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
                    "amount": "7.5"
                  }
                ],
                "balance_proof": {
                  "claimed_delta": "7.5"
                }
              }
            }
          ]
        }
        """;
    final OfflineTransferList list =
        OfflineJsonParser.parseTransfers(json.getBytes(StandardCharsets.UTF_8));
    final OfflineTransferList.OfflineTransferItem item = list.items().get(0);
    final var summary = item.firstReceiptSummary();
    assert summary.isPresent() : "receipt summary missing";
    assert "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp".equals(summary.get().senderId()) : "sender mismatch";
    assert "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp".equals(summary.get().receiverId()) : "receiver mismatch";
    assert "norito:00#6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp".equals(summary.get().assetId())
        : "asset mismatch";
    assert "7.5".equals(summary.get().amount()) : "amount mismatch";
  }

  private static void parsesRevocationPayload() {
    final String json =
        """
        {
          "total": 1,
          "items": [
            {
              "verdict_id_hex": "ABCD",
              "issuer_id": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "issuer_display": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "revoked_at_ms": 1730314876000,
              "reason": "compromised_key",
              "note": "hardware breach",
              "metadata": { "jurisdiction": "eu" },
              "record": { "verdict_id": "ABCD" }
            }
          ]
        }
        """;
    final OfflineRevocationList list =
        OfflineJsonParser.parseRevocations(json.getBytes(StandardCharsets.UTF_8));
    assert list.total() == 1 : "revocation total mismatch";
    final OfflineRevocationList.OfflineRevocationItem item = list.items().get(0);
    assert "ABCD".equals(item.verdictIdHex()) : "verdict id mismatch";
    assert "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp".equals(item.issuerId()) : "issuer mismatch";
    assert item.revokedAtMs() == 1_730_314_876_000L : "revoked timestamp mismatch";
    assert "compromised_key".equals(item.reason()) : "reason mismatch";
    assert "hardware breach".equals(item.note()) : "note mismatch";
    assert "eu".equals(item.metadataAsMap().get("jurisdiction")) : "metadata mismatch";
    assert item.recordAsMap().containsKey("verdict_id") : "record JSON missing";
  }

  private static void parsesCertificateIssueResponse() {
    final String json =
        """
        {
          "certificate_id_hex": "deadbeef",
          "certificate": {
            "controller": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
            "operator": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
            "allowance": { "asset": "norito:00", "amount": "10", "commitment": [1, 2, 3] },
            "spend_public_key": "ed0120deadbeef",
            "attestation_report": [4, 5, 6],
            "issued_at_ms": 1700000000000,
            "expires_at_ms": 1700500000000,
            "policy": { "max_balance": "10", "max_tx_value": "5", "expires_at_ms": 1700500000000 },
            "operator_signature": "AA",
            "metadata": {},
            "verdict_id": null,
            "attestation_nonce": null,
            "refresh_at_ms": null
          }
        }
        """;
    final OfflineCertificateIssueResponse response =
        OfflineJsonParser.parseCertificateIssueResponse(json.getBytes(StandardCharsets.UTF_8));
    assert "deadbeef".equals(response.certificateIdHex()) : "certificate id mismatch";
    final OfflineWalletCertificate certificate = response.certificate();
    assert "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp".equals(certificate.operator()) : "operator mismatch";
    assert "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp".equals(certificate.toJsonMap().get("operator"))
        : "operator missing from JSON map";
  }

  private static void parsesSettlementSubmitResponse() {
    final String json =
        """
        {
          "bundle_id_hex": "deadbeef",
          "transaction_hash_hex": "cafebabe"
        }
        """;
    final OfflineSettlementSubmitResponse response =
        OfflineJsonParser.parseSettlementSubmitResponse(json.getBytes(StandardCharsets.UTF_8));
    assert "deadbeef".equals(response.bundleIdHex()) : "settlement bundle mismatch";
    assert "cafebabe".equals(response.transactionHashHex()) : "settlement tx hash mismatch";
  }

  private static void parsesBundleProofStatusResponse() {
    final String json =
        """
        {
          "bundle_id_hex": "deadbeef",
          "receipts_root_hex": "aa",
          "aggregate_proof_root_hex": null,
          "receipts_root_matches": null,
          "proof_status": "missing",
          "proof_summary": null
        }
        """;
    final OfflineBundleProofStatus status =
        OfflineJsonParser.parseBundleProofStatus(json.getBytes(StandardCharsets.UTF_8));
    assert "deadbeef".equals(status.bundleIdHex()) : "proof bundle mismatch";
    assert "missing".equals(status.proofStatus()) : "proof status mismatch";
    assert status.aggregateProofRootHex() == null : "proof root should be null";
    assert status.receiptsRootMatches() == null : "root match should be null";
    assert status.proofSummary() == null : "proof summary should be null";
  }

  private static void draftJsonOmitsOperator() {
    final OfflineAllowanceCommitment allowance =
        new OfflineAllowanceCommitment("norito:00", "10", new byte[] {1, 2, 3});
    final OfflineWalletPolicy policy = new OfflineWalletPolicy("10", "5", 1700500000000L);
    final OfflineWalletCertificateDraft draft =
        new OfflineWalletCertificateDraft(
            "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
            allowance,
            "ed0120deadbeef",
            new byte[] {4, 5, 6},
            1700000000000L,
            1700500000000L,
            policy,
            null,
            null,
            null,
            null);
    assert !draft.toJsonMap().containsKey("operator")
        : "draft operator must be derived by Torii and omitted from JSON map";
  }

  private static void rejectsFractionalTotals() {
    final String json =
        """
        {
          "total": 1.5,
          "items": []
        }
        """;
    boolean thrown = false;
    try {
      OfflineJsonParser.parseAllowances(json.getBytes(StandardCharsets.UTF_8));
    } catch (Exception ex) {
      thrown = true;
    }
    assert thrown : "expected non-integer totals to be rejected";
  }

  private static void rejectsFractionalOptionalTimestamp() {
    final String json =
        """
        {
          "total": 1,
          "items": [
            {
              "certificate_id_hex": "deadbeef",
              "controller_id": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "controller_display": "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
              "asset_id": "norito:00",
              "registered_at_ms": 1700000000000,
              "expires_at_ms": 1700500000000,
              "policy_expires_at_ms": 1700600000000,
              "refresh_at_ms": 1700400000000.5,
              "verdict_id_hex": "feedface",
              "attestation_nonce_hex": "1234abcd",
              "remaining_amount": "42",
              "record": { "policy": { "max_tx_value": "10" } }
            }
          ]
        }
        """;
    boolean thrown = false;
    try {
      OfflineJsonParser.parseAllowances(json.getBytes(StandardCharsets.UTF_8));
    } catch (Exception ex) {
      thrown = true;
    }
    assert thrown : "expected non-integer timestamps to be rejected";
  }
}
