package org.hyperledger.iroha.sdk.offline

import kotlin.test.*

class KagemushaOperationIntentV1Test {
    @Test fun `lost sync response retains exact ID and command across owner recreation`() {
        val store = TestOperationIntentStoreV1().apply { throwAfterSave = true }
        val original = KagemushaOperationIntentOwnerV1(store)
        assertFailsWith<IllegalStateException> {
            original.beginInternal(KagemushaDeviceControlCommandV1::BootstrapAggregateState)
        }
        store.throwAfterSave = false
        val saved = store.pendingInternal().single()
        val recovered = KagemushaOperationIntentOwnerV1(store)
            .beginInternal(KagemushaDeviceControlCommandV1::BootstrapAggregateState)
        assertContentEquals(saved.operationId(), recovered.operationId())
        assertContentEquals(saved.publicBinding(), recovered.publicBinding())
        assertEquals(1, store.pendingInternal().size)
    }

    @Test fun `pending fold resumes original selector and rejects different credit after restart`() {
        val store = TestOperationIntentStoreV1()
        val selector = KagemushaPendingCreditSelectorV1(KagemushaPendingCreditKindV1.RECEIVE, ByteArray(32) { 3 })
        val first = KagemushaOperationIntentOwnerV1(store).beginInternal {
            KagemushaDeviceControlCommandV1.FoldReceiveCredit(it, selector)
        }
        val resumed = KagemushaOperationIntentOwnerV1(store).beginInternal {
            KagemushaDeviceControlCommandV1.FoldReceiveCredit(it, selector)
        }
        assertContentEquals(first.operationId(), resumed.operationId())
        val decoded = KagemushaDeviceOperationCodecV1.decodeControlCommand(17, resumed.operationId(), resumed.publicBinding())
            as KagemushaDeviceControlCommandV1.FoldReceiveCredit
        assertContentEquals(selector.creditId(), decoded.selector.creditId())
        assertFailsWith<IllegalArgumentException> {
            KagemushaOperationIntentOwnerV1(store).beginInternal {
                KagemushaDeviceControlCommandV1.FoldReceiveCredit(it,
                    KagemushaPendingCreditSelectorV1(KagemushaPendingCreditKindV1.RECEIVE, ByteArray(32) { 4 }))
            }
        }
        assertEquals(1, store.pendingInternal().size)
    }

    @Test fun `identical new actions retain distinct IDs and conflicting reuse never mutates history`() {
        val store = TestOperationIntentStoreV1()
        val owner = KagemushaOperationIntentOwnerV1(store)
        val first = ByteArray(32) { 1 }
        val second = ByteArray(32) { 2 }
        owner.reserve(5, first, byteArrayOf(7))
        owner.reserve(5, second, byteArrayOf(7))
        assertFailsWith<IllegalArgumentException> { owner.reserve(5, first, byteArrayOf(8)) }
        assertContentEquals(byteArrayOf(7), store.load(5, first)!!.publicBinding())
        assertContentEquals(second, store.load(5, second)!!.operationId())
    }

    @Test fun `acknowledgement requires accepted reply and retains an immutable complete transcript`() {
        val store = TestOperationIntentStoreV1()
        val owner = KagemushaOperationIntentOwnerV1(store)
        val pending = owner.beginInternal(KagemushaDeviceControlCommandV1::BootstrapAggregateState)
        val id = pending.operationId()
        assertFailsWith<IllegalArgumentException> { owner.acknowledge(20, id) }
        owner.dispatched(20, id, pending.publicBinding(), byteArrayOf(5), KagemushaOperationIntentPurposeV1.INTERNAL)
        val signature = ByteArray(64).also { it[31] = 1; it[63] = 1 }
        owner.accepted(20, id, byteArrayOf(6), signature, byteArrayOf(5))
        val beforeAck = store.load(20, id)!!
        assertFalse(beforeAck.acknowledged)
        assertEquals(1, store.pendingInternal().size)
        assertFailsWith<IllegalArgumentException> { owner.acknowledge(20, id) }
        owner.reconciled(20, id, byteArrayOf(8))
        owner.acknowledge(20, id)
        assertTrue(store.load(20, id)!!.acknowledged)
        assertTrue(store.pendingInternal().isEmpty())
        assertContentEquals(signature, store.load(20, id)!!.responseAuthenticator())
        assertFailsWith<IllegalArgumentException> { owner.accepted(20, id, byteArrayOf(9), signature, byteArrayOf(5)) }
        val next = owner.beginInternal(KagemushaDeviceControlCommandV1::BootstrapAggregateState)
        assertFalse(next.operationId().contentEquals(id))
    }

