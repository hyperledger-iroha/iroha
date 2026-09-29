package org.hyperledger.iroha.sdk.core.model;

import java.util.Collections;

/** Calls the public Kotlin constructor exactly as a Java application does. */
public final class TransactionPayloadJavaCalls {
  private TransactionPayloadJavaCalls() {}

  public static TransactionPayload create(NetworkId networkId, String authority) {
    return new TransactionPayload(
        networkId,
        authority,
        1000L,
        Executable.ivm(new byte[0]),
        100000L,
        null,
        FeePaymentIntent.authority(Collections.emptyList(), null),
        Collections.emptyMap(),
        null);
  }
}
