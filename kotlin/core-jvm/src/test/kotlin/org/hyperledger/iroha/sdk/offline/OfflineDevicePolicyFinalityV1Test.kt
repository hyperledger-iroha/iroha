package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.nio.charset.StandardCharsets
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver.OfflineDeviceFinalityTrustAnchorV1
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver.OfflineDevicePolicyCheckpointV1
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver.OfflineDevicePolicyVerifiedPageV1

class OfflineDevicePolicyFinalityV1Test {
    private fun projection(
        moreAvailable: Boolean,
        policy: ByteArray = ByteArray(0),
    ): ByteArray {
        val result = ArrayList<Byte>()
        result.addAll(byteArrayOf(0x49, 0x44, 0x50, 0x50, 0x56, 0x31, 0, 0).toList())
        result.addAll(byteArrayOf(0, 0, 0, 0, 0, 0, 0, 17).toList())
        result.addAll(ByteArray(32) { 0x23 }.toList())
        result.add((if (moreAvailable) 1 else 0).toByte())
        result.addAll(byteArrayOf(0, 0, 0).toList())
        val length = policy.size
        result.add(((length ushr 24) and 0xff).toByte())
        result.add(((length ushr 16) and 0xff).toByte())
        result.add(((length ushr 8) and 0xff).toByte())
        result.add((length and 0xff).toByte())
        result.addAll(policy.toList())
        return result.toByteArray()
    }

    @Test
    fun trustAnchorRequiresExactMarkedContextAndOwnsItsBytes() {
        val network = NetworkId.fromBytes(ByteArray(32) { 0x11 })
        val context = ByteArray(32) { 0x23 }
        val anchor = OfflineDeviceFinalityTrustAnchorV1(network, context)
        context[0] = 0
        assertContentEquals(ByteArray(32) { 0x23 }, anchor.trustedHeightContextId())

        assertFailsWith<IllegalArgumentException> {
            OfflineDeviceFinalityTrustAnchorV1(network, ByteArray(31) { 0x23 })
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineDeviceFinalityTrustAnchorV1(network, ByteArray(32) { 0x22 })
        }
    }

    @Test
    fun durableCheckpointAndVerifiedProjectionFailClosed() {
        val network = NetworkId.fromBytes(ByteArray(32) { 0x11 })
        val expectedCheckpoint = OfflineDevicePolicyCheckpointV1(
            network,
            17,
            ByteArray(32) { 0x23 },
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineDevicePolicyCheckpointV1(network, 0, ByteArray(32) { 0x23 })
        }
        val intermediate = OfflineDevicePolicyVerifiedPageV1(
            projection(moreAvailable = true),
            network,
        )
        assertEquals(expectedCheckpoint, intermediate.evaluatedCheckpoint)
        assertTrue(intermediate.moreAvailable)
        assertNull(intermediate.terminalPolicyView)

        val policy = byteArrayOf(0x21, 0x22, 0x23)
        val terminal = OfflineDevicePolicyVerifiedPageV1(
            projection(moreAvailable = false, policy = policy),
            network,
        )
        assertFalse(terminal.moreAvailable)
        assertContentEquals(policy, terminal.terminalPolicyView!!.noritoEncoded())

        val badMagic = projection(moreAvailable = true).also { it[0] = 0 }
        assertFailsWith<IllegalArgumentException> {
            OfflineDevicePolicyVerifiedPageV1(badMagic, network)
        }
        val badReserved = projection(moreAvailable = true).also { it[49] = 1 }
        assertFailsWith<IllegalArgumentException> {
            OfflineDevicePolicyVerifiedPageV1(badReserved, network)
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineDevicePolicyVerifiedPageV1(projection(moreAvailable = false), network)
        }
        val badLength = projection(moreAvailable = false, policy = policy).also { it[55] = 4 }
        assertFailsWith<IllegalArgumentException> {
            OfflineDevicePolicyVerifiedPageV1(badLength, network)
        }
    }