    @Test fun `record codec rejects corrupt tails and retired schema while retaining defensive copies`() {
        val scope = byteArrayOf(1)
        val id = ByteArray(32) { 2 }
        val binding = byteArrayOf(3)
        val record = KagemushaOperationIntentV1(scope, 5, id, KagemushaOperationIntentPurposeV1.CALLER, binding)
        scope.fill(7); id.fill(7); binding.fill(7)
        assertContentEquals(byteArrayOf(1), record.scope())
        val bytes = KagemushaOperationIntentCodecV1.encode(record)
        val decoded = KagemushaOperationIntentCodecV1.decodeExact(bytes)
        assertContentEquals(ByteArray(32) { 2 }, decoded.operationId())
        decoded.operationId().fill(0)
        assertContentEquals(ByteArray(32) { 2 }, decoded.operationId())
        assertFailsWith<IllegalArgumentException> { KagemushaOperationIntentCodecV1.decodeExact(bytes + 0) }
        val corrupt = bytes.copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() }
        assertFailsWith<IllegalArgumentException> { KagemushaOperationIntentCodecV1.decodeExact(corrupt) }
    }

    @Test fun `accepted retry rejects changed authenticator or historical qualification after owner recreation`() {
        val store = TestOperationIntentStoreV1()
        val first = KagemushaOperationIntentOwnerV1(store)
        val id = ByteArray(32) { 2 }
        val command = byteArrayOf(3)
        val qualification = byteArrayOf(4)
        val reply = byteArrayOf(5)
        val signature = ByteArray(64).also { it[31] = 1; it[63] = 1 }
        first.dispatched(5, id, command, qualification)
        first.accepted(5, id, reply, signature, qualification)
        val resumed = KagemushaOperationIntentOwnerV1(store)
        resumed.accepted(5, id, reply, signature, qualification)
        val saved = KagemushaOperationIntentCodecV1.encode(store.load(5, id)!!)
        assertFailsWith<IllegalArgumentException> {
            resumed.accepted(5, id, reply, signature.copyOf().also { it[63] = 2 }, qualification)
        }
        assertFailsWith<IllegalArgumentException> {
            resumed.accepted(5, id, reply, signature, byteArrayOf(6))
        }
        assertContentEquals(saved, KagemushaOperationIntentCodecV1.encode(store.load(5, id)!!))
    }

    @Test fun `scope replacement cannot admit old pending intents`() {
        val store = TestOperationIntentStoreV1()
        val owner = KagemushaOperationIntentOwnerV1(store)
        val retained = owner.beginInternal(KagemushaDeviceControlCommandV1::BootstrapAggregateState)
        store.scopeValue = byteArrayOf(43)
        assertFailsWith<IllegalArgumentException> { owner.reserve(5, ByteArray(32) { 2 }, byteArrayOf(3)) }
        assertFailsWith<IllegalArgumentException> { owner.load(20, retained.operationId()) }
        assertFailsWith<IllegalArgumentException> { owner.pendingInternal() }
        assertFailsWith<IllegalArgumentException> { KagemushaOperationIntentOwnerV1(store).pendingInternal() }
    }

    @Test fun `read operations cannot enter the durable operation store`() {
        for (operation in listOf(1, 13, 18, 21)) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaOperationIntentV1(byteArrayOf(1), operation, ByteArray(32) { 2 },
                    KagemushaOperationIntentPurposeV1.CALLER, byteArrayOf(3))
            }
        }
    }

    @Test fun `Core recovered terminal result remains immutable when the original install reply was lost`() {
        val store = TestOperationIntentStoreV1()
        val owner = KagemushaOperationIntentOwnerV1(store)
        val id = ByteArray(32) { 12 }
        owner.reserve(9, id, byteArrayOf(9))
        owner.dispatched(9, id, byteArrayOf(9), byteArrayOf(5))
        owner.reserve(10, id, byteArrayOf(10))
        owner.dispatched(10, id, byteArrayOf(10), byteArrayOf(5))
        owner.accepted(10, id, byteArrayOf(6), ByteArray(64).also { it[31] = 1; it[63] = 1 }, byteArrayOf(5))
        owner.completedResult(10, id, byteArrayOf(7))
        owner.acknowledge(10, id)
        assertNull(store.load(9, id)!!.canonicalReply())
        assertContentEquals(byteArrayOf(7), store.load(10, id)!!.canonicalResult())
        assertFailsWith<IllegalArgumentException> { owner.completedResult(10, id, byteArrayOf(8)) }
    }
}
