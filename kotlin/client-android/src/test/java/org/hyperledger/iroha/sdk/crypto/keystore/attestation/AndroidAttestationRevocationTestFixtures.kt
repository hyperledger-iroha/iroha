package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.nio.charset.StandardCharsets
import java.security.MessageDigest

internal object AndroidAttestationRevocationTestFixtures {
    const val CANONICAL_VECTOR_SHA256 =
        "154efc56abd2b7e403c5a362147971a561acc95f7274238ae0c52af196501b03"

    fun policy(
        responseDateEpochMillis: Long,
        cacheMaxAgeSeconds: Long,
        serials: List<String> = emptyList(),
        tbsDigests: List<ByteArray> = emptyList(),
    ): AndroidAttestationRevocationPolicyV1 {
        val snapshot = canonicalSnapshot(
            responseDateEpochMillis = responseDateEpochMillis,
            cacheMaxAgeSeconds = cacheMaxAgeSeconds,
            serials = serials,
            tbsDigests = tbsDigests,
        )
        return AndroidAttestationRevocationPolicyV1.fromCanonicalSnapshot(
            snapshot,
            sha256(snapshot),
        )
    }

    fun canonicalSnapshot(
        responseDateEpochMillis: Long,
        lastModifiedEpochMillis: Long? = null,
        cacheMaxAgeSeconds: Long,
        serials: List<String> = emptyList(),
        tbsDigests: List<ByteArray> = emptyList(),
    ): ByteArray {
        val canonicalSerials = serials.sorted()
        val canonicalTbs = tbsDigests.map(::hex).sorted()
        return buildString {
            append(AndroidAttestationRevocationPolicyV1.SNAPSHOT_DOMAIN).append('\n')
            append("payload_sha256=").append("11".repeat(32)).append('\n')
            append("response_date_ms=").append(responseDateEpochMillis).append('\n')
            append("last_modified_ms=").append(lastModifiedEpochMillis ?: "-").append('\n')
            append("cache_max_age_seconds=").append(cacheMaxAgeSeconds).append('\n')
            append("serial_count=").append(canonicalSerials.size).append('\n')
            canonicalSerials.forEach { append("serial=").append(it).append('\n') }
            append("tbs_sha256_count=").append(canonicalTbs.size).append('\n')
            canonicalTbs.forEach { append("tbs_sha256=").append(it).append('\n') }
        }.toByteArray(StandardCharsets.US_ASCII)
    }

    fun sha256(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)

    fun hex(bytes: ByteArray): String = buildString(bytes.size * 2) {
        val alphabet = "0123456789abcdef"
        bytes.forEach { byte ->
            val value = byte.toInt() and 0xff
            append(alphabet[value ushr 4])
            append(alphabet[value and 0x0f])
        }
    }
}
