package org.hyperledger.iroha.sdk.crypto.keystore

import android.system.ErrnoException
import android.system.Os
import android.system.OsConstants
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.FileDescriptor
import java.io.ByteArrayOutputStream

/** Platform file operations use no-follow opens, create-new, fsync, and a process-shared lock. */
internal interface KagemushaAndroidOriginalJournalIoV1 {
    fun exists(file: File): Boolean
    fun read(file: File, maximum: Int): ByteArray
    fun writeNew(file: File, bytes: ByteArray)
    fun syncExisting(file: File)
    fun <T> withLock(file: File, action: () -> T): T
}

internal object AndroidOriginalJournalIoV1 : KagemushaAndroidOriginalJournalIoV1 {
    private val processLocks = Array(256) { Any() }

    private fun closeAfterFailure(descriptor: FileDescriptor) {
        try { Os.close(descriptor) } catch (_: Exception) { /* The stream may have closed it. */ }
    }

    override fun exists(file: File): Boolean = try {
        val stat = Os.lstat(file.absolutePath)
        require(OsConstants.S_ISREG(stat.st_mode)) { "original journal entry is not regular" }
        true
    } catch (error: ErrnoException) {
        if (error.errno == OsConstants.ENOENT) false else throw error
    }

    override fun read(file: File, maximum: Int): ByteArray {
        val descriptor = Os.open(
            file.absolutePath,
            OsConstants.O_RDONLY or OsConstants.O_NOFOLLOW or OsConstants.O_CLOEXEC,
            0,
        )
        try {
            val stat = Os.fstat(descriptor)
            require(OsConstants.S_ISREG(stat.st_mode) && stat.st_size in 1L..maximum.toLong()) {
                "original journal entry is not a bounded regular file"
            }
            return FileInputStream(descriptor).use { stream ->
                val bytes = ByteArrayOutputStream()
                val buffer = ByteArray(4096)
                while (true) {
                    val count = stream.read(buffer)
                    if (count < 0) break
                    bytes.write(buffer, 0, count)
                    require(bytes.size() <= maximum) { "original journal entry grew during read" }
                }
                bytes.toByteArray()
            }
        } catch (error: Throwable) {
            closeAfterFailure(descriptor)
            throw error
        }
    }

    override fun writeNew(file: File, bytes: ByteArray) {
        val descriptor = Os.open(
            file.absolutePath,
            OsConstants.O_WRONLY or OsConstants.O_CREAT or OsConstants.O_EXCL or
                OsConstants.O_NOFOLLOW or OsConstants.O_CLOEXEC,
            384,
        )
        try {
            FileOutputStream(descriptor).use { stream ->
                stream.write(bytes)
                stream.fd.sync()
            }
        } catch (error: Throwable) {
            closeAfterFailure(descriptor)
            throw error
        }
        syncDirectory(file)
    }

    override fun syncExisting(file: File) {
        val descriptor = Os.open(file.absolutePath,
            OsConstants.O_RDONLY or OsConstants.O_NOFOLLOW or OsConstants.O_CLOEXEC, 0)
        try {
            require(OsConstants.S_ISREG(Os.fstat(descriptor).st_mode)) { "original journal entry is not regular" }
            Os.fsync(descriptor)
        } finally { Os.close(descriptor) }
        syncDirectory(file)
    }

    private fun syncDirectory(file: File) {
        val directory = checkNotNull(file.parentFile)
        val directoryDescriptor = Os.open(
            directory.absolutePath,
            OsConstants.O_RDONLY or OsConstants.O_NOFOLLOW or OsConstants.O_CLOEXEC,
            0,
        )
        try {
            Os.fsync(directoryDescriptor)
        } finally {
            Os.close(directoryDescriptor)
        }
    }

    override fun <T> withLock(file: File, action: () -> T): T {
        val monitor = processLocks[(file.absolutePath.hashCode() and Int.MAX_VALUE) % processLocks.size]
        return synchronized(monitor) {
            val descriptor = Os.open(
                file.absolutePath,
                OsConstants.O_RDWR or OsConstants.O_CREAT or OsConstants.O_NOFOLLOW or
                    OsConstants.O_CLOEXEC,
                384,
            )
            try {
                require(OsConstants.S_ISREG(Os.fstat(descriptor).st_mode)) {
                    "original lock entry is not regular"
                }
                FileOutputStream(descriptor).use { stream ->
                    stream.channel.lock().use { action() }
                }
            } catch (error: Throwable) {
                closeAfterFailure(descriptor)
                throw error
            }
        }
    }
}
