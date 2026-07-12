package org.hyperledger.iroha.android.client.queue;

import java.util.Optional;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Canonical private record codec used only by online transaction retry queues. */
final class PendingTransactionRecordCodec {

  private static final String SCHEMA = "iroha.android.client.PendingTransactionRecord.v1";
  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<byte[]> BYTES_ADAPTER = NoritoAdapters.bytesAdapter();
  private static final TypeAdapter<Optional<String>> OPTIONAL_STRING_ADAPTER =
      NoritoAdapters.option(STRING_ADAPTER);
  private static final TypeAdapter<Optional<byte[]>> OPTIONAL_BYTES_ADAPTER =
      NoritoAdapters.option(BYTES_ADAPTER);
  private static final TypeAdapter<SignedTransaction> ADAPTER = new RecordAdapter();

  private PendingTransactionRecordCodec() {}

  static byte[] encode(final SignedTransaction transaction) throws NoritoException {
    if (transaction.multisigSignatures().isPresent()) {
      throw new NoritoException(
          "Pending transaction queue does not accept multisig transactions");
    }
    try {
      return NoritoCodec.encode(transaction, SCHEMA, ADAPTER);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to encode pending transaction record", ex);
    }
  }

  static SignedTransaction decode(final byte[] encoded) throws NoritoException {
    try {
      return NoritoCodec.decode(encoded, ADAPTER, SCHEMA);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to decode pending transaction record", ex);
    }
  }

  private static final class RecordAdapter implements TypeAdapter<SignedTransaction> {
    @Override
    public void encode(final NoritoEncoder encoder, final SignedTransaction value) {
      STRING_ADAPTER.encode(encoder, value.schemaName());
      OPTIONAL_STRING_ADAPTER.encode(encoder, value.keyAlias());
      BYTES_ADAPTER.encode(encoder, value.encodedPayload());
      BYTES_ADAPTER.encode(encoder, value.signature());
      BYTES_ADAPTER.encode(encoder, value.publicKey());
      OPTIONAL_BYTES_ADAPTER.encode(encoder, value.exportedKeyBundle());
      OPTIONAL_BYTES_ADAPTER.encode(encoder, value.blsPublicKey());
    }

    @Override
    public SignedTransaction decode(final NoritoDecoder decoder) {
      return SignedTransaction.builder()
          .setSchemaName(STRING_ADAPTER.decode(decoder))
          .setKeyAlias(OPTIONAL_STRING_ADAPTER.decode(decoder).orElse(null))
          .setEncodedPayload(BYTES_ADAPTER.decode(decoder))
          .setSignature(BYTES_ADAPTER.decode(decoder))
          .setPublicKey(BYTES_ADAPTER.decode(decoder))
          .setExportedKeyBundle(OPTIONAL_BYTES_ADAPTER.decode(decoder).orElse(null))
          .setBlsPublicKey(OPTIONAL_BYTES_ADAPTER.decode(decoder).orElse(null))
          .build();
    }
  }
}
