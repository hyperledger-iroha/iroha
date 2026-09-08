// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.security.SecureRandom
import java.util.concurrent.locks.Lock
import kotlin.concurrent.withLock
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Ownership of a public retry record; none of these records confer monetary authority. */
enum class KagemushaOperationIntentPurposeV1 { CALLER, INTERNAL }

/** Immutable current-format public intent and, when accepted by Core, original device transcript. */
class KagemushaOperationIntentV1(
    scope: ByteArray,
    @JvmField val operation: Int,
    operationId: ByteArray,
    @JvmField val purpose: KagemushaOperationIntentPurposeV1,
    publicBinding: ByteArray,
    canonicalCommand: ByteArray? = null,
    canonicalQualification: ByteArray? = null,
    canonicalReply: ByteArray? = null,
    responseAuthenticator: ByteArray? = null,
    canonicalReplyQualification: ByteArray? = null,
    reconciliationEvidence: ByteArray? = null,
    canonicalResult: ByteArray? = null,
    @JvmField val acknowledged: Boolean = false,
) {
    private val scopeValue = scope.copyOf()
    private val id = operationId.copyOf()
    private val binding = publicBinding.copyOf()
    private val command = canonicalCommand?.copyOf()
    private val qualification = canonicalQualification?.copyOf()
    private val reply = canonicalReply?.copyOf()
    private val authenticator = responseAuthenticator?.copyOf()
    private val replyQualification = canonicalReplyQualification?.copyOf()
    private val reconciliation = reconciliationEvidence?.copyOf()
    private val result = canonicalResult?.copyOf()

    init {
        require(scopeValue.isNotEmpty() && scopeValue.size <= 4096)
        require(operation in 1..22 && id.size == 32 && id.any { it.toInt() != 0 })
        require(operation !in setOf(1, 13, 18, 21)) { "read observations cannot be durable operation intents" }
        require(binding.isNotEmpty() && binding.size <= 64 * 1024)
        listOf(command, qualification, reply, replyQualification, reconciliation, result).forEach { require(it == null || (it.isNotEmpty() && it.size <= 64 * 1024)) }
        require((reply == null) == (authenticator == null) && (reply == null) == (replyQualification == null))
        require(reply == null || (command != null && qualification != null))
        authenticator?.let(KagemushaP256Codec::requireRawLowSSignature)
        require(!acknowledged || reply != null)
        if (purpose == KagemushaOperationIntentPurposeV1.INTERNAL) require(operation in setOf(17, 19, 20))
        require(reconciliation == null || (operation in setOf(10, 17, 19, 20) && reply != null))
        require(result == null || (operation == 10 && reply != null))
        require(!acknowledged || purpose != KagemushaOperationIntentPurposeV1.INTERNAL || reconciliation != null)
    }

    fun scope(): ByteArray = scopeValue.copyOf()
    fun operationId(): ByteArray = id.copyOf()
    fun publicBinding(): ByteArray = binding.copyOf()
    fun canonicalCommand(): ByteArray? = command?.copyOf()
    fun canonicalQualification(): ByteArray? = qualification?.copyOf()
    fun canonicalReply(): ByteArray? = reply?.copyOf()
    fun responseAuthenticator(): ByteArray? = authenticator?.copyOf()
    fun canonicalReplyQualification(): ByteArray? = replyQualification?.copyOf()
    fun reconciliationEvidence(): ByteArray? = reconciliation?.copyOf()
    fun canonicalResult(): ByteArray? = result?.copyOf()

    /** Check a replacement before a store makes it durable. Identity and accepted history never change. */
    fun requireSuccessor(next: KagemushaOperationIntentV1) {
        require(operation == next.operation && purpose == next.purpose)
        require(scopeValue.contentEquals(next.scopeValue) && id.contentEquals(next.id) && binding.contentEquals(next.binding))
        command?.let { require(it.contentEquals(next.command)) }
        qualification?.let { require(it.contentEquals(next.qualification)) }
        reply?.let { require(it.contentEquals(next.reply)) }
        authenticator?.let { require(it.contentEquals(next.authenticator)) }
        replyQualification?.let { require(it.contentEquals(next.replyQualification)) }
        reconciliation?.let { require(it.contentEquals(next.reconciliation)) }
        result?.let { require(it.contentEquals(next.result)) }
        require(!acknowledged || next.acknowledged)
    }

    internal fun dispatched(command: ByteArray, qualification: ByteArray): KagemushaOperationIntentV1 =
        replacement(command = command, qualification = this.qualification ?: qualification)

    internal fun accepted(reply: ByteArray, authenticator: ByteArray, qualification: ByteArray): KagemushaOperationIntentV1 {
        this.reply?.let { require(it.contentEquals(reply)) { "device retry changed the accepted canonical reply" } }
        this.authenticator?.let { require(it.contentEquals(authenticator)) { "device retry changed the accepted authenticator" } }
        this.replyQualification?.let { require(it.contentEquals(qualification)) { "device retry changed the accepted reply qualification" } }
        return replacement(reply = this.reply ?: reply, authenticator = this.authenticator ?: authenticator,
            replyQualification = this.replyQualification ?: qualification)
    }

    internal fun acknowledged(): KagemushaOperationIntentV1 = replacement(acknowledged = true)
    internal fun reconciled(evidence: ByteArray): KagemushaOperationIntentV1 =
        replacement(reconciliation = reconciliation ?: evidence)
    internal fun completed(result: ByteArray): KagemushaOperationIntentV1 = replacement(result = result)

    private fun replacement(
        command: ByteArray? = this.command,
        qualification: ByteArray? = this.qualification,
        reply: ByteArray? = this.reply,
        authenticator: ByteArray? = this.authenticator,
        replyQualification: ByteArray? = this.replyQualification,
        reconciliation: ByteArray? = this.reconciliation,
        result: ByteArray? = this.result,
        acknowledged: Boolean = this.acknowledged,
    ): KagemushaOperationIntentV1 = KagemushaOperationIntentV1(
        scopeValue, operation, id, purpose, binding, command, qualification, reply, authenticator, replyQualification, reconciliation, result, acknowledged,
    ).also(::requireSuccessor)
}

