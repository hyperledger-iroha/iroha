package org.hyperledger.iroha.android.tx;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.KeyPair;
import java.security.Signature;
import java.util.Arrays;
import java.util.Base64;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.crypto.KeyProviderMetadata;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.crypto.SoftwareKeyProvider;
import org.hyperledger.iroha.android.crypto.keystore.KeyAttestation;
import org.hyperledger.iroha.android.crypto.keystore.KeyGenParameters;
import org.hyperledger.iroha.android.crypto.keystore.KeyGenerationResult;
import org.hyperledger.iroha.android.crypto.keystore.KeystoreBackend;
import org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProvider;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoCodecAdapter;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.offline.InMemoryOfflineNoteStore;
import org.hyperledger.iroha.android.offline.KagemushaInstructionArchives;
import org.hyperledger.iroha.android.offline.OfflineBearerCashWallet;
import org.hyperledger.iroha.android.offline.OfflineCashLifecycle;
import org.hyperledger.iroha.android.offline.OfflineNote;
import org.hyperledger.iroha.android.offline.OfflineNoteAttestationProvider;
import org.hyperledger.iroha.android.offline.OfflineNoteProofProvider;
import org.hyperledger.iroha.android.offline.OfflineNoteProofVerifier;
import org.hyperledger.iroha.android.offline.OfflineNoteWallet;
import org.hyperledger.iroha.android.offline.SecureOfflineNoteRandomSource;
import org.hyperledger.iroha.android.offline.UuidOfflineNoteIdGenerator;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.android.tx.offline.OfflineEnvelopeOptions;
import org.hyperledger.iroha.android.tx.offline.OfflineTransactionBundle;

public final class TransactionBuilderTests {

  private TransactionBuilderTests() {}

  private static final byte[] DUMMY_CERT = new byte[] {0x01};

  public static void main(final String[] args) throws Exception {
    encodeAndSignWithExplicitSigner();
    encodeAndSignWithKeyManagerAlias();
    instructionsVariantRoundTrips();
    transactionPayloadRejectsPaddedIdsBeforeSigning();
    kagemushaInstructionArchivesBuildPayloads();
    kagemushaInstructionArchivesAcceptAbi7Fixtures();
    kagemushaInstructionArchivesRejectPaddedIdsBeforeArchiveOrNativeRedeem();
    kagemushaInstructionArchivesRejectAdversarialInputs();
    offlineCashLifecycleAndTransportGuards();
    encodeAndSignEnvelopeWithAttestationBundle();
    encodeAndSignEnvelopeWithAttestationWithoutHardware();
    System.out.println("[IrohaAndroid] Transaction builder tests passed.");
  }

  private static void encodeAndSignWithExplicitSigner() throws Exception {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000002")
            .setAuthority(TestAccountIds.ed25519Authority(0x28))
            .setCreationTimeMs(1_735_000_001_234L)
            .setExecutable(Executable.ivm("payload-bytes".getBytes()))
            .setTimeToLiveMs(10_000L)
            .setNonce(7)
            .putMetadata("channel", "builder-test")
            .build();

    final FakeSigner signer = new FakeSigner();
    final NoritoCodecAdapter codec = new NoritoJavaCodecAdapter();
    final TransactionBuilder builder =
        new TransactionBuilder(codec, IrohaKeyManager.withSoftwareProvider());

    final SignedTransaction signed = builder.encodeAndSign(payload, signer);
    final byte[] expectedSignature = concat(signed.encodedPayload(), "-signature".getBytes());
    assert Arrays.equals(expectedSignature, signed.signature())
        : "Fake signer should append signature suffix";
    assert Arrays.equals("fake-public-key".getBytes(), signed.publicKey())
        : "Fake signer should return test public key";

    final TransactionPayload decoded = codec.decodeTransaction(signed.encodedPayload());
    assert decoded.chainId().equals(payload.chainId()) : "Chain must round-trip";
    assert decoded.authority().equals(payload.authority()) : "Authority must round-trip";
    assert decoded.creationTimeMs() == payload.creationTimeMs() : "Timestamp must round-trip";
    assert Arrays.equals(
            payload.executable().ivmBytes(), decoded.executable().ivmBytes())
        : "Norito codec must roundtrip instructions";
    assert decoded.timeToLiveMs().equals(payload.timeToLiveMs()) : "TTL must round-trip";
    assert decoded.nonce().equals(payload.nonce()) : "Nonce must round-trip";
    assert decoded.metadata().equals(payload.metadata()) : "Metadata must round-trip";
  }

