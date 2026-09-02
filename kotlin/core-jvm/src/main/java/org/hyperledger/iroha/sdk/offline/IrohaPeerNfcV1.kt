package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream

/** Transport-neutral NFC V1 constants. No legacy AID or codec negotiation is supported. */
object IrohaPeerNfcV1 {
    private val applicationIdentifierBytes = byteArrayOf(
        0xf0.toByte(), 0x50, 0x4b, 0x45, 0x50, 0x4b, 0x52, 0x4e, 0x46, 0x43, 0x01,
    )
    const val APPLICATION_IDENTIFIER_HEX = "F0504B45504B524E464301"
    const val APPLICATION_IDENTIFIER_SIZE = 11
    const val COMMAND_CLASS = 0x80
    const val WIRE_VERSION = 1
    const val SESSION_ID_BYTES = 16
    const val HASH_BYTES = 32
    const val MAXIMUM_CHUNK_BYTES = 4_096
    const val MAXIMUM_MESSAGE_BYTES =
        IrohaPeerWireMessageV1.HEADER_LENGTH + IrohaPeerWireMessageV1.MAXIMUM_OFFLINE_CASH_ENCODED_BYTES
    const val INFO_BYTES = 98
    const val STATUS_BYTES = 174
    const val PAYMENT_ADMISSION_BYTES = 244

    internal val INFO_MAGIC = "INF1".toByteArray(Charsets.US_ASCII)
    internal val STATUS_MAGIC = "NST1".toByteArray(Charsets.US_ASCII)
    internal val PAYMENT_ADMISSION_MAGIC = "IPA1".toByteArray(Charsets.US_ASCII)
    internal val DURABLE_ACK_MAGIC = "IDA1".toByteArray(Charsets.US_ASCII)
    internal val SENDER_CHECKPOINT_MAGIC = "ISC1".toByteArray(Charsets.US_ASCII)

    /** Returns a defensive copy of the sole first-release ISO 7816 AID. */
    @JvmStatic fun applicationIdentifier(): ByteArray = applicationIdentifierBytes.copyOf()

    /** Compares without exposing the process-wide backing bytes. */
    @JvmStatic fun matchesApplicationIdentifier(candidate: ByteArray): Boolean =
        candidate.contentEquals(applicationIdentifierBytes)
}

enum class IrohaPeerNfcInstructionV1(val code: Int) {
    GET_INFO(0x10),
    READ_REQUEST(0x11),
    BEGIN_PAYMENT(0x20),
    WRITE(0x21),
    COMMIT(0x22),
    READ_ACKNOWLEDGEMENT(0x23),
    CONFIRM_ACKNOWLEDGEMENT(0x24),
    GET_STATUS(0x25);

    companion object {
        @JvmStatic fun fromCode(code: Int): IrohaPeerNfcInstructionV1? =
            entries.firstOrNull { it.code == code }
    }
}

enum class IrohaPeerNfcPhaseV1(val code: Int) {
    REQUEST_READY(1),
    PAYMENT_RECEIVING(2),
    ACKNOWLEDGEMENT_READY(3),
    COMPLETE(4);

    companion object {
        @JvmStatic fun fromCode(code: Int): IrohaPeerNfcPhaseV1? =
            entries.firstOrNull { it.code == code }
    }
}

class IrohaPeerNfcFlagsV1(val rawValue: Int) {
    init { require(rawValue in 0..KNOWN) { "Invalid NFC flags" } }
    fun contains(flag: Int): Boolean = rawValue and flag == flag
    override fun equals(other: Any?): Boolean = other is IrohaPeerNfcFlagsV1 && rawValue == other.rawValue
    override fun hashCode(): Int = rawValue

    companion object {
        const val IDEMPOTENT_WRITES = 1
        const val DURABLE_ACKNOWLEDGEMENT = 2
        const val KNOWN = IDEMPOTENT_WRITES or DURABLE_ACKNOWLEDGEMENT
        @JvmField val REQUEST = IrohaPeerNfcFlagsV1(IDEMPOTENT_WRITES)
        @JvmField val DURABLE = IrohaPeerNfcFlagsV1(KNOWN)
    }
}

class IrohaPeerNfcLimitsV1 @JvmOverloads constructor(
    val maximumMessageBytes: Int = IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES,
    val maximumReadChunkBytes: Int = IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES,
    val maximumWriteChunkBytes: Int = IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES,
) {
    init {
        require(
            maximumMessageBytes in
                IrohaPeerWireMessageV1.HEADER_LENGTH..IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES
        )
        require(maximumReadChunkBytes in 1..IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES)
        require(maximumWriteChunkBytes in 1..IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES)
    }

    companion object { @JvmField val DEFAULT = IrohaPeerNfcLimitsV1() }
}

/** One immutable profile shared by request, payment, and acknowledgement. */
class IrohaPeerNfcProfilePolicyV1(
    val profile: IrohaPeerPayloadProfile,
) {
    fun accepts(candidate: IrohaPeerPayloadProfile): Boolean = profile == candidate

    override fun equals(other: Any?): Boolean =
        other is IrohaPeerNfcProfilePolicyV1 && profile == other.profile

    override fun hashCode(): Int = profile.hashCode()
}

class IrohaPeerNfcRequestIdentityV1(
    val profile: IrohaPeerPayloadProfile,
    sessionId: ByteArray,
    requestCanonicalHash: ByteArray,
    requestWireHash: ByteArray,
) {
    private val session = sessionId.copyOf()
    private val canonicalHash = requestCanonicalHash.copyOf()
    private val wireHash = requestWireHash.copyOf()
    val sessionId: ByteArray get() = session.copyOf()
    val requestCanonicalHash: ByteArray get() = canonicalHash.copyOf()
    val requestWireHash: ByteArray get() = wireHash.copyOf()

    init {
        requireNfcSession(session)
        requireNfcHash(canonicalHash)
        requireNfcHash(wireHash)
    }

    override fun equals(other: Any?): Boolean = other is IrohaPeerNfcRequestIdentityV1 &&
        profile == other.profile && session.contentEquals(other.session) &&
        canonicalHash.contentEquals(other.canonicalHash) && wireHash.contentEquals(other.wireHash)

    override fun hashCode(): Int = 31 * profile.hashCode() + session.contentHashCode()
}

/** Fixed 98-byte INF1 response. */
class IrohaPeerNfcInfoV1(
    val phase: IrohaPeerNfcPhaseV1,
    val flags: IrohaPeerNfcFlagsV1,
    val identity: IrohaPeerNfcRequestIdentityV1,
    val requestLength: Int,
    val maximumReadChunkBytes: Int,
    val maximumWriteChunkBytes: Int,
) {
    init {
        require(requestLength > IrohaPeerWireMessageV1.HEADER_LENGTH &&
            requestLength <= IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES) { "Invalid NFC request length" }
        requireNfcChunkLimit(maximumReadChunkBytes)
        requireNfcChunkLimit(maximumWriteChunkBytes)
        val mustBeDurable = phase == IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY ||
            phase == IrohaPeerNfcPhaseV1.COMPLETE
        require(flags.contains(IrohaPeerNfcFlagsV1.IDEMPOTENT_WRITES) &&
            flags.contains(IrohaPeerNfcFlagsV1.DURABLE_ACKNOWLEDGEMENT) == mustBeDurable) {
            "Invalid NFC info flags"
        }
    }

    fun encode(): ByteArray = ByteArrayOutputStream(IrohaPeerNfcV1.INFO_BYTES).also { output ->
        output.write(IrohaPeerNfcV1.INFO_MAGIC)
        output.write(IrohaPeerNfcV1.WIRE_VERSION)
        output.write(phase.code)
        output.nfcWriteU16(identity.profile.code)
        output.write(flags.rawValue)
        output.write(0)
        output.write(identity.sessionId)
        output.nfcWriteU32(requestLength.toLong())
        output.write(identity.requestCanonicalHash)
        output.write(identity.requestWireHash)
        output.nfcWriteU16(maximumReadChunkBytes)
        output.nfcWriteU16(maximumWriteChunkBytes)
    }.toByteArray().also { check(it.size == IrohaPeerNfcV1.INFO_BYTES) }

    override fun equals(other: Any?): Boolean = other is IrohaPeerNfcInfoV1 &&
        phase == other.phase && flags == other.flags && identity == other.identity &&
        requestLength == other.requestLength && maximumReadChunkBytes == other.maximumReadChunkBytes &&
        maximumWriteChunkBytes == other.maximumWriteChunkBytes

    override fun hashCode(): Int = 31 * phase.hashCode() + identity.hashCode()

    companion object {
        @JvmStatic fun decode(data: ByteArray): IrohaPeerNfcInfoV1 {
            require(data.size == IrohaPeerNfcV1.INFO_BYTES &&
                data.copyOfRange(0, 4).contentEquals(IrohaPeerNfcV1.INFO_MAGIC) &&
                data[4].toInt() and 0xff == IrohaPeerNfcV1.WIRE_VERSION && data[9].toInt() == 0) {
                "Malformed INF1 record"
            }
            val phase = IrohaPeerNfcPhaseV1.fromCode(data[5].toInt() and 0xff)
                ?: throw IllegalArgumentException("Invalid NFC phase")
            val profile = IrohaPeerPayloadProfile.fromCode(data.nfcReadU16(6))
                ?: throw IllegalArgumentException("Invalid NFC profile")
            return IrohaPeerNfcInfoV1(
                phase,
                IrohaPeerNfcFlagsV1(data[8].toInt() and 0xff),
                IrohaPeerNfcRequestIdentityV1(
                    profile,
                    data.copyOfRange(10, 26),
                    data.copyOfRange(30, 62),
                    data.copyOfRange(62, 94),
                ),
                data.nfcReadU32(26).nfcCheckedInt(),
                data.nfcReadU16(94),
                data.nfcReadU16(96),
            )
        }
    }
}

