// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.math.BigInteger
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader

/** Typed consumer view of the first-release validator and staking Norito records. */
object ValidatorStakingNoritoV1 {
    private const val FLAGS = NoritoHeader.COMPACT_LEN

    /** Immutable raw payload for cryptographic values with a separately defined codec. */
    class Bytes(value: ByteArray) {
        private val value = value.copyOf()

        fun bytes(): ByteArray = value.copyOf()

        override fun equals(other: Any?): Boolean = other is Bytes && value.contentEquals(other.value)

        override fun hashCode(): Int = value.contentHashCode()
    }

    /** A decoded record re-encodes its exact typed fields with canonical compact lengths. */
    abstract class Record protected constructor(payload: ByteArray, fieldCount: Int) {
        protected val fields: List<ByteArray> = decodeFields(payload, fieldCount)

        fun encode(): ByteArray = encodeFields(fields)

        protected fun raw(index: Int): Bytes = Bytes(fields[index])

        protected fun fixed(index: Int, size: Int): Bytes {
            val bytes = fields[index]
            require(bytes.size == size) { "fixed Norito field has the wrong size" }
            return Bytes(bytes)
        }

        protected fun u16(index: Int): Int = decodeUInt(fields[index], 16).toInt()

        protected fun u32(index: Int): Long = decodeUInt(fields[index], 32)

        protected fun u64(index: Int): Long = decodeUInt(fields[index], 64)

        protected fun network(index: Int): NetworkId = NetworkId.fromBytes(fixed(index, 32).bytes())

        protected fun <T> vector(index: Int, limit: Int, decode: (ByteArray) -> T): List<T> =
            decodeVector(fields[index], limit, decode)

        protected fun <T> option(index: Int, decode: (ByteArray) -> T): T? =
            decodeOption(fields[index], decode)

        protected fun variant(index: Int): Pair<Long, ByteArray> = decodeVariant(fields[index])
    }

    /** One exact BLS consensus identity and paired generation-bound Pasta keys. */
    class ValidatorKeys private constructor(payload: ByteArray) : Record(payload, 3) {
        val validator: Bytes = raw(0)
        val eqProofPublicKey: Bytes = fixed(1, 32)
        val epProofPublicKey: Bytes = fixed(2, 32)

        companion object {
            fun decode(payload: ByteArray): ValidatorKeys = ValidatorKeys(payload)
        }
    }

    /** Immutable key generation; scheduling epochs are authorized separately. */
    class AuthorityGeneration private constructor(payload: ByteArray) : Record(payload, 4) {
        val version: Int = u16(0)
        val networkId: NetworkId = network(1)
        val generation: Long = u64(2)
        val validators: List<ValidatorKeys> = vector(3, 31, ValidatorKeys::decode)

        init {
            require(version == 1) { "unsupported authority-generation version" }
            require(validators.size >= 4 && (validators.size - 1) % 3 == 0) {
                "authority generation requires an exact 3f + 1 roster"
            }
        }

        companion object {
            fun decode(payload: ByteArray): AuthorityGeneration = AuthorityGeneration(payload)
        }
    }

    /** Installed DKG session and exact public transcript commitment. */
    class InstalledBeacon private constructor(payload: ByteArray) : Record(payload, 2) {
        val sessionId: Bytes = fixed(0, 32)
        val transcriptHash: Bytes = fixed(1, 32)

        companion object {
            fun decode(payload: ByteArray): InstalledBeacon = InstalledBeacon(payload)
        }
    }

    /** Authorization of one scheduling epoch under a separately retained generation. */
    class EpochAuthorization private constructor(payload: ByteArray) : Record(payload, 11) {
        val version: Int = u16(0)
        val networkId: NetworkId = network(1)
        val epoch: Long = u64(2)
        val firstHeight: Long = u64(3)
        val lastHeight: Long = u64(4)
        val authorityGeneration: Long = u64(5)
        val authorityId: Bytes = fixed(6, 32)
        val beacon: InstalledBeacon?
        val previousAuthorizationId: Bytes = fixed(8, 32)
        val transitionId: Bytes = fixed(9, 32)
        val decision: Decision

        init {
            require(version == 1 && firstHeight > 0 && lastHeight >= firstHeight) {
                "invalid epoch authorization interval"
            }
            val binding = variant(7)
            beacon = when (binding.first) {
                0L -> {
                    require(fields[7].size == 4) { "bootstrap beacon must be a unit variant" }
                    null
                }
                1L -> InstalledBeacon.decode(binding.second)
                else -> throw IllegalArgumentException("unknown beacon epoch binding")
            }
            val disposition = variant(10)
            require(fields[10].size == 4) { "epoch decision must be a unit variant" }
            decision = Decision.entries.firstOrNull { it.tag == disposition.first }
                ?: throw IllegalArgumentException("unknown epoch decision")
        }

