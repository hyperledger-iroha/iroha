package org.hyperledger.iroha.android.client;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.BasicFileAttributes;
import java.nio.file.attribute.FileTime;
import java.time.Duration;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ThreadFactory;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;

/**
 * Watches a manifest file and rebuilds {@link ClientConfig} whenever the file changes.
 *
 * <p>The watcher emits {@code android.telemetry.config.reload} events via the telemetry sink
 * configured on the resulting {@link ClientConfig} instances.
 */
public final class ConfigWatcher implements AutoCloseable {
  private static final int MAX_RELOAD_ATTEMPTS = 5;
  private static final long INITIAL_BACKOFF_MILLIS = 50L;

  /** Listener invoked when the manifest is reloaded or when parsing fails. */
  public interface Listener {
    default void onReload(final ClientConfigManifestLoader.LoadedClientConfig config) {}

    default void onReloadError(final Throwable error) {}
  }

  private final Path manifestPath;
  private final Duration pollInterval;
  private final ClientConfigManifestLoader.Customizer customizer;
  private final Listener listener;
  private final ScheduledExecutorService executor;
  private final AtomicBoolean closed = new AtomicBoolean(false);

  private volatile ClientConfigManifestLoader.LoadedClientConfig current;
  private volatile FileTime lastModified;
  private volatile long lastSize = -1L;
  private volatile String lastDigest;

  /**
   * Creates a watcher for {@code manifestPath}. When {@code pollInterval} is positive the watcher
   * periodically reloads the manifest in the background; otherwise callers may invoke {@link
   * #checkNow()} manually.
   */
  public ConfigWatcher(
      final Path manifestPath,
      final Duration pollInterval,
      final ClientConfigManifestLoader.Customizer customizer,
      final Listener listener) {
    this.manifestPath =
        Objects.requireNonNull(manifestPath, "manifestPath").toAbsolutePath();
    this.pollInterval = normaliseInterval(pollInterval);
    this.customizer = customizer;
    this.listener = listener;
    this.executor =
        this.pollInterval.isZero()
            ? null
            : Executors.newSingleThreadScheduledExecutor(createThreadFactory());
    safeCheck();
    if (executor != null) {
      executor.scheduleWithFixedDelay(
          this::safeCheck,
          this.pollInterval.toMillis(),
          this.pollInterval.toMillis(),
          TimeUnit.MILLISECONDS);
    }
  }

  /** Triggers a manifest reload immediately. */
  public void checkNow() {
    safeCheck();
  }

  private void safeCheck() {
    synchronized (this) {
      if (closed.get()) {
        return;
      }
      final FileSnapshot snapshot = captureSnapshotIfChanged();
      if (snapshot == null) {
        return;
      }
      attemptReloadWithBackoff(snapshot);
    }
  }

  private void handleError(final Throwable error, final String digest) {
    if (listener != null) {
      listener.onReloadError(error);
    }
    final ClientConfigManifestLoader.LoadedClientConfig snapshot = current;
    if (snapshot != null) {
      emitReloadSignal(snapshot.clientConfig(), digest, "error", 0L, error);
    }
  }

  private void emitReloadSignal(
      final ClientConfig config,
      final String digest,
      final String result,
      final long durationMs,
      final Throwable error) {
    final Optional<TelemetrySink> sink = config.telemetrySink();
    if (sink.isEmpty()) {
      return;
    }
    final Map<String, Object> fields = new LinkedHashMap<>();
    fields.put("source", manifestPath.toString());
    fields.put("result", result);
    fields.put("duration_ms", durationMs);
    if (digest != null) {
      fields.put("digest", digest);
    }
    if (error != null) {
      fields.put("error", error.getClass().getSimpleName());
      fields.put("message", String.valueOf(error.getMessage()));
    }
    try {
      sink.get().emitSignal("android.telemetry.config.reload", fields);
    } catch (final RuntimeException ignored) {
      // best-effort emission; avoid surfacing telemetry failures to callers.
    }
  }