/** Fixed 174-byte NST1 response with independently typed payment and ACK profiles. */
class IrohaPeerNfcStatusV1(
    val phase: IrohaPeerNfcPhaseV1,
    val flags: IrohaPeerNfcFlagsV1,
    val identity: IrohaPeerNfcRequestIdentityV1,
    val paymentProfile: IrohaPeerPayloadProfile?,
    val paymentLength: Int,
    val receivedPaymentBytes: Int,
    paymentWireHash: ByteArray,
    val acknowledgementProfile: IrohaPeerPayloadProfile?,
    val acknowledgementLength: Int,
    acknowledgementWireHash: ByteArray,
    val maximumReadChunkBytes: Int,
    val maximumWriteChunkBytes: Int,
) {
    private val paymentHash = paymentWireHash.copyOf()
    private val acknowledgementHash = acknowledgementWireHash.copyOf()
    val paymentWireHash: ByteArray get() = paymentHash.copyOf()
    val acknowledgementWireHash: ByteArray get() = acknowledgementHash.copyOf()

    init {
        require(flags.contains(IrohaPeerNfcFlagsV1.IDEMPOTENT_WRITES)) { "Invalid NFC status flags" }
        requireNfcChunkLimit(maximumReadChunkBytes)
        requireNfcChunkLimit(maximumWriteChunkBytes)
        val zero = ByteArray(32)
        val valid = when (phase) {
            IrohaPeerNfcPhaseV1.REQUEST_READY -> paymentProfile == null && paymentLength == 0 &&
                receivedPaymentBytes == 0 && paymentHash.contentEquals(zero) &&
                acknowledgementProfile == null && acknowledgementLength == 0 &&
                acknowledgementHash.contentEquals(zero) &&
                !flags.contains(IrohaPeerNfcFlagsV1.DURABLE_ACKNOWLEDGEMENT)
            IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING -> paymentProfile != null &&
                paymentLength in (IrohaPeerWireMessageV1.HEADER_LENGTH + 1)..IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES &&
                receivedPaymentBytes in 0..paymentLength && paymentHash.size == 32 &&
                !paymentHash.contentEquals(zero) && acknowledgementProfile == null &&
                acknowledgementLength == 0 && acknowledgementHash.contentEquals(zero) &&
                !flags.contains(IrohaPeerNfcFlagsV1.DURABLE_ACKNOWLEDGEMENT)
            IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, IrohaPeerNfcPhaseV1.COMPLETE ->
                paymentProfile != null && acknowledgementProfile != null &&
                    paymentLength in (IrohaPeerWireMessageV1.HEADER_LENGTH + 1)..IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES &&
                    receivedPaymentBytes == paymentLength && paymentHash.size == 32 &&
                    !paymentHash.contentEquals(zero) &&
                    acknowledgementLength in (IrohaPeerWireMessageV1.HEADER_LENGTH + 1)..IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES &&
                    acknowledgementHash.size == 32 && !acknowledgementHash.contentEquals(zero) &&
                    flags.contains(IrohaPeerNfcFlagsV1.DURABLE_ACKNOWLEDGEMENT)
        }
        require(valid) { "Invalid NFC status fields" }
    }

    fun encode(): ByteArray = ByteArrayOutputStream(IrohaPeerNfcV1.STATUS_BYTES).also { output ->
        output.write(IrohaPeerNfcV1.STATUS_MAGIC)
        output.write(IrohaPeerNfcV1.WIRE_VERSION)
        output.write(phase.code)
        output.nfcWriteU16(identity.profile.code)
        output.write(flags.rawValue)
        output.write(0)
        output.write(identity.sessionId)
        output.write(identity.requestCanonicalHash)
        output.write(identity.requestWireHash)
        output.nfcWriteU16(paymentProfile?.code ?: 0)
        output.nfcWriteU32(paymentLength.toLong())
        output.nfcWriteU32(receivedPaymentBytes.toLong())
        output.write(paymentHash)
        output.nfcWriteU16(acknowledgementProfile?.code ?: 0)
        output.nfcWriteU32(acknowledgementLength.toLong())
        output.write(acknowledgementHash)
        output.nfcWriteU16(maximumReadChunkBytes)
        output.nfcWriteU16(maximumWriteChunkBytes)
    }.toByteArray().also { check(it.size == IrohaPeerNfcV1.STATUS_BYTES) }

    override fun equals(other: Any?): Boolean = other is IrohaPeerNfcStatusV1 &&
        phase == other.phase && flags == other.flags && identity == other.identity &&
        paymentProfile == other.paymentProfile && paymentLength == other.paymentLength &&
        receivedPaymentBytes == other.receivedPaymentBytes && paymentHash.contentEquals(other.paymentHash) &&
        acknowledgementProfile == other.acknowledgementProfile &&
        acknowledgementLength == other.acknowledgementLength &&
        acknowledgementHash.contentEquals(other.acknowledgementHash) &&
        maximumReadChunkBytes == other.maximumReadChunkBytes &&
        maximumWriteChunkBytes == other.maximumWriteChunkBytes

    override fun hashCode(): Int = 31 * phase.hashCode() + paymentHash.contentHashCode()

    companion object {
        @JvmStatic fun decode(data: ByteArray): IrohaPeerNfcStatusV1 {
            require(data.size == IrohaPeerNfcV1.STATUS_BYTES &&
                data.copyOfRange(0, 4).contentEquals(IrohaPeerNfcV1.STATUS_MAGIC) &&
                data[4].toInt() and 0xff == IrohaPeerNfcV1.WIRE_VERSION && data[9].toInt() == 0) {
                "Malformed NST1 record"
            }
            val phase = IrohaPeerNfcPhaseV1.fromCode(data[5].toInt() and 0xff)
                ?: throw IllegalArgumentException("Invalid NFC phase")
            val profile = IrohaPeerPayloadProfile.fromCode(data.nfcReadU16(6))
                ?: throw IllegalArgumentException("Invalid NFC request profile")
            val paymentCode = data.nfcReadU16(90)
            val acknowledgementCode = data.nfcReadU16(132)
            val paymentProfile = if (paymentCode == 0) null else IrohaPeerPayloadProfile.fromCode(paymentCode)
            val acknowledgementProfile = if (acknowledgementCode == 0) null
            else IrohaPeerPayloadProfile.fromCode(acknowledgementCode)
            require(paymentCode == 0 || paymentProfile != null)
            require(acknowledgementCode == 0 || acknowledgementProfile != null)
            return IrohaPeerNfcStatusV1(
                phase,
                IrohaPeerNfcFlagsV1(data[8].toInt() and 0xff),
                IrohaPeerNfcRequestIdentityV1(
                    profile,
                    data.copyOfRange(10, 26),
                    data.copyOfRange(26, 58),
                    data.copyOfRange(58, 90),
                ),
                paymentProfile,
                data.nfcReadU32(92).nfcCheckedInt(),
                data.nfcReadU32(96).nfcCheckedInt(),
                data.copyOfRange(100, 132),
                acknowledgementProfile,
                data.nfcReadU32(134).nfcCheckedInt(),
                data.copyOfRange(138, 170),
                data.nfcReadU16(170),
                data.nfcReadU16(172),
            )
        }
    }
}

enum class IrohaPeerNfcCommandTypeV1 {
    SELECT_APPLICATION,
    GET_INFO,
    READ_REQUEST,
    BEGIN_PAYMENT,
    WRITE,
    COMMIT,
    READ_ACKNOWLEDGEMENT,
    CONFIRM_ACKNOWLEDGEMENT,
    GET_STATUS,
}

/** Immutable typed command; constructors below enforce all fixed field widths. */
class IrohaPeerNfcCommandV1 private constructor(
    val type: IrohaPeerNfcCommandTypeV1,
    sessionId: ByteArray? = null,
    firstHash: ByteArray? = null,
    secondHash: ByteArray? = null,
    val offset: Long = 0,
    val length: Int = 0,
    bytes: ByteArray = byteArrayOf(),
) {
    private val commandSession = sessionId?.copyOf()
    private val commandFirstHash = firstHash?.copyOf()
    private val commandSecondHash = secondHash?.copyOf()
    private val commandBytes = bytes.copyOf()
    val sessionId: ByteArray? get() = commandSession?.copyOf()
    val firstHash: ByteArray? get() = commandFirstHash?.copyOf()
    val secondHash: ByteArray? get() = commandSecondHash?.copyOf()
    val bytes: ByteArray get() = commandBytes.copyOf()

    override fun equals(other: Any?): Boolean = other is IrohaPeerNfcCommandV1 &&
        type == other.type && commandSession.contentEqualsNfc(other.commandSession) &&
        commandFirstHash.contentEqualsNfc(other.commandFirstHash) &&
        commandSecondHash.contentEqualsNfc(other.commandSecondHash) && offset == other.offset &&
        length == other.length && commandBytes.contentEquals(other.commandBytes)

    override fun hashCode(): Int = 31 * type.hashCode() + commandBytes.contentHashCode()

    companion object {
        @JvmField val SELECT_APPLICATION = IrohaPeerNfcCommandV1(
            IrohaPeerNfcCommandTypeV1.SELECT_APPLICATION,
        )
        @JvmField val GET_INFO = IrohaPeerNfcCommandV1(IrohaPeerNfcCommandTypeV1.GET_INFO)

        @JvmStatic fun readRequest(
            sessionId: ByteArray,
            requestCanonicalHash: ByteArray,
            offset: Long,
            length: Int,
        ): IrohaPeerNfcCommandV1 = readCommand(
            IrohaPeerNfcCommandTypeV1.READ_REQUEST,
            sessionId,
            requestCanonicalHash,
            offset,
            length,
        )

        @JvmStatic fun beginPayment(
            sessionId: ByteArray,
            requestCanonicalHash: ByteArray,
            paymentHeader: ByteArray,
        ): IrohaPeerNfcCommandV1 {
            requireNfcSession(sessionId)
            requireNfcHash(requestCanonicalHash)
            val header = IrohaPeerWireMessageV1.decodeHeader(paymentHeader)
            require(header.kind == IrohaPeerPayloadKind.PAYMENT) { "BEGIN_PAYMENT requires payment IPM1" }
            return IrohaPeerNfcCommandV1(
                IrohaPeerNfcCommandTypeV1.BEGIN_PAYMENT,
                sessionId,
                requestCanonicalHash,
                bytes = paymentHeader,
            )
        }

        @JvmStatic fun write(
            sessionId: ByteArray,
            paymentWireHash: ByteArray,
            offset: Long,
            bytes: ByteArray,
        ): IrohaPeerNfcCommandV1 {
            requireNfcSession(sessionId)
            requireNfcHash(paymentWireHash)
            require(offset in 0..0xffff_ffffL)
            require(bytes.isNotEmpty() && bytes.size <= IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES)
            return IrohaPeerNfcCommandV1(
                IrohaPeerNfcCommandTypeV1.WRITE,
                sessionId,
                paymentWireHash,
                offset = offset,
                bytes = bytes,
            )
        }

        @JvmStatic fun commit(
            sessionId: ByteArray,
            requestCanonicalHash: ByteArray,
            paymentWireHash: ByteArray,
        ): IrohaPeerNfcCommandV1 = controlCommand(
            IrohaPeerNfcCommandTypeV1.COMMIT,
            sessionId,
            requestCanonicalHash,
            paymentWireHash,
        )

        @JvmStatic fun readAcknowledgement(
            sessionId: ByteArray,
            paymentWireHash: ByteArray,
            offset: Long,
            length: Int,
        ): IrohaPeerNfcCommandV1 = readCommand(
            IrohaPeerNfcCommandTypeV1.READ_ACKNOWLEDGEMENT,
            sessionId,
            paymentWireHash,
            offset,
            length,
        )

        @JvmStatic fun confirmAcknowledgement(
            sessionId: ByteArray,
            paymentWireHash: ByteArray,
            acknowledgementWireHash: ByteArray,
        ): IrohaPeerNfcCommandV1 = controlCommand(
            IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT,
            sessionId,
            paymentWireHash,
            acknowledgementWireHash,
        )

        @JvmStatic fun getStatus(
            sessionId: ByteArray,
            requestCanonicalHash: ByteArray,
        ): IrohaPeerNfcCommandV1 {
            requireNfcSession(sessionId)
            requireNfcHash(requestCanonicalHash)
            return IrohaPeerNfcCommandV1(
                IrohaPeerNfcCommandTypeV1.GET_STATUS,
                sessionId,
                requestCanonicalHash,
            )
        }

        private fun readCommand(
            type: IrohaPeerNfcCommandTypeV1,
            sessionId: ByteArray,
            hash: ByteArray,
            offset: Long,
            length: Int,
        ): IrohaPeerNfcCommandV1 {
            requireNfcSession(sessionId)
            requireNfcHash(hash)
            require(offset in 0..0xffff_ffffL)
            requireNfcChunkLimit(length)
            return IrohaPeerNfcCommandV1(type, sessionId, hash, offset = offset, length = length)
        }

        private fun controlCommand(
            type: IrohaPeerNfcCommandTypeV1,
            sessionId: ByteArray,
            firstHash: ByteArray,
            secondHash: ByteArray,
        ): IrohaPeerNfcCommandV1 {
            requireNfcSession(sessionId)
            requireNfcHash(firstHash)
            requireNfcHash(secondHash)
            return IrohaPeerNfcCommandV1(type, sessionId, firstHash, secondHash)
        }
    }
}

