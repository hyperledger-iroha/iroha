package org.hyperledger.iroha.android.tx;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.KeyPair;
import java.security.Signature;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.model.ContractInvocation;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.ExecutableBatchItem;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.crypto.SignatureAdmission;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoCodecAdapter;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.testing.TestNetworkIds;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoHeader;

public final class TransactionBuilderTests {

  private TransactionBuilderTests() {}

  public static void main(final String[] args) throws Exception {
    encodeAndSignWithExplicitSigner();
    publicBuilderPreservesExplicitQueuePlanIntent();
    encodeAndSignWithKeyManagerAlias();
    instructionsVariantRoundTrips();
    mixedBatchBuilderAndSignerPreserveOrder();
    publicBuilderRejectsMalformedFixedShapeSignerOutput();
    transactionPayloadRejectsPaddedAuthorityBeforeSigning();
    System.out.println("[IrohaAndroid] Transaction builder tests passed.");
  }

  private static void encodeAndSignWithExplicitSigner() throws Exception {
    final TransactionPayload payload =
        TransactionPayload.builder().setFeePayment(FeePaymentIntent.authority(Collections.emptyList(), 1L))
            .setNetworkId(TestNetworkIds.fromSeed(2L))
            .setAuthority(TestAccountIds.ed25519Authority(0x28))
            .setCreationTimeMs(1_735_000_001_234L)
            .setExecutable(Executable.ivm("payload-bytes".getBytes()))
            .setTimeToLiveMs(10_000L)
            .setNonce(7)
            .putMetadata("channel", "builder-test")
            .build();

    final FakeSigner signer = new FakeSigner();
    final NoritoCodecAdapter codec = new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final TransactionBuilder builder =
        new TransactionBuilder(codec, IrohaKeyManager.withSoftwareProvider());

    final TransactionPayload direct = codec.decodeTransaction(codec.encodeTransaction(payload));
    assert direct.admissionIntent() == TransactionAdmissionIntent.ORDINARY
        : "Direct codec payloads must remain ordinary";
    try {
      NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(
          codec.encodeTransaction(payload), TransactionAdmissionIntent.QUEUE_PLAN_SYNCED);
      throw new AssertionError("QueuePlan validation must reject ordinary payloads");
    } catch (final org.hyperledger.iroha.android.norito.NoritoException expected) {
      // Expected.
    }

    final SignedTransaction signed = builder.encodeAndSign(payload, signer);
    final byte[] expectedSignature =
        repeatedByte(0x51, SignatureAdmission.ED25519_SIGNATURE_LENGTH);
    assert Arrays.equals(expectedSignature, signed.signature())
        : "Fake signer should return the configured signature";
    assert Arrays.equals(signed.encodedPayload(), signer.lastMessage())
        : "Fake signer should receive the canonical encoded payload";
    assert Arrays.equals("fake-public-key".getBytes(), signed.publicKey())
        : "Fake signer should return test public key";

    final TransactionPayload decoded = codec.decodeTransaction(signed.encodedPayload());
    NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(
        signed.encodedPayload(), TransactionAdmissionIntent.QUEUE_PLAN_SYNCED);
    assert decoded.networkId().equals(payload.networkId()) : "NetworkId must round-trip";
    assert decoded.authority().equals(payload.authority()) : "Authority must round-trip";
    assert decoded.creationTimeMs() == payload.creationTimeMs() : "Timestamp must round-trip";
    assert Arrays.equals(
            payload.executable().ivmBytes(), decoded.executable().ivmBytes())
        : "Norito codec must roundtrip instructions";
    assert decoded.timeToLiveMs().equals(payload.timeToLiveMs()) : "TTL must round-trip";
    assert decoded.nonce().equals(payload.nonce()) : "Nonce must round-trip";
    assert decoded.metadata().get("channel").equals(payload.metadata().get("channel"))
        : "Caller metadata must round-trip";
    assert decoded.admissionIntent() == TransactionAdmissionIntent.QUEUE_PLAN_SYNCED
        : "Public signing must bind QueuePlan admission";
    assert payload.admissionIntent() == TransactionAdmissionIntent.ORDINARY
        : "Public signing must not mutate the caller payload";
  }

