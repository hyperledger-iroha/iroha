package org.hyperledger.iroha.android.offline;

import android.content.Context;
import android.os.Build;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyInfo;
import android.security.keystore.KeyProperties;
import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.math.BigDecimal;
import java.security.KeyFactory;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.KeyStore;
import java.security.PrivateKey;
import java.security.Signature;
import java.security.cert.Certificate;
import java.security.interfaces.ECPublicKey;
import java.security.spec.ECGenParameterSpec;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.List;
import java.util.Objects;

/** Strict Android KeyMint/StrongBox P-256 secure element for Offline Bearer purses. */
public final class AndroidKeyMintOfflineBearerSecureElement
    implements OfflineBearerWallet.SecureElement {
  private static final String ANDROID_KEYSTORE = "AndroidKeyStore";
  private static final String HARDWARE_CLASS_STRONGBOX = "strongbox";
  private static final byte[] DUMMY_SIGNATURE = new byte[] {0};

  private final String keyAlias;
  private final PurseStore store;
  private final PrivateKey privateKey;
  private final byte[] publicKeyX963;
  private final byte[] attestationEvidence;

  private AndroidKeyMintOfflineBearerSecureElement(
      final String keyAlias,
      final PurseStore store,
      final PrivateKey privateKey,
      final byte[] publicKeyX963,
      final byte[] attestationEvidence) {
    this.keyAlias = requireNonBlank(keyAlias, "keyAlias");
    this.store = Objects.requireNonNull(store, "store");
    this.privateKey = Objects.requireNonNull(privateKey, "privateKey");
    this.publicKeyX963 = Arrays.copyOf(publicKeyX963, publicKeyX963.length);
    this.attestationEvidence = Arrays.copyOf(attestationEvidence, attestationEvidence.length);
    if (!store.rollbackResistant()) {
      throw new OfflineBearerWallet.PolicyException(
          "Offline Bearer requires rollback-resistant purse state");
    }
    if (publicKeyX963.length != 65 || publicKeyX963[0] != 0x04) {
      throw new OfflineBearerWallet.PolicyException(
          "Offline Bearer requires an uncompressed P-256 public key");
    }
    if (attestationEvidence.length == 0) {
      throw new OfflineBearerWallet.PolicyException(
          "Offline Bearer requires KeyMint attestation evidence");
    }
  }

  /**
   * Create a strict production secure element.
   *
   * <p>This constructor fails closed unless a StrongBox P-256 signing key can be generated or
   * loaded, the private key is non-extractable and hardware-backed, attestation evidence is
   * available, and the supplied purse store declares rollback resistance.
   */
  public static AndroidKeyMintOfflineBearerSecureElement createStrict(
      final Context context,
      final String keyAlias,
      final byte[] attestationChallenge,
      final PurseStore store) {
    Objects.requireNonNull(context, "context");
    requireNonBlank(keyAlias, "keyAlias");
    Objects.requireNonNull(attestationChallenge, "attestationChallenge");
    Objects.requireNonNull(store, "store");
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.P) {
      throw new OfflineBearerWallet.PolicyException(
          "Offline Bearer requires Android P or newer StrongBox KeyMint support");
    }
    try {
      final KeyStore keyStore = KeyStore.getInstance(ANDROID_KEYSTORE);
      keyStore.load(null);
      if (!keyStore.containsAlias(keyAlias)) {
        generateStrongBoxKey(keyAlias, attestationChallenge);
      }
      final PrivateKey privateKey = (PrivateKey) keyStore.getKey(keyAlias, null);
      if (privateKey == null || privateKey.getEncoded() != null) {
        throw new OfflineBearerWallet.PolicyException(
            "Offline Bearer KeyMint private key must be non-extractable");
      }
      final KeyFactory keyFactory = KeyFactory.getInstance(privateKey.getAlgorithm(), ANDROID_KEYSTORE);
      final KeyInfo keyInfo = keyFactory.getKeySpec(privateKey, KeyInfo.class);
      if (!keyInfo.isInsideSecureHardware()) {
        throw new OfflineBearerWallet.PolicyException(
            "Offline Bearer KeyMint key must be hardware-backed");
      }
      final Certificate certificate = keyStore.getCertificate(keyAlias);
      if (certificate == null || !(certificate.getPublicKey() instanceof ECPublicKey)) {
        throw new OfflineBearerWallet.PolicyException(
            "Offline Bearer KeyMint attestation certificate is missing a P-256 public key");
      }
      final byte[] publicKey = publicKeyX963((ECPublicKey) certificate.getPublicKey());
      final byte[] evidence = encodeCertificateChain(keyStore.getCertificateChain(keyAlias));
      return new AndroidKeyMintOfflineBearerSecureElement(
          keyAlias, store, privateKey, publicKey, evidence);
    } catch (final OfflineBearerWallet.PolicyException ex) {
      throw ex;
    } catch (final Exception ex) {
      throw new OfflineBearerWallet.PolicyException(
          "Offline Bearer KeyMint secure element is unavailable: " + ex.getMessage());
    }
  }

  @Override
  public OfflineBearerWallet.SecureElementCapabilities capabilities() {
    return new OfflineBearerWallet.SecureElementCapabilities(
        true,
        true,
        HARDWARE_CLASS_STRONGBOX,
        keyAlias,
        OfflineBearerWallet.SIGNATURE_ALGORITHM_ECDSA_P256_SHA256,
        OfflineBearerWallet.PUBLIC_KEY_ENCODING_X963_P256,
        true,
        attestationEvidence);
  }

  @Override
  public OfflineBearerWallet.CertificateV2 currentCertificate() {
    final PurseRecord record = store.load();
    return record == null ? null : record.certificate();
  }

  @Override
  public OfflineBearerWallet.PurseStateV2 currentState() {
    final PurseRecord record = store.load();
    return record == null ? null : record.state();
  }

  @Override
  public void installPurse(
      final OfflineBearerWallet.CertificateV2 certificate,
      final OfflineBearerWallet.PurseStateV2 state) {
    requireCertificateMatchesKey(certificate);
    store.save(new PurseRecord(certificate, state));
  }

  @Override
  public OfflineBearerWallet.ReceiveRequestV2 createReceiveRequest(
      final String paymentRequestId,
      final String amount,
      final long createdAtMs,
      final long expiresAtMs,
      final String policyHashHex) {
    final OfflineBearerWallet.CertificateV2 certificate = requireRecord().certificate();
    final OfflineBearerWallet.ReceiveRequestV2 unsigned =
        new OfflineBearerWallet.ReceiveRequestV2(
            OfflineBearerWallet.ReceiveRequestV2.VERSION,
            certificate.chainId(),
            paymentRequestId,
            certificate,
            certificate.assetDefinitionId(),
            amount,
            createdAtMs,
            expiresAtMs,
            policyHashHex,
            certificate.signatureAlgorithm(),
            DUMMY_SIGNATURE);
    return new OfflineBearerWallet.ReceiveRequestV2(
        unsigned.version(),
        unsigned.chainId(),
        unsigned.paymentRequestId(),
        unsigned.recipientCertificate(),
        unsigned.assetDefinitionId(),
        unsigned.amount(),
        unsigned.createdAtMs(),
        unsigned.expiresAtMs(),
        unsigned.policyHashHex(),
        unsigned.signatureAlgorithm(),
        sign(OfflineBearerWallet.Payloads.receiveRequestUnsignedPayload(unsigned)));
  }

  @Override
  public OfflineBearerWallet.DebitReceiptV2 debit(
      final OfflineBearerWallet.ReceiveRequestV2 request,
      final String transferId,
      final long createdAtMs,
      final long expiresAtMs) {
    final PurseRecord record = requireRecord();
    final OfflineBearerWallet.CertificateV2 certificate = record.certificate();
    final OfflineBearerWallet.PurseStateV2 state = record.state();
    final String postBalance = subtract(state.balance(), request.amount());
    final long nextSequence = state.sequence() + 1L;
    final OfflineBearerWallet.DebitReceiptV2 unsigned =
        new OfflineBearerWallet.DebitReceiptV2(
            OfflineBearerWallet.DebitReceiptV2.VERSION,
            transferId,
            certificate.chainId(),
            request.paymentRequestId(),
            certificate,
            request.recipientCertificate(),
            request.assetDefinitionId(),
            request.amount(),
            state.balance(),
            postBalance,
            nextSequence,
            createdAtMs,
            expiresAtMs,
            request.policyHashHex(),
            request.challengeSignature(),
            certificate.signatureAlgorithm(),
            DUMMY_SIGNATURE);
    final OfflineBearerWallet.DebitReceiptV2 receipt =
        new OfflineBearerWallet.DebitReceiptV2(
            unsigned.version(),
            unsigned.transferId(),
            unsigned.chainId(),
            unsigned.paymentRequestId(),
            unsigned.senderCertificate(),
            unsigned.recipientCertificate(),
            unsigned.assetDefinitionId(),
            unsigned.amount(),
            unsigned.senderPreBalance(),
            unsigned.senderPostBalance(),
            unsigned.senderSequence(),
            unsigned.createdAtMs(),
            unsigned.expiresAtMs(),
            unsigned.policyHashHex(),
            unsigned.receiveChallengeSignature(),
            unsigned.signatureAlgorithm(),
            sign(OfflineBearerWallet.Payloads.debitReceiptUnsignedPayload(unsigned)));
    store.save(
        new PurseRecord(
            certificate,
            new OfflineBearerWallet.PurseStateV2(
                state.chainId(),
                state.accountId(),
                state.assetDefinitionId(),
                state.purseId(),
                postBalance,
                nextSequence,
                state.policyHashHex(),
                createdAtMs)));
    store.appendDebitReceipt(receipt);
    return receipt;
  }

  @Override
  public OfflineBearerWallet.CreditReceiptV2 credit(
      final OfflineBearerWallet.DebitReceiptV2 receipt, final long acceptedAtMs) {
    final PurseRecord record = requireRecord();
    final OfflineBearerWallet.CertificateV2 certificate = record.certificate();
    final OfflineBearerWallet.PurseStateV2 state = record.state();
    final String postBalance = add(state.balance(), receipt.amount());
    final long nextSequence = state.sequence() + 1L;
    final OfflineBearerWallet.CreditReceiptV2 unsigned =
        new OfflineBearerWallet.CreditReceiptV2(
            OfflineBearerWallet.CreditReceiptV2.VERSION,
            receipt.transferId(),
            receipt.chainId(),
            certificate,
            receipt.amount(),
            state.balance(),
            postBalance,
            nextSequence,
            acceptedAtMs,
            certificate.signatureAlgorithm(),
            DUMMY_SIGNATURE);
    final OfflineBearerWallet.CreditReceiptV2 credit =
        new OfflineBearerWallet.CreditReceiptV2(
            unsigned.version(),
            unsigned.transferId(),
            unsigned.chainId(),
            unsigned.recipientCertificate(),
            unsigned.amount(),
            unsigned.recipientPreBalance(),
            unsigned.recipientPostBalance(),
            unsigned.recipientSequence(),
            unsigned.acceptedAtMs(),
            unsigned.signatureAlgorithm(),
            sign(OfflineBearerWallet.Payloads.creditReceiptUnsignedPayload(unsigned)));
    store.save(
        new PurseRecord(
            certificate,
            new OfflineBearerWallet.PurseStateV2(
                state.chainId(),
                state.accountId(),
                state.assetDefinitionId(),
                state.purseId(),
                postBalance,
                nextSequence,
                state.policyHashHex(),
                acceptedAtMs)));
    store.appendCreditReceipt(credit);
    return credit;
  }

  @Override
  public OfflineBearerWallet.SettlementBatchV2 exportSettlementBatch(final int maxReceipts) {
    if (maxReceipts <= 0) {
      throw new IllegalArgumentException("maxReceipts must be positive");
    }
    final PurseRecord record = requireRecord();
    return new OfflineBearerWallet.SettlementBatchV2(
        OfflineBearerWallet.SettlementBatchV2.VERSION,
        record.state().chainId(),
        record.state().purseId(),
        takeLast(store.debitReceipts(), maxReceipts),
        takeLast(store.creditReceipts(), maxReceipts));
  }

  @Override
  public void pruneSettled(final Collection<String> transferIds) {
    store.pruneSettled(transferIds);
  }

  /** Rollback-resistant storage for a hardware purse record and pending receipts. */
  public interface PurseStore {
    boolean rollbackResistant();

    PurseRecord load();

    void save(PurseRecord record);

    List<OfflineBearerWallet.DebitReceiptV2> debitReceipts();

    List<OfflineBearerWallet.CreditReceiptV2> creditReceipts();

    void appendDebitReceipt(OfflineBearerWallet.DebitReceiptV2 receipt);

    void appendCreditReceipt(OfflineBearerWallet.CreditReceiptV2 receipt);

    void pruneSettled(Collection<String> transferIds);
  }

  /** Stored hardware purse record. */
  public static final class PurseRecord {
    private final OfflineBearerWallet.CertificateV2 certificate;
    private final OfflineBearerWallet.PurseStateV2 state;

    public PurseRecord(
        final OfflineBearerWallet.CertificateV2 certificate,
        final OfflineBearerWallet.PurseStateV2 state) {
      this.certificate = Objects.requireNonNull(certificate, "certificate");
      this.state = Objects.requireNonNull(state, "state");
    }

    public OfflineBearerWallet.CertificateV2 certificate() {
      return certificate;
    }

    public OfflineBearerWallet.PurseStateV2 state() {
      return state;
    }
  }

  private static void generateStrongBoxKey(final String keyAlias, final byte[] challenge)
      throws Exception {
    final KeyGenParameterSpec.Builder builder =
        new KeyGenParameterSpec.Builder(
                keyAlias, KeyProperties.PURPOSE_SIGN | KeyProperties.PURPOSE_VERIFY)
            .setAlgorithmParameterSpec(new ECGenParameterSpec("secp256r1"))
            .setDigests(KeyProperties.DIGEST_SHA256)
            .setAttestationChallenge(Arrays.copyOf(challenge, challenge.length));
    builder.setIsStrongBoxBacked(true);
    final KeyPairGenerator generator =
        KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, ANDROID_KEYSTORE);
    generator.initialize(builder.build());
    final KeyPair ignored = generator.generateKeyPair();
    if (ignored.getPrivate() == null) {
      throw new OfflineBearerWallet.PolicyException("StrongBox key generation did not return a key");
    }
  }

  private PurseRecord requireRecord() {
    final PurseRecord record = store.load();
    if (record == null) {
      throw new OfflineBearerWallet.PolicyException("Offline Bearer purse is not installed");
    }
    requireCertificateMatchesKey(record.certificate());
    return record;
  }

  private void requireCertificateMatchesKey(final OfflineBearerWallet.CertificateV2 certificate) {
    if (!OfflineBearerWallet.SIGNATURE_ALGORITHM_ECDSA_P256_SHA256.equals(
            certificate.signatureAlgorithm())
        || !OfflineBearerWallet.PUBLIC_KEY_ENCODING_X963_P256.equals(certificate.publicKeyEncoding())
        || !Arrays.equals(certificate.publicKey(), publicKeyX963)) {
      throw new OfflineBearerWallet.PolicyException(
          "Offline Bearer certificate does not match the StrongBox purse key");
    }
  }

  private byte[] sign(final byte[] payload) {
    try {
      final Signature signer = Signature.getInstance("SHA256withECDSA");
      signer.initSign(privateKey);
      signer.update(payload);
      return signer.sign();
    } catch (final Exception ex) {
      throw new OfflineBearerWallet.PolicyException(
          "Offline Bearer KeyMint signature failed: " + ex.getMessage());
    }
  }

  private static byte[] encodeCertificateChain(final Certificate[] certificates) throws Exception {
    if (certificates == null || certificates.length == 0) {
      return new byte[0];
    }
    final ByteArrayOutputStream bytes = new ByteArrayOutputStream();
    final DataOutputStream output = new DataOutputStream(bytes);
    for (final Certificate certificate : certificates) {
      final byte[] encoded = certificate.getEncoded();
      output.writeInt(encoded.length);
      output.write(encoded);
    }
    output.flush();
    return bytes.toByteArray();
  }

  private static byte[] publicKeyX963(final ECPublicKey key) {
    final byte[] x = fixedWidthUnsigned(key.getW().getAffineX().toByteArray(), 32);
    final byte[] y = fixedWidthUnsigned(key.getW().getAffineY().toByteArray(), 32);
    final byte[] encoded = new byte[65];
    encoded[0] = 0x04;
    System.arraycopy(x, 0, encoded, 1, x.length);
    System.arraycopy(y, 0, encoded, 33, y.length);
    return encoded;
  }

  private static byte[] fixedWidthUnsigned(final byte[] value, final int width) {
    final byte[] result = new byte[width];
    final int sourceOffset = Math.max(0, value.length - width);
    final int length = Math.min(value.length, width);
    System.arraycopy(value, sourceOffset, result, width - length, length);
    return result;
  }

  private static String add(final String lhs, final String rhs) {
    return canonical(new BigDecimal(lhs).add(new BigDecimal(rhs)));
  }

  private static String subtract(final String lhs, final String rhs) {
    final BigDecimal result = new BigDecimal(lhs).subtract(new BigDecimal(rhs));
    if (result.compareTo(BigDecimal.ZERO) < 0) {
      throw new OfflineBearerWallet.PolicyException("Offline Bearer purse balance is insufficient");
    }
    return canonical(result);
  }

  private static String canonical(final BigDecimal value) {
    if (value.compareTo(BigDecimal.ZERO) == 0) {
      return "0";
    }
    return value.stripTrailingZeros().toPlainString();
  }

  private static <T> List<T> takeLast(final List<T> values, final int max) {
    final List<T> copy = new ArrayList<>(Objects.requireNonNull(values, "values"));
    if (copy.size() <= max) {
      return copy;
    }
    return copy.subList(copy.size() - max, copy.size());
  }

  private static String requireNonBlank(final String value, final String name) {
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalArgumentException(name + " must not be blank");
    }
    return value;
  }
}