/** Strict ISO 7816 APDU codec with u32 body offsets and no P1/P2 fallback. */
object IrohaPeerNfcAPDUCodecV1 {
    @JvmStatic fun encode(command: IrohaPeerNfcCommandV1): ByteArray = when (command.type) {
        IrohaPeerNfcCommandTypeV1.SELECT_APPLICATION -> encodeEnvelope(
            0,
            0xa4,
            0x04,
            0,
            IrohaPeerNfcV1.applicationIdentifier(),
            256,
        )
        IrohaPeerNfcCommandTypeV1.GET_INFO -> proprietary(
            IrohaPeerNfcInstructionV1.GET_INFO,
            expectedLength = IrohaPeerNfcV1.INFO_BYTES,
        )
        IrohaPeerNfcCommandTypeV1.READ_REQUEST -> proprietary(
            IrohaPeerNfcInstructionV1.READ_REQUEST,
            command.sessionId!! + command.firstHash!! + nfcU32(command.offset),
            command.length,
        )
        IrohaPeerNfcCommandTypeV1.BEGIN_PAYMENT -> proprietary(
            IrohaPeerNfcInstructionV1.BEGIN_PAYMENT,
            command.sessionId!! + command.firstHash!! + command.bytes,
        )
        IrohaPeerNfcCommandTypeV1.WRITE -> proprietary(
            IrohaPeerNfcInstructionV1.WRITE,
            command.sessionId!! + command.firstHash!! + nfcU32(command.offset) + command.bytes,
        )
        IrohaPeerNfcCommandTypeV1.COMMIT -> proprietary(
            IrohaPeerNfcInstructionV1.COMMIT,
            command.sessionId!! + command.firstHash!! + command.secondHash!!,
        )
        IrohaPeerNfcCommandTypeV1.READ_ACKNOWLEDGEMENT -> proprietary(
            IrohaPeerNfcInstructionV1.READ_ACKNOWLEDGEMENT,
            command.sessionId!! + command.firstHash!! + nfcU32(command.offset),
            command.length,
        )
        IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT -> proprietary(
            IrohaPeerNfcInstructionV1.CONFIRM_ACKNOWLEDGEMENT,
            command.sessionId!! + command.firstHash!! + command.secondHash!!,
        )
        IrohaPeerNfcCommandTypeV1.GET_STATUS -> proprietary(
            IrohaPeerNfcInstructionV1.GET_STATUS,
            command.sessionId!! + command.firstHash!!,
            IrohaPeerNfcV1.STATUS_BYTES,
        )
    }

    @JvmStatic fun decode(apdu: ByteArray): IrohaPeerNfcCommandV1 {
        val envelope = decodeEnvelope(apdu)
        if (envelope.cla == 0 && envelope.instruction == 0xa4) {
            require(envelope.p1 == 0x04 && envelope.p2 == 0 &&
                IrohaPeerNfcV1.matchesApplicationIdentifier(envelope.data) &&
                envelope.expectedLength == 256) { "Invalid NFC application selection" }
            return IrohaPeerNfcCommandV1.SELECT_APPLICATION
        }
        require(envelope.cla == IrohaPeerNfcV1.COMMAND_CLASS && envelope.p1 == 0 && envelope.p2 == 0) {
            "Invalid proprietary NFC APDU"
        }
        val instruction = IrohaPeerNfcInstructionV1.fromCode(envelope.instruction)
            ?: throw IllegalArgumentException("Unsupported NFC instruction")
        return when (instruction) {
            IrohaPeerNfcInstructionV1.GET_INFO -> {
                require(envelope.data.isEmpty() && envelope.expectedLength == IrohaPeerNfcV1.INFO_BYTES)
                IrohaPeerNfcCommandV1.GET_INFO
            }
            IrohaPeerNfcInstructionV1.READ_REQUEST,
            IrohaPeerNfcInstructionV1.READ_ACKNOWLEDGEMENT -> {
                require(envelope.data.size == 52 && envelope.expectedLength != null)
                val session = envelope.data.copyOfRange(0, 16)
                val hash = envelope.data.copyOfRange(16, 48)
                val offset = envelope.data.nfcReadU32(48)
                if (instruction == IrohaPeerNfcInstructionV1.READ_REQUEST) {
                    IrohaPeerNfcCommandV1.readRequest(session, hash, offset, envelope.expectedLength)
                } else {
                    IrohaPeerNfcCommandV1.readAcknowledgement(session, hash, offset, envelope.expectedLength)
                }
            }
            IrohaPeerNfcInstructionV1.BEGIN_PAYMENT -> {
                require(envelope.expectedLength == null &&
                    envelope.data.size == 48 + IrohaPeerWireMessageV1.HEADER_LENGTH)
                IrohaPeerNfcCommandV1.beginPayment(
                    envelope.data.copyOfRange(0, 16),
                    envelope.data.copyOfRange(16, 48),
                    envelope.data.copyOfRange(48, envelope.data.size),
                )
            }
            IrohaPeerNfcInstructionV1.WRITE -> {
                require(envelope.expectedLength == null && envelope.data.size in 53..(52 + IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES))
                IrohaPeerNfcCommandV1.write(
                    envelope.data.copyOfRange(0, 16),
                    envelope.data.copyOfRange(16, 48),
                    envelope.data.nfcReadU32(48),
                    envelope.data.copyOfRange(52, envelope.data.size),
                )
            }
            IrohaPeerNfcInstructionV1.COMMIT,
            IrohaPeerNfcInstructionV1.CONFIRM_ACKNOWLEDGEMENT -> {
                require(envelope.expectedLength == null && envelope.data.size == 80)
                if (instruction == IrohaPeerNfcInstructionV1.COMMIT) {
                    IrohaPeerNfcCommandV1.commit(
                        envelope.data.copyOfRange(0, 16),
                        envelope.data.copyOfRange(16, 48),
                        envelope.data.copyOfRange(48, 80),
                    )
                } else {
                    IrohaPeerNfcCommandV1.confirmAcknowledgement(
                        envelope.data.copyOfRange(0, 16),
                        envelope.data.copyOfRange(16, 48),
                        envelope.data.copyOfRange(48, 80),
                    )
                }
            }
            IrohaPeerNfcInstructionV1.GET_STATUS -> {
                require(envelope.data.size == 48 && envelope.expectedLength == IrohaPeerNfcV1.STATUS_BYTES)
                IrohaPeerNfcCommandV1.getStatus(
                    envelope.data.copyOfRange(0, 16),
                    envelope.data.copyOfRange(16, 48),
                )
            }
        }
    }

    private fun proprietary(
        instruction: IrohaPeerNfcInstructionV1,
        data: ByteArray = byteArrayOf(),
        expectedLength: Int? = null,
    ): ByteArray = encodeEnvelope(
        IrohaPeerNfcV1.COMMAND_CLASS,
        instruction.code,
        0,
        0,
        data,
        expectedLength,
    )

    private fun encodeEnvelope(
        cla: Int,
        instruction: Int,
        p1: Int,
        p2: Int,
        data: ByteArray,
        expectedLength: Int?,
    ): ByteArray {
        require(data.size <= 0xffff && (expectedLength == null || expectedLength in 1..65_536))
        val output = ByteArrayOutputStream()
        output.write(byteArrayOf(cla.toByte(), instruction.toByte(), p1.toByte(), p2.toByte()))
        if (data.isEmpty()) {
            if (expectedLength == null) return output.toByteArray()
            if (expectedLength <= 256) output.write(if (expectedLength == 256) 0 else expectedLength)
            else {
                output.write(0)
                output.nfcWriteU16(if (expectedLength == 65_536) 0 else expectedLength)
            }
            return output.toByteArray()
        }
        val extended = data.size > 255 || (expectedLength ?: 0) > 256
        if (extended) {
            output.write(0)
            output.nfcWriteU16(data.size)
            output.write(data)
            if (expectedLength != null) output.nfcWriteU16(if (expectedLength == 65_536) 0 else expectedLength)
        } else {
            output.write(data.size)
            output.write(data)
            if (expectedLength != null) output.write(if (expectedLength == 256) 0 else expectedLength)
        }
        return output.toByteArray()
    }

    private class Envelope(
        val cla: Int,
        val instruction: Int,
        val p1: Int,
        val p2: Int,
        val data: ByteArray,
        val expectedLength: Int?,
    )

    private fun decodeEnvelope(apdu: ByteArray): Envelope {
        require(apdu.size >= 4) { "Malformed NFC APDU" }
        val cla = apdu[0].toInt() and 0xff
        val instruction = apdu[1].toInt() and 0xff
        val p1 = apdu[2].toInt() and 0xff
        val p2 = apdu[3].toInt() and 0xff
        if (apdu.size == 4) return Envelope(cla, instruction, p1, p2, byteArrayOf(), null)
        val firstLength = apdu[4].toInt() and 0xff
        if (firstLength != 0) {
            if (apdu.size == 5) return Envelope(cla, instruction, p1, p2, byteArrayOf(), firstLength)
            if (apdu.size == 5 + firstLength) {
                return Envelope(cla, instruction, p1, p2, apdu.copyOfRange(5, apdu.size), null)
            }
            if (apdu.size == 6 + firstLength) {
                val rawLe = apdu.last().toInt() and 0xff
                return Envelope(
                    cla, instruction, p1, p2,
                    apdu.copyOfRange(5, 5 + firstLength),
                    if (rawLe == 0) 256 else rawLe,
                )
            }
            throw IllegalArgumentException("Malformed short NFC APDU")
        }
        if (apdu.size == 5) return Envelope(cla, instruction, p1, p2, byteArrayOf(), 256)
        require(apdu.size >= 7) { "Malformed extended NFC APDU" }
        val extendedLength = apdu.nfcReadU16(5)
        if (apdu.size == 7) {
            require(extendedLength == 0 || extendedLength > 256) {
                "Non-canonical extended NFC response length"
            }
            return Envelope(cla, instruction, p1, p2, byteArrayOf(),
                if (extendedLength == 0) 65_536 else extendedLength)
        }
        require(extendedLength > 0)
        if (apdu.size == 7 + extendedLength) {
            require(extendedLength > 0xff) { "Non-canonical extended NFC data length" }
            return Envelope(cla, instruction, p1, p2, apdu.copyOfRange(7, apdu.size), null)
        }
        if (apdu.size == 9 + extendedLength) {
            val rawLe = apdu.nfcReadU16(7 + extendedLength)
            val expectedLength = if (rawLe == 0) 65_536 else rawLe
            require(extendedLength > 0xff || expectedLength > 256) {
                "Non-canonical extended NFC APDU"
            }
            return Envelope(
                cla, instruction, p1, p2,
                apdu.copyOfRange(7, 7 + extendedLength),
                expectedLength,
            )
        }
        throw IllegalArgumentException("Malformed extended NFC APDU")
    }
}

private fun nfcU32(value: Long): ByteArray = ByteArrayOutputStream(4).also {
    it.nfcWriteU32(value)
}.toByteArray()

private fun ByteArray?.contentEqualsNfc(other: ByteArray?): Boolean = when {
    this == null -> other == null
    other == null -> false
    else -> contentEquals(other)
}

