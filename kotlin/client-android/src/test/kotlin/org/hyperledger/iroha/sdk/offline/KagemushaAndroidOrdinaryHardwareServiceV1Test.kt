package org.hyperledger.iroha.sdk.offline

import android.content.Context
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.UUID
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertSame
import kotlin.test.assertTrue
import kotlin.test.fail

/** Scripted strict C21/JNI projections only. No Native authority, signature or platform key is created. */
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
                activatedSignatoryAccountId: String, transport: KagemushaOrdinaryIdentityOriginalTransportV1,
                selection: KagemushaNativeWalletAccountSelectionOriginalV1,
                requireOriginalOwner: () -> Unit): KagemushaAndroidOrdinaryHardwareServiceV1 =
                error("A substitute must never open")
        }
        reject { KagemushaAndroidOrdinaryHardwareServiceFactoryV1.selectOriginalFactory(listOf(substitute).iterator()) }
    }
    @Test fun activatedSIsBoundToProjectedSAndDistinctNativeW() {
        val endpoint = Endpoint()
        val selected = requireAccount(native(endpoint), "activated-S")
        assertEquals("native-W", selected.walletAccountId())
        assertEquals("activated-S", selected.signatoryAccountId())
        assertTrue(endpoint.calls.all { it.first == 21 && it.second in setOf(12, 15) })
        assertTrue(endpoint.calls.none { it.second == 12 })
        assertEquals(0, endpoint.closes)
    }
    @Test fun anotherActivatedSCannotReserveNativeIdentity() {
        val endpoint = Endpoint()
        reject { requireAccount(native(endpoint), "another-S") }
        assertTrue(endpoint.calls.isNotEmpty())
        assertTrue(endpoint.calls.none { it.second == 12 })
    }
    @Test fun nativeWalletWCannotBePassedAsTheActivatedMemberS() {
        val endpoint = Endpoint()
        reject { requireAccount(native(endpoint), "native-W") }
        assertTrue(endpoint.calls.none { it.second == 12 })
    }
    @Test fun capturedSelectionCannotDispatchAfterOriginalOwnerRevocation() {
        val endpoint = Endpoint(); val owner = native(endpoint)
        val selected = owner.appIdentityOperations().currentWalletAccountSelection()
        val prior = endpoint.calls.size
        reject { KagemushaAndroidOrdinaryHardwareServiceV1.requireOriginalNativeAccount(
            owner, "activated-S", selected) { error("Original account revoked") } }
        assertEquals(prior, endpoint.calls.size)
    }
    @Test fun postProjectionRevocationWithholdsSelectionBeforeReservation() {
        val endpoint = Endpoint(); var guards = 0
        reject { requireAccount(native(endpoint), "activated-S") { guards++; check(guards == 1) } }
        assertTrue(endpoint.calls.isNotEmpty())
        assertTrue(endpoint.calls.none { it.second == 12 })
    }
    @Test fun missingNativeSessionClosesAndCannotSelectManagedSigningOrAnotherRoute() {
        val endpoint = Endpoint().apply { unavailable = true }; val owner = native(endpoint)
        reject { requireAccount(owner, "activated-S") }
        reject { requireAccount(owner, "activated-S") }
        assertEquals(listOf(21 to 15), endpoint.calls)
        assertEquals(1, endpoint.closes)
    }
    @Test fun zeroSessionProjectionIsRejectedBeforeReservation() {
        val endpoint = Endpoint().apply { selection[0].fill(0) }
        reject { requireAccount(native(endpoint), "activated-S") }
        assertTrue(endpoint.calls.none { it.second == 12 })
        assertEquals(1, endpoint.closes)
    }
    @Test fun equalWalletAndSignatoryProjectionIsRejectedBeforeReservation() {
        val endpoint = Endpoint().apply { selection[1] = selection[2].copyOf() }
        reject { requireAccount(native(endpoint), "activated-S") }
        assertTrue(endpoint.calls.none { it.second == 12 })
        assertEquals(1, endpoint.closes)
    }
    @Test fun replacedNativeSessionCannotReuseTheOriginalProjection() {
        assertReplacedSelectionRefused(0, ByteArray(8) { 9 })
    }
    @Test fun replacedNativeWalletCannotReuseTheOriginalProjection() {
        assertReplacedSelectionRefused(1, "another-W".toByteArray())
    }
    @Test fun replacedNativeSignatoryCannotReuseTheOriginalProjection() {
        assertReplacedSelectionRefused(2, "another-S".toByteArray())
    }
    @Test fun completedSelectionDoesNotReserveNewFinancialIdentityToReachRecovery() {
        val endpoint = Endpoint().apply { reservation[1] = "another-W".toByteArray() }
        val selected=requireAccount(native(endpoint), "activated-S")
        assertEquals("native-W",selected.walletAccountId())
        assertTrue(endpoint.calls.none {it.second==12})
        assertEquals(0,endpoint.closes)
    }
    @Test fun dataOnlyFactoryNeverRenewsAConsumedFinancialReservation() {
        val endpoint=Endpoint().apply {substituteReservation=true}
        requireAccount(native(endpoint),"activated-S")
        assertTrue(endpoint.calls.none {it.second==12});assertEquals(0,endpoint.closes)
    }
    @Test fun retainedSelectionCannotCrossNativeCoordinators() {
        val originalEndpoint = Endpoint()
        val selected = requireAccount(native(originalEndpoint), "activated-S")
        val otherEndpoint = Endpoint()
        reject { KagemushaAndroidOrdinaryHardwareServiceV1.requireOriginalNativeAccount(
            native(otherEndpoint), "activated-S", selected) {} }
        assertTrue(otherEndpoint.calls.isEmpty())
    }
    @Test fun absentCurrentSessionCannotReviveTheRetainedSelection() {
        val endpoint = Endpoint(); val owner = native(endpoint)
        val selected = requireAccount(owner, "activated-S")
        endpoint.unavailable = true
        reject { selected.requireCurrent() }
        val calls = endpoint.calls.size
        reject { selected.requireCurrent() }
        assertEquals(calls, endpoint.calls.size)
        assertEquals(1, endpoint.closes)
    }
    @Test fun emptyActivatedSignatoryCannotDispatchFurtherNativeWork() {
        val endpoint = Endpoint(); val owner = native(endpoint)
        val selected = owner.appIdentityOperations().currentWalletAccountSelection()
        val prior = endpoint.calls.size
        reject { KagemushaAndroidOrdinaryHardwareServiceV1.requireOriginalNativeAccount(owner, " ", selected) {} }
        assertEquals(prior, endpoint.calls.size)
    }
    private fun assertReplacedSelectionRefused(index: Int, replacement: ByteArray) {
        val endpoint = Endpoint().apply { substituteSelectionIndex = index; substituteSelection = replacement }
        reject { requireAccount(native(endpoint), "activated-S") }
        assertTrue(endpoint.calls.none { it.second == 12 })
        assertEquals(1, endpoint.closes)
    }
    private fun requireAccount(owner: KagemushaNativeCoreCoordinatorAdapterV1, activatedS: String,
        guard: () -> Unit = {}): KagemushaNativeWalletAccountSelectionOriginalV1 =
        KagemushaAndroidOrdinaryHardwareServiceV1.requireOriginalNativeAccount(
            owner, activatedS, owner.appIdentityOperations().currentWalletAccountSelection(), guard)
    private fun native(endpoint: Endpoint) = KagemushaNativeCoreCoordinatorAdapterV1.openEndpoint("/fixture/ordinary-account-service", endpoint)
    private fun reject(block: () -> Unit) {
        try { block() } catch (_: IllegalStateException) { return } catch (_: IllegalArgumentException) { return }
        fail("Expected the route to reject")
    }
    private class Endpoint : KagemushaCoreCoordinatorEndpointV1 {
        val calls = mutableListOf<Pair<Int, Int>>()
        var closes = 0; var unavailable = false; var substituteReservation = false
        var substituteSelectionIndex: Int? = null; var substituteSelection = ByteArray(0)
        private var selectionReads = 0; private var reservationReads = 0
        val selection = arrayOf(ByteArray(8) { 1 }, "native-W".toByteArray(), "activated-S".toByteArray())
        val reservation = run {
            val nonce = ByteArray(32) { 2 }; val uuid = nonce.copyOf(16)
            uuid[6] = ((uuid[6].toInt() and 0x0f) or 0x40).toByte()
            uuid[8] = ((uuid[8].toInt() and 0x3f) or 0x80).toByte()
            arrayOf(ByteArray(8) { 1 }, "native-W".toByteArray(), nonce,
                ByteArray(32) { 3 }, ByteArray(32) { 4 }, ByteArray(32) { 5 }, ByteArray(32) { 6 },
                UUID(ByteBuffer.wrap(uuid).long, ByteBuffer.wrap(uuid, 8, 8).long).toString().toByteArray())
        }
        override fun contract() = intArrayOf(2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 1L
        override fun close(handle: Long): Int { closes++; return 0 }
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? {
            val phase = ByteBuffer.wrap(fields[0]).order(ByteOrder.LITTLE_ENDIAN).int
            check(method == 21 && phase in setOf(12, 15))
            calls.add(method to phase)
            if (unavailable) return null
            return when (phase) {
                15 -> {
                    selectionReads++
                    selection.map(ByteArray::copyOf).toTypedArray().also { response ->
                        if (selectionReads > 2) substituteSelectionIndex?.let { response[it] = substituteSelection.copyOf() }
                    }
                }
                else -> {
                    reservationReads++
                    reservation.map(ByteArray::copyOf).toTypedArray().also {
                        if (substituteReservation && reservationReads > 1) it[1] = "another-W".toByteArray()
                    }
                }
            }
        }
    }
}
