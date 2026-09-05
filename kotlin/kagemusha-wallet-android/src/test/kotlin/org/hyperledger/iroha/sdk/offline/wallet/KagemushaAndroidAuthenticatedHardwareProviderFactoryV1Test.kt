// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import java.lang.reflect.Proxy
import java.util.ServiceLoader
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.offline.KagemushaAuthenticatedHardwareProviderV1
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1
import org.hyperledger.iroha.sdk.offline.KagemushaNativeCoreCoordinatorFactoryV1
import org.hyperledger.iroha.sdk.offline.KagemushaNativeCoreCoordinatorV1
import org.junit.jupiter.api.Test

class KagemushaAndroidAuthenticatedHardwareProviderFactoryV1Test {
    @Test
    fun `exactly one native Core factory creates the authenticated provider`() {
        var creations = 0
        val core = coordinatorProxy()
        val factory = factoryWith(
            KagemushaNativeCoreCoordinatorFactoryV1 {
                creations += 1
                core
            },
        )

        val provider = factory.open(availableBridge())

        assertIs<KagemushaAuthenticatedHardwareProviderV1>(provider)
        assertEquals(1, creations)
    }

    @Test
    fun `missing or multiple native Core factories fail closed before creating a coordinator`() {
        val missing = assertFailsWith<IllegalStateException> {
            factoryWith().open(availableBridge())
        }
        assertTrue(missing.message!!.contains("none was installed"))

        var creations = 0
        fun candidate() = KagemushaNativeCoreCoordinatorFactoryV1 {
            creations += 1
            coordinatorProxy()
        }
        val multiple = assertFailsWith<IllegalStateException> {
            factoryWith(candidate(), candidate()).open(availableBridge())
        }
        assertTrue(multiple.message!!.contains("multiple were installed"))
        assertEquals(0, creations)
    }

    @Test
    fun `unavailable bridge fails before loading native Core services`() {
        var discoveryCalls = 0
        val factory = KagemushaAndroidAuthenticatedHardwareProviderFactoryV1 {
            discoveryCalls += 1
            emptyList<KagemushaNativeCoreCoordinatorFactoryV1>().iterator()
        }

        assertFailsWith<IllegalStateException> {
            factory.open(KagemushaDeviceLifecycleBridgeV1.onlineOnly())
        }
        assertEquals(0, discoveryCalls)
    }

    @Test
    fun `coordinator construction failure cannot expose a provider`() {
        val factory = factoryWith(
            KagemushaNativeCoreCoordinatorFactoryV1 {
                throw IllegalArgumentException("qualified runtime rejected startup")
            },
        )

        val failure = assertFailsWith<IllegalStateException> {
            factory.open(availableBridge())
        }

        assertTrue(failure.message!!.contains("could not be created"))
        assertIs<IllegalArgumentException>(failure.cause)
    }

    @Test
    fun `generic Android factory is registered as the sole SDK service provider`() {
        val providers = ServiceLoader.load(
            KagemushaAndroidHardwareProviderFactoryV1::class.java,
            KagemushaAndroidAuthenticatedHardwareProviderFactoryV1::class.java.classLoader,
        ).iterator().asSequence().map { it.javaClass.name }.toList()

        assertEquals(
            listOf(KagemushaAndroidAuthenticatedHardwareProviderFactoryV1::class.java.name),
            providers,
        )
    }

    @Test
    fun `stock SDK installs no native Core coordinator factory`() {
        val providers = ServiceLoader.load(
            KagemushaNativeCoreCoordinatorFactoryV1::class.java,
            KagemushaNativeCoreCoordinatorFactoryV1::class.java.classLoader,
        ).iterator().asSequence().take(1).toList()

        assertTrue(providers.isEmpty())
        val failure = assertFailsWith<IllegalStateException> {
            KagemushaAndroidAuthenticatedHardwareProviderFactoryV1().open(availableBridge())
        }
        assertTrue(failure.message!!.contains("none was installed"))
    }

    private fun factoryWith(
        vararg factories: KagemushaNativeCoreCoordinatorFactoryV1,
    ): KagemushaAndroidAuthenticatedHardwareProviderFactoryV1 =
        KagemushaAndroidAuthenticatedHardwareProviderFactoryV1 {
            factories.iterator()
        }

    private fun coordinatorProxy(): KagemushaNativeCoreCoordinatorV1 =
        Proxy.newProxyInstance(
            KagemushaNativeCoreCoordinatorV1::class.java.classLoader,
            arrayOf(KagemushaNativeCoreCoordinatorV1::class.java),
        ) { _, method, _ ->
            throw AssertionError("coordinator method ${method.name} must not run while opening")
        } as KagemushaNativeCoreCoordinatorV1

    private fun availableBridge(): KagemushaDeviceLifecycleBridgeV1 {
        val bridgeClass = KagemushaDeviceLifecycleBridgeV1::class.java
        val endpointClass = Class.forName("${bridgeClass.name}\u0024Endpoint")
        val capabilitiesClass = Class.forName("${bridgeClass.name}\u0024Capabilities")
        val endpoint = Proxy.newProxyInstance(
            endpointClass.classLoader,
            arrayOf(endpointClass),
        ) { _, method, _ ->
            throw AssertionError("endpoint method ${method.name} must not run while opening")
        }
        val capabilities = capabilitiesClass.declaredConstructors
            .single { it.parameterCount == 2 }
            .apply { isAccessible = true }
            .newInstance(ByteArray(32) { 0x21 }, ByteArray(32) { 0x32 })
        return bridgeClass.declaredConstructors
            .single { it.parameterCount == 2 }
            .apply { isAccessible = true }
            .newInstance(endpoint, capabilities) as KagemushaDeviceLifecycleBridgeV1
    }
}
