package org.hyperledger.iroha.android.norito;

import java.util.List;
import java.util.Optional;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

public final class SignedTransactionEncoder {

  private static final byte VERSION_BYTE = 0x01;
  private static final TransactionPayloadAdapter PAYLOAD_ADAPTER = new TransactionPayloadAdapter();
  private static final TypeAdapter<byte[]> BYTE_VECTOR_ADAPTER = NoritoAdapters.byteVecAdapter();
  private static final TypeAdapter<byte[]> SIGNATURE_ADAPTER = new TransactionSignatureAdapter();
  private static final TypeAdapter<Optional<byte[]>> EMPTY_OPTION_ADAPTER =
      NoritoAdapters.option(BYTE_VECTOR_ADAPTER);
  private static final NoritoJavaCodecAdapter PAYLOAD_CODEC = new NoritoJavaCodecAdapter();

  private SignedTransactionEncoder() {}

  public static byte[] encode(final SignedTransaction transaction) throws NoritoException {
    final TransactionPayload payload = PAYLOAD_CODEC.decodeTransaction(transaction.encodedPayload());
    final SignedRecord record = new SignedRecord(transaction.signature(), payload);
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

  private static final class SignedRecord {
    private final byte[] signature;
    private final TransactionPayload payload;

    private SignedRecord(final byte[] signature, final TransactionPayload payload) {
      this.signature = signature;
      this.payload = payload;
    }

    private byte[] signature() {
      return signature;
    }

    private TransactionPayload payload() {
      return payload;
    }
  }

  private static final class SignedTransactionAdapter implements TypeAdapter<SignedRecord> {
    private static final SignedTransactionAdapter INSTANCE = new SignedTransactionAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final SignedRecord value) {
      encodeSizedField(encoder, SIGNATURE_ADAPTER, value.signature());
      encodeSizedField(encoder, PAYLOAD_ADAPTER, value.payload());
      encodeSizedField(encoder, EMPTY_OPTION_ADAPTER, Optional.empty());
      // TODO: Encode multisig signatures once Android SDK exposes them.
      encodeSizedField(encoder, EMPTY_OPTION_ADAPTER, Optional.empty());
    }

    @Override
    public SignedRecord decode(final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding signed transactions is not supported");
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
      throw new UnsupportedOperationException("Decoding signatures is not supported");
    }
  }
}
