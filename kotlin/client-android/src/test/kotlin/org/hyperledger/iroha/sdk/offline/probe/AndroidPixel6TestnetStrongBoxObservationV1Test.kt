// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import org.junit.jupiter.api.Test

class AndroidPixel6TestnetStrongBoxObservationV1Test {
    private class Device : Pixel6TestnetObservationDeviceV1 {
        override var apiLevel = 31
        var pixel6 = true
        var strongBox = true
        var generated = 0
        var signed = 0
        var deleted = 0
        var failSign = false
        var failDelete = false
        var aliasExists = false
        var observedChallenge: ByteArray? = null
        var observedMessage: ByteArray? = null
        override fun isPixel6(): Boolean = pixel6
        override fun hasStrongBox(): Boolean = strongBox
        override fun newNonce(): ByteArray = ByteArray(32) { 0x31 }
        override fun hasAlias(alias: String): Boolean = aliasExists
        override fun generate(alias: String, challenge: ByteArray): ProbeKeyMaterialV1 {
            generated++
            observedChallenge = challenge.copyOf()
            return ProbeKeyMaterialV1(byteArrayOf(0x04) + ByteArray(64) { 1 },
                listOf(byteArrayOf(1, 2, 3)))
        }
        override fun sign(alias: String, message: ByteArray): ByteArray {
            signed++
            observedMessage = message.copyOf()
            if (failSign) throw IllegalStateException("injected lost sign result")
            return byteArrayOf(0x30, 0x01)
        }
        override fun delete(alias: String) {
            deleted++
            if (failDelete) throw IllegalStateException("injected cleanup failure")
        }
    }

    private class Store : Pixel6TestnetObservationStoreV1 {
        var slot: String? = null
        var intent: ByteArray? = null
        var evidence: Pixel6TestnetObservationResultV1.Evidence? = null
        var reserves = 0
        override fun <T> withSlotLock(slot: String, action: () -> T): T = action()
        override fun lookup(slot: String, intent: ByteArray): Pixel6TestnetObservationLookupV1 {
            val existing = this.intent ?: return Pixel6TestnetObservationLookupV1.Empty
            if (this.slot != slot || !existing.copyOfRange(0, 32).contentEquals(intent)) {
                return Pixel6TestnetObservationLookupV1.Frozen
            }
            return evidence?.let { Pixel6TestnetObservationLookupV1.Recovered(it) }
                ?: Pixel6TestnetObservationLookupV1.Frozen
        }
        override fun reserve(slot: String, intent: ByteArray) {
            check(this.intent == null)
            this.slot = slot
            this.intent = intent.copyOf()
            reserves++
        }
        override fun persist(
            slot: String,
            intent: ByteArray,
            evidence: Pixel6TestnetObservationResultV1.Evidence,
        ) {
            check(this.slot == slot && this.intent!!.contentEquals(intent))
            this.evidence = evidence
        }
    }

    private val network = ByteArray(32) { 1 }
    private val release = ByteArray(32) { 2 }
    private val lane = ByteArray(32) { 3 }
    private val before = ByteArray(16)
    private val after = byteArrayOf(1) + ByteArray(15)
    private fun selectionFrame(networkId: ByteArray, releaseId: ByteArray): ByteArray {
        val domain = "iroha:kagemusha:v1:hardware-transition-selection\u0000"
            .toByteArray(Charsets.US_ASCII)
        val message = ByteArray(460)
        domain.copyInto(message)
        message[49] = 0x93.toByte() // 403 fixed body bytes, u64-LE.
        message[50] = 0x01
        message[57] = 1 // V1, u16-LE.
        releaseId.copyInto(message, 59)
        message[91] = 1 // Nonzero provider policy root.
        message[123] = 1 // Nonzero app policy digest.
        message[155] = 1 // Nonzero credential ID.
        networkId.copyInto(message, 187)
        lane.copyInto(message, 219)
        message[251] = 1 // Nonzero hardware profile ID.
        message[283] = 1 // Policy epoch 1.
        message[291] = 1 // Nonzero hardware epoch ID.
        message[323] = 1 // Hardware epoch generation 1.
        message[331] = 1 // MintFold operation tag.
        message[332] = 1 // Nonzero transition statement digest.
        before.copyInto(message, 428)
        after.copyInto(message, 444)
        return message
    }
    private val frame = selectionFrame(network, release)

