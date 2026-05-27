package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Canonical Norito and text handoff codecs for Offline Bearer v2 app transports. */
public final class OfflineBearerV2TextCodec {
  public static final String RECEIVE_REQUEST_TEXT_PREFIX = "wallet-offline-bearer-receive:";
  public static final String PAYMENT_TEXT_PREFIX = "wallet-offline-bearer-payment:";
  public static final String ACK_TEXT_PREFIX = "wallet-offline-bearer-ack:";

  private static final String POLICY_TYPE =
      "iroha_data_model::offline::model::OfflineBearerPolicyBundleV2";
  private static final String CERTIFICATE_TYPE =
      "iroha_data_model::offline::model::OfflineBearerCertificateV2";
  private static final String RECEIVE_REQUEST_TYPE =
      "iroha_data_model::offline::model::OfflineBearerReceiveRequestV2";
  private static final String DEBIT_RECEIPT_TYPE =
      "iroha_data_model::offline::model::OfflineBearerDebitReceiptV2";
  private static final String CREDIT_RECEIPT_TYPE =
      "iroha_data_model::offline::model::OfflineBearerCreditReceiptV2";
  private static final String SETTLEMENT_BATCH_TYPE =
      "iroha_data_model::offline::model::OfflineBearerSettlementBatchV2";

  private OfflineBearerV2TextCodec() {}

  public enum PayloadKind {
    RECEIVE_REQUEST,
    PAYMENT,
    ACK
  }

