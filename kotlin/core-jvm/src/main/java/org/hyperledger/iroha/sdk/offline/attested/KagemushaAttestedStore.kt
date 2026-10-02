// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.attested

/**
 * Durable record store of one attested-app wallet (one scheme and one account).
 *
 * The engine owns every record format; the store only persists opaque values under string keys.
 * Implementations must be private to the app, excluded from backups and device transfer, and
 * used by exactly one engine instance at a time.
 *
 * Android uses SQLite in WAL mode with `synchronous=FULL` under `noBackupFilesDir`; iOS uses
 * SQLite under Application Support with `isExcludedFromBackup`.
 */
interface KagemushaAttestedStore {
    /** Every stored record. An empty map means this installation holds no wallet state. */
    fun readAll(): Map<String, ByteArray>

    /**
     * Apply [puts] and [deletes] as one atomic transaction and make it durable (fsync) before
     * returning. A failure must leave the previous state intact and throw.
     */
    fun commit(puts: Map<String, ByteArray>, deletes: Set<String>)
}

/** Engine-owned record formats. Each record starts with its own version field. */
internal object KagemushaAttestedRecords {
    const val VERSION: Int = 1

    const val DESCRIPTOR = "descriptor"
    const val ENROLLMENT = "enrollment"
    const val STATE = "state"
    const val CERT = "cert"
    const val CRL = "crl"
    const val GOSSIP = "gossip"
    const val JOURNAL = "journal/"
    const val OUTBOX = "outbox/"
    const val REQUEST = "request/"
    const val PEER = "peer/"
    const val VOUCHER = "voucher/"
    const val REDEMPTION = "redemption/"
    const val FORK = "fork/"

    fun journalKey(seq: Long): String = JOURNAL + String.format("%016x", seq)

    fun hex(bytes: ByteArray): String {
        val chars = CharArray(bytes.size * 2)
        bytes.forEachIndexed { index, byte ->
            val value = byte.toInt() and 0xff
            chars[index * 2] = HEX[value ushr 4]
            chars[index * 2 + 1] = HEX[value and 0x0f]
        }
        return String(chars)
    }

    private val HEX = "0123456789abcdef".toCharArray()

    fun encode(write: (NoritoOut) -> Unit): ByteArray = KagemushaAttestedNorito.payload { out ->
        out.u8(VERSION)
        write(out)
    }

    fun <T> decode(bytes: ByteArray, read: (NoritoIn) -> T): T = KagemushaAttestedNorito.readPayload(bytes) { input ->
        require(input.u8() == VERSION) { "unsupported KAGEMUSHA wallet record version" }
        read(input)
    }

    fun NoritoOut.blob(bytes: ByteArray) = nested { it.raw(bytes) }

    fun NoritoIn.blob(maximum: Int): ByteArray = opaque(maximum)

    fun NoritoOut.optionalBlob(bytes: ByteArray?) = option(bytes) { inner, value -> inner.raw(value) }

    fun NoritoIn.optionalBlob(maximum: Int): ByteArray? = option { inner ->
        require(inner.remaining() <= maximum) { "wallet record blob is too large" }
        inner.raw(inner.remaining())
    }
}

/** Enrollment in progress: written before the platform key is generated. */
internal class EnrollmentRecord(
    val accountId: String,
    val platform: KagemushaAttestedPlatform,
    val serverNonce: ByteArray,
    val clientNonce: ByteArray,
    val attestedKeyId: ByteArray,
    val signingPublicKey: ByteArray,
    val createdAtMs: Long,
) {
    fun encode(): ByteArray = KagemushaAttestedRecords.encode { out ->
        out.string(accountId)
        out.u8(platform.code)
        out.array(serverNonce, 32)
        out.array(clientNonce, 32)
        out.array(attestedKeyId, 32)
        out.array(signingPublicKey, 65)
        out.u64(createdAtMs)
    }

    companion object {
        fun decode(bytes: ByteArray): EnrollmentRecord = KagemushaAttestedRecords.decode(bytes) { input ->
            EnrollmentRecord(
                input.string(KagemushaAttestedLimits.MAXIMUM_STRING_BYTES),
                KagemushaAttestedPlatform.fromCode(input.u8()) ?: throw IllegalArgumentException("bad platform"),
                input.array(32),
                input.array(32),
                input.array(32),
                input.array(65),
                input.u64(),
            )
        }
    }
}