  private static void encodeAndSignWithKeyManagerAlias() throws Exception {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000003")
            .setAuthority(TestAccountIds.ed25519Authority(0x29))
            .setCreationTimeMs(1_735_000_111_000L)
            .setExecutable(Executable.ivm("alias-sign".getBytes()))
            .setTimeToLiveMs(null)
            .setNonce(null)
            .build();

    final IrohaKeyManager keyManager = IrohaKeyManager.withSoftwareProvider();
    final TransactionBuilder builder =
        new TransactionBuilder(new NoritoJavaCodecAdapter(), keyManager);

    final SignedTransaction signed =
        builder.encodeAndSign(
            payload,
            "transaction-alias",
            IrohaKeyManager.KeySecurityPreference.SOFTWARE_ONLY);

    final TransactionPayload decoded =
        new NoritoJavaCodecAdapter().decodeTransaction(signed.encodedPayload());
    assert Arrays.equals(payload.executable().ivmBytes(), decoded.executable().ivmBytes())
        : "Decoded transaction must match original instructions";
    assert decoded.chainId().equals(payload.chainId()) : "Chain must match";
    assert decoded.authority().equals(payload.authority()) : "Authority must match";

    final KeyPair keyPair =
        keyManager.generateOrLoad(
            "transaction-alias", IrohaKeyManager.KeySecurityPreference.SOFTWARE_ONLY);

    final Signature verifier = Signature.getInstance("Ed25519");
    verifier.initVerify(keyPair.getPublic());
    verifier.update(IrohaHash.prehash(signed.encodedPayload()));
    assert verifier.verify(signed.signature())
        : "Signature produced via key manager must verify";
  }

