package org.hyperledger.iroha.android.client.stream;

import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Convenience helper that keeps a Torii SSE stream alive by reconnecting when the underlying
 * connection drops. Reconnection delays honour `retry:` hints emitted by the server and fall back to
 * exponential backoff.
 */
public final class ToriiEventStreamSubscription implements AutoCloseable {

  /** Factory used to open event streams (allows dependency injection in tests). */
  @FunctionalInterface
  public interface StreamOpener {
    ToriiEventStream open(ToriiEventStreamListener listener);
  }

  private final StreamOpener opener;
  private final ToriiEventStreamListener delegate;
  private final ScheduledExecutorService executor;
  private final boolean ownsExecutor;
  private final long initialBackoffMs;
  private final long maxBackoffMs;
  private final AtomicBoolean started = new AtomicBoolean(false);
  private final AtomicBoolean closed = new AtomicBoolean(false);
  private final AtomicLong nextBackoffMs;
  private final AtomicLong lastRetryHintMs = new AtomicLong(-1);
  private final AtomicLong streamGeneration = new AtomicLong(0);
  private final AtomicReference<ToriiEventStream> currentStream = new AtomicReference<>(null);
  private final AtomicReference<ScheduledFuture<?>> scheduledTask = new AtomicReference<>(null);
  private final List<ToriiEventStreamObserver> observers;

  private ToriiEventStreamSubscription(final Builder builder) {
    this.opener = builder.opener;
    this.delegate = builder.listener;
    this.executor = builder.executor;
    this.ownsExecutor = builder.ownsExecutor;
    this.initialBackoffMs = builder.initialBackoffMs;
    this.maxBackoffMs = builder.maxBackoffMs;
    this.nextBackoffMs = new AtomicLong(initialBackoffMs);
    this.observers = List.copyOf(builder.observers);
  }

  /** Returns a builder that opens streams via the provided client/path/options triple. */
  public static Builder builder(
      final ToriiEventStreamClient client,
      final String path,
      final ToriiEventStreamOptions options,
      final ToriiEventStreamListener listener) {
    Objects.requireNonNull(client, "client");
    final ToriiEventStreamOptions resolved =
        options != null ? options : ToriiEventStreamOptions.defaultOptions();
    return new Builder(listener, delegate ->
        client.openSseStream(path, resolved, delegate));
  }

  /** Returns a builder that uses a custom {@link StreamOpener}. */
  public static Builder builder(
      final StreamOpener opener, final ToriiEventStreamListener listener) {
    return new Builder(listener, opener);
  }

  /** Starts the subscription, initiating the first stream immediately. */
  public synchronized ToriiEventStreamSubscription start() {
    if (!started.compareAndSet(false, true)) {
      throw new IllegalStateException("subscription already started");
    }
    scheduleReconnect(0, ToriiEventStreamObserver.ReconnectReason.INITIAL);
    return this;
  }

  /** Returns {@code true} when the subscription is actively running. */
  public boolean isRunning() {
    return started.get() && !closed.get();
  }

  /** Stops the subscription, closes the underlying stream, and shuts down internal resources. */
  @Override
  public void close() {
    if (!closed.compareAndSet(false, true)) {
      return;
    }
    final ScheduledFuture<?> future = scheduledTask.getAndSet(null);
    if (future != null) {
      future.cancel(true);
    }
    final ToriiEventStream stream = currentStream.getAndSet(null);
    if (stream != null) {
      stream.close();
    }
    if (ownsExecutor) {
      executor.shutdownNow();
    }
  }

  private void scheduleReconnect(
      final long delayMs, final ToriiEventStreamObserver.ReconnectReason reason) {
    if (closed.get()) {
      return;
    }
    final ScheduledFuture<?> existing = scheduledTask.get();
    if (existing != null && !existing.isDone() && !existing.isCancelled()) {
      return;
    }
    final long clampedDelay = Math.max(0, delayMs);
    notifyReconnectScheduled(clampedDelay, reason);
    final ScheduledFuture<?> future =
        executor.schedule(this::openStream, clampedDelay, TimeUnit.MILLISECONDS);
    scheduledTask.set(future);
  }

  private void notifyReconnectScheduled(
      final long delayMs, final ToriiEventStreamObserver.ReconnectReason reason) {
    if (observers.isEmpty()) {
      return;
    }
    final Duration duration = Duration.ofMillis(Math.max(0L, delayMs));
    for (final ToriiEventStreamObserver observer : observers) {
      observer.onReconnectScheduled(duration, reason);
    }
  }

