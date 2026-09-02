package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest

/**
 * Fail-closed Android entry point for the Offline Cash V1 secure-device lifecycle.
 *
 * Android KeyMint single-use signing keys do not provide the atomic journal, authenticated
 * multi-credit inbox, trusted clock, exact-next counter, hardware-epoch rotation, or authenticated
 * payment outbox required by Offline Cash V1. This bridge therefore becomes available only when
 * the loaded native bridge exposes the complete hardware capability frame. Missing symbols,
 * partial capabilities, malformed replies, and every native failure leave the wallet online-only.
 * There is no software backend or downgrade path.
 */
class OfflineCashDeviceLifecycleBridgeV1 private constructor(
    private val endpoint: Endpoint?,
    private val acceptedCapabilities: Capabilities?,
) {
    /** Whether this device may execute the complete offline lifecycle. */
    enum class Availability {
        /** No qualifying secure backend is present; ordinary online wallet use remains valid. */
        ONLINE_ONLY,

        /** The exact native secure-backend capability contract matched structurally. */
        AVAILABLE,
    }

    /**
     * Exact operations in Core's sealed reservation, transition, and recovery flow.
     *
     * The service reserves bytes before accepting authority, prepares a deterministic candidate
     * before consuming a predecessor, commits that candidate exactly once, and recovers every
     * terminal certificate and installed envelope byte-identically.
     */
    enum class Operation(val code: Int) {
        READ_ACTIVE_HARDWARE_CREDENTIAL(1),
        PREPARE_ACCEPTANCE_INTENT_AUTHORIZATION(2),
        RECOVER_ACCEPTANCE_INTENT_AUTHORIZATION(3),
        VERIFY_AUTHORIZATION_RESERVE_INBOX_AND_ISSUE_ACCEPTANCE_TICKET(4),
        RECOVER_ACCEPTANCE_TICKET(5),
        STAGE_INBOUND_PAYMENT(6),
        RECOVER_STAGED_INBOUND_PAYMENT(7),
        RECOVER_INBOUND_INBOX_PAGE(8),
        PREPARE_EXACT_NEXT_TRANSITION(9),
        RECOVER_PREPARED_TRANSITION(10),
        ABANDON_UNCOMMITTED_PREPARED_TRANSITION(11),
        COMMIT_VERIFIED_CANDIDATE(12),
        RECOVER_TERMINAL_COMMIT_CERTIFICATE(13),
        INSTALL_FINAL_COMMIT_WRAPPER(14),
        RECOVER_INSTALLED_ENVELOPE_OR_STATE_PROOF(15),
        SIGN_RECEIVE_ACKNOWLEDGEMENT(16),
        RELEASE_OUTBOX_ENTRY(17),
        READ_TRUSTED_TIME_OR_LEASE(18),
        PREPARE_MINT_AUTHORIZATION(19),
        RECOVER_MINT_AUTHORIZATION(20),
        VERIFY_AUTHORIZATION_AND_STAGE_MINT_CREDIT(21),
        FOLD_RECEIVE(22),
        READ_PENDING_CREDIT_WATERMARK(23),
        ROTATE_HARDWARE_EPOCH(24),
    }

    /** Exact secure-backend capabilities required by Offline Cash V1. */
    enum class Capability(val mask: Int) {
        EXACT_NEXT_PREDECESSOR_CONSUMPTION(1 shl 0),
        ONE_USE_SUCCESSOR_AUTHORIZATION(1 shl 1),
        ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL(1 shl 2),
        SEALED_TRANSITION_RECOVERY(1 shl 3),
        ONE_USE_ACCEPTANCE_TICKETS(1 shl 4),
        DURABLE_INBOX_RESERVATION(1 shl 5),
        AUTHENTICATED_INBOUND_STAGING(1 shl 6),
        AUTHORITATIVE_REPLAY_ROOT_RECOVERY(1 shl 7),
        SENDER_OUTBOX_RESERVATION(1 shl 8),
        AUTHENTICATED_DURABLE_RETRY_OUTBOX(1 shl 9),
        ATOMIC_VERIFIED_CANDIDATE_COMMIT(1 shl 10),
        RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE(1 shl 11),
        TRUSTED_TIME_OR_LEASE(1 shl 12),
        OFFLINE_HARDWARE_EPOCH_ROTATION(1 shl 13),
        ROLLBACK_SAFE_COUNTER_ROLLOVER(1 shl 14),
        NO_SOFTWARE_FALLBACK(1 shl 15),
    }

    /** Stable native failure classification; only [SUCCESS] may carry authoritative bytes. */
    enum class Status(val code: Int) {
        SUCCESS(0),
        UNAVAILABLE(1),
        STALE_OR_CONCURRENT(2),
        BINDING_MISMATCH(3),
        TRUSTED_TIME_REJECTED(4),
        REJECTED(5),
        MISSING(6),
        CONFLICT(7),
        CORRUPT(8),
        MALFORMED_REQUEST(9),
        RECOVERY_REQUIRED(10),
    }

    /** Accepted, exact secure-backend identity. */
    class Capabilities internal constructor(
        hardwarePolicyId: ByteArray,
        attestationDigest: ByteArray,
    ) {
        init {
            requireDigest(hardwarePolicyId, "hardwarePolicyId")
            requireDigest(attestationDigest, "attestationDigest")
        }

        private val policy = hardwarePolicyId.copyOf()
        private val attestation = attestationDigest.copyOf()

        fun hardwarePolicyId(): ByteArray = policy.copyOf()

        fun attestationDigest(): ByteArray = attestation.copyOf()
    }

    /** Bounded result returned by the secure backend. */
    class Result internal constructor(
        val operation: Operation,
        val status: Status,
        payload: ByteArray,
        authenticator: ByteArray,
    ) {
        // The decoder creates these arrays solely for this result and transfers
        // ownership exactly once. Public accessors still return caller-owned copies.
        private val payloadBytes = payload
        private val authenticatorBytes = authenticator

        fun payload(): ByteArray = payloadBytes.copyOf()

        fun authenticator(): ByteArray = authenticatorBytes.copyOf()
    }

    /** Stable local mode. Unsupported devices are intentionally not exceptional at discovery. */
    val availability: Availability =
        if (endpoint != null && acceptedCapabilities != null) {
            Availability.AVAILABLE
        } else {
            Availability.ONLINE_ONLY
        }

    /** The accepted hardware policy, or `null` while the device is online-only. */
    fun capabilities(): Capabilities? = acceptedCapabilities

    /**
     * Execute one exact canonical Core command.
     *
     * [requestId] is a non-zero 32-byte idempotency binding. [canonicalCommand] is the bounded,
     * canonical Offline Cash V1 command archive for [operation].
     */
    fun execute(
        operation: Operation,
        requestId: ByteArray,
        canonicalCommand: ByteArray,
    ): Result {
        val nativeEndpoint = endpoint
            ?: throw IllegalStateException(ONLINE_ONLY_MESSAGE)
        val request = Codec.encodeCommand(operation, requestId, canonicalCommand)
        val rawResponse = try {
            nativeEndpoint.execute(request)
        } catch (error: RuntimeException) {
            throw IllegalStateException("Offline Cash V1 secure backend execution failed", error)
        } catch (error: LinkageError) {
            throw IllegalStateException("Offline Cash V1 secure backend execution failed", error)
        } finally {
            request.fill(0)
        }
        return try {
            Codec.decodeResponse(rawResponse, operation, requestId)
        } finally {
            rawResponse.fill(0)
        }
    }

    internal interface Endpoint {
        fun capabilities(): ByteArray

        fun execute(command: ByteArray): ByteArray
    }

    companion object {
        const val PROTOCOL_VERSION: Int = 1
        const val MAXIMUM_COMMAND_PAYLOAD_BYTES: Int = 64 * 1024
        const val MAXIMUM_RESPONSE_PAYLOAD_BYTES: Int = 64 * 1024
        const val MAXIMUM_AUTHENTICATOR_BYTES: Int = 8 * 1024

        private const val LIBRARY_NAME = "connect_norito_bridge"
        private const val ANDROID_PLATFORM_CODE = 1
        private val REQUIRED_FEATURES = Capability.values().fold(0) { mask, capability ->
            mask or capability.mask
        }
        private const val ONLINE_ONLY_MESSAGE =
            "Offline Cash V1 requires a rollback-resistant secure journal/outbox backend; this device remains online-only"

        /** Discover the optional native secure backend without permitting a software fallback. */
        @JvmStatic
        fun production(): OfflineCashDeviceLifecycleBridgeV1 {
            val nativeEndpoint = NativeEndpoint.create() ?: return onlineOnly()
            val capabilities = runCatching {
                Codec.decodeCapabilities(nativeEndpoint.capabilities(), ANDROID_PLATFORM_CODE)
            }.getOrNull() ?: return onlineOnly()
            return OfflineCashDeviceLifecycleBridgeV1(nativeEndpoint, capabilities)
        }

        /** Explicit online-only instance for products that do not ship a qualifying backend. */
        @JvmStatic
        fun onlineOnly(): OfflineCashDeviceLifecycleBridgeV1 =
            OfflineCashDeviceLifecycleBridgeV1(null, null)

        internal fun withEndpointForTests(endpoint: Endpoint): OfflineCashDeviceLifecycleBridgeV1 {
            val capabilities = Codec.decodeCapabilities(endpoint.capabilities(), ANDROID_PLATFORM_CODE)
            return OfflineCashDeviceLifecycleBridgeV1(endpoint, capabilities)
        }
    }

    private object NativeEndpoint : Endpoint {
        // TODO: implement these optional JNI methods only for an audited OEM/StrongBox service
        // that can attest every required capability; stock AndroidKeyStore is insufficient.
        fun create(): Endpoint? = try {
            System.loadLibrary(LIBRARY_NAME)
            val capabilities = nativeCapabilitiesV1()
            if (capabilities == null) null else this
        } catch (_: RuntimeException) {
            null
        } catch (_: LinkageError) {
            null
        }

        override fun capabilities(): ByteArray =
            nativeCapabilitiesV1()
                ?: throw IllegalStateException("native Offline Cash V1 capabilities are unavailable")

        override fun execute(command: ByteArray): ByteArray {
            val nativeCommand = command.copyOf()
            return try {
                nativeExecuteV1(nativeCommand)
                    ?: throw IllegalStateException("native Offline Cash V1 execution returned no response")
            } finally {
                nativeCommand.fill(0)
            }
        }

        @JvmStatic
        private external fun nativeCapabilitiesV1(): ByteArray?

        @JvmStatic
        private external fun nativeExecuteV1(command: ByteArray): ByteArray?
    }

    internal object Codec {
        private val capabilityMagic = "IOCFJCP1".toByteArray(Charsets.US_ASCII)
        private val commandMagic = "IOCFJCM1".toByteArray(Charsets.US_ASCII)
        private val responseMagic = "IOCFJRS1".toByteArray(Charsets.US_ASCII)
        private const val CAPABILITY_BYTES = 96
        private const val COMMAND_HEADER_BYTES = 80
        private const val RESPONSE_HEADER_BYTES = 116

        fun decodeCapabilities(encoded: ByteArray, expectedPlatform: Int): Capabilities {
            require(encoded.size == CAPABILITY_BYTES) { "invalid Offline Cash V1 capability size" }
            val input = reader(encoded)
            requireMagic(input, capabilityMagic, "capabilities")
            require(readU16(input) == PROTOCOL_VERSION) { "unsupported Offline Cash device bridge version" }
            require(readU8(input) == expectedPlatform) { "Offline Cash device bridge platform mismatch" }
            require(readU8(input) == 0) { "non-canonical Offline Cash capability flags" }
            require(readU32(input) == REQUIRED_FEATURES.toLong()) { "incomplete Offline Cash secure backend" }
            require(readU32(input) == MAXIMUM_COMMAND_PAYLOAD_BYTES.toLong()) { "Offline Cash command bound mismatch" }
            require(readU32(input) == MAXIMUM_RESPONSE_PAYLOAD_BYTES.toLong()) { "Offline Cash response bound mismatch" }
            val policy = ByteArray(32).also(input::get)
            val attestation = ByteArray(32).also(input::get)
            require(readU64(input) == 0L) { "non-canonical Offline Cash capability trailer" }
            require(!policy.contentEquals(attestation)) { "Offline Cash policy and attestation bindings must differ" }
            return Capabilities(policy, attestation)
        }

        fun encodeCommand(
            operation: Operation,
            requestId: ByteArray,
            payload: ByteArray,
        ): ByteArray {
            requireDigest(requestId, "requestId")
            require(payload.isNotEmpty() && payload.size <= MAXIMUM_COMMAND_PAYLOAD_BYTES) {
                "canonicalCommand must contain 1..$MAXIMUM_COMMAND_PAYLOAD_BYTES bytes"
            }
            val output = writer(COMMAND_HEADER_BYTES + payload.size)
            output.put(commandMagic)
            writeU16(output, PROTOCOL_VERSION)
            writeU8(output, operation.code)
            writeU8(output, 0)
            output.put(requestId)
            writeU32(output, payload.size)
            output.put(sha256(payload))
            output.put(payload)
            return output.array()
        }

        fun decodeResponse(
            encoded: ByteArray,
            expectedOperation: Operation,
            expectedRequestId: ByteArray,
        ): Result {
            require(encoded.size >= RESPONSE_HEADER_BYTES) { "truncated Offline Cash V1 response" }
            require(encoded.size <= RESPONSE_HEADER_BYTES + MAXIMUM_RESPONSE_PAYLOAD_BYTES + MAXIMUM_AUTHENTICATOR_BYTES) {
                "oversized Offline Cash V1 response"
            }
            val input = reader(encoded)
            var payload = ByteArray(0)
            var authenticator = ByteArray(0)
            var transferred = false
            try {
                requireMagic(input, responseMagic, "response")
                require(readU16(input) == PROTOCOL_VERSION) { "unsupported Offline Cash device bridge version" }
                val operationCode = readU8(input)
                val operation = Operation.values().singleOrNull { it.code == operationCode }
                    ?: throw IllegalArgumentException("unknown Offline Cash V1 operation")
                require(operation == expectedOperation) { "Offline Cash response operation mismatch" }
                val statusCode = readU8(input)
                val status = Status.values().singleOrNull { it.code == statusCode }
                    ?: throw IllegalArgumentException("unknown Offline Cash V1 status")
                val requestId = ByteArray(32).also(input::get)
                require(MessageDigest.isEqual(requestId, expectedRequestId)) { "Offline Cash response request mismatch" }
                val payloadLength = readBoundedLength(input, MAXIMUM_RESPONSE_PAYLOAD_BYTES, "payload")
                val authenticatorLength = readBoundedLength(input, MAXIMUM_AUTHENTICATOR_BYTES, "authenticator")
                val payloadDigest = ByteArray(32).also(input::get)
                val authenticatorDigest = ByteArray(32).also(input::get)
                require(input.remaining() == payloadLength + authenticatorLength) { "Offline Cash response length mismatch" }
                payload = ByteArray(payloadLength).also(input::get)
                authenticator = ByteArray(authenticatorLength).also(input::get)
                require(MessageDigest.isEqual(payloadDigest, sha256(payload))) { "Offline Cash response payload digest mismatch" }
                require(MessageDigest.isEqual(authenticatorDigest, sha256(authenticator))) { "Offline Cash response authenticator digest mismatch" }
                if (status == Status.SUCCESS) {
                    require(payload.isNotEmpty()) { "successful Offline Cash response has no payload" }
                    require(authenticator.isNotEmpty() && authenticator.any { it != 0.toByte() }) {
                        "successful Offline Cash response is unauthenticated"
                    }
                } else {
                    require(payload.isEmpty() && authenticator.isEmpty()) {
                        "failed Offline Cash response must not expose bytes"
                    }
                }
                val result = Result(operation, status, payload, authenticator)
                transferred = true
                return result
            } finally {
                if (!transferred) {
                    payload.fill(0)
                    authenticator.fill(0)
                }
            }
        }

        internal fun encodeCapabilitiesForTests(
            platform: Int,
            policy: ByteArray,
            attestation: ByteArray,
        ): ByteArray {
            requireDigest(policy, "policy")
            requireDigest(attestation, "attestation")
            val output = writer(CAPABILITY_BYTES)
            output.put(capabilityMagic)
            writeU16(output, PROTOCOL_VERSION)
            writeU8(output, platform)
            writeU8(output, 0)
            writeU32(output, REQUIRED_FEATURES)
            writeU32(output, MAXIMUM_COMMAND_PAYLOAD_BYTES)
            writeU32(output, MAXIMUM_RESPONSE_PAYLOAD_BYTES)
            output.put(policy)
            output.put(attestation)
            writeU64(output, 0)
            return output.array()
        }

        internal fun encodeResponseForTests(
            operation: Operation,
            status: Status,
            requestId: ByteArray,
            payload: ByteArray,
            authenticator: ByteArray,
        ): ByteArray {
            val output = writer(RESPONSE_HEADER_BYTES + payload.size + authenticator.size)
            output.put(responseMagic)
            writeU16(output, PROTOCOL_VERSION)
            writeU8(output, operation.code)
            writeU8(output, status.code)
            output.put(requestId)
            writeU32(output, payload.size)
            writeU32(output, authenticator.size)
            output.put(sha256(payload))
            output.put(sha256(authenticator))
            output.put(payload)
            output.put(authenticator)
            return output.array()
        }

        private fun reader(bytes: ByteArray): ByteBuffer =
            ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)

        private fun writer(size: Int): ByteBuffer =
            ByteBuffer.allocate(size).order(ByteOrder.LITTLE_ENDIAN)

        private fun requireMagic(input: ByteBuffer, expected: ByteArray, label: String) {
            val actual = ByteArray(expected.size).also(input::get)
            require(actual.contentEquals(expected)) { "invalid Offline Cash V1 $label magic" }
        }

        private fun readBoundedLength(input: ByteBuffer, maximum: Int, label: String): Int {
            val value = readU32(input)
            require(value <= maximum.toLong()) { "Offline Cash response $label exceeds its bound" }
            return value.toInt()
        }

        private fun readU8(input: ByteBuffer): Int = input.get().toInt() and 0xff

        private fun readU16(input: ByteBuffer): Int = input.short.toInt() and 0xffff

        private fun readU32(input: ByteBuffer): Long = input.int.toLong() and 0xffff_ffffL

        private fun readU64(input: ByteBuffer): Long = input.long

        private fun writeU8(output: ByteBuffer, value: Int) = output.put(value.toByte())

        private fun writeU16(output: ByteBuffer, value: Int) = output.putShort(value.toShort())

        private fun writeU32(output: ByteBuffer, value: Int) = output.putInt(value)

        private fun writeU64(output: ByteBuffer, value: Long) = output.putLong(value)

        private fun sha256(bytes: ByteArray): ByteArray =
            MessageDigest.getInstance("SHA-256").digest(bytes)
    }
}

private fun requireDigest(value: ByteArray, field: String) {
    require(value.size == 32 && value.any { it != 0.toByte() }) {
        "$field must be exactly 32 non-zero bytes"
    }
}
