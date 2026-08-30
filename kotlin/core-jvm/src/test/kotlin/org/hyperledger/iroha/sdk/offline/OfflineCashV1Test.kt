package org.hyperledger.iroha.sdk.offline

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue

class OfflineCashV1Test {
    @Test
    fun `release probe is dual-identity and fail closed without a published release`() {
        val status = OfflineCashReleaseStatusV1.installed()
        assertFalse(status.available)
        assertNull(status.installedReleaseId)
        assertNull(status.installedArtifactManifestSHA256)
        assertTrue(status.blocker?.startsWith("offline-cash-v1-") == true)
    }

    @Test
    fun `public constants freeze the exact first release transport caps`() {
        assertEquals(22, OfflineCashReleaseStatusV1.REQUIRED_NATIVE_BRIDGE_ABI_VERSION)
        assertEquals(768, OfflineCashLimitsV1.PAYMENT_REQUEST_RAW_MAX_BYTES)
        assertEquals(7_936, OfflineCashLimitsV1.PAYMENT_RAW_MAX_BYTES)
        assertEquals(256, OfflineCashLimitsV1.ACKNOWLEDGEMENT_RAW_MAX_BYTES)
        assertEquals(1_029, OfflineCashLimitsV1.PAYMENT_REQUEST_TEXT_MAX_BYTES)
        assertEquals(10_587, OfflineCashLimitsV1.PAYMENT_TEXT_MAX_BYTES)
        assertEquals(347, OfflineCashLimitsV1.ACKNOWLEDGEMENT_TEXT_MAX_BYTES)
        assertEquals(9_211, OfflineCashLimitsV1.RAW_SESSION_MAX_BYTES)
        assertEquals(12_288, OfflineCashLimitsV1.TEXT_SESSION_MAX_BYTES)
        assertEquals(6_400, OfflineCashLimitsV1.PAIRED_PROOF_MAX_BYTES)
        assertEquals(3_200, OfflineCashLimitsV1.PARITY_PROOF_MAX_BYTES)
        assertEquals(384, OfflineCashLimitsV1.ENCRYPTED_CREDIT_MAX_BYTES)
        assertEquals(
            OfflineCashLimitsV1.PAYMENT_REQUEST_RAW_MAX_BYTES,
            OfflineCashPaymentRequestV1.MAX_CANONICAL_BYTES,
        )
        assertEquals(OfflineCashLimitsV1.PAYMENT_RAW_MAX_BYTES, OfflineCashPaymentV1.MAX_CANONICAL_BYTES)
        assertEquals(
            OfflineCashLimitsV1.ACKNOWLEDGEMENT_RAW_MAX_BYTES,
            OfflineCashAcknowledgementV1.MAX_CANONICAL_BYTES,
        )
        assertEquals("kgm2:", OfflineCashPeerAdapterV1.TEXT_PREFIX)
        assertEquals(
            OfflineCashLimitsV1.RAW_SESSION_MAX_BYTES,
            OfflineCashPeerAdapterV1.MAX_RAW_SESSION_BYTES,
        )
        assertEquals(
            OfflineCashLimitsV1.TEXT_SESSION_MAX_BYTES,
            OfflineCashPeerAdapterV1.MAX_TEXT_SESSION_BYTES,
        )
        assertEquals(34, OfflineCashArtifactSetInstallerV1.REQUIRED_ARTIFACT_COUNT)
        val expectedArtifactRoles =
            listOf(
                OfflineCashArtifactRoleV1.PARAMS_EQ,
                OfflineCashArtifactRoleV1.PARAMS_EP,
                OfflineCashArtifactRoleV1.STATE_PK_EQ,
                OfflineCashArtifactRoleV1.STATE_VK_EQ,
                OfflineCashArtifactRoleV1.STATE_PK_EP,
                OfflineCashArtifactRoleV1.STATE_VK_EP,
                OfflineCashArtifactRoleV1.GUARD_USE_PK_EQ,
                OfflineCashArtifactRoleV1.GUARD_USE_VK_EQ,
                OfflineCashArtifactRoleV1.GUARD_USE_PK_EP,
                OfflineCashArtifactRoleV1.GUARD_USE_VK_EP,
                OfflineCashArtifactRoleV1.PLATFORM_BIND_PK_EQ,
                OfflineCashArtifactRoleV1.PLATFORM_BIND_VK_EQ,
                OfflineCashArtifactRoleV1.PLATFORM_BIND_PK_EP,
                OfflineCashArtifactRoleV1.PLATFORM_BIND_VK_EP,
                OfflineCashArtifactRoleV1.ANDROID_KEY_CERT_PK_EQ,
                OfflineCashArtifactRoleV1.ANDROID_KEY_CERT_VK_EQ,
                OfflineCashArtifactRoleV1.ANDROID_KEY_CERT_PK_EP,
                OfflineCashArtifactRoleV1.ANDROID_KEY_CERT_VK_EP,
                OfflineCashArtifactRoleV1.GUARD_BUNDLE_PK_EQ,
                OfflineCashArtifactRoleV1.GUARD_BUNDLE_VK_EQ,
                OfflineCashArtifactRoleV1.GUARD_BUNDLE_PK_EP,
                OfflineCashArtifactRoleV1.GUARD_BUNDLE_VK_EP,
                OfflineCashArtifactRoleV1.P256_V3_PK_EQ,
                OfflineCashArtifactRoleV1.P256_V3_VK_EQ,
                OfflineCashArtifactRoleV1.P256_V3_PK_EP,
                OfflineCashArtifactRoleV1.P256_V3_VK_EP,
                OfflineCashArtifactRoleV1.STATE_LEAF_PK_EQ,
                OfflineCashArtifactRoleV1.STATE_LEAF_VK_EQ,
                OfflineCashArtifactRoleV1.STATE_LEAF_PK_EP,
                OfflineCashArtifactRoleV1.STATE_LEAF_VK_EP,
                OfflineCashArtifactRoleV1.GUARD_BUNDLE_LEAF_PK_EQ,
                OfflineCashArtifactRoleV1.GUARD_BUNDLE_LEAF_VK_EQ,
                OfflineCashArtifactRoleV1.GUARD_BUNDLE_LEAF_PK_EP,
                OfflineCashArtifactRoleV1.GUARD_BUNDLE_LEAF_VK_EP,
            )
        assertEquals(expectedArtifactRoles, OfflineCashArtifactRoleV1.entries)
        assertEquals(
            (0 until OfflineCashArtifactSetInstallerV1.REQUIRED_ARTIFACT_COUNT).toList(),
            OfflineCashArtifactRoleV1.entries.map { it.ordinal },
        )
    }