/** Mutable-by-copy wallet counters. */
internal class StateRecord(
    val accountId: String,
    val seq: Long,
    val head: ByteArray,
    val balance: Long,
    val ackedSeq: Long,
    /** Digest of the issuer-acknowledged transition; all zero until the first sync. */
    val ackedDigest: ByteArray,
    val unsyncedOut: Long,
    val syncNonce: ByteArray,
    /** Effective CRL epoch: the full list plus contiguous gossiped deltas. */
    val crlEpoch: Long,
    /** Revocations learned from gossip beyond the installed full list. */
    val extraRevocations: List<KagemushaAttestedRevocationEntryV1>,
    val highWaterMs: Long,
    val clockAnomaly: Boolean,
    val lastSyncMs: Long,
) {
    val everAcked: Boolean get() = ackedDigest.any { it.toInt() != 0 }

    fun copy(
        seq: Long = this.seq,
        head: ByteArray = this.head,
        balance: Long = this.balance,
        ackedSeq: Long = this.ackedSeq,
        ackedDigest: ByteArray = this.ackedDigest,
        unsyncedOut: Long = this.unsyncedOut,
        syncNonce: ByteArray = this.syncNonce,
        crlEpoch: Long = this.crlEpoch,
        extraRevocations: List<KagemushaAttestedRevocationEntryV1> = this.extraRevocations,
        highWaterMs: Long = this.highWaterMs,
        clockAnomaly: Boolean = this.clockAnomaly,
        lastSyncMs: Long = this.lastSyncMs,
    ) = StateRecord(
        accountId, seq, head, balance, ackedSeq, ackedDigest, unsyncedOut, syncNonce, crlEpoch,
        extraRevocations, highWaterMs, clockAnomaly, lastSyncMs,
    )

    fun encode(): ByteArray = KagemushaAttestedRecords.encode { out ->
        out.string(accountId)
        out.u64(seq)
        out.array(head, 32)
        out.u64(balance)
        out.u64(ackedSeq)
        out.array(ackedDigest, 32)
        out.u64(unsyncedOut)
        out.array(syncNonce, 32)
        out.u64(crlEpoch)
        out.vec(extraRevocations) { element, entry -> entry.write(element) }
        out.u64(highWaterMs)
        out.bool(clockAnomaly)
        out.u64(lastSyncMs)
    }

    companion object {
        fun decode(bytes: ByteArray): StateRecord = KagemushaAttestedRecords.decode(bytes) { input ->
            StateRecord(
                input.string(KagemushaAttestedLimits.MAXIMUM_STRING_BYTES),
                input.u64(),
                input.array(32),
                input.u64(),
                input.u64(),
                input.array(32),
                input.u64(),
                input.array(32),
                input.u64(),
                input.vec(KagemushaAttestedLimits.MAXIMUM_CRL_ENTRIES, KagemushaAttestedRevocationEntryV1::read),
                input.u64(),
                input.bool(),
                input.u64(),
            )
        }
    }
}

/** One own transition; [signature] is null while UNSIGNED (written ahead, not yet signed). */
internal class JournalRecord(val transition: KagemushaAttestedTransitionV1, val signature: ByteArray?) {
    val digest: ByteArray by lazy { transition.digest() }

    fun encode(): ByteArray = KagemushaAttestedRecords.encode { out ->
        out.nested { transition.write(it) }
        out.option(signature) { inner, value -> inner.bareByteArray(value) }
    }

    companion object {
        fun decode(bytes: ByteArray): JournalRecord = KagemushaAttestedRecords.decode(bytes) { input ->
            JournalRecord(input.nested(KagemushaAttestedTransitionV1::read), input.option { it.bareByteArray(64) })
        }
    }
}

/** One outgoing payment. Its SendSplit is kept here after the journal is pruned. */
internal class OutboxRecord(
    val transition: KagemushaAttestedTransitionV1,
    val signature: ByteArray?,
    val receiverPublicKey: ByteArray,
    val delivered: Boolean,
    val ack: ByteArray?,
    val createdAtMs: Long,
) {
    val paymentId: ByteArray by lazy { transition.digest() }

    fun withSignature(value: ByteArray) = OutboxRecord(transition, value, receiverPublicKey, delivered, ack, createdAtMs)

    fun delivered(ackBytes: ByteArray) = OutboxRecord(transition, signature, receiverPublicKey, true, ackBytes, createdAtMs)

    fun encode(): ByteArray = KagemushaAttestedRecords.encode { out ->
        out.nested { transition.write(it) }
        out.option(signature) { inner, value -> inner.bareByteArray(value) }
        out.array(receiverPublicKey, 65)
        out.bool(delivered)
        with(KagemushaAttestedRecords) { out.optionalBlob(ack) }
        out.u64(createdAtMs)
    }

    companion object {
        fun decode(bytes: ByteArray): OutboxRecord = KagemushaAttestedRecords.decode(bytes) { input ->
            OutboxRecord(
                input.nested(KagemushaAttestedTransitionV1::read),
                input.option { it.bareByteArray(64) },
                input.array(65),
                input.bool(),
                with(KagemushaAttestedRecords) { input.optionalBlob(KagemushaAttestedLimits.ACK_BYTES) },
                input.u64(),
            )
        }
    }
}

/** One open payment request with its reserved headroom. */
internal class RequestRecord(val request: KagemushaAttestedPaymentRequestV1, val reserved: Long, val uses: Int) {
    val digest: ByteArray by lazy { request.digest() }

    fun encode(): ByteArray = KagemushaAttestedRecords.encode { out ->
        with(KagemushaAttestedRecords) { out.blob(request.encode()) }
        out.u64(reserved)
        out.u16(uses)
    }