  public static byte[] encodePolicyBundleNorito(
      final OfflineBearerWallet.PolicyBundleV2 policy) {
    return NoritoCodec.encode(
        Objects.requireNonNull(policy, "policy"),
        POLICY_TYPE,
        POLICY_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static OfflineBearerWallet.PolicyBundleV2 decodePolicyBundleNorito(
      final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"), POLICY_ADAPTER, POLICY_TYPE);
  }

  public static byte[] encodeCertificateNorito(
      final OfflineBearerWallet.CertificateV2 certificate) {
    return NoritoCodec.encode(
        Objects.requireNonNull(certificate, "certificate"),
        CERTIFICATE_TYPE,
        CERTIFICATE_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static OfflineBearerWallet.CertificateV2 decodeCertificateNorito(final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"), CERTIFICATE_ADAPTER, CERTIFICATE_TYPE);
  }

  public static byte[] encodeReceiveRequestNorito(
      final OfflineBearerWallet.ReceiveRequestV2 request) {
    return NoritoCodec.encode(
        Objects.requireNonNull(request, "request"),
        RECEIVE_REQUEST_TYPE,
        RECEIVE_REQUEST_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static OfflineBearerWallet.ReceiveRequestV2 decodeReceiveRequestNorito(
      final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"), RECEIVE_REQUEST_ADAPTER, RECEIVE_REQUEST_TYPE);
  }

  public static byte[] encodeDebitReceiptNorito(
      final OfflineBearerWallet.DebitReceiptV2 receipt) {
    return NoritoCodec.encode(
        Objects.requireNonNull(receipt, "receipt"),
        DEBIT_RECEIPT_TYPE,
        DEBIT_RECEIPT_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static OfflineBearerWallet.DebitReceiptV2 decodeDebitReceiptNorito(
      final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"), DEBIT_RECEIPT_ADAPTER, DEBIT_RECEIPT_TYPE);
  }

  public static byte[] encodeCreditReceiptNorito(
      final OfflineBearerWallet.CreditReceiptV2 receipt) {
    return NoritoCodec.encode(
        Objects.requireNonNull(receipt, "receipt"),
        CREDIT_RECEIPT_TYPE,
        CREDIT_RECEIPT_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static OfflineBearerWallet.CreditReceiptV2 decodeCreditReceiptNorito(
      final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"), CREDIT_RECEIPT_ADAPTER, CREDIT_RECEIPT_TYPE);
  }

  public static byte[] encodeSettlementBatchNorito(
      final OfflineBearerWallet.SettlementBatchV2 batch) {
    return NoritoCodec.encode(
        Objects.requireNonNull(batch, "batch"),
        SETTLEMENT_BATCH_TYPE,
        SETTLEMENT_BATCH_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  public static OfflineBearerWallet.SettlementBatchV2 decodeSettlementBatchNorito(
      final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"),
        SETTLEMENT_BATCH_ADAPTER,
        SETTLEMENT_BATCH_TYPE);
  }

  public static String encodeReceiveRequestText(
      final OfflineBearerWallet.ReceiveRequestV2 request) {
    return RECEIVE_REQUEST_TEXT_PREFIX + encodeBase64Url(encodeReceiveRequestNorito(request));
  }

  public static OfflineBearerWallet.ReceiveRequestV2 decodeReceiveRequestText(
      final String text) {
    return decodeReceiveRequestNorito(
        decodeTextPayload(text, RECEIVE_REQUEST_TEXT_PREFIX, "receive request"));
  }

  public static String encodePaymentText(final OfflineBearerWallet.DebitReceiptV2 receipt) {
    return PAYMENT_TEXT_PREFIX + encodeBase64Url(encodeDebitReceiptNorito(receipt));
  }

  public static OfflineBearerWallet.DebitReceiptV2 decodePaymentText(final String text) {
    return decodeDebitReceiptNorito(decodeTextPayload(text, PAYMENT_TEXT_PREFIX, "payment"));
  }

  public static String encodeAckText(final OfflineBearerWallet.CreditReceiptV2 receipt) {
    return ACK_TEXT_PREFIX + encodeBase64Url(encodeCreditReceiptNorito(receipt));
  }

  public static OfflineBearerWallet.CreditReceiptV2 decodeAckText(final String text) {
    return decodeCreditReceiptNorito(decodeTextPayload(text, ACK_TEXT_PREFIX, "ack"));
  }

  public static PayloadKind payloadKind(final String text) {
    final String trimmed = Objects.requireNonNull(text, "text").trim();
    if (trimmed.startsWith(RECEIVE_REQUEST_TEXT_PREFIX)) {
      return PayloadKind.RECEIVE_REQUEST;
    }
    if (trimmed.startsWith(PAYMENT_TEXT_PREFIX)) {
      return PayloadKind.PAYMENT;
    }
    if (trimmed.startsWith(ACK_TEXT_PREFIX)) {
      return PayloadKind.ACK;
    }
    return null;
  }

  private static final TypeAdapter<OfflineBearerWallet.AssetSendLimitV2> ASSET_SEND_LIMIT_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final OfflineBearerWallet.AssetSendLimitV2 value) {
          writeField(encoder, child -> writeString(child, value.assetDefinitionId()));
          writeField(encoder, child -> writeString(child, value.maxTransactionAmount()));
          writeField(encoder, child -> writeString(child, value.dailySendLimit()));
          writeField(encoder, child -> writeString(child, value.monthlySendLimit()));
        }

        @Override
        public OfflineBearerWallet.AssetSendLimitV2 decode(final NoritoDecoder decoder) {
          return new OfflineBearerWallet.AssetSendLimitV2(
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString));
        }
      };

  private static final TypeAdapter<OfflineBearerWallet.PolicyBundleV2> POLICY_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final OfflineBearerWallet.PolicyBundleV2 value) {
          writeField(encoder, child -> writeString(child, value.policyId()));
          writeField(encoder, child -> writeString(child, value.policyHashHex()));
          writeField(encoder, child -> writeString(child, value.issuerId()));
          writeField(encoder, child -> child.writeUInt(value.issuedAtMs(), 64));
          writeField(encoder, child -> child.writeUInt(value.expiresAtMs(), 64));
          writeField(encoder, child -> child.writeUInt(value.maxCertificateAgeMs(), 64));
          writeField(encoder, child -> child.writeUInt(value.maxPolicyAgeMs(), 64));
          writeField(encoder, child -> child.writeUInt(value.maxTokenAgeMs(), 64));
          writeField(encoder, child -> writeString(child, value.maxOfflineBalance()));
          writeField(encoder, child -> writeString(child, value.maxTransactionAmount()));
          writeField(encoder, child -> writeStringList(child, sorted(value.allowedHardwareClasses())));
          writeField(encoder, child -> writeStringList(child, sorted(value.blacklistedAccountIds())));
          writeField(encoder, child -> writeStringList(child, sorted(value.blacklistedDeviceIds())));
          writeField(encoder, child -> writeStringList(child, sorted(value.blacklistedKeyIds())));
          writeField(encoder, child -> writeString(child, value.signatureAlgorithm()));
          writeField(encoder, child -> writeBytesVec(child, value.issuerSignature()));
          writeField(encoder, child -> child.writeUInt(value.policyEpoch(), 64));
          writeField(encoder, child -> writeString(child, value.policySource()));
          writeField(encoder, child -> writeStringList(child, sorted(value.revokedCertificateIds())));
          writeField(encoder, child -> writeStringList(child, sorted(value.revokedTransferIds())));
          writeField(encoder, child -> writeList(child, value.assetSendLimits(), ASSET_SEND_LIMIT_ADAPTER));
        }

        @Override
        public OfflineBearerWallet.PolicyBundleV2 decode(final NoritoDecoder decoder) {
          final String policyId = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String policyHashHex = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String issuerId = readField(decoder, OfflineBearerV2TextCodec::readString);
          final long issuedAtMs = readField(decoder, child -> child.readUInt(64));
          final long expiresAtMs = readField(decoder, child -> child.readUInt(64));
          final long maxCertificateAgeMs = readField(decoder, child -> child.readUInt(64));
          final long maxPolicyAgeMs = readField(decoder, child -> child.readUInt(64));
          final long maxTokenAgeMs = readField(decoder, child -> child.readUInt(64));
          final String maxOfflineBalance = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String maxTransactionAmount = readField(decoder, OfflineBearerV2TextCodec::readString);
          final List<String> allowedHardwareClasses =
              readField(decoder, OfflineBearerV2TextCodec::readStringList);
          final List<String> blacklistedAccountIds =
              readField(decoder, OfflineBearerV2TextCodec::readStringList);
          final List<String> blacklistedDeviceIds =
              readField(decoder, OfflineBearerV2TextCodec::readStringList);
          final List<String> blacklistedKeyIds =
              readField(decoder, OfflineBearerV2TextCodec::readStringList);
          final String signatureAlgorithm = readField(decoder, OfflineBearerV2TextCodec::readString);
          final byte[] issuerSignature = readField(decoder, OfflineBearerV2TextCodec::readBytesVec);
          final long policyEpoch = readField(decoder, child -> child.readUInt(64));
          final String policySource = readField(decoder, OfflineBearerV2TextCodec::readString);
          final List<String> revokedCertificateIds =
              readField(decoder, OfflineBearerV2TextCodec::readStringList);
          final List<String> revokedTransferIds =
              readField(decoder, OfflineBearerV2TextCodec::readStringList);
          final List<OfflineBearerWallet.AssetSendLimitV2> assetSendLimits =
              readField(decoder, child -> readList(child, ASSET_SEND_LIMIT_ADAPTER));
          return new OfflineBearerWallet.PolicyBundleV2(
              policyId,
              policyHashHex,
              issuerId,
              issuedAtMs,
              expiresAtMs,
              maxCertificateAgeMs,
              maxPolicyAgeMs,
              maxTokenAgeMs,
              maxOfflineBalance,
              maxTransactionAmount,
              allowedHardwareClasses,
              blacklistedAccountIds,
              blacklistedDeviceIds,
              blacklistedKeyIds,
              signatureAlgorithm,
              issuerSignature,
              policyEpoch,
              policySource,
              revokedCertificateIds,
              revokedTransferIds,
              assetSendLimits);
        }
      };

  private static final TypeAdapter<OfflineBearerWallet.CertificateV2> CERTIFICATE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final OfflineBearerWallet.CertificateV2 value) {
          writeField(encoder, child -> writeString(child, value.certificateId()));
          writeField(encoder, child -> writeString(child, value.chainId()));
          writeField(encoder, child -> writeString(child, value.issuerId()));
          writeField(encoder, child -> writeString(child, value.purseId()));
          writeField(encoder, child -> writeString(child, value.accountId()));
          writeField(encoder, child -> writeString(child, value.assetDefinitionId()));
          writeField(encoder, child -> writeString(child, value.deviceId()));
          writeField(encoder, child -> writeString(child, value.keyId()));
          writeField(encoder, child -> writeString(child, value.hardwareClass()));
          writeField(encoder, child -> writeString(child, value.signatureAlgorithm()));
          writeField(encoder, child -> writeString(child, value.publicKeyEncoding()));
          writeField(encoder, child -> writeBytesVec(child, value.publicKey()));
          writeField(encoder, child -> child.writeUInt(value.issuedAtMs(), 64));
          writeField(encoder, child -> child.writeUInt(value.expiresAtMs(), 64));
          writeField(encoder, child -> writeString(child, value.policyId()));
          writeField(encoder, child -> writeString(child, value.policyHashHex()));
          writeField(encoder, child -> writeBytesVec(child, value.issuerSignature()));
        }

        @Override
        public OfflineBearerWallet.CertificateV2 decode(final NoritoDecoder decoder) {
          final String certificateId = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String chainId = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String issuerId = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String purseId = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String accountId = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String assetDefinitionId = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String deviceId = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String keyId = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String hardwareClass = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String signatureAlgorithm = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String publicKeyEncoding = readField(decoder, OfflineBearerV2TextCodec::readString);
          final byte[] publicKey = readField(decoder, OfflineBearerV2TextCodec::readBytesVec);
          final long issuedAtMs = readField(decoder, child -> child.readUInt(64));
          final long expiresAtMs = readField(decoder, child -> child.readUInt(64));
          final String policyId = readField(decoder, OfflineBearerV2TextCodec::readString);
          final String policyHashHex = readField(decoder, OfflineBearerV2TextCodec::readString);
          final byte[] issuerSignature = readField(decoder, OfflineBearerV2TextCodec::readBytesVec);
          return new OfflineBearerWallet.CertificateV2(
              certificateId,
              chainId,
              issuerId,
              purseId,
              accountId,
              assetDefinitionId,
              deviceId,
              keyId,
              hardwareClass,
              signatureAlgorithm,
              publicKeyEncoding,
              publicKey,
              issuedAtMs,
              expiresAtMs,
              policyId,
              policyHashHex,
              issuerSignature);
        }
      };

  private static final TypeAdapter<OfflineBearerWallet.ReceiveRequestV2> RECEIVE_REQUEST_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final OfflineBearerWallet.ReceiveRequestV2 value) {
          writeField(encoder, child -> child.writeUInt(value.version(), 16));
          writeField(encoder, child -> writeString(child, value.chainId()));
          writeField(encoder, child -> writeString(child, value.paymentRequestId()));
          writeField(encoder, child -> CERTIFICATE_ADAPTER.encode(child, value.recipientCertificate()));
          writeField(encoder, child -> writeString(child, value.assetDefinitionId()));
          writeField(encoder, child -> writeString(child, value.amount()));
          writeField(encoder, child -> child.writeUInt(value.createdAtMs(), 64));
          writeField(encoder, child -> child.writeUInt(value.expiresAtMs(), 64));
          writeField(encoder, child -> writeString(child, value.policyHashHex()));
          writeField(encoder, child -> writeString(child, value.signatureAlgorithm()));
          writeField(encoder, child -> writeBytesVec(child, value.challengeSignature()));
        }

        @Override
        public OfflineBearerWallet.ReceiveRequestV2 decode(final NoritoDecoder decoder) {
          return new OfflineBearerWallet.ReceiveRequestV2(
              Math.toIntExact(readField(decoder, child -> child.readUInt(16))),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, child -> CERTIFICATE_ADAPTER.decode(child)),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, child -> child.readUInt(64)),
              readField(decoder, child -> child.readUInt(64)),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readBytesVec));
        }
      };

  private static final TypeAdapter<OfflineBearerWallet.DebitReceiptV2> DEBIT_RECEIPT_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final OfflineBearerWallet.DebitReceiptV2 value) {
          writeField(encoder, child -> child.writeUInt(value.version(), 16));
          writeField(encoder, child -> writeString(child, value.transferId()));
          writeField(encoder, child -> writeString(child, value.chainId()));
          writeField(encoder, child -> writeString(child, value.paymentRequestId()));
          writeField(encoder, child -> CERTIFICATE_ADAPTER.encode(child, value.senderCertificate()));
          writeField(encoder, child -> CERTIFICATE_ADAPTER.encode(child, value.recipientCertificate()));
          writeField(encoder, child -> writeString(child, value.assetDefinitionId()));
          writeField(encoder, child -> writeString(child, value.amount()));
          writeField(encoder, child -> writeString(child, value.senderPreBalance()));
          writeField(encoder, child -> writeString(child, value.senderPostBalance()));
          writeField(encoder, child -> child.writeUInt(value.senderSequence(), 64));
          writeField(encoder, child -> child.writeUInt(value.createdAtMs(), 64));
          writeField(encoder, child -> child.writeUInt(value.expiresAtMs(), 64));
          writeField(encoder, child -> writeString(child, value.policyHashHex()));
          writeField(encoder, child -> writeBytesVec(child, value.receiveChallengeSignature()));
          writeField(encoder, child -> writeString(child, value.signatureAlgorithm()));
          writeField(encoder, child -> writeBytesVec(child, value.debitSignature()));
        }

        @Override
        public OfflineBearerWallet.DebitReceiptV2 decode(final NoritoDecoder decoder) {
          return new OfflineBearerWallet.DebitReceiptV2(
              Math.toIntExact(readField(decoder, child -> child.readUInt(16))),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, child -> CERTIFICATE_ADAPTER.decode(child)),
              readField(decoder, child -> CERTIFICATE_ADAPTER.decode(child)),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, child -> child.readUInt(64)),
              readField(decoder, child -> child.readUInt(64)),
              readField(decoder, child -> child.readUInt(64)),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readBytesVec),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readBytesVec));
        }
      };

  private static final TypeAdapter<OfflineBearerWallet.CreditReceiptV2> CREDIT_RECEIPT_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final OfflineBearerWallet.CreditReceiptV2 value) {
          writeField(encoder, child -> child.writeUInt(value.version(), 16));
          writeField(encoder, child -> writeString(child, value.transferId()));
          writeField(encoder, child -> writeString(child, value.chainId()));
          writeField(encoder, child -> CERTIFICATE_ADAPTER.encode(child, value.recipientCertificate()));
          writeField(encoder, child -> writeString(child, value.amount()));
          writeField(encoder, child -> writeString(child, value.recipientPreBalance()));
          writeField(encoder, child -> writeString(child, value.recipientPostBalance()));
          writeField(encoder, child -> child.writeUInt(value.recipientSequence(), 64));
          writeField(encoder, child -> child.writeUInt(value.acceptedAtMs(), 64));
          writeField(encoder, child -> writeString(child, value.signatureAlgorithm()));
          writeField(encoder, child -> writeBytesVec(child, value.creditSignature()));
        }

        @Override
        public OfflineBearerWallet.CreditReceiptV2 decode(final NoritoDecoder decoder) {
          return new OfflineBearerWallet.CreditReceiptV2(
              Math.toIntExact(readField(decoder, child -> child.readUInt(16))),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, child -> CERTIFICATE_ADAPTER.decode(child)),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, child -> child.readUInt(64)),
              readField(decoder, child -> child.readUInt(64)),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readBytesVec));
        }
      };

  private static final TypeAdapter<OfflineBearerWallet.SettlementBatchV2> SETTLEMENT_BATCH_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final OfflineBearerWallet.SettlementBatchV2 value) {
          writeField(encoder, child -> child.writeUInt(value.version(), 16));
          writeField(encoder, child -> writeString(child, value.chainId()));
          writeField(encoder, child -> writeString(child, value.purseId()));
          writeField(encoder, child -> writeList(child, value.debitReceipts(), DEBIT_RECEIPT_ADAPTER));
          writeField(encoder, child -> writeList(child, value.creditReceipts(), CREDIT_RECEIPT_ADAPTER));
        }

        @Override
        public OfflineBearerWallet.SettlementBatchV2 decode(final NoritoDecoder decoder) {
          return new OfflineBearerWallet.SettlementBatchV2(
              Math.toIntExact(readField(decoder, child -> child.readUInt(16))),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, OfflineBearerV2TextCodec::readString),
              readField(decoder, child -> readList(child, DEBIT_RECEIPT_ADAPTER)),
              readField(decoder, child -> readList(child, CREDIT_RECEIPT_ADAPTER)));
        }
      };

  private static String encodeBase64Url(final byte[] payload) {
    return Base64.getUrlEncoder().withoutPadding().encodeToString(payload);
  }

  private static List<String> sorted(final Iterable<String> values) {
    final List<String> sorted = new ArrayList<>();
    for (final String value : values) {
      sorted.add(value);
    }
    Collections.sort(sorted);
    return sorted;
  }

  private static byte[] decodeTextPayload(
      final String text, final String prefix, final String label) {
    final String trimmed = Objects.requireNonNull(text, "text").trim();
    if (!trimmed.startsWith(prefix)) {
      throw new IllegalArgumentException("Offline Bearer " + label + " prefix missing");
    }
    return Base64.getUrlDecoder().decode(trimmed.substring(prefix.length()));
  }

  private interface FieldWriter {
    void write(NoritoEncoder encoder);
  }

  private interface FieldReader<T> {
    T read(NoritoDecoder decoder);
  }

  private static void writeField(final NoritoEncoder encoder, final FieldWriter write) {
    final NoritoEncoder child = encoder.childEncoder();
    write.write(child);
    final byte[] payload = child.toByteArray();
    encoder.writeLength(payload.length, true);
    encoder.writeBytes(payload);
  }

  private static <T> T readField(final NoritoDecoder decoder, final FieldReader<T> read) {
    final long length = decoder.readLength(true);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Offline Bearer field length overflow");
    }
    final NoritoDecoder child =
        new NoritoDecoder(decoder.readBytes((int) length), decoder.flags(), decoder.flagsHint());
    final T value = read.read(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after Offline Bearer field decode");
    }
    return value;
  }

  private static void writeString(final NoritoEncoder encoder, final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    encoder.writeLength(bytes.length, true);
    encoder.writeBytes(bytes);
  }

  private static String readString(final NoritoDecoder decoder) {
    final long length = decoder.readLength(true);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Offline Bearer string length overflow");
    }
    return new String(decoder.readBytes((int) length), StandardCharsets.UTF_8);
  }

  private static void writeBytesVec(final NoritoEncoder encoder, final byte[] value) {
    encoder.writeUInt(value.length, 64);
    encoder.writeBytes(value);
  }

  private static byte[] readBytesVec(final NoritoDecoder decoder) {
    final long length = decoder.readUInt(64);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Offline Bearer bytes length overflow");
    }
    return decoder.readBytes((int) length);
  }

  private static final TypeAdapter<String> STRING_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final String value) {
          writeString(encoder, value);
        }

        @Override
        public String decode(final NoritoDecoder decoder) {
          return readString(decoder);
        }
      };

  private static void writeStringList(final NoritoEncoder encoder, final List<String> values) {
    writeList(encoder, values, STRING_ADAPTER);
  }

  private static List<String> readStringList(final NoritoDecoder decoder) {
    return readList(decoder, STRING_ADAPTER);
  }

  private static <T> void writeList(
      final NoritoEncoder encoder, final List<T> values, final TypeAdapter<T> adapter) {
    encoder.writeLength(values.size(), false);
    for (final T value : values) {
      writeField(encoder, child -> adapter.encode(child, value));
    }
  }

  private static <T> List<T> readList(
      final NoritoDecoder decoder, final TypeAdapter<T> adapter) {
    final long count = decoder.readLength(false);
    if (count > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Offline Bearer list length overflow");
    }
    final List<T> values = new ArrayList<>((int) count);
    for (int index = 0; index < count; index++) {
      values.add(readField(decoder, child -> adapter.decode(child)));
    }
    return values;
  }
}