    private fun collect(
        device: Device,
        store: Store,
        networkId: ByteArray = network,
        releaseId: ByteArray = release,
        selectionFrame: ByteArray = frame,
    ): Pixel6TestnetObservationResultV1 = Pixel6TestnetObservationRunnerV1(device, store)
        .collect(networkId, releaseId, selectionFrame, lane, before, after)

    @Test fun evidenceIsExplicitlyUnqualifiedAndScopeBound() {
        val device = Device()
        val store = Store()
        val result = assertIs<Pixel6TestnetObservationResultV1.Evidence>(collect(device, store))
        assertEquals(AndroidPixel6TestnetStrongBoxObservationV1.PROFILE, result.profile)
        assertFalse(result.hardwareOneUseQualified)
        assertContentEquals(network, result.networkId())
        assertContentEquals(release, result.releaseId())
        assertContentEquals(device.observedChallenge, result.attestationChallenge())
        assertContentEquals(device.observedMessage, result.signedMessage())
        assertEquals(32, result.attestationNonce().size)
        assertEquals(1, store.reserves)
        assertEquals(1, device.generated)
        assertEquals(1, device.signed)
        assertEquals(1, device.deleted)
        val recovered = assertIs<Pixel6TestnetObservationResultV1.Evidence>(collect(device, store))
        assertContentEquals(result.signatureDer(), recovered.signatureDer())
        assertEquals(1, device.signed)
    }

    @Test fun samePredecessorCannotBeReselectedForAnotherNetworkReleaseOrFrame() {
        val device = Device()
        val store = Store()
        assertIs<Pixel6TestnetObservationResultV1.Evidence>(collect(device, store))
        val anotherNetwork = ByteArray(32) { 4 }
        val anotherRelease = ByteArray(32) { 5 }
        assertIs<Pixel6TestnetObservationResultV1.Frozen>(
            collect(device, store, networkId = anotherNetwork,
                selectionFrame = selectionFrame(anotherNetwork, release)),
        )
        assertIs<Pixel6TestnetObservationResultV1.Frozen>(
            collect(device, store, releaseId = anotherRelease,
                selectionFrame = selectionFrame(network, anotherRelease)),
        )
        assertIs<Pixel6TestnetObservationResultV1.Frozen>(
            collect(device, store, selectionFrame = frame.copyOf().also { it[332] = 0x43 }),
        )
        assertEquals(1, device.signed)
        assertEquals(1, store.reserves)
    }

    @Test fun lostSignResultFreezesPredecessorWithoutRegenerating() {
        val device = Device().apply { failSign = true }
        val store = Store()
        val first = assertIs<Pixel6TestnetObservationResultV1.Frozen>(collect(device, store))
        assertEquals("sign", first.stage)
        assertEquals(1, device.deleted)
        device.failSign = false
        assertIs<Pixel6TestnetObservationResultV1.Frozen>(collect(device, store))
        assertEquals(1, device.generated)
        assertEquals(1, device.signed)
    }

    @Test fun absentStrongBoxAndInvalidScopeNeverReserveOrGenerate() {
        val device = Device().apply { strongBox = false }
        val store = Store()
        assertIs<Pixel6TestnetObservationResultV1.Unavailable>(collect(device, store))
        assertEquals(0, store.reserves)
        assertEquals(0, device.generated)
        assertFailsWith<IllegalArgumentException> {
            collect(device, store, networkId = ByteArray(32))
        }
        assertFailsWith<IllegalArgumentException> {
            collect(device, store, selectionFrame = frame.dropLast(1).toByteArray())
        }
        assertEquals(0, store.reserves)
    }

