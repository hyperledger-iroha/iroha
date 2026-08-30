package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver.OfflineDeviceEligibilityRequestV1
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver.OfflineDeviceEligibilityResponseV1
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver.OfflineDeviceFinalityTrustAnchorV1

class OfflineDeviceEligibilityV1Test {
    private fun be16(value: Int): ByteArray = ByteBuffer.allocate(2).putShort(value.toShort()).array()
    private fun be32(value: Int): ByteArray = ByteBuffer.allocate(4).putInt(value).array()
    private fun be64(value: Long): ByteArray = ByteBuffer.allocate(8).putLong(value).array()

    private fun eligibleProjection(registrationHash: ByteArray): ByteArray {
        val issuer = byteArrayOf(0x11, 0x12)
        val credential = byteArrayOf(0x21, 0x22, 0x23)
        val policy = byteArrayOf(0x31, 0x32)
        val claims = listOf(
            "account@test".toByteArray(),
            "device-1".toByteArray(),
            "attestation-key-1".toByteArray(),
            byteArrayOf(0x04) + ByteArray(64) { 0x41 },
            byteArrayOf(0x04) + ByteArray(64) { 0x51 },
        )
        val out = ArrayList<Byte>()
        fun add(bytes: ByteArray) { out.addAll(bytes.toList()) }
        add(byteArrayOf(0x49, 0x44, 0x45, 0x52, 0x53, 0x50, 0x31, 0))
        add(byteArrayOf(0, 0, 1, 0))
        add(be64(42))
        add(registrationHash)
        add(ByteArray(32) { 0x43 })
        add(ByteArray(32) { 0x45 })
        add(be16(0))
        add(byteArrayOf(0, 0))
        add(be32(0))
        add(be32(issuer.size))
        add(be32(credential.size))
        add(be32(policy.size))
        add(be64(7))
        add(ByteArray(32) { 0x47 })
        add(be64(2_000_000))
        add(be64(44))
        add(ByteArray(32) { 0x49 })
        add(be64(1_000_000))
        add(ByteArray(32) { 0x4b })
        add(be64(1_100_000))
        add(be64(1_200_000))
        claims.forEach { add(be16(it.size)) }
        add(byteArrayOf(0, 0))
        add(be32(claims.sumOf(ByteArray::size)))
        add(issuer)
        add(credential)
        add(policy)
        claims.forEach(::add)
        return out.toByteArray()
    }

    @Test
    fun typedProjectionExposesOnlyVerifiedPublicArchivesAndClaims() {
        val network = NetworkId.fromBytes(ByteArray(32) { 0x11 })
        val trust = OfflineDeviceFinalityTrustAnchorV1(network, ByteArray(32) { 0x23 })
        val registration = ByteArray(32) { 0x41 }
        val response = OfflineDeviceEligibilityResponseV1(
            eligibleProjection(registration),
            byteArrayOf(0x61),
            registration,
            trust,
        )
        assertEquals(
            KagemushaRecursiveSpendProver.OfflineDeviceEligibilityOutcomeV1.ELIGIBLE,
            response.decision.outcome,
        )
        assertNotNull(response.credential)
        val claims = assertNotNull(response.credentialClaims)
        assertEquals("account@test", claims.accountId)
        assertEquals("device-1", claims.deviceId)
        assertEquals(65, claims.devicePublicKey().size)
        assertEquals(7, response.policyClaims.policyEpoch)
        assertEquals(44, response.policyClaims.finality.finalizedBlockHeight)
        assertEquals(42, response.admission.admissionHeight)
        assertContentEquals(byteArrayOf(0x61), response.noritoEncoded())

        assertFailsWith<IllegalArgumentException> {
            OfflineDeviceEligibilityResponseV1(
                eligibleProjection(registration),
                byteArrayOf(0x61),
                ByteArray(32) { 0x71 },
                trust,
            )
        }
    }

    @Test
    fun requestAndAuthenticatedToriiSurfaceFailClosed() {
        val request = OfflineDeviceEligibilityRequestV1(
            ByteArray(32) { 0x41 },
            "device-1",
            "attestation-key-1",
            60_000,
        )
        assertEquals(60_000, request.requestedTtlMilliseconds)
        assertFailsWith<IllegalArgumentException> {
            OfflineDeviceEligibilityRequestV1(
                ByteArray(32),
                "device-1",
                "attestation-key-1",
                60_000,
            )
        }

        val root = repositoryRoot()
        val transport = String(
            Files.readAllBytes(
                root.resolve("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/HttpClientTransport.kt"),
            ),
            StandardCharsets.UTF_8,
        )
        val prover = String(
            Files.readAllBytes(
                root.resolve("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"),
            ),
            StandardCharsets.UTF_8,
        )
        assertTrue(transport.contains("postOfflineDeviceEligibilityV1("))
        assertTrue(transport.contains("expectedIssuer:"))
        assertTrue(transport.contains("/v1/offline/device-eligibility"))
        assertTrue(transport.contains("buildExactNoritoPostRequest("))
        assertTrue(prover.contains("nativeVerifyOfflineDeviceEligibilityResponseV1("))
    }

    private fun repositoryRoot(): Path {
        var cursor: Path? = Paths.get("").toAbsolutePath()
        while (cursor != null) {
            if (Files.isRegularFile(cursor.resolve("Cargo.toml")) &&
                Files.isDirectory(cursor.resolve("kotlin/core-jvm"))) {
                return cursor
            }
            cursor = cursor.parent
        }
        error("could not locate repository root")
    }
}