    @Test
    fun `verification session vocabulary does not claim device commit`() {
        assertEquals(
            listOf(
                "UNAVAILABLE",
                "REQUEST_VERIFIED",
                "PAYMENT_VERIFIED",
                "ACKNOWLEDGEMENT_VERIFIED",
            ),
            OfflineCashVerificationSessionStateV1.entries.map { it.name },
        )
        assertEquals(
            listOf(
                "PAYMENT_VERIFIED",
                "PAYMENT_VERIFICATION_REPLAY",
                "ACKNOWLEDGEMENT_VERIFIED",
                "ACKNOWLEDGEMENT_VERIFICATION_REPLAY",
            ),
            OfflineCashVerificationSessionEventV1.entries.map { it.name },
        )
    }

    @Test
    fun `verification session construction requires exact network and asset context`() {
        val constructor = OfflineCashVerificationSessionV1::class.java.declaredConstructors.single()

        assertEquals(
            listOf(
                OfflineCashPaymentRequestV1::class.java,
                ByteArray::class.java,
                ByteArray::class.java,
                String::class.java,
                String::class.java,
            ),
            constructor.parameterTypes.toList(),
        )
    }

    @Test
    fun `wallet facade has exact stable states and always fails closed`() {
        assertEquals((0..12).toList(), OfflineCashWalletSessionStateV1.entries.map { it.code })
        assertEquals(
            listOf(
                "UNAVAILABLE",
                "SETUP_REQUIRED",
                "EMPTY",
                "TOP_UP_PENDING",
                "AVAILABLE",
                "RECEIVE_REQUEST_READY",
                "SEND_PREPARING",
                "PAYMENT_COMMITTED",
                "AWAITING_ACKNOWLEDGEMENT",
                "RECEIVED",
                "REDEEM_PENDING",
                "RECOVERY_REQUIRED",
                "ERROR",
            ),
            OfflineCashWalletSessionStateV1.entries.map { it.name },
        )
        assertEquals(listOf(0), OfflineCashWalletSessionStatusV1.entries.map { it.code })
        assertEquals((0..8).toList(), OfflineCashWalletSessionActionV1.entries.map { it.code })
        assertEquals(
            listOf(
                "SET_UP",
                "TOP_UP",
                "CREATE_RECEIVE_REQUEST",
                "PREPARE_SEND",
                "COMMIT_PAYMENT",
                "RECORD_ACKNOWLEDGEMENT_EVIDENCE",
                "RECEIVE_PAYMENT",
                "REDEEM",
                "RECOVER",
            ),
            OfflineCashWalletSessionActionV1.entries.map { it.name },
        )
        assertFailsWith<OfflineCashWalletSessionExceptionV1> {
            OfflineCashWalletSessionV1.open()
        }.also { error ->
            assertEquals(OfflineCashWalletSessionErrorV1.UNAVAILABLE, error.reason)
        }

        val session = OfflineCashWalletSessionV1.unavailable()
        assertEquals(OfflineCashWalletSessionStatusV1.UNAVAILABLE, session.status)
        assertEquals(OfflineCashWalletSessionStateV1.UNAVAILABLE, session.state)
        for (action in OfflineCashWalletSessionActionV1.entries) {
            assertFailsWith<OfflineCashWalletSessionExceptionV1> {
                session.attempt(action)
            }.also { error ->
                assertEquals(OfflineCashWalletSessionErrorV1.UNAVAILABLE, error.reason)
            }
            assertEquals(OfflineCashWalletSessionStateV1.UNAVAILABLE, session.state)
        }
    }
}