  private void notifyStreamOpened() {
    for (final ToriiEventStreamObserver observer : observers) {
      observer.onStreamOpened();
    }
  }

  private void notifyStreamClosed() {
    for (final ToriiEventStreamObserver observer : observers) {
      observer.onStreamClosed();
    }
  }

  private void notifyStreamFailure(final Throwable error) {
    for (final ToriiEventStreamObserver observer : observers) {
      observer.onStreamFailure(error);
    }
  }

  private void openStream() {
    if (closed.get()) {
      return;
    }
    final long generation = streamGeneration.incrementAndGet();
    final ManagedListener listener = new ManagedListener(generation);
    try {
      final ToriiEventStream stream = opener.open(listener);
      currentStream.set(stream);
      nextBackoffMs.set(initialBackoffMs);
      notifyStreamOpened();
    } catch (final RuntimeException ex) {
      handleFailure(generation, ex);
    }
  }

  private void handleFailure(final long generation, final Throwable error) {
    if (closed.get()) {
      return;
    }
    if (generation != streamGeneration.get()) {
      return;
    }
    delegate.onError(error);
    notifyStreamFailure(error);
    final long retryHint = lastRetryHintMs.getAndSet(-1);
    final long delay =
        retryHint >= 0 ? retryHint : nextBackoffMs.getAndUpdate(this::computeNextBackoff);
    scheduleReconnect(delay, ToriiEventStreamObserver.ReconnectReason.STREAM_FAILURE);
  }

  private long computeNextBackoff(final long current) {
    final long doubled = Math.min(maxBackoffMs, current * 2);
    return doubled <= 0 ? maxBackoffMs : doubled;
  }

  private final class ManagedListener implements ToriiEventStreamListener {
    private final long generation;

    private ManagedListener(final long generation) {
      this.generation = generation;
    }

    private boolean isCurrent() {
      return generation == streamGeneration.get();
    }

    @Override
    public void onOpen() {
      if (!isCurrent()) {
        return;
      }
      delegate.onOpen();
    }

    @Override
    public void onEvent(final ServerSentEvent event) {
      if (!isCurrent()) {
        return;
      }
      delegate.onEvent(event);
    }

    @Override
    public void onRetryHint(final Duration duration) {
      if (!isCurrent()) {
        return;
      }
      if (duration != null) {
        lastRetryHintMs.set(Math.max(0L, duration.toMillis()));
      }
      delegate.onRetryHint(duration);
    }

    @Override
    public void onClosed() {
      if (!isCurrent()) {
        return;
      }
      delegate.onClosed();
      notifyStreamClosed();
      scheduleReconnect(
          pickBackoffDelay(), ToriiEventStreamObserver.ReconnectReason.STREAM_CLOSED);
    }

    @Override
    public void onError(final Throwable error) {
      handleFailure(generation, error);
    }
  }

  private long pickBackoffDelay() {
    final long retryHint = lastRetryHintMs.getAndSet(-1);
    if (retryHint >= 0) {
      return retryHint;
    }
    return nextBackoffMs.getAndUpdate(this::computeNextBackoff);
  }

  /** Builder for {@link ToriiEventStreamSubscription}. */
  public static final class Builder {
    private final ToriiEventStreamListener listener;
    private final StreamOpener opener;
    private ScheduledExecutorService executor;
    private boolean ownsExecutor = false;
    private long initialBackoffMs = 1_000L;
    private long maxBackoffMs = 30_000L;
    private final List<ToriiEventStreamObserver> observers = new ArrayList<>();

    private Builder(final ToriiEventStreamListener listener, final StreamOpener opener) {
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

    public Builder addObserver(final ToriiEventStreamObserver observer) {
      observers.add(Objects.requireNonNull(observer, "observer"));
      return this;
    }

    public Builder observers(final List<ToriiEventStreamObserver> values) {
      observers.clear();
      if (values != null) {
        values.forEach(this::addObserver);
      }
      return this;
    }

    public ToriiEventStreamSubscription build() {
      if (executor == null) {
        executor = java.util.concurrent.Executors.newSingleThreadScheduledExecutor(r -> {
          final Thread thread = new Thread(r, "torii-event-stream-subscription");
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
      return new ToriiEventStreamSubscription(this);
    }
  }
}
