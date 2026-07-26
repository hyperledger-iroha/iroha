package org.hyperledger.iroha.android.crypto;

import java.util.Arrays;
import java.util.Objects;

/** Canonical versioned transaction bytes and native transaction hash returned by zk signers. */
public final class NativeSignedTransaction {
  private final byte[] versionedSignedTransaction;
  private final byte[] transactionHash;

  public NativeSignedTransaction(
      final byte[] versionedSignedTransaction, final byte[] transactionHash) {
    this.versionedSignedTransaction =
        Arrays.copyOf(
            Objects.requireNonNull(versionedSignedTransaction, "versionedSignedTransaction"),
            versionedSignedTransaction.length);
    this.transactionHash =
        Arrays.copyOf(Objects.requireNonNull(transactionHash, "transactionHash"), transactionHash.length);
    if (this.versionedSignedTransaction.length == 0) {
      throw new IllegalArgumentException("versionedSignedTransaction must not be empty");
    }
    if (this.transactionHash.length != 32) {
      throw new IllegalArgumentException("transactionHash must be exactly 32 bytes");
    }
  }

  public byte[] versionedSignedTransaction() {
    return Arrays.copyOf(versionedSignedTransaction, versionedSignedTransaction.length);
  }

  public byte[] transactionHash() {
    return Arrays.copyOf(transactionHash, transactionHash.length);
  }
}
