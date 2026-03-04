package org.hyperledger.iroha.android.client.stream;

import java.util.Objects;

/** Representation of a single server-sent event frame. */
public final class ServerSentEvent {

  private final String event;
  private final String data;
  private final String id;

  ServerSentEvent(final String event, final String data, final String id) {
    this.event = Objects.requireNonNull(event, "event");
    this.data = Objects.requireNonNull(data, "data");
    this.id = id;
  }

  /** Name of the event (defaults to {@code message} when unspecified). */
  public String event() {
    return event;
  }

  /**
   * Raw data payload. When a frame contains multiple {@code data:} lines, they are joined with
   * {@code '\n'} as mandated by the SSE specification.
   */
  public String data() {
    return data;
  }

  /** Event identifier supplied via the {@code id:} field (may be {@code null}). */
  public String id() {
    return id;
  }
}
