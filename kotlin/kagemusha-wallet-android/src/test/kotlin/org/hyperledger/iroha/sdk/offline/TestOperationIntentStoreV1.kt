package org.hyperledger.iroha.sdk.offline

import java.util.concurrent.locks.ReentrantLock

/** Test-only durable-state model; tests retain this object across owner recreation. */
class TestOperationIntentStoreV1 : KagemushaOperationIntentStoreV1 {
    private val records = mutableMapOf<String, ByteArray>()
    private val lock = ReentrantLock(true)
    var failSave = false
    var scopeValue = byteArrayOf(42)
    val events = mutableListOf<String>()
    override fun scope() = scopeValue.copyOf()
    override fun exclusiveLock() = lock
    private fun key(op: Int, id: ByteArray) = "$op:" + id.joinToString("") { "%02x".format(it) }
    override fun load(operation: Int, operationId: ByteArray) = records[key(operation, operationId)]?.let(KagemushaOperationIntentCodecV1::decodeExact)
    override fun pendingInternal() = records.values.map(KagemushaOperationIntentCodecV1::decodeExact)
        .filter { it.purpose == KagemushaOperationIntentPurposeV1.INTERNAL && !it.acknowledged }
    override fun save(intent: KagemushaOperationIntentV1) {
        check(!failSave) { "interrupted sync" }
        load(intent.operation, intent.operationId())?.requireSuccessor(intent)
        records[key(intent.operation, intent.operationId())] = KagemushaOperationIntentCodecV1.encode(intent)
        events.add("saved:${intent.operation}:${intent.canonicalReply() != null}:${intent.acknowledged}")
    }
}
