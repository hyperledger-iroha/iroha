package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest

private const val KEY_LENGTH = 32

/** Key material used to authenticate `OfflineJournal` records. */
class OfflineJournalKey @Throws(OfflineJournalException::class) constructor(
    rawBytes: ByteArray?,
) {
    private val _raw: ByteArray

    init {
        if (rawBytes == null || rawBytes.size != KEY_LENGTH) {
            throw OfflineJournalException(
                OfflineJournalException.Reason.INVALID_KEY_LENGTH,
                "offline journal key must be exactly 32 bytes",
            )
        }
        _raw = rawBytes.copyOf()
    }

    internal fun raw(): ByteArray = _raw.copyOf()

    companion object {
        /** Derives a key from arbitrary seed material via SHA-256. */
        @JvmStatic
        fun derive(seed: ByteArray?): OfflineJournalKey {
            try {
                val digest = MessageDigest.getInstance("SHA-256")
                val output = digest.digest(seed ?: ByteArray(0))
                return OfflineJournalKey(output)
            } catch (ex: Exception) {
                throw IllegalStateException("Failed to derive offline journal key", ex)
            }
        }

        /** Derives a key from a passphrase (UTF-8 encoded). */
        @JvmStatic
        fun deriveFromPassphrase(passphrase: CharArray?): OfflineJournalKey {
            if (passphrase == null) return derive(ByteArray(0))
            val utf8 = String(passphrase).toByteArray(Charsets.UTF_8)
            return derive(utf8)
        }
    }
}
