// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import java.util.EnumSet
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.offline.KagemushaDevicePublicKeyV1
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceSignatureV1
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareCapabilityV1
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareCredentialV1
import org.hyperledger.iroha.sdk.offline.KagemushaHardwarePlatformClassV1
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareProfileV1
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareQualificationV1
import org.junit.jupiter.api.Test

/** Public correlation checks only; these synthetic records never qualify a native provider. */
class KagemushaAndroidNativePolicyBindingV1Test {
    @Test
    fun `matching policy digest accepts a distinct hardware profile identity`() {
        val qualification = qualification()
        assertFalse(qualification.profile.hardwareProfileId().contentEquals(qualification.hardwarePolicyDigest()))

        KagemushaAndroidWalletV1.requireNativeQualificationBinding(
            qualification,
            qualification.hardwarePolicyDigest(),
            qualification.profile.qualificationReportDigest(),
        )
    }

    @Test
    fun `profile identity cannot substitute for the native policy digest`() {
        val qualification = qualification()
        for (wrongPolicy in listOf(qualification.profile.hardwareProfileId(), digest(99))) {
            val error = assertFailsWith<IllegalArgumentException> {
                KagemushaAndroidWalletV1.requireNativeQualificationBinding(
                    qualification,
                    wrongPolicy,
                    qualification.profile.qualificationReportDigest(),
                )
            }
            assertTrue(error.message!!.contains("hardware policy"))
        }
    }

    @Test
    fun `matching policy cannot substitute the qualification report`() {
        val qualification = qualification()
        val error = assertFailsWith<IllegalArgumentException> {
            KagemushaAndroidWalletV1.requireNativeQualificationBinding(
                qualification,
                qualification.hardwarePolicyDigest(),
                digest(99),
            )
        }
        assertTrue(error.message!!.contains("attestation"))
    }

    @Test
    fun `matching digests cannot admit a software fallback capability set`() {
        val capabilities = EnumSet.allOf(KagemushaHardwareCapabilityV1::class.java)
        capabilities.remove(KagemushaHardwareCapabilityV1.NO_SOFTWARE_FALLBACK)
        val qualification = qualification(capabilities)
        val error = assertFailsWith<IllegalArgumentException> {
            KagemushaAndroidWalletV1.requireNativeQualificationBinding(
                qualification,
                qualification.hardwarePolicyDigest(),
                qualification.profile.qualificationReportDigest(),
            )
        }
        assertTrue(error.message!!.contains("complete non-forking hardware capability set"))
    }

    private fun qualification(
        capabilities: Set<KagemushaHardwareCapabilityV1> =
            EnumSet.allOf(KagemushaHardwareCapabilityV1::class.java),
    ): KagemushaHardwareQualificationV1 {
        // The standard P-256 generator and shape-only r=s=1 signature are public test data.
        // No operation-1 response, governance authentication or monetary proof is simulated.
        val deviceKey = KagemushaDevicePublicKeyV1(
            ("04" +
                "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296" +
                "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5")
                .chunked(2).map { it.toInt(16).toByte() }.toByteArray(),
        )
        val signature = KagemushaDeviceSignatureV1(ByteArray(64).also {
            it[31] = 1
            it[63] = 1
        })
        val profile = KagemushaHardwareProfileV1(
            version = 1, protocolVersion = 1,
            hardwareProfileId = digest(1), providerId = digest(3),
            platformClass = KagemushaHardwarePlatformClassV1.ANDROID_OEM_SERVICE,
            productClassDigest = digest(4), firmwarePolicyDigest = digest(5),
            enrollmentAttestationVerifierDigest = digest(6), attestationTrustRootsDigest = digest(7),
            allowedSuiteCommitment = digest(8), policyEpoch = 1,
            governanceCredentialPublicKey = deviceKey, capabilityMask = 0xffff,
            qualificationReportDigest = digest(9), validFromMs = 1, expiresAtMs = 20000,
        )
        val credential = KagemushaHardwareCredentialV1(
            version = 1, credentialId = digest(10), networkId = NetworkId.fromBytes(digest(11)),
            hardwareProfileId = profile.hardwareProfileId(), suiteId = digest(12),
            firmwarePolicyDigest = profile.firmwarePolicyDigest(), policyEpoch = profile.policyEpoch,
            laneCommitment = digest(13), hardwareEpochId = digest(14), hardwareEpochGeneration = 1,
            devicePublicKey = deviceKey, deviceKeyReference = digest(15),
            issuedAtMs = 10, expiresAtMs = 19000, governanceSignature = signature,
        )
        return KagemushaHardwareQualificationV1(
            protocolVersion = 1, profile = profile, credential = credential,
            releaseId = digest(16), hardwarePolicyDigest = digest(2),
            coreAuthorizationKeyReference = digest(17), capabilities = capabilities,
        )
    }

    private fun digest(value: Int): ByteArray = ByteArray(32) { value.toByte() }
}