  private static void publicBuilderPreservesExplicitQueuePlanIntent() throws Exception {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList(), 1L))
            .setNetworkId(TestNetworkIds.fromSeed(12L))
            .setAuthority(TestAccountIds.ed25519Authority(0x32))
            .setExecutable(Executable.ivm("canonical-marker".getBytes()))
            .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
            .build();
    final NoritoCodecAdapter codec =
        new NoritoJavaCodecAdapter(
            org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final SignedTransaction signed =
        new TransactionBuilder(codec, IrohaKeyManager.withSoftwareProvider())
            .encodeAndSign(payload, new FakeSigner());
    final TransactionPayload decoded = codec.decodeTransaction(signed.encodedPayload());

    assert decoded.admissionIntent() == TransactionAdmissionIntent.QUEUE_PLAN_SYNCED
        : "Public signing must preserve QueuePlan intent";
    assert payload.admissionIntent() == TransactionAdmissionIntent.QUEUE_PLAN_SYNCED
        : "Public signing must not mutate the caller payload";
  }

  private static void encodeAndSignWithKeyManagerAlias() throws Exception {
    final TransactionPayload payload =
        TransactionPayload.builder().setFeePayment(FeePaymentIntent.authority(Collections.emptyList(), 1L))
            .setNetworkId(TestNetworkIds.fromSeed(3L))
            .setAuthority(TestAccountIds.ed25519Authority(0x29))
            .setCreationTimeMs(1_735_000_111_000L)
            .setExecutable(Executable.ivm("alias-sign".getBytes()))
            .setNonce(null)
            .build();

    final IrohaKeyManager keyManager = IrohaKeyManager.withSoftwareProvider();
    final TransactionBuilder builder =
        new TransactionBuilder(new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT), keyManager);

    final SignedTransaction signed =
        builder.encodeAndSign(
            payload,
            "transaction-alias",
            IrohaKeyManager.KeySecurityPreference.SOFTWARE_ONLY);

    final TransactionPayload decoded =
        new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT).decodeTransaction(signed.encodedPayload());
    assert Arrays.equals(payload.executable().ivmBytes(), decoded.executable().ivmBytes())
        : "Decoded transaction must match original instructions";
    assert decoded.networkId().equals(payload.networkId()) : "NetworkId must match";
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
        TransactionPayload.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList()))
            .setNetworkId(TestNetworkIds.fromSeed(0L))
            .setAuthority(TestAccountIds.ed25519Authority(0x2A))
            .setExecutable(
                Executable.instructions(
                    List.of(
                        InstructionBox.fromWirePayload("iroha.register.domain", wirePayloadA),
                        InstructionBox.fromWirePayload("iroha.register.account", wirePayloadB))))
            .build();
    final TransactionBuilder builder =
        new TransactionBuilder(new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT), IrohaKeyManager.withSoftwareProvider());
    final SignedTransaction signed = builder.encodeAndSign(payload, new FakeSigner());
    final TransactionPayload decoded = new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT).decodeTransaction(signed.encodedPayload());
    assert decoded.executable().isInstructions() : "Executable variant must remain instructions";
    assert decoded.executable().instructions().equals(payload.executable().instructions())
        : "Instruction list must round-trip";
  }

  private static void mixedBatchBuilderAndSignerPreserveOrder() throws Exception {
    final InstructionBox first =
        InstructionBox.fromWirePayload(
            "iroha.batch.first",
            NoritoCodec.encode("first", "iroha.test.Batch", NoritoAdapters.stringAdapter()));
    final InstructionBox last =
        InstructionBox.fromWirePayload(
            "iroha.batch.last",
            NoritoCodec.encode("last", "iroha.test.Batch", NoritoAdapters.stringAdapter()));
    final ContractInvocation invocation =
        new ContractInvocation(
            "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh",
            repeatedByte(0x51, 32),
            "run",
            new byte[] {0x01, 0x02});
    final List<ExecutableBatchItem> batch =
        Arrays.asList(
            ExecutableBatchItem.instruction(first),
            ExecutableBatchItem.contractCall(invocation),
            ExecutableBatchItem.instruction(last));
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList(), 5_000L))
            .setNetworkId(TestNetworkIds.fromSeed(4L))
            .setAuthority(TestAccountIds.ed25519Authority(0x2A))
            .setCreationTimeMs(1_735_000_222_000L)
            .setBatch(batch)
            .build();

    final TransactionBuilder builder =
        new TransactionBuilder(
            new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT), IrohaKeyManager.withSoftwareProvider());
    final SignedTransaction signed = builder.encodeAndSign(payload, new FakeSigner());
    final TransactionPayload decoded =
        new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT).decodeTransaction(signed.encodedPayload());

    assert decoded.executable().isBatch() : "Executable variant must remain Batch";
    assert batch.equals(decoded.executable().batchItems())
        : "Signing must preserve mixed batch order";
  }

  private static void transactionPayloadRejectsPaddedAuthorityBeforeSigning() {
    final String authority = TestAccountIds.ed25519Authority(0x2F);
    assertIllegalArgumentMessage(
        () -> TransactionPayload.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList())).setAuthority(" " + authority),
        "authority must not contain surrounding whitespace");
  }

  private static void publicBuilderRejectsMalformedFixedShapeSignerOutput() throws Exception {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList(), 1L))
            .setNetworkId(TestNetworkIds.fromSeed(13L))
            .setAuthority(TestAccountIds.ed25519Authority(0x33))
            .setExecutable(Executable.instructions(Collections.emptyList()))
            .build();
    final TransactionBuilder builder =
        new TransactionBuilder(
            new NoritoJavaCodecAdapter(
                org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
            IrohaKeyManager.withSoftwareProvider());

    final int[] malformedLengths = {
      1,
      64,
      SignatureAdmission.ML_DSA_65_SIGNATURE_LENGTH - 1,
      SignatureAdmission.ML_DSA_65_SIGNATURE_LENGTH + 1
    };
    for (final int length : malformedLengths) {
      expectSigningFailure(
          () ->
              builder.encodeAndSign(
                  payload, new FakeSigner(repeatedByte(0x5A, length), "ML-DSA-65")),
          "ML-DSA-65 signer output length " + length);
    }
    expectSigningFailure(
        () ->
            builder.encodeAndSign(
                payload,
                new FakeSigner(
                    new byte[SignatureAdmission.ML_DSA_65_SIGNATURE_LENGTH], "ML-DSA-65")),
        "all-zero ML-DSA-65 signer output");
    expectSigningFailure(
        () ->
            builder.encodeAndSign(
                payload,
                new FakeSigner(
                    repeatedByte(0x5A, SignatureAdmission.ED25519_SIGNATURE_LENGTH - 1),
                    "Ed25519")),
        "short Ed25519 signer output");
    expectSigningFailure(
        () ->
            builder.encodeAndSign(
                payload,
                new FakeSigner(
                    new byte[SignatureAdmission.ED25519_SIGNATURE_LENGTH], "Ed25519")),
        "all-zero Ed25519 signer output");

    final byte[] validMlDsaSignature =
        repeatedByte(0x6B, SignatureAdmission.ML_DSA_65_SIGNATURE_LENGTH);
    final SignedTransaction signed =
        builder.encodeAndSign(payload, new FakeSigner(validMlDsaSignature, "ML-DSA-65"));
    assert Arrays.equals(validMlDsaSignature, signed.signature())
        : "canonical ML-DSA-65 signer output must be preserved";
  }

  private static final class FakeSigner implements Signer {
    private final byte[] signature;
    private final String algorithm;
    private byte[] lastMessage;

    private FakeSigner() {
      this(
          repeatedByte(0x51, SignatureAdmission.ED25519_SIGNATURE_LENGTH),
          "Ed25519");
    }

    private FakeSigner(final byte[] signature, final String algorithm) {
      this.signature = Arrays.copyOf(signature, signature.length);
      this.algorithm = algorithm;
    }

    @Override
    public byte[] sign(final byte[] message) throws SigningException {
      if (message == null) {
        throw new SigningException("message must not be null");
      }
      lastMessage = Arrays.copyOf(message, message.length);
      return Arrays.copyOf(signature, signature.length);
    }

    @Override
    public byte[] publicKey() {
      return "fake-public-key".getBytes();
    }

    @Override
    public String algorithm() {
      return algorithm;
    }

    private byte[] lastMessage() {
      return lastMessage == null ? null : Arrays.copyOf(lastMessage, lastMessage.length);
    }
  }

  private static void expectSigningFailure(
      final CheckedRunnable action, final String name) throws Exception {
    try {
      action.run();
    } catch (final SigningException expected) {
      return;
    }
    throw new AssertionError(name + " must be rejected");
  }

  private static byte[] repeatedByte(final int value, final int length) {
    final byte[] bytes = new byte[length];
    Arrays.fill(bytes, (byte) value);
    return bytes;
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

  @FunctionalInterface
  private interface CheckedRunnable {
    void run() throws Exception;
  }

}
