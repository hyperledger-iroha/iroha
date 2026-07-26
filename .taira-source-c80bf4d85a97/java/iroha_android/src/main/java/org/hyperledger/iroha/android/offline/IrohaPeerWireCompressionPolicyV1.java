package org.hyperledger.iroha.android.offline;

/** Compression is opt-in and QR mode is accepted only when it saves bytes and a shard. */
public enum IrohaPeerWireCompressionPolicyV1 {
  DISABLED,
  /** Deterministic compression policy shared by every peer V1 rail. */
  PEER_OPTIMIZED
}
