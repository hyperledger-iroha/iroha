package org.hyperledger.iroha.android.norito;

import org.hyperledger.iroha.android.model.TransactionPayload;

/**
 * Abstraction over the Norito codec so the Android library can delegate encoding/decoding to either
 * the in-repo Java implementation or a platform supplied binding.
 */
public interface NoritoCodecAdapter {

  /**
     * Encodes the provided transaction payload into canonical Norito bytes.
     *
     * @throws NoritoException when encoding fails
     */
  byte[] encodeTransaction(TransactionPayload payload) throws NoritoException;

  /**
     * Decodes the provided Norito payload into a structured transaction representation.
     *
     * @throws NoritoException when decoding fails
     */
  TransactionPayload decodeTransaction(byte[] encoded) throws NoritoException;

  /** Returns the schema name used for encoding/decoding (useful for diagnostics). */
  default String schemaName() {
    return "iroha.android.transaction";
  }
}
