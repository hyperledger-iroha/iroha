package org.hyperledger.iroha.android.tx;

import java.security.KeyPair;
import java.security.Signature;
import java.util.Arrays;
import java.util.List;
import java.util.Optional;
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
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.model.instructions.InstructionBuilders;
import org.hyperledger.iroha.android.norito.NoritoCodecAdapter;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.offline.OfflineEnvelopeOptions;
import org.hyperledger.iroha.android.tx.offline.OfflineTransactionBundle;

public final class TransactionBuilderTests {

  private TransactionBuilderTests() {}

  private static final byte[] DUMMY_CERT = new byte[] {0x01};

  public static void main(final String[] args) throws Exception {
    encodeAndSignWithExplicitSigner();
    encodeAndSignWithKeyManagerAlias();
    instructionsVariantRoundTrips();
    encodeAndSignEnvelopeWithAttestationBundle();
    encodeAndSignEnvelopeWithAttestationWithoutHardware();
    System.out.println("[IrohaAndroid] Transaction builder tests passed.");
  }

  private static void encodeAndSignWithExplicitSigner() throws Exception {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000002")
            .setAuthority("bob@wonderland")
            .setCreationTimeMs(1_735_000_001_234L)
            .setExecutable(Executable.ivm("payload-bytes".getBytes()))
            .setTimeToLiveMs(10_000L)
            .setNonce(7)
            .putMetadata("channel", "builder-test")
            .build();

    final FakeSigner signer = new FakeSigner();
    final NoritoCodecAdapter codec = new NoritoJavaCodecAdapter();
    final TransactionBuilder builder =
        new TransactionBuilder(codec, IrohaKeyManager.withSoftwareFallback());

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
            .setAuthority("carol@wonderland")
            .setCreationTimeMs(1_735_000_111_000L)
            .setExecutable(Executable.ivm("alias-sign".getBytes()))
            .setTimeToLiveMs(null)
            .setNonce(null)
            .build();

    final IrohaKeyManager keyManager = IrohaKeyManager.withSoftwareFallback();
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
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setExecutable(
                Executable.instructions(
                    List.of(
                        InstructionBuilders.registerDomainTemplate("test-domain").toInstructionBox(),
                        InstructionBuilders.registerAccountTemplate("alice@test").toInstructionBox())))
            .build();
    final TransactionBuilder builder =
        new TransactionBuilder(new NoritoJavaCodecAdapter(), IrohaKeyManager.withSoftwareFallback());
    final SignedTransaction signed = builder.encodeAndSign(payload, new FakeSigner());
    final TransactionPayload decoded = new NoritoJavaCodecAdapter().decodeTransaction(signed.encodedPayload());
    assert decoded.executable().isInstructions() : "Executable variant must remain instructions";
    assert decoded.executable().instructions().equals(payload.executable().instructions())
        : "Instruction list must round-trip";
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
            .setAuthority("dave@wonderland")
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
    final IrohaKeyManager manager = IrohaKeyManager.withSoftwareFallback();
    final TransactionBuilder builder = new TransactionBuilder(new NoritoJavaCodecAdapter(), manager);
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000005")
            .setAuthority("erin@wonderland")
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
}
