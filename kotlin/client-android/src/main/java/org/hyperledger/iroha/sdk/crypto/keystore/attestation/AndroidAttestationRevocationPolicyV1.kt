package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.MessageDigest

/**
 * Canonical, offline Android attestation certificate-status snapshot.
 *
 * Construction requires the exact domain-separated V1 snapshot bytes and a SHA-256 commitment
 * obtained through a separate trusted governance channel. Every freshness field and both deny
 * lists are decoded from those committed bytes; callers cannot assemble them independently.
 */
class AndroidAttestationRevocationPolicyV1 private constructor(
    canonicalSnapshot: ByteArray,
    trustedSnapshotSha256: ByteArray,
) {
    private val payloadSha256: ByteArray
    val responseDateEpochMillis: Long
    val lastModifiedEpochMillis: Long?
    val cacheMaxAgeSeconds: Long
    private val nonValidCertificateSerials: List<String>
    private val revokedCertificateTbsSha256: List<ByteArray>

    init {
        require(canonicalSnapshot.isNotEmpty() && canonicalSnapshot.size <= MAX_SNAPSHOT_BYTES) {
            "Revocation snapshot size is outside the V1 bounds"
        }
        require(
            trustedSnapshotSha256.size == SHA256_BYTES &&
                trustedSnapshotSha256.any { it.toInt() != 0 }
        ) {
            "Trusted revocation snapshot SHA-256 must be a non-zero 32-byte digest"
        }
        val actualSnapshotSha256 = MessageDigest.getInstance("SHA-256").digest(canonicalSnapshot)
        require(MessageDigest.isEqual(trustedSnapshotSha256, actualSnapshotSha256)) {
            "Revocation snapshot SHA-256 does not match the trusted governance commitment"
        }

        val decoded = decodeCanonicalSnapshot(canonicalSnapshot)
        payloadSha256 = decoded.payloadSha256
        responseDateEpochMillis = decoded.responseDateEpochMillis
        lastModifiedEpochMillis = decoded.lastModifiedEpochMillis
        cacheMaxAgeSeconds = decoded.cacheMaxAgeSeconds
        nonValidCertificateSerials = decoded.nonValidCertificateSerials
        revokedCertificateTbsSha256 = decoded.revokedCertificateTbsSha256
        freshUntilEpochMillis()
    }

    /** Fails unless `evaluationTimeEpochMillis` falls in the snapshot's half-open freshness window. */
    @Throws(AttestationVerificationException::class)
    fun validateAt(evaluationTimeEpochMillis: Long) {
        val freshUntil = freshUntilEpochMillis()
        if (evaluationTimeEpochMillis < responseDateEpochMillis) {
            throw AttestationVerificationException(
                "Revocation status response date is in the future"
            )
        }
        if (evaluationTimeEpochMillis >= freshUntil) {
            throw AttestationVerificationException("Revocation status snapshot is stale")
        }
    }

    /** Returns true when either governed deny list rejects the certificate. */
    fun rejects(certificateSerial: BigInteger, certificateTbsSha256: ByteArray): Boolean {
        val serial = certificateSerial.toString(16)
        if (nonValidCertificateSerials.binarySearch(serial) >= 0) {
            return true
        }
        return revokedCertificateTbsSha256.any { candidate ->
            MessageDigest.isEqual(candidate, certificateTbsSha256)
        }
    }

    private fun freshUntilEpochMillis(): Long = try {
        Math.addExact(
            responseDateEpochMillis,
            Math.multiplyExact(cacheMaxAgeSeconds, 1_000L),
        )
    } catch (ex: ArithmeticException) {
        throw IllegalArgumentException("Revocation freshness bound overflows epoch milliseconds", ex)
    }

    private data class DecodedSnapshot(
        val payloadSha256: ByteArray,
        val responseDateEpochMillis: Long,
        val lastModifiedEpochMillis: Long?,
        val cacheMaxAgeSeconds: Long,
        val nonValidCertificateSerials: List<String>,
        val revokedCertificateTbsSha256: List<ByteArray>,
    )

    companion object {
        /** Exact first line of the canonical V1 snapshot. */
        const val SNAPSHOT_DOMAIN = "iroha.android.attestation.revocation.snapshot.v1"

        private const val SHA256_BYTES = 32
        private const val MAX_SNAPSHOT_BYTES = 512 * 1024
        private const val MAX_SERIALS = 4_096
        private const val MAX_SERIAL_HEX_LENGTH = 40
        private const val MAX_TBS_DIGESTS = 256
        private const val MAX_CACHE_AGE_SECONDS = 86_400L

        /** Verifies and decodes one canonical V1 snapshot against a trusted commitment. */
        @JvmStatic
        fun fromCanonicalSnapshot(
            canonicalSnapshot: ByteArray,
            trustedSnapshotSha256: ByteArray,
        ): AndroidAttestationRevocationPolicyV1 = AndroidAttestationRevocationPolicyV1(
            canonicalSnapshot.copyOf(),
            trustedSnapshotSha256.copyOf(),
        )

        private fun decodeCanonicalSnapshot(bytes: ByteArray): DecodedSnapshot {
            require(bytes.last() == '\n'.code.toByte()) {
                "Canonical revocation snapshot must end with one newline"
            }
            require(bytes.all { byte ->
                val value = byte.toInt() and 0xff
                value == '\n'.code || value in 0x20..0x7e
            }) {
                "Canonical revocation snapshot must contain printable ASCII lines"
            }
            val text = String(bytes, StandardCharsets.US_ASCII)
            val lines = text.substring(0, text.length - 1).split('\n')
            require(lines.none { it.isEmpty() }) {
                "Canonical revocation snapshot contains an empty line"
            }
            var cursor = 0
            fun next(label: String): String {
                require(cursor < lines.size) { "Canonical revocation snapshot is missing $label" }
                return lines[cursor++]
            }
            require(next("domain") == SNAPSHOT_DOMAIN) {
                "Canonical revocation snapshot domain/version is unsupported"
            }
            val payloadSha256 = parseDigest(
                exactValue(next("payload_sha256"), "payload_sha256"),
                "payload_sha256",
            )
            val responseDate = parsePositiveLong(
                exactValue(next("response_date_ms"), "response_date_ms"),
                "response_date_ms",
            )
            require(responseDate % 1_000L == 0L) {
                "Revocation response date must be a whole-second epoch timestamp"
            }
            val lastModifiedValue = exactValue(next("last_modified_ms"), "last_modified_ms")
            val lastModified = if (lastModifiedValue == "-") {
                null
            } else {
                parsePositiveLong(lastModifiedValue, "last_modified_ms")
            }
            if (lastModified != null) {
                require(lastModified % 1_000L == 0L && lastModified <= responseDate) {
                    "Revocation last-modified date is outside the canonical bounds"
                }
            }
            val cacheMaxAge = parsePositiveLong(
                exactValue(next("cache_max_age_seconds"), "cache_max_age_seconds"),
                "cache_max_age_seconds",
            )
            require(cacheMaxAge in 1L..MAX_CACHE_AGE_SECONDS) {
                "Revocation cache max-age is outside the V1 bounds"
            }

            val serialCount = parseCount(
                exactValue(next("serial_count"), "serial_count"),
                "serial_count",
                MAX_SERIALS,
            )
            val serials = ArrayList<String>(serialCount)
            repeat(serialCount) {
                val serial = exactValue(next("serial"), "serial")
                require(isCanonicalSerial(serial)) {
                    "Revocation certificate serial is not canonical lowercase hexadecimal: $serial"
                }
                require(serials.isEmpty() || serials.last() < serial) {
                    "Revocation certificate serials must be sorted and unique"
                }
                serials.add(serial)
            }

            val tbsCount = parseCount(
                exactValue(next("tbs_sha256_count"), "tbs_sha256_count"),
                "tbs_sha256_count",
                MAX_TBS_DIGESTS,
            )
            val tbsDigests = ArrayList<ByteArray>(tbsCount)
            var previousTbs: String? = null
            repeat(tbsCount) {
                val encoded = exactValue(next("tbs_sha256"), "tbs_sha256")
                val digest = parseDigest(encoded, "tbs_sha256")
                require(previousTbs == null || previousTbs < encoded) {
                    "Revoked certificate TBS SHA-256 values must be sorted and unique"
                }
                previousTbs = encoded
                tbsDigests.add(digest)
            }
            require(cursor == lines.size) {
                "Canonical revocation snapshot contains trailing fields"
            }
            return DecodedSnapshot(
                payloadSha256,
                responseDate,
                lastModified,
                cacheMaxAge,
                serials,
                tbsDigests,
            )
        }

        private fun exactValue(line: String, key: String): String {
            val prefix = "$key="
            require(line.startsWith(prefix) && line.length > prefix.length) {
                "Canonical revocation snapshot expected $key"
            }
            return line.substring(prefix.length)
        }

        private fun parsePositiveLong(value: String, label: String): Long {
            require(value.isNotEmpty() && value.all { it in '0'..'9' } && value[0] != '0') {
                "Canonical revocation snapshot $label is not a positive decimal integer"
            }
            return value.toLongOrNull()
                ?: throw IllegalArgumentException("Canonical revocation snapshot $label overflows")
        }

        private fun parseCount(value: String, label: String, maximum: Int): Int {
            require(
                value.isNotEmpty() &&
                    value.all { it in '0'..'9' } &&
                    (value == "0" || value[0] != '0')
            ) {
                "Canonical revocation snapshot $label is not a canonical decimal integer"
            }
            val parsed = value.toIntOrNull()
                ?: throw IllegalArgumentException("Canonical revocation snapshot $label overflows")
            require(parsed in 0..maximum) {
                "Canonical revocation snapshot $label is outside the V1 bounds"
            }
            return parsed
        }

        private fun parseDigest(value: String, label: String): ByteArray {
            require(
                value.length == SHA256_BYTES * 2 &&
                    value.all { it in '0'..'9' || it in 'a'..'f' }
            ) {
                "Canonical revocation snapshot $label is not lowercase SHA-256"
            }
            val decoded = ByteArray(SHA256_BYTES)
            for (index in decoded.indices) {
                decoded[index] = value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
            }
            require(decoded.any { it.toInt() != 0 }) {
                "Canonical revocation snapshot $label must not be all zero"
            }
            return decoded
        }

        private fun isCanonicalSerial(serial: String): Boolean {
            if (serial.isEmpty() || serial.length > MAX_SERIAL_HEX_LENGTH) return false
            if (serial.length > 1 && serial[0] == '0') return false
            return serial.all { character -> character in '0'..'9' || character in 'a'..'f' }
        }
    }
}
