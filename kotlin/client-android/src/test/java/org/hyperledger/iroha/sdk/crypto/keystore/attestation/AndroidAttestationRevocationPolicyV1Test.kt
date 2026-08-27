package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.math.BigInteger
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class AndroidAttestationRevocationPolicyV1Test {
    @Test
    fun canonicalSnapshotMatchesCrossLanguageVector() {
        val snapshot = AndroidAttestationRevocationTestFixtures.canonicalSnapshot(
            responseDateEpochMillis = 1_761_408_000_000L,
            lastModifiedEpochMillis = 1_761_407_999_000L,
            cacheMaxAgeSeconds = 60L,
            serials = listOf("a", "ff"),
            tbsDigests = listOf(ByteArray(32) { 0x22 }),
        )
        assertTrue(
            AndroidAttestationRevocationTestFixtures.hex(
                AndroidAttestationRevocationTestFixtures.sha256(snapshot),
            ) == AndroidAttestationRevocationTestFixtures.CANONICAL_VECTOR_SHA256,
        )
    }

    @Test
    fun canonicalSnapshotEnforcesFreshnessAndBothDenyLists() {
        val responseDate = 1_761_408_000_000L
        val revokedTbs = ByteArray(32) { 0x22 }
        val snapshot = AndroidAttestationRevocationTestFixtures.canonicalSnapshot(
            responseDateEpochMillis = responseDate,
            lastModifiedEpochMillis = responseDate - 1_000L,
            cacheMaxAgeSeconds = 60L,
            serials = listOf("a", "ff"),
            tbsDigests = listOf(revokedTbs),
        )
        val policy = AndroidAttestationRevocationPolicyV1.fromCanonicalSnapshot(
            snapshot,
            AndroidAttestationRevocationTestFixtures.sha256(snapshot),
        )

        policy.validateAt(responseDate)
        policy.validateAt(responseDate + 59_999L)
        assertFailsWith<AttestationVerificationException> {
            policy.validateAt(responseDate - 1L)
        }
        assertFailsWith<AttestationVerificationException> {
            policy.validateAt(responseDate + 60_000L)
        }
        assertTrue(policy.rejects(BigInteger.TEN, ByteArray(32) { 0x33 }))
        assertTrue(policy.rejects(BigInteger.ONE, revokedTbs))
        assertFalse(policy.rejects(BigInteger.ONE, ByteArray(32) { 0x33 }))
    }

    @Test
    fun unchangedTrustedCommitmentRejectsEverySecurityFieldMutation() {
        val responseDate = 1_761_408_000_000L
        val snapshot = AndroidAttestationRevocationTestFixtures.canonicalSnapshot(
            responseDateEpochMillis = responseDate,
            lastModifiedEpochMillis = responseDate - 1_000L,
            cacheMaxAgeSeconds = 60L,
            serials = listOf("a"),
            tbsDigests = listOf(ByteArray(32) { 0x22 }),
        )
        val trustedCommitment = AndroidAttestationRevocationTestFixtures.sha256(snapshot)
        val replacements = listOf(
            "payload_sha256=" to "payload_sha256=${"12".repeat(32)}",
            "response_date_ms=$responseDate" to "response_date_ms=${responseDate + 1_000L}",
            "last_modified_ms=${responseDate - 1_000L}" to "last_modified_ms=${responseDate - 2_000L}",
            "cache_max_age_seconds=60" to "cache_max_age_seconds=61",
            "serial=a" to "serial=b",
            "tbs_sha256=${"22".repeat(32)}" to "tbs_sha256=${"23".repeat(32)}",
        )
        val text = snapshot.toString(Charsets.US_ASCII)
        for ((from, to) in replacements) {
            val mutated = text.replace(from, to).toByteArray(Charsets.US_ASCII)
            assertFailsWith<IllegalArgumentException>("mutation of $from must fail") {
                AndroidAttestationRevocationPolicyV1.fromCanonicalSnapshot(
                    mutated,
                    trustedCommitment,
                )
            }
        }
    }

    @Test
    fun malformedOrNonCanonicalSnapshotsFailClosed() {
        val responseDate = 1_761_408_000_000L
        val snapshot = AndroidAttestationRevocationTestFixtures.canonicalSnapshot(
            responseDateEpochMillis = responseDate,
            cacheMaxAgeSeconds = 60L,
        )
        assertFailsWith<IllegalArgumentException> {
            AndroidAttestationRevocationPolicyV1.fromCanonicalSnapshot(
                snapshot,
                ByteArray(32) { 0x44 },
            )
        }

        val nonCanonical = snapshot.toString(Charsets.US_ASCII)
            .replace("serial_count=0", "serial_count=01\nserial=0a")
            .toByteArray(Charsets.US_ASCII)
        assertFailsWith<IllegalArgumentException> {
            AndroidAttestationRevocationPolicyV1.fromCanonicalSnapshot(
                nonCanonical,
                AndroidAttestationRevocationTestFixtures.sha256(nonCanonical),
            )
        }
    }
}
