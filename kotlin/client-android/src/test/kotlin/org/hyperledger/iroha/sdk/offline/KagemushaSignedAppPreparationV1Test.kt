// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class KagemushaSignedAppPreparationV1Test {
    private val policy = byteArrayOf(1, 2, 3)
    private val digest = MessageDigest.getInstance("SHA-256").digest(policy)
    // Scripted JNI transports inspect copying and rejection; canonical I105 is checked in Rust.
    private val account = "mock-account-for-transport-test"
    private val selection = KagemushaNativeEnrollmentPhasesV1.Selection(
        account, ByteArray(32) { 9 }, ByteArray(32) { 1 }, ByteArray(32) { 2 },
        ByteArray(32) { 3 }, ByteArray(32) { 4 }, ByteArray(32) { 5 },
        byteArrayOf(1, 0, 0, 0, 0, 0, 0, 0),
    )

    @Test
    fun `verified nonce is tied to the signed frame and caller inputs are detached`() {
        val endpoint = Endpoint()
        val verifier = KagemushaSignedAppPreparationV1.openEndpoint(policy, digest, endpoint)
        policy.fill(0)
        digest.fill(0)
        val token = ByteArray(273)
        token[0] = 1
        token.fill(6, 49, 81)
        val nonce = verifier.verifyForKeyMint(selection, token, 1_000)
        assertContentEquals(ByteArray(32) { 6 }, nonce)
        nonce.fill(0)
        assertContentEquals(ByteArray(32) { 6 }, verifier.verifyForKeyMint(selection, token, 1_000))
        assertContentEquals(ByteArray(273).also { it[0] = 1; it.fill(6, 49, 81) }, token)
        assertContentEquals(byteArrayOf(1, 2, 3), endpoint.seenPolicy)
        assertContentEquals(selection.clientNonce(), endpoint.seenClientNonce)
        assertContentEquals(selection.releaseId(), endpoint.seenReleaseId)
        assertContentEquals(selection.profileId(), endpoint.seenProfileId)
        assertContentEquals(selection.laneId(), endpoint.seenLaneId)
        assertEquals(account, endpoint.seenAccount.toString(Charsets.UTF_8))
        assertEquals(1_000, endpoint.seenTime)
    }

    @Test
    fun `policy pin and JNI contract fail before key creation`() {
        val endpoint = Endpoint()
        assertFailsWith<IllegalArgumentException> {
            KagemushaSignedAppPreparationV1.openEndpoint(policy, ByteArray(32), endpoint)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaSignedAppPreparationV1.openEndpoint(policy, ByteArray(32) { 8 }, endpoint)
        }
        assertEquals(0, endpoint.contractCalls)
        endpoint.contractWords[0] = 2
        assertFailsWith<IllegalStateException> {
            KagemushaSignedAppPreparationV1.openEndpoint(policy, digest, endpoint)
        }
        assertEquals(1, endpoint.contractCalls)
    }

    @Test
    fun `malformed request and substituted JNI nonce fail closed`() {
        val endpoint = Endpoint()
        val verifier = KagemushaSignedAppPreparationV1.openEndpoint(policy, digest, endpoint)
        val token = ByteArray(273).also { it.fill(6, 49, 81) }
        assertFailsWith<IllegalArgumentException> { verifier.verifyForKeyMint(selection, byteArrayOf(1), 1) }
        assertFailsWith<IllegalArgumentException> { verifier.verifyForKeyMint(selection, token, 0) }
        assertEquals(0, endpoint.verifyCalls)
        endpoint.returned = ByteArray(32) { 7 }
        assertFailsWith<IllegalStateException> { verifier.verifyForKeyMint(selection, token, 1) }
        endpoint.returned = ByteArray(32)
        assertFailsWith<IllegalStateException> { verifier.verifyForKeyMint(selection, token, 1) }
        endpoint.returned = byteArrayOf(6)
        assertFailsWith<IllegalStateException> { verifier.verifyForKeyMint(selection, token, 1) }
        endpoint.returned = null
        assertFailsWith<IllegalStateException> { verifier.verifyForKeyMint(selection, token, 1) }
    }

    private class Endpoint : KagemushaSignedAppPreparationEndpointV1 {
        val contractWords = intArrayOf(1, 273, 8192, 512)
        var contractCalls = 0
        var verifyCalls = 0
        var returned: ByteArray? = ByteArray(32) { 6 }
        var seenPolicy = byteArrayOf()
        var seenClientNonce = byteArrayOf()
        var seenReleaseId = byteArrayOf()
        var seenProfileId = byteArrayOf()
        var seenLaneId = byteArrayOf()
        var seenAccount = byteArrayOf()
        var seenTime = 0L
        override fun contract(): IntArray? { contractCalls++; return contractWords.copyOf() }
        override fun verify(
            signedPreparation: ByteArray,
            canonicalPolicy: ByteArray,
            pinnedPolicySha256: ByteArray,
            accountI105: ByteArray,
            clientNonce: ByteArray,
            releaseId: ByteArray,
            profileId: ByteArray,
            laneId: ByteArray,
            trustedNowMs: Long,
        ): ByteArray? {
            verifyCalls++
            seenPolicy = canonicalPolicy.copyOf()
            seenClientNonce = clientNonce.copyOf()
            seenReleaseId = releaseId.copyOf()
            seenProfileId = profileId.copyOf()
            seenLaneId = laneId.copyOf()
            seenAccount = accountI105.copyOf()
            seenTime = trustedNowMs
            signedPreparation.fill(0)
            canonicalPolicy.fill(0)
            pinnedPolicySha256.fill(0)
            clientNonce.fill(0)
            return returned
        }
    }
}
