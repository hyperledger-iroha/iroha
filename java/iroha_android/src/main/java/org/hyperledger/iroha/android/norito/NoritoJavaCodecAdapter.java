package org.hyperledger.iroha.android.norito;

import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.TypeAdapter;

/**
 * Norito codec adapter that delegates to the shared JVM Norito implementation bundled with the
 * workspace. This ensures Android tooling stays in lockstep with the canonical Rust codecs and
 * schema hashes.
 */
public final class NoritoJavaCodecAdapter implements NoritoCodecAdapter {

  private static final String DEFAULT_SCHEMA = "iroha.android.transaction.Payload.v1";

  private final String schemaName;
  private final TypeAdapter<TransactionPayload> adapter;

  public NoritoJavaCodecAdapter() {
    this(DEFAULT_SCHEMA);
  }

  public NoritoJavaCodecAdapter(final String schemaName) {
    this.schemaName = schemaName;
    this.adapter = new TransactionPayloadAdapter();
  }

  @Override
  public byte[] encodeTransaction(final TransactionPayload payload) throws NoritoException {
    try {
      return NoritoCodec.encode(payload, schemaName, adapter);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to encode Norito transaction payload", ex);
    }
  }

  @Override
  public TransactionPayload decodeTransaction(final byte[] encoded) throws NoritoException {
    try {
      return NoritoCodec.decode(encoded, adapter, schemaName);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to decode Norito transaction payload", ex);
    }
  }

  @Override
  public String schemaName() {
    return schemaName;
  }
}