        enum class Decision(val tag: Long) {
            GENESIS(0), ACTIVATE(1), RETAIN(2), RETAIN_AND_CANCEL(3),
        }

        companion object {
            fun decode(payload: ByteArray): EpochAuthorization = EpochAuthorization(payload)
        }
    }

    /** Frozen attempt, generation and the three exclusive DKG cutoffs. */
    class DkgSession private constructor(payload: ByteArray) : Record(payload, 12) {
        val version: Int = u16(0)
        val networkId: NetworkId = network(1)
        val sessionId: Bytes = fixed(2, 32)
        val attemptId: Bytes = fixed(3, 32)
        val authorityGeneration: Long = u64(4)
        val rosterHash: Bytes = fixed(5, 32)
        val committeeSize: Int = u16(6)
        val threshold: Int = u16(7)
        val startHeight: Long = u64(8)
        val commitmentsEndHeight: Long = u64(9)
        val deliveriesEndHeight: Long = u64(10)
        val acceptancesEndHeight: Long = u64(11)

        init {
            require(version == 1 && committeeSize >= 4 && (committeeSize - 1) % 3 == 0) {
                "invalid DKG committee geometry"
            }
            require(threshold == (committeeSize - 1) / 3 + 1) { "invalid DKG threshold" }
            require(startHeight < commitmentsEndHeight &&
                commitmentsEndHeight < deliveriesEndHeight &&
                deliveriesEndHeight < acceptancesEndHeight) {
                "invalid DKG cutoffs"
            }
        }

        companion object {
            fun decode(payload: ByteArray): DkgSession = DkgSession(payload)
        }
    }

    /** Attempt-bound hybrid encryption key signed by its exact recipient seat. */
    class DkgRecipientKey private constructor(payload: ByteArray) : Record(payload, 5) {
        val recipientIndex: Int = u16(0)
        val validator: Bytes = raw(1)
        val x25519PublicKey: Bytes = fixed(2, 32)
        val mlkem768PublicKey: Bytes = decodeByteVector(fields[3])
        val signature: Bytes = raw(4)

        companion object {
            fun decode(payload: ByteArray): DkgRecipientKey = DkgRecipientKey(payload)
        }
    }

    /** Dealer-signed ciphertext for one private contribution. */
    class DkgEncryptedShare private constructor(payload: ByteArray) : Record(payload, 9) {
        val dealerIndex: Int = u16(0)
        val recipientIndex: Int = u16(1)
        val dealerCommitmentHash: Bytes = fixed(2, 32)
        val recipientKeyHash: Bytes = fixed(3, 32)
        val deliveryHeight: Long = u64(4)
        val ephemeralX25519PublicKey: Bytes = fixed(5, 32)
        val mlkem768Ciphertext: Bytes = decodeByteVector(fields[6])
        val encryptedShare: Bytes = decodeByteVector(fields[7])
        val signature: Bytes = raw(8)

        companion object {
            fun decode(payload: ByteArray): DkgEncryptedShare = DkgEncryptedShare(payload)
        }
    }

    /** Recipient-signed acknowledgement of an exact encrypted edge. */
    class DkgShareAcceptance private constructor(payload: ByteArray) : Record(payload, 6) {
        val dealerIndex: Int = u16(0)
        val recipientIndex: Int = u16(1)
        val dealerCommitmentHash: Bytes = fixed(2, 32)
        val encryptedShareHash: Bytes = fixed(3, 32)
        val acceptedHeight: Long = u64(4)
        val signature: Bytes = raw(5)

        companion object {
            fun decode(payload: ByteArray): DkgShareAcceptance = DkgShareAcceptance(payload)
        }
    }

    /** Signed public dealer commitment and knowledge proof. */
    class DkgDealerCommitment private constructor(payload: ByteArray) : Record(payload, 4) {
        val dealerIndex: Int = u16(0)
        val coefficientCommitments: List<Bytes> = vector(1, 32) { bytes ->
            decodeFixedByteArray(bytes, 96)
        }
        val constantTermProof: DkgConstantProof = DkgConstantProof.decode(fields[2])
        val signature: Bytes = raw(3)

