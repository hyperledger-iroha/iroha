package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;

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
    this.signedTransaction = Objects.requireNonNull(signedTransaction, "signedTransaction");
    if (transactionHashHex == null
        || !transactionHashHex.matches("[0-9a-f]{63}[13579bdf]")) {
      throw new IllegalArgumentException(
          "transactionHashHex must match [0-9a-f]{63}[13579bdf] with the Iroha HashOf marker");
    }
    if (!transactionHashHex.equals(SignedTransactionHasher.hashHex(this.signedTransaction))) {
      throw new IllegalArgumentException(
          "transactionHashHex must identify the exact signed transaction");
    }
    this.transactionHashHex = transactionHashHex;
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
