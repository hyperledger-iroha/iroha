package org.hyperledger.iroha.android.connect.error;

import java.util.Objects;

/** Queue back-pressure / expiration events surfaced by Connect. */
public final class ConnectQueueError extends RuntimeException
    implements ConnectErrorConvertible {

  private static final long serialVersionUID = 1L;

  public enum Kind {
    OVERFLOW,
    EXPIRED
  }

  private final Kind kind;
  private final Integer limit;
  private final Long ttlMillis;

  private ConnectQueueError(final Kind kind, final Integer limit, final Long ttlMillis) {
    super(kind == Kind.EXPIRED ? "Connect queue entry expired" : "Connect queue overflow");
    this.kind = Objects.requireNonNull(kind, "kind");
    this.limit = limit;
    this.ttlMillis = ttlMillis;
  }

  public static ConnectQueueError overflow(final Integer limit) {
    return new ConnectQueueError(Kind.OVERFLOW, limit, null);
  }

  public static ConnectQueueError expired(final Long ttlMillis) {
    return new ConnectQueueError(Kind.EXPIRED, null, ttlMillis);
  }

  public Kind kind() {
    return kind;
  }

  public Integer limit() {
    return limit;
  }

  public Long ttlMillis() {
    return ttlMillis;
  }

  @Override
  public ConnectError toConnectError() {
    final ConnectError.Builder builder =
        ConnectError.builder().message(getMessage()).cause(this);
    switch (kind) {
      case EXPIRED -> {
        builder.category(ConnectErrorCategory.TIMEOUT).code("queue.expired");
        if (ttlMillis != null) {
          builder.underlying("ttlMs=" + ttlMillis);
        }
      }
      case OVERFLOW -> {
        builder.category(ConnectErrorCategory.QUEUE_OVERFLOW).code("queue.overflow");
        if (limit != null) {
          builder.underlying("limit=" + limit);
        }
      }
    }
    return builder.build();
  }
}