        companion object {
            fun decode(payload: ByteArray): DkgDealerCommitment = DkgDealerCommitment(payload)
        }
    }

    /** Exact canonical widths of the dealer's constant-term knowledge proof. */
    class DkgConstantProof private constructor(payload: ByteArray) : Record(payload, 2) {
        val commitment: Bytes = fixed(0, 96)
        val response: Bytes = fixed(1, 32)

        companion object {
            fun decode(payload: ByteArray): DkgConstantProof = DkgConstantProof(payload)
        }
    }

    /** Complete signed public all-edge ceremony transcript. */
    class DkgTranscript private constructor(payload: ByteArray) : Record(payload, 10) {
        val session: DkgSession = DkgSession.decode(fields[0])
        val generatorH: Bytes = fixed(1, 96)
        val generatorV: Bytes = fixed(2, 96)
        val dealerCommitments: List<DkgDealerCommitment> =
            vector(3, 31, DkgDealerCommitment::decode)
        val recipientKeys: List<DkgRecipientKey> = vector(4, 31, DkgRecipientKey::decode)
        val encryptedShares: List<DkgEncryptedShare> = vector(5, 31 * 31, DkgEncryptedShare::decode)
        val shareAcceptances: List<DkgShareAcceptance> =
            vector(6, 31 * 31, DkgShareAcceptance::decode)
        val qualifiedDealers: List<Int> = vector(7, 31) { decodeUInt(it, 16).toInt() }
        val eventHash: Bytes = fixed(8, 32)
        val finalizedAtHeight: Long = u64(9)

        companion object {
            fun decode(payload: ByteArray): DkgTranscript = DkgTranscript(payload)
        }
    }

    /** Exact candidate in a frozen ordered committee. */
    class ValidatorPower private constructor(payload: ByteArray) : Record(payload, 2) {
        val validator: Bytes = raw(0)
        val power: Long = u64(1)

        companion object {
            fun decode(payload: ByteArray): ValidatorPower = ValidatorPower(payload)
        }
    }

    /** Authenticated election inputs for E+2, frozen at the E boundary. */
    class CommitteePreparation private constructor(payload: ByteArray) : Record(payload, 13) {
        val version: Int = u16(0)
        val networkId: NetworkId = network(1)
        val selectionEpoch: Long = u64(2)
        val selectionHeight: Long = u64(3)
        val selectionAnchor: Bytes = fixed(4, 32)
        val targetEpoch: Long = u64(5)
        val firstHeight: Long = u64(6)
        val lastHeight: Long = u64(7)
        val authorityGeneration: Long = u64(8)
        val preparingAuthorizationId: Bytes = fixed(9, 32)
        val electionSeed: Bytes = fixed(10, 32)
        val roster: List<ValidatorPower> = vector(11, 31, ValidatorPower::decode)
        val validatorSetPops: List<Bytes> = vector(12, 31) { decodeByteVector(it) }

        init {
            require(version == 1 && targetEpoch == selectionEpoch + 2) {
                "invalid E+2 committee preparation"
            }
            require(roster.size >= 4 && (roster.size - 1) % 3 == 0) {
                "invalid frozen committee geometry"
            }
            require(roster.size == validatorSetPops.size) { "missing candidate possession proof" }
        }

        companion object {
            fun decode(payload: ByteArray): CommitteePreparation = CommitteePreparation(payload)
        }
    }

    /** Exact generation and installed beacon transcript prepared for activation. */
    class CommitteeCredentials private constructor(payload: ByteArray) : Record(payload, 2) {
        val authority: AuthorityGeneration = AuthorityGeneration.decode(fields[0])
        val beacon: InstalledBeacon = InstalledBeacon.decode(fields[1])

        companion object {
            fun decode(payload: ByteArray): CommitteeCredentials = CommitteeCredentials(payload)
        }
    }

    /** Seat-indexed proof payloads retained until the certified boundary. */
    class SeatReadiness private constructor(payload: ByteArray) : Record(payload, 3) {
        val validatorIndex: Long = u32(0)
        val pastaPossession: Bytes = raw(1)
        val beaconPossession: Bytes = raw(2)

        companion object {
            fun decode(payload: ByteArray): SeatReadiness = SeatReadiness(payload)
        }
    }

    /** Atomic preparation, readiness and certified outcome record. */
    class CommitteeTransition private constructor(payload: ByteArray) : Record(payload, 4) {
        val preparation: CommitteePreparation = CommitteePreparation.decode(fields[0])
        val credentials: CommitteeCredentials? = option(1, CommitteeCredentials::decode)
        val readiness: List<SeatReadiness> = vector(2, 31, SeatReadiness::decode)
        val outcome: EpochAuthorization? = option(3, EpochAuthorization::decode)

