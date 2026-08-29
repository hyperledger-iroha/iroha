package org.hyperledger.iroha.android.client;

import java.util.Arrays;

/** Bounded exact JSON response whose proof/capsule inner material remains opaque. */
public final class AtomicPrivateSettlementJsonResponseV1 implements AutoCloseable {
  private final String route;
  private final byte[] body;
  private boolean closed;

  AtomicPrivateSettlementJsonResponseV1(final String route, final byte[] body) {
    this.route = route;
    this.body = body.clone();
  }

  /** Exact route used to obtain this response. */
  public String route() {
    return route;
  }

  /** Defensive copy suitable for the native wallet or auditor worker. */
  public synchronized byte[] bytes() {
    if (closed) {
      throw new IllegalStateException("atomic private settlement response is closed");
    }
    return body.clone();
  }

  /** Erase the SDK-owned response copy. */
  @Override
  public synchronized void close() {
    if (!closed) {
      Arrays.fill(body, (byte) 0);
      closed = true;
    }
  }

  @Override
  public String toString() {
    return "AtomicPrivateSettlementJsonResponseV1(route=" + route + ", body=[REDACTED])";
  }
}
