package org.hyperledger.iroha.android.client.websocket;

import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Convenience helper that keeps a Torii WebSocket session alive by reconnecting when the underlying
 * connection drops. Reconnection delays follow an exponential backoff policy.
 */
public final class ToriiWebSocketSubscription implements AutoCloseable {

  /** Factory used to open WebSocket sessions (allows dependency injection in tests). */
  @FunctionalInterface
  public interface SessionOpener {
    ToriiWebSocketSession open(ToriiWebSocketListener listener);
  }

  private final SessionOpener opener;
  private final ToriiWebSocketListener delegate;
  private final ScheduledExecutorService executor;
  private final boolean ownsExecutor;
  private final long initialBackoffMs;
  private final long maxBackoffMs;
  private final AtomicBoolean started = new AtomicBoolean(false);
  private final AtomicBoolean closed = new AtomicBoolean(false);
  private final AtomicLong nextBackoffMs;
  private final AtomicLong sessionGeneration = new AtomicLong(0);
  private final AtomicReference<ToriiWebSocketSession> currentSession = new AtomicReference<>(null);
  private final AtomicReference<ScheduledFuture<?>> scheduledTask = new AtomicReference<>(null);
  private final List<ToriiWebSocketObserver> observers;

  private ToriiWebSocketSubscription(final Builder builder) {
    this.opener = builder.opener;
    this.delegate = builder.listener;
    this.executor = builder.executor;
    this.ownsExecutor = builder.ownsExecutor;
    this.initialBackoffMs = builder.initialBackoffMs;
    this.maxBackoffMs = builder.maxBackoffMs;
    this.nextBackoffMs = new AtomicLong(initialBackoffMs);
    this.observers = List.copyOf(builder.observers);
  }

  /** Returns a builder that opens sessions via the provided client/path/options triple. */
  public static Builder builder(
      final ToriiWebSocketClient client,
      final String path,
      final ToriiWebSocketOptions options,
      final ToriiWebSocketListener listener) {
    Objects.requireNonNull(client, "client");
    final ToriiWebSocketOptions resolved =
        options != null ? options : ToriiWebSocketOptions.defaultOptions();
    return new Builder(listener, delegate -> client.connect(path, resolved, delegate));
  }

  /** Returns a builder that uses a custom {@link SessionOpener}. */
  public static Builder builder(final SessionOpener opener, final ToriiWebSocketListener listener) {
    return new Builder(listener, opener);
  }

  /** Starts the subscription, initiating the first session immediately. */
  public synchronized ToriiWebSocketSubscription start() {
    if (!started.compareAndSet(false, true)) {
      throw new IllegalStateException("subscription already started");
    }
    scheduleReconnect(0, ToriiWebSocketObserver.ReconnectReason.INITIAL);
    return this;
  }

  /** Returns {@code true} when the subscription is actively running. */
  public boolean isRunning() {
    return started.get() && !closed.get();
  }

  /** Stops the subscription, closes the underlying session, and shuts down resources. */
  @Override
  public void close() {
    if (!closed.compareAndSet(false, true)) {
      return;
    }
    final ScheduledFuture<?> future = scheduledTask.getAndSet(null);
    if (future != null) {
      future.cancel(true);
    }
    final ToriiWebSocketSession session = currentSession.getAndSet(null);
    if (session != null && session.isOpen()) {
      session.close(1000, "client_shutdown");
    }
    if (ownsExecutor) {
      executor.shutdownNow();
    }
  }

  private void scheduleReconnect(
      final long delayMs, final ToriiWebSocketObserver.ReconnectReason reason) {
    if (closed.get()) {
      return;
    }
    final ScheduledFuture<?> existing = scheduledTask.get();
    if (existing != null && !existing.isDone() && !existing.isCancelled()) {
      return;
    }
    final long clampedDelay = Math.max(0L, delayMs);
    notifyReconnectScheduled(clampedDelay, reason);
    final ScheduledFuture<?> future =
        executor.schedule(this::openSession, clampedDelay, TimeUnit.MILLISECONDS);
    scheduledTask.set(future);
  }

  private void notifyReconnectScheduled(
      final long delayMs, final ToriiWebSocketObserver.ReconnectReason reason) {
    if (observers.isEmpty()) {
      return;
    }
    final Duration duration = Duration.ofMillis(Math.max(0L, delayMs));
    for (final ToriiWebSocketObserver observer : observers) {
      observer.onReconnectScheduled(duration, reason);
    }
  }

  private void notifySessionOpened() {
    for (final ToriiWebSocketObserver observer : observers) {
      observer.onSessionOpened();
    }
  }

  private void notifySessionClosed() {
    for (final ToriiWebSocketObserver observer : observers) {
      observer.onSessionClosed();
    }
  }

  private void notifySessionFailure(final Throwable error) {
    for (final ToriiWebSocketObserver observer : observers) {
      observer.onSessionFailure(error);
    }
  }

  private void openSession() {
    if (closed.get()) {
      return;
    }
    final long generation = sessionGeneration.incrementAndGet();
    final ManagedListener listener = new ManagedListener(generation);
    try {
      final ToriiWebSocketSession session = opener.open(listener);
      currentSession.set(session);
    } catch (final RuntimeException ex) {
      handleFailure(generation, null, ex);
    }
  }

  private void handleFailure(
      final long generation, final ToriiWebSocketSession session, final Throwable error) {
    if (closed.get()) {
      return;
    }
    if (generation != sessionGeneration.get()) {
      return;
    }
    final ToriiWebSocketSession targetSession =
        session != null ? session : currentSession.get();
    delegate.onError(targetSession != null ? targetSession : NullSession.INSTANCE, error);
    notifySessionFailure(error);
    scheduleReconnect(
        pickBackoffDelay(), ToriiWebSocketObserver.ReconnectReason.SESSION_FAILURE);
  }