        companion object {
            fun decode(payload: ByteArray): CommitteeTransition = CommitteeTransition(payload)
        }
    }

    /** Exact asset balance bucket, including its dataspace scope. */
    class AssetId private constructor(payload: ByteArray) : Record(payload, 3) {
        val account: Bytes = raw(0)
        val definition: Bytes = decodeFixedByteArray(fields[1], 16)
        val scopeDataspace: Long?

        init {
            val scope = variant(2)
            scopeDataspace = when (scope.first) {
                0L -> {
                    require(fields[2].size == 4) { "global asset scope must be a unit variant" }
                    null
                }
                1L -> decodeUInt(scope.second, 64)
                else -> throw IllegalArgumentException("unknown asset balance scope")
            }
        }

        companion object {
            fun decode(payload: ByteArray): AssetId = AssetId(payload)
        }
    }

    /** Canonical nonnegative fixed-point quantity. */
    class Quantity private constructor(payload: ByteArray) : Record(payload, 2) {
        val mantissa: BigInteger
        val scale: Long = u32(1)

        init {
            val decoder = NoritoDecoder(fields[0], FLAGS)
            val byteCount = decoder.readUInt(32)
            require(byteCount <= 64) { "quantity mantissa exceeds 512 bits" }
            val value = decoder.readBytes(byteCount.toInt())
            require(decoder.remaining() == 0) {
                "quantity mantissa has trailing bytes"
            }
            mantissa = if (value.isEmpty()) BigInteger.ZERO else BigInteger(value.reversedArray())
            val canonical = if (mantissa.signum() == 0) {
                byteArrayOf()
            } else {
                mantissa.toByteArray().reversedArray()
            }
            require(mantissa.signum() >= 0 &&
                canonical.contentEquals(value)) {
                "quantity mantissa is not canonical and nonnegative"
            }
            require(scale <= 28 &&
                (scale == 0L || (mantissa.signum() > 0 && mantissa.mod(BigInteger.TEN) != BigInteger.ZERO))) {
                "quantity has a non-canonical decimal scale"
            }
        }

        companion object {
            fun decode(payload: ByteArray): Quantity = Quantity(payload)
        }
    }

    /** One exact operation-specific monetary precondition. */
    class MonetaryPrecondition private constructor(payload: ByteArray) {
        private val encoded = payload.copyOf()
        val kind: Kind
        val activationHeight: Long
        val binding: Bytes?
        val slashableExposure: Quantity?

        init {
            val (tag, value) = decodeVariant(encoded)
            kind = Kind.entries.firstOrNull { it.tag == tag }
                ?: throw IllegalArgumentException("unknown staking monetary precondition")
            val fields = decodeFields(value, if (kind == Kind.REGISTRATION) 1 else 2)
            activationHeight = decodeUInt(fields[0], 64)
            binding = if (kind == Kind.BOND || kind == Kind.UNBOND) Bytes(fields[1]) else null
            slashableExposure = if (kind == Kind.SLASH) Quantity.decode(fields[1]) else null
        }

        fun encode(): ByteArray = encoded.copyOf()

        enum class Kind(val tag: Long) {
            REGISTRATION(0), BOND(1), UNBOND(2), SLASH(3),
        }

        companion object {
            fun decode(payload: ByteArray): MonetaryPrecondition = MonetaryPrecondition(payload)
        }
    }

    /** Signed exact source, destination, quantity and retained-state staking plan. */
    class MonetaryPlan private constructor(payload: ByteArray) : Record(payload, 6) {
        val networkScope: NetworkId?
        val validUntilHeight: Long = u64(1)
        val sourceAsset: AssetId = AssetId.decode(fields[2])
        val destinationAsset: AssetId = AssetId.decode(fields[3])
        val amount: Quantity = Quantity.decode(fields[4])
        val precondition: MonetaryPrecondition = MonetaryPrecondition.decode(fields[5])

        init {
            val scope = variant(0)
            networkScope = when (scope.first) {
                0L -> {
                    require(fields[0].size == 4) { "genesis monetary scope must be a unit variant" }
                    null
                }
                1L -> NetworkId.fromBytes(scope.second)
                else -> throw IllegalArgumentException("unknown staking monetary scope")
            }
            require(validUntilHeight > 0 && amount.mantissa.signum() > 0) {
                "invalid staking monetary plan"
            }
            require(sourceAsset.definition == destinationAsset.definition &&
                sourceAsset.scopeDataspace == destinationAsset.scopeDataspace) {
                "staking transfer changes asset definition or scope"
            }
        }

