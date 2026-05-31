package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.tx.SignedTransaction;

/** Receipt returned after a signed transfer is finalized and submitted. */
public final class NexusTransferReceipt {

  private final String transactionHashHex;
  private final SignedTransaction signedTransaction;
  private final ClientResponse submission;
  private final Map<String, Object> finalStatus;

  public NexusTransferReceipt(
      final String transactionHashHex,
      final SignedTransaction signedTransaction,
      final ClientResponse submission,
      final Map<String, Object> finalStatus) {
    this.transactionHashHex = NexusModelUtils.requireNonBlank(transactionHashHex, "transactionHashHex");
    this.signedTransaction = Objects.requireNonNull(signedTransaction, "signedTransaction");
    this.submission = Objects.requireNonNull(submission, "submission");
    if (finalStatus == null || finalStatus.isEmpty()) {
      this.finalStatus = null;
    } else {
      this.finalStatus = Collections.unmodifiableMap(new LinkedHashMap<>(finalStatus));
    }
  }

  public String transactionHashHex() {
    return transactionHashHex;
  }

  public SignedTransaction signedTransaction() {
    return signedTransaction;
  }

  public ClientResponse submission() {
    return submission;
  }

  public Map<String, Object> finalStatus() {
    return finalStatus;
  }
}
