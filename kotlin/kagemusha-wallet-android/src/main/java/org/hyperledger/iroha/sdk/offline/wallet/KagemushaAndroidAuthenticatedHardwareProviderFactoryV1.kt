// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import java.util.ServiceConfigurationError
import java.util.ServiceLoader
import org.hyperledger.iroha.sdk.offline.KagemushaAndroidAuthenticatedDeviceTransportV1
import org.hyperledger.iroha.sdk.offline.KagemushaAuthenticatedHardwareProviderV1
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareProviderV1
import org.hyperledger.iroha.sdk.offline.KagemushaNativeCoreCoordinatorFactoryV1

/**
 * Generic Android adapter from an admitted secure-device bridge to the authenticated provider.
 *
 * The SDK registers this class as its hardware-provider factory. Opening it succeeds only when a
 * qualified runtime package registers exactly one [KagemushaNativeCoreCoordinatorFactoryV1]. The
 * SDK contains no coordinator implementation and never substitutes software state.
 */
class KagemushaAndroidAuthenticatedHardwareProviderFactoryV1 internal constructor(
    private val loadCoordinatorFactories:
        () -> Iterator<KagemushaNativeCoreCoordinatorFactoryV1>,
) : KagemushaAndroidHardwareProviderFactoryV1 {
    /** Public zero-argument constructor required by [ServiceLoader]. */
    constructor() : this({
        ServiceLoader.load(
            KagemushaNativeCoreCoordinatorFactoryV1::class.java,
            KagemushaNativeCoreCoordinatorFactoryV1::class.java.classLoader,
        ).iterator()
    })

    override fun open(bridge: KagemushaDeviceLifecycleBridgeV1): KagemushaHardwareProviderV1 {
        val transport = KagemushaAndroidAuthenticatedDeviceTransportV1(bridge)
        val coordinatorFactory = loadExactlyOneCoordinatorFactory()
        val coordinator = try {
            checkNotNull(coordinatorFactory.create()) {
                "Offline native-Core coordinator factory returned no coordinator"
            }
        } catch (error: RuntimeException) {
            throw IllegalStateException(
                "Offline native-Core coordinator could not be created",
                error,
            )
        } catch (error: LinkageError) {
            throw IllegalStateException(
                "Offline native-Core coordinator could not be linked",
                error,
            )
        }
        return KagemushaAuthenticatedHardwareProviderV1(transport, coordinator)
    }

    private fun loadExactlyOneCoordinatorFactory(): KagemushaNativeCoreCoordinatorFactoryV1 {
        try {
            val candidates = loadCoordinatorFactories()
            check(candidates.hasNext()) {
                "Exactly one Offline native-Core coordinator factory is required; none was installed"
            }
            val selected = candidates.next()
            check(!candidates.hasNext()) {
                "Exactly one Offline native-Core coordinator factory is required; multiple were installed"
            }
            return selected
        } catch (error: ServiceConfigurationError) {
            throw IllegalStateException(
                "Offline native-Core coordinator factory discovery failed",
                error,
            )
        } catch (error: LinkageError) {
            throw IllegalStateException(
                "Offline native-Core coordinator factory could not be linked",
                error,
            )
        }
    }
}
