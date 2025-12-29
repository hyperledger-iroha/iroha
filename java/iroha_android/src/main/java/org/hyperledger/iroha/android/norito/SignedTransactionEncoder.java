package org.hyperledger.iroha.android.norito;

import java.util.List;
import java.util.Optional;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.TypeAdapter;

public final class SignedTransactionEncoder {

  private static final String SIGNED_SCHEMA = "iroha.transaction.SignedTransaction.v1";
  private static final TransactionPayloadAdapter PAYLOAD_ADAPTER = new TransactionPayloadAdapter();
  private static final TypeAdapter<byte[]> SIGNATURE_ADAPTER = NoritoAdapters.bytesAdapter();
  private static final TypeAdapter<Optional<List<byte[]>>> ATTACHMENTS_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.sequence(NoritoAdapters.bytesAdapter()));
  private static final NoritoJavaCodecAdapter PAYLOAD_CODEC = new NoritoJavaCodecAdapter();

  private SignedTransactionEncoder() {}

  public static byte[] encode(final SignedTransaction transaction) throws NoritoException {
    final TransactionPayload payload = PAYLOAD_CODEC.decodeTransaction(transaction.encodedPayload());
    final SignedRecord record = new SignedRecord(transaction.signature(), payload);
    try {
      return NoritoCodec.encode(record, SIGNED_SCHEMA, SignedTransactionAdapter.INSTANCE);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to encode signed transaction", ex);
    }
  }

  private record SignedRecord(byte[] signature, TransactionPayload payload) {}

  private static final class SignedTransactionAdapter implements TypeAdapter<SignedRecord> {
    private static final SignedTransactionAdapter INSTANCE = new SignedTransactionAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final SignedRecord value) {
      SIGNATURE_ADAPTER.encode(encoder, value.signature());
      PAYLOAD_ADAPTER.encode(encoder, value.payload());
      ATTACHMENTS_ADAPTER.encode(encoder, Optional.empty());
    }

    @Override
    public SignedRecord decode(final org.hyperledger.iroha.norito.NoritoDecoder decoder) {
      throw new UnsupportedOperationException("Decoding signed transactions is not supported");
    }
  }
}
