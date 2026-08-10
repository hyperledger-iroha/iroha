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
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.core.model.instructions.LanePrivacyMerkleWitness
import org.hyperledger.iroha.sdk.core.model.instructions.LanePrivacyProof
import org.hyperledger.iroha.sdk.core.model.instructions.LanePrivacyWitness
import org.hyperledger.iroha.sdk.core.model.instructions.ProofAttachment
import org.hyperledger.iroha.sdk.core.model.instructions.ProofVerifierKeyRef
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

internal class TransactionPayloadAdapter private constructor(
    private val chainDiscriminant: Int,
) : TypeAdapter<TransactionPayload> {

    override fun encode(encoder: NoritoEncoder, value: TransactionPayload) {
        withChainContext(chainDiscriminant) {
            require(!value.executable.requiresTransactionGasLimit() || value.feePayment.gasLimit != null) {
                "feePayment.gasLimit is required for IVM and contract-call executables"
            }
            encodeSizedField(encoder, TRANSACTION_DOMAIN_ADAPTER, value.networkId)
            encodeSizedField(encoder, ACCOUNT_ID_ADAPTER, value.authority)
            encodeSizedField(encoder, UINT64_ADAPTER, value.creationTimeMs)
            encodeSizedField(encoder, EXECUTABLE_ADAPTER, value.executable)
            encodeSizedField(encoder, TTL_ADAPTER, Optional.ofNullable(value.timeToLiveMs))
            encodeSizedField(encoder, NONCE_ADAPTER, Optional.ofNullable(value.nonce))
            encodeSizedField(encoder, FEE_PAYMENT_ADAPTER, value.feePayment)
            encodeSizedField(encoder, METADATA_ADAPTER, value.metadata)
            encodeSizedField(
                encoder,
                ATTACHMENTS_OPTION_ADAPTER,
                Optional.ofNullable(value.attachments),
            )
        }
    }

    override fun decode(decoder: NoritoDecoder): TransactionPayload =
        withChainContext(chainDiscriminant) {
            val networkId = decodeSizedField(decoder, TRANSACTION_DOMAIN_ADAPTER)
            val authority = decodeAuthorityField(decoder)
            val creationTimeMs = decodeSizedField(decoder, UINT64_ADAPTER)
            val executable = decodeSizedField(decoder, EXECUTABLE_ADAPTER)
            val ttl: Optional<Long> = decodeSizedField(decoder, TTL_ADAPTER)
            require(ttl.isPresent) {
                "TransactionPayload.time_to_live_ms must be signature-bound"
            }
            val nonceRaw: Optional<Long> = decodeSizedField(decoder, NONCE_ADAPTER)
            val feePayment = decodeSizedField(decoder, FEE_PAYMENT_ADAPTER)
            val metadata = LinkedHashMap(decodeSizedField(decoder, METADATA_ADAPTER))
            val attachments: Optional<List<ProofAttachment>> =
                decodeSizedField(decoder, ATTACHMENTS_OPTION_ADAPTER)

            TransactionPayload(
                networkId = networkId,
                authority = authority,
                creationTimeMs = creationTimeMs,
                executable = executable,
                timeToLiveMs = ttl.get(),
                nonce = nonceRaw.orElse(null),
                feePayment = feePayment,
                metadata = metadata,
                attachments = attachments.orElse(null),
            )
        }

    private class ProofBoxValue(
        val backend: String,
        bytes: ByteArray,
    ) {
        private val encodedBytes = bytes.copyOf()

        fun bytes(): ByteArray = encodedBytes.copyOf()
    }

    private class ProofBoxAdapter : TypeAdapter<ProofBoxValue> {
        override fun encode(encoder: NoritoEncoder, value: ProofBoxValue) {
            encodeSizedField(encoder, STRING_ADAPTER, value.backend)
            encodeSizedField(encoder, RAW_BYTE_VEC_ADAPTER, value.bytes())
        }

        override fun decode(decoder: NoritoDecoder): ProofBoxValue =
            ProofBoxValue(
                decodeBoundedSizedField(
                    decoder,
                    STRING_ADAPTER,
                    PORTABLE_IDENTIFIER_MAX_ENCODED_BYTES,
                    "ProofBox backend",
                ),
                decodeSizedField(decoder, RAW_BYTE_VEC_ADAPTER),
            )
    }

    private class ProofVerifierKeyRefAdapter : TypeAdapter<ProofVerifierKeyRef> {
        override fun encode(encoder: NoritoEncoder, value: ProofVerifierKeyRef) {
            encodeSizedField(encoder, STRING_ADAPTER, value.backend)
            encodeSizedField(encoder, STRING_ADAPTER, value.name)
        }

        override fun decode(decoder: NoritoDecoder): ProofVerifierKeyRef =
            ProofVerifierKeyRef(
                decodeBoundedSizedField(
                    decoder,
                    STRING_ADAPTER,
                    PORTABLE_IDENTIFIER_MAX_ENCODED_BYTES,
                    "verifier-key backend",
                ),
                decodeBoundedSizedField(
                    decoder,
                    STRING_ADAPTER,
                    PORTABLE_IDENTIFIER_MAX_ENCODED_BYTES,
                    "verifier-key name",
                ),
            )
    }

    private object LanePrivacyAuditPathAdapter : TypeAdapter<List<ByteArray>> {
        private val delegate = NoritoAdapters.sequence(OPTIONAL_LANE_PRIVACY_HASH_ADAPTER)

        override fun encode(encoder: NoritoEncoder, value: List<ByteArray>) {
            require(value.size in 1..LanePrivacyMerkleWitness.MAX_DEPTH) {
                "lane privacy audit path depth must be between 1 and ${LanePrivacyMerkleWitness.MAX_DEPTH}"
            }
            delegate.encode(encoder, value.map { Optional.of(it.copyOf()) })
        }

        override fun decode(decoder: NoritoDecoder): List<ByteArray> {
            val countValue = decoder.readLength(false)
            require(countValue in 1L..LanePrivacyMerkleWitness.MAX_DEPTH.toLong()) {
                "lane privacy audit path depth must be between 1 and ${LanePrivacyMerkleWitness.MAX_DEPTH}"
            }
            val count = countValue.toInt()
            return if ((decoder.flags and NoritoHeader.PACKED_SEQ) != 0) {
                decodePacked(decoder, count)
            } else {
                decodeDelimited(decoder, count)
            }
        }

        override fun isSelfDelimiting(): Boolean = true

        private fun decodeDelimited(decoder: NoritoDecoder, count: Int): List<ByteArray> {
            val siblings = ArrayList<ByteArray>(count)
            repeat(count) { index ->
                val length = decoder.readLength(decoder.compactLenActive())
                require(length in 1L..LANE_PRIVACY_MAX_SIBLING_OPTION_ENCODED_BYTES) {
                    "lane privacy sibling $index payload is oversized"
                }
                siblings.add(decodeSibling(decoder.readBytes(length.toInt()), decoder, index))
            }
            return siblings
        }

        private fun decodePacked(decoder: NoritoDecoder, count: Int): List<ByteArray> {
            var previous = decoder.readUInt(64)
            require(previous == 0L) { "packed lane privacy offsets must start at zero" }
            val sizes = ArrayList<Int>(count)
            repeat(count) { index ->
                val current = decoder.readUInt(64)
                require(current >= previous) { "packed lane privacy offsets must be monotonic" }
                val size = current - previous
                require(size in 1L..LANE_PRIVACY_MAX_SIBLING_OPTION_ENCODED_BYTES) {
                    "lane privacy sibling $index payload is oversized"
                }
                sizes.add(size.toInt())
                previous = current
            }
            require(previous == decoder.remaining().toLong()) {
                "packed lane privacy offsets must cover the complete path payload"
            }
            return sizes.mapIndexedTo(ArrayList(count)) { index, size ->
                decodeSibling(decoder.readBytes(size), decoder, index)
            }
        }

        private fun decodeSibling(
            payload: ByteArray,
            parent: NoritoDecoder,
            index: Int,
        ): ByteArray {
            val child = NoritoDecoder(payload, parent.flags, parent.flagsHint)
            val sibling = OPTIONAL_LANE_PRIVACY_HASH_ADAPTER.decode(child)
            require(child.remaining() == 0) {
                "lane privacy sibling $index has trailing bytes"
            }
            require(sibling.isPresent) {
                "lane privacy sibling $index must be present"
            }
            val bytes = sibling.get()
            require(bytes.last().toInt() and 1 == 1) {
                "lane privacy sibling $index is missing the Iroha prehashed marker"
            }
            return bytes
        }
    }

    private class LanePrivacyMerkleProofValue(
        val leafIndex: Long,
        val auditPath: List<ByteArray>,
    )

    private class LanePrivacyMerkleProofAdapter : TypeAdapter<LanePrivacyMerkleProofValue> {
        override fun encode(encoder: NoritoEncoder, value: LanePrivacyMerkleProofValue) {
            encodeSizedField(encoder, UINT32_ADAPTER, value.leafIndex)
            encodeSizedField(encoder, LANE_PRIVACY_AUDIT_PATH_ADAPTER, value.auditPath)
        }

        override fun decode(decoder: NoritoDecoder): LanePrivacyMerkleProofValue {
            val leafIndex = decodeSizedField(decoder, UINT32_ADAPTER)
            val auditPath = decodeBoundedSizedField(
                decoder,
                LANE_PRIVACY_AUDIT_PATH_ADAPTER,
                LANE_PRIVACY_MAX_AUDIT_PATH_ENCODED_BYTES,
                "lane privacy audit path",
            )
            return LanePrivacyMerkleProofValue(leafIndex, auditPath)
        }
    }

    private class LanePrivacyMerkleWitnessAdapter : TypeAdapter<LanePrivacyMerkleWitness> {
        override fun encode(encoder: NoritoEncoder, value: LanePrivacyMerkleWitness) {
            encodeSizedField(encoder, HASH_ARRAY_ADAPTER, value.leafBytes())
            encodeSizedField(
                encoder,
                LANE_PRIVACY_MERKLE_PROOF_ADAPTER,
                LanePrivacyMerkleProofValue(value.leafIndex, value.auditPathBytes()),
            )
        }

        override fun decode(decoder: NoritoDecoder): LanePrivacyMerkleWitness {
            val leaf = decodeSizedField(decoder, HASH_ARRAY_ADAPTER)
            val proof = decodeSizedField(decoder, LANE_PRIVACY_MERKLE_PROOF_ADAPTER)
            return LanePrivacyMerkleWitness(leaf, proof.leafIndex, proof.auditPath)
        }
    }

    private class LanePrivacyWitnessAdapter : TypeAdapter<LanePrivacyWitness> {
        override fun encode(encoder: NoritoEncoder, value: LanePrivacyWitness) {
            when (value) {
                is LanePrivacyWitness.Merkle -> {
                    ENUM_TAG_ADAPTER.encode(encoder, LANE_PRIVACY_MERKLE_TAG)
                    encodeSizedField(encoder, LANE_PRIVACY_MERKLE_WITNESS_ADAPTER, value.value)
                }
            }
        }

        override fun decode(decoder: NoritoDecoder): LanePrivacyWitness =
            when (val tag = ENUM_TAG_ADAPTER.decode(decoder)) {
                LANE_PRIVACY_MERKLE_TAG -> LanePrivacyWitness.Merkle(
                    decodeSizedField(decoder, LANE_PRIVACY_MERKLE_WITNESS_ADAPTER),
                )
                else -> throw IllegalArgumentException("unknown lane privacy witness tag: $tag")
            }
    }

    private class LanePrivacyProofAdapter : TypeAdapter<LanePrivacyProof> {
        override fun encode(encoder: NoritoEncoder, value: LanePrivacyProof) {
            encodeSizedField(encoder, UINT16_ADAPTER, value.commitmentId.toLong())
            encodeSizedField(encoder, LANE_PRIVACY_WITNESS_ADAPTER, value.witness)
        }

        override fun decode(decoder: NoritoDecoder): LanePrivacyProof =
            LanePrivacyProof(
                decodeSizedField(decoder, UINT16_ADAPTER).toInt(),
                decodeSizedField(decoder, LANE_PRIVACY_WITNESS_ADAPTER),
            )
    }

    private class ProofAttachmentAdapter : TypeAdapter<ProofAttachment> {
        override fun encode(encoder: NoritoEncoder, value: ProofAttachment) {
            encodeSizedField(encoder, STRING_ADAPTER, value.backend)
            encodeSizedField(
                encoder,
                PROOF_BOX_ADAPTER,
                ProofBoxValue(value.backend, value.proofBytes()),
            )
            encodeSizedField(encoder, PROOF_VERIFIER_KEY_REF_ADAPTER, value.verifyingKeyRef)

            val commitment = value.verifyingKeyCommitmentBytes()
            val envelopeHash = value.envelopeHashBytes()
            val lanePrivacy = value.lanePrivacy
            if (commitment != null || envelopeHash != null || lanePrivacy != null) {
                encodeSizedField(
                    encoder,
                    OPTIONAL_HASH_ADAPTER,
                    Optional.ofNullable(commitment),
                )
            }
            if (envelopeHash != null || lanePrivacy != null) {
                encodeSizedField(
                    encoder,
                    OPTIONAL_HASH_ADAPTER,
                    Optional.ofNullable(envelopeHash),
                )
            }
            if (lanePrivacy != null) {
                encodeSizedField(
                    encoder,
                    OPTIONAL_LANE_PRIVACY_ADAPTER,
                    Optional.of(lanePrivacy),
                )
            }
        }

        override fun decode(decoder: NoritoDecoder): ProofAttachment {
            val backend = decodeBoundedSizedField(
                decoder,
                STRING_ADAPTER,
                PORTABLE_IDENTIFIER_MAX_ENCODED_BYTES,
                "ProofAttachment backend",
            )
            val proof = decodeBoundedSizedField(
                decoder,
                PROOF_BOX_ADAPTER,
                PROOF_BOX_MAX_ENCODED_BYTES,
                "ProofBox",
            )
            require(proof.backend == backend) {
                "proof.backend must match attachment backend"
            }
            val verifyingKeyRef = decodeBoundedSizedField(
                decoder,
                PROOF_VERIFIER_KEY_REF_ADAPTER,
                VERIFYING_KEY_REF_MAX_ENCODED_BYTES,
                "verifier-key reference",
            )
            require(verifyingKeyRef.backend == backend) {
                "vk_ref.backend must match attachment backend"
            }

            val commitment = if (decoder.remaining() == 0) {
                null
            } else {
                decodeBoundedSizedField(
                    decoder,
                    OPTIONAL_HASH_ADAPTER,
                    OPTIONAL_FIXED_ARRAY_HASH_MAX_ENCODED_BYTES,
                    "verifier-key commitment",
                ).orElse(null)
            }
            val envelopeHash = if (decoder.remaining() == 0) {
                null
            } else {
                decodeBoundedSizedField(
                    decoder,
                    OPTIONAL_HASH_ADAPTER,
                    OPTIONAL_FIXED_ARRAY_HASH_MAX_ENCODED_BYTES,
                    "envelope hash",
                ).orElse(null)
            }
            val lanePrivacy = if (decoder.remaining() == 0) {
                null
            } else {
                decodeBoundedSizedField(
                    decoder,
                    OPTIONAL_LANE_PRIVACY_ADAPTER,
                    LANE_PRIVACY_MAX_OPTION_ENCODED_BYTES,
                    "lane privacy proof",
                ).orElse(null)
            }
            require(decoder.remaining() == 0) { "trailing ProofAttachment fields" }

            return ProofAttachment(
                backend,
                proof.bytes(),
                verifyingKeyRef,
                commitment,
                envelopeHash,
                lanePrivacy,
            )
        }
    }

    private class ProofAttachmentListAdapter : TypeAdapter<List<ProofAttachment>> {
        override fun encode(encoder: NoritoEncoder, value: List<ProofAttachment>) {
            encodeSizedField(encoder, PROOF_ATTACHMENT_SEQUENCE_ADAPTER, value)
        }

        override fun decode(decoder: NoritoDecoder): List<ProofAttachment> =
            decodeSizedField(decoder, PROOF_ATTACHMENT_SEQUENCE_ADAPTER)
    }

    private object FixedHashArrayAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            require(value.size == 32) { "expected 32-byte hash" }
            encodeFixedByteArray(encoder, value)
        }

        override fun decode(decoder: NoritoDecoder): ByteArray =
            decodeFixedByteArray(decoder, 32, "proof attachment hash")
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
            value.requirePrivacyExact12ConstructionAdmission()
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
                    AccountAddress.parseEncodedIgnoringCurveSupport(
                        canonicalAuthority,
                        requiredChainDiscriminant(),
                    )
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
                    address.toI105(requiredChainDiscriminant())
                } catch (e: AccountAddressException) {
                    throw IllegalArgumentException("Invalid single-key AccountController payload", e)
                }
            }

            private fun renderMultisigAuthority(policy: MultisigPolicyPayload): String {
                try {
                    val address = AccountAddress.fromMultisigPolicy(policy)
                    return address.toI105(requiredChainDiscriminant())
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

    private class TransactionDomainAdapter : TypeAdapter<NetworkId> {
        override fun encode(encoder: NoritoEncoder, value: NetworkId) {
            ENUM_TAG_ADAPTER.encode(encoder, TRANSACTION_DOMAIN_NETWORK_TAG)
            encodeSizedField(encoder, HASH_ADAPTER, value.bytes())
        }

        override fun decode(decoder: NoritoDecoder): NetworkId {
            val tag = ENUM_TAG_ADAPTER.decode(decoder)
            require(tag == TRANSACTION_DOMAIN_NETWORK_TAG) {
                if (tag == TRANSACTION_DOMAIN_GENESIS_TAG) {
                    "Genesis-only transaction domains are not accepted by the SDK"
                } else {
                    "Unknown TransactionDomain discriminant: $tag"
                }
            }
            return NetworkId.fromBytes(decodeSizedField(decoder, HASH_ADAPTER))
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
            encodeSizedField(encoder, JSON_VALUE_ADAPTER, value.value.canonicalJson)
        }

        override fun decode(decoder: NoritoDecoder): MetadataEntry {
            val key = decodeSizedField(decoder, STRING_ADAPTER)
            val raw = decodeSizedField(decoder, JSON_VALUE_ADAPTER)
            return MetadataEntry(key, JsonValue.fromCanonicalWire(raw))
        }
    }

    companion object {
        // Validation never exposes rendered account text. Controller bytes are independent of the
        // I105 display prefix, so this private synthetic context does not become a network default.
        private const val CANONICAL_VALIDATION_DISCRIMINANT = 0
        private const val PROOF_BOX_MAX_ENCODED_BYTES = 64L * 1024L * 1024L
        private const val PORTABLE_IDENTIFIER_MAX_ENCODED_BYTES = 8L + 256L
        private const val VERIFYING_KEY_REF_MAX_ENCODED_BYTES =
            2L * (8L + PORTABLE_IDENTIFIER_MAX_ENCODED_BYTES)
        private const val OPTIONAL_FIXED_ARRAY_HASH_MAX_ENCODED_BYTES = 1L + 8L + 32L * 9L
        private const val LANE_PRIVACY_MAX_SIBLING_OPTION_ENCODED_BYTES = 1L + 8L + 32L
        private const val LANE_PRIVACY_MAX_AUDIT_PATH_ENCODED_BYTES =
            8L + (LanePrivacyMerkleWitness.MAX_DEPTH + 1L) * 8L +
                LanePrivacyMerkleWitness.MAX_DEPTH * LANE_PRIVACY_MAX_SIBLING_OPTION_ENCODED_BYTES
        private const val LANE_PRIVACY_MAX_OPTION_ENCODED_BYTES = 16L * 1024L
        private const val LANE_PRIVACY_MERKLE_TAG = 0L
        private val CHAIN_DISCRIMINANT = ThreadLocal<Int?>()

        fun forChain(chainDiscriminant: Int): TransactionPayloadAdapter {
            require(chainDiscriminant in 0..0xffff) {
                "chainDiscriminant must fit in u16"
            }
            return TransactionPayloadAdapter(chainDiscriminant)
        }

        fun validateCanonicalPayloadBytes(encoded: ByteArray) {
            val validator = forChain(CANONICAL_VALIDATION_DISCRIMINANT)
            val decoded = NoritoCodec.decodeAdaptive(encoded, validator)
            val reencoded = NoritoCodec.encodeAdaptive(decoded, validator).payload()
            require(encoded.contentEquals(reencoded)) {
                "transaction payload bytes are not the exact canonical encoding"
            }
        }

        private fun requiredChainDiscriminant(): Int =
            checkNotNull(CHAIN_DISCRIMINANT.get()) {
                "Account controller encoding/rendering requires an explicit chainDiscriminant"
            }

        private fun <T> withChainContext(chainDiscriminant: Int, operation: () -> T): T {
            require(chainDiscriminant in 0..0xffff) {
                "chainDiscriminant must fit in u16"
            }
            val previous = CHAIN_DISCRIMINANT.get()
            check(previous == null || previous == chainDiscriminant) {
                "Conflicting nested chainDiscriminant context"
            }
            CHAIN_DISCRIMINANT.set(chainDiscriminant)
            return try {
                operation()
            } finally {
                if (previous == null) {
                    CHAIN_DISCRIMINANT.remove()
                } else {
                    CHAIN_DISCRIMINANT.set(previous)
                }
            }
        }

        private val STRING_ADAPTER: TypeAdapter<String> = NoritoAdapters.stringAdapter()
        private val ACCOUNT_ID_ADAPTER: TypeAdapter<String> = AccountIdAdapter()
        private val TRANSACTION_DOMAIN_ADAPTER: TypeAdapter<NetworkId> = TransactionDomainAdapter()
        private val JSON_VALUE_ADAPTER: TypeAdapter<String> = JsonValueFieldAdapter()
        private val UINT64_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(64)
        private val UINT32_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(32)
        private val UINT16_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(16)
        private val UINT8_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(8)
        private val BYTE_VECTOR_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.byteVecAdapter()
        private val RAW_BYTE_VEC_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.rawByteVecAdapter()
        private val HASH_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.fixedBytes(32)
        private val HASH_ARRAY_ADAPTER: TypeAdapter<ByteArray> = FixedHashArrayAdapter
        private val OPTIONAL_HASH_ADAPTER: TypeAdapter<Optional<ByteArray>> =
            NoritoAdapters.option(HASH_ARRAY_ADAPTER)
        private val OPTIONAL_LANE_PRIVACY_HASH_ADAPTER: TypeAdapter<Optional<ByteArray>> =
            NoritoAdapters.option(HASH_ADAPTER)
        private val LANE_PRIVACY_AUDIT_PATH_ADAPTER: TypeAdapter<List<ByteArray>> =
            LanePrivacyAuditPathAdapter
        private val LANE_PRIVACY_MERKLE_PROOF_ADAPTER: TypeAdapter<LanePrivacyMerkleProofValue> =
            LanePrivacyMerkleProofAdapter()
        private val LANE_PRIVACY_MERKLE_WITNESS_ADAPTER: TypeAdapter<LanePrivacyMerkleWitness> =
            LanePrivacyMerkleWitnessAdapter()
        private val LANE_PRIVACY_WITNESS_ADAPTER: TypeAdapter<LanePrivacyWitness> =
            LanePrivacyWitnessAdapter()
        private val LANE_PRIVACY_PROOF_ADAPTER: TypeAdapter<LanePrivacyProof> =
            LanePrivacyProofAdapter()
        private val OPTIONAL_LANE_PRIVACY_ADAPTER: TypeAdapter<Optional<LanePrivacyProof>> =
            NoritoAdapters.option(LANE_PRIVACY_PROOF_ADAPTER)
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
        private const val TRANSACTION_DOMAIN_NETWORK_TAG = 0L
        private const val TRANSACTION_DOMAIN_GENESIS_TAG = 1L
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
        private val PROOF_BOX_ADAPTER: TypeAdapter<ProofBoxValue> = ProofBoxAdapter()
        private val PROOF_VERIFIER_KEY_REF_ADAPTER: TypeAdapter<ProofVerifierKeyRef> =
            ProofVerifierKeyRefAdapter()
        private val PROOF_ATTACHMENT_ADAPTER: TypeAdapter<ProofAttachment> =
            ProofAttachmentAdapter()
        private val PROOF_ATTACHMENT_SEQUENCE_ADAPTER: TypeAdapter<List<ProofAttachment>> =
            NoritoAdapters.sequence(PROOF_ATTACHMENT_ADAPTER)
        private val PROOF_ATTACHMENT_LIST_ADAPTER: TypeAdapter<List<ProofAttachment>> =
            ProofAttachmentListAdapter()
        private val ATTACHMENTS_OPTION_ADAPTER: TypeAdapter<Optional<List<ProofAttachment>>> =
            NoritoAdapters.option(PROOF_ATTACHMENT_LIST_ADAPTER)
        private const val INSTRUCTION_BOX_SCHEMA =
            "(alloc::string::String, alloc::vec::Vec<u8>)"
        private const val FEE_PAYER_AUTHORITY_TAG = 0L
        private const val FEE_PAYER_SPONSOR_TAG = 1L
        private const val FEE_CHARGE_NEXUS_TAG = 0L
        private const val FEE_CHARGE_PIPELINE_GAS_TAG = 1L

        internal fun encodeInstructionBox(value: InstructionBox): ByteArray =
            NoritoCodec.encode(value, INSTRUCTION_BOX_SCHEMA, InstructionAdapter())

        internal fun encodeProofAttachmentPayload(value: ProofAttachment, flags: Int = 0): ByteArray {
            val encoder = NoritoEncoder(flags)
            PROOF_ATTACHMENT_ADAPTER.encode(encoder, value)
            return encoder.toByteArray()
        }

        internal fun decodeProofAttachmentPayload(encoded: ByteArray, flags: Int = 0): ProofAttachment {
            val decoder = NoritoDecoder(encoded, flags, NoritoHeader.MINOR_VERSION)
            val value = PROOF_ATTACHMENT_ADAPTER.decode(decoder)
            require(decoder.remaining() == 0) { "trailing ProofAttachment payload bytes" }
            return value
        }

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