    companion object {
        fun decode(bytes: ByteArray): RequestRecord = KagemushaAttestedRecords.decode(bytes) { input ->
            RequestRecord(
                KagemushaAttestedPaymentRequestV1.decode(with(KagemushaAttestedRecords) { input.blob(KagemushaAttestedLimits.REQUEST_BYTES) }),
                input.u64(),
                input.u16(),
            )
        }
    }
}

/** Status of a payer payment this device has seen. */
internal enum class PeerStatus(val code: Int) {
    CREDITED(1),
    REFUSED(2),
}

/**
 * One authentic payer payment addressed to this device: credited (with its ReceiveFold and Ack)
 * or refused (with the reason). It also feeds the per-payer last-seen and fork index.
 */
internal class PeerRecord(
    val payment: KagemushaAttestedPaymentV1,
    val status: PeerStatus,
    val refusal: KagemushaRefusal?,
    val receiveSeq: Long,
    val ack: ByteArray?,
    val uploaded: Boolean,
    val atMs: Long,
) {
    val paymentId: ByteArray by lazy { payment.paymentId() }

    fun withAck(value: ByteArray) = PeerRecord(payment, status, refusal, receiveSeq, value, uploaded, atMs)

    fun markUploaded() = PeerRecord(payment, status, refusal, receiveSeq, ack, true, atMs)

    fun encode(): ByteArray = KagemushaAttestedRecords.encode { out ->
        with(KagemushaAttestedRecords) { out.blob(payment.encode()) }
        out.u8(status.code)
        out.u8(refusal?.ordinal?.plus(1) ?: 0)
        out.u64(receiveSeq)
        with(KagemushaAttestedRecords) { out.optionalBlob(ack) }
        out.bool(uploaded)
        out.u64(atMs)
    }

    companion object {
        fun decode(bytes: ByteArray): PeerRecord = KagemushaAttestedRecords.decode(bytes) { input ->
            val payment = KagemushaAttestedPaymentV1.decode(with(KagemushaAttestedRecords) { input.blob(KagemushaAttestedLimits.PAYMENT_BYTES) })
            val status = PeerStatus.entries.firstOrNull { it.code == input.u8() } ?: throw IllegalArgumentException("bad peer status")
            val refusalCode = input.u8()
            val refusal = if (refusalCode == 0) null else KagemushaRefusal.entries.getOrNull(refusalCode - 1)
                ?: throw IllegalArgumentException("bad refusal")
            PeerRecord(
                payment,
                status,
                refusal,
                input.u64(),
                with(KagemushaAttestedRecords) { input.optionalBlob(KagemushaAttestedLimits.ACK_BYTES) },
                input.bool(),
                input.u64(),
            )
        }
    }
}

/** One folded mint voucher; its presence makes voucher reuse impossible. */
internal class VoucherRecord(val voucher: KagemushaAttestedMintVoucherV1, val seq: Long) {
    fun encode(): ByteArray = KagemushaAttestedRecords.encode { out ->
        with(KagemushaAttestedRecords) { out.blob(voucher.encode()) }
        out.u64(seq)
    }

    companion object {
        fun decode(bytes: ByteArray): VoucherRecord = KagemushaAttestedRecords.decode(bytes) { input ->
            VoucherRecord(
                KagemushaAttestedMintVoucherV1.decode(with(KagemushaAttestedRecords) { input.blob(KagemushaAttestedLimits.VOUCHER_BYTES) }),
                input.u64(),
            )
        }
    }
}

/** One committed redemption to the enrolled account. */
internal class RedemptionRecord(
    val redemptionId: ByteArray,
    val seq: Long,
    val amount: Long,
    val state: KagemushaRedemptionState,
    val txHash: ByteArray?,
) {
    fun encode(): ByteArray = KagemushaAttestedRecords.encode { out ->
        out.array(redemptionId, 32)
        out.u64(seq)
        out.u64(amount)
        out.u8(state.code)
        out.option(txHash) { inner, value -> inner.bareByteArray(value) }
    }

    companion object {
        fun decode(bytes: ByteArray): RedemptionRecord = KagemushaAttestedRecords.decode(bytes) { input ->
            RedemptionRecord(
                input.array(32),
                input.u64(),
                input.u64(),
                KagemushaRedemptionState.fromCode(input.u8()) ?: throw IllegalArgumentException("bad redemption state"),
                input.option { it.bareByteArray(32) },
            )
        }
    }
}

/** Fork evidence captured by this receiver. */
internal class ForkRecord(val evidence: KagemushaAttestedForkEvidenceV1, val uploaded: Boolean) {
    fun encode(): ByteArray = KagemushaAttestedRecords.encode { out ->
        with(KagemushaAttestedRecords) { out.blob(evidence.encode()) }
        out.bool(uploaded)
    }

    companion object {
        fun decode(bytes: ByteArray): ForkRecord = KagemushaAttestedRecords.decode(bytes) { input ->
            ForkRecord(
                KagemushaAttestedForkEvidenceV1.decode(with(KagemushaAttestedRecords) { input.blob(KagemushaAttestedLimits.FORK_EVIDENCE_BYTES) }),
                input.bool(),
            )
        }
    }
}
