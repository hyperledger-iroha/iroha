package org.hyperledger.iroha.sdk.tx.norito

import java.util.LinkedHashMap
import java.util.Optional
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.MultisigMemberPayload
import org.hyperledger.iroha.sdk.address.MultisigPolicyPayload
import org.hyperledger.iroha.sdk.address.PublicKeyPayload
import org.hyperledger.iroha.sdk.address.algorithmForCurveId
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

internal class TransactionPayloadAdapter : TypeAdapter<TransactionPayload> {

    override fun encode(encoder: NoritoEncoder, value: TransactionPayload) {
        encodeSizedField(encoder, CHAIN_ID_ADAPTER, value.chainId)
        encodeSizedField(encoder, ACCOUNT_ID_ADAPTER, value.authority)
        encodeSizedField(encoder, UINT64_ADAPTER, value.creationTimeMs)
        encodeSizedField(encoder, EXECUTABLE_ADAPTER, value.executable)
        encodeSizedField(encoder, TTL_ADAPTER, Optional.ofNullable(value.timeToLiveMs))
        encodeSizedField(encoder, NONCE_ADAPTER, Optional.ofNullable(value.nonce?.toLong()))
        encodeSizedField(encoder, METADATA_ADAPTER, value.metadata)
    }

    override fun decode(decoder: NoritoDecoder): TransactionPayload {
        val chainId = decodeSizedField(decoder, CHAIN_ID_ADAPTER)
        val authority = decodeAuthorityField(decoder)
        val creationTimeMs = decodeSizedField(decoder, UINT64_ADAPTER)
        val executable = decodeSizedField(decoder, EXECUTABLE_ADAPTER)
        val ttl: Optional<Long> = decodeSizedField(decoder, TTL_ADAPTER)
        val nonceRaw: Optional<Long> = decodeSizedField(decoder, NONCE_ADAPTER)
        val metadata = LinkedHashMap(decodeSizedField(decoder, METADATA_ADAPTER))

        return TransactionPayload(
            chainId = chainId,
            authority = authority,
            creationTimeMs = creationTimeMs,
            executable = executable,
            timeToLiveMs = ttl.orElse(null),
            nonce = nonceRaw.map { Math.toIntExact(it) }.orElse(null),
            metadata = metadata,
        )
    }

    private class InstructionAdapter : TypeAdapter<InstructionBox> {
        override fun encode(encoder: NoritoEncoder, value: InstructionBox) {
            val payload = value.payload
            if (payload is WirePayload) {
                require(isWirePayloadCandidate(payload.wireName, payload.payloadBytes)) {
                    "Wire payload must include a valid Norito header"
                }
                encodeSizedField(encoder, STRING_ADAPTER, payload.wireName)
                encodeSizedField(encoder, RAW_BYTE_VEC_ADAPTER, payload.payloadBytes)
                return
            }
            throw IllegalArgumentException("Instruction payload must be wire-framed")
        }

        override fun decode(decoder: NoritoDecoder): InstructionBox {
            val payload = decoder.readBytes(decoder.remaining())
            require(payload.isNotEmpty()) { "Instruction payload must not be empty" }
            return tryDecodeWireInstruction(payload, decoder.flags, decoder.flagsHint)
                ?: throw IllegalArgumentException("Instruction payload must be wire-framed")
        }
    }

    private class ExecutableAdapter : TypeAdapter<Executable> {
        override fun encode(encoder: NoritoEncoder, value: Executable) {
            encodeExecutable(encoder, value)
        }

        override fun decode(decoder: NoritoDecoder): Executable {
            return decodeExecutable(decoder)
        }
    }

