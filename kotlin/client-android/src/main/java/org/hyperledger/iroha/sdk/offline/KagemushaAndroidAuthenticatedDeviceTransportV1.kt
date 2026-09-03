// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

/**
 * Binds the Android lifecycle bridge admitted from JNI or OMAPI to the authenticated wallet
 * provider transport.
 *
 * Construction requires the exact KAGEMUSHA V1 capability frame. Every successful response has already
 * passed the native P-256 response-authenticator verifier inside [KagemushaDeviceLifecycleBridgeV1]
 * before this adapter exposes its canonical bytes.
 */
class KagemushaAndroidAuthenticatedDeviceTransportV1(
    private val bridge: KagemushaDeviceLifecycleBridgeV1,
) : KagemushaNativeAuthenticatedDeviceTransportV1 {
    private val acceptedCapabilities = checkNotNull(bridge.capabilities()) {
        "Offline secure-device transport is unavailable on this Android device"
    }

    init {
        check(bridge.availability == KagemushaDeviceLifecycleBridgeV1.Availability.AVAILABLE) {
            "Offline secure-device transport is unavailable on this Android device"
        }
    }

    override fun hardwarePolicyId(): ByteArray = acceptedCapabilities.hardwarePolicyId()

    override fun qualificationReportDigest(): ByteArray =
        acceptedCapabilities.qualificationReportDigest()

    override fun executeAndVerify(
        operation: Int,
        requestId: ByteArray,
        canonicalCommand: ByteArray,
        acceptedDevicePublicKey: ByteArray?,
    ): KagemushaAuthenticatedDeviceResponseV1 {
        val lifecycleOperation = KagemushaDeviceLifecycleBridgeV1.Operation.values()
            .singleOrNull { candidate -> candidate.code == operation }
            ?: throw IllegalArgumentException("operation is outside the frozen KAGEMUSHA V1 inventory")
        val result = bridge.executeAuthenticated(
            lifecycleOperation,
            requestId,
            canonicalCommand,
            acceptedDevicePublicKey,
        )
        check(result.operation == lifecycleOperation) {
            "Offline secure-device response substituted its operation"
        }
        val status = KagemushaAuthenticatedDeviceStatusV1.values()
            .singleOrNull { candidate -> candidate.code == result.status.code }
            ?: throw IllegalStateException("Offline secure-device response used an unknown status")
        return KagemushaAuthenticatedDeviceResponseV1(
            operation,
            status,
            result.payload(),
            result.authenticator(),
        )
    }
}
