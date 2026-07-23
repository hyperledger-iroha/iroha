package org.hyperledger.iroha.sdk.tx.norito

import java.math.BigInteger
import java.util.LinkedHashMap
import java.util.Optional
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.MultisigMemberPayload
import org.hyperledger.iroha.sdk.address.MultisigPolicyPayload
import org.hyperledger.iroha.sdk.address.PublicKeyPayload
import org.hyperledger.iroha.sdk.address.algorithmForCurveId
import org.hyperledger.iroha.sdk.address.compactPublicKeyPayload
import org.hyperledger.iroha.sdk.address.decodeCompactPublicKeyPayload
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.ContractInvocation
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.ExecutableBatchItem
import org.hyperledger.iroha.sdk.core.model.FeeChargeKind
import org.hyperledger.iroha.sdk.core.model.FeeChargeLimit
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.FeeSponsorProgramId
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.MAX_CONTRACT_ARGUMENT_RECORD_BYTES
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

internal class TransactionPayloadAdapter : TypeAdapter<TransactionPayload> {

    override fun encode(encoder: NoritoEncoder, value: TransactionPayload) {
        require(!value.executable.requiresTransactionGasLimit() || value.feePayment.gasLimit != null) {
            "feePayment.gasLimit is required for IVM and contract-call executables"
        }
        encodeSizedField(encoder, CHAIN_ID_ADAPTER, value.chainId)
        encodeSizedField(encoder, ACCOUNT_ID_ADAPTER, value.authority)
        encodeSizedField(encoder, UINT64_ADAPTER, value.creationTimeMs)
        encodeSizedField(encoder, EXECUTABLE_ADAPTER, value.executable)
        encodeSizedField(encoder, TTL_ADAPTER, Optional.ofNullable(value.timeToLiveMs))
        encodeSizedField(encoder, NONCE_ADAPTER, Optional.ofNullable(value.nonce?.toLong()))
        encodeSizedField(encoder, FEE_PAYMENT_ADAPTER, value.feePayment)
        encodeSizedField(encoder, METADATA_ADAPTER, value.metadata)
    }

    override fun decode(decoder: NoritoDecoder): TransactionPayload {
        val chainId = decodeSizedField(decoder, CHAIN_ID_ADAPTER)
        val authority = decodeAuthorityField(decoder)
        val creationTimeMs = decodeSizedField(decoder, UINT64_ADAPTER)
        val executable = decodeSizedField(decoder, EXECUTABLE_ADAPTER)
        val ttl: Optional<Long> = decodeSizedField(decoder, TTL_ADAPTER)
        val nonceRaw: Optional<Long> = decodeSizedField(decoder, NONCE_ADAPTER)
        val feePayment = decodeSizedField(decoder, FEE_PAYMENT_ADAPTER)
        val metadata = LinkedHashMap(decodeSizedField(decoder, METADATA_ADAPTER))

        return TransactionPayload(
            chainId = chainId,
            authority = authority,
            creationTimeMs = creationTimeMs,
            executable = executable,
            timeToLiveMs = ttl.orElse(null),
            nonce = nonceRaw.map { Math.toIntExact(it) }.orElse(null),
            feePayment = feePayment,
            metadata = metadata,
        )
    }

    private class FeePaymentIntentAdapter : TypeAdapter<FeePaymentIntent> {
        override fun encode(encoder: NoritoEncoder, value: FeePaymentIntent) {
            when (value) {
                is FeePaymentIntent.Authority -> {
                    ENUM_TAG_ADAPTER.encode(encoder, FEE_PAYER_AUTHORITY_TAG)
                    encodeSizedField(encoder, AUTHORITY_FEE_PAYMENT_ADAPTER, value)
                }
                is FeePaymentIntent.Sponsor -> {
                    ENUM_TAG_ADAPTER.encode(encoder, FEE_PAYER_SPONSOR_TAG)
                    encodeSizedField(encoder, SPONSOR_FEE_PAYMENT_ADAPTER, value)
                }
            }
        }