class IrohaPeerNfcPaymentDescriptorV1 @JvmOverloads constructor(
    paymentHeader: ByteArray,
    limits: IrohaPeerNfcLimitsV1 = IrohaPeerNfcLimitsV1.DEFAULT,
) {
    private val descriptorHeader = paymentHeader.copyOf()
    private val descriptorCanonicalHash: ByteArray
    private val descriptorWireHash: ByteArray
    val profile: IrohaPeerPayloadProfile
    val schemaVersion: Int
    val messageLength: Int
    val canonicalHash: ByteArray get() = descriptorCanonicalHash.copyOf()
    val wireHash: ByteArray get() = descriptorWireHash.copyOf()
    val header: ByteArray get() = descriptorHeader.copyOf()

    init {
        val inspected = IrohaPeerWireMessageV1.decodeHeader(descriptorHeader)
        require(inspected.kind == IrohaPeerPayloadKind.PAYMENT) { "NFC descriptor is not a payment" }
        messageLength = IrohaPeerWireMessageV1.HEADER_LENGTH + inspected.encodedLength
        require(messageLength <= limits.maximumMessageBytes) { "NFC payment exceeds bound" }
        profile = inspected.profile
        schemaVersion = inspected.schemaVersion
        descriptorCanonicalHash = inspected.canonicalHash
        descriptorWireHash = inspected.wireHash
    }

    override fun equals(other: Any?): Boolean = other is IrohaPeerNfcPaymentDescriptorV1 &&
        profile == other.profile && schemaVersion == other.schemaVersion &&
        messageLength == other.messageLength && descriptorHeader.contentEquals(other.descriptorHeader)

    override fun hashCode(): Int = 31 * profile.hashCode() + descriptorWireHash.contentHashCode()
}

/** Ephemeral validated BEGIN_PAYMENT input presented to durable storage. */
class IrohaPeerNfcPaymentAdmissionContextV1 @JvmOverloads constructor(
    val identity: IrohaPeerNfcRequestIdentityV1,
    val profilePolicy: IrohaPeerNfcProfilePolicyV1,
    paymentHeader: ByteArray,
    limits: IrohaPeerNfcLimitsV1 = IrohaPeerNfcLimitsV1.DEFAULT,
) {
    val descriptor = IrohaPeerNfcPaymentDescriptorV1(paymentHeader.copyOf(), limits)
    val paymentHeader: ByteArray get() = descriptor.header

    init {
        require(profilePolicy.profile == identity.profile &&
            profilePolicy.accepts(descriptor.profile)) {
            "NFC payment admission profile mismatch"
        }
    }

    override fun equals(other: Any?): Boolean =
        other is IrohaPeerNfcPaymentAdmissionContextV1 &&
            identity == other.identity && profilePolicy == other.profilePolicy &&
            descriptor == other.descriptor

    override fun hashCode(): Int =
        31 * (31 * identity.hashCode() + profilePolicy.hashCode()) + descriptor.hashCode()
}

/**
 * Persistable exact IPA1 returned by the admission callback. Store [encode]
 * atomically and restore with [decode]; never reconstruct this record from
 * projected descriptor fields. Decoding revalidates every redundant field and
 * the full exact IPM1 header under the current local limits.
 */
class IrohaPeerNfcDurablePaymentAdmissionV1 @JvmOverloads constructor(
    val context: IrohaPeerNfcPaymentAdmissionContextV1,
    limits: IrohaPeerNfcLimitsV1 = IrohaPeerNfcLimitsV1.DEFAULT,
) {
    val identity: IrohaPeerNfcRequestIdentityV1 get() = context.identity
    val profilePolicy: IrohaPeerNfcProfilePolicyV1 get() = context.profilePolicy
    val descriptor: IrohaPeerNfcPaymentDescriptorV1 get() = context.descriptor
    val paymentHeader: ByteArray get() = context.paymentHeader

    init {
        require(IrohaPeerNfcPaymentAdmissionContextV1(
            context.identity,
            context.profilePolicy,
            context.paymentHeader,
            limits,
        ) == context) { "Invalid durable NFC payment admission" }
    }

    override fun equals(other: Any?): Boolean =
        other is IrohaPeerNfcDurablePaymentAdmissionV1 && context == other.context

    override fun hashCode(): Int = context.hashCode()

    /** Exact fixed-width IPA1 record stored before BEGIN_PAYMENT returns success. */
    fun encode(): ByteArray = ByteArrayOutputStream(IrohaPeerNfcV1.PAYMENT_ADMISSION_BYTES).also {
        output ->
        output.write(IrohaPeerNfcV1.PAYMENT_ADMISSION_MAGIC)
        output.write(IrohaPeerNfcV1.WIRE_VERSION)
        output.nfcWriteU16(identity.profile.code)
        output.write(0)
        output.write(identity.sessionId)
        output.write(identity.requestCanonicalHash)
        output.write(identity.requestWireHash)
        output.nfcWriteU16(descriptor.profile.code)
        output.nfcWriteU16(descriptor.schemaVersion)
        output.nfcWriteU32(descriptor.messageLength.toLong())
        output.write(descriptor.canonicalHash)
        output.write(descriptor.wireHash)
        output.write(descriptor.header)
    }.toByteArray().also { check(it.size == IrohaPeerNfcV1.PAYMENT_ADMISSION_BYTES) }

    companion object {
        @JvmStatic
        @JvmOverloads
        fun decode(
            data: ByteArray,
            profilePolicy: IrohaPeerNfcProfilePolicyV1? = null,
            limits: IrohaPeerNfcLimitsV1 = IrohaPeerNfcLimitsV1.DEFAULT,
        ): IrohaPeerNfcDurablePaymentAdmissionV1 {
            require(data.size == IrohaPeerNfcV1.PAYMENT_ADMISSION_BYTES &&
                data.copyOfRange(0, 4).contentEquals(IrohaPeerNfcV1.PAYMENT_ADMISSION_MAGIC) &&
                data[4].toInt() and 0xff == IrohaPeerNfcV1.WIRE_VERSION && data[7].toInt() == 0) {
                "Malformed IPA1 record"
            }
            val requestProfile = IrohaPeerPayloadProfile.fromCode(data.nfcReadU16(5))
                ?: throw IllegalArgumentException("Invalid IPA1 request profile")
            val paymentProfile = IrohaPeerPayloadProfile.fromCode(data.nfcReadU16(88))
                ?: throw IllegalArgumentException("Invalid IPA1 payment profile")
            val effectivePolicy = profilePolicy ?: IrohaPeerNfcProfilePolicyV1(requestProfile)
            val context = IrohaPeerNfcPaymentAdmissionContextV1(
                IrohaPeerNfcRequestIdentityV1(
                    requestProfile,
                    data.copyOfRange(8, 24),
                    data.copyOfRange(24, 56),
                    data.copyOfRange(56, 88),
                ),
                effectivePolicy,
                data.copyOfRange(160, 244),
                limits,
            )
            require(effectivePolicy.profile == requestProfile &&
                context.descriptor.profile == paymentProfile &&
                context.descriptor.schemaVersion == data.nfcReadU16(90) &&
                context.descriptor.messageLength == data.nfcReadU32(92).nfcCheckedInt() &&
                context.descriptor.canonicalHash.contentEquals(data.copyOfRange(96, 128)) &&
                context.descriptor.wireHash.contentEquals(data.copyOfRange(128, 160))) {
                "IPA1 descriptor mismatch"
            }
            return IrohaPeerNfcDurablePaymentAdmissionV1(context, limits)
        }
    }
}

class IrohaPeerNfcCommitContextV1(
    val identity: IrohaPeerNfcRequestIdentityV1,
    val profilePolicy: IrohaPeerNfcProfilePolicyV1,
    val payment: IrohaPeerWireMessageV1,
) {
    init {
        require(profilePolicy.profile == identity.profile &&
            profilePolicy.accepts(payment.canonicalPayload.profile) &&
            payment.canonicalPayload.kind == IrohaPeerPayloadKind.PAYMENT) {
            "NFC session profile mismatch"
        }
    }
}

/** Persistable IDA1 record. The application stores this before COMMIT returns success. */
class IrohaPeerNfcDurableAcknowledgementV1 private constructor(
    val identity: IrohaPeerNfcRequestIdentityV1,
    val paymentProfile: IrohaPeerPayloadProfile,
    val paymentLength: Int,
    paymentWireHash: ByteArray,
    val acknowledgement: IrohaPeerWireMessageV1,
) {
    private val paymentHash = paymentWireHash.copyOf()
    val paymentWireHash: ByteArray get() = paymentHash.copyOf()

    @JvmOverloads
    constructor(
        context: IrohaPeerNfcCommitContextV1,
        acknowledgement: ByteArray,
        limits: IrohaPeerNfcLimitsV1 = IrohaPeerNfcLimitsV1.DEFAULT,
    ) : this(
        context.identity,
        context.payment.canonicalPayload.profile,
        context.payment.encode().size,
        context.payment.wireHash,
        decodeNfcMessage(
            acknowledgement,
            context.profilePolicy.profile,
            IrohaPeerPayloadKind.ACKNOWLEDGEMENT,
            limits,
        ),
    ) {
        require(context.payment.encode().size <= limits.maximumMessageBytes) {
            "NFC payment exceeds bound"
        }
        require(context.profilePolicy.accepts(this.acknowledgement.canonicalPayload.profile)) {
            "NFC acknowledgement profile mismatch"
        }
    }

    init {
        require(paymentLength in (IrohaPeerWireMessageV1.HEADER_LENGTH + 1)..IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES)
        requireNfcHash(paymentHash)
        require(acknowledgement.canonicalPayload.kind == IrohaPeerPayloadKind.ACKNOWLEDGEMENT)
    }

    fun encode(): ByteArray = ByteArrayOutputStream().also { output ->
        val acknowledgementBytes = acknowledgement.encode()
        output.write(IrohaPeerNfcV1.DURABLE_ACK_MAGIC)
        output.write(IrohaPeerNfcV1.WIRE_VERSION)
        output.nfcWriteU16(identity.profile.code)
        output.write(0)
        output.write(identity.sessionId)
        output.write(identity.requestCanonicalHash)
        output.write(identity.requestWireHash)
        output.nfcWriteU16(paymentProfile.code)
        output.nfcWriteU32(paymentLength.toLong())
        output.write(paymentHash)
        output.nfcWriteU32(acknowledgementBytes.size.toLong())
        output.write(acknowledgementBytes)
    }.toByteArray()

    override fun equals(other: Any?): Boolean = other is IrohaPeerNfcDurableAcknowledgementV1 &&
        identity == other.identity && paymentProfile == other.paymentProfile &&
        paymentLength == other.paymentLength && paymentHash.contentEquals(other.paymentHash) &&
        acknowledgement == other.acknowledgement

    override fun hashCode(): Int = 31 * identity.hashCode() + paymentHash.contentHashCode()

    companion object {
        @JvmStatic
        @JvmOverloads
        fun decode(
            data: ByteArray,
            profilePolicy: IrohaPeerNfcProfilePolicyV1? = null,
            limits: IrohaPeerNfcLimitsV1 = IrohaPeerNfcLimitsV1.DEFAULT,
        ): IrohaPeerNfcDurableAcknowledgementV1 {
            val fixed = 130
            require(data.size > fixed && data.copyOfRange(0, 4)
                .contentEquals(IrohaPeerNfcV1.DURABLE_ACK_MAGIC) &&
                data[4].toInt() and 0xff == IrohaPeerNfcV1.WIRE_VERSION && data[7].toInt() == 0) {
                "Malformed IDA1 record"
            }
            val requestProfile = IrohaPeerPayloadProfile.fromCode(data.nfcReadU16(5))
                ?: throw IllegalArgumentException("Invalid IDA1 request profile")
            val paymentProfile = IrohaPeerPayloadProfile.fromCode(data.nfcReadU16(88))
                ?: throw IllegalArgumentException("Invalid IDA1 payment profile")
            val acknowledgementLength = data.nfcReadU32(126).nfcCheckedInt()
            require(acknowledgementLength > IrohaPeerWireMessageV1.HEADER_LENGTH &&
                acknowledgementLength <= limits.maximumMessageBytes && fixed + acknowledgementLength == data.size)
            val paymentLength = data.nfcReadU32(90).nfcCheckedInt()
            require(paymentLength <= limits.maximumMessageBytes) {
                "IDA1 payment exceeds local NFC bound"
            }
            val policy = profilePolicy ?: IrohaPeerNfcProfilePolicyV1(requestProfile)
            val acknowledgement = decodeNfcMessage(
                data.copyOfRange(fixed, data.size),
                policy.profile,
                IrohaPeerPayloadKind.ACKNOWLEDGEMENT,
                limits,
            )
            require(policy.profile == requestProfile && policy.accepts(paymentProfile) &&
                policy.accepts(acknowledgement.canonicalPayload.profile)) {
                "IDA1 profile policy mismatch"
            }
            return IrohaPeerNfcDurableAcknowledgementV1(
                IrohaPeerNfcRequestIdentityV1(
                    requestProfile,
                    data.copyOfRange(8, 24),
                    data.copyOfRange(24, 56),
                    data.copyOfRange(56, 88),
                ),
                paymentProfile,
                paymentLength,
                data.copyOfRange(94, 126),
                acknowledgement,
            )
        }
    }
}

