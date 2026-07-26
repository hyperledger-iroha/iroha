package org.hyperledger.iroha.sdk.connect

import java.nio.file.Path
import java.nio.file.Paths
import java.time.Duration

/** Queue limits and retention policy for `ConnectQueueJournal`. */
class JournalConfiguration(
    @JvmField val rootDirectory: Path = defaultRootDirectory(),
    @JvmField val maxRecordsPerQueue: Int = 32,
    @JvmField val maxBytesPerQueue: Int = 1 shl 20,
    @JvmField val retentionMillis: Long = Duration.ofHours(24).toMillis(),
) {
    init {
        require(maxRecordsPerQueue > 0) { "maxRecordsPerQueue must be positive" }
        require(maxBytesPerQueue > 0) { "maxBytesPerQueue must be positive" }
        require(retentionMillis > 0) { "retentionMillis must be positive" }
    }
}

private fun defaultRootDirectory(): Path {
    val home = System.getProperty("user.home")
    val base = if (home != null) Paths.get(home) else Paths.get(".")
    return base.resolve(".iroha").resolve("connect").resolve("queues")
}