  private static void instructionsVariantRoundTrips() throws Exception {
    final byte[] wirePayloadA =
        NoritoCodec.encode("wire-A", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final byte[] wirePayloadB =
        NoritoCodec.encode("wire-B", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setExecutable(
                Executable.instructions(
                    List.of(
                        InstructionBox.fromWirePayload("iroha.register.domain", wirePayloadA),
                        InstructionBox.fromWirePayload("iroha.register.account", wirePayloadB))))
            .build();
    final TransactionBuilder builder =
        new TransactionBuilder(new NoritoJavaCodecAdapter(), IrohaKeyManager.withSoftwareProvider());
    final SignedTransaction signed = builder.encodeAndSign(payload, new FakeSigner());
    final TransactionPayload decoded = new NoritoJavaCodecAdapter().decodeTransaction(signed.encodedPayload());
    assert decoded.executable().isInstructions() : "Executable variant must remain instructions";
    assert decoded.executable().instructions().equals(payload.executable().instructions())
        : "Instruction list must round-trip";
  }

  private static void transactionPayloadRejectsPaddedIdsBeforeSigning() {
    final String authority = TestAccountIds.ed25519Authority(0x2F);
    assertIllegalArgumentMessage(
        () -> TransactionPayload.builder().setChainId(" 00000042"),
        "chainId must not contain surrounding whitespace");
    assertIllegalArgumentMessage(
        () -> TransactionPayload.builder().setChainId("00000042 "),
        "chainId must not contain surrounding whitespace");
    assertIllegalArgumentMessage(
        () -> TransactionPayload.builder().setAuthority(" " + authority),
        "authority must not contain surrounding whitespace");
  }

  private static void kagemushaInstructionArchivesBuildPayloads() {
    final byte[] archive =
        kagemushaArchive(KagemushaInstructionArchives.InstructionType.REDEEM_RECURSIVE);
    final InstructionBox box = KagemushaInstructionArchives.recursiveRedeemInstructionBox(archive);
    archive[0] = 0;

    final InstructionBox.WirePayload wire = (InstructionBox.WirePayload) box.payload();
    assert wire
        .wireName()
        .equals("iroha_data_model::isi::offline::RedeemKagemushaRecursive")
        : "Redeem instruction wire name must be canonical";
    assert Arrays.equals(
            kagemushaArchive(KagemushaInstructionArchives.InstructionType.REDEEM_RECURSIVE),
            wire.payloadBytes())
        : "Archive bytes must be defensively copied";

    final byte[] transferArchive =
        kagemushaArchive(KagemushaInstructionArchives.InstructionType.TRANSFER);
    final byte[] expectedTransferArchive =
        Arrays.copyOf(transferArchive, transferArchive.length);
    final TransactionPayload payload =
        KagemushaInstructionArchives.transactionPayload(
            KagemushaInstructionArchives.InstructionType.TRANSFER,
            transferArchive,
            "00000042",
            TestAccountIds.ed25519Authority(0x2C),
            1_735_000_000_000L,
            3_500L,
            17,
            Map.of("mode", "kagemusha", "enabled", JsonValue.bool(true)));
    assert payload.executable().isInstructions() : "Payload must use instruction executable";
    final InstructionBox.WirePayload transferWire =
        (InstructionBox.WirePayload) payload.executable().instructions().get(0).payload();
    assert transferWire
        .wireName()
        .equals("iroha_data_model::isi::offline::KagemushaTransfer")
        : "Transfer instruction wire name must be canonical";
    assert Arrays.equals(transferArchive, transferWire.payloadBytes())
        : "Transfer archive bytes must be preserved";
    transferArchive[0] = (byte) 0x7F;
    assert Arrays.equals(expectedTransferArchive, transferWire.payloadBytes())
        : "Transfer archive bytes must be defensively copied";
    assert JsonValue.string("kagemusha").equals(payload.metadata().get("mode"))
        : "String metadata must be encoded as JSON string";
    assert JsonValue.bool(true).equals(payload.metadata().get("enabled"))
        : "Typed JSON metadata must be preserved";
    assert KagemushaInstructionArchives.TRANSFER_INSTRUCTION_WIRE_NAME.equals(
            KagemushaInstructionArchives.InstructionType.TRANSFER.wireName())
        : "Transfer wire-name constant must match enum";
    assert KagemushaInstructionArchives.RECURSIVE_REDEEM_REQUEST_WIRE_NAME.equals(
            "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1")
        : "Redeem request wire-name constant must be canonical";

    final byte[] redeemArchive =
        kagemushaArchive(KagemushaInstructionArchives.InstructionType.REDEEM_RECURSIVE);
    final byte[] expectedRedeemArchive = Arrays.copyOf(redeemArchive, redeemArchive.length);
    final TransactionPayload redeemPayload =
        KagemushaInstructionArchives.recursiveRedeemTransactionPayload(
            redeemArchive,
            "00000042",
            TestAccountIds.ed25519Authority(0x2C),
            1_735_000_000_000L,
            3_500L,
            17,
            Map.of("mode", "kagemusha"));
    assert redeemPayload.executable().isInstructions()
        : "Recursive redeem payload must use instruction executable";
    final InstructionBox.WirePayload redeemWire =
        (InstructionBox.WirePayload) redeemPayload.executable().instructions().get(0).payload();
    assert redeemWire
        .wireName()
        .equals("iroha_data_model::isi::offline::RedeemKagemushaRecursive")
        : "Recursive redeem instruction wire name must be canonical";
    assert Arrays.equals(expectedRedeemArchive, redeemWire.payloadBytes())
        : "Recursive redeem archive bytes must be preserved";
    redeemArchive[0] = (byte) 0x7E;
    assert Arrays.equals(expectedRedeemArchive, redeemWire.payloadBytes())
        : "Recursive redeem transaction archive bytes must be defensively copied";
  }

  private static void kagemushaInstructionArchivesAcceptAbi7Fixtures() {
    final byte[] archive = sharedRecursiveSpendAbi7Archive("redeem_instruction");
    final InstructionBox box = KagemushaInstructionArchives.recursiveRedeemInstructionBox(archive);
    final InstructionBox.WirePayload wire = (InstructionBox.WirePayload) box.payload();

    assert wire
        .wireName()
        .equals("iroha_data_model::isi::offline::RedeemKagemushaRecursive")
        : "ABI-7 redeem instruction wire name must be canonical";
    assert Arrays.equals(archive, wire.payloadBytes())
        : "ABI-7 redeem instruction archive bytes must be preserved";
  }

  private static void kagemushaInstructionArchivesRejectPaddedIdsBeforeArchiveOrNativeRedeem() {
    final String authority = TestAccountIds.ed25519Authority(0x2C);
    assertIllegalArgumentMessage(
        () ->
            KagemushaInstructionArchives.transactionPayload(
                KagemushaInstructionArchives.InstructionType.TRANSFER,
                new byte[0],
                " 00000042",
                authority,
                1_735_000_000_000L,
                null,
                null,
                Map.of()),
        "chainId must not contain surrounding whitespace");
    assertIllegalArgumentMessage(
        () ->
            KagemushaInstructionArchives.transactionPayload(
                KagemushaInstructionArchives.InstructionType.TRANSFER,
                new byte[0],
                "00000042",
                " " + authority,
                1_735_000_000_000L,
                null,
                null,
                Map.of()),
        "authority must not contain surrounding whitespace");
    assertIllegalArgumentMessage(
        () ->
            KagemushaInstructionArchives.recursiveRedeemTransactionPayloadFromRequest(
                new byte[0],
                " 00000042",
                authority,
                1_735_000_000_000L,
                null,
                null,
                Map.of()),
        "chainId must not contain surrounding whitespace");
    assertIllegalArgumentMessage(
        () ->
            KagemushaInstructionArchives.recursiveRedeemTransactionPayloadFromRequest(
                new byte[0],
                "00000042",
                " " + authority,
                1_735_000_000_000L,
                null,
                null,
                Map.of()),
        "authority must not contain surrounding whitespace");
  }

  private static void kagemushaInstructionArchivesRejectAdversarialInputs() {
    assertThrows(
        () -> KagemushaInstructionArchives.recursiveRedeemInstructionBox(new byte[0]),
        "empty archive must be rejected");
    assertThrows(
        () -> KagemushaInstructionArchives.recursiveRedeemInstructionBoxFromRequest(new byte[0]),
        "empty redeem request archive must be rejected");
    assertThrows(
        () ->
            KagemushaInstructionArchives.recursiveRedeemTransactionPayloadFromRequest(
                new byte[0],
                "00000042",
                TestAccountIds.ed25519Authority(0x2C),
                1_735_000_000_000L,
                3_500L,
                17,
                Map.of("mode", "kagemusha")),
        "empty redeem request transaction archive must be rejected");
    assertThrows(
        () -> KagemushaInstructionArchives.recursiveRedeemInstructionBox(new byte[] {0}),
        "malformed archive must be rejected");
    assertThrows(
        () ->
            KagemushaInstructionArchives.recursiveRedeemInstructionBox(
                NoritoCodec.encode(
                    "request",
                    "KagemushaRecursiveSpendRedeemRequestV1",
                    NoritoAdapters.stringAdapter())),
        "wrong schema archive must be rejected");

    final byte[] tampered =
        kagemushaArchive(KagemushaInstructionArchives.InstructionType.REDEEM_RECURSIVE);
    tampered[tampered.length - 1] ^= 0x01;
    assertThrows(
        () -> KagemushaInstructionArchives.recursiveRedeemInstructionBox(tampered),
        "checksum drift must be rejected");

    final byte[] compressed =
        kagemushaArchive(KagemushaInstructionArchives.InstructionType.REDEEM_RECURSIVE);
    compressed[22] = 1;
    assertThrows(
        () -> KagemushaInstructionArchives.recursiveRedeemInstructionBox(compressed),
        "compressed archive must be rejected");

    final byte[] unsupportedFlags =
        kagemushaArchive(KagemushaInstructionArchives.InstructionType.REDEEM_RECURSIVE);
    unsupportedFlags[39] = (byte) NoritoHeader.VARINT_OFFSETS;
    assertThrows(
        () -> KagemushaInstructionArchives.recursiveRedeemInstructionBox(unsupportedFlags),
        "unsupported archive flags must be rejected");

    final byte[] invalidFieldBitsetFlags =
        kagemushaArchive(KagemushaInstructionArchives.InstructionType.REDEEM_RECURSIVE);
    invalidFieldBitsetFlags[39] = (byte) NoritoHeader.FIELD_BITSET;
    assertThrows(
        () -> KagemushaInstructionArchives.recursiveRedeemInstructionBox(invalidFieldBitsetFlags),
        "invalid field-bitset archive flags must be rejected");

    final byte[] nonZeroPadding =
        withNonZeroHeaderPadding(
            kagemushaArchive(KagemushaInstructionArchives.InstructionType.REDEEM_RECURSIVE));
    assertThrows(
        () -> KagemushaInstructionArchives.recursiveRedeemInstructionBox(nonZeroPadding),
        "non-zero header padding must be rejected");
  }

  private static void offlineCashLifecycleAndTransportGuards() throws Exception {
    final OfflineCashLifecycle.TransportCapabilities capabilities =
        new OfflineCashLifecycle.TransportCapabilities(
            true,
            OfflineCashLifecycle.NfcCapability.unavailable("missing HCE"),
            true);
    assert capabilities.supportedTransportKinds().equals(List.of("qr", "nearby"))
        : "Unsupported NFC must be hidden from app-facing transport choices";

    final OfflineCashLifecycle.ConfigurationSnapshot snapshot =
        new OfflineCashLifecycle.ConfigurationSnapshot(true, "issuer-key", 7, 1_000L);
    snapshot.requireUsableForOfflineExchange(999L, 7);
    assert snapshot.chainId() == null : "Legacy snapshot constructor must preserve null chain id";
    assert snapshot.assetDefinitionId() == null
        : "Legacy snapshot constructor must preserve null asset definition id";
    assert snapshot.artifactSetId() == null
        : "Legacy snapshot constructor must preserve null artifact set id";
    assert snapshot.circuitId() == null : "Legacy snapshot constructor must preserve null circuit id";
    assert snapshot.createdAtMs() == 0L
        : "Legacy snapshot constructor must default created_at_ms";

    final OfflineCashLifecycle.ConfigurationSnapshot identifiedSnapshot =
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            "issuer-key",
            7,
            "artifact-set",
            "kagemusha-v1",
            100L,
            1_000L);
    identifiedSnapshot.requireUsableForOfflineExchange(999L, 7);
    assert "00000042".equals(identifiedSnapshot.chainId()) : "Snapshot must expose chain id";
    assert "pkr#sbp".equals(identifiedSnapshot.assetDefinitionId())
        : "Snapshot must expose asset definition id";
    assert identifiedSnapshot.offlinePaymentsEnabled()
        : "Snapshot must expose offline payments flag";
    assert "issuer-key".equals(identifiedSnapshot.issuerPublicKeyBase64())
        : "Snapshot must expose issuer public key";
    assert Integer.valueOf(7).equals(identifiedSnapshot.nativeBridgeAbiVersion())
        : "Snapshot must expose native bridge ABI version";
    assert "artifact-set".equals(identifiedSnapshot.artifactSetId())
        : "Snapshot must expose artifact set id";
    assert "kagemusha-v1".equals(identifiedSnapshot.circuitId())
        : "Snapshot must expose circuit id";
    assert identifiedSnapshot.createdAtMs() == 100L : "Snapshot must expose creation time";
    assert Long.valueOf(1_000L).equals(identifiedSnapshot.expiresAtMs())
        : "Snapshot must expose expiry time";

    assertThrowsRuntime(
        () ->
            new OfflineCashLifecycle.ConfigurationSnapshot(true, " ", 7, null)
                .requireUsableForOfflineExchange(200L, 7),
        "cached issuer key must be required before offline exchange");
    assertSnapshotRejected(
        new OfflineCashLifecycle.ConfigurationSnapshot(false, "issuer-key", 7, null),
        200L,
        7,
        "offline_payments_disabled");
    assertSnapshotRejected(
        new OfflineCashLifecycle.ConfigurationSnapshot(true, "issuer-key", 6, null),
        200L,
        7,
        "unsupported_native_bridge_abi");
    assertSnapshotRejected(
        new OfflineCashLifecycle.ConfigurationSnapshot(true, "issuer-key", 7, 1_000L),
        1_000L,
        7,
        "expired");

    final List<String> events = new java.util.ArrayList<>();
    final OfflineCashLifecycle.Controller controller =
        new OfflineCashLifecycle.Controller(
            new OfflineCashLifecycle.Wallet() {
              @Override
              public CompletableFuture<Object> load(
                  final String assetDefinitionId, final String amount) {
                events.add("load:" + assetDefinitionId + ":" + amount);
                return CompletableFuture.completedFuture("ok");
              }

              @Override
              public Object prepareReceive(final String assetDefinitionId, final String amount) {
                throw new UnsupportedOperationException();
              }

              @Override
              public Object createPayment(final Object receiveRequest) {
                throw new UnsupportedOperationException();
              }

              @Override
              public Object acceptPayment(final Object paymentToken) {
                throw new UnsupportedOperationException();
              }

              @Override
              public CompletableFuture<Object> redeem(final Object note, final String recipient) {
                throw new UnsupportedOperationException();
              }
            },
            new OfflineCashLifecycle.AuditReceiptSynchronizer() {
              @Override
              public CompletableFuture<Boolean> hasPendingAuditReceipts() {
                events.add("hasPending");
                return CompletableFuture.completedFuture(true);
              }

              @Override
              public CompletableFuture<Void> syncPendingAuditReceipts() {
                events.add("sync");
                return CompletableFuture.completedFuture(null);
              }
            });

    assert "ok".equals(controller.load("pkr#sbp", "10").get())
        : "Lifecycle controller must return wallet load result";
    assert events.equals(List.of("hasPending", "sync", "load:pkr#sbp:10"))
        : "Lifecycle controller must sync pending receipts before loading";

    final List<String> failedEvents = new java.util.ArrayList<>();
    final OfflineCashLifecycle.Controller failingController =
        new OfflineCashLifecycle.Controller(
            new OfflineCashLifecycle.Wallet() {
              @Override
              public CompletableFuture<Object> load(
                  final String assetDefinitionId, final String amount) {
                failedEvents.add("load:" + assetDefinitionId + ":" + amount);
                return CompletableFuture.completedFuture("unexpected");
              }

              @Override
              public Object prepareReceive(final String assetDefinitionId, final String amount) {
                throw new UnsupportedOperationException();
              }

              @Override
              public Object createPayment(final Object receiveRequest) {
                throw new UnsupportedOperationException();
              }

              @Override
              public Object acceptPayment(final Object paymentToken) {
                throw new UnsupportedOperationException();
              }

              @Override
              public CompletableFuture<Object> redeem(final Object note, final String recipient) {
                throw new UnsupportedOperationException();
              }
            },
            new OfflineCashLifecycle.AuditReceiptSynchronizer() {
              @Override
              public CompletableFuture<Boolean> hasPendingAuditReceipts() {
                failedEvents.add("hasPending");
                return CompletableFuture.completedFuture(true);
              }

              @Override
              public CompletableFuture<Void> syncPendingAuditReceipts() {
                failedEvents.add("sync");
                return CompletableFuture.failedFuture(new IllegalStateException("audit sync failed"));
              }
            });
    try {
      failingController.load("pkr#sbp", "10").get();
      throw new AssertionError("Lifecycle controller must not load when audit sync fails");
    } catch (final java.util.concurrent.ExecutionException expected) {
      assert expected.getCause() instanceof IllegalStateException
          : "Expected audit sync failure to propagate";
    }
    assert failedEvents.equals(List.of("hasPending", "sync"))
        : "Lifecycle controller must stop before wallet load after sync failure";

    final List<String> noteWalletEvents = new java.util.ArrayList<>();
    final OfflineCashLifecycle.Controller noteWalletController =
        new OfflineCashLifecycle.Controller(
            testOfflineNoteWallet(),
            new OfflineCashLifecycle.AuditReceiptSynchronizer() {
              @Override
              public CompletableFuture<Boolean> hasPendingAuditReceipts() {
                noteWalletEvents.add("hasPending");
                return CompletableFuture.completedFuture(true);
              }

              @Override
              public CompletableFuture<Void> syncPendingAuditReceipts() {
                noteWalletEvents.add("sync");
                return CompletableFuture.completedFuture(null);
              }
            });
    try {
      noteWalletController.load("pkr#sbp", "10").get();
      throw new AssertionError("OfflineNoteWallet adapter must propagate load failure");
    } catch (final java.util.concurrent.ExecutionException expected) {
      assert expected.getCause() instanceof IllegalStateException
          : "Expected OfflineNoteWallet load failure to propagate";
    }
    assert noteWalletEvents.equals(List.of("hasPending", "sync"))
        : "OfflineNoteWallet adapter must sync pending receipts before loading";

    final OfflineCashLifecycle.Controller bearerWalletController =
        new OfflineCashLifecycle.Controller(
            new OfflineBearerCashWallet(testOfflineNoteWallet()), null);
    try {
      bearerWalletController.load("pkr#sbp", "10").get();
      throw new AssertionError("OfflineBearerCashWallet adapter must propagate load failure");
    } catch (final java.util.concurrent.ExecutionException expected) {
      assert expected.getCause() instanceof IllegalStateException
          : "Expected OfflineBearerCashWallet load failure to propagate";
    }
  }

  private static void encodeAndSignEnvelopeWithAttestationBundle() throws Exception {
    final AttestingBackend backend = new AttestingBackend();
    final KeystoreKeyProvider provider =
        new KeystoreKeyProvider(
            backend, KeyGenParameters.builder().setRequireStrongBox(true).build());
    final IrohaKeyManager manager =
        IrohaKeyManager.fromProviders(List.of(provider, new SoftwareKeyProvider()));
    final TransactionBuilder builder = new TransactionBuilder(new NoritoJavaCodecAdapter(), manager);

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000004")
            .setAuthority(TestAccountIds.ed25519Authority(0x2A))
            .setExecutable(Executable.ivm("attested".getBytes()))
            .build();

    final OfflineTransactionBundle bundle =
        builder.encodeAndSignEnvelopeWithAttestation(
            payload,
            "attesting-alias",
            IrohaKeyManager.KeySecurityPreference.STRONGBOX_PREFERRED,
            OfflineEnvelopeOptions.builder().setIssuedAtMs(1_735_123_456_789L).build(),
            new byte[] {0x01, 0x02, 0x03});

    assert bundle.attestation().isPresent() : "StrongBox provider should supply attestation";
    assert Arrays.equals(bundle.envelope().encodedPayload(), bundle.envelope().encodedPayload())
        : "Envelope must be present";
    assert backend.generatedAlias("attesting-alias") : "Backend should record generated alias";
    assert backend.attestationGenerated("attesting-alias") : "Backend should store generated attestation";
  }

  private static void encodeAndSignEnvelopeWithAttestationWithoutHardware() throws Exception {
    final IrohaKeyManager manager = IrohaKeyManager.withSoftwareProvider();
    final TransactionBuilder builder = new TransactionBuilder(new NoritoJavaCodecAdapter(), manager);
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000005")
            .setAuthority(TestAccountIds.ed25519Authority(0x2B))
            .setExecutable(Executable.ivm("software".getBytes()))
            .build();

    final OfflineTransactionBundle bundle =
        builder.encodeAndSignEnvelopeWithAttestation(
            payload,
            "software-only",
            IrohaKeyManager.KeySecurityPreference.SOFTWARE_ONLY,
            new byte[] {0x0A});
    assert bundle.attestation().isEmpty() : "Software-only provider should not produce attestation";
  }

  private static final class AttestingBackend implements KeystoreBackend {
    private final SoftwareKeyProvider delegate = new SoftwareKeyProvider();
    private final ConcurrentMap<String, KeyPair> keys = new ConcurrentHashMap<>();
    private final ConcurrentMap<String, KeyAttestation> attestations = new ConcurrentHashMap<>();
    private final KeyProviderMetadata metadata =
        KeyProviderMetadata.builder("attesting-strongbox")
            .setStrongBoxBacked(true)
            .setSupportsAttestationCertificates(true)
            .build();

    @Override
    public Optional<KeyPair> load(final String alias) {
      return Optional.ofNullable(keys.get(alias));
    }

    @Override
    public KeyGenerationResult generate(final String alias, final KeyGenParameters parameters)
        throws KeyManagementException {
      final KeyPair keyPair = delegate.generate(alias);
      keys.put(alias, keyPair);
      return new KeyGenerationResult(keyPair, metadata.strongBoxBacked());
    }

    @Override
    public KeyPair generateEphemeral(final KeyGenParameters parameters)
        throws KeyManagementException {
      return delegate.generateEphemeral();
    }

    @Override
    public KeyProviderMetadata metadata() {
      return metadata;
    }

    @Override
    public String name() {
      return metadata.name();
    }

    @Override
    public Optional<KeyAttestation> attestation(final String alias) {
      return Optional.ofNullable(attestations.get(alias));
    }

    @Override
    public Optional<KeyAttestation> generateAttestation(
        final String alias, final byte[] challenge) {
      if (!keys.containsKey(alias)) {
        return Optional.empty();
      }
      final KeyAttestation attestation =
          KeyAttestation.builder()
              .setAlias(alias)
              .addCertificate(DUMMY_CERT)
              .addCertificate(DUMMY_CERT)
              .build();
      attestations.put(alias, attestation);
      return Optional.of(attestation);
    }

    boolean generatedAlias(final String alias) {
      return keys.containsKey(alias);
    }

    boolean attestationGenerated(final String alias) {
      return attestations.containsKey(alias);
    }
  }

  private static final class FakeSigner implements Signer {
    @Override
    public byte[] sign(final byte[] message) throws SigningException {
      if (message == null) {
        throw new SigningException("message must not be null");
      }
      final byte[] suffix = "-signature".getBytes();
      final byte[] combined = new byte[message.length + suffix.length];
      System.arraycopy(message, 0, combined, 0, message.length);
      System.arraycopy(suffix, 0, combined, message.length, suffix.length);
      return combined;
    }

    @Override
    public byte[] publicKey() {
      return "fake-public-key".getBytes();
    }

    @Override
    public String algorithm() {
      return "Ed25519";
    }
  }

  private static byte[] concat(final byte[] left, final byte[] right) {
    final byte[] out = new byte[left.length + right.length];
    System.arraycopy(left, 0, out, 0, left.length);
    System.arraycopy(right, 0, out, left.length, right.length);
    return out;
  }

  private static byte[] kagemushaArchive(final KagemushaInstructionArchives.InstructionType type) {
    return NoritoCodec.encode("payload", type.wireName(), NoritoAdapters.stringAdapter());
  }

  private static byte[] withNonZeroHeaderPadding(final byte[] archive) {
    final byte[] padded = new byte[archive.length + 1];
    System.arraycopy(archive, 0, padded, 0, NoritoHeader.HEADER_LENGTH);
    padded[NoritoHeader.HEADER_LENGTH] = 0x7f;
    System.arraycopy(
        archive,
        NoritoHeader.HEADER_LENGTH,
        padded,
        NoritoHeader.HEADER_LENGTH + 1,
        archive.length - NoritoHeader.HEADER_LENGTH);
    return padded;
  }

  @SuppressWarnings("unchecked")
  private static byte[] sharedRecursiveSpendAbi7Archive(final String name) {
    final Map<String, Object> root =
        (Map<String, Object>) JsonParser.parse(sharedRecursiveSpendAbi7Fixture("archives.json"));
    final List<Map<String, Object>> archives = (List<Map<String, Object>>) root.get("archives");
    for (final Map<String, Object> archive : archives) {
      if (name.equals(archive.get("name"))) {
        return Base64.getDecoder().decode((String) archive.get("bytes_base64"));
      }
    }
    throw new AssertionError("missing shared recursive spend ABI-7 archive " + name);
  }

  private static String sharedRecursiveSpendAbi7Fixture(final String fileName) {
    Path directory = Paths.get("").toAbsolutePath();
    while (directory != null) {
      final Path candidate =
          directory.resolve("fixtures/kagemusha_recursive_spend_abi7").resolve(fileName);
      if (Files.isRegularFile(candidate)) {
        try {
          return new String(Files.readAllBytes(candidate), StandardCharsets.UTF_8);
        } catch (final java.io.IOException error) {
          throw new AssertionError("failed to read shared recursive spend ABI-7 fixture", error);
        }
      }
      directory = directory.getParent();
    }
    throw new AssertionError("missing shared recursive spend ABI-7 fixture " + fileName);
  }

  private static OfflineNoteWallet testOfflineNoteWallet() {
    return new OfflineNoteWallet(
        "00000042",
        "merchant",
        unusedAttestationProvider(),
        new InMemoryOfflineNoteStore(),
        null,
        null,
        unusedProofProvider(),
        unusedProofVerifier(),
        new SecureOfflineNoteRandomSource(),
        new UuidOfflineNoteIdGenerator(),
        () -> 1_000L);
  }

  private static OfflineNoteAttestationProvider unusedAttestationProvider() {
    return () -> {
      throw new UnsupportedOperationException("not used");
    };
  }

  private static OfflineNoteProofProvider unusedProofProvider() {
    return new OfflineNoteProofProvider() {
      @Override
      public OfflineNote.RecursiveProof proveAudit(final OfflineNote.AuditBundle audit) {
        throw new UnsupportedOperationException("not used");
      }

      @Override
      public OfflineNote.RecursiveProof proveRedeem(final OfflineNote.Redeem redemption) {
        throw new UnsupportedOperationException("not used");
      }
    };
  }

  private static OfflineNoteProofVerifier unusedProofVerifier() {
    return new OfflineNoteProofVerifier() {
      @Override
      public boolean verifyAudit(final OfflineNote.AuditBundle audit) {
        throw new UnsupportedOperationException("not used");
      }

      @Override
      public boolean verifyRedeem(final OfflineNote.Redeem redemption) {
        throw new UnsupportedOperationException("not used");
      }
    };
  }

  private static void assertThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError(message);
  }

  private static void assertIllegalArgumentMessage(final Runnable runnable, final String expected) {
    try {
      runnable.run();
    } catch (final IllegalArgumentException actual) {
      assert expected.equals(actual.getMessage()) : "Expected " + expected + " but got " + actual;
      return;
    }
    throw new AssertionError("Expected IllegalArgumentException: " + expected);
  }

  private static void assertThrowsRuntime(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final RuntimeException expected) {
      return;
    }
    throw new AssertionError(message);
  }

  private static void assertSnapshotRejected(
      final OfflineCashLifecycle.ConfigurationSnapshot snapshot,
      final long nowMs,
      final Integer requiredNativeBridgeAbiVersion,
      final String expectedCode) {
    try {
      snapshot.requireUsableForOfflineExchange(nowMs, requiredNativeBridgeAbiVersion);
    } catch (final OfflineCashLifecycle.ConfigurationSnapshotException expected) {
      assert expectedCode.equals(expected.code())
          : "Expected snapshot rejection code " + expectedCode + ", got " + expected.code();
      return;
    }
    throw new AssertionError("Expected snapshot rejection " + expectedCode);
  }
}