/** ISC1 sender checkpoint saved before the first payment WRITE. */
class IrohaPeerNfcSenderCheckpointV1 @JvmOverloads constructor(
    sessionId: ByteArray,
    receiveRequest: ByteArray,
    payment: ByteArray,
    durableAcknowledgement: ByteArray? = null,
    profilePolicy: IrohaPeerNfcProfilePolicyV1? = null,
    limits: IrohaPeerNfcLimitsV1 = IrohaPeerNfcLimitsV1.DEFAULT,
) {
    val receiveRequest = decodeNfcMessage(
        receiveRequest,
        null,
        IrohaPeerPayloadKind.RECEIVE_REQUEST,
        limits,
    )
    val profilePolicy = profilePolicy ?: IrohaPeerNfcProfilePolicyV1(
        this.receiveRequest.canonicalPayload.profile,
    )
    val payment = decodeNfcMessage(payment, null, IrohaPeerPayloadKind.PAYMENT, limits)
    val durableAcknowledgement = durableAcknowledgement?.let {
        decodeNfcMessage(it, null, IrohaPeerPayloadKind.ACKNOWLEDGEMENT, limits)
    }
    val identity: IrohaPeerNfcRequestIdentityV1

    init {
        requireNfcSession(sessionId)
        require(this.profilePolicy.profile == this.receiveRequest.canonicalPayload.profile &&
            this.profilePolicy.accepts(this.payment.canonicalPayload.profile) &&
            (this.durableAcknowledgement == null || this.profilePolicy.accepts(
                this.durableAcknowledgement.canonicalPayload.profile,
            ))) { "ISC1 profile policy mismatch" }
        identity = IrohaPeerNfcRequestIdentityV1(
            this.receiveRequest.canonicalPayload.profile,
            sessionId,
            this.receiveRequest.canonicalHash,
            this.receiveRequest.wireHash,
        )
    }

    fun encode(): ByteArray = ByteArrayOutputStream().also { output ->
        val request = receiveRequest.encode()
        val payment = payment.encode()
        val acknowledgement = durableAcknowledgement?.encode() ?: byteArrayOf()
        output.write(IrohaPeerNfcV1.SENDER_CHECKPOINT_MAGIC)
        output.write(IrohaPeerNfcV1.WIRE_VERSION)
        output.nfcWriteU16(identity.profile.code)
        output.write(0)
        output.write(identity.sessionId)
        output.nfcWriteU32(request.size.toLong())
        output.nfcWriteU32(payment.size.toLong())
        output.nfcWriteU32(acknowledgement.size.toLong())
        output.write(request)
        output.write(payment)
        output.write(acknowledgement)
    }.toByteArray()

    override fun equals(other: Any?): Boolean = other is IrohaPeerNfcSenderCheckpointV1 &&
        identity == other.identity && profilePolicy == other.profilePolicy &&
        receiveRequest == other.receiveRequest && payment == other.payment &&
        durableAcknowledgement == other.durableAcknowledgement

    override fun hashCode(): Int = 31 * identity.hashCode() + payment.hashCode()

    companion object {
        @JvmStatic
        @JvmOverloads
        fun decode(
            data: ByteArray,
            profilePolicy: IrohaPeerNfcProfilePolicyV1? = null,
            limits: IrohaPeerNfcLimitsV1 = IrohaPeerNfcLimitsV1.DEFAULT,
        ): IrohaPeerNfcSenderCheckpointV1 {
            val fixed = 36
            require(data.size > fixed && data.copyOfRange(0, 4)
                .contentEquals(IrohaPeerNfcV1.SENDER_CHECKPOINT_MAGIC) &&
                data[4].toInt() and 0xff == IrohaPeerNfcV1.WIRE_VERSION && data[7].toInt() == 0) {
                "Malformed ISC1 checkpoint"
            }
            val profile = IrohaPeerPayloadProfile.fromCode(data.nfcReadU16(5))
                ?: throw IllegalArgumentException("Invalid ISC1 profile")
            val requestLength = data.nfcReadU32(24).nfcCheckedInt()
            val paymentLength = data.nfcReadU32(28).nfcCheckedInt()
            val acknowledgementLength = data.nfcReadU32(32).nfcCheckedInt()
            require(requestLength > IrohaPeerWireMessageV1.HEADER_LENGTH &&
                paymentLength > IrohaPeerWireMessageV1.HEADER_LENGTH &&
                (acknowledgementLength == 0 || acknowledgementLength > IrohaPeerWireMessageV1.HEADER_LENGTH) &&
                requestLength <= limits.maximumMessageBytes && paymentLength <= limits.maximumMessageBytes &&
                acknowledgementLength <= limits.maximumMessageBytes &&
                fixed + requestLength + paymentLength + acknowledgementLength == data.size)
            val paymentStart = fixed + requestLength
            val acknowledgementStart = paymentStart + paymentLength
            val checkpoint = IrohaPeerNfcSenderCheckpointV1(
                data.copyOfRange(8, 24),
                data.copyOfRange(fixed, paymentStart),
                data.copyOfRange(paymentStart, acknowledgementStart),
                if (acknowledgementLength == 0) null else data.copyOfRange(acknowledgementStart, data.size),
                profilePolicy,
                limits,
            )
            require(checkpoint.identity.profile == profile)
            return checkpoint
        }
    }
}

private fun decodeNfcMessage(
    data: ByteArray,
    expectedProfile: IrohaPeerPayloadProfile?,
    expectedKind: IrohaPeerPayloadKind,
    limits: IrohaPeerNfcLimitsV1,
): IrohaPeerWireMessageV1 {
    require(data.size in (IrohaPeerWireMessageV1.HEADER_LENGTH + 1)..limits.maximumMessageBytes) {
        "NFC IPM1 message length is invalid"
    }
    return IrohaPeerWireMessageV1.decode(data, expectedProfile, expectedKind)
}

sealed class IrohaPeerNfcCommitDispositionV1 {
    class RequiresDurableCommit(val context: IrohaPeerNfcCommitContextV1) :
        IrohaPeerNfcCommitDispositionV1()
    object AlreadyCommitted : IrohaPeerNfcCommitDispositionV1()
}

sealed class IrohaPeerNfcPaymentAdmissionDispositionV1 {
    class RequiresDurableAdmission(val context: IrohaPeerNfcPaymentAdmissionContextV1) :
        IrohaPeerNfcPaymentAdmissionDispositionV1()
    object AlreadyAdmitted : IrohaPeerNfcPaymentAdmissionDispositionV1()
}

