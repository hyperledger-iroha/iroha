// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.crypto.keystore

import java.security.MessageDigest
import org.hyperledger.iroha.sdk.offline.KagemushaPlatformAttestationOriginalV1
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull

/** Data-only synthetic DER carriers; no certificate, platform, native or issuer authority. */
class KagemushaAndroidHardwareAppKeyEvidenceOriginalV1Test {
    @Test fun originalContainerAndDigestPreserveEveryOrderedByte() {
        val chain = chain()
        val expected = KagemushaPlatformAttestationOriginalV1.android(chain).canonicalBytes()
        val evidence = newEvidence(chain)
        assertContentEquals(expected, evidence.platformAttestationOriginal())
        assertContentEquals(sha(expected), evidence.platformAttestationOriginalSha256())
        val decoded = assertNotNull(KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(
            evidence.platformAttestationOriginal()).androidCertificateChainDer())
        assertEquals(chain.size, decoded.size)
        chain.indices.forEach { assertContentEquals(chain[it], decoded[it]) }
        val reordered = newEvidence(chain.reversed())
        assertFalse(evidence.platformAttestationOriginalSha256().contentEquals(reordered.platformAttestationOriginalSha256()))
    }

    @Test fun callerAndReturnedArraysCannotChangeCapturedOriginals() {
        val chain = chain().toMutableList()
        val originalChain = chain.map(ByteArray::copyOf)
        val point = byteArrayOf(4) + ByteArray(64) { 7 }
        val keyId = sha(point)
        val evidence = KagemushaAndroidHardwareAppKeyEvidenceV1(1, keyId, point, chain)
        val expected = evidence.platformAttestationOriginal()
        chain.forEach { it.fill(0) }; chain.clear(); point.fill(0); keyId.fill(0)
        evidence.certificateChainDer().forEach { it.fill(0) }
        evidence.platformAttestationOriginal().fill(0)
        evidence.platformAttestationOriginalSha256().fill(0)
        evidence.publicKeySec1().fill(0); evidence.attestedKeyId().fill(0)
        assertContentEquals(expected, evidence.platformAttestationOriginal())
        assertContentEquals(sha(expected), evidence.platformAttestationOriginalSha256())
        originalChain.indices.forEach { assertContentEquals(originalChain[it], evidence.certificateChainDer()[it]) }
        assertContentEquals(byteArrayOf(4) + ByteArray(64) { 7 }, evidence.publicKeySec1())
        assertContentEquals(sha(evidence.publicKeySec1()), evidence.attestedKeyId())
    }

    @Test fun carrierUsesSoleCodecBoundsWithoutInventingAttestationVerdict() {
        assertFailsWith<IllegalArgumentException> { newEvidence(listOf(chain().first())) }
        assertFailsWith<IllegalArgumentException> { newEvidence(List(9) { chain().first() }) }
        assertFailsWith<IllegalArgumentException> { newEvidence(listOf(byteArrayOf(), chain().last())) }
        assertFailsWith<IllegalArgumentException> { newEvidence(listOf(ByteArray(16_385), chain().last())) }
    }

    private fun chain(): List<ByteArray> = listOf(byteArrayOf(0x30, 0x01, 0x01), byteArrayOf(0x30, 0x01, 0x02))
    private fun newEvidence(chain: List<ByteArray>): KagemushaAndroidHardwareAppKeyEvidenceV1 {
        val point = byteArrayOf(4) + ByteArray(64) { 7 }
        return KagemushaAndroidHardwareAppKeyEvidenceV1(1, sha(point), point, chain)
    }
    private fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes)
}
