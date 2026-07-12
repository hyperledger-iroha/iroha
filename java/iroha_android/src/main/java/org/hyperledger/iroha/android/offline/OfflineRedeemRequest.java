package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Canonical redemption request submitted directly as a Norito archive. */
public final class OfflineRedeemRequest {
  private final String operationId;
  private final byte[] noritoArchive;

  public OfflineRedeemRequest(final byte[] noritoArchive) {
    final OfflineOperationCodec.CanonicalRequest request =
        OfflineOperationCodec.requireRedeemRequest(noritoArchive);
    this.operationId = request.operationId();
    this.noritoArchive = request.archive();
  }

  /** Lowercase hexadecimal operation identifier embedded in the canonical request. */
  public String operationId() {
    return operationId;
  }

  public byte[] noritoArchive() {
    return Arrays.copyOf(noritoArchive, noritoArchive.length);
  }
}
