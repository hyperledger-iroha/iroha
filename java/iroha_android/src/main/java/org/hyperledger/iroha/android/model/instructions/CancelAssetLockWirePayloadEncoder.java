package org.hyperledger.iroha.android.model.instructions;

import java.math.BigInteger;
import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.numeric.NumericV1;
import org.hyperledger.iroha.android.util.HashLiteral;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Canonical native Norito encoder/decoder for {@link CancelAssetLockInstruction}. */
public final class CancelAssetLockWirePayloadEncoder {

  /** Registered native instruction type and Norito schema path. */
  public static final String WIRE_NAME = CancelAssetLockInstruction.WIRE_NAME;

  private static final TypeAdapter<Long> UINT32_ADAPTER = NoritoAdapters.uint(32);
  private static final TypeAdapter<CancelAssetLockInstruction> PAYLOAD_ADAPTER =
      new PayloadAdapter();
  private static final TypeAdapter<NumericV1.QuantityValue> QUANTITY_ADAPTER =
      new QuantityAdapter();

  private CancelAssetLockWirePayloadEncoder() {}

  /** Encode a typed cancellation as a wire-framed {@link InstructionBox}. */
  public static InstructionBox encode(final CancelAssetLockInstruction instruction) {
    return InstructionBox.fromWirePayload(WIRE_NAME, encodePayload(instruction));
  }

  /** Encode only the canonical native {@code CancelAssetLock} Norito frame. */
  public static byte[] encodePayload(final CancelAssetLockInstruction instruction) {
    return NoritoCodec.encode(
        Objects.requireNonNull(instruction, "instruction"),
        WIRE_NAME,
        PAYLOAD_ADAPTER);
  }

  /** Decode a canonical native {@code CancelAssetLock} Norito frame. */
  public static CancelAssetLockInstruction decodePayload(final byte[] payload) {
    return NoritoCodec.decode(
        Objects.requireNonNull(payload, "payload"), PAYLOAD_ADAPTER, WIRE_NAME);
  }

  private static final class PayloadAdapter
      implements TypeAdapter<CancelAssetLockInstruction> {

    @Override
    public void encode(
        final NoritoEncoder encoder, final CancelAssetLockInstruction value) {
      encodeSizedRawField(encoder, canonicalEscrowBytes(value.escrowId()));
      encodeSizedField(encoder, QUANTITY_ADAPTER, value.expectedRemainingAmount());
    }

    @Override
    public CancelAssetLockInstruction decode(final NoritoDecoder decoder) {
      final byte[] escrowBytes =
          decodeSizedRawField(decoder, "CancelAssetLock.escrow_id");
      if (escrowBytes.length != 32) {
        throw new IllegalArgumentException(
            "CancelAssetLock.escrow_id must contain exactly 32 bytes");
      }
      if ((escrowBytes[escrowBytes.length - 1] & 1) == 0) {
        throw new IllegalArgumentException(
            "CancelAssetLock.escrow_id must use a native hash with its marker bit set");
      }
      final NumericV1.QuantityValue expected =
          decodeSizedField(
              decoder,
              QUANTITY_ADAPTER,
              "CancelAssetLock.expected_remaining_amount");
      return CancelAssetLockInstruction.fromEscrowId(
          HashLiteral.canonicalize(escrowBytes), expected);
    }
  }