        companion object {
            fun decode(payload: ByteArray): MonetaryPlan = MonetaryPlan(payload)
        }
    }

    /** Validator rebind with mandatory replacement-peer consent. */
    class RebindPeer private constructor(payload: ByteArray) : Record(payload, 4) {
        // LaneId is a one-field Norito newtype around the u32 lane number.
        val laneId: Long = decodeUInt(decodeFields(fields[0], 1)[0], 32)
        val validator: Bytes = raw(1)
        val peerId: Bytes = raw(2)
        val peerSignature: Bytes = raw(3)

        init {
            require(peerSignature.bytes().isNotEmpty()) { "replacement peer consent is required" }
        }

        companion object {
            fun decode(payload: ByteArray): RebindPeer = RebindPeer(payload)
        }
    }

    private fun decodeFields(payload: ByteArray, count: Int): List<ByteArray> {
        val decoder = NoritoDecoder(payload, FLAGS)
        val fields = List(count) {
            val length = decoder.readLength(true)
            decoder.readBytes(length.toInt())
        }
        require(decoder.remaining() == 0) { "Norito record contains trailing fields" }
        require(encodeFields(fields).contentEquals(payload)) {
            "Norito record contains non-canonical field lengths"
        }
        return fields
    }

    private fun encodeFields(fields: List<ByteArray>): ByteArray {
        val encoder = NoritoEncoder(FLAGS)
        fields.forEach { field ->
            encoder.writeLength(field.size.toLong(), true)
            encoder.writeBytes(field)
        }
        return encoder.toByteArray()
    }

    private fun decodeUInt(payload: ByteArray, bits: Int): Long {
        val decoder = NoritoDecoder(payload, FLAGS)
        val value = decoder.readUInt(bits)
        require(decoder.remaining() == 0) { "integer field contains trailing bytes" }
        return value
    }

    private fun decodeByteVector(payload: ByteArray): Bytes {
        val decoder = NoritoDecoder(payload, FLAGS)
        val length = decoder.readUInt(64)
        require(length in 0..Int.MAX_VALUE.toLong()) { "byte vector exceeds SDK bound" }
        val value = decoder.readBytes(length.toInt())
        require(decoder.remaining() == 0) { "byte vector contains trailing bytes" }
        return Bytes(value)
    }

    private fun decodeFixedByteArray(payload: ByteArray, count: Int): Bytes {
        val decoder = NoritoDecoder(payload, FLAGS)
        val bytes = ByteArray(count) {
            require(decoder.readLength(true) == 1L) { "non-canonical fixed byte array" }
            decoder.readByte().toByte()
        }
        require(decoder.remaining() == 0) { "fixed byte array contains trailing bytes" }
        return Bytes(bytes)
    }

    private fun <T> decodeVector(
        payload: ByteArray,
        limit: Int,
        decode: (ByteArray) -> T,
    ): List<T> {
        val decoder = NoritoDecoder(payload, FLAGS)
        val count = decoder.readUInt(64)
        require(count in 0..limit.toLong()) { "Norito vector exceeds protocol bound" }
        val values = List(count.toInt()) {
            val length = decoder.readLength(true)
            decode(decoder.readBytes(length.toInt()))
        }
        require(decoder.remaining() == 0) { "Norito vector contains trailing bytes" }
        return values
    }

    private fun <T> decodeOption(payload: ByteArray, decode: (ByteArray) -> T): T? {
        val decoder = NoritoDecoder(payload, FLAGS)
        val tag = decoder.readByte()
        val value = when (tag) {
            0 -> null
            1 -> {
                val length = decoder.readLength(true)
                decode(decoder.readBytes(length.toInt()))
            }
            else -> throw IllegalArgumentException("unknown Norito option tag")
        }
        require(decoder.remaining() == 0) { "Norito option contains trailing bytes" }
        return value
    }

    private fun decodeVariant(payload: ByteArray): Pair<Long, ByteArray> {
        val decoder = NoritoDecoder(payload, FLAGS)
        val tag = decoder.readUInt(32)
        val value = if (decoder.remaining() == 0) {
            byteArrayOf()
        } else {
            val length = decoder.readLength(true)
            decoder.readBytes(length.toInt())
        }
        require(decoder.remaining() == 0) { "Norito variant contains trailing bytes" }
        return tag to value
    }
}