    private class AccountIdAdapter : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            CONTROLLER_ADAPTER.encode(encoder, parseAuthority(value))
        }

        override fun decode(decoder: NoritoDecoder): String {
            val payload = decoder.readBytes(decoder.remaining())
            return decodePayload(payload, decoder.flags, decoder.flagsHint)
        }

        companion object {
            private const val SINGLE_CONTROLLER_TAG = 0L
            private const val MULTISIG_CONTROLLER_TAG = 1L
            private val CONTROLLER_ADAPTER: TypeAdapter<ControllerPayload> = AccountControllerAdapter()
            private val MULTISIG_POLICY_ADAPTER: TypeAdapter<MultisigPolicyPayload> = MultisigPolicyNoritoAdapter()
            private val MULTISIG_MEMBER_ADAPTER: TypeAdapter<MultisigMemberPayload> = MultisigMemberNoritoAdapter()
            private val MULTISIG_MEMBER_LIST_ADAPTER: TypeAdapter<List<MultisigMemberPayload>> =
                NoritoAdapters.sequence(MULTISIG_MEMBER_ADAPTER)

            fun decodePayload(payload: ByteArray, flags: Int, flagsHint: Int): String {
                val decoder = NoritoDecoder(payload, flags, flagsHint)
                val controller = decodeControllerPayload(decoder)
                require(decoder.remaining() == 0) { "Trailing bytes after authority payload" }
                return renderAuthority(controller)
            }

            private fun decodeControllerPayload(decoder: NoritoDecoder): ControllerPayload {
                val controllerTag = ENUM_TAG_ADAPTER.decode(decoder)
                return when (controllerTag) {
                    SINGLE_CONTROLLER_TAG -> {
                        val publicKeyLiteral = decodeSizedField(decoder, STRING_ADAPTER)
                        ControllerPayload.single(publicKeyLiteral)
                    }
                    MULTISIG_CONTROLLER_TAG -> {
                        val policy = decodeSizedField(decoder, MULTISIG_POLICY_ADAPTER)
                        ControllerPayload.multisig(policy)
                    }
                    else -> throw IllegalArgumentException("Unsupported AccountController tag: $controllerTag")
                }
            }

            private fun parseAuthority(authority: String): ControllerPayload {
                val canonicalAuthority = requireCanonicalI105Address(authority, "authority")
                val parsed = try {
                    AccountAddress.parseEncodedIgnoringCurveSupport(canonicalAuthority, null)
                } catch (e: AccountAddressException) {
                    throw IllegalArgumentException("authority must use canonical I105 encoding", e)
                }
                return parseAddressToController(parsed.address)
            }

            private fun parseAddressToController(address: AccountAddress): ControllerPayload {
                try {
                    val singlePayload = address.singleKeyPayloadIgnoringCurveSupport()
                    if (singlePayload != null) {
                        val publicKey = encodePublicKeyMultihash(singlePayload.curveId, singlePayload.publicKey)
                        return ControllerPayload.single(publicKey)
                    }
                    val multisigPayload = address.multisigPolicyPayloadIgnoringCurveSupport()
                    if (multisigPayload != null) {
                        return ControllerPayload.multisig(multisigPayload)
                    }
                } catch (e: AccountAddressException) {
                    throw IllegalArgumentException(
                        "Failed to extract controller from canonical I105 account id",
                        e,
                    )
                }
                throw IllegalArgumentException(
                    "Address contains neither single-key nor multisig controller"
                )
            }

            private fun renderAuthority(controller: ControllerPayload): String {
                if (controller.isSingle) {
                    val publicKeyLiteral = controller.publicKeyLiteral!!
                    val payload = decodePublicKeyLiteral(publicKeyLiteral)
                        ?: throw IllegalArgumentException("Invalid single-key AccountController payload")
                    return renderSingleAuthority(payload)
                }
                return renderMultisigAuthority(controller.multisigPolicy!!)
            }

            private fun renderSingleAuthority(payload: PublicKeyPayload): String {
                val algorithm = algorithmForCurveId(payload.curveId)
                    ?: throw IllegalArgumentException(
                        "Unsupported curve id in AccountController payload: ${payload.curveId}"
                    )
                return try {
                    val address = AccountAddress.fromAccount(payload.keyBytes, algorithm)
                    address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
                } catch (e: AccountAddressException) {
                    throw IllegalArgumentException("Invalid single-key AccountController payload", e)
                }
            }

            private fun renderMultisigAuthority(policy: MultisigPolicyPayload): String {
                try {
                    val address = AccountAddress.fromMultisigPolicy(policy)
                    return address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
                } catch (ex: AccountAddressException) {
                    throw IllegalArgumentException("Invalid multisig policy for AccountId", ex)
                }
            }
        }

        private class ControllerPayload private constructor(
            val publicKeyLiteral: String?,
            val multisigPolicy: MultisigPolicyPayload?,
        ) {
            val isSingle: Boolean get() = multisigPolicy == null

            companion object {
                fun single(publicKeyLiteral: String): ControllerPayload {
                    require(publicKeyLiteral.isNotBlank()) { "public key literal must not be blank" }
                    return ControllerPayload(publicKeyLiteral.trim(), null)
                }

                fun multisig(multisigPolicy: MultisigPolicyPayload): ControllerPayload =
                    ControllerPayload(null, multisigPolicy)
            }
        }

        private class AccountControllerAdapter : TypeAdapter<ControllerPayload> {
            override fun encode(encoder: NoritoEncoder, value: ControllerPayload) {
                if (value.isSingle) {
                    ENUM_TAG_ADAPTER.encode(encoder, SINGLE_CONTROLLER_TAG)
                    encodeSizedField(encoder, STRING_ADAPTER, value.publicKeyLiteral!!)
                    return
                }
                ENUM_TAG_ADAPTER.encode(encoder, MULTISIG_CONTROLLER_TAG)
                encodeSizedField(encoder, MULTISIG_POLICY_ADAPTER, value.multisigPolicy!!)
            }

            override fun decode(decoder: NoritoDecoder): ControllerPayload {
                val controllerTag = ENUM_TAG_ADAPTER.decode(decoder)
                val controller = when (controllerTag) {
                    SINGLE_CONTROLLER_TAG -> {
                        val publicKeyLiteral = decodeSizedField(decoder, STRING_ADAPTER)
                        ControllerPayload.single(publicKeyLiteral)
                    }
                    MULTISIG_CONTROLLER_TAG -> {
                        val policy = decodeSizedField(decoder, MULTISIG_POLICY_ADAPTER)
                        ControllerPayload.multisig(policy)
                    }
                    else -> throw IllegalArgumentException("Unsupported AccountController tag: $controllerTag")
                }
                require(decoder.remaining() == 0) { "Trailing bytes after AccountController payload" }
                return controller
            }
        }

        private class MultisigPolicyNoritoAdapter : TypeAdapter<MultisigPolicyPayload> {
            override fun encode(encoder: NoritoEncoder, value: MultisigPolicyPayload) {
                UINT8_ADAPTER.encode(encoder, value.version.toLong())
                UINT16_ADAPTER.encode(encoder, value.threshold.toLong())
                MULTISIG_MEMBER_LIST_ADAPTER.encode(encoder, value.members)
            }

            override fun decode(decoder: NoritoDecoder): MultisigPolicyPayload {
                val version = Math.toIntExact(UINT8_ADAPTER.decode(decoder))
                val threshold = Math.toIntExact(UINT16_ADAPTER.decode(decoder))
                val members = MULTISIG_MEMBER_LIST_ADAPTER.decode(decoder)
                return MultisigPolicyPayload.of(version, threshold, members)
            }
        }

        private class MultisigMemberNoritoAdapter : TypeAdapter<MultisigMemberPayload> {
            override fun encode(encoder: NoritoEncoder, value: MultisigMemberPayload) {
                val publicKey = encodePublicKeyMultihash(value.curveId, value.publicKey)
                STRING_ADAPTER.encode(encoder, publicKey)
                UINT16_ADAPTER.encode(encoder, value.weight.toLong())
            }

            override fun decode(decoder: NoritoDecoder): MultisigMemberPayload {
                val publicKeyLiteral = STRING_ADAPTER.decode(decoder)
                val weight = Math.toIntExact(UINT16_ADAPTER.decode(decoder))
                val payload = decodePublicKeyLiteral(publicKeyLiteral)
                    ?: throw IllegalArgumentException("Invalid multisig member public key")
                return MultisigMemberPayload(payload.curveId, weight, payload.keyBytes)
            }
        }
    }

    private class DomainIdAdapter : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            encodeSizedField(encoder, STRING_ADAPTER, value)
        }

        override fun decode(decoder: NoritoDecoder): String {
            val payload = decoder.readBytes(decoder.remaining())
            return decodeDomainPayload(payload, decoder.flags, decoder.flagsHint)
        }

        companion object {
            fun decodeDomainPayload(payload: ByteArray, flags: Int, flagsHint: Int): String {
                try {
                    val child = NoritoDecoder(payload, flags, flagsHint)
                    val domain = decodeSizedField(child, STRING_ADAPTER)
                    require(child.remaining() == 0) { "Trailing bytes after DomainId payload" }
                    return domain
                } catch (_: IllegalArgumentException) {
                    val child = NoritoDecoder(payload, flags, flagsHint)
                    val domain = STRING_ADAPTER.decode(child)
                    require(child.remaining() == 0) { "Trailing bytes after DomainId payload" }
                    return domain
                }
            }
        }
    }

    private class ChainIdAdapter : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            encodeSizedField(encoder, STRING_ADAPTER, value)
        }

        override fun decode(decoder: NoritoDecoder): String {
            val payload = decoder.readBytes(decoder.remaining())
            val sized = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
            val value = decodeSizedField(sized, STRING_ADAPTER)
            require(sized.remaining() == 0) { "Trailing bytes after ChainId payload" }
            return value
        }
    }

    private class IvmBytecodeAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            encodeSizedField(encoder, RAW_BYTE_VEC_ADAPTER, value)
        }

        override fun decode(decoder: NoritoDecoder): ByteArray {
            val payload = decoder.readBytes(decoder.remaining())
            val sized = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
            val value = decodeSizedField(sized, RAW_BYTE_VEC_ADAPTER)
            require(sized.remaining() == 0) { "Trailing bytes after IVM payload" }
            return value
        }
    }

    private class JsonAdapter : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            encodeSizedField(encoder, JSON_STRING_ADAPTER, value)
        }

        override fun decode(decoder: NoritoDecoder): String {
            return decodeSizedField(decoder, JSON_STRING_ADAPTER)
        }

        override fun isSelfDelimiting(): Boolean = true
    }

    private class MetadataAdapter : TypeAdapter<Map<String, String>> {
        private val entryListAdapter: TypeAdapter<List<MetadataEntry>> =
            NoritoAdapters.sequence(MetadataEntryAdapter())

        override fun encode(encoder: NoritoEncoder, value: Map<String, String>) {
            val keys = value.keys.sorted()
            val entries = keys.map { key ->
                val entryValue = value[key]
                    ?: throw IllegalArgumentException("Metadata values must not be null")
                MetadataEntry(key, entryValue)
            }
            entryListAdapter.encode(encoder, entries)
        }

        override fun decode(decoder: NoritoDecoder): Map<String, String> {
            val entries = entryListAdapter.decode(decoder)
            val decoded = LinkedHashMap<String, String>(entries.size)
            for (entry in entries) {
                require(decoded.put(entry.key, entry.value) == null) { "Duplicate metadata key" }
            }
            return decoded
        }
    }

    private class MetadataEntry(val key: String, val value: String)

    private class MetadataEntryAdapter : TypeAdapter<MetadataEntry> {
        override fun encode(encoder: NoritoEncoder, value: MetadataEntry) {
            encodeSizedField(encoder, STRING_ADAPTER, value.key)
            encodeSizedField(encoder, JSON_ADAPTER, value.value)
        }

        override fun decode(decoder: NoritoDecoder): MetadataEntry {
            val key = decodeSizedField(decoder, STRING_ADAPTER)
            val value = decodeSizedField(decoder, JSON_ADAPTER)
            return MetadataEntry(key, value)
        }
    }

    private class JsonStringAdapter : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            STRING_ADAPTER.encode(encoder, encodeJsonString(value))
        }

        override fun decode(decoder: NoritoDecoder): String {
            val raw = STRING_ADAPTER.decode(decoder)
            return decodeJsonString(raw)
        }

        override fun isSelfDelimiting(): Boolean = true
    }

    companion object {
        private val STRING_ADAPTER: TypeAdapter<String> = NoritoAdapters.stringAdapter()
        private val DOMAIN_ID_ADAPTER: TypeAdapter<String> = DomainIdAdapter()
        private val ACCOUNT_ID_ADAPTER: TypeAdapter<String> = AccountIdAdapter()
        private val CHAIN_ID_ADAPTER: TypeAdapter<String> = ChainIdAdapter()
        private val JSON_STRING_ADAPTER: TypeAdapter<String> = JsonStringAdapter()
        private val JSON_ADAPTER: TypeAdapter<String> = JsonAdapter()
        private val UINT64_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(64)
        private val UINT16_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(16)
        private val UINT8_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(8)
        private val RAW_BYTE_VEC_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.rawByteVecAdapter()
        private val IVM_BYTECODE_ADAPTER: TypeAdapter<ByteArray> = IvmBytecodeAdapter()
        private val INSTRUCTION_LIST_ADAPTER: TypeAdapter<List<InstructionBox>> =
            NoritoAdapters.sequence(InstructionAdapter())
        private val ENUM_TAG_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(32)
        private val TTL_ADAPTER: TypeAdapter<Optional<Long>> = NoritoAdapters.option(NoritoAdapters.uint(64))
        private val NONCE_ADAPTER: TypeAdapter<Optional<Long>> = NoritoAdapters.option(NoritoAdapters.uint(32))
        private val EXECUTABLE_ADAPTER: TypeAdapter<Executable> = ExecutableAdapter()
        private val METADATA_ADAPTER: TypeAdapter<Map<String, String>> = MetadataAdapter()

        private val HEX_DIGITS = "0123456789ABCDEF".toCharArray()

        private fun <T> encodeSizedField(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
            val child = encoder.childEncoder()
            adapter.encode(child, value)
            val payload = child.toByteArray()
            val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
            encoder.writeLength(payload.size.toLong(), compact)
            encoder.writeBytes(payload)
        }

        private fun <T> decodeSizedField(decoder: NoritoDecoder, adapter: TypeAdapter<T>): T {
            val length = decoder.readLength(decoder.compactLenActive())
            require(length <= Int.MAX_VALUE) { "Field payload too large" }
            val payload = decoder.readBytes(length.toInt())
            val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
            val value = adapter.decode(child)
            require(child.remaining() == 0) { "Trailing bytes after field payload" }
            return value
        }

        private fun decodeAuthorityField(decoder: NoritoDecoder): String {
            val length = decoder.readLength(decoder.compactLenActive())
            require(length <= Int.MAX_VALUE) { "Field payload too large" }
            val payload = decoder.readBytes(length.toInt())
            return AccountIdAdapter.decodePayload(payload, decoder.flags, decoder.flagsHint)
        }

        private fun encodeExecutable(encoder: NoritoEncoder, executable: Executable) {
            when (executable) {
                is Executable.Ivm -> {
                    ENUM_TAG_ADAPTER.encode(encoder, 1L)
                    encodeSizedField(encoder, IVM_BYTECODE_ADAPTER, executable.ivmBytes)
                }
                is Executable.Instructions -> {
                    ENUM_TAG_ADAPTER.encode(encoder, 0L)
                    encodeSizedField(encoder, INSTRUCTION_LIST_ADAPTER, executable.instructions)
                }
            }
        }

        private fun decodeExecutable(decoder: NoritoDecoder): Executable {
            val tag = ENUM_TAG_ADAPTER.decode(decoder)
            return when (tag) {
                1L -> {
                    val bytes = decodeSizedField(decoder, IVM_BYTECODE_ADAPTER)
                    Executable.ivm(bytes)
                }
                0L -> {
                    val instructions = decodeSizedField(decoder, INSTRUCTION_LIST_ADAPTER)
                    Executable.instructions(instructions)
                }
                else -> throw IllegalArgumentException("Unknown Executable discriminant: $tag")
            }
        }

        private fun tryDecodeWireInstruction(payload: ByteArray, flags: Int, flagsHint: Int): InstructionBox? {
            return try {
                val wireDecoder = NoritoDecoder(payload, flags, flagsHint)
                val wireName = decodeSizedField(wireDecoder, STRING_ADAPTER)
                val wirePayload = decodeSizedField(wireDecoder, RAW_BYTE_VEC_ADAPTER)
                if (wireDecoder.remaining() != 0) return null
                if (!isWirePayloadCandidate(wireName, wirePayload)) return null
                InstructionBox.fromWirePayload(wireName, wirePayload)
            } catch (_: IllegalArgumentException) {
                null
            }
        }

        private fun isWirePayloadCandidate(wireName: String?, payload: ByteArray?): Boolean {
            if (wireName.isNullOrBlank()) return false
            if (payload == null || payload.size < NoritoHeader.HEADER_LENGTH) return false
            if (payload[0] != 'N'.code.toByte() || payload[1] != 'R'.code.toByte() ||
                payload[2] != 'T'.code.toByte() || payload[3] != '0'.code.toByte()
            ) return false
            return try {
                val decoded = NoritoHeader.decode(payload, null)
                decoded.header.validateChecksum(decoded.payload)
                true
            } catch (_: IllegalArgumentException) {
                false
            }
        }

        private fun encodeJsonString(value: String): String {
            val builder = StringBuilder(value.length + 2)
            builder.append('"')
            for (c in value) {
                when (c) {
                    '"' -> builder.append("\\\"")
                    '\\' -> builder.append("\\\\")
                    '\b' -> builder.append("\\b")
                    '\u000C' -> builder.append("\\f")
                    '\n' -> builder.append("\\n")
                    '\r' -> builder.append("\\r")
                    '\t' -> builder.append("\\t")
                    else -> {
                        if (c < '\u0020') {
                            builder.append("\\u00")
                            builder.append(HEX_DIGITS[(c.code shr 4) and 0xF])
                            builder.append(HEX_DIGITS[c.code and 0xF])
                        } else {
                            builder.append(c)
                        }
                    }
                }
            }
            builder.append('"')
            return builder.toString()
        }

        private fun decodeJsonString(raw: String?): String {
            if (raw == null) return raw.toString()
            val trimmed = raw.trim()
            if (trimmed.length < 2 || trimmed[0] != '"' || trimmed[trimmed.length - 1] != '"') return raw
            return try {
                parseJsonString(trimmed)
            } catch (_: IllegalArgumentException) {
                raw
            }
        }

        private fun parseJsonString(input: String): String {
            val builder = StringBuilder()
            var i = 1
            while (i < input.length - 1) {
                val c = input[i++]
                if (c == '\\') {
                    require(i < input.length - 1) { "Invalid JSON escape" }
                    val esc = input[i++]
                    when (esc) {
                        '"' -> builder.append('"')
                        '\\' -> builder.append('\\')
                        '/' -> builder.append('/')
                        'b' -> builder.append('\b')
                        'f' -> builder.append('\u000C')
                        'n' -> builder.append('\n')
                        'r' -> builder.append('\r')
                        't' -> builder.append('\t')
                        'u' -> {
                            require(i + 4 <= input.length - 1) { "Invalid unicode escape" }
                            var codePoint = 0
                            for (j in 0 until 4) {
                                codePoint = (codePoint shl 4) or hexNibble(input[i + j])
                            }
                            builder.append(codePoint.toChar())
                            i += 4
                        }
                        else -> throw IllegalArgumentException("Unsupported escape: \\$esc")
                    }
                } else {
                    builder.append(c)
                }
            }
            return builder.toString()
        }

        private fun hexNibble(c: Char): Int = when (c) {
            in '0'..'9' -> c - '0'
            in 'a'..'f' -> 10 + (c - 'a')
            in 'A'..'F' -> 10 + (c - 'A')
            else -> throw IllegalArgumentException("Invalid hex digit: $c")
        }
    }
}