/** Receiver/card state with contiguous idempotent writes and an explicit durable COMMIT boundary. */
class IrohaPeerNfcReceiverSessionV1 @JvmOverloads constructor(
    sessionId: ByteArray,
    receiveRequest: ByteArray,
    durableAcknowledgement: IrohaPeerNfcDurableAcknowledgementV1? = null,
    profilePolicy: IrohaPeerNfcProfilePolicyV1? = null,
    val limits: IrohaPeerNfcLimitsV1 = IrohaPeerNfcLimitsV1.DEFAULT,
    restoredPaymentAdmission: IrohaPeerNfcDurablePaymentAdmissionV1? = null,
) {
    val receiveRequest = decodeNfcMessage(
        receiveRequest,
        null,
        IrohaPeerPayloadKind.RECEIVE_REQUEST,
        limits,
    )
    val identity = IrohaPeerNfcRequestIdentityV1(
        this.receiveRequest.canonicalPayload.profile,
        sessionId,
        this.receiveRequest.canonicalHash,
        this.receiveRequest.wireHash,
    )
    val profilePolicy = profilePolicy ?: IrohaPeerNfcProfilePolicyV1(
        this.receiveRequest.canonicalPayload.profile,
    )

    private var pendingDescriptor: IrohaPeerNfcPaymentDescriptorV1? = null
    private var pendingBytes = byteArrayOf()
    private var durableRecord = durableAcknowledgement
    private var acknowledgementConfirmed = false

    init {
        require(this.profilePolicy.profile == this.receiveRequest.canonicalPayload.profile)
        durableAcknowledgement?.let {
            require(it.identity == identity && this.profilePolicy.accepts(it.paymentProfile) &&
                this.profilePolicy.accepts(it.acknowledgement.canonicalPayload.profile)) {
                "Durable NFC record continuity mismatch"
            }
        }
        restoredPaymentAdmission?.let { restored ->
            require(restored.identity == identity && restored.profilePolicy == this.profilePolicy) {
                "Restored NFC payment admission continuity mismatch"
            }
            val validated = IrohaPeerNfcDurablePaymentAdmissionV1.decode(
                restored.encode(),
                this.profilePolicy,
                limits,
            )
            require(validated == restored) { "Restored NFC payment admission mismatch" }
            if (durableAcknowledgement != null) {
                require(validated.descriptor.profile == durableAcknowledgement.paymentProfile &&
                    validated.descriptor.messageLength == durableAcknowledgement.paymentLength &&
                    validated.descriptor.wireHash.contentEquals(
                        durableAcknowledgement.paymentWireHash,
                    )) {
                    "Durable NFC acknowledgement conflicts with payment admission"
                }
                // IDA1 is authoritative after COMMIT. Honest readers discover the
                // ACK phase through GET_STATUS and never replay BEGIN_PAYMENT.
            } else {
                pendingDescriptor = validated.descriptor
                pendingBytes = byteArrayOf()
            }
        }
    }

    val phase: IrohaPeerNfcPhaseV1
        get() = when {
            acknowledgementConfirmed -> IrohaPeerNfcPhaseV1.COMPLETE
            durableRecord != null -> IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY
            pendingDescriptor != null -> IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING
            else -> IrohaPeerNfcPhaseV1.REQUEST_READY
        }

    fun info(): IrohaPeerNfcInfoV1 = IrohaPeerNfcInfoV1(
        phase,
        if (durableRecord == null) IrohaPeerNfcFlagsV1.REQUEST else IrohaPeerNfcFlagsV1.DURABLE,
        identity,
        receiveRequest.encode().size,
        limits.maximumReadChunkBytes,
        limits.maximumWriteChunkBytes,
    )

    fun status(): IrohaPeerNfcStatusV1 {
        val durable = durableRecord
        val descriptor = pendingDescriptor
        val zero = ByteArray(32)
        return IrohaPeerNfcStatusV1(
            phase,
            if (durable == null) IrohaPeerNfcFlagsV1.REQUEST else IrohaPeerNfcFlagsV1.DURABLE,
            identity,
            durable?.paymentProfile ?: descriptor?.profile,
            durable?.paymentLength ?: descriptor?.messageLength ?: 0,
            durable?.paymentLength ?: pendingBytes.size,
            durable?.paymentWireHash ?: descriptor?.wireHash ?: zero,
            durable?.acknowledgement?.canonicalPayload?.profile,
            durable?.acknowledgement?.encode()?.size ?: 0,
            durable?.acknowledgement?.wireHash ?: zero,
            limits.maximumReadChunkBytes,
            limits.maximumWriteChunkBytes,
        )
    }

    fun handle(command: IrohaPeerNfcCommandV1): ByteArray = when (command.type) {
        IrohaPeerNfcCommandTypeV1.SELECT_APPLICATION -> byteArrayOf()
        IrohaPeerNfcCommandTypeV1.GET_INFO -> info().encode()
        IrohaPeerNfcCommandTypeV1.READ_REQUEST -> {
            requireRequestContinuity(command.sessionId!!, command.firstHash!!)
            readSlice(receiveRequest.encode(), command.offset, command.length)
        }
        IrohaPeerNfcCommandTypeV1.BEGIN_PAYMENT -> {
            throw IllegalStateException("BEGIN_PAYMENT requires durable application handling")
        }
        IrohaPeerNfcCommandTypeV1.WRITE -> {
            writePayment(command.sessionId!!, command.firstHash!!, command.offset, command.bytes)
            byteArrayOf()
        }
        IrohaPeerNfcCommandTypeV1.COMMIT ->
            throw IllegalStateException("COMMIT requires durable application handling")
        IrohaPeerNfcCommandTypeV1.READ_ACKNOWLEDGEMENT -> {
            val durable = durableRecord ?: throw IllegalStateException("NFC acknowledgement is not durable")
            requireSession(command.sessionId!!)
            require(command.firstHash!!.contentEquals(durable.paymentWireHash))
            readSlice(durable.acknowledgement.encode(), command.offset, command.length)
        }
        IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT -> {
            confirmAcknowledgement(command.sessionId!!, command.firstHash!!, command.secondHash!!)
            byteArrayOf()
        }
        IrohaPeerNfcCommandTypeV1.GET_STATUS -> {
            requireRequestContinuity(command.sessionId!!, command.firstHash!!)
            status().encode()
        }
    }

    fun preparePaymentAdmission(
        command: IrohaPeerNfcCommandV1,
    ): IrohaPeerNfcPaymentAdmissionDispositionV1 {
        require(command.type == IrohaPeerNfcCommandTypeV1.BEGIN_PAYMENT)
        requireRequestContinuity(command.sessionId!!, command.firstHash!!)
        val context = IrohaPeerNfcPaymentAdmissionContextV1(
            identity,
            profilePolicy,
            command.bytes,
            limits,
        )
        val descriptor = context.descriptor
        if (durableRecord != null) {
            throw IllegalStateException("BEGIN_PAYMENT is invalid after durable COMMIT")
        }
        pendingDescriptor?.let {
            require(it == descriptor) { "Conflicting BEGIN_PAYMENT replay" }
            return IrohaPeerNfcPaymentAdmissionDispositionV1.AlreadyAdmitted
        }
        return IrohaPeerNfcPaymentAdmissionDispositionV1.RequiresDurableAdmission(context)
    }

    fun installPaymentAdmission(record: IrohaPeerNfcDurablePaymentAdmissionV1) {
        require(record.identity == identity && record.profilePolicy == profilePolicy) {
            "NFC payment admission continuity mismatch"
        }
        val validated = IrohaPeerNfcDurablePaymentAdmissionV1.decode(
            record.encode(),
            profilePolicy,
            limits,
        )
        require(validated == record) { "NFC payment admission mismatch" }
        val descriptor = validated.descriptor
        if (durableRecord != null) {
            throw IllegalStateException("Payment admission cannot replace durable COMMIT")
        }
        pendingDescriptor?.let {
            require(it == descriptor) { "Conflicting BEGIN_PAYMENT replay" }
            return
        }
        pendingDescriptor = descriptor
        pendingBytes = byteArrayOf()
    }

    fun prepareCommit(command: IrohaPeerNfcCommandV1): IrohaPeerNfcCommitDispositionV1 {
        require(command.type == IrohaPeerNfcCommandTypeV1.COMMIT)
        requireRequestContinuity(command.sessionId!!, command.firstHash!!)
        durableRecord?.let {
            require(command.secondHash!!.contentEquals(it.paymentWireHash))
            return IrohaPeerNfcCommitDispositionV1.AlreadyCommitted
        }
        val descriptor = pendingDescriptor ?: throw IllegalStateException("No pending NFC payment")
        require(command.secondHash!!.contentEquals(descriptor.wireHash))
        require(pendingBytes.size == descriptor.messageLength) { "NFC payment is incomplete" }
        val payment = decodeNfcMessage(pendingBytes, null, IrohaPeerPayloadKind.PAYMENT, limits)
        require(profilePolicy.accepts(payment.canonicalPayload.profile) &&
            payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH)
                .contentEquals(descriptor.header) && payment.wireHash.contentEquals(descriptor.wireHash))
        return IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit(
            IrohaPeerNfcCommitContextV1(identity, profilePolicy, payment),
        )
    }

    fun installDurableAcknowledgement(record: IrohaPeerNfcDurableAcknowledgementV1) {
        require(record.identity == identity && profilePolicy.accepts(record.paymentProfile) &&
            profilePolicy.accepts(record.acknowledgement.canonicalPayload.profile))
        durableRecord?.let {
            require(it == record) { "Conflicting durable NFC replay" }
            return
        }
        val descriptor = pendingDescriptor ?: throw IllegalStateException("No pending NFC payment")
        require(record.paymentProfile == descriptor.profile &&
            record.paymentLength == descriptor.messageLength &&
            record.paymentWireHash.contentEquals(descriptor.wireHash))
        durableRecord = record
        pendingDescriptor = null
        pendingBytes.fill(0)
        pendingBytes = byteArrayOf()
    }

    private fun writePayment(
        sessionId: ByteArray,
        paymentHash: ByteArray,
        offset: Long,
        bytes: ByteArray,
    ) {
        requireSession(sessionId)
        require(bytes.isNotEmpty() && bytes.size <= limits.maximumWriteChunkBytes)
        val descriptor = pendingDescriptor ?: throw IllegalStateException("No pending NFC payment")
        require(paymentHash.contentEquals(descriptor.wireHash))
        val start = offset.nfcCheckedInt()
        require(start <= pendingBytes.size && start <= descriptor.messageLength - bytes.size) {
            "Invalid NFC write offset"
        }
        val overlap = minOf(pendingBytes.size - start, bytes.size)
        if (overlap > 0) {
            require(pendingBytes.copyOfRange(start, start + overlap)
                .contentEquals(bytes.copyOfRange(0, overlap))) { "Conflicting NFC write replay" }
        }
        if (overlap < bytes.size) pendingBytes += bytes.copyOfRange(overlap, bytes.size)
    }

    private fun confirmAcknowledgement(
        sessionId: ByteArray,
        paymentHash: ByteArray,
        acknowledgementHash: ByteArray,
    ) {
        requireSession(sessionId)
        val durable = durableRecord ?: throw IllegalStateException("NFC acknowledgement is not durable")
        require(paymentHash.contentEquals(durable.paymentWireHash) &&
            acknowledgementHash.contentEquals(durable.acknowledgement.wireHash))
        acknowledgementConfirmed = true
    }

    private fun requireSession(value: ByteArray) {
        requireNfcSession(value)
        require(value.contentEquals(identity.sessionId)) { "NFC session continuity mismatch" }
    }

    private fun requireRequestContinuity(sessionId: ByteArray, requestHash: ByteArray) {
        requireSession(sessionId)
        requireNfcHash(requestHash)
        require(requestHash.contentEquals(identity.requestCanonicalHash)) {
            "NFC request continuity mismatch"
        }
    }

    private fun readSlice(message: ByteArray, offset: Long, requestedLength: Int): ByteArray {
        require(requestedLength in 1..limits.maximumReadChunkBytes)
        val start = offset.nfcCheckedInt()
        require(start in message.indices)
        return message.copyOfRange(start, minOf(start + requestedLength, message.size))
    }
}

sealed class IrohaPeerNfcSenderActionV1 {
    class Send(val command: IrohaPeerNfcCommandV1) : IrohaPeerNfcSenderActionV1()
    class PersistAcknowledgement(bytes: ByteArray) : IrohaPeerNfcSenderActionV1() {
        private val value = bytes.copyOf()
        val bytes: ByteArray get() = value.copyOf()
    }
    class Complete(bytes: ByteArray) : IrohaPeerNfcSenderActionV1() {
        private val value = bytes.copyOf()
        val bytes: ByteArray get() = value.copyOf()
    }
}

/** Portable status values returned by a reader transceiver. */
enum class IrohaPeerNfcReaderStatusV1(val code: Int) {
    SUCCESS(0x9000),
    STORAGE_FAILURE(0x6581),
    WRONG_LENGTH(0x6700),
    SECURITY_STATUS_NOT_SATISFIED(0x6982),
    CONDITIONS_NOT_SATISFIED(0x6985),
    WRONG_DATA(0x6a80),
    NOT_FOUND(0x6a82),
    INSTRUCTION_NOT_SUPPORTED(0x6d00),
    CLASS_NOT_SUPPORTED(0x6e00);

    companion object {
        @JvmStatic fun fromCode(code: Int): IrohaPeerNfcReaderStatusV1? =
            entries.firstOrNull { it.code == code }
    }
}

/** Immutable portable NFC response. Platform adapters map their APDU result to this type. */
class IrohaPeerNfcReaderResponseV1 @JvmOverloads constructor(
    data: ByteArray = byteArrayOf(),
    val status: IrohaPeerNfcReaderStatusV1,
) {
    private val responseData = data.copyOf()
    val data: ByteArray get() = responseData.copyOf()

    override fun equals(other: Any?): Boolean = other is IrohaPeerNfcReaderResponseV1 &&
        status == other.status && responseData.contentEquals(other.responseData)

    override fun hashCode(): Int = 31 * status.hashCode() + responseData.contentHashCode()

    companion object {
        @JvmStatic fun success(data: ByteArray = byteArrayOf()) =
            IrohaPeerNfcReaderResponseV1(data, IrohaPeerNfcReaderStatusV1.SUCCESS)
    }
}

/** A recognized non-success status returned by the peer. RF/transport failures pass through. */
class IrohaPeerNfcReaderStatusExceptionV1(
    val status: IrohaPeerNfcReaderStatusV1,
) : IllegalStateException("NFC peer returned status %04X".format(status.code))

