package org.hyperledger.iroha.android.model.instructions;

import java.util.Objects;
import org.hyperledger.iroha.android.client.IdentifierReceiptCanonicalEncoder;
import org.hyperledger.iroha.android.client.IdentifierResolutionReceipt;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/**
 * Encodes {@code ClaimIdentifier} instructions in wire-framed Norito format.
 *
 * <p>The Torii identifier endpoints expose canonical {@code payload} plus {@code attestation}
 * receipts, so the encoder derives the canonical payload bytes and writes the explicit attestation.
 */
public final class ClaimIdentifierWirePayloadEncoder {

  public static final String WIRE_NAME = "identity::ClaimIdentifier";

  private static final String SCHEMA_PATH = "iroha_data_model::isi::identifier::ClaimIdentifier";

  private ClaimIdentifierWirePayloadEncoder() {}

  /**
   * Encodes a signed {@code ClaimIdentifier} instruction as a wire-framed {@link InstructionBox}.
   */
  public static InstructionBox encode(
      final String accountId, final IdentifierResolutionReceipt receipt) {
    Objects.requireNonNull(accountId, "accountId");
    Objects.requireNonNull(receipt, "receipt");
    final String normalizedAccountId = requireExactNonBlank(accountId, "accountId");
    final String receiptAccountId = requireExactNonBlank(receipt.accountId(), "receipt.accountId");
    if (!normalizedAccountId.equals(receiptAccountId)) {
      throw new IllegalArgumentException(
          "ClaimIdentifier accountId must match receipt.accountId");
    }
    final byte[] accountPayload =
        TransferWirePayloadEncoder.encodeAccountIdPayload(normalizedAccountId);
    final byte[] receiptPayload = IdentifierReceiptCanonicalEncoder.encodePayload(receipt.payload());
    final byte[] attestationPayload =
        IdentifierReceiptCanonicalEncoder.encodeAttestation(receipt.attestation());
    final byte[] wirePayload =
        NoritoCodec.encode(
            new ClaimIdentifierPayload(accountPayload, receiptPayload, attestationPayload),
            SCHEMA_PATH,
            new ClaimIdentifierPayloadAdapter());
    return InstructionBox.fromWirePayload(WIRE_NAME, wirePayload);
  }

  /** Decodes a Norito-framed {@code ClaimIdentifier} payload. */
  static DecodedClaimIdentifierPayload decodePayload(final byte[] wirePayload) {
    Objects.requireNonNull(wirePayload, "wirePayload");
    final ClaimIdentifierPayload payload =
        NoritoCodec.decode(wirePayload, new ClaimIdentifierPayloadAdapter(), SCHEMA_PATH);
    return new DecodedClaimIdentifierPayload(
        TransferWirePayloadEncoder.decodeAccountIdPayload(payload.accountPayload),
        payload.receiptPayload,
        payload.attestationPayload);
  }

  /** Decoded claim identifier payload with canonical account text and raw receipt bytes. */
  static final class DecodedClaimIdentifierPayload {
    private final String accountId;
    private final byte[] receiptPayloadBytes;
    private final byte[] attestationPayloadBytes;

    private DecodedClaimIdentifierPayload(
        final String accountId,
        final byte[] receiptPayloadBytes,
        final byte[] attestationPayloadBytes) {
      this.accountId = accountId;
      this.receiptPayloadBytes = receiptPayloadBytes.clone();
      this.attestationPayloadBytes = attestationPayloadBytes.clone();
    }

    String accountId() {
      return accountId;
    }

    byte[] receiptPayloadBytes() {
      return receiptPayloadBytes.clone();
    }

    byte[] attestationPayloadBytes() {
      return attestationPayloadBytes.clone();
    }
  }

  private static final class ClaimIdentifierPayload {
    private final byte[] accountPayload;
    private final byte[] receiptPayload;
    private final byte[] attestationPayload;

    private ClaimIdentifierPayload(
        final byte[] accountPayload, final byte[] receiptPayload, final byte[] attestationPayload) {
      this.accountPayload = accountPayload.clone();
      this.receiptPayload = receiptPayload.clone();
      this.attestationPayload = attestationPayload.clone();
    }
  }