/**
 * Required durable host storage, scoped to the exact wallet account and signed runtime.
 *
 * The lock serializes complete reserve/dispatch/reconciliation sequences across every provider
 * using this scope. Implementations also exclude another process. Writes must sync the file and
 * containing directory, reopen and verify exact bytes, and throw on missing/corrupt indexed data.
 * Acknowledged records are retained but do not consume a pending-operation capacity limit.
 * This store is public correlation/retry data, never a substitute for authenticated Core state.
 */
interface KagemushaOperationIntentStoreV1 {
    fun scope(): ByteArray
    fun exclusiveLock(): Lock
    fun load(operation: Int, operationId: ByteArray): KagemushaOperationIntentV1?
    fun pendingInternal(): List<KagemushaOperationIntentV1>
    fun save(intent: KagemushaOperationIntentV1)
}

/**
 * Exact Norito host-storage format. No prior schema or permissive decoder is accepted.
 * Uncompressed archives with zero layout flags are required before decoding any fields.
 */
object KagemushaOperationIntentCodecV1 {
    const val MAXIMUM_BYTES = 256 * 1024
    private const val SCHEMA = "iroha::sdk::offline::OperationIntentV1"
    private val adapter = object : TypeAdapter<KagemushaOperationIntentV1> {
        override fun encode(encoder: NoritoEncoder, value: KagemushaOperationIntentV1) {
            encoder.writeUInt(1, 16)
            encoder.writeUInt(value.operation.toLong(), 32)
            encoder.writeUInt(value.purpose.ordinal.toLong(), 8)
            encoder.writeUInt(if (value.acknowledged) 1 else 0, 8)
            listOf(value.scope(), value.operationId(), value.publicBinding(), value.canonicalCommand(),
                value.canonicalQualification(), value.canonicalReply(), value.responseAuthenticator(), value.canonicalReplyQualification(),
                value.reconciliationEvidence(), value.canonicalResult()).forEach {
                encoder.writeUInt((it?.size ?: 0).toLong(), 32)
                if (it != null) encoder.writeBytes(it)
            }
        }
        override fun decode(decoder: NoritoDecoder): KagemushaOperationIntentV1 {
            require(decoder.readUInt(16) == 1L)
            val operation = decoder.readUInt(32).toInt()
            val purpose = decoder.readUInt(8).toInt()
            require(purpose in 0..1)
            val acknowledged = decoder.readUInt(8)
            require(acknowledged in 0..1)
            val fields = (0..9).map {
                val size = decoder.readUInt(32)
                require(size in 0..65536)
                decoder.readBytes(size.toInt())
            }
            return KagemushaOperationIntentV1(fields[0], operation, fields[1],
                KagemushaOperationIntentPurposeV1.values()[purpose], fields[2],
                fields[3].takeIf { it.isNotEmpty() }, fields[4].takeIf { it.isNotEmpty() },
                fields[5].takeIf { it.isNotEmpty() }, fields[6].takeIf { it.isNotEmpty() },
                fields[7].takeIf { it.isNotEmpty() }, fields[8].takeIf { it.isNotEmpty() },
                fields[9].takeIf { it.isNotEmpty() }, acknowledged == 1L)
        }
    }
    private const val QUALIFICATION_SCHEMA = "iroha::sdk::offline::OperationQualificationV1"
    /** Historical accepted evidence only. Its nonce must never seed another observation. */
    internal fun encodeReconciliation(id: ByteArray, command: ByteArray, reply: ByteArray,
        authenticator: ByteArray, qualification: KagemushaHardwareQualificationV1): ByteArray {
        KagemushaDeviceOperationCodecV1.decodeControlCommand(21, id, command)
        KagemushaDeviceOperationCodecV1.decodeControlReplyAfterAuthentication(21, reply)
        KagemushaP256Codec.requireRawLowSSignature(authenticator)
        val fields = listOf(id, command, reply, authenticator, encodeQualification(qualification))
        val adapter = object : TypeAdapter<List<ByteArray>> {
            override fun encode(encoder: NoritoEncoder, value: List<ByteArray>) {
                value.forEach { encoder.writeUInt(it.size.toLong(), 32); encoder.writeBytes(it) }
            }
            override fun decode(decoder: NoritoDecoder): List<ByteArray> = error("historical evidence is not read authority")
        }
        return NoritoCodec.encode(fields, "iroha::sdk::offline::OperationReconciliationV1", adapter, 0)
    }
    private val qualificationAdapter = object : TypeAdapter<List<ByteArray>> {
        override fun encode(encoder: NoritoEncoder, value: List<ByteArray>) {
            require(value.size == 6)
            value.forEach { encoder.writeUInt(it.size.toLong(), 32); encoder.writeBytes(it) }
        }
        override fun decode(decoder: NoritoDecoder): List<ByteArray> = (0..5).map {
            val size = decoder.readUInt(32)
            require(size in 1..2048)
            decoder.readBytes(size.toInt())
        }
    }
    internal fun encodeQualification(value: KagemushaHardwareQualificationV1): ByteArray {
        value.requireProductionReady()
        val fields = listOf(KagemushaCoreCoordinatorFrameV1.u32(value.protocolVersion), value.releaseId(),
            value.hardwarePolicyDigest(), value.coreAuthorizationKeyReference(),
            KagemushaNoritoV1.encodeHardwareProfileShape(value.profile),
            KagemushaNoritoV1.encodeHardwareCredentialShape(value.credential))
        return NoritoCodec.encode(fields, QUALIFICATION_SCHEMA, qualificationAdapter, 0)
    }
    /** Structural history decoding only; native Core must authenticate the retained tuple. */
    internal fun decodeQualification(bytes: ByteArray): KagemushaHardwareQualificationV1 {
        require(bytes.size in 1..4096)
        val canonical = bytes.copyOf()
        val header = NoritoHeader.decodeView(ByteBuffer.wrap(canonical), null).header
        require(header.compression == NoritoHeader.COMPRESSION_NONE && header.flags == 0) {
            "operation qualification requires an uncompressed archive with zero layout flags"
        }
        val fields = NoritoCodec.decode(canonical, qualificationAdapter, QUALIFICATION_SCHEMA)
        require(fields[0].contentEquals(KagemushaCoreCoordinatorFrameV1.u32(1)))
        val value = KagemushaHardwareQualificationV1(1,
            KagemushaNoritoV1.decodeHardwareProfileShapeExact(fields[4]),
            KagemushaNoritoV1.decodeHardwareCredentialShapeExact(fields[5]), fields[1], fields[2], fields[3],
            java.util.EnumSet.allOf(KagemushaHardwareCapabilityV1::class.java))
        require(encodeQualification(value).contentEquals(canonical))
        return value
    }
    @JvmStatic fun encode(value: KagemushaOperationIntentV1): ByteArray =
        NoritoCodec.encode(value, SCHEMA, adapter, 0).also { require(it.size <= MAXIMUM_BYTES) }
    @JvmStatic fun decodeExact(bytes: ByteArray): KagemushaOperationIntentV1 {
        require(bytes.size in 1..MAXIMUM_BYTES)
        val canonical = bytes.copyOf()
        val header = NoritoHeader.decodeView(ByteBuffer.wrap(canonical), null).header
        require(header.compression == NoritoHeader.COMPRESSION_NONE && header.flags == 0) {
            "operation intent requires an uncompressed archive with zero layout flags"
        }
        return NoritoCodec.decode(canonical, adapter, SCHEMA).also {
            require(encode(it).contentEquals(canonical)) { "noncanonical operation intent" }
        }
    }
}

