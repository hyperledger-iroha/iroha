// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareProviderV1
import org.hyperledger.iroha.sdk.offline.KagemushaWalletV1

/**
 * OEM boundary that binds the canonical wallet provider to an audited Android device service.
 *
 * The factory must use the supplied bridge for every authoritative operation. It must not wrap
 * AndroidKeyStore, application files, preferences, a database, or another software fallback.
 */
fun interface KagemushaAndroidHardwareProviderFactoryV1 {
    /** Open the OEM provider backed exclusively by [bridge]. */
    fun open(bridge: KagemushaDeviceLifecycleBridgeV1): KagemushaHardwareProviderV1

    /**
     * Return the already admitted lifecycle bridge used by this provider.
     *
     * OEM integrations may override this with a cached OMAPI bridge opened during application
     * startup. The default preserves JNI discovery for existing integrations.
     */
    fun deviceLifecycleBridge(): KagemushaDeviceLifecycleBridgeV1 =
        KagemushaDeviceLifecycleBridgeV1.production()
}

/** Fail-closed Android entry point for aggregate-balance KAGEMUSHA V1. */
object KagemushaAndroidWalletV1 {
    /** Open a wallet around an already provisioned, completely qualified hardware provider. */
    @JvmStatic
    fun open(provider: KagemushaHardwareProviderV1): KagemushaWalletV1 =
        KagemushaWalletV1.open(provider)

    /**
     * Discover the audited native device service and bind it to an OEM provider adapter.
     *
     * Stock Android devices currently return online-only because KeyMint and StrongBox do not
     * expose the atomic journal/counter/inbox/outbox primitive required by this protocol. Missing
     * native support is an error, never permission to substitute a software wallet.
     */
    @JvmStatic
    fun openProduction(
        factory: KagemushaAndroidHardwareProviderFactoryV1,
    ): KagemushaWalletV1 = openBridge(factory.deviceLifecycleBridge(), factory)

    internal fun openBridge(
        bridge: KagemushaDeviceLifecycleBridgeV1,
        factory: KagemushaAndroidHardwareProviderFactoryV1,
    ): KagemushaWalletV1 {
        val bridgeCapabilities = bridge.capabilities()
            ?: throw IllegalStateException(
                "KAGEMUSHA V1 is online-only: no qualified non-forking Android hardware service",
            )
        val provider = factory.open(bridge)
        val qualification = provider.qualification()
        qualification.requireProductionReady()
        require(
            qualification.profile.hardwareProfileId()
                .contentEquals(bridgeCapabilities.hardwarePolicyId()),
        ) { "OEM provider hardware policy does not match the native device service" }
        require(
            qualification.profile.qualificationReportDigest()
                .contentEquals(bridgeCapabilities.attestationDigest()),
        ) { "OEM provider attestation does not match the native device service" }
        return KagemushaWalletV1.open(provider)
    }
}