        override fun decode(decoder: NoritoDecoder): FeePaymentIntent = when (val tag = ENUM_TAG_ADAPTER.decode(decoder)) {
            FEE_PAYER_AUTHORITY_TAG -> decodeSizedField(decoder, AUTHORITY_FEE_PAYMENT_ADAPTER)
            FEE_PAYER_SPONSOR_TAG -> decodeSizedField(decoder, SPONSOR_FEE_PAYMENT_ADAPTER)
            else -> throw IllegalArgumentException("Unknown FeePaymentIntent discriminant: $tag")
        }
    }

    private class AuthorityFeePaymentAdapter : TypeAdapter<FeePaymentIntent.Authority> {
        override fun encode(encoder: NoritoEncoder, value: FeePaymentIntent.Authority) {
            encodeSizedField(encoder, FEE_CHARGE_LIMIT_LIST_ADAPTER, value.chargeLimits)
            encodeSizedField(encoder, GAS_LIMIT_ADAPTER, Optional.ofNullable(value.gasLimit))
        }

        override fun decode(decoder: NoritoDecoder): FeePaymentIntent.Authority =
            FeePaymentIntent.Authority(
                decodeSizedField(decoder, FEE_CHARGE_LIMIT_LIST_ADAPTER),
                decodeSizedField<Optional<Long>>(decoder, GAS_LIMIT_ADAPTER).orElse(null),
            )
    }

    private class SponsorFeePaymentAdapter : TypeAdapter<FeePaymentIntent.Sponsor> {
        override fun encode(encoder: NoritoEncoder, value: FeePaymentIntent.Sponsor) {
            encodeSizedField(encoder, FEE_SPONSOR_PROGRAM_ID_ADAPTER, value.programId)
            encodeSizedField(encoder, UINT64_ADAPTER, value.programRevision)
            encodeSizedField(encoder, FEE_CHARGE_LIMIT_LIST_ADAPTER, value.chargeLimits)
            encodeSizedField(encoder, GAS_LIMIT_ADAPTER, Optional.ofNullable(value.gasLimit))
        }

        override fun decode(decoder: NoritoDecoder): FeePaymentIntent.Sponsor =
            FeePaymentIntent.Sponsor(
                decodeSizedField(decoder, FEE_SPONSOR_PROGRAM_ID_ADAPTER),
                decodeSizedField(decoder, UINT64_ADAPTER),
                decodeSizedField(decoder, FEE_CHARGE_LIMIT_LIST_ADAPTER),
                decodeSizedField<Optional<Long>>(decoder, GAS_LIMIT_ADAPTER).orElse(null),
            )
    }

    private class FeeSponsorProgramIdAdapter : TypeAdapter<FeeSponsorProgramId> {
        override fun encode(encoder: NoritoEncoder, value: FeeSponsorProgramId) {
            encodeSizedField(encoder, ACCOUNT_ID_ADAPTER, value.sponsor)
            encodeSizedField(encoder, STRING_ADAPTER, value.name)
        }

        override fun decode(decoder: NoritoDecoder): FeeSponsorProgramId =
            FeeSponsorProgramId(
                decodeSizedField(decoder, ACCOUNT_ID_ADAPTER),
                decodeSizedField(decoder, STRING_ADAPTER),
            )
    }

    private class FeeChargeLimitAdapter : TypeAdapter<FeeChargeLimit> {
        override fun encode(encoder: NoritoEncoder, value: FeeChargeLimit) {
            encodeSizedField(encoder, FEE_CHARGE_KIND_ADAPTER, value.kind)
            encodeSizedField(encoder, ASSET_DEFINITION_ID_ADAPTER, value.assetDefinitionId)
            encodeSizedField(encoder, QUANTITY_ADAPTER, KotodamaQuantity.parseCanonical(value.maxAmount))
        }

        override fun decode(decoder: NoritoDecoder): FeeChargeLimit =
            FeeChargeLimit(
                decodeSizedField(decoder, FEE_CHARGE_KIND_ADAPTER),
                decodeSizedField(decoder, ASSET_DEFINITION_ID_ADAPTER),
                decodeSizedField(decoder, QUANTITY_ADAPTER).toString(),
            )
    }