    @Test fun otherDeviceCannotCollectOrRecoverPixel6Profile() {
        val device = Device()
        val store = Store()
        assertIs<Pixel6TestnetObservationResultV1.Evidence>(collect(device, store))
        device.pixel6 = false
        assertIs<Pixel6TestnetObservationResultV1.Unavailable>(collect(device, store))
        assertEquals(1, device.signed)
        assertEquals(1, store.reserves)
    }

    @Test fun cleanupFailureFreezesIntentInsteadOfPersistingIncompleteEvidence() {
        val device = Device().apply { failDelete = true }
        val store = Store()
        val first = assertIs<Pixel6TestnetObservationResultV1.Frozen>(collect(device, store))
        assertEquals("cleanup", first.stage)
        assertEquals(null, store.evidence)
        device.failDelete = false
        assertIs<Pixel6TestnetObservationResultV1.Frozen>(collect(device, store))
        assertEquals(1, device.generated)
        assertEquals(1, device.signed)
    }

    @Test fun frameFieldsMustExactlyMatchNetworkReleaseLaneAndIndices() {
        val device = Device()
        val store = Store()
        for (offset in listOf(59, 187, 219, 428, 444)) {
            assertFailsWith<IllegalArgumentException> {
                collect(device, store, selectionFrame = frame.copyOf().also {
                    it[offset] = (it[offset].toInt() xor 1).toByte()
                })
            }
        }
        for (offset in listOf(49, 50, 57, 58)) {
            assertFailsWith<IllegalArgumentException> {
                collect(device, store, selectionFrame = frame.copyOf().also {
                    it[offset] = (it[offset].toInt() xor 1).toByte()
                })
            }
        }
        assertEquals(0, store.reserves)
        assertEquals(0, device.generated)
    }

    @Test fun frameMustSatisfyRustSelectionShape() {
        val device = Device()
        val store = Store()
        for (offset in listOf(91, 123, 155, 251, 283, 291, 323, 332)) {
            assertFailsWith<IllegalArgumentException> {
                collect(device, store, selectionFrame = frame.copyOf().also { it[offset] = 0 })
            }
        }
        for (operation in listOf(0, 6, 255)) {
            assertFailsWith<IllegalArgumentException> {
                collect(device, store, selectionFrame = frame.copyOf().also { it[331] = operation.toByte() })
            }
        }
        for (offset in listOf(364, 396)) {
            assertFailsWith<IllegalArgumentException> {
                collect(device, store, selectionFrame = frame.copyOf().also { it[offset] = 1 })
            }
        }
        for (operation in listOf(2, 4)) {
            assertFailsWith<IllegalArgumentException> {
                collect(device, store, selectionFrame = frame.copyOf().also { it[331] = operation.toByte() })
            }
            val outgoing = frame.copyOf().also {
                it[331] = operation.toByte()
                it[364] = 1
                it[396] = 1
            }
            assertIs<Pixel6TestnetObservationResultV1.Evidence>(collect(device, Store(),
                selectionFrame = outgoing))
        }
        assertEquals(0, store.reserves)
    }

    @Test fun skippedAndOverflowedIndicesNeverReserveOrTouchStrongBox() {
        val device = Device()
        val store = Store()
        val skipped = byteArrayOf(2) + ByteArray(15)
        assertFailsWith<IllegalArgumentException> {
            Pixel6TestnetObservationRunnerV1(device, store).collect(
                network, release, frame.copyOf().also { skipped.copyInto(it, 444) },
                lane, before, skipped,
            )
        }
        val maximum = ByteArray(16) { 0xff.toByte() }
        assertFailsWith<IllegalArgumentException> {
            Pixel6TestnetObservationRunnerV1(device, store).collect(
                network, release, frame.copyOf().also {
                    maximum.copyInto(it, 428)
                    before.copyInto(it, 444)
                },
                lane, maximum, before,
            )
        }
        assertEquals(0, store.reserves)
        assertEquals(0, device.generated)
    }
}
