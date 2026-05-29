package org.hyperledger.iroha.android.norito;

import java.util.List;
import java.util.Optional;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.tx.MultisigSignature;
import org.hyperledger.iroha.android.tx.MultisigSignatures;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

public final class SignedTransactionEncoder {

  private static final byte VERSION_BYTE = 0x01;
  private static final String SIGNED_SCHEMA = "iroha.transaction.SignedTransaction.v1";
  private static final TransactionPayloadAdapter PAYLOAD_ADAPTER = new TransactionPayloadAdapter();
  private static final TypeAdapter<byte[]> BYTE_VECTOR_ADAPTER = NoritoAdapters.byteVecAdapter();
  private static final TypeAdapter<byte[]> SIGNATURE_ADAPTER = new TransactionSignatureAdapter();
  private static final TypeAdapter<Optional<byte[]>> EMPTY_OPTION_ADAPTER =
      NoritoAdapters.option(BYTE_VECTOR_ADAPTER);
  private static final TypeAdapter<MultisigSignature> MULTISIG_SIGNATURE_ADAPTER =
      new MultisigSignatureAdapter();
  private static final TypeAdapter<List<MultisigSignature>> MULTISIG_SIGNATURE_LIST_ADAPTER =
      NoritoAdapters.sequence(MULTISIG_SIGNATURE_ADAPTER);
  private static final TypeAdapter<MultisigSignatures> MULTISIG_SIGNATURES_ADAPTER =
      new MultisigSignaturesAdapter();
  private static final TypeAdapter<Optional<MultisigSignatures>> MULTISIG_SIGNATURES_OPTION_ADAPTER =
      NoritoAdapters.option(MULTISIG_SIGNATURES_ADAPTER);
  private static final NoritoJavaCodecAdapter PAYLOAD_CODEC = new NoritoJavaCodecAdapter();

  private SignedTransactionEncoder() {}

  public static byte[] encode(final SignedTransaction transaction) throws NoritoException {
    final TransactionPayload payload = PAYLOAD_CODEC.decodeTransaction(transaction.encodedPayload());
    final SignedRecord record =
        new SignedRecord(transaction.signature(), payload, transaction.multisigSignatures());
    try {
      return NoritoCodec.encodeAdaptive(record, SignedTransactionAdapter.INSTANCE).payload();
    } catch (final Exception ex) {
      throw new NoritoException("Failed to encode signed transaction", ex);
    }
  }

  public static byte[] encodeVersioned(final SignedTransaction transaction) throws NoritoException {
    final byte[] bare = encode(transaction);
    final byte[] out = new byte[1 + bare.length];
    out[0] = VERSION_BYTE;
    System.arraycopy(bare, 0, out, 1, bare.length);
    return out;
  }

  public static SignedTransaction decode(final byte[] encoded) throws NoritoException {
    try {
      final SignedRecord record = NoritoCodec.decodeAdaptive(encoded, SignedTransactionAdapter.INSTANCE);
      final byte[] payloadBytes = PAYLOAD_CODEC.encodeTransaction(record.payload());
      return SignedTransaction.builder()
          .setEncodedPayload(payloadBytes)
          .setSignature(record.signature())
          .setPublicKey(new byte[0])
          .setSchemaName(SIGNED_SCHEMA)
          .setMultisigSignatures(record.multisigSignatures().orElse(null))
          .build();
    } catch (final Exception ex) {
      throw new NoritoException("Failed to decode signed transaction", ex);
    }
  }

  public static SignedTransaction decodeVersioned(final byte[] encoded) throws NoritoException {
    try {
      if (encoded.length == 0) {
        throw new IllegalArgumentException("Versioned signed transaction must not be empty");
      }
      if (encoded[0] != VERSION_BYTE) {
        throw new IllegalArgumentException(
            "Unsupported signed transaction version byte: " + (encoded[0] & 0xFF));
      }
      final byte[] bare = new byte[encoded.length - 1];
      System.arraycopy(encoded, 1, bare, 0, bare.length);
      return decode(bare);
    } catch (final NoritoException ex) {
      throw ex;
    } catch (final Exception ex) {
      throw new NoritoException("Failed to decode versioned signed transaction", ex);
    }
  }