/** One durable owner of caller correlation, deliberately separate from Core preparation authority. */
internal class KagemushaOperationIntentOwnerV1(private val store: KagemushaOperationIntentStoreV1) {
    private val scope = store.scope().copyOf().also { require(it.isNotEmpty() && it.size <= 4096) }
    val lock: Lock = store.exclusiveLock()

    fun reserve(operation: Int, id: ByteArray, binding: ByteArray,
        purpose: KagemushaOperationIntentPurposeV1 = KagemushaOperationIntentPurposeV1.CALLER,
    ): KagemushaOperationIntentV1 = lock.withLock {
        require(store.scope().contentEquals(scope)) { "operation intent scope changed" }
        val expected = KagemushaOperationIntentV1(scope, operation, id, purpose, binding)
        val previous = store.load(operation, id)
        if (previous != null) {
            require(previous.scope().contentEquals(scope) && previous.purpose == purpose && previous.publicBinding().contentEquals(binding)) {
                "operation ID was reused for another exact intent"
            }
            previous
        } else expected.also(::save)
    }

    fun beginInternal(makeCommand: (ByteArray) -> KagemushaDeviceControlCommandV1): KagemushaOperationIntentV1 = lock.withLock {
        val pending = pendingInternal()
        require(pending.size <= 1) { "multiple unresolved internal transitions require reconciliation" }
        val id = pending.firstOrNull()?.operationId() ?: newId()
        val command = makeCommand(id)
        val canonical = KagemushaDeviceOperationCodecV1.encodeControlCommand(command)
        pending.firstOrNull()?.let {
            require(it.operation == command.operation && it.publicBinding().contentEquals(canonical)) {
                "the previous internal operation must be recovered before another action"
            }
            return@withLock it
        }
        reserve(command.operation, id, canonical, KagemushaOperationIntentPurposeV1.INTERNAL)
    }

