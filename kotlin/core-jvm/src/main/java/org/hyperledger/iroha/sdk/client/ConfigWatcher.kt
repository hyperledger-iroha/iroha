package org.hyperledger.iroha.sdk.client

import java.io.IOException
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.attribute.BasicFileAttributes
import java.nio.file.attribute.FileTime
import java.time.Duration
import java.util.LinkedHashMap
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Watches a manifest file and rebuilds [ClientConfig] whenever the file changes.
 *
 * The watcher emits `sdk.telemetry.config.reload` events via the telemetry sink
 * configured on the resulting [ClientConfig] instances.
 */
class ConfigWatcher(
    manifestPath: Path,
    pollInterval: Duration?,
    private val customizer: ClientConfigManifestLoader.Customizer?,
    private val listener: Listener?
) : AutoCloseable {

    interface Listener {
        fun onReload(config: ClientConfigManifestLoader.LoadedClientConfig) {}
        fun onReloadError(error: Throwable) {}
    }

    private val manifestPath: Path = manifestPath.toAbsolutePath()
    private val pollInterval: Duration = normaliseInterval(pollInterval)
    private val executor: ScheduledExecutorService?
    private val closed = AtomicBoolean(false)

    @Volatile private var current: ClientConfigManifestLoader.LoadedClientConfig? = null
    @Volatile private var lastModified: FileTime? = null
    @Volatile private var lastSize: Long = -1L
    @Volatile private var lastDigest: String? = null

    init {
        this.executor = if (this.pollInterval.isZero) null
        else Executors.newSingleThreadScheduledExecutor { runnable ->
            Thread(runnable, "sdk-config-watcher").apply { isDaemon = true }
        }
        safeCheck()
        executor?.scheduleWithFixedDelay(::safeCheck, this.pollInterval.toMillis(), this.pollInterval.toMillis(), TimeUnit.MILLISECONDS)
    }

    fun checkNow() { safeCheck() }

    private fun safeCheck() {
        synchronized(this) {
            if (closed.get()) return
            val snapshot = captureSnapshotIfChanged() ?: return
            attemptReloadWithBackoff(snapshot)
        }
    }

    private fun handleError(error: Throwable, digest: String?) {
        listener?.onReloadError(error)
        val snapshot = current
        if (snapshot != null) emitReloadSignal(snapshot.clientConfig(), digest, "error", 0L, error)
    }

    private fun emitReloadSignal(config: ClientConfig, digest: String?, result: String, durationMs: Long, error: Throwable?) {
        val sink = config.telemetrySink()
        if (sink.isEmpty) return
        val fields = LinkedHashMap<String, Any>()
        fields["source"] = manifestPath.toString()
        fields["result"] = result
        fields["duration_ms"] = durationMs
        if (digest != null) fields["digest"] = digest
        if (error != null) {
            fields["error"] = error.javaClass.simpleName
            fields["message"] = error.message.toString()
        }
        try { sink.get().emitSignal("sdk.telemetry.config.reload", fields) } catch (_: RuntimeException) {}
    }

    private fun captureSnapshotIfChanged(): FileSnapshot? {
        val attrs: BasicFileAttributes
        try { attrs = Files.readAttributes(manifestPath, BasicFileAttributes::class.java) } catch (ex: IOException) { handleError(ex, null); return null }
        val modified = attrs.lastModifiedTime()
        val size = attrs.size()
        if (current != null && lastModified != null && modified == lastModified && size == lastSize) return null
        val payload: ByteArray
        try { payload = Files.readAllBytes(manifestPath) } catch (ex: IOException) { handleError(ex, null); return null }
        val digest = ClientConfigManifestLoader.sha256Hex(payload)
        if (digest == lastDigest) { lastModified = modified; lastSize = size; return null }
        return FileSnapshot(payload, digest, modified, size)
    }

    private fun attemptReloadWithBackoff(initialSnapshot: FileSnapshot) {
        var snapshot = initialSnapshot
        var backoff = Duration.ofMillis(INITIAL_BACKOFF_MILLIS)
        for (attempt in 1..MAX_RELOAD_ATTEMPTS) {
            if (closed.get()) return
            if (lastDigest != null && snapshot.digest == lastDigest) { lastModified = snapshot.modified; lastSize = snapshot.size; return }
            try {
                val loaded = applySnapshot(snapshot)
                if (listener != null) {
                    try { listener.onReload(loaded) } catch (listenerError: RuntimeException) { listener.onReloadError(listenerError) }
                }
                return
            } catch (ex: RuntimeException) { handleError(ex, snapshot.digest) }
            if (attempt == MAX_RELOAD_ATTEMPTS) return
            sleep(backoff)
            backoff = backoff.multipliedBy(2)
            try { snapshot = readSnapshot() } catch (ex: IOException) { handleError(ex, null); return }
        }
    }

    private fun applySnapshot(snapshot: FileSnapshot): ClientConfigManifestLoader.LoadedClientConfig {
        val start = System.nanoTime()
        val loaded = ClientConfigManifestLoader.parse(manifestPath, snapshot.payload, snapshot.digest, customizer)
        current = loaded; lastModified = snapshot.modified; lastSize = snapshot.size; lastDigest = snapshot.digest
        emitReloadSignal(loaded.clientConfig(), snapshot.digest, "success", TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - start), null)
        return loaded
    }

    @Throws(IOException::class)
    private fun readSnapshot(): FileSnapshot {
        val attrs = Files.readAttributes(manifestPath, BasicFileAttributes::class.java)
        val payload = Files.readAllBytes(manifestPath)
        return FileSnapshot(payload, ClientConfigManifestLoader.sha256Hex(payload), attrs.lastModifiedTime(), attrs.size())
    }

    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        executor?.shutdownNow()
    }

    private data class FileSnapshot(val payload: ByteArray, val digest: String, val modified: FileTime, val size: Long)

    companion object {
        private const val MAX_RELOAD_ATTEMPTS = 5
        private const val INITIAL_BACKOFF_MILLIS = 50L

        private fun sleep(delay: Duration) {
            if (delay.isZero || delay.isNegative) return
            try { TimeUnit.MILLISECONDS.sleep(delay.toMillis()) } catch (_: InterruptedException) { Thread.currentThread().interrupt() }
        }

        private fun normaliseInterval(interval: Duration?): Duration {
            if (interval == null || interval.isNegative) return Duration.ZERO
            return interval
        }
    }
}