    private class FeeChargeKindAdapter : TypeAdapter<FeeChargeKind> {
        override fun encode(encoder: NoritoEncoder, value: FeeChargeKind) {
            ENUM_TAG_ADAPTER.encode(
                encoder,
                when (value) {
                    FeeChargeKind.NEXUS -> FEE_CHARGE_NEXUS_TAG
                    FeeChargeKind.PIPELINE_GAS -> FEE_CHARGE_PIPELINE_GAS_TAG
                },
            )
        }

        override fun decode(decoder: NoritoDecoder): FeeChargeKind = when (val tag = ENUM_TAG_ADAPTER.decode(decoder)) {
            FEE_CHARGE_NEXUS_TAG -> FeeChargeKind.NEXUS
            FEE_CHARGE_PIPELINE_GAS_TAG -> FeeChargeKind.PIPELINE_GAS
            else -> throw IllegalArgumentException("Unknown FeeChargeKind discriminant: $tag")
        }
    }

    private class AssetDefinitionIdAdapter : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            encodeFixedByteArray(encoder, AssetDefinitionIdEncoder.parseAddressBytes(value))
        }

        override fun decode(decoder: NoritoDecoder): String =
            AssetDefinitionIdEncoder.encodeFromBytes(decodeFixedByteArray(decoder, 16, "AssetDefinitionId"))
    }

    private class QuantityAdapter : TypeAdapter<KotodamaQuantity> {
        override fun encode(encoder: NoritoEncoder, value: KotodamaQuantity) {
            encodeSizedBigInt(encoder, value.mantissa)
            encodeSizedField(encoder, UINT32_ADAPTER, value.scale.toLong())
        }

        override fun decode(decoder: NoritoDecoder): KotodamaQuantity =
            KotodamaQuantity.of(
                decodeSizedBigInt(decoder),
                Math.toIntExact(decodeSizedField(decoder, UINT32_ADAPTER)),
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

    private class ContractArgumentRecordAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            require(value.size <= MAX_CONTRACT_ARGUMENT_RECORD_BYTES) {
                "Contract argument record exceeds $MAX_CONTRACT_ARGUMENT_RECORD_BYTES bytes"
            }
            RAW_BYTE_VEC_ADAPTER.encode(encoder, value)
        }

        override fun decode(decoder: NoritoDecoder): ByteArray {
            val length = decoder.readLength(false)
            require(length in 0L..MAX_CONTRACT_ARGUMENT_RECORD_BYTES.toLong()) {
                "Contract argument record exceeds $MAX_CONTRACT_ARGUMENT_RECORD_BYTES bytes"
            }
            return decoder.readBytes(length.toInt())
        }
    }

    private class ContractInvocationAdapter : TypeAdapter<ContractInvocation> {
        override fun encode(encoder: NoritoEncoder, value: ContractInvocation) {
            encodeSizedField(encoder, STRING_ADAPTER, value.contractAddress)
            encodeSizedField(encoder, HASH_ADAPTER, value.expectedCodeHash)
            encodeSizedField(encoder, STRING_ADAPTER, value.entrypoint)
            encodeSizedField(
                encoder,
                CONTRACT_ARGUMENTS_ADAPTER,
                Optional.ofNullable(value.arguments),
            )
        }

        override fun decode(decoder: NoritoDecoder): ContractInvocation =
            ContractInvocation(
                contractAddress = decodeSizedField(decoder, STRING_ADAPTER),
                expectedCodeHash = decodeSizedField(decoder, HASH_ADAPTER),
                entrypoint = decodeSizedField(decoder, STRING_ADAPTER),
                arguments = decodeBoundedSizedField<Optional<ByteArray>>(
                    decoder,
                    CONTRACT_ARGUMENTS_ADAPTER,
                    MAX_CONTRACT_ARGUMENT_RECORD_BYTES.toLong() + 32L,
                    "ContractInvocation.arguments",
                ).orElse(null),
            )
    }

    private class ExecutableBatchItemAdapter : TypeAdapter<ExecutableBatchItem> {
        override fun encode(encoder: NoritoEncoder, value: ExecutableBatchItem) {
            when (value) {
                is ExecutableBatchItem.Instruction -> {
                    ENUM_TAG_ADAPTER.encode(encoder, BATCH_ITEM_INSTRUCTION_TAG)
                    encodeSizedField(encoder, INSTRUCTION_ADAPTER, value.instruction)
                }
                is ExecutableBatchItem.ContractCall -> {
                    ENUM_TAG_ADAPTER.encode(encoder, BATCH_ITEM_CONTRACT_CALL_TAG)
                    encodeSizedField(encoder, CONTRACT_INVOCATION_ADAPTER, value.invocation)
                }
            }
        }

        override fun decode(decoder: NoritoDecoder): ExecutableBatchItem =
            when (val tag = ENUM_TAG_ADAPTER.decode(decoder)) {
                BATCH_ITEM_INSTRUCTION_TAG -> ExecutableBatchItem.instruction(
                    decodeSizedField(decoder, INSTRUCTION_ADAPTER),
                )
                BATCH_ITEM_CONTRACT_CALL_TAG -> ExecutableBatchItem.contractCall(
                    decodeSizedField(decoder, CONTRACT_INVOCATION_ADAPTER),
                )
                else -> throw IllegalArgumentException(
                    "Unknown ExecutableBatchItem discriminant: $tag",
                )
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
                        val publicKeyPayload = decodeSizedField(decoder, BYTE_VECTOR_ADAPTER)
                        ControllerPayload.single(publicKeyPayload)
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
                        val publicKeyPayload = compactPublicKeyPayload(singlePayload.curveId, singlePayload.publicKey)
                        return ControllerPayload.single(publicKeyPayload)
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
                    val publicKeyPayload = controller.publicKeyPayload()
                    val payload = decodeCompactPublicKeyPayload(publicKeyPayload)
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
            publicKeyPayload: ByteArray?,
            val multisigPolicy: MultisigPolicyPayload?,
        ) {
            private val publicKeyPayload: ByteArray? = publicKeyPayload?.copyOf()

            val isSingle: Boolean get() = multisigPolicy == null

            fun publicKeyPayload(): ByteArray = publicKeyPayload!!.copyOf()

            companion object {
                fun single(publicKeyPayload: ByteArray): ControllerPayload {
                    require(publicKeyPayload.isNotEmpty()) { "public key payload must not be empty" }
                    return ControllerPayload(publicKeyPayload, null)
                }

                fun multisig(multisigPolicy: MultisigPolicyPayload): ControllerPayload =
                    ControllerPayload(null, multisigPolicy)
            }
        }

        private class AccountControllerAdapter : TypeAdapter<ControllerPayload> {
            override fun encode(encoder: NoritoEncoder, value: ControllerPayload) {
                if (value.isSingle) {
                    ENUM_TAG_ADAPTER.encode(encoder, SINGLE_CONTROLLER_TAG)
                    encodeSizedField(encoder, BYTE_VECTOR_ADAPTER, value.publicKeyPayload())
                    return
                }
                ENUM_TAG_ADAPTER.encode(encoder, MULTISIG_CONTROLLER_TAG)
                encodeSizedField(encoder, MULTISIG_POLICY_ADAPTER, value.multisigPolicy!!)
            }

            override fun decode(decoder: NoritoDecoder): ControllerPayload {
                val controllerTag = ENUM_TAG_ADAPTER.decode(decoder)
                val controller = when (controllerTag) {
                    SINGLE_CONTROLLER_TAG -> {
                        val publicKeyPayload = decodeSizedField(decoder, BYTE_VECTOR_ADAPTER)
                        ControllerPayload.single(publicKeyPayload)
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
                val publicKeyPayload = compactPublicKeyPayload(value.curveId, value.publicKey)
                BYTE_VECTOR_ADAPTER.encode(encoder, publicKeyPayload)
                UINT16_ADAPTER.encode(encoder, value.weight.toLong())
            }

            override fun decode(decoder: NoritoDecoder): MultisigMemberPayload {
                val publicKeyPayload = BYTE_VECTOR_ADAPTER.decode(decoder)
                val weight = Math.toIntExact(UINT16_ADAPTER.decode(decoder))
                val payload = decodeCompactPublicKeyPayload(publicKeyPayload)
                    ?: throw IllegalArgumentException("Invalid multisig member public key")
                return MultisigMemberPayload(payload.curveId, weight, payload.keyBytes)
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

    private class JsonValueFieldAdapter : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            encodeSizedField(encoder, STRING_ADAPTER, value)
        }

        override fun decode(decoder: NoritoDecoder): String {
            return decodeSizedField(decoder, STRING_ADAPTER)
        }

        override fun isSelfDelimiting(): Boolean = true
    }

    private class MetadataAdapter : TypeAdapter<Map<String, JsonValue>> {
        private val entryListAdapter: TypeAdapter<List<MetadataEntry>> =
            NoritoAdapters.sequence(MetadataEntryAdapter())

        override fun encode(encoder: NoritoEncoder, value: Map<String, JsonValue>) {
            val keys = value.keys.sorted()
            val entries = keys.map { key ->
                val entryValue = value[key]
                    ?: throw IllegalArgumentException("Metadata values must not be null")
                MetadataEntry(key, entryValue)
            }
            entryListAdapter.encode(encoder, entries)
        }

        override fun decode(decoder: NoritoDecoder): Map<String, JsonValue> {
            val entries = entryListAdapter.decode(decoder)
            val decoded = LinkedHashMap<String, JsonValue>(entries.size)
            for (entry in entries) {
                require(decoded.put(entry.key, entry.value) == null) { "Duplicate metadata key" }
            }
            return decoded
        }
    }

    private class MetadataEntry(val key: String, val value: JsonValue)

    private class MetadataEntryAdapter : TypeAdapter<MetadataEntry> {
        override fun encode(encoder: NoritoEncoder, value: MetadataEntry) {
            encodeSizedField(encoder, STRING_ADAPTER, value.key)
            encodeSizedField(encoder, JSON_VALUE_ADAPTER, value.value.rawJson)
        }

        override fun decode(decoder: NoritoDecoder): MetadataEntry {
            val key = decodeSizedField(decoder, STRING_ADAPTER)
            val raw = decodeSizedField(decoder, JSON_VALUE_ADAPTER)
            return MetadataEntry(key, JsonValue.raw(raw))
        }
    }

    companion object {
        private val STRING_ADAPTER: TypeAdapter<String> = NoritoAdapters.stringAdapter()
        private val ACCOUNT_ID_ADAPTER: TypeAdapter<String> = AccountIdAdapter()
        private val CHAIN_ID_ADAPTER: TypeAdapter<String> = ChainIdAdapter()
        private val JSON_VALUE_ADAPTER: TypeAdapter<String> = JsonValueFieldAdapter()
        private val UINT64_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(64)
        private val UINT32_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(32)
        private val UINT16_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(16)
        private val UINT8_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(8)
        private val BYTE_VECTOR_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.byteVecAdapter()
        private val RAW_BYTE_VEC_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.rawByteVecAdapter()
        private val HASH_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.fixedBytes(32)
        private val IVM_BYTECODE_ADAPTER: TypeAdapter<ByteArray> = IvmBytecodeAdapter()
        private val INSTRUCTION_ADAPTER: TypeAdapter<InstructionBox> = InstructionAdapter()
        private val INSTRUCTION_LIST_ADAPTER: TypeAdapter<List<InstructionBox>> =
            NoritoAdapters.sequence(INSTRUCTION_ADAPTER)
        private val CONTRACT_ARGUMENT_RECORD_ADAPTER: TypeAdapter<ByteArray> =
            ContractArgumentRecordAdapter()
        private val CONTRACT_ARGUMENTS_ADAPTER: TypeAdapter<Optional<ByteArray>> =
            NoritoAdapters.option(CONTRACT_ARGUMENT_RECORD_ADAPTER)
        private val CONTRACT_INVOCATION_ADAPTER: TypeAdapter<ContractInvocation> =
            ContractInvocationAdapter()
        private val BATCH_ITEM_ADAPTER: TypeAdapter<ExecutableBatchItem> =
            ExecutableBatchItemAdapter()
        private val BATCH_ADAPTER: TypeAdapter<List<ExecutableBatchItem>> =
            NoritoAdapters.sequence(BATCH_ITEM_ADAPTER)
        private val ENUM_TAG_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(32)
        private const val EXECUTABLE_INSTRUCTIONS_TAG = 0L
        private const val EXECUTABLE_CONTRACT_CALL_TAG = 1L
        private const val EXECUTABLE_IVM_TAG = 2L
        private const val EXECUTABLE_IVM_PROVED_TAG = 3L
        private const val EXECUTABLE_BATCH_TAG = 4L
        private const val BATCH_ITEM_INSTRUCTION_TAG = 0L
        private const val BATCH_ITEM_CONTRACT_CALL_TAG = 1L
        private val TTL_ADAPTER: TypeAdapter<Optional<Long>> = NoritoAdapters.option(NoritoAdapters.uint(64))
        private val NONCE_ADAPTER: TypeAdapter<Optional<Long>> = NoritoAdapters.option(NoritoAdapters.uint(32))
        private val GAS_LIMIT_ADAPTER: TypeAdapter<Optional<Long>> = NoritoAdapters.option(NoritoAdapters.uint(64))
        private val FEE_CHARGE_KIND_ADAPTER: TypeAdapter<FeeChargeKind> = FeeChargeKindAdapter()
        private val ASSET_DEFINITION_ID_ADAPTER: TypeAdapter<String> = AssetDefinitionIdAdapter()
        private val QUANTITY_ADAPTER: TypeAdapter<KotodamaQuantity> = QuantityAdapter()
        private val FEE_CHARGE_LIMIT_LIST_ADAPTER: TypeAdapter<List<FeeChargeLimit>> =
            NoritoAdapters.sequence(FeeChargeLimitAdapter())
        private val FEE_SPONSOR_PROGRAM_ID_ADAPTER: TypeAdapter<FeeSponsorProgramId> =
            FeeSponsorProgramIdAdapter()
        private val AUTHORITY_FEE_PAYMENT_ADAPTER: TypeAdapter<FeePaymentIntent.Authority> =
            AuthorityFeePaymentAdapter()
        private val SPONSOR_FEE_PAYMENT_ADAPTER: TypeAdapter<FeePaymentIntent.Sponsor> =
            SponsorFeePaymentAdapter()
        private val FEE_PAYMENT_ADAPTER: TypeAdapter<FeePaymentIntent> = FeePaymentIntentAdapter()
        private val EXECUTABLE_ADAPTER: TypeAdapter<Executable> = ExecutableAdapter()
        private val METADATA_ADAPTER: TypeAdapter<Map<String, JsonValue>> = MetadataAdapter()
        private const val INSTRUCTION_BOX_SCHEMA = "iroha.data_model.isi.InstructionBox.v1"
        private const val FEE_PAYER_AUTHORITY_TAG = 0L
        private const val FEE_PAYER_SPONSOR_TAG = 1L
        private const val FEE_CHARGE_NEXUS_TAG = 0L
        private const val FEE_CHARGE_PIPELINE_GAS_TAG = 1L

        internal fun encodeInstructionBox(value: InstructionBox): ByteArray =
            NoritoCodec.encode(value, INSTRUCTION_BOX_SCHEMA, InstructionAdapter())

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

        private fun <T> decodeBoundedSizedField(
            decoder: NoritoDecoder,
            adapter: TypeAdapter<T>,
            maxEncodedLength: Long,
            fieldName: String,
        ): T {
            val length = decoder.readLength(decoder.compactLenActive())
            require(length in 0L..maxEncodedLength) {
                "$fieldName payload exceeds $maxEncodedLength bytes"
            }
            val payload = decoder.readBytes(length.toInt())
            val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
            val value = adapter.decode(child)
            require(child.remaining() == 0) { "Trailing bytes after $fieldName payload" }
            return value
        }

        private fun encodeFixedByteArray(encoder: NoritoEncoder, bytes: ByteArray) {
            val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
            for (byte in bytes) {
                encoder.writeLength(1, compact)
                encoder.writeByte(byte.toInt())
            }
        }

        private fun decodeFixedByteArray(decoder: NoritoDecoder, length: Int, fieldName: String): ByteArray {
            val compact = (decoder.flags and NoritoHeader.COMPACT_LEN) != 0
            return ByteArray(length) { index ->
                require(decoder.readLength(compact) == 1L) {
                    "$fieldName element $index must contain exactly one byte"
                }
                decoder.readByte().toByte()
            }
        }

        private fun encodeSizedBigInt(encoder: NoritoEncoder, value: BigInteger) {
            val child = encoder.childEncoder()
            val bytes = toTwosComplementLittleEndian(value)
            child.writeUInt(bytes.size.toLong(), 32)
            child.writeBytes(bytes)
            val payload = child.toByteArray()
            encoder.writeLength(payload.size.toLong(), (encoder.flags and NoritoHeader.COMPACT_LEN) != 0)
            encoder.writeBytes(payload)
        }

        private fun decodeSizedBigInt(decoder: NoritoDecoder): BigInteger {
            val length = decoder.readLength(decoder.compactLenActive())
            require(length <= Int.MAX_VALUE) { "numeric mantissa payload too large" }
            val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags, decoder.flagsHint)
            val byteLength = child.readUInt(32)
            require(byteLength <= 64L) { "numeric mantissa exceeds 512 bits" }
            val bytes = child.readBytes(byteLength.toInt())
            require(child.remaining() == 0) { "Trailing bytes after numeric mantissa payload" }
            val value = fromTwosComplementLittleEndian(bytes)
            require(toTwosComplementLittleEndian(value).contentEquals(bytes)) {
                "Numeric mantissa is not canonical"
            }
            return value
        }

        private fun fromTwosComplementLittleEndian(bytes: ByteArray): BigInteger {
            if (bytes.isEmpty()) return BigInteger.ZERO
            return BigInteger(bytes.reversedArray())
        }

        private fun toTwosComplementLittleEndian(value: BigInteger): ByteArray {
            if (value.signum() == 0) return ByteArray(0)
            val result = value.toByteArray().reversedArray()
            var length = result.size
            if (value.signum() > 0) {
                while (length > 1 && result[length - 1] == 0.toByte() &&
                    (result[length - 2].toInt() and 0x80) == 0
                ) length--
            } else {
                while (length > 1 && result[length - 1] == 0xff.toByte() &&
                    (result[length - 2].toInt() and 0x80) != 0
                ) length--
            }
            return if (length == result.size) result else result.copyOf(length)
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
                    ENUM_TAG_ADAPTER.encode(encoder, EXECUTABLE_IVM_TAG)
                    encodeSizedField(encoder, IVM_BYTECODE_ADAPTER, executable.ivmBytes)
                }
                is Executable.Instructions -> {
                    ENUM_TAG_ADAPTER.encode(encoder, EXECUTABLE_INSTRUCTIONS_TAG)
                    encodeSizedField(encoder, INSTRUCTION_LIST_ADAPTER, executable.instructions)
                }
                is Executable.ContractCall -> {
                    ENUM_TAG_ADAPTER.encode(encoder, EXECUTABLE_CONTRACT_CALL_TAG)
                    encodeSizedField(encoder, CONTRACT_INVOCATION_ADAPTER, executable.invocation)
                }
                is Executable.Batch -> {
                    ENUM_TAG_ADAPTER.encode(encoder, EXECUTABLE_BATCH_TAG)
                    encodeSizedField(encoder, BATCH_ADAPTER, executable.entries)
                }
            }
        }

        private fun decodeExecutable(decoder: NoritoDecoder): Executable {
            val tag = ENUM_TAG_ADAPTER.decode(decoder)
            return when (tag) {
                EXECUTABLE_IVM_TAG -> {
                    val bytes = decodeSizedField(decoder, IVM_BYTECODE_ADAPTER)
                    Executable.ivm(bytes)
                }
                EXECUTABLE_INSTRUCTIONS_TAG -> {
                    val instructions = decodeSizedField(decoder, INSTRUCTION_LIST_ADAPTER)
                    Executable.instructions(instructions)
                }
                EXECUTABLE_CONTRACT_CALL_TAG -> Executable.contractCall(
                    decodeSizedField(decoder, CONTRACT_INVOCATION_ADAPTER),
                )
                EXECUTABLE_BATCH_TAG -> Executable.batch(
                    decodeSizedField(decoder, BATCH_ADAPTER),
                )
                EXECUTABLE_IVM_PROVED_TAG ->
                    throw IllegalArgumentException("Unsupported Executable discriminant: $tag")
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
    }
}