    fun pendingInternal(): List<KagemushaOperationIntentV1> = lock.withLock {
        requireCurrentScope()
        store.pendingInternal().also { records ->
        records.forEach { require(it.scope().contentEquals(scope) && !it.acknowledged && it.purpose == KagemushaOperationIntentPurposeV1.INTERNAL) }
        }
    }

    fun dispatched(operation: Int, id: ByteArray, command: ByteArray, qualification: ByteArray,
        purpose: KagemushaOperationIntentPurposeV1 = KagemushaOperationIntentPurposeV1.CALLER,
    ): KagemushaOperationIntentV1 = lock.withLock {
        val previous = load(operation, id) ?: reserve(operation, id, command, purpose)
        previous.dispatched(command, qualification).also(::save)
    }

    fun accepted(operation: Int, id: ByteArray, reply: ByteArray, authenticator: ByteArray, qualification: ByteArray) = lock.withLock {
        requireNotNull(load(operation, id)) { "dispatched operation intent is missing" }
            .accepted(reply, authenticator, qualification).also(::save)
    }

    fun acknowledge(operation: Int, id: ByteArray) = lock.withLock {
        requireNotNull(load(operation, id)) { "accepted operation intent is missing" }
            .acknowledged().also(::save)
    }

    fun reconciled(operation: Int, id: ByteArray, evidence: ByteArray) = lock.withLock {
        requireNotNull(load(operation, id)) { "accepted operation intent is missing" }.reconciled(evidence).also(::save)
    }

    fun completedResult(operation: Int, id: ByteArray, result: ByteArray) = lock.withLock {
        requireNotNull(load(operation, id)) { "accepted operation intent is missing" }.completed(result).also(::save)
    }

    fun load(operation: Int, id: ByteArray): KagemushaOperationIntentV1? = lock.withLock {
        requireCurrentScope()
        store.load(operation, id)?.also {
            require(it.scope().contentEquals(scope) && it.operation == operation && it.operationId().contentEquals(id))
        }
    }

    fun requireCurrentScope() {
        require(store.scope().contentEquals(scope)) { "operation intent scope changed" }
    }

    private fun save(intent: KagemushaOperationIntentV1) {
        requireCurrentScope()
        require(intent.scope().contentEquals(scope))
        store.save(intent)
        val retained = requireNotNull(store.load(intent.operation, intent.operationId())) { "operation intent was not durably retained" }
        require(KagemushaOperationIntentCodecV1.encode(retained).contentEquals(KagemushaOperationIntentCodecV1.encode(intent))) {
            "operation intent storage substituted the durable record"
        }
    }

    companion object {
        private val random = SecureRandom()
        fun newId(): ByteArray {
            val id = ByteArray(32)
            do { random.nextBytes(id) } while (id.all { it.toInt() == 0 })
            return id
        }
    }
}