  private FileSnapshot captureSnapshotIfChanged() {
    final BasicFileAttributes attrs;
    try {
      attrs = Files.readAttributes(manifestPath, BasicFileAttributes.class);
    } catch (final IOException ex) {
      handleError(ex, null);
      return null;
    }
    final FileTime modified = attrs.lastModifiedTime();
    final long size = attrs.size();
    if (current != null
        && lastModified != null
        && modified.equals(lastModified)
        && size == lastSize) {
      return null;
    }
    final byte[] payload;
    try {
      payload = Files.readAllBytes(manifestPath);
    } catch (final IOException ex) {
      handleError(ex, null);
      return null;
    }
    final String digest = ClientConfigManifestLoader.sha256Hex(payload);
    if (digest.equals(lastDigest)) {
      lastModified = modified;
      lastSize = size;
      return null;
    }
    return new FileSnapshot(payload, digest, modified, size);
  }

  private void attemptReloadWithBackoff(FileSnapshot snapshot) {
    Duration backoff = Duration.ofMillis(INITIAL_BACKOFF_MILLIS);
    for (int attempt = 1; attempt <= MAX_RELOAD_ATTEMPTS; attempt++) {
      if (closed.get()) {
        return;
      }
      if (lastDigest != null && snapshot.digest().equals(lastDigest)) {
        lastModified = snapshot.modified();
        lastSize = snapshot.size();
        return;
      }
      try {
        final ClientConfigManifestLoader.LoadedClientConfig loaded = applySnapshot(snapshot);
        if (listener != null) {
          try {
            listener.onReload(loaded);
          } catch (final RuntimeException listenerError) {
            listener.onReloadError(listenerError);
          }
        }
        return;
      } catch (final RuntimeException ex) {
        handleError(ex, snapshot.digest());
      }
      if (attempt == MAX_RELOAD_ATTEMPTS) {
        return;
      }
      sleep(backoff);
      backoff = backoff.multipliedBy(2);
      try {
        snapshot = readSnapshot();
      } catch (final IOException ex) {
        handleError(ex, null);
        return;
      }
    }
  }

  private ClientConfigManifestLoader.LoadedClientConfig applySnapshot(
      final FileSnapshot snapshot) {
    final long start = System.nanoTime();
    final ClientConfigManifestLoader.LoadedClientConfig loaded =
        ClientConfigManifestLoader.parse(
            manifestPath, snapshot.payload(), snapshot.digest(), customizer);
    current = loaded;
    lastModified = snapshot.modified();
    lastSize = snapshot.size();
    lastDigest = snapshot.digest();
    emitReloadSignal(
        loaded.clientConfig(),
        snapshot.digest(),
        "success",
        TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - start),
        null);
    return loaded;
  }

  private FileSnapshot readSnapshot() throws IOException {
    final BasicFileAttributes attrs = Files.readAttributes(manifestPath, BasicFileAttributes.class);
    final byte[] payload = Files.readAllBytes(manifestPath);
    final String digest = ClientConfigManifestLoader.sha256Hex(payload);
    return new FileSnapshot(payload, digest, attrs.lastModifiedTime(), attrs.size());
  }

  private static void sleep(final Duration delay) {
    if (delay.isZero() || delay.isNegative()) {
      return;
    }
    try {
      TimeUnit.MILLISECONDS.sleep(delay.toMillis());
    } catch (final InterruptedException ex) {
      Thread.currentThread().interrupt();
    }
  }

  private static Duration normaliseInterval(final Duration interval) {
    if (interval == null || interval.isNegative()) {
      return Duration.ZERO;
    }
    return interval;
  }

  private static ThreadFactory createThreadFactory() {
    return runnable -> {
      final Thread thread = new Thread(runnable, "android-config-watcher");
      thread.setDaemon(true);
      return thread;
    };
  }

  @Override
  public void close() {
    if (!closed.compareAndSet(false, true)) {
      return;
    }
    if (executor != null) {
      executor.shutdownNow();
    }
  }

  private record FileSnapshot(
      byte[] payload, String digest, FileTime modified, long size) {}
}