  private static final class SignedRecord {
    private final byte[] signature;
    private final TransactionPayload payload;
    private final Optional<MultisigSignatures> multisigSignatures;

    private SignedRecord(
        final byte[] signature,
        final TransactionPayload payload,
        final Optional<MultisigSignatures> multisigSignatures) {
      this.signature = signature;
      this.payload = payload;
      this.multisigSignatures = multisigSignatures == null ? Optional.empty() : multisigSignatures;
    }

    private byte[] signature() {
      return signature;
    }

    private TransactionPayload payload() {
      return payload;
    }

    private Optional<MultisigSignatures> multisigSignatures() {
      return multisigSignatures;
    }
  }

  private static final class SignedTransactionAdapter implements TypeAdapter<SignedRecord> {
    private static final SignedTransactionAdapter INSTANCE = new SignedTransactionAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final SignedRecord value) {
      encodeSizedField(encoder, SIGNATURE_ADAPTER, value.signature());
      encodeSizedField(encoder, PAYLOAD_ADAPTER, value.payload());
      encodeSizedField(encoder, EMPTY_OPTION_ADAPTER, Optional.empty());
      encodeSizedField(encoder, MULTISIG_SIGNATURES_OPTION_ADAPTER, value.multisigSignatures());
    }

    @Override
    public SignedRecord decode(final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      final byte[] signature = decodeSizedField(decoder, SIGNATURE_ADAPTER, "signature");
      final TransactionPayload payload = decodeSizedField(decoder, PAYLOAD_ADAPTER, "payload");
      final Optional<byte[]> attachments =
          decodeSizedField(decoder, EMPTY_OPTION_ADAPTER, "attachments");
      if (attachments.isPresent()) {
        throw new IllegalArgumentException("Signed transaction attachments are not supported");
      }
      final Optional<MultisigSignatures> multisigSignatures =
          decodeSizedField(
              decoder, MULTISIG_SIGNATURES_OPTION_ADAPTER, "multisig_signatures");
      return new SignedRecord(signature, payload, multisigSignatures);
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

  private static final class TransactionSignatureAdapter implements TypeAdapter<byte[]> {
    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      encodeSizedField(encoder, BYTE_VECTOR_ADAPTER, value);
    }

    @Override
    public byte[] decode(final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      return decodeSizedField(decoder, BYTE_VECTOR_ADAPTER, "signature.bytes");
    }
  }

  private static final class MultisigSignatureAdapter implements TypeAdapter<MultisigSignature> {
    @Override
    public void encode(final NoritoEncoder encoder, final MultisigSignature value) {
      BYTE_VECTOR_ADAPTER.encode(encoder, value.publicKeyNoritoPayload());
      BYTE_VECTOR_ADAPTER.encode(encoder, value.signature());
    }

    @Override
    public MultisigSignature decode(final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      final byte[] publicKeyPayload = BYTE_VECTOR_ADAPTER.decode(decoder);
      final byte[] signature = BYTE_VECTOR_ADAPTER.decode(decoder);
      final PublicKeyCodec.PublicKeyPayload publicKey =
          PublicKeyCodec.decodeCompactPublicKeyPayload(publicKeyPayload);
      if (publicKey == null) {
        throw new IllegalArgumentException("Invalid multisig public key payload");
      }
      return MultisigSignature.fromCurveId(
          publicKey.curveId(), publicKey.keyBytes(), signature);
    }
  }

  private static final class MultisigSignaturesAdapter implements TypeAdapter<MultisigSignatures> {
    @Override
    public void encode(final NoritoEncoder encoder, final MultisigSignatures value) {
      MULTISIG_SIGNATURE_LIST_ADAPTER.encode(encoder, value.signatures());
    }

    @Override
    public MultisigSignatures decode(final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      return MultisigSignatures.of(MULTISIG_SIGNATURE_LIST_ADAPTER.decode(decoder));
    }
  }

  private static <T> T decodeSizedField(
      final NoritoDecoder decoder, final TypeAdapter<T> adapter, final String fieldName) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(fieldName + " payload too large");
    }
    final byte[] payload = decoder.readBytes((int) length);
    final NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after " + fieldName + " payload");
    }
    return value;
  }
}
