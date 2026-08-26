package org.hyperledger.iroha.android.nexus;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.Collections;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.alias.AccountOnboardingCurrentStateV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingPlanReceiptV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingPlanRequestV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingProofRequiredPrepareResponseV1;
import org.hyperledger.iroha.android.alias.TairaPublicResetMutationBindingV1;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.IrohaClient;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.PipelineStatusOptions;
import org.hyperledger.iroha.android.client.ToriiCanonicalRequestAuth;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.numeric.NumericV1;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.junit.Test;

public final class NexusAppClientTest {

  private static final String ASSET_DEFINITION_ID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1";
  private static final byte[] SIGNING_PRIVATE_KEY_SEED = filled(0x11, 32);
  private static final byte[] PUBLIC_KEY =
      new Ed25519PrivateKeyParameters(SIGNING_PRIVATE_KEY_SEED, 0)
          .generatePublicKey()
          .getEncoded();
  private static final byte[] WALLET_SIGNATURE = fixtureExpectedBytes("wallet_signature_hex");
  private static final FeePaymentIntent TEST_FEE_PAYMENT =
      FeePaymentIntent.authority(Collections.emptyList());
  private static final NetworkId TEST_NETWORK_ID =
      NetworkId.parse(
          "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");
  private static final String ACCOUNT_ID =
      "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB";
  private static final String DESTINATION_ACCOUNT_ID =
      "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L";

  @Test
  public void transferInputRequiresCanonicalQuantityStrings() {
    for (final String quantity : new String[] {" ", "+1", "01", "1e0", "-1", "1.0", "1.2300"}) {
      boolean threw = false;
      try {
        NexusTransferInput.builder()
            .sourceAssetId("asset")
            .quantity(quantity)
            .destinationAccountId(DESTINATION_ACCOUNT_ID)
            .feePayment(TEST_FEE_PAYMENT)
            .build();
      } catch (final IllegalArgumentException expected) {
        threw = true;
      }
      assertTrue("noncanonical quantity was accepted: " + quantity, threw);
    }

    assertEquals(
        "1.25",
        NexusTransferInput.builder()
            .sourceAssetId("asset")
            .quantity(NumericV1.QuantityValue.parseCanonical("1.25"))
            .destinationAccountId(DESTINATION_ACCOUNT_ID)
            .feePayment(TEST_FEE_PAYMENT)
            .build()
            .quantity());
  }

  @Test
  public void transferWithWalletBuildsSignsSubmitsAndWaits() {
    final FakeConnect connect = new FakeConnect();
    final FakeToriiClient torii = new FakeToriiClient();
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, "sample-app", null, null, null, PUBLIC_KEY, Collections.emptyMap()),
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

