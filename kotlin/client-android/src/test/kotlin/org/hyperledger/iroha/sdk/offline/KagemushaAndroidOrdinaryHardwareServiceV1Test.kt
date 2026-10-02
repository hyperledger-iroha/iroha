package org.hyperledger.iroha.sdk.offline

import android.content.Context
import java.nio.ByteBuffer
import java.util.UUID
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertSame
import kotlin.test.assertTrue
import kotlin.test.fail

/** Scripted strict C21/JNI shapes only. No Native authority or platform key is created. */
class KagemushaAndroidOrdinaryHardwareServiceV1Test {
    @Test fun packagedOrdinaryRouteIsTheActualSharedFactory() {
        val selected = KagemushaAndroidOrdinaryHardwareServiceFactoryV1.discover(checkNotNull(javaClass.classLoader))
        assertSame<Class<*>>(KagemushaAndroidAppOwnedHardwareServiceFactoryV1::class.java, selected.javaClass)
    }
    @Test fun missingOrdinaryRouteHasNoAppletFallback() {
        reject { KagemushaAndroidOrdinaryHardwareServiceFactoryV1.selectOriginalFactory(emptyList<KagemushaAndroidOrdinaryHardwareServiceFactoryV1>().iterator()) }
    }
    @Test fun ambiguousOrdinaryRoutesAreRejected() {
        reject { KagemushaAndroidOrdinaryHardwareServiceFactoryV1.selectOriginalFactory(listOf(KagemushaAndroidAppOwnedHardwareServiceFactoryV1(), KagemushaAndroidAppOwnedHardwareServiceFactoryV1()).iterator()) }
    }
    @Test fun substitutedRouteCannotClaimTheSharedFactory() {
        val substitute = object : KagemushaAndroidOrdinaryHardwareServiceFactoryV1 {
            override fun open(context: Context, coordinator: KagemushaNativeCoreCoordinatorAdapterV1,
                accountId: String, transport: KagemushaOrdinaryIdentityOriginalTransportV1,
                walletSigner: KagemushaOrdinaryWalletAccountSignerV1,
                requireOriginalOwner: () -> Unit): KagemushaAndroidOrdinaryHardwareServiceV1 =
                error("A substitute must never open")
        }
        reject { KagemushaAndroidOrdinaryHardwareServiceFactoryV1.selectOriginalFactory(listOf(substitute).iterator()) }
    }
    @Test fun originalAccountUsesOnlyRealC21ReservationReads() {
        val endpoint = Endpoint(); val owner = native(endpoint)
        KagemushaAndroidOrdinaryHardwareServiceV1.requireOriginalAccount(owner, "original-account") {}
        assertEquals(listOf(21 to 12, 21 to 12), endpoint.calls)
        assertEquals(0, endpoint.closes)
    }
    @Test fun anotherAccountCannotOpenTheOrdinaryRoute() {
        val endpoint = Endpoint()
        reject { KagemushaAndroidOrdinaryHardwareServiceV1.requireOriginalAccount(native(endpoint), "another-account") {} }
        assertEquals(listOf(21 to 12, 21 to 12), endpoint.calls)
    }
    @Test fun revokedOriginalOwnerStopsBeforeNativeReservation() {
        val endpoint = Endpoint()
        reject { KagemushaAndroidOrdinaryHardwareServiceV1.requireOriginalAccount(native(endpoint), "original-account") { error("Original account revoked") } }
        assertTrue(endpoint.calls.isEmpty())
    }
    @Test fun postDispatchRevocationWithholdsSelectedOriginal() {
        val endpoint = Endpoint(); var guards = 0
        reject { KagemushaAndroidOrdinaryHardwareServiceV1.requireOriginalAccount(native(endpoint), "original-account") { guards++; check(guards == 1) } }
        assertEquals(listOf(21 to 12), endpoint.calls)
    }
    @Test fun missingNativeOwnerClosesAndCannotChooseAnotherRoute() {
        val endpoint = Endpoint().apply { unavailable = true }; val owner = native(endpoint)
        reject { KagemushaAndroidOrdinaryHardwareServiceV1.requireOriginalAccount(owner, "original-account") {} }
        reject { KagemushaAndroidOrdinaryHardwareServiceV1.requireOriginalAccount(owner, "original-account") {} }
        assertEquals(1, endpoint.calls.size); assertEquals(1, endpoint.closes)
    }
    @Test fun substitutedRetainedReservationIsRejected() {
        val endpoint = Endpoint().apply { substitute = true }
        reject { KagemushaAndroidOrdinaryHardwareServiceV1.requireOriginalAccount(native(endpoint), "original-account") {} }
        assertEquals(2, endpoint.calls.size); assertEquals(1, endpoint.closes)
    }
    @Test fun emptyAccountCannotDispatchNativeWork() {
        val endpoint = Endpoint()
        reject { KagemushaAndroidOrdinaryHardwareServiceV1.requireOriginalAccount(native(endpoint), " ") {} }
        assertTrue(endpoint.calls.isEmpty())
    }
    private fun native(endpoint: Endpoint) = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/fixture/ordinary-account-service", endpoint)
    private fun reject(block: () -> Unit) {
        try { block() } catch (_: IllegalStateException) { return } catch (_: IllegalArgumentException) { return }
        fail("Expected the route to reject")
    }
    private class Endpoint : KagemushaCoreCoordinatorEndpointV1 {
        val calls = mutableListOf<Pair<Int, Int>>()
        var closes = 0; var unavailable = false; var substitute = false
        val fields = run {
            val nonce = ByteArray(32) { 2 }; val uuid = nonce.copyOf(16)
            uuid[6] = ((uuid[6].toInt() and 0x0f) or 0x40).toByte()
            uuid[8] = ((uuid[8].toInt() and 0x3f) or 0x80).toByte()
            arrayOf(ByteArray(8) { 1 }, "original-account".toByteArray(), nonce,
                ByteArray(32) { 3 }, ByteArray(32) { 4 }, ByteArray(32) { 5 }, ByteArray(32) { 6 },
                UUID(ByteBuffer.wrap(uuid).long, ByteBuffer.wrap(uuid, 8, 8).long).toString().toByteArray())
        }
        override fun contract() = intArrayOf(2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 1L
        override fun close(handle: Long): Int { closes++; return 0 }
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? {
            val phase = ByteBuffer.wrap(fields[0]).order(java.nio.ByteOrder.LITTLE_ENDIAN).int
            check(method == 21 && phase == 12)
            calls.add(method to phase)
            if (unavailable) return null
            return this.fields.map(ByteArray::copyOf).toTypedArray().also {
                if (substitute && calls.size > 1) it[1] = "another-account".toByteArray()
            }
        }
    }
}