  private static final class QuantityAdapter
      implements TypeAdapter<NumericV1.QuantityValue> {

    @Override
    public void encode(
        final NoritoEncoder encoder, final NumericV1.QuantityValue value) {
      requirePositiveQuantity(value);
      final byte[] mantissaBytes = toTwosComplementLittleEndian(value.mantissa());
      final NoritoEncoder mantissa = encoder.childEncoder();
      mantissa.writeUInt(mantissaBytes.length, 32);
      mantissa.writeBytes(mantissaBytes);
      encodeSizedRawField(encoder, mantissa.toByteArray());

      final NoritoEncoder scale = encoder.childEncoder();
      UINT32_ADAPTER.encode(scale, (long) value.scale());
      encodeSizedRawField(encoder, scale.toByteArray());
    }

    @Override
    public NumericV1.QuantityValue decode(final NoritoDecoder decoder) {
      final byte[] mantissaPayload =
          decodeSizedRawField(decoder, "Quantity.mantissa");
      final NoritoDecoder mantissaDecoder =
          new NoritoDecoder(
              mantissaPayload, decoder.flags());
      final int byteLength =
          checkedLength(
              mantissaDecoder.readUInt(32), "Quantity.mantissa byte length");
      final byte[] encodedMantissa = mantissaDecoder.readBytes(byteLength);
      if (mantissaDecoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after Quantity.mantissa");
      }
      final BigInteger mantissa =
          decodeTwosComplementLittleEndian(encodedMantissa);
      if (!Arrays.equals(
          toTwosComplementLittleEndian(mantissa), encodedMantissa)) {
        throw new IllegalArgumentException("Quantity.mantissa is not canonical");
      }

      final byte[] scalePayload = decodeSizedRawField(decoder, "Quantity.scale");
      if (scalePayload.length != 4) {
        throw new IllegalArgumentException(
            "Quantity.scale must contain exactly four bytes");
      }
      final NoritoDecoder scaleDecoder =
          new NoritoDecoder(scalePayload, decoder.flags());
      final int scale = Math.toIntExact(UINT32_ADAPTER.decode(scaleDecoder));
      if (scaleDecoder.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after Quantity.scale");
      }

      final NumericV1.QuantityValue quantity =
          NumericV1.QuantityValue.of(mantissa, scale);
      if (!quantity.mantissa().equals(mantissa) || quantity.scale() != scale) {
        throw new IllegalArgumentException("Quantity is not canonically encoded");
      }
      return requirePositiveQuantity(quantity);
    }
  }

  private static byte[] canonicalEscrowBytes(final String escrowId) {
    return HashLiteral.decode(escrowId);
  }

  private static <T> void encodeSizedField(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final NoritoEncoder child = encoder.childEncoder();
    adapter.encode(child, value);
    encodeSizedRawField(encoder, child.toByteArray());
  }

  private static void encodeSizedRawField(
      final NoritoEncoder encoder, final byte[] payload) {
    encoder.writeLength(
        payload.length, (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
    encoder.writeBytes(payload);
  }

  private static <T> T decodeSizedField(
      final NoritoDecoder decoder,
      final TypeAdapter<T> adapter,
      final String fieldName) {
    final byte[] payload = decodeSizedRawField(decoder, fieldName);
    final NoritoDecoder child =
        new NoritoDecoder(payload, decoder.flags());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after " + fieldName);
    }
    return value;
  }

  private static byte[] decodeSizedRawField(
      final NoritoDecoder decoder, final String fieldName) {
    final int length =
        checkedLength(
            decoder.readLength(
                (decoder.flags() & NoritoHeader.COMPACT_LEN) != 0),
            fieldName + " length");
    return decoder.readBytes(length);
  }

  private static int checkedLength(final long value, final String fieldName) {
    if (value < 0 || value > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(
          fieldName + " is outside the supported range");
    }
    return (int) value;
  }

  private static NumericV1.QuantityValue requirePositiveQuantity(
      final NumericV1.QuantityValue value) {
    final NumericV1.QuantityValue nonNull =
        Objects.requireNonNull(value, "expectedRemainingAmount");
    if (nonNull.mantissa().signum() <= 0) {
      throw new IllegalArgumentException(
          "expected_remaining_amount must be greater than zero");
    }
    return nonNull;
  }

  private static byte[] toTwosComplementLittleEndian(final BigInteger value) {
    if (value.signum() == 0) {
      return new byte[0];
    }
    final byte[] bigEndian = value.toByteArray();
    final byte[] littleEndian = new byte[bigEndian.length];
    for (int index = 0; index < bigEndian.length; index++) {
      littleEndian[index] = bigEndian[bigEndian.length - 1 - index];
    }
    return littleEndian;
  }

  private static BigInteger decodeTwosComplementLittleEndian(
      final byte[] value) {
    if (value.length == 0) {
      return BigInteger.ZERO;
    }
    final byte[] bigEndian = new byte[value.length];
    for (int index = 0; index < value.length; index++) {
      bigEndian[index] = value[value.length - 1 - index];
    }
    return new BigInteger(bigEndian);
  }
}