/** Marker for a transport failure after command emission with no known response. */
interface IrohaPeerNfcAmbiguousResponseErrorV1

fun interface IrohaPeerNfcReaderTransceiverV1 {
    fun transceive(command: IrohaPeerNfcCommandV1): IrohaPeerNfcReaderResponseV1
}

fun interface IrohaPeerNfcSenderCheckpointStoreV1 {
    /**
     * Loads an existing exact ISC1 or creates and stores it atomically. This
     * method must return only after the returned checkpoint is durable.
     */
    fun loadOrCreateDurableCheckpoint(
        info: IrohaPeerNfcInfoV1,
        receiveRequest: IrohaPeerWireMessageV1,
    ): IrohaPeerNfcSenderCheckpointV1
}

fun interface IrohaPeerNfcSenderCheckpointUpdaterV1 {
    /** Durably installs the monotonic ACK-bearing update to an existing ISC1. */
    fun updateDurableCheckpoint(encodedCheckpoint: ByteArray)
}

enum class IrohaPeerNfcConfirmationStateV1 {
    CONFIRMED,
    RESPONSE_UNKNOWN,
}

class IrohaPeerNfcReaderExchangeResultV1 @JvmOverloads constructor(
    val checkpoint: IrohaPeerNfcSenderCheckpointV1,
    val acknowledgement: IrohaPeerWireMessageV1,
    val confirmationState: IrohaPeerNfcConfirmationStateV1 =
        IrohaPeerNfcConfirmationStateV1.CONFIRMED,
)

/** Request-reading planner that intersects this reader's limit with the peer advertisement. */
object IrohaPeerNfcReaderPlanningV1 {
    @JvmStatic fun getStatusCommand(info: IrohaPeerNfcInfoV1): IrohaPeerNfcCommandV1 =
        IrohaPeerNfcCommandV1.getStatus(
            info.identity.sessionId,
            info.identity.requestCanonicalHash,
        )

    @JvmStatic
    @JvmOverloads
    fun readRequestCommand(
        info: IrohaPeerNfcInfoV1,
        offset: Int,
        localLimits: IrohaPeerNfcLimitsV1 = IrohaPeerNfcLimitsV1.DEFAULT,
    ): IrohaPeerNfcCommandV1 {
        require(info.requestLength <= localLimits.maximumMessageBytes) {
            "NFC request exceeds local message bound"
        }
        require(offset in 0 until info.requestLength) { "Invalid NFC request offset" }
        return IrohaPeerNfcCommandV1.readRequest(
            info.identity.sessionId,
            info.identity.requestCanonicalHash,
            offset.toLong(),
            minOf(
                localLimits.maximumReadChunkBytes,
                info.maximumReadChunkBytes,
                info.requestLength - offset,
            ),
        )
    }
}

/** Status-authoritative two-tap sender reducer using the exact persisted ISC1 payment. */
class IrohaPeerNfcTwoTapReducerV1 @JvmOverloads constructor(
    checkpoint: IrohaPeerNfcSenderCheckpointV1,
    val limits: IrohaPeerNfcLimitsV1 = IrohaPeerNfcLimitsV1.DEFAULT,
) {
    var checkpoint = checkpoint
        private set
    private var acknowledgementBuffer = byteArrayOf()
    private var expectedAcknowledgementLength: Int? = null
    private var expectedAcknowledgementHash: ByteArray? = null

    fun requireSamePeer(info: IrohaPeerNfcInfoV1) {
        require(info.identity == checkpoint.identity &&
            info.requestLength == checkpoint.receiveRequest.encode().size) {
            "NFC peer continuity mismatch"
        }
    }

    fun nextAction(status: IrohaPeerNfcStatusV1): IrohaPeerNfcSenderActionV1 {
        require(status.identity == checkpoint.identity) { "NFC status continuity mismatch" }
        val payment = checkpoint.payment
        return when (status.phase) {
            IrohaPeerNfcPhaseV1.REQUEST_READY -> {
                require(checkpoint.durableAcknowledgement == null)
                resetAcknowledgementBuffer()
                IrohaPeerNfcSenderActionV1.Send(IrohaPeerNfcCommandV1.beginPayment(
                    checkpoint.identity.sessionId,
                    checkpoint.identity.requestCanonicalHash,
                    payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
                ))
            }
            IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING -> {
                require(checkpoint.durableAcknowledgement == null &&
                    status.paymentProfile == payment.canonicalPayload.profile &&
                    status.paymentLength == payment.encode().size &&
                    status.paymentWireHash.contentEquals(payment.wireHash))
                resetAcknowledgementBuffer()
                if (status.receivedPaymentBytes < status.paymentLength) {
                    val offset = status.receivedPaymentBytes
                    val count = minOf(
                        limits.maximumWriteChunkBytes,
                        status.maximumWriteChunkBytes,
                        status.paymentLength - offset,
                    )
                    IrohaPeerNfcSenderActionV1.Send(IrohaPeerNfcCommandV1.write(
                        checkpoint.identity.sessionId,
                        payment.wireHash,
                        offset.toLong(),
                        payment.encode().copyOfRange(offset, offset + count),
                    ))
                } else {
                    IrohaPeerNfcSenderActionV1.Send(IrohaPeerNfcCommandV1.commit(
                        checkpoint.identity.sessionId,
                        checkpoint.identity.requestCanonicalHash,
                        payment.wireHash,
                    ))
                }
            }
            IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY -> {
                requireAckMetadata(status)
                checkpoint.durableAcknowledgement?.let {
                    require(it.canonicalPayload.profile == status.acknowledgementProfile &&
                        it.encode().size == status.acknowledgementLength &&
                        it.wireHash.contentEquals(status.acknowledgementWireHash))
                    return IrohaPeerNfcSenderActionV1.Send(
                        IrohaPeerNfcCommandV1.confirmAcknowledgement(
                            checkpoint.identity.sessionId,
                            payment.wireHash,
                            it.wireHash,
                        ),
                    )
                }
                configureAcknowledgementBuffer(status)
                if (acknowledgementBuffer.size == status.acknowledgementLength) {
                    IrohaPeerNfcSenderActionV1.PersistAcknowledgement(acknowledgementBuffer)
                } else {
                    val offset = acknowledgementBuffer.size
                    IrohaPeerNfcSenderActionV1.Send(IrohaPeerNfcCommandV1.readAcknowledgement(
                        checkpoint.identity.sessionId,
                        payment.wireHash,
                        offset.toLong(),
                        minOf(
                            limits.maximumReadChunkBytes,
                            status.maximumReadChunkBytes,
                            status.acknowledgementLength - offset,
                        ),
                    ))
                }
            }
            IrohaPeerNfcPhaseV1.COMPLETE -> {
                requireAckMetadata(status)
                val durable = checkpoint.durableAcknowledgement
                    ?: throw IllegalStateException("NFC acknowledgement is not durable locally")
                require(durable.encode().size == status.acknowledgementLength &&
                    durable.wireHash.contentEquals(status.acknowledgementWireHash))
                IrohaPeerNfcSenderActionV1.Complete(durable.encode())
            }
        }
    }

    fun consumeAcknowledgementChunk(bytes: ByteArray): Boolean {
        val expectedLength = expectedAcknowledgementLength
            ?: throw IllegalStateException("No pending NFC acknowledgement")
        require(bytes.isNotEmpty() && bytes.size <= limits.maximumReadChunkBytes &&
            acknowledgementBuffer.size < expectedLength &&
            bytes.size <= expectedLength - acknowledgementBuffer.size)
        acknowledgementBuffer += bytes
        if (acknowledgementBuffer.size != expectedLength) return false
        val acknowledgement = decodeNfcMessage(
            acknowledgementBuffer,
            null,
            IrohaPeerPayloadKind.ACKNOWLEDGEMENT,
            limits,
        )
        require(checkpoint.profilePolicy.accepts(acknowledgement.canonicalPayload.profile) &&
            acknowledgement.wireHash.contentEquals(expectedAcknowledgementHash!!))
        return true
    }

    fun persistAcknowledgement(persist: (ByteArray) -> Unit) {
        val expectedLength = expectedAcknowledgementLength
            ?: throw IllegalStateException("No complete NFC acknowledgement")
        require(acknowledgementBuffer.size == expectedLength)
        val candidate = IrohaPeerNfcSenderCheckpointV1(
            checkpoint.identity.sessionId,
            checkpoint.receiveRequest.encode(),
            checkpoint.payment.encode(),
            acknowledgementBuffer,
            checkpoint.profilePolicy,
            limits,
        )
        require(candidate.durableAcknowledgement!!.wireHash.contentEquals(expectedAcknowledgementHash!!))
        persist(candidate.encode())
        checkpoint = candidate
        resetAcknowledgementBuffer()
    }

    private fun requireAckMetadata(status: IrohaPeerNfcStatusV1) {
        require(status.paymentProfile == checkpoint.payment.canonicalPayload.profile &&
            status.paymentLength == checkpoint.payment.encode().size &&
            status.paymentWireHash.contentEquals(checkpoint.payment.wireHash) &&
            status.acknowledgementProfile != null &&
            checkpoint.profilePolicy.accepts(status.acknowledgementProfile) &&
            status.acknowledgementLength in
            (IrohaPeerWireMessageV1.HEADER_LENGTH + 1)..limits.maximumMessageBytes)
    }

    private fun configureAcknowledgementBuffer(status: IrohaPeerNfcStatusV1) {
        if (expectedAcknowledgementLength != status.acknowledgementLength ||
            !expectedAcknowledgementHash.contentEqualsNfc(status.acknowledgementWireHash)) {
            acknowledgementBuffer.fill(0)
            acknowledgementBuffer = byteArrayOf()
            expectedAcknowledgementLength = status.acknowledgementLength
            expectedAcknowledgementHash = status.acknowledgementWireHash
        }
    }

    private fun resetAcknowledgementBuffer() {
        acknowledgementBuffer.fill(0)
        acknowledgementBuffer = byteArrayOf()
        expectedAcknowledgementLength = null
        expectedAcknowledgementHash?.fill(0)
        expectedAcknowledgementHash = null
    }
}

/**
 * Complete, durable, status-authoritative NFC reader exchange.
 *
 * A fresh request crosses one transactional load-or-create boundary that
 * returns an already-durable exact ISC1 before BEGIN_PAYMENT. Successful
 * contiguous WRITE and ACK reads are then burst using min(local, peer) chunk
 * sizes. GET_STATUS remains authoritative at phase boundaries and after a new
 * invocation, so an ambiguous RF loss resumes only from the exact restored
 * checkpoint. The ACK-bearing ISC1 is a separate monotonic durable update
 * before CONFIRM_ACK. A successful CONFIRM, or an ambiguous transport loss
 * after that cleanup-only command is emitted, returns immediately; an explicit
 * peer error status still fails.
 */
object IrohaPeerNfcReaderExchangeV1 {
    /**
     * Covers three protocol-maximum messages at the minimum one-byte chunk,
     * plus SELECT/INFO, phase probes, controls, and durable transitions.
     */
    const val DEFAULT_MAXIMUM_ACTIONS =
        3 * IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES + 16

