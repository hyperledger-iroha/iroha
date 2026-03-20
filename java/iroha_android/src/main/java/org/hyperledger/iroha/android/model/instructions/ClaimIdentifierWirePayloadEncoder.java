package org.hyperledger.iroha.android.model.instructions;

import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.client.IdentifierResolutionReceipt;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/**
 * Encodes {@code ClaimIdentifier} instructions in wire-framed Norito format.
 *
 * <p>The Torii identifier endpoints already expose the canonical signed receipt payload as
 * {@code signature_payload_hex}, so the encoder can reuse those bytes directly instead of trying to
 * re-encode receipt internals on the wallet.
 */
public final class ClaimIdentifierWirePayloadEncoder {

  public static final String WIRE_NAME = "identity::ClaimIdentifier";

  private static final String SCHEMA_PATH = "iroha_data_model::isi::identifier::ClaimIdentifier";

  private static final TypeAdapter<byte[]> RAW_BYTE_VEC_ADAPTER = NoritoAdapters.rawByteVecAdapter();
  private static final TypeAdapter<Optional<byte[]>> OPTIONAL_SIGNATURE_ADAPTER =
      NoritoAdapters.option(RAW_BYTE_VEC_ADAPTER);
  private static final TypeAdapter<Optional<byte[]>> OPTIONAL_PROOF_ADAPTER =
      NoritoAdapters.option(RAW_BYTE_VEC_ADAPTER);

  private ClaimIdentifierWirePayloadEncoder() {}

  /**
   * Encodes a signed {@code ClaimIdentifier} instruction as a wire-framed {@link InstructionBox}.
   */
  public static InstructionBox encode(
      final String accountId, final IdentifierResolutionReceipt receipt) {
    Objects.requireNonNull(accountId, "accountId");
    Objects.requireNonNull(receipt, "receipt");
    final String normalizedAccountId = requireNonBlank(accountId, "accountId");
    final String receiptAccountId = requireNonBlank(receipt.accountId(), "receipt.accountId");
    if (!normalizedAccountId.equals(receiptAccountId)) {
      throw new IllegalArgumentException(
          "ClaimIdentifier accountId must match receipt.accountId");
    }
    final byte[] accountPayload =
        TransferWirePayloadEncoder.encodeAccountIdPayload(normalizedAccountId);
    final byte[] receiptPayload = decodeHex(receipt.signaturePayloadHex(), "receipt.signaturePayloadHex");
    final byte[] signaturePayload = decodeHex(receipt.signature(), "receipt.signature");
    final byte[] wirePayload =
        NoritoCodec.encode(
            new ClaimIdentifierPayload(accountPayload, receiptPayload, signaturePayload),
            SCHEMA_PATH,
            new ClaimIdentifierPayloadAdapter());
    return InstructionBox.fromWirePayload(WIRE_NAME, wirePayload);
  }

  private static final class ClaimIdentifierPayload {
    private final byte[] accountPayload;
    private final byte[] receiptPayload;
    private final byte[] signaturePayload;

    private ClaimIdentifierPayload(
        final byte[] accountPayload, final byte[] receiptPayload, final byte[] signaturePayload) {
      this.accountPayload = accountPayload.clone();
      this.receiptPayload = receiptPayload.clone();
      this.signaturePayload = signaturePayload.clone();
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
          new ReceiptPayload(value.receiptPayload, Optional.of(value.signaturePayload), Optional.empty()));
    }

    @Override
    public ClaimIdentifierPayload decode(
        final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding ClaimIdentifier is not supported");
    }
  }

  private static final class ReceiptPayload {
    private final byte[] payloadBytes;
    private final Optional<byte[]> signatureBytes;
    private final Optional<byte[]> proofBytes;

    private ReceiptPayload(
        final byte[] payloadBytes,
        final Optional<byte[]> signatureBytes,
        final Optional<byte[]> proofBytes) {
      this.payloadBytes = payloadBytes.clone();
      this.signatureBytes = Objects.requireNonNull(signatureBytes, "signatureBytes");
      this.proofBytes = Objects.requireNonNull(proofBytes, "proofBytes");
    }
  }

  private static final class ReceiptPayloadAdapter implements TypeAdapter<ReceiptPayload> {
    private static final TypeAdapter<byte[]> PASSTHROUGH_ADAPTER = new PassthroughBytesAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final ReceiptPayload value) {
      encodeSizedField(encoder, PASSTHROUGH_ADAPTER, value.payloadBytes);
      encodeSizedField(encoder, OPTIONAL_SIGNATURE_ADAPTER, value.signatureBytes);
      encodeSizedField(encoder, OPTIONAL_PROOF_ADAPTER, value.proofBytes);
    }

    @Override
    public ReceiptPayload decode(final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding identifier receipts is not supported");
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
      throw new UnsupportedOperationException("Decoding passthrough payloads is not supported");
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

  private static String requireNonBlank(final String value, final String field) {
    final String trimmed = value == null ? "" : value.trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    return trimmed;
  }

  private static byte[] decodeHex(final String value, final String field) {
    String trimmed = requireNonBlank(value, field);
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      trimmed = trimmed.substring(2);
    }
    if ((trimmed.length() & 1) == 1) {
      throw new IllegalArgumentException(field + " must contain an even number of hex characters");
    }
    final byte[] out = new byte[trimmed.length() / 2];
    for (int i = 0; i < trimmed.length(); i += 2) {
      final int high = Character.digit(trimmed.charAt(i), 16);
      final int low = Character.digit(trimmed.charAt(i + 1), 16);
      if (high < 0 || low < 0) {
        throw new IllegalArgumentException(field + " contains non-hex characters");
      }
      out[i / 2] = (byte) ((high << 4) | low);
    }
    return out;
  }
}