  private long pickBackoffDelay() {
    return nextBackoffMs.getAndUpdate(this::computeNextBackoff);
  }

  private long computeNextBackoff(final long current) {
    final long doubled = Math.min(maxBackoffMs, current * 2);
    return doubled <= 0 ? maxBackoffMs : doubled;
  }

  private final class ManagedListener implements ToriiWebSocketListener {
    private final long generation;

    private ManagedListener(final long generation) {
      this.generation = generation;
    }

    private boolean isCurrent() {
      return generation == sessionGeneration.get();
    }

    @Override
    public void onOpen(final ToriiWebSocketSession session) {
      if (!isCurrent()) {
        return;
      }
      nextBackoffMs.set(initialBackoffMs);
      delegate.onOpen(session);
      notifySessionOpened();
    }

    @Override
    public void onText(
        final ToriiWebSocketSession session,
        final CharSequence data,
        final boolean last) {
      if (!isCurrent()) {
        return;
      }
      delegate.onText(session, data, last);
    }

    @Override
    public void onBinary(
        final ToriiWebSocketSession session,
        final java.nio.ByteBuffer data,
        final boolean last) {
      if (!isCurrent()) {
        return;
      }
      delegate.onBinary(session, data, last);
    }

    @Override
    public void onPing(final ToriiWebSocketSession session, final java.nio.ByteBuffer message) {
      if (!isCurrent()) {
        return;
      }
      delegate.onPing(session, message);
    }

    @Override
    public void onPong(final ToriiWebSocketSession session, final java.nio.ByteBuffer message) {
      if (!isCurrent()) {
        return;
      }
      delegate.onPong(session, message);
    }

    @Override
    public void onClose(
        final ToriiWebSocketSession session, final int statusCode, final String reason) {
      if (!isCurrent()) {
        return;
      }
      delegate.onClose(session, statusCode, reason);
      notifySessionClosed();
      scheduleReconnect(
          pickBackoffDelay(), ToriiWebSocketObserver.ReconnectReason.SESSION_CLOSED);
    }

    @Override
    public void onError(final ToriiWebSocketSession session, final Throwable error) {
      handleFailure(generation, session, error);
    }
  }

  private enum NullSession implements ToriiWebSocketSession {
    INSTANCE;

    @Override
    public CompletableFuture<Void> sendText(final CharSequence data, final boolean last) {
      return CompletableFuture.failedFuture(new IllegalStateException("session not open"));
    }

    @Override
    public CompletableFuture<Void> sendBinary(
        final java.nio.ByteBuffer data, final boolean last) {
      return CompletableFuture.failedFuture(new IllegalStateException("session not open"));
    }

    @Override
    public CompletableFuture<Void> sendPing(final java.nio.ByteBuffer message) {
      return CompletableFuture.failedFuture(new IllegalStateException("session not open"));
    }

    @Override
    public CompletableFuture<Void> sendPong(final java.nio.ByteBuffer message) {
      return CompletableFuture.failedFuture(new IllegalStateException("session not open"));
    }

    @Override
    public CompletableFuture<Void> close(final int statusCode, final String reason) {
      return CompletableFuture.completedFuture(null);
    }

    @Override
    public boolean isOpen() {
      return false;
    }

    @Override
    public String subprotocol() {
      return "";
    }
  }

  /** Builder for {@link ToriiWebSocketSubscription}. */
  public static final class Builder {
    private final ToriiWebSocketListener listener;
    private final SessionOpener opener;
    private ScheduledExecutorService executor;
    private boolean ownsExecutor = false;
    private long initialBackoffMs = 1_000L;
    private long maxBackoffMs = 30_000L;
    private final List<ToriiWebSocketObserver> observers = new ArrayList<>();

    private Builder(final ToriiWebSocketListener listener, final SessionOpener opener) {
      this.listener = Objects.requireNonNull(listener, "listener");
      this.opener = Objects.requireNonNull(opener, "opener");
    }

    public Builder setExecutor(final ScheduledExecutorService executor) {
      this.executor = Objects.requireNonNull(executor, "executor");
      this.ownsExecutor = false;
      return this;
    }

    public Builder setInitialBackoff(final Duration duration) {
      if (duration != null) {
        this.initialBackoffMs = Math.max(0L, duration.toMillis());
      }
      return this;
    }

    public Builder setMaxBackoff(final Duration duration) {
      if (duration != null) {
        this.maxBackoffMs = Math.max(0L, duration.toMillis());
      }
      return this;
    }

    public Builder addObserver(final ToriiWebSocketObserver observer) {
      observers.add(Objects.requireNonNull(observer, "observer"));
      return this;
    }

    public Builder observers(final List<ToriiWebSocketObserver> values) {
      observers.clear();
      if (values != null) {
        values.forEach(this::addObserver);
      }
      return this;
    }

    public ToriiWebSocketSubscription build() {
      if (executor == null) {
        executor = java.util.concurrent.Executors.newSingleThreadScheduledExecutor(r -> {
          final Thread thread = new Thread(r, "torii-websocket-subscription");
          thread.setDaemon(true);
          return thread;
        });
        ownsExecutor = true;
      }
      if (initialBackoffMs == 0) {
        initialBackoffMs = 1_000L;
      }
      if (maxBackoffMs < initialBackoffMs) {
        maxBackoffMs = initialBackoffMs;
      }
      return new ToriiWebSocketSubscription(this);
    }
  }
}