    @JvmStatic
    @JvmOverloads
    fun run(
        profilePolicy: IrohaPeerNfcProfilePolicyV1,
        transceiver: IrohaPeerNfcReaderTransceiverV1,
        checkpointStore: IrohaPeerNfcSenderCheckpointStoreV1,
        checkpointUpdater: IrohaPeerNfcSenderCheckpointUpdaterV1,
        restoredCheckpoint: ByteArray? = null,
        limits: IrohaPeerNfcLimitsV1 = IrohaPeerNfcLimitsV1.DEFAULT,
        maximumActions: Int = DEFAULT_MAXIMUM_ACTIONS,
    ): IrohaPeerNfcReaderExchangeResultV1 {
        require(maximumActions > 0) { "NFC action budget must be positive" }
        var remainingActions = maximumActions
        fun consumeAction() {
            check(remainingActions > 0) { "NFC action budget exhausted" }
            remainingActions -= 1
        }

        consumeAction()
        requireEmptySuccess(transceiver.transceive(IrohaPeerNfcCommandV1.SELECT_APPLICATION))
        consumeAction()
        val info = IrohaPeerNfcInfoV1.decode(
            requireSuccess(transceiver.transceive(IrohaPeerNfcCommandV1.GET_INFO)),
        )
        require(info.identity.profile == profilePolicy.profile) {
            "NFC request profile mismatch"
        }
        require(info.requestLength <= limits.maximumMessageBytes) {
            "NFC request exceeds local message bound"
        }
        if (restoredCheckpoint == null) {
            require(info.phase == IrohaPeerNfcPhaseV1.REQUEST_READY) {
                "A later NFC phase requires the exact restored ISC1 checkpoint"
            }
        }

        val checkpoint = if (restoredCheckpoint != null) {
            IrohaPeerNfcSenderCheckpointV1.decode(
                restoredCheckpoint.copyOf(),
                profilePolicy,
                limits,
            ).also { IrohaPeerNfcTwoTapReducerV1(it, limits).requireSamePeer(info) }
        } else {
            val requestOutput = ByteArrayOutputStream(info.requestLength)
            while (requestOutput.size() < info.requestLength) {
                val command = IrohaPeerNfcReaderPlanningV1.readRequestCommand(
                    info,
                    requestOutput.size(),
                    limits,
                )
                consumeAction()
                val chunk = requireSuccess(transceiver.transceive(command))
                require(chunk.isNotEmpty() && chunk.size <= command.length &&
                    chunk.size <= info.requestLength - requestOutput.size()) {
                    "Invalid NFC request chunk"
                }
                requestOutput.write(chunk)
            }
            val requestBytes = requestOutput.toByteArray()
            val request = decodeNfcMessage(
                requestBytes,
                info.identity.profile,
                IrohaPeerPayloadKind.RECEIVE_REQUEST,
                limits,
            )
            require(requestBytes.size == info.requestLength &&
                request.canonicalHash.contentEquals(info.identity.requestCanonicalHash) &&
                request.wireHash.contentEquals(info.identity.requestWireHash)) {
                "NFC receive request does not match INF1"
            }
            consumeAction()
            checkpointStore.loadOrCreateDurableCheckpoint(info, request).also { created ->
                require(created.profilePolicy == profilePolicy && created.receiveRequest == request) {
                    "NFC durable checkpoint does not match the exact request policy"
                }
                IrohaPeerNfcTwoTapReducerV1(created, limits).requireSamePeer(info)
            }
        }

        val reducer = IrohaPeerNfcTwoTapReducerV1(checkpoint, limits)

        while (true) {
            consumeAction()
            val status = IrohaPeerNfcStatusV1.decode(requireSuccess(transceiver.transceive(
                IrohaPeerNfcReaderPlanningV1.getStatusCommand(info),
            )))
            val action = reducer.nextAction(status)
            consumeAction()
            when (action) {
                is IrohaPeerNfcSenderActionV1.Send -> {
                    val rawResponse = try {
                        transceiver.transceive(action.command)
                    } catch (failure: Exception) {
                        if (failure is IrohaPeerNfcAmbiguousResponseErrorV1 &&
                            action.command.type ==
                            IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT) {
                            reducer.checkpoint.durableAcknowledgement?.let { durable ->
                                // Both wallets already hold the exact payment/ACK durably.
                                // CONFIRM is cleanup-only, so an ambiguous RF loss after
                                // emission is financially complete. A returned error status
                                // is still validated below and remains a failure.
                                return IrohaPeerNfcReaderExchangeResultV1(
                                    reducer.checkpoint,
                                    durable,
                                    IrohaPeerNfcConfirmationStateV1.RESPONSE_UNKNOWN,
                                )
                            }
                        }
                        throw failure
                    }
                    val response = requireSuccess(rawResponse)
                    if (action.command.type != IrohaPeerNfcCommandTypeV1.READ_ACKNOWLEDGEMENT) {
                        require(response.isEmpty()) { "Unexpected NFC control response data" }
                    }
                    when (action.command.type) {
                        IrohaPeerNfcCommandTypeV1.WRITE -> {
                            val paymentBytes = reducer.checkpoint.payment.encode()
                            var offset = action.command.offset.nfcCheckedInt() + action.command.bytes.size
                            require(offset <= paymentBytes.size) { "Invalid NFC write offset" }
                            while (offset < paymentBytes.size) {
                                val count = minOf(
                                    limits.maximumWriteChunkBytes,
                                    status.maximumWriteChunkBytes,
                                    paymentBytes.size - offset,
                                )
                                require(count > 0 && offset <= 0xffff_ffffL) {
                                    "Invalid NFC write offset"
                                }
                                consumeAction()
                                requireEmptySuccess(transceiver.transceive(IrohaPeerNfcCommandV1.write(
                                    reducer.checkpoint.identity.sessionId,
                                    reducer.checkpoint.payment.wireHash,
                                    offset.toLong(),
                                    paymentBytes.copyOfRange(offset, offset + count),
                                )))
                                offset += count
                            }
                            consumeAction()
                            requireEmptySuccess(transceiver.transceive(IrohaPeerNfcCommandV1.commit(
                                reducer.checkpoint.identity.sessionId,
                                reducer.checkpoint.identity.requestCanonicalHash,
                                reducer.checkpoint.payment.wireHash,
                            )))
                        }

                        IrohaPeerNfcCommandTypeV1.READ_ACKNOWLEDGEMENT -> {
                            require(response.size <= action.command.length) {
                                "Invalid NFC acknowledgement chunk"
                            }
                            reducer.consumeAcknowledgementChunk(response)
                            var acknowledgementAction = reducer.nextAction(status)
                            while (true) {
                                consumeAction()
                                when (acknowledgementAction) {
                                    is IrohaPeerNfcSenderActionV1.Send -> when (
                                        acknowledgementAction.command.type
                                    ) {
                                        IrohaPeerNfcCommandTypeV1.READ_ACKNOWLEDGEMENT -> {
                                            val chunk = requireSuccess(transceiver.transceive(
                                                acknowledgementAction.command,
                                            ))
                                            require(chunk.size <= acknowledgementAction.command.length) {
                                                "Invalid NFC acknowledgement chunk"
                                            }
                                            reducer.consumeAcknowledgementChunk(chunk)
                                        }

                                        IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT -> {
                                            val durable = checkNotNull(
                                                reducer.checkpoint.durableAcknowledgement,
                                            ) { "NFC acknowledgement is not durable locally" }
                                            val confirmResponse = try {
                                                transceiver.transceive(acknowledgementAction.command)
                                            } catch (failure: Exception) {
                                                // The exact ACK-bearing ISC1 was persisted before
                                                // this command was emitted. A missing transport
                                                // response cannot make the value transfer incomplete.
                                                if (failure !is IrohaPeerNfcAmbiguousResponseErrorV1) {
                                                    throw failure
                                                }
                                                return IrohaPeerNfcReaderExchangeResultV1(
                                                    reducer.checkpoint,
                                                    durable,
                                                    IrohaPeerNfcConfirmationStateV1.RESPONSE_UNKNOWN,
                                                )
                                            }
                                            // Keep an explicit peer rejection distinct from an
                                            // ambiguous transport loss.
                                            requireEmptySuccess(confirmResponse)
                                            return IrohaPeerNfcReaderExchangeResultV1(
                                                reducer.checkpoint,
                                                durable,
                                            )
                                        }

                                        else -> error("Invalid NFC acknowledgement action")
                                    }

                                    is IrohaPeerNfcSenderActionV1.PersistAcknowledgement ->
                                        reducer.persistAcknowledgement(
                                            checkpointUpdater::updateDurableCheckpoint,
                                        )

                                    is IrohaPeerNfcSenderActionV1.Complete ->
                                        error("NFC completed before acknowledgement confirmation")
                                }
                                acknowledgementAction = reducer.nextAction(status)
                            }
                        }

                        IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT -> {
                            val durable = checkNotNull(reducer.checkpoint.durableAcknowledgement) {
                                "NFC acknowledgement is not durable locally"
                            }
                            return IrohaPeerNfcReaderExchangeResultV1(
                                reducer.checkpoint,
                                durable,
                            )
                        }

                        else -> Unit // BEGIN_PAYMENT and COMMIT are phase boundaries.
                    }
                }

                is IrohaPeerNfcSenderActionV1.PersistAcknowledgement ->
                    reducer.persistAcknowledgement(
                        checkpointUpdater::updateDurableCheckpoint,
                    )

                is IrohaPeerNfcSenderActionV1.Complete -> {
                    val durable = checkNotNull(reducer.checkpoint.durableAcknowledgement) {
                        "NFC acknowledgement is not durable locally"
                    }
                    return IrohaPeerNfcReaderExchangeResultV1(reducer.checkpoint, durable)
                }
            }
        }
    }

    private fun requireSuccess(response: IrohaPeerNfcReaderResponseV1): ByteArray {
        if (response.status != IrohaPeerNfcReaderStatusV1.SUCCESS) {
            throw IrohaPeerNfcReaderStatusExceptionV1(response.status)
        }
        return response.data
    }

    private fun requireEmptySuccess(response: IrohaPeerNfcReaderResponseV1) {
        require(requireSuccess(response).isEmpty()) { "Unexpected NFC control response data" }
    }
}

private fun requireNfcSession(value: ByteArray) {
    require(value.size == IrohaPeerNfcV1.SESSION_ID_BYTES && value.any { it.toInt() != 0 }) {
        "Invalid NFC session"
    }
}

private fun requireNfcHash(value: ByteArray) {
    require(value.size == IrohaPeerNfcV1.HASH_BYTES && value.any { it.toInt() != 0 }) {
        "Invalid NFC hash"
    }
}

private fun requireNfcChunkLimit(value: Int) {
    require(value in 1..IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES) { "Invalid NFC chunk limit" }
}

private fun Long.nfcCheckedInt(): Int {
    require(this in 0..Int.MAX_VALUE.toLong()) { "NFC length is out of range" }
    return toInt()
}

private fun ByteArray.nfcReadU16(offset: Int): Int =
    ((this[offset].toInt() and 0xff) shl 8) or (this[offset + 1].toInt() and 0xff)

private fun ByteArray.nfcReadU32(offset: Int): Long =
    ((this[offset].toLong() and 0xff) shl 24) or
        ((this[offset + 1].toLong() and 0xff) shl 16) or
        ((this[offset + 2].toLong() and 0xff) shl 8) or
        (this[offset + 3].toLong() and 0xff)

private fun ByteArrayOutputStream.nfcWriteU16(value: Int) {
    write(value ushr 8)
    write(value)
}

private fun ByteArrayOutputStream.nfcWriteU32(value: Long) {
    write((value ushr 24).toInt())
    write((value ushr 16).toInt())
    write((value ushr 8).toInt())
    write(value.toInt())
}