    @Test
    fun finalizedPolicyClaimsProjectionIsClosedDefensiveAndExact() {
        val projection = ByteBuffer.allocate(136)
            .order(ByteOrder.BIG_ENDIAN)
            .put(byteArrayOf(0x49, 0x44, 0x50, 0x56, 0x43, 0x4c, 0x31, 0))
            .putLong(7)
            .put(ByteArray(32) { 0x31 })
            .putLong(2_000)
            .putLong(83)
            .put(ByteArray(32) { 0x41 })
            .putLong(1_000)
            .put(ByteArray(32) { 0x51 })
            .array()
        val claims =
            KagemushaRecursiveSpendProver.OfflineDeviceAttestationPolicyViewClaimsV1(
                projection,
            )
        projection.fill(0)
        assertEquals(7, claims.policyEpoch)
        assertEquals(2_000, claims.freshnessDeadlineMilliseconds)
        assertEquals(83, claims.finalizedBlockHeight)
        assertEquals(1_000, claims.finalizedBlockTimestampMilliseconds)
        assertContentEquals(ByteArray(32) { 0x31 }, claims.policyHash())
        assertContentEquals(ByteArray(32) { 0x41 }, claims.finalizedBlockHash())
        assertContentEquals(ByteArray(32) { 0x51 }, claims.finalityEvidenceHash())

        val badMagic = policyClaimsProjection().also { it[0] = 0 }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.OfflineDeviceAttestationPolicyViewClaimsV1(badMagic)
        }
        val stale = policyClaimsProjection().also {
            ByteBuffer.wrap(it).order(ByteOrder.BIG_ENDIAN).putLong(48, 999)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.OfflineDeviceAttestationPolicyViewClaimsV1(stale)
        }
        val oversizedHeight = policyClaimsProjection().also { it[56] = 0x80.toByte() }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.OfflineDeviceAttestationPolicyViewClaimsV1(
                oversizedHeight,
            )
        }
    }

    @Test
    fun productionSurfaceUsesAuthenticatedQueryAndFinalizedNativeGate() {
        val root = repositoryRoot()
        val transport = String(Files.readAllBytes(root.resolve(
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/HttpClientTransport.kt",
        )), StandardCharsets.UTF_8)
        val prover = String(Files.readAllBytes(root.resolve(
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
        )), StandardCharsets.UTF_8)
        val nativeBridge = String(Files.readAllBytes(root.resolve(
            "crates/connect_norito_bridge/src/lib.rs",
        )), StandardCharsets.UTF_8)
        val nativeJni = String(Files.readAllBytes(root.resolve(
            "crates/connect_norito_bridge/src/platform_jni/part_3.rs",
        )), StandardCharsets.UTF_8)
        assertTrue(transport.contains("/v1/offline/device-attestation-policy"))
        assertTrue(transport.contains("/v1/offline/device-attestation-policy/proof"))
        assertTrue(transport.contains("buildExactNoritoGetRequest("))
        assertTrue(transport.contains("buildExactNoritoPostRequest("))
        assertTrue(transport.contains("MAX_DEVICE_POLICY_PROOF_PAGE_ARCHIVE_BYTES_V1"))
        assertTrue(transport.contains("verifyOfflineDevicePolicyProofPageV1("))
        assertTrue(transport.contains("verifyDeviceAttestationPolicyViewV1("))
        assertTrue(prover.contains("nativeValidateEligibilityPaymentFirstDeliveryFinalizedV1("))
        assertTrue(prover.contains("nativeProjectOfflineDeviceAttestationPolicyViewClaimsV1("))
        assertTrue(prover.contains("createDrainOnlySameAccountRedemptionAuthorizationV1("))
        assertTrue(prover.contains("nativeFinalizeDrainOnlyRedemptionAuthorizationV1("))
        assertTrue(prover.contains("buildDrainOnlyRedeemInstructionV4("))
        assertTrue(prover.contains("nativeBuildDrainOnlyRedeemInstructionV4("))
        assertTrue(prover.contains("carries no eligibility credential, device registration hash"))
        assertTrue(prover.contains("policy view must be returned by the native finalized-policy verifier"))
        assertTrue(prover.contains("verificationTrustAnchor"))
        assertTrue(nativeBridge.contains("OFFLINE_DEVICE_POLICY_VIEW_CLAIMS_MAGIC_V1"))
        assertTrue(nativeBridge.contains("verify_offline_device_policy_view_finality_v1("))
        assertTrue(nativeBridge.contains("policy_view.finality.finalized_block_hash.as_ref()"))
        assertTrue(nativeBridge.contains("policy_view.finality.finality_evidence_hash.as_ref()"))
        assertTrue(nativeJni.contains("nativeProjectOfflineDeviceAttestationPolicyViewClaimsV1"))
        assertTrue(nativeJni.contains("nativeFinalizeDrainOnlyRedemptionAuthorizationV1"))
        assertTrue(nativeJni.contains("nativeBuildDrainOnlyRedeemInstructionV4"))

        val nativeBuilder = String(Files.readAllBytes(root.resolve(
            "crates/connect_norito_bridge/src/platform_jni/part_2.rs",
        )), StandardCharsets.UTF_8).substringAfter(
            "java_native_kagemusha_build_drain_only_redeem_instruction_v4",
        ).substringBefore("java_native_kagemusha_finalize_hardware_authorization_v2")
        assertTrue(nativeBuilder.contains("request.recipient != authority"))
        assertTrue(nativeBuilder.contains("request.authorization.authority != authority"))
        assertTrue(nativeBuilder.contains("AccountAuthorityDrainOnly"))
        assertTrue(nativeBuilder.contains("RedeemKagemushaRecursiveV4::new(request)"))
        assertTrue(nativeBuilder.contains("native registry returned another redemption wire id"))
    }

    private fun policyClaimsProjection(): ByteArray = ByteBuffer.allocate(136)
        .order(ByteOrder.BIG_ENDIAN)
        .put(byteArrayOf(0x49, 0x44, 0x50, 0x56, 0x43, 0x4c, 0x31, 0))
        .putLong(1)
        .put(ByteArray(32) { 0x31 })
        .putLong(2_000)
        .putLong(1)
        .put(ByteArray(32) { 0x41 })
        .putLong(1_000)
        .put(ByteArray(32) { 0x51 })
        .array()

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