  private static final class ClaimIdentifierPayloadAdapter
      implements TypeAdapter<ClaimIdentifierPayload> {
    private static final TypeAdapter<byte[]> PASSTHROUGH_ADAPTER = new PassthroughBytesAdapter();
    private static final TypeAdapter<ReceiptPayload> RECEIPT_ADAPTER = new ReceiptPayloadAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final ClaimIdentifierPayload value) {
      encodeSizedField(encoder, PASSTHROUGH_ADAPTER, value.accountPayload);
      encodeSizedField(
          encoder,
          RECEIPT_ADAPTER,
          new ReceiptPayload(value.receiptPayload, value.attestationPayload));
    }

    @Override
    public ClaimIdentifierPayload decode(
        final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      final byte[] accountPayload =
          decodeSizedField(decoder, PASSTHROUGH_ADAPTER, "ClaimIdentifier.account_id");
      final ReceiptPayload receipt =
          decodeSizedField(decoder, RECEIPT_ADAPTER, "ClaimIdentifier.receipt");
      return new ClaimIdentifierPayload(
          accountPayload, receipt.payloadBytes, receipt.attestationBytes);
    }
  }

  private static final class ReceiptPayload {
    private final byte[] payloadBytes;
    private final byte[] attestationBytes;

    private ReceiptPayload(final byte[] payloadBytes, final byte[] attestationBytes) {
      this.payloadBytes = payloadBytes.clone();
      this.attestationBytes = attestationBytes.clone();
    }
  }

  private static final class ReceiptPayloadAdapter implements TypeAdapter<ReceiptPayload> {
    private static final TypeAdapter<byte[]> PASSTHROUGH_ADAPTER = new PassthroughBytesAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final ReceiptPayload value) {
      encodeSizedField(encoder, PASSTHROUGH_ADAPTER, value.payloadBytes);
      encodeSizedField(encoder, PASSTHROUGH_ADAPTER, value.attestationBytes);
    }

    @Override
    public ReceiptPayload decode(final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      final byte[] payloadBytes =
          decodeSizedField(decoder, PASSTHROUGH_ADAPTER, "IdentifierReceipt.payload");
      final byte[] attestationBytes =
          decodeSizedField(decoder, PASSTHROUGH_ADAPTER, "IdentifierReceipt.attestation");
      return new ReceiptPayload(payloadBytes, attestationBytes);
    }
  }

  private static final class PassthroughBytesAdapter implements TypeAdapter<byte[]> {
    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      if (value == null || value.length == 0) {
        throw new IllegalArgumentException("payload bytes must not be empty");
      }
      encoder.writeBytes(value);
    }

    @Override
    public byte[] decode(final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      final byte[] payload = decoder.readBytes(decoder.remaining());
      if (payload.length == 0) {
        throw new IllegalArgumentException("payload bytes must not be empty");
      }
      return payload;
    }
  }

  private static <T> void encodeSizedField(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final NoritoEncoder child = encoder.childEncoder();
    adapter.encode(child, value);
    final byte[] payload = child.toByteArray();
    final boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    encoder.writeLength(payload.length, compact);
    encoder.writeBytes(payload);
  }

  private static <T> T decodeSizedField(
      final org.hyperledger.iroha.norito.NoritoDecoder decoder,
      final TypeAdapter<T> adapter,
      final String fieldName) {
    final long length = decoder.readLength((decoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(fieldName + " payload too large");
    }
    final byte[] payload = decoder.readBytes((int) length);
    final org.hyperledger.iroha.norito.NoritoDecoder child =
        new org.hyperledger.iroha.norito.NoritoDecoder(
            payload, decoder.flags(), decoder.flagsHint());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after " + fieldName + " payload");
    }
    return value;
  }

  private static String requireExactNonBlank(final String value, final String field) {
    final String exact = value == null ? "" : value;
    if (exact.trim().isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    if (!exact.trim().equals(exact)) {
      throw new IllegalArgumentException(field + " must not contain surrounding whitespace");
    }
    return exact;
  }

}