    @SuppressWarnings("unchecked")
    final Map<String, Object> finalStatus =
        (Map<String, Object>) receipt.finalStatus().get("status");
    assertEquals("Applied", finalStatus.get("kind"));
    assertEquals(receipt.transactionHashHex(), torii.submittedHash);
    assertEquals(receipt.transactionHashHex(), SignedTransactionHasher.hashHex(receipt.signedTransaction()));
    assertArrayEquals(PUBLIC_KEY, receipt.signedTransaction().publicKey());
    assertArrayEquals(connect.signature, receipt.signedTransaction().signature());
    assertNotNull(connect.lastSignable);
    assertTrue(connect.lastSignable.payloadBytes().length > 0);
    expectIllegalArgument(
        () ->
            new NexusTransferReceipt(
                "aa".repeat(32),
                receipt.signedTransaction(),
                receipt.submission(),
                receipt.finalStatus()));
    final String wrongMarkedHash =
        (receipt.transactionHashHex().charAt(0) == '0' ? "1" : "0")
            + receipt.transactionHashHex().substring(1);
    expectIllegalArgument(
        () ->
            new NexusTransferReceipt(
                wrongMarkedHash,
                receipt.signedTransaction(),
                receipt.submission(),
                receipt.finalStatus()));
  }

  @Test
  public void buildTransferDraftFailsClosedWithoutSigningPublicKey() {
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, ACCOUNT_ID, null, Collections.emptyMap()));

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
                TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, ACCOUNT_ID, PUBLIC_KEY, Collections.emptyMap()));

    final NexusTransferDraft draft = client.buildTransferDraft(sampleInput());
    final byte[] fixtureSignature = hexToBytes(string(expected, "wallet_signature_hex"));
    final SignedTransaction signed =
        SignedTransaction.builder()
            .setEncodedPayload(draft.signable().payloadBytes())
            .setSignature(fixtureSignature)
            .setPublicKey(PUBLIC_KEY)
            .setSchemaName(new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT).schemaName())
            .build();

    assertEquals(string(expected, "payload_hash_hex"), draft.signable().payloadHashHex());
    assertArrayEquals(hexToBytes(string(expected, "payload_bytes_hex")), draft.signable().payloadBytes());
    assertArrayEquals(signPayload(draft.signable().payloadBytes()), fixtureSignature);
    assertEquals(
        string(expected, "signed_transaction_hash_hex"), SignedTransactionHasher.hashHex(signed));
    assertEquals(Arrays.asList("Submitted", "Applied"), expected.get("status_sequence"));
  }

  @Test
  public void finalizeAndSubmitAcceptsExactZeroSignatureAlgorithmAlias() {
    final FakeToriiClient torii = new FakeToriiClient();
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, ACCOUNT_ID, PUBLIC_KEY, Collections.emptyMap()),
            null,
            null,
            torii);
    final NexusTransferDraft draft = client.buildTransferDraft(sampleInput());
    final NexusSignableTransaction signable =
        new NexusSignableTransaction(
            draft.signable().payloadBytes(),
            draft.signable().payloadHashHex(),
            draft.signable().authority(),
            draft.signable().signingPublicKey(),
            "0");
    final byte[] walletSignature = signPayload(signable.payloadBytes());

    final NexusTransferReceipt receipt =
        client.finalizeAndSubmit(
            signable,
            new NexusWalletSignature(walletSignature, "0"),
            new NexusFinalizeOptions(false, null));

    assertEquals(receipt.transactionHashHex(), torii.submittedHash);
    assertEquals(receipt.transactionHashHex(), SignedTransactionHasher.hashHex(receipt.signedTransaction()));
    assertArrayEquals(walletSignature, receipt.signedTransaction().signature());
    assertArrayEquals(PUBLIC_KEY, receipt.signedTransaction().publicKey());
  }

  @Test
  public void finalizeAndSubmitRejectsUnsupportedSignatureAlgorithm() {
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, ACCOUNT_ID, PUBLIC_KEY, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient());
    final NexusTransferDraft draft = client.buildTransferDraft(sampleInput());

    for (final String algorithm :
        new String[] {
          "ed25519 ",
          " ed25519",
          "\ted25519",
          "ed25519\n",
          "ed25519\u00A0",
          "0 ",
          " 0",
          "\t0",
          "00",
          "\uFF10",
          "secp256k1",
          "ed\t25519",
          "ed" + (char) 0 + "25519",
          "ed" + (char) 0x001F + "25519",
          "ed" + (char) 0x007F + "25519",
          "ed\u200B25519",
          "\u0435d25519",
          "ed\uFF0D25519",
          "ED25519",
          "Ed25519",
          " ED25519 ",
        }) {
      final NexusAppError error =
          expectNexusError(
              () ->
                  client.finalizeAndSubmit(
                      draft.signable(), new NexusWalletSignature(filled(0x07, 64), algorithm)));

      assertEquals("unsupported_signature_algorithm", error.code());
    }

    for (final String algorithm :
        new String[] {
          "ed25519 ",
          " ed25519",
          "0 ",
          " 0",
          "00",
          "ED25519",
          "ed" + (char) 0 + "25519",
          "ed\u200B25519",
          "\u0435d25519",
        }) {
      final NexusAppError signableError =
          expectNexusError(
              () ->
                  client.finalizeAndSubmit(
                      new NexusSignableTransaction(
                          draft.signable().payloadBytes(),
                          draft.signable().payloadHashHex(),
                          draft.signable().authority(),
                          draft.signable().signingPublicKey(),
                          algorithm),
                      new NexusWalletSignature(WALLET_SIGNATURE)));

      assertEquals("unsupported_signature_algorithm", signableError.code());
    }
  }

  @Test
  public void requestSignatureRejectsUnsupportedAlgorithmsAtTransportBoundary() {
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
    final NexusSignableTransaction signable =
        new NexusSignableTransaction(
            new byte[] {0x01, 0x02, 0x03},
            "0".repeat(64),
            ACCOUNT_ID,
            PUBLIC_KEY,
            NexusAppClient.SIGNATURE_ALGORITHM_ED25519);

    for (final String algorithm :
        new String[] {"ed25519 ", " 0", "ED25519", "ed\u200B25519"}) {
      final SignatureConnect connect = new SignatureConnect(WALLET_SIGNATURE);
      final NexusAppClient client =
          new NexusAppClient(
              new NexusAppConfig(TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, null, null, Collections.emptyMap()),
              connect,
              null,
              null);
      final NexusSignableTransaction badSignable =
          new NexusSignableTransaction(
              signable.payloadBytes(),
              signable.payloadHashHex(),
              signable.authority(),
              signable.signingPublicKey(),
              algorithm);

      final NexusAppError error = expectNexusError(() -> client.requestSignature(session, badSignable));

      assertEquals("unsupported_signature_algorithm", error.code());
      assertEquals(null, connect.lastSignable);
    }

    for (final String algorithm :
        new String[] {"ed25519 ", " 0", "\uFF10", "ed" + (char) 0 + "25519", "\u0435d25519"}) {
      final SignatureConnect connect = new SignatureConnect(WALLET_SIGNATURE, algorithm);
      final NexusAppClient client =
          new NexusAppClient(
              new NexusAppConfig(TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, null, null, Collections.emptyMap()),
              connect,
              null,
              null);

      final NexusAppError error = expectNexusError(() -> client.requestSignature(session, signable));

      assertEquals("unsupported_signature_algorithm", error.code());
      assertNotNull(connect.lastSignable);
    }
  }

  @Test
  public void awaitApprovalRejectsMissingAccountAndSigningKey() {
    final NexusAppClient missingAccount =
        new NexusAppClient(
            new NexusAppConfig(TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, null, null, Collections.emptyMap()),
            new ApprovalConnect(new NexusApprovedAccount("", null)),
            null,
            null);

    final NexusAppError missingAccountError =
        expectNexusError(
            () -> missingAccount.awaitApproval(new NexusConnectSession("session-1", "sora://wallet/connect")));
    assertEquals("approval_missing_account", missingAccountError.code());

    final NexusAppClient missingKey =
        new NexusAppClient(
            new NexusAppConfig(TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, null, null, Collections.emptyMap()),
            new ApprovalConnect(new NexusApprovedAccount(ACCOUNT_ID, null)),
            null,
            null);

    final NexusAppError missingKeyError =
        expectNexusError(
            () -> missingKey.awaitApproval(new NexusConnectSession("session-1", "sora://wallet/connect")));
    assertEquals("missing_signing_public_key", missingKeyError.code());

    final NexusAppClient invalidKey =
        new NexusAppClient(
            new NexusAppConfig(TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, null, null, Collections.emptyMap()),
            new ApprovalConnect(new NexusApprovedAccount(ACCOUNT_ID, filled(0x01, 31))),
            null,
            null);

    final NexusAppError invalidKeyError =
        expectNexusError(
            () -> invalidKey.awaitApproval(new NexusConnectSession("session-1", "sora://wallet/connect")));
    assertEquals("invalid_signing_public_key", invalidKeyError.code());

    final NexusAppClient mixedTorsionKey =
        new NexusAppClient(
            new NexusAppConfig(TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, null, null, Collections.emptyMap()),
            new ApprovalConnect(new NexusApprovedAccount(ACCOUNT_ID, filled(0x11, 32))),
            null,
            null);

    final NexusAppError mixedTorsionError =
        expectNexusError(
            () ->
                mixedTorsionKey.awaitApproval(
                    new NexusConnectSession("session-1", "sora://wallet/connect")));
    assertEquals("invalid_signing_public_key", mixedTorsionError.code());
  }

  @Test
  public void transferWithWalletRejectsAuthorityMismatchBeforeSigning() {
    final FakeConnect connect = new FakeConnect();
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, null, PUBLIC_KEY, Collections.emptyMap()),
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
                TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, ACCOUNT_ID, PUBLIC_KEY, Collections.emptyMap()),
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
                TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, ACCOUNT_ID, PUBLIC_KEY, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient());
    final NexusTransferDraft draft = draftClient.buildTransferDraft(sampleInput());
    final NexusWalletSignature signature =
        new NexusWalletSignature(signPayload(draft.signable().payloadBytes()));

    final NexusAppClient mismatchClient =
        new NexusAppClient(
            new NexusAppConfig(TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, null, null, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient("f".repeat(64), null, null));
    final NexusAppError mismatchError =
        expectNexusError(() -> mismatchClient.finalizeAndSubmit(draft.signable(), signature));
    assertEquals("transaction_hash_mismatch", mismatchError.code());

    final NexusAppClient submitFailureClient =
        new NexusAppClient(
            new NexusAppConfig(TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, null, null, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient(null, new RuntimeException("down"), null));
    final NexusAppError submitError =
        expectNexusError(() -> submitFailureClient.finalizeAndSubmit(draft.signable(), signature));
    assertEquals("submit_failed", submitError.code());

    final NexusAppClient statusFailureClient =
        new NexusAppClient(
            new NexusAppConfig(TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, null, null, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient(null, null, new RuntimeException("timeout")));
    final NexusAppError statusError =
        expectNexusError(() -> statusFailureClient.finalizeAndSubmit(draft.signable(), signature));
    assertEquals("status_wait_failed", statusError.code());

    final NexusAppClient committedOnlyClient =
        new NexusAppClient(
            new NexusAppConfig(TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, null, null, Collections.emptyMap()),
            null,
            null,
            new FakeToriiClient(null, null, null, "Committed"));
    final NexusAppError committedOnlyError =
        expectNexusError(() -> committedOnlyClient.finalizeAndSubmit(draft.signable(), signature));
    assertEquals("status_wait_non_applied", committedOnlyError.code());
  }

  @Test
  public void finalizeAndSubmitRejectsValidSignatureBoundToDifferentPayloadBytes() {
    final FakeToriiClient torii = new FakeToriiClient();
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, ACCOUNT_ID, PUBLIC_KEY, Collections.emptyMap()),
            null,
            null,
            torii);
    final NexusTransferDraft draft = client.buildTransferDraft(sampleInput());
    final byte[] signature = signPayload(draft.signable().payloadBytes());
    final byte[] tamperedPayload = draft.signable().payloadBytes();
    tamperedPayload[tamperedPayload.length - 1] ^= 0x01;
    final NexusSignableTransaction tamperedSignable =
        new NexusSignableTransaction(
            tamperedPayload,
            draft.signable().payloadHashHex(),
            draft.signable().authority(),
            draft.signable().signingPublicKey(),
            draft.signable().signatureAlgorithm());

    final NexusAppError error =
        expectNexusError(
            () ->
                client.finalizeAndSubmit(
                    tamperedSignable, new NexusWalletSignature(signature)));

    assertEquals("invalid_signature", error.code());
    assertEquals(null, torii.submittedHash);
  }

  @Test
  public void finalizeAndSubmitRejectsInvalidSignatureBytes() {
    final NexusAppClient client =
        new NexusAppClient(
            new NexusAppConfig(
                TEST_NETWORK_ID, "test-chain", AccountAddress.DEFAULT_I105_DISCRIMINANT, null, null, null, ACCOUNT_ID, PUBLIC_KEY, Collections.emptyMap()),
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
        .feePayment(TEST_FEE_PAYMENT)
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

  private static byte[] fixtureExpectedBytes(final String key) {
    try {
      return hexToBytes(string(asMap(loadNexusFixture().get("expected")), key));
    } catch (final Exception ex) {
      throw new ExceptionInInitializerError(ex);
    }
  }

  private static byte[] signPayload(final byte[] payloadBytes) {
    final byte[] message = IrohaHash.prehash(payloadBytes);
    final Ed25519Signer signer = new Ed25519Signer();
    signer.init(true, new Ed25519PrivateKeyParameters(SIGNING_PRIVATE_KEY_SEED, 0));
    signer.update(message, 0, message.length);
    return signer.generateSignature();
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

  private static void expectIllegalArgument(final ThrowingRunnable runnable) {
    try {
      runnable.run();
    } catch (final IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError("expected IllegalArgumentException");
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
    private byte[] signature;
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
      signature = signPayload(signable.payloadBytes());
      return new NexusWalletSignature(signature);
    }
  }

  private static final class SignatureConnect implements NexusConnectTransport {
    private final byte[] signature;
    private final String algorithm;
    private NexusSignableTransaction lastSignable;

    private SignatureConnect(final byte[] signature) {
      this(signature, NexusAppClient.SIGNATURE_ALGORITHM_ED25519);
    }

    private SignatureConnect(final byte[] signature, final String algorithm) {
      this.signature = Arrays.copyOf(signature, signature.length);
      this.algorithm = algorithm;
    }

    @Override
    public NexusConnectSession startConnect(
        final NexusConnectOptions options, final NexusAppConfig config) {
      return new NexusConnectSession("session-1", "sora://wallet/connect?session=session-1");
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
      return new NexusWalletSignature(signature, algorithm);
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
    private final String statusKind;
    private String submittedHash;

    private FakeToriiClient() {
      this(null, null, null, "Applied");
    }

    private FakeToriiClient(
        final String responseHash,
        final RuntimeException submitFailure,
        final RuntimeException statusFailure) {
      this(responseHash, submitFailure, statusFailure, "Applied");
    }

    private FakeToriiClient(
        final String responseHash,
        final RuntimeException submitFailure,
        final RuntimeException statusFailure,
        final String statusKind) {
      this.responseHash = responseHash;
      this.submitFailure = submitFailure;
      this.statusFailure = statusFailure;
      this.statusKind = statusKind;
    }

    @Override
    public CompletableFuture<AccountOnboardingCurrentStateV1>
        verifyAccountOnboardingCurrentState(
            final AccountOnboardingProofRequiredPrepareResponseV1 proofRequired,
            final AccountOnboardingPlanRequestV1 request,
            final AccountOnboardingPlanReceiptV1 receipt,
            final TairaPublicResetMutationBindingV1 binding,
            final String expectedAuthority,
            final NetworkId expectedNetworkId,
            final ToriiCanonicalRequestAuth canonicalAuth) {
      throw new AssertionError("account onboarding is not used by this test fake");
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
      final Map<String, Object> status =
          "Applied".equals(statusKind)
              ? Map.of("kind", statusKind, "block_height", 7)
              : Map.of("kind", statusKind);
      return CompletableFuture.completedFuture(
          Map.of(
              "hash", hashHex,
              "status", status,
              "scope", "global",
              "resolved_from", "Applied".equals(statusKind) ? "state" : "cache"));
    }
  }
}
