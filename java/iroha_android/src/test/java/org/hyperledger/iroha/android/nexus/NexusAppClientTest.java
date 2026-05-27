package org.hyperledger.iroha.android.nexus;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

import java.util.Arrays;
import java.util.Collections;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.IrohaClient;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.PipelineStatusOptions;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.junit.Test;

public final class NexusAppClientTest {

  private static final String ASSET_DEFINITION_ID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1";
  private static final byte[] PUBLIC_KEY =
      hexToBytes("d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737");
  private static final byte[] WALLET_SIGNATURE =
      hexToBytes(
          "c82d2ee732a9251153eff6f510a0d12b292cb51a5d961a7eddb84f6ee944e34eaca60ca2f1ccfe7a53fd6813fc9a6db9e35cb276b2411b7d583d45fdc6caee05");
  private static final String ACCOUNT_ID =
      "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB";
  private static final String DESTINATION_ACCOUNT_ID =
      "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L";

  @Test
  public void transferWithWalletBuildsSignsSubmitsAndWaits() {
    final FakeConnect connect = new FakeConnect();
    final FakeToriiClient torii = new FakeToriiClient();
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                "test-chain", "sample-app", null, null, null, PUBLIC_KEY, Collections.emptyMap()),
            connect,
            null,
            torii);

    final NexusConnectSession session =
        client.startConnect(
            new NexusConnectOptions(
                Collections.emptySet(), "sora://wallet/connect", null, Collections.emptyMap()));
    final NexusApprovedAccount approved = client.awaitApproval(session);
    assertEquals(ACCOUNT_ID, approved.accountId());
    assertNotNull(approved.session());

    final NexusTransferReceipt receipt =
        client.transferWithWallet(approved.session(), sampleInput(), new NexusFinalizeOptions());

    assertEquals("Committed", receipt.finalStatus().get("status"));
    assertEquals(receipt.transactionHashHex(), torii.submittedHash);
    assertEquals(receipt.transactionHashHex(), SignedTransactionHasher.hashHex(receipt.signedTransaction()));
    assertArrayEquals(PUBLIC_KEY, receipt.signedTransaction().publicKey());
    assertArrayEquals(connect.signature, receipt.signedTransaction().signature());
    assertNotNull(connect.lastSignable);
    assertTrue(connect.lastSignable.payloadBytes().length > 0);
  }

  @Test
  public void buildTransferDraftFailsClosedWithoutSigningPublicKey() {
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                "test-chain", null, null, null, ACCOUNT_ID, null, Collections.emptyMap()));

    final NexusAppError error =
        expectNexusError(() -> client.buildTransferDraft(sampleInput()));

    assertEquals("missing_signing_public_key", error.code());
  }

  @Test
  public void buildTransferDraftMatchesSharedFixturePayload() throws Exception {
    final Map<String, Object> fixture = loadNexusFixture();
    final Map<String, Object> expected = asMap(fixture.get("expected"));
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                "test-chain", null, null, null, ACCOUNT_ID, PUBLIC_KEY, Collections.emptyMap()));

    final NexusTransferDraft draft = client.buildTransferDraft(sampleInput());

    assertEquals(string(expected, "payload_hash_hex"), draft.signable().payloadHashHex());
    assertArrayEquals(hexToBytes(string(expected, "payload_bytes_hex")), draft.signable().payloadBytes());
  }

  @Test
  public void finalizeAndSubmitRejectsUnsupportedSignatureAlgorithm() {
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                "test-chain", null, null, null, ACCOUNT_ID, PUBLIC_KEY, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient());
    final NexusTransferDraft draft = client.buildTransferDraft(sampleInput());

    final NexusAppError error =
        expectNexusError(
            () ->
                client.finalizeAndSubmit(
                    draft.signable(), new NexusWalletSignature(filled(0x07, 64), "secp256k1")));

    assertEquals("unsupported_signature_algorithm", error.code());
  }

  @Test
  public void awaitApprovalRejectsMissingAccountAndSigningKey() {
    final NexusAppClient missingAccount =
        new NexusAppClient(
            new NexusAppConfig("test-chain", null, null, null, null, null, Collections.emptyMap()),
            new ApprovalConnect(new NexusApprovedAccount("", null)),
            null,
            null);

    final NexusAppError missingAccountError =
        expectNexusError(
            () -> missingAccount.awaitApproval(new NexusConnectSession("session-1", "sora://wallet/connect")));
    assertEquals("approval_missing_account", missingAccountError.code());

    final NexusAppClient missingKey =
        new NexusAppClient(
            new NexusAppConfig("test-chain", null, null, null, null, null, Collections.emptyMap()),
            new ApprovalConnect(new NexusApprovedAccount(ACCOUNT_ID, null)),
            null,
            null);

    final NexusAppError missingKeyError =
        expectNexusError(
            () -> missingKey.awaitApproval(new NexusConnectSession("session-1", "sora://wallet/connect")));
    assertEquals("missing_signing_public_key", missingKeyError.code());

    final NexusAppClient invalidKey =
        new NexusAppClient(
            new NexusAppConfig("test-chain", null, null, null, null, null, Collections.emptyMap()),
            new ApprovalConnect(new NexusApprovedAccount(ACCOUNT_ID, filled(0x01, 31))),
            null,
            null);

    final NexusAppError invalidKeyError =
        expectNexusError(
            () -> invalidKey.awaitApproval(new NexusConnectSession("session-1", "sora://wallet/connect")));
    assertEquals("invalid_signing_public_key", invalidKeyError.code());
  }

  @Test
  public void transferWithWalletRejectsAuthorityMismatchBeforeSigning() {
    final FakeConnect connect = new FakeConnect();
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                "test-chain", null, null, null, null, PUBLIC_KEY, Collections.emptyMap()),
            connect,
            null,
            new FakeToriiClient());
    final NexusConnectSession session =
        new NexusConnectSession(
            "session-1",
            "sora://wallet/connect?session=session-1",
            null,
            null,
            null,
            ACCOUNT_ID,
            PUBLIC_KEY,
            Collections.emptyMap());

    final NexusAppError error =
        expectNexusError(
            () ->
                client.transferWithWallet(
                    session, sampleInput().toBuilder().authority(DESTINATION_ACCOUNT_ID).build()));

    assertEquals("approval_account_mismatch", error.code());
    assertEquals(null, connect.lastSignable);
  }

  @Test
  public void finalizeAndSubmitRejectsInvalidSignatureLength() {
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                "test-chain", null, null, null, ACCOUNT_ID, PUBLIC_KEY, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient());
    final NexusTransferDraft draft = client.buildTransferDraft(sampleInput());

    final NexusAppError error =
        expectNexusError(
            () -> client.finalizeAndSubmit(draft.signable(), new NexusWalletSignature(filled(0x07, 63))));

    assertEquals("invalid_signature", error.code());
  }

  @Test
  public void finalizeAndSubmitRejectsHashMismatchAndMapsSubmitStatusFailures() {
    final NexusAppClient draftClient =
        new NexusAppClient(
            new NexusAppConfig(
                "test-chain", null, null, null, ACCOUNT_ID, PUBLIC_KEY, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient());
    final NexusTransferDraft draft = draftClient.buildTransferDraft(sampleInput());
    final NexusWalletSignature signature = new NexusWalletSignature(WALLET_SIGNATURE);

    final NexusAppClient mismatchClient =
        new NexusAppClient(
            new NexusAppConfig("test-chain", null, null, null, null, null, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient("f".repeat(64), null, null));
    final NexusAppError mismatchError =
        expectNexusError(() -> mismatchClient.finalizeAndSubmit(draft.signable(), signature));
    assertEquals("transaction_hash_mismatch", mismatchError.code());

    final NexusAppClient submitFailureClient =
        new NexusAppClient(
            new NexusAppConfig("test-chain", null, null, null, null, null, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient(null, new RuntimeException("down"), null));
    final NexusAppError submitError =
        expectNexusError(() -> submitFailureClient.finalizeAndSubmit(draft.signable(), signature));
    assertEquals("submit_failed", submitError.code());

    final NexusAppClient statusFailureClient =
        new NexusAppClient(
            new NexusAppConfig("test-chain", null, null, null, null, null, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient(null, null, new RuntimeException("timeout")));
    final NexusAppError statusError =
        expectNexusError(() -> statusFailureClient.finalizeAndSubmit(draft.signable(), signature));
    assertEquals("status_wait_failed", statusError.code());
  }

  @Test
  public void finalizeAndSubmitRejectsInvalidSignatureBytes() {
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                "test-chain", null, null, null, ACCOUNT_ID, PUBLIC_KEY, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient());
    final NexusTransferDraft draft = client.buildTransferDraft(sampleInput());

    final NexusAppError error =
        expectNexusError(
            () -> client.finalizeAndSubmit(draft.signable(), new NexusWalletSignature(filled(0x07, 64))));

    assertEquals("invalid_signature", error.code());
  }

  private static NexusTransferInput sampleInput() {
    return NexusTransferInput.builder()
        .sourceAssetId(ASSET_DEFINITION_ID + "#" + ACCOUNT_ID)
        .quantity("12.34")
        .destinationAccountId(DESTINATION_ACCOUNT_ID)
        .creationTimeMs(1_700_000_000_000L)
        .ttlMs(30_000L)
        .nonce(7)
        .metadata(Collections.singletonMap("purpose", "nexus-app-fixture"))
        .build();
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> loadNexusFixture() throws Exception {
    Path cursor = Paths.get("").toAbsolutePath();
    while (cursor != null) {
      final Path candidate = cursor.resolve("fixtures/sdk/nexus_connect_transfer_v1.json");
      if (Files.isRegularFile(candidate)) {
        return (Map<String, Object>)
            JsonParser.parse(new String(Files.readAllBytes(candidate), StandardCharsets.UTF_8));
      }
      cursor = cursor.getParent();
    }
    throw new AssertionError("fixtures/sdk/nexus_connect_transfer_v1.json was not found");
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> asMap(final Object value) {
    return (Map<String, Object>) value;
  }

  private static String string(final Map<String, Object> map, final String key) {
    return (String) map.get(key);
  }

  private static byte[] hexToBytes(final String hex) {
    final byte[] bytes = new byte[hex.length() / 2];
    for (int i = 0; i < bytes.length; i++) {
      bytes[i] = (byte) Integer.parseInt(hex.substring(i * 2, i * 2 + 2), 16);
    }
    return bytes;
  }

  private static NexusAppError expectNexusError(final ThrowingRunnable runnable) {
    try {
      runnable.run();
    } catch (final NexusAppError ex) {
      return ex;
    }
    throw new AssertionError("expected NexusAppError");
  }

  private static byte[] filled(final int value, final int size) {
    final byte[] bytes = new byte[size];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private interface ThrowingRunnable {
    void run();
  }

  private static final class FakeConnect implements NexusConnectTransport {
    private final byte[] signature = Arrays.copyOf(WALLET_SIGNATURE, WALLET_SIGNATURE.length);
    private NexusSignableTransaction lastSignable;

    @Override
    public NexusConnectSession startConnect(
        final NexusConnectOptions options, final NexusAppConfig config) {
      final String base =
          options.walletUriBase() == null ? "sora://wallet/connect" : options.walletUriBase();
      final String sessionId = options.sessionId() == null ? "session-1" : options.sessionId();
      return new NexusConnectSession(
          sessionId,
          base + "?session=" + sessionId,
          config.appId(),
          config.relayUrl(),
          options.node() == null ? config.node() : options.node(),
          null,
          null,
          Collections.emptyMap());
    }

    @Override
    public NexusApprovedAccount awaitApproval(
        final NexusConnectSession session, final NexusAppConfig config) {
      return new NexusApprovedAccount(ACCOUNT_ID, PUBLIC_KEY);
    }

    @Override
    public NexusWalletSignature requestSignature(
        final NexusConnectSession session,
        final NexusSignableTransaction signable,
        final NexusAppConfig config) {
      lastSignable = signable;
      assertEquals(NexusAppClient.SIGNATURE_ALGORITHM_ED25519, signable.signatureAlgorithm());
      return new NexusWalletSignature(signature);
    }
  }

  private static final class ApprovalConnect implements NexusConnectTransport {
    private final NexusApprovedAccount approval;

    private ApprovalConnect(final NexusApprovedAccount approval) {
      this.approval = approval;
    }

    @Override
    public NexusConnectSession startConnect(
        final NexusConnectOptions options, final NexusAppConfig config) {
      return new NexusConnectSession("session-1", "sora://wallet/connect?session=session-1");
    }

    @Override
    public NexusApprovedAccount awaitApproval(
        final NexusConnectSession session, final NexusAppConfig config) {
      return approval;
    }

    @Override
    public NexusWalletSignature requestSignature(
        final NexusConnectSession session,
        final NexusSignableTransaction signable,
        final NexusAppConfig config) {
      throw new AssertionError("signature request should not be called");
    }
  }

  private static final class FakeToriiClient implements IrohaClient {
    private final String responseHash;
    private final RuntimeException submitFailure;
    private final RuntimeException statusFailure;
    private String submittedHash;

    private FakeToriiClient() {
      this(null, null, null);
    }

    private FakeToriiClient(
        final String responseHash,
        final RuntimeException submitFailure,
        final RuntimeException statusFailure) {
      this.responseHash = responseHash;
      this.submitFailure = submitFailure;
      this.statusFailure = statusFailure;
    }

    @Override
    public CompletableFuture<ClientResponse> submitTransaction(final SignedTransaction transaction) {
      if (submitFailure != null) {
        final CompletableFuture<ClientResponse> failed = new CompletableFuture<>();
        failed.completeExceptionally(submitFailure);
        return failed;
      }
      submittedHash = SignedTransactionHasher.hashHex(transaction);
      return CompletableFuture.completedFuture(
          new ClientResponse(202, new byte[0], "accepted", responseHash == null ? submittedHash : responseHash));
    }

    @Override
    public CompletableFuture<Map<String, Object>> waitForTransactionStatus(
        final String hashHex, final PipelineStatusOptions options) {
      if (statusFailure != null) {
        final CompletableFuture<Map<String, Object>> failed = new CompletableFuture<>();
        failed.completeExceptionally(statusFailure);
        return failed;
      }
      return CompletableFuture.completedFuture(Map.of("status", "Committed", "hash", hashHex));
    }
  }
}
