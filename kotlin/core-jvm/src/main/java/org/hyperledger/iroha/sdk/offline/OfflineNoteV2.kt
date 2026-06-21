package org.hyperledger.iroha.sdk.offline

import java.math.BigDecimal
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.MultisigMemberPayload
import org.hyperledger.iroha.sdk.address.MultisigPolicyPayload
import org.hyperledger.iroha.sdk.address.algorithmForCurveId
import org.hyperledger.iroha.sdk.address.compactPublicKeyPayload
import org.hyperledger.iroha.sdk.address.decodeCompactPublicKeyPayload
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Native JVM implementation of Iroha Offline Note V2 canonical Norito encodings. */
object OfflineNoteV2 {
    const val KEY_CERTIFICATE_PAYLOAD_DOMAIN: String =
        "iroha:offline-note:key-certificate-payload"
    const val ISSUED_CLAIM_DOMAIN: String = "iroha:offline-note:issued-claim"
    const val REDEEM_PUBLIC_INPUTS_DOMAIN: String =
        "iroha:offline-note:redeem-public-inputs"
    const val AUDIT_PUBLIC_INPUTS_DOMAIN: String =
        "iroha:offline-note:audit-public-inputs"
    const val RECURSIVE_BACKEND: String = "halo2/ipa"
    const val RECURSIVE_VERIFIER_NAME: String = "offline-note-v2-recursive-v1"
    const val RECURSIVE_PUBLIC_INPUTS_SCHEMA_V1: String =
        "{\"schema\":\"offline_note_recursive\",\"public_inputs\":[\"public_inputs_hash_limb0\",\"public_inputs_hash_limb1\",\"public_inputs_hash_limb2\",\"public_inputs_hash_limb3\",\"proof_mode\",\"input_count\",\"output_count\",\"input_amount_sum\",\"output_amount_sum\",\"input_nullifier_sum_limb0\",\"output_commitment_sum_limb0\",\"key_certificate_payload_hash_limb0\",\"source_or_token_limb0\",\"input_claim_hash_sum_limb0\",\"output_claim_hash_sum_limb0\",\"reserved_zero\"]}"
    const val KEY_CERTIFICATE_VERSION: Int = 1

    private const val MULTISIG_POLICY_VERSION_V1 = 1
    private const val MAX_NUMERIC_SCALE = 28
    private const val MAX_BIGINT_BYTES = 64
    private const val PUBLIC_VALUE_COUNT = 16
    private const val MAX_INPUT_AMOUNTS = 4
    private const val MAX_OUTPUT_AMOUNTS = 2
    private const val MODE_REDEEM = 1L
    private const val MODE_AUDIT = 2L
    private val MAX_U64: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

    private const val KEY_CERTIFICATE_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteKeyCertificate"
    private const val KEY_CERTIFICATE_PAYLOAD_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteKeyCertificatePayload"
    private const val ISSUE_SCHEMA = "iroha_data_model::offline::model::OfflineNoteIssue"
    private const val ISSUED_CLAIM_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteIssuedClaim"
    private const val AUDIT_OUTPUT_CLAIM_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteAuditOutputClaim"
    private const val RECURSIVE_PROOF_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteRecursiveProof"
    private const val REDEEM_SCHEMA = "iroha_data_model::offline::model::OfflineNoteRedeem"
    private const val REDEEM_PUBLIC_INPUTS_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteRedeemPublicInputs"
    private const val AUDIT_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteAuditBundle"
    private const val AUDIT_PUBLIC_INPUTS_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteAuditPublicInputs"
    const val ISSUE_INSTRUCTION_SCHEMA: String =
        "iroha_data_model::isi::offline::IssueOfflineNote"
    const val REDEEM_INSTRUCTION_SCHEMA: String =
        "iroha_data_model::isi::offline::RedeemOfflineNote"
    const val AUDIT_INSTRUCTION_SCHEMA: String =
        "iroha_data_model::isi::offline::AuditOfflineNote"
    private const val ISSUE_INSTRUCTION_ALIAS_SCHEMA =
        "iroha_data_model::isi::offline::IssueOfflineNoteV2"
    private const val REDEEM_INSTRUCTION_ALIAS_SCHEMA =
        "iroha_data_model::isi::offline::RedeemOfflineNoteV2"
    private const val AUDIT_INSTRUCTION_ALIAS_SCHEMA =
        "iroha_data_model::isi::offline::AuditOfflineNoteV2"

    @JvmStatic
    fun encodeCertificatePayload(value: KeyCertificatePayloadV2): ByteArray =
        encodeWithHeader(value, KEY_CERTIFICATE_PAYLOAD_SCHEMA, KeyCertificatePayloadAdapter)

    @JvmStatic
    fun encodeCertificate(value: KeyCertificateV2): ByteArray =
        encodeWithHeader(value, KEY_CERTIFICATE_SCHEMA, KeyCertificateAdapter)

    @JvmStatic
    fun encodeIssue(value: IssueV2): ByteArray =
        encodeWithHeader(value, ISSUE_SCHEMA, IssueAdapter)

    @JvmStatic
    fun encodeIssuedClaim(value: IssuedClaimV2): ByteArray =
        encodeWithHeader(value, ISSUED_CLAIM_SCHEMA, IssuedClaimAdapter)

    @JvmStatic
    fun encodeAuditOutputClaim(value: AuditOutputClaimV2): ByteArray =
        encodeWithHeader(value, AUDIT_OUTPUT_CLAIM_SCHEMA, AuditOutputClaimAdapter)

    @JvmStatic
    fun encodeRecursiveProof(value: RecursiveProofV2): ByteArray =
        encodeWithHeader(value, RECURSIVE_PROOF_SCHEMA, RecursiveProofAdapter)

    @JvmStatic
    fun encodeRedeem(value: RedeemV2): ByteArray =
        encodeWithHeader(value, REDEEM_SCHEMA, RedeemAdapter)

    @JvmStatic
    fun encodeRedeemPublicInputs(value: RedeemPublicInputsV2): ByteArray =
        encodeWithHeader(value, REDEEM_PUBLIC_INPUTS_SCHEMA, RedeemPublicInputsAdapter)

    @JvmStatic
    fun encodeAudit(value: AuditBundleV2): ByteArray =
        encodeWithHeader(value, AUDIT_SCHEMA, AuditAdapter)

    @JvmStatic
    fun encodeAuditPublicInputs(value: AuditPublicInputsV2): ByteArray =
        encodeWithHeader(value, AUDIT_PUBLIC_INPUTS_SCHEMA, AuditPublicInputsAdapter)

    @JvmStatic
    fun issueInstruction(value: IssueV2): InstructionBox =
        InstructionBox.fromWirePayload(
            ISSUE_INSTRUCTION_SCHEMA,
            encodeInstructionWrapper(ISSUE_INSTRUCTION_SCHEMA, value, IssueAdapter),
        )

    @JvmStatic
    fun redeemInstruction(value: RedeemV2): InstructionBox {
        value.validateProofBinding()
        return InstructionBox.fromWirePayload(
            REDEEM_INSTRUCTION_SCHEMA,
            encodeInstructionWrapper(REDEEM_INSTRUCTION_SCHEMA, value, RedeemAdapter),
        )
    }

    @JvmStatic
    fun auditInstruction(value: AuditBundleV2): InstructionBox {
        value.validateProofBinding()
        return InstructionBox.fromWirePayload(
            AUDIT_INSTRUCTION_SCHEMA,
            encodeInstructionWrapper(AUDIT_INSTRUCTION_SCHEMA, value, AuditAdapter),
        )
    }

    @JvmStatic
    fun decodeCertificatePayload(bytes: ByteArray): KeyCertificatePayloadV2 =
        decodeWithHeader(bytes, KEY_CERTIFICATE_PAYLOAD_SCHEMA, KeyCertificatePayloadAdapter)

    @JvmStatic
    fun decodeCertificate(bytes: ByteArray): KeyCertificateV2 =
        decodeWithHeader(bytes, KEY_CERTIFICATE_SCHEMA, KeyCertificateAdapter)

    @JvmStatic
    fun decodeIssue(bytes: ByteArray): IssueV2 =
        decodeWithHeader(bytes, ISSUE_SCHEMA, IssueAdapter)

    @JvmStatic
    fun decodeIssuedClaim(bytes: ByteArray): IssuedClaimV2 =
        decodeWithHeader(bytes, ISSUED_CLAIM_SCHEMA, IssuedClaimAdapter)

    @JvmStatic
    fun decodeAuditOutputClaim(bytes: ByteArray): AuditOutputClaimV2 =
        decodeWithHeader(bytes, AUDIT_OUTPUT_CLAIM_SCHEMA, AuditOutputClaimAdapter)

    @JvmStatic
    fun decodeRecursiveProof(bytes: ByteArray): RecursiveProofV2 =
        decodeWithHeader(bytes, RECURSIVE_PROOF_SCHEMA, RecursiveProofAdapter)

    @JvmStatic
    fun decodeRedeem(bytes: ByteArray): RedeemV2 =
        decodeWithHeader(bytes, REDEEM_SCHEMA, RedeemAdapter)

    @JvmStatic
    fun decodeRedeemPublicInputs(bytes: ByteArray): RedeemPublicInputsV2 =
        decodeWithHeader(bytes, REDEEM_PUBLIC_INPUTS_SCHEMA, RedeemPublicInputsAdapter)

    @JvmStatic
    fun decodeAudit(bytes: ByteArray): AuditBundleV2 =
        decodeWithHeader(bytes, AUDIT_SCHEMA, AuditAdapter)

    @JvmStatic
    fun decodeAuditPublicInputs(bytes: ByteArray): AuditPublicInputsV2 =
        decodeWithHeader(bytes, AUDIT_PUBLIC_INPUTS_SCHEMA, AuditPublicInputsAdapter)

    @JvmStatic
    fun decodeIssueInstruction(bytes: ByteArray): IssueV2 =
        decodeInstructionModel(
            bytes,
            ISSUE_INSTRUCTION_SCHEMA,
            ISSUE_INSTRUCTION_ALIAS_SCHEMA,
            ISSUE_SCHEMA,
            IssueAdapter,
        )

    @JvmStatic
    fun decodeRedeemInstruction(bytes: ByteArray): RedeemV2 =
        decodeInstructionModel(
            bytes,
            REDEEM_INSTRUCTION_SCHEMA,
            REDEEM_INSTRUCTION_ALIAS_SCHEMA,
            REDEEM_SCHEMA,
            RedeemAdapter,
        )

    @JvmStatic
    fun decodeAuditInstruction(bytes: ByteArray): AuditBundleV2 =
        decodeInstructionModel(
            bytes,
            AUDIT_INSTRUCTION_SCHEMA,
            AUDIT_INSTRUCTION_ALIAS_SCHEMA,
            AUDIT_SCHEMA,
            AuditAdapter,
        )

    @JvmStatic
    fun hash(bytes: ByteArray): ByteArray = IrohaHash.prehash(bytes)

    @JvmStatic
    fun instanceScalarBytes(value: Long): ByteArray {
        val out = ByteArray(32)
        var word = value
        for (idx in 0 until 8) {
            out[idx] = (word and 0xffL).toByte()
            word = word ushr 8
        }
        return out
    }

    private fun <T> encodeWithHeader(value: T, schema: String, adapter: TypeAdapter<T>): ByteArray =
        NoritoCodec.encode(value, schema, adapter, NoritoHeader.COMPACT_LEN)

    private fun <T> decodeWithHeader(bytes: ByteArray, schema: String, adapter: TypeAdapter<T>): T =
        NoritoCodec.decode(bytes, adapter, schema)

    private fun <T> encodeInstructionWrapper(
        schema: String,
        value: T,
        adapter: TypeAdapter<T>,
    ): ByteArray {
        val modelPayload = NoritoCodec.encodeAdaptive(value, adapter, NoritoHeader.COMPACT_LEN)
        return NoritoCodec.encode(
            InstructionModelPayload(modelPayload.payload(), modelPayload.flags),
            schema,
            InstructionWrapperPayloadAdapter,
            modelPayload.flags,
        )
    }

    private fun <T> decodeInstructionModel(
        bytes: ByteArray,
        instructionSchema: String,
        instructionAliasSchema: String,
        modelSchema: String,
        modelAdapter: TypeAdapter<T>,
    ): T {
        val instructionSchemas = listOf(instructionSchema, instructionAliasSchema)
        val wirePayload = extractInstructionWirePayload(bytes, instructionSchemas)
        var lastError: RuntimeException? = null
        for (candidateSchema in instructionSchemas) {
            try {
                val modelPayload = NoritoCodec.decode(
                    wirePayload,
                    InstructionWrapperPayloadAdapter,
                    candidateSchema,
                )
                return decodeModelPayload(modelPayload.bytes, modelSchema, modelAdapter, modelPayload.flags)
            } catch (ex: RuntimeException) {
                lastError = ex
            }
        }
        throw IllegalArgumentException("Offline Note V2 instruction envelope is invalid", lastError)
    }

    private fun extractInstructionWirePayload(bytes: ByteArray, expectedWireNames: List<String>): ByteArray {
        if (isNoritoFrame(bytes)) return bytes.copyOf()
        tryDecodeInstructionPair(bytes, expectedWireNames, NoritoHeader.COMPACT_LEN)?.let { return it }
        tryDecodeInstructionPair(bytes, expectedWireNames, 0)?.let { return it }
        throw IllegalArgumentException("Offline Note V2 instruction envelope is invalid")
    }

    private fun tryDecodeInstructionPair(
        bytes: ByteArray,
        expectedWireNames: List<String>,
        flags: Int,
    ): ByteArray? = try {
        val decoder = NoritoDecoder(bytes, flags)
        val wireName = readField(decoder) { readString(it) }
        require(wireName in expectedWireNames) {
            "Offline Note V2 instruction wire name mismatch: $wireName"
        }
        val wirePayload = readField(decoder) { readBytesVec(it) }
        require(decoder.remaining() == 0) { "Trailing bytes after Offline Note V2 instruction envelope" }
        wirePayload
    } catch (_: RuntimeException) {
        null
    }

    private fun <T> decodeModelPayload(
        bytes: ByteArray,
        modelSchema: String,
        modelAdapter: TypeAdapter<T>,
        flags: Int,
    ): T {
        if (isNoritoFrame(bytes)) {
            return decodeWithHeader(bytes, modelSchema, modelAdapter)
        }
        val attempts = if (flags == NoritoHeader.COMPACT_LEN) {
            intArrayOf(flags, 0)
        } else {
            intArrayOf(flags, NoritoHeader.COMPACT_LEN)
        }
        var lastError: RuntimeException? = null
        for (attemptFlags in attempts) {
            try {
                val decoder = NoritoDecoder(bytes, attemptFlags)
                val value = modelAdapter.decode(decoder)
                require(decoder.remaining() == 0) {
                    "Trailing bytes after Offline Note V2 instruction model decode"
                }
                return value
            } catch (ex: RuntimeException) {
                lastError = ex
            }
        }
        throw IllegalArgumentException("Offline Note V2 instruction model payload is invalid", lastError)
    }

    private fun isNoritoFrame(bytes: ByteArray): Boolean =
        bytes.size >= NoritoHeader.HEADER_LENGTH &&
            bytes[0] == 'N'.code.toByte() &&
            bytes[1] == 'R'.code.toByte() &&
            bytes[2] == 'T'.code.toByte() &&
            bytes[3] == '0'.code.toByte()

    class VerifyingKeyIdReference @JvmOverloads constructor(
        backend: String = RECURSIVE_BACKEND,
        name: String = RECURSIVE_VERIFIER_NAME,
    ) {
        val backend: String = requireNonBlankUnpadded(backend, "verifying key backend")
        val name: String = requireNonBlankUnpadded(name, "verifying key name")

        init {
            require(this.backend.indexOf(':') < 0) { "verifying key backend must not contain ':'" }
            require(this.name.indexOf(':') < 0) { "verifying key name must not contain ':'" }
        }
    }

    class ProofBox(backend: String, bytes: ByteArray) {
        val backend: String = requireNonBlankUnpadded(backend, "proof backend")
        private val _bytes = bytes.copyOf()

        init {
            require(_bytes.isNotEmpty()) { "proof bytes must not be empty" }
        }

        fun bytes(): ByteArray = _bytes.copyOf()
    }

    class RecursiveProofV2 @JvmOverloads constructor(
        val verifierKeyId: VerifyingKeyIdReference = VerifyingKeyIdReference(),
        publicInputsHash: ByteArray,
        val proof: ProofBox,
    ) {
        private val _publicInputsHash = publicInputsHash.copyOf()

        init {
            requireHash(_publicInputsHash, "public_inputs_hash")
        }

        fun publicInputsHash(): ByteArray = _publicInputsHash.copyOf()
    }

    class KeyCertificatePayloadV2 @JvmOverloads constructor(
        val domain: String = KEY_CERTIFICATE_PAYLOAD_DOMAIN,
        val version: Int,
        val platform: String,
        val keyId: String,
        val deviceId: String,
        val accountId: String,
        publicKey: ByteArray,
        val assertionScheme: String,
        val assertionKeyAlgorithm: String,
        assertionPublicKey: ByteArray,
        val assertionUsageCountLimit: Int?,
        val oneUse: Boolean,
    ) {
        private val _publicKey = publicKey.copyOf()
        private val _assertionPublicKey = assertionPublicKey.copyOf()

        init {
            require(domain == KEY_CERTIFICATE_PAYLOAD_DOMAIN) {
                "unsupported key certificate payload domain"
            }
            requireCertificateCore(version, accountId, _publicKey, oneUse)
            require(assertionUsageCountLimit == null || assertionUsageCountLimit >= 0) {
                "assertion usage count limit must be non-negative"
            }
        }

        fun publicKey(): ByteArray = _publicKey.copyOf()
        fun assertionPublicKey(): ByteArray = _assertionPublicKey.copyOf()
        fun noritoEncoded(): ByteArray = encodeCertificatePayload(this)
        fun payloadHash(): ByteArray = hash(noritoEncoded())
    }

    class KeyCertificateV2 @JvmOverloads constructor(
        val version: Int = KEY_CERTIFICATE_VERSION,
        val platform: String,
        val keyId: String,
        val deviceId: String,
        val accountId: String,
        publicKey: ByteArray,
        val assertionScheme: String,
        val assertionKeyAlgorithm: String,
        assertionPublicKey: ByteArray,
        val assertionUsageCountLimit: Int?,
        val oneUse: Boolean = true,
        issuerSignature: ByteArray,
    ) {
        private val _publicKey = publicKey.copyOf()
        private val _assertionPublicKey = assertionPublicKey.copyOf()
        private val _issuerSignature = issuerSignature.copyOf()

        init {
            requireCertificateCore(version, accountId, _publicKey, oneUse)
            require(_issuerSignature.size == 64) { "issuer signature must be 64 bytes" }
            require(assertionUsageCountLimit == null || assertionUsageCountLimit >= 0) {
                "assertion usage count limit must be non-negative"
            }
        }

        fun publicKey(): ByteArray = _publicKey.copyOf()
        fun assertionPublicKey(): ByteArray = _assertionPublicKey.copyOf()
        fun issuerSignature(): ByteArray = _issuerSignature.copyOf()

        fun signingPayload(): KeyCertificatePayloadV2 = KeyCertificatePayloadV2(
            version = version,
            platform = platform,
            keyId = keyId,
            deviceId = deviceId,
            accountId = accountId,
            publicKey = publicKey(),
            assertionScheme = assertionScheme,
            assertionKeyAlgorithm = assertionKeyAlgorithm,
            assertionPublicKey = assertionPublicKey(),
            assertionUsageCountLimit = assertionUsageCountLimit,
            oneUse = oneUse,
        )

        fun signingBytes(): ByteArray = signingPayload().noritoEncoded()
        fun payloadHash(): ByteArray = hash(signingBytes())
        fun noritoEncoded(): ByteArray = encodeCertificate(this)
    }

    class IssueV2(
        noteCommitment: ByteArray,
        val keyCertificate: KeyCertificateV2,
        val assetId: String,
        val amount: String,
    ) {
        private val _noteCommitment = noteCommitment.copyOf()
        val canonicalAmount: String = parseNumeric(amount).canonicalString

        init {
            requireHash(_noteCommitment, "note_commitment")
            parseAssetId(assetId)
        }

        fun noteCommitment(): ByteArray = _noteCommitment.copyOf()
        fun issuedClaim(): IssuedClaimV2 = IssuedClaimV2(
            noteCommitment = noteCommitment(),
            keyCertificatePayloadHash = keyCertificate.payloadHash(),
            assetId = assetId,
            amount = canonicalAmount,
        )
        fun noritoEncoded(): ByteArray = encodeIssue(this)
    }

    class IssuedClaimV2 @JvmOverloads constructor(
        val domain: String = ISSUED_CLAIM_DOMAIN,
        noteCommitment: ByteArray,
        keyCertificatePayloadHash: ByteArray,
        val assetId: String,
        val amount: String,
    ) {
        private val _noteCommitment = noteCommitment.copyOf()
        private val _keyCertificatePayloadHash = keyCertificatePayloadHash.copyOf()
        val canonicalAmount: String = parseNumeric(amount).canonicalString

        init {
            require(domain == ISSUED_CLAIM_DOMAIN) { "unsupported issued claim domain" }
            requireHash(_noteCommitment, "note_commitment")
            requireHash(_keyCertificatePayloadHash, "key_certificate_payload_hash")
            parseAssetId(assetId)
        }

        fun noteCommitment(): ByteArray = _noteCommitment.copyOf()
        fun keyCertificatePayloadHash(): ByteArray = _keyCertificatePayloadHash.copyOf()
        fun noritoEncoded(): ByteArray = encodeIssuedClaim(this)
        fun claimHash(): ByteArray = hash(noritoEncoded())
    }

    class AuditOutputClaimV2(
        noteCommitment: ByteArray,
        val keyCertificate: KeyCertificateV2,
        val assetId: String,
        val amount: String,
    ) {
        private val _noteCommitment = noteCommitment.copyOf()
        val canonicalAmount: String = parseNumeric(amount).canonicalString

        init {
            requireHash(_noteCommitment, "note_commitment")
            parseAssetId(assetId)
        }

        fun noteCommitment(): ByteArray = _noteCommitment.copyOf()
        fun issuedClaim(): IssuedClaimV2 = IssuedClaimV2(
            noteCommitment = noteCommitment(),
            keyCertificatePayloadHash = keyCertificate.payloadHash(),
            assetId = assetId,
            amount = canonicalAmount,
        )
    }

    class RedeemPublicInputsV2 @JvmOverloads constructor(
        val domain: String = REDEEM_PUBLIC_INPUTS_DOMAIN,
        sourceNoteCommitment: ByteArray,
        inputNullifiers: List<ByteArray>,
        keyCertificatePayloadHash: ByteArray,
        val recipient: String,
        val assetId: String,
        val amount: String,
    ) {
        private val _sourceNoteCommitment = sourceNoteCommitment.copyOf()
        private val _inputNullifiers = inputNullifiers.map { it.copyOf() }
        private val _keyCertificatePayloadHash = keyCertificatePayloadHash.copyOf()
        val canonicalAmount: String = parseNumeric(amount).canonicalString

        init {
            require(domain == REDEEM_PUBLIC_INPUTS_DOMAIN) { "unsupported redeem public inputs domain" }
            requireHash(_sourceNoteCommitment, "source_note_commitment")
            requireHashes(_inputNullifiers, "input_nullifiers")
            requireHash(_keyCertificatePayloadHash, "key_certificate_payload_hash")
            encodeAccountIdPayload(recipient)
            parseAssetId(assetId)
        }

        fun sourceNoteCommitment(): ByteArray = _sourceNoteCommitment.copyOf()
        fun inputNullifiers(): List<ByteArray> = _inputNullifiers.map { it.copyOf() }
        fun keyCertificatePayloadHash(): ByteArray = _keyCertificatePayloadHash.copyOf()
        fun noritoEncoded(): ByteArray = encodeRedeemPublicInputs(this)
        fun publicInputsHash(): ByteArray = hash(noritoEncoded())
    }

    class RedeemV2(
        sourceNoteCommitment: ByteArray,
        inputNullifiers: List<ByteArray>,
        val senderKeyCertificate: KeyCertificateV2,
        val recipient: String,
        val assetId: String,
        val amount: String,
        val recursiveProof: RecursiveProofV2,
    ) {
        private val _sourceNoteCommitment = sourceNoteCommitment.copyOf()
        private val _inputNullifiers = inputNullifiers.map { it.copyOf() }
        val canonicalAmount: String = parseNumeric(amount).canonicalString

        init {
            requireHash(_sourceNoteCommitment, "source_note_commitment")
            requireHashes(_inputNullifiers, "input_nullifiers")
            encodeAccountIdPayload(recipient)
            parseAssetId(assetId)
        }

        fun sourceNoteCommitment(): ByteArray = _sourceNoteCommitment.copyOf()
        fun inputNullifiers(): List<ByteArray> = _inputNullifiers.map { it.copyOf() }
        fun publicInputs(): RedeemPublicInputsV2 = RedeemPublicInputsV2(
            sourceNoteCommitment = sourceNoteCommitment(),
            inputNullifiers = inputNullifiers(),
            keyCertificatePayloadHash = senderKeyCertificate.payloadHash(),
            recipient = recipient,
            assetId = assetId,
            amount = canonicalAmount,
        )
        fun publicInputsHash(): ByteArray = publicInputs().publicInputsHash()
        fun validateProofBinding() {
            require(recursiveProof.publicInputsHash().contentEquals(publicInputsHash())) {
                "recursive proof public inputs hash mismatch"
            }
        }

        fun replacingRecursiveProof(recursiveProof: RecursiveProofV2): RedeemV2 = RedeemV2(
            sourceNoteCommitment = sourceNoteCommitment(),
            inputNullifiers = inputNullifiers(),
            senderKeyCertificate = senderKeyCertificate,
            recipient = recipient,
            assetId = assetId,
            amount = amount,
            recursiveProof = recursiveProof,
        )

        fun noritoEncoded(): ByteArray = encodeRedeem(this)
    }

    class AuditPublicInputsV2 @JvmOverloads constructor(
        val domain: String = AUDIT_PUBLIC_INPUTS_DOMAIN,
        tokenId: ByteArray,
        keyCertificatePayloadHash: ByteArray,
        inputNullifiers: List<ByteArray>,
        val inputClaims: List<IssuedClaimV2>,
        outputCommitments: List<ByteArray>,
        val outputClaims: List<IssuedClaimV2>,
    ) {
        private val _tokenId = tokenId.copyOf()
        private val _keyCertificatePayloadHash = keyCertificatePayloadHash.copyOf()
        private val _inputNullifiers = inputNullifiers.map { it.copyOf() }
        private val _outputCommitments = outputCommitments.map { it.copyOf() }

        init {
            require(domain == AUDIT_PUBLIC_INPUTS_DOMAIN) { "unsupported audit public inputs domain" }
            requireHash(_tokenId, "token_id")
            requireHash(_keyCertificatePayloadHash, "key_certificate_payload_hash")
            requireHashes(_inputNullifiers, "input_nullifiers")
            require(inputClaims.isNotEmpty()) { "input claims must not be empty" }
            require(inputClaims.size == _inputNullifiers.size) {
                "input nullifier count must match input claim count"
            }
            requireHashes(_outputCommitments, "output_commitments")
            require(outputClaims.isNotEmpty()) { "output claims must not be empty" }
            val committed = _outputCommitments.map { hexLower(it) }.toSet()
            for (claim in outputClaims) {
                require(hexLower(claim.noteCommitment()) in committed) {
                    "audit output claim is not listed in output commitments"
                }
            }
        }

        fun tokenId(): ByteArray = _tokenId.copyOf()
        fun keyCertificatePayloadHash(): ByteArray = _keyCertificatePayloadHash.copyOf()
        fun inputNullifiers(): List<ByteArray> = _inputNullifiers.map { it.copyOf() }
        fun outputCommitments(): List<ByteArray> = _outputCommitments.map { it.copyOf() }
        fun noritoEncoded(): ByteArray = encodeAuditPublicInputs(this)
        fun publicInputsHash(): ByteArray = hash(noritoEncoded())
    }

    class AuditBundleV2(
        tokenId: ByteArray,
        val senderKeyCertificate: KeyCertificateV2,
        inputNullifiers: List<ByteArray>,
        val inputClaims: List<IssuedClaimV2>,
        outputCommitments: List<ByteArray>,
        val outputClaims: List<AuditOutputClaimV2>,
        val recursiveProof: RecursiveProofV2,
    ) {
        private val _tokenId = tokenId.copyOf()
        private val _inputNullifiers = inputNullifiers.map { it.copyOf() }
        private val _outputCommitments = outputCommitments.map { it.copyOf() }

        init {
            requireHash(_tokenId, "token_id")
            requireHashes(_inputNullifiers, "input_nullifiers")
            require(inputClaims.isNotEmpty()) { "input claims must not be empty" }
            require(inputClaims.size == _inputNullifiers.size) {
                "input nullifier count must match input claim count"
            }
            requireHashes(_outputCommitments, "output_commitments")
            require(outputClaims.isNotEmpty()) { "output claims must not be empty" }
            val committed = _outputCommitments.map { hexLower(it) }.toSet()
            for (claim in outputClaims) {
                require(hexLower(claim.noteCommitment()) in committed) {
                    "audit output claim is not listed in output commitments"
                }
            }
        }

        fun tokenId(): ByteArray = _tokenId.copyOf()
        fun inputNullifiers(): List<ByteArray> = _inputNullifiers.map { it.copyOf() }
        fun outputCommitments(): List<ByteArray> = _outputCommitments.map { it.copyOf() }
        fun publicInputs(): AuditPublicInputsV2 = AuditPublicInputsV2(
            tokenId = tokenId(),
            keyCertificatePayloadHash = senderKeyCertificate.payloadHash(),
            inputNullifiers = inputNullifiers(),
            inputClaims = inputClaims,
            outputCommitments = outputCommitments(),
            outputClaims = outputClaims.map { it.issuedClaim() },
        )
        fun publicInputsHash(): ByteArray = publicInputs().publicInputsHash()
        fun validateProofBinding() {
            require(recursiveProof.publicInputsHash().contentEquals(publicInputsHash())) {
                "recursive proof public inputs hash mismatch"
            }
        }

        fun replacingRecursiveProof(recursiveProof: RecursiveProofV2): AuditBundleV2 = AuditBundleV2(
            tokenId = tokenId(),
            senderKeyCertificate = senderKeyCertificate,
            inputNullifiers = inputNullifiers(),
            inputClaims = inputClaims,
            outputCommitments = outputCommitments(),
            outputClaims = outputClaims,
            recursiveProof = recursiveProof,
        )

        fun noritoEncoded(): ByteArray = encodeAudit(this)
    }

    class InstanceValues(
        publicValues: LongArray,
        inputAmounts: LongArray,
        outputAmounts: LongArray,
    ) {
        private val _publicValues = publicValues.copyOf()
        private val _inputAmounts = inputAmounts.copyOf()
        private val _outputAmounts = outputAmounts.copyOf()

        init {
            require(_publicValues.size == PUBLIC_VALUE_COUNT) {
                "Offline V2 public instance count must be $PUBLIC_VALUE_COUNT"
            }
            require(_inputAmounts.size == MAX_INPUT_AMOUNTS) {
                "Offline V2 input amount witness count must be $MAX_INPUT_AMOUNTS"
            }
            require(_outputAmounts.size == MAX_OUTPUT_AMOUNTS) {
                "Offline V2 output amount witness count must be $MAX_OUTPUT_AMOUNTS"
            }
        }

        fun publicValues(): LongArray = _publicValues.copyOf()
        fun inputAmounts(): LongArray = _inputAmounts.copyOf()
        fun outputAmounts(): LongArray = _outputAmounts.copyOf()
        fun publicInstanceColumns(): List<ByteArray> = _publicValues.map { instanceScalarBytes(it) }
    }

    object InstanceBuilder {
        @JvmStatic
        fun redeemInstanceValues(redemption: RedeemV2): InstanceValues {
            val inputCount = validateCount(
                redemption.inputNullifiers().size,
                MAX_INPUT_AMOUNTS,
                "redemption input",
            )
            val normalizedAmounts = normalizedAmountUnits(
                listOf(redemption.canonicalAmount, redemption.canonicalAmount)
            )
            val inputSum = normalizedAmounts[0]
            val outputSum = normalizedAmounts[1]
            val issuedClaimHash = IssuedClaimV2(
                noteCommitment = redemption.sourceNoteCommitment(),
                keyCertificatePayloadHash = redemption.senderKeyCertificate.payloadHash(),
                assetId = redemption.assetId,
                amount = redemption.canonicalAmount,
            ).claimHash()
            val publicValues = publicValues(
                publicInputsHash = redemption.publicInputsHash(),
                mode = MODE_REDEEM,
                inputCount = inputCount,
                outputCount = 1L,
                inputSum = inputSum,
                outputSum = outputSum,
                inputNullifierSum = hashLimb0Sum(redemption.inputNullifiers()),
                outputCommitmentSum = 0L,
                keyCertificatePayloadHash = redemption.senderKeyCertificate.payloadHash(),
                sourceOrToken = redemption.sourceNoteCommitment(),
                inputClaimHashSum = hashLimb0(issuedClaimHash),
                outputClaimHashSum = 0L,
            )
            val inputAmounts = LongArray(MAX_INPUT_AMOUNTS)
            inputAmounts[0] = inputSum
            val outputAmounts = LongArray(MAX_OUTPUT_AMOUNTS)
            outputAmounts[0] = outputSum
            return InstanceValues(publicValues, inputAmounts, outputAmounts)
        }

        @JvmStatic
        fun auditInstanceValues(audit: AuditBundleV2): InstanceValues {
            val inputCount = validateCount(audit.inputClaims.size, MAX_INPUT_AMOUNTS, "audit input")
            val outputCount = validateCount(audit.outputClaims.size, MAX_OUTPUT_AMOUNTS, "audit output")
            require(audit.inputNullifiers().size == audit.inputClaims.size) {
                "audit input nullifier count must match input claim count"
            }

            val inputClaimHashes = audit.inputClaims.map { it.claimHash() }
            val outputClaimHashes = audit.outputClaims.map { it.issuedClaim().claimHash() }
            val normalizedAmounts = normalizedAmountUnits(
                audit.inputClaims.map { it.canonicalAmount } +
                    audit.outputClaims.map { it.canonicalAmount }
            )
            val inputUnits = normalizedAmounts.take(audit.inputClaims.size)
            val outputUnits = normalizedAmounts.drop(audit.inputClaims.size)
            val inputSum = checkedSum(inputUnits, "input")
            val outputSum = checkedSum(outputUnits, "output")
            require(inputSum == outputSum) {
                "Offline V2 audit amounts are not conserved"
            }

            val inputAmounts = LongArray(MAX_INPUT_AMOUNTS)
            inputUnits.forEachIndexed { index, amount -> inputAmounts[index] = amount }
            val outputAmounts = LongArray(MAX_OUTPUT_AMOUNTS)
            outputUnits.forEachIndexed { index, amount -> outputAmounts[index] = amount }

            return InstanceValues(
                publicValues(
                    publicInputsHash = audit.publicInputsHash(),
                    mode = MODE_AUDIT,
                    inputCount = inputCount,
                    outputCount = outputCount,
                    inputSum = inputSum,
                    outputSum = outputSum,
                    inputNullifierSum = hashLimb0Sum(audit.inputNullifiers()),
                    outputCommitmentSum = hashLimb0Sum(audit.outputCommitments()),
                    keyCertificatePayloadHash = audit.senderKeyCertificate.payloadHash(),
                    sourceOrToken = audit.tokenId(),
                    inputClaimHashSum = hashLimb0Sum(inputClaimHashes),
                    outputClaimHashSum = hashLimb0Sum(outputClaimHashes),
                ),
                inputAmounts,
                outputAmounts,
            )
        }
    }

    private class InstructionModelPayload(
        val bytes: ByteArray,
        val flags: Int,
    )

    private object InstructionWrapperPayloadAdapter : TypeAdapter<InstructionModelPayload> {
        override fun encode(encoder: NoritoEncoder, value: InstructionModelPayload) {
            writeField(encoder) { it.writeBytes(value.bytes) }
        }

        override fun decode(decoder: NoritoDecoder): InstructionModelPayload =
            InstructionModelPayload(
                bytes = readField(decoder) { it.readBytes(it.remaining()) },
                flags = decoder.flags,
            )
    }

    private object KeyCertificatePayloadAdapter : TypeAdapter<KeyCertificatePayloadV2> {
        override fun encode(encoder: NoritoEncoder, value: KeyCertificatePayloadV2) {
            writeField(encoder) { writeString(it, value.domain) }
            writeField(encoder) { it.writeUInt(value.version.toLong(), 16) }
            writeField(encoder) { writeString(it, value.platform) }
            writeField(encoder) { writeString(it, value.keyId) }
            writeField(encoder) { writeString(it, value.deviceId) }
            writeField(encoder) { writeAccountId(it, value.accountId) }
            writeField(encoder) { writeBytesVec(it, value.publicKey()) }
            writeField(encoder) { writeString(it, value.assertionScheme) }
            writeField(encoder) { writeString(it, value.assertionKeyAlgorithm) }
            writeField(encoder) { writeBytesVec(it, value.assertionPublicKey()) }
            writeField(encoder) { writeOptionU32(it, value.assertionUsageCountLimit) }
            writeField(encoder) { it.writeByte(if (value.oneUse) 1 else 0) }
        }

        override fun decode(decoder: NoritoDecoder): KeyCertificatePayloadV2 =
            KeyCertificatePayloadV2(
                domain = readField(decoder) { readString(it) },
                version = readField(decoder) { it.readUInt(16).toInt() },
                platform = readField(decoder) { readString(it) },
                keyId = readField(decoder) { readString(it) },
                deviceId = readField(decoder) { readString(it) },
                accountId = readField(decoder) { readAccountId(it) },
                publicKey = readField(decoder) { readBytesVec(it) },
                assertionScheme = readField(decoder) { readString(it) },
                assertionKeyAlgorithm = readField(decoder) { readString(it) },
                assertionPublicKey = readField(decoder) { readBytesVec(it) },
                assertionUsageCountLimit = readField(decoder) { readOptionU32(it) },
                oneUse = readField(decoder) { readBool(it) },
            )
    }

    private object KeyCertificateAdapter : TypeAdapter<KeyCertificateV2> {
        override fun encode(encoder: NoritoEncoder, value: KeyCertificateV2) {
            writeField(encoder) { it.writeUInt(value.version.toLong(), 16) }
            writeField(encoder) { writeString(it, value.platform) }
            writeField(encoder) { writeString(it, value.keyId) }
            writeField(encoder) { writeString(it, value.deviceId) }
            writeField(encoder) { writeAccountId(it, value.accountId) }
            writeField(encoder) { writeBytesVec(it, value.publicKey()) }
            writeField(encoder) { writeString(it, value.assertionScheme) }
            writeField(encoder) { writeString(it, value.assertionKeyAlgorithm) }
            writeField(encoder) { writeBytesVec(it, value.assertionPublicKey()) }
            writeField(encoder) { writeOptionU32(it, value.assertionUsageCountLimit) }
            writeField(encoder) { it.writeByte(if (value.oneUse) 1 else 0) }
            writeField(encoder) { writeConstVec(it, value.issuerSignature()) }
        }

        override fun decode(decoder: NoritoDecoder): KeyCertificateV2 =
            KeyCertificateV2(
                version = readField(decoder) { it.readUInt(16).toInt() },
                platform = readField(decoder) { readString(it) },
                keyId = readField(decoder) { readString(it) },
                deviceId = readField(decoder) { readString(it) },
                accountId = readField(decoder) { readAccountId(it) },
                publicKey = readField(decoder) { readBytesVec(it) },
                assertionScheme = readField(decoder) { readString(it) },
                assertionKeyAlgorithm = readField(decoder) { readString(it) },
                assertionPublicKey = readField(decoder) { readBytesVec(it) },
                assertionUsageCountLimit = readField(decoder) { readOptionU32(it) },
                oneUse = readField(decoder) { readBool(it) },
                issuerSignature = readField(decoder) { readConstVec(it) },
            )
    }

    private object RecursiveProofAdapter : TypeAdapter<RecursiveProofV2> {
        override fun encode(encoder: NoritoEncoder, value: RecursiveProofV2) {
            writeField(encoder) { writeVerifyingKeyId(it, value.verifierKeyId) }
            writeField(encoder) { it.writeBytes(value.publicInputsHash()) }
            writeField(encoder) { writeProofBox(it, value.proof) }
        }

        override fun decode(decoder: NoritoDecoder): RecursiveProofV2 =
            RecursiveProofV2(
                verifierKeyId = readField(decoder) { readVerifyingKeyId(it) },
                publicInputsHash = readField(decoder) { readHash(it, "public_inputs_hash") },
                proof = readField(decoder) { readProofBox(it) },
            )
    }

    private object IssueAdapter : TypeAdapter<IssueV2> {
        override fun encode(encoder: NoritoEncoder, value: IssueV2) {
            writeField(encoder) { it.writeBytes(value.noteCommitment()) }
            writeField(encoder) { KeyCertificateAdapter.encode(it, value.keyCertificate) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
        }

        override fun decode(decoder: NoritoDecoder): IssueV2 =
            IssueV2(
                noteCommitment = readField(decoder) { readHash(it, "note_commitment") },
                keyCertificate = readField(decoder) { KeyCertificateAdapter.decode(it) },
                assetId = readField(decoder) { readAssetId(it) },
                amount = readField(decoder) { readNumeric(it) },
            )
    }

    private object IssuedClaimAdapter : TypeAdapter<IssuedClaimV2> {
        override fun encode(encoder: NoritoEncoder, value: IssuedClaimV2) {
            writeField(encoder) { writeString(it, value.domain) }
            writeField(encoder) { it.writeBytes(value.noteCommitment()) }
            writeField(encoder) { it.writeBytes(value.keyCertificatePayloadHash()) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
        }

        override fun decode(decoder: NoritoDecoder): IssuedClaimV2 =
            IssuedClaimV2(
                domain = readField(decoder) { readString(it) },
                noteCommitment = readField(decoder) { readHash(it, "note_commitment") },
                keyCertificatePayloadHash = readField(decoder) {
                    readHash(it, "key_certificate_payload_hash")
                },
                assetId = readField(decoder) { readAssetId(it) },
                amount = readField(decoder) { readNumeric(it) },
            )
    }

    private object AuditOutputClaimAdapter : TypeAdapter<AuditOutputClaimV2> {
        override fun encode(encoder: NoritoEncoder, value: AuditOutputClaimV2) {
            writeField(encoder) { it.writeBytes(value.noteCommitment()) }
            writeField(encoder) { KeyCertificateAdapter.encode(it, value.keyCertificate) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
        }

        override fun decode(decoder: NoritoDecoder): AuditOutputClaimV2 =
            AuditOutputClaimV2(
                noteCommitment = readField(decoder) { readHash(it, "note_commitment") },
                keyCertificate = readField(decoder) { KeyCertificateAdapter.decode(it) },
                assetId = readField(decoder) { readAssetId(it) },
                amount = readField(decoder) { readNumeric(it) },
            )
    }

    private object RedeemPublicInputsAdapter : TypeAdapter<RedeemPublicInputsV2> {
        override fun encode(encoder: NoritoEncoder, value: RedeemPublicInputsV2) {
            writeField(encoder) { writeString(it, value.domain) }
            writeField(encoder) { it.writeBytes(value.sourceNoteCommitment()) }
            writeField(encoder) { writeVec(it, value.inputNullifiers()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { it.writeBytes(value.keyCertificatePayloadHash()) }
            writeField(encoder) { writeAccountId(it, value.recipient) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
        }

        override fun decode(decoder: NoritoDecoder): RedeemPublicInputsV2 =
            RedeemPublicInputsV2(
                domain = readField(decoder) { readString(it) },
                sourceNoteCommitment = readField(decoder) { readHash(it, "source_note_commitment") },
                inputNullifiers = readField(decoder) {
                    readVec(it) { child -> readHash(child, "input_nullifier") }
                },
                keyCertificatePayloadHash = readField(decoder) {
                    readHash(it, "key_certificate_payload_hash")
                },
                recipient = readField(decoder) { readAccountId(it) },
                assetId = readField(decoder) { readAssetId(it) },
                amount = readField(decoder) { readNumeric(it) },
            )
    }

    private object RedeemAdapter : TypeAdapter<RedeemV2> {
        override fun encode(encoder: NoritoEncoder, value: RedeemV2) {
            writeField(encoder) { it.writeBytes(value.sourceNoteCommitment()) }
            writeField(encoder) { writeVec(it, value.inputNullifiers()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { KeyCertificateAdapter.encode(it, value.senderKeyCertificate) }
            writeField(encoder) { writeAccountId(it, value.recipient) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
            writeField(encoder) { RecursiveProofAdapter.encode(it, value.recursiveProof) }
        }

        override fun decode(decoder: NoritoDecoder): RedeemV2 =
            RedeemV2(
                sourceNoteCommitment = readField(decoder) { readHash(it, "source_note_commitment") },
                inputNullifiers = readField(decoder) {
                    readVec(it) { child -> readHash(child, "input_nullifier") }
                },
                senderKeyCertificate = readField(decoder) { KeyCertificateAdapter.decode(it) },
                recipient = readField(decoder) { readAccountId(it) },
                assetId = readField(decoder) { readAssetId(it) },
                amount = readField(decoder) { readNumeric(it) },
                recursiveProof = readField(decoder) { RecursiveProofAdapter.decode(it) },
            )
    }

    private object AuditPublicInputsAdapter : TypeAdapter<AuditPublicInputsV2> {
        override fun encode(encoder: NoritoEncoder, value: AuditPublicInputsV2) {
            writeField(encoder) { writeString(it, value.domain) }
            writeField(encoder) { it.writeBytes(value.tokenId()) }
            writeField(encoder) { it.writeBytes(value.keyCertificatePayloadHash()) }
            writeField(encoder) { writeVec(it, value.inputNullifiers()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { writeVec(it, value.inputClaims) { out, claim -> IssuedClaimAdapter.encode(out, claim) } }
            writeField(encoder) { writeVec(it, value.outputCommitments()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { writeVec(it, value.outputClaims) { out, claim -> IssuedClaimAdapter.encode(out, claim) } }
        }

        override fun decode(decoder: NoritoDecoder): AuditPublicInputsV2 =
            AuditPublicInputsV2(
                domain = readField(decoder) { readString(it) },
                tokenId = readField(decoder) { readHash(it, "token_id") },
                keyCertificatePayloadHash = readField(decoder) {
                    readHash(it, "key_certificate_payload_hash")
                },
                inputNullifiers = readField(decoder) {
                    readVec(it) { child -> readHash(child, "input_nullifier") }
                },
                inputClaims = readField(decoder) {
                    readVec(it) { child -> IssuedClaimAdapter.decode(child) }
                },
                outputCommitments = readField(decoder) {
                    readVec(it) { child -> readHash(child, "output_commitment") }
                },
                outputClaims = readField(decoder) {
                    readVec(it) { child -> IssuedClaimAdapter.decode(child) }
                },
            )
    }

    private object AuditAdapter : TypeAdapter<AuditBundleV2> {
        override fun encode(encoder: NoritoEncoder, value: AuditBundleV2) {
            writeField(encoder) { it.writeBytes(value.tokenId()) }
            writeField(encoder) { KeyCertificateAdapter.encode(it, value.senderKeyCertificate) }
            writeField(encoder) { writeVec(it, value.inputNullifiers()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { writeVec(it, value.inputClaims) { out, claim -> IssuedClaimAdapter.encode(out, claim) } }
            writeField(encoder) { writeVec(it, value.outputCommitments()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { writeVec(it, value.outputClaims) { out, claim -> AuditOutputClaimAdapter.encode(out, claim) } }
            writeField(encoder) { RecursiveProofAdapter.encode(it, value.recursiveProof) }
        }

        override fun decode(decoder: NoritoDecoder): AuditBundleV2 =
            AuditBundleV2(
                tokenId = readField(decoder) { readHash(it, "token_id") },
                senderKeyCertificate = readField(decoder) { KeyCertificateAdapter.decode(it) },
                inputNullifiers = readField(decoder) {
                    readVec(it) { child -> readHash(child, "input_nullifier") }
                },
                inputClaims = readField(decoder) {
                    readVec(it) { child -> IssuedClaimAdapter.decode(child) }
                },
                outputCommitments = readField(decoder) {
                    readVec(it) { child -> readHash(child, "output_commitment") }
                },
                outputClaims = readField(decoder) {
                    readVec(it) { child -> AuditOutputClaimAdapter.decode(child) }
                },
                recursiveProof = readField(decoder) { RecursiveProofAdapter.decode(it) },
            )
    }

    private fun writeField(parent: NoritoEncoder, writePayload: (NoritoEncoder) -> Unit) {
        val child = parent.childEncoder()
        writePayload(child)
        val payload = child.toByteArray()
        parent.writeLength(payload.size.toLong(), compact(parent))
        parent.writeBytes(payload)
    }

    private fun <T> writeVec(
        encoder: NoritoEncoder,
        values: List<T>,
        writeElement: (NoritoEncoder, T) -> Unit,
    ) {
        encoder.writeUInt(values.size.toLong(), 64)
        for (value in values) {
            writeField(encoder) { writeElement(it, value) }
        }
    }

    private fun writeString(encoder: NoritoEncoder, value: String) {
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        encoder.writeLength(bytes.size.toLong(), compact(encoder))
        encoder.writeBytes(bytes)
    }

    private fun writeBytesVec(encoder: NoritoEncoder, value: ByteArray) {
        encoder.writeUInt(value.size.toLong(), 64)
        encoder.writeBytes(value)
    }

    private fun writeConstVec(encoder: NoritoEncoder, value: ByteArray) {
        encoder.writeUInt(value.size.toLong(), 64)
        for (byte in value) {
            encoder.writeLength(1, compact(encoder))
            encoder.writeByte(byte.toInt())
        }
    }

    private fun <T> readField(parent: NoritoDecoder, readPayload: (NoritoDecoder) -> T): T {
        val length = checkedLength(parent.readLength(compact(parent)), "field length")
        val child = NoritoDecoder(parent.readBytes(length), parent.flags, parent.flagsHint)
        val value = readPayload(child)
        require(child.remaining() == 0) { "Trailing bytes after Offline Note V2 field decode" }
        return value
    }

    private fun readString(decoder: NoritoDecoder): String {
        val length = checkedLength(decoder.readLength(compact(decoder)), "string length")
        return String(decoder.readBytes(length), StandardCharsets.UTF_8)
    }

    private fun readBool(decoder: NoritoDecoder): Boolean =
        when (val tag = decoder.readByte()) {
            0 -> false
            1 -> true
            else -> throw IllegalArgumentException("invalid boolean tag: $tag")
        }

    private fun readBytesVec(decoder: NoritoDecoder): ByteArray {
        val length = checkedLength(decoder.readUInt(64), "byte vector length")
        return decoder.readBytes(length)
    }

    private fun readConstVec(decoder: NoritoDecoder): ByteArray {
        val length = checkedLength(decoder.readUInt(64), "const vector length")
        val out = ByteArray(length)
        for (idx in out.indices) {
            val elementLength = decoder.readLength(compact(decoder))
            require(elementLength == 1L) { "const u8 vector element length must be 1" }
            out[idx] = decoder.readByte().toByte()
        }
        return out
    }

    private fun readOptionU32(decoder: NoritoDecoder): Int? =
        when (val tag = decoder.readByte()) {
            0 -> null
            1 -> readField(decoder) { it.readUInt(32).toInt() }
            else -> throw IllegalArgumentException("invalid option tag: $tag")
        }

    private fun <T> readVec(
        decoder: NoritoDecoder,
        readElement: (NoritoDecoder) -> T,
    ): List<T> {
        val count = checkedLength(decoder.readUInt(64), "vector length")
        val values = ArrayList<T>(count)
        repeat(count) {
            values.add(readField(decoder, readElement))
        }
        return values
    }

    private fun readHash(decoder: NoritoDecoder, field: String): ByteArray {
        val bytes = decoder.readBytes(32)
        requireHash(bytes, field)
        return bytes
    }

    private fun readVerifyingKeyId(decoder: NoritoDecoder): VerifyingKeyIdReference =
        VerifyingKeyIdReference(
            backend = readField(decoder) { readString(it) },
            name = readField(decoder) { readString(it) },
        )

    private fun readProofBox(decoder: NoritoDecoder): ProofBox =
        ProofBox(
            backend = readField(decoder) { readString(it) },
            bytes = readField(decoder) { readBytesVec(it) },
        )

    private fun readAccountId(decoder: NoritoDecoder): String =
        when (val tag = decoder.readUInt(32)) {
            0L -> readField(decoder) { payload ->
                val publicKey = readPublicKeyPayload(payload)
                val algorithm = algorithmForCurveId(publicKey.curveId)
                    ?: throw IllegalArgumentException("unsupported public key curve id: ${publicKey.curveId}")
                try {
                    AccountAddress.fromAccount(publicKey.keyBytes, algorithm).toI105Default()
                } catch (ex: AccountAddressException) {
                    throw IllegalArgumentException("invalid decoded account id", ex)
                }
            }
            1L -> readField(decoder) { payload ->
                try {
                    AccountAddress.fromMultisigPolicy(readMultisigPolicy(payload)).toI105Default()
                } catch (ex: AccountAddressException) {
                    throw IllegalArgumentException("invalid decoded multisig account id", ex)
                }
            }
            else -> throw IllegalArgumentException("unsupported account controller tag: $tag")
        }

    private fun readPublicKeyPayload(decoder: NoritoDecoder) =
        decodeCompactPublicKeyPayload(readConstVec(decoder))
            ?: throw IllegalArgumentException("invalid public key payload")

    private fun readMultisigPolicy(decoder: NoritoDecoder): MultisigPolicyPayload {
        val version = readField(decoder) { it.readUInt(8).toInt() }
        val threshold = readField(decoder) { it.readUInt(16).toInt() }
        val members = readField(decoder) { payload ->
            readVec(payload) { member ->
                val publicKey = readField(member) { readPublicKeyPayload(it) }
                val weight = readField(member) { it.readUInt(16).toInt() }
                MultisigMemberPayload(publicKey.curveId, weight, publicKey.keyBytes)
            }
        }
        return MultisigPolicyPayload.of(version, threshold, members)
    }

    private fun readAssetId(decoder: NoritoDecoder): String {
        val accountId = readField(decoder) { readAccountId(it) }
        val definitionBytes = readField(decoder) { readAssetDefinitionAddress(it) }
        val definitionId = AssetDefinitionIdEncoder.encodeFromBytes(definitionBytes)
        val dataspaceId = readField(decoder) { readAssetBalanceScope(it) }
        val base = "$definitionId#$accountId"
        return if (dataspaceId == null) base else "$base#dataspace:$dataspaceId"
    }

    private fun readAssetDefinitionAddress(decoder: NoritoDecoder): ByteArray {
        val bytes = ArrayList<Byte>()
        while (decoder.remaining() > 0) {
            val length = decoder.readLength(compact(decoder))
            require(length == 1L) { "asset definition byte field length must be 1" }
            bytes.add(decoder.readByte().toByte())
        }
        return bytes.toByteArray()
    }

    private fun readAssetBalanceScope(decoder: NoritoDecoder): Long? =
        when (val tag = decoder.readUInt(32)) {
            0L -> null
            1L -> readField(decoder) { it.readUInt(64) }
            else -> throw IllegalArgumentException("unsupported asset balance scope tag: $tag")
        }

    private fun readNumeric(decoder: NoritoDecoder): String {
        val mantissaBytes = readField(decoder) { payload ->
            val length = checkedLength(payload.readUInt(32), "numeric mantissa length")
            payload.readBytes(length)
        }
        val scale = readField(decoder) { it.readUInt(32).toInt() }
        return canonicalNumericString(bigIntegerFromLittleEndianTwosComplement(mantissaBytes), scale)
    }

    private fun bigIntegerFromLittleEndianTwosComplement(bytes: ByteArray): BigInteger {
        if (bytes.isEmpty()) return BigInteger.ZERO
        val bigEndian = ByteArray(bytes.size)
        for (idx in bytes.indices) {
            bigEndian[idx] = bytes[bytes.size - 1 - idx]
        }
        return BigInteger(bigEndian)
    }

    private fun checkedLength(value: Long, field: String): Int {
        require(value >= 0) { "$field must be non-negative" }
        require(value <= Int.MAX_VALUE) { "$field exceeds JVM array limit" }
        return value.toInt()
    }

    private fun writeOptionU32(encoder: NoritoEncoder, value: Int?) {
        if (value == null) {
            encoder.writeByte(0)
            return
        }
        encoder.writeByte(1)
        writeField(encoder) { it.writeUInt(value.toLong(), 32) }
    }

    private fun writeVerifyingKeyId(encoder: NoritoEncoder, value: VerifyingKeyIdReference) {
        writeField(encoder) { writeString(it, value.backend) }
        writeField(encoder) { writeString(it, value.name) }
    }

    private fun writeProofBox(encoder: NoritoEncoder, value: ProofBox) {
        writeField(encoder) { writeString(it, value.backend) }
        writeField(encoder) { writeBytesVec(it, value.bytes()) }
    }

    private fun writeAccountId(encoder: NoritoEncoder, accountId: String) {
        encoder.writeBytes(encodeAccountIdPayload(accountId))
    }

    private fun encodeAccountIdPayload(accountId: String): ByteArray {
        val address = try {
            AccountAddress.parseEncodedIgnoringCurveSupport(accountId, null).address
        } catch (ex: AccountAddressException) {
            throw IllegalArgumentException("account id must use canonical I105 form", ex)
        }
        val single = address.singleKeyPayloadIgnoringCurveSupport()
        if (single != null) {
            val encoder = NoritoEncoder(NoritoHeader.COMPACT_LEN)
            encoder.writeUInt(0, 32)
            writeField(encoder) { writePublicKey(it, single.curveId, single.publicKey) }
            return encoder.toByteArray()
        }
        val multisig = address.multisigPolicyPayloadIgnoringCurveSupport()
            ?: throw IllegalArgumentException("account id has no supported controller")
        val encoder = NoritoEncoder(NoritoHeader.COMPACT_LEN)
        encoder.writeUInt(1, 32)
        writeField(encoder) { writeMultisigPolicy(it, multisig) }
        return encoder.toByteArray()
    }

    private fun writeMultisigPolicy(encoder: NoritoEncoder, policy: MultisigPolicyPayload) {
        require(policy.version == MULTISIG_POLICY_VERSION_V1) { "unsupported multisig policy version" }
        require(policy.threshold > 0) { "multisig threshold must be positive" }
        require(policy.members.isNotEmpty()) { "multisig policy must have members" }
        writeField(encoder) { it.writeUInt(policy.version.toLong(), 8) }
        writeField(encoder) { it.writeUInt(policy.threshold.toLong(), 16) }
        writeField(encoder) { writeMultisigMembers(it, policy.members) }
    }

    private fun writeMultisigMembers(encoder: NoritoEncoder, members: List<MultisigMemberPayload>) {
        val sorted = members.sortedWith { a, b -> compareUnsigned(canonicalSortKey(a), canonicalSortKey(b)) }
        for (i in 1 until sorted.size) {
            require(!canonicalSortKey(sorted[i - 1]).contentEquals(canonicalSortKey(sorted[i]))) {
                "duplicate multisig member"
            }
        }
        encoder.writeUInt(sorted.size.toLong(), 64)
        for (member in sorted) {
            writeField(encoder) { memberEncoder ->
                writeField(memberEncoder) { writePublicKey(it, member.curveId, member.publicKey) }
                writeField(memberEncoder) { it.writeUInt(member.weight.toLong(), 16) }
            }
        }
    }

    private fun writePublicKey(encoder: NoritoEncoder, curveId: Int, publicKey: ByteArray) {
        writeConstVec(encoder, compactPublicKeyPayload(curveId, publicKey))
    }

    private fun writeAssetId(encoder: NoritoEncoder, assetId: String) {
        val parsed = parseAssetId(assetId)
        writeField(encoder) { writeAccountId(it, parsed.accountId) }
        writeField(encoder) { writeAssetDefinitionAddress(it, parsed.definitionBytes) }
        writeField(encoder) { writeAssetBalanceScope(it, parsed.dataspaceId) }
    }

    private fun writeAssetDefinitionAddress(encoder: NoritoEncoder, bytes: ByteArray) {
        for (byte in bytes) {
            encoder.writeLength(1, compact(encoder))
            encoder.writeByte(byte.toInt())
        }
    }

    private fun writeAssetBalanceScope(encoder: NoritoEncoder, dataspaceId: Long?) {
        if (dataspaceId == null) {
            encoder.writeUInt(0, 32)
            return
        }
        encoder.writeUInt(1, 32)
        writeField(encoder) { it.writeUInt(dataspaceId, 64) }
    }

    private fun writeNumeric(encoder: NoritoEncoder, value: String) {
        val numeric = parseNumeric(value)
        writeField(encoder) { bigint ->
            val bytes = numeric.mantissaBytes
            bigint.writeUInt(bytes.size.toLong(), 32)
            bigint.writeBytes(bytes)
        }
        writeField(encoder) { it.writeUInt(numeric.scale.toLong(), 32) }
    }

    private fun parseAssetId(value: String): ParsedAssetId {
        val parts = value.split('#')
        require(parts.size == 2 || parts.size == 3) {
            "asset id must be '<asset-definition>#<account>' with optional '#dataspace:<id>'"
        }
        val definitionBytes = AssetDefinitionIdEncoder.parseAddressBytes(parts[0])
        encodeAccountIdPayload(parts[1])
        val dataspaceId = if (parts.size == 3) {
            val scope = parts[2]
            require(scope.startsWith("dataspace:")) { "asset scope must use dataspace:<id>" }
            scope.substring("dataspace:".length).toLong()
        } else {
            null
        }
        return ParsedAssetId(parts[1], definitionBytes, dataspaceId)
    }

    private fun parseNumeric(value: String): NumericValue {
        val decimal = BigDecimal(value)
        val scale = decimal.scale().coerceAtLeast(0)
        require(scale <= MAX_NUMERIC_SCALE) { "numeric scale exceeds $MAX_NUMERIC_SCALE" }
        val mantissa = decimal.movePointRight(scale).toBigIntegerExact()
        val mantissaBytes = toTwosComplementLittleEndian(mantissa)
        require(mantissaBytes.size <= MAX_BIGINT_BYTES) { "numeric mantissa exceeds $MAX_BIGINT_BYTES bytes" }
        return NumericValue(mantissaBytes, scale, canonicalNumericString(mantissa, scale))
    }

    private fun canonicalNumericString(mantissa: BigInteger, scale: Int): String {
        val negative = mantissa.signum() < 0
        var digits = mantissa.abs().toString()
        while (digits.length > 1 && digits[0] == '0') {
            digits = digits.substring(1)
        }
        if (scale == 0) {
            return if (negative && digits != "0") "-$digits" else digits
        }
        while (digits.length <= scale) {
            digits = "0$digits"
        }
        val splitAt = digits.length - scale
        val body = digits.substring(0, splitAt) + "." + digits.substring(splitAt)
        return if (negative && mantissa.signum() != 0) "-$body" else body
    }

    private fun toTwosComplementLittleEndian(value: BigInteger): ByteArray {
        if (value.signum() == 0) return ByteArray(0)
        val be = value.toByteArray()
        val le = ByteArray(be.size)
        for (i in be.indices) {
            le[i] = be[be.size - 1 - i]
        }
        var len = le.size
        if (value.signum() > 0) {
            while (len > 1 && le[len - 1].toInt() == 0 && (le[len - 2].toInt() and 0x80) == 0) len--
        } else {
            while (len > 1 && le[len - 1] == 0xFF.toByte() && (le[len - 2].toInt() and 0x80) != 0) len--
        }
        return if (len == le.size) le else le.copyOf(len)
    }

    private fun validateCount(count: Int, max: Int, label: String): Long {
        require(count in 1..max) {
            "Offline V2 $label count $count must be in 1..$max"
        }
        return count.toLong()
    }

    private fun publicValues(
        publicInputsHash: ByteArray,
        mode: Long,
        inputCount: Long,
        outputCount: Long,
        inputSum: Long,
        outputSum: Long,
        inputNullifierSum: Long,
        outputCommitmentSum: Long,
        keyCertificatePayloadHash: ByteArray,
        sourceOrToken: ByteArray,
        inputClaimHashSum: Long,
        outputClaimHashSum: Long,
    ): LongArray {
        val limbs = hashLimbsLE(publicInputsHash)
        return longArrayOf(
            limbs[0],
            limbs[1],
            limbs[2],
            limbs[3],
            mode,
            inputCount,
            outputCount,
            inputSum,
            outputSum,
            inputNullifierSum,
            outputCommitmentSum,
            hashLimb0(keyCertificatePayloadHash),
            hashLimb0(sourceOrToken),
            inputClaimHashSum,
            outputClaimHashSum,
            0L,
        )
    }

    private fun normalizedAmountUnits(amounts: List<String>): List<Long> {
        val trimmed = amounts.map { trimmedNumeric(it) }
        val targetScale = trimmed.maxOfOrNull { it.scale } ?: 0
        return trimmed.map { numeric ->
            require(numeric.mantissa.signum() >= 0) {
                "Offline V2 amount ${numeric.original} must not be negative"
            }
            val scaleDelta = targetScale - numeric.scale
            val aligned = numeric.mantissa.multiply(BigInteger.TEN.pow(scaleDelta))
            require(aligned.bitLength() <= 64) {
                "Offline V2 amount ${numeric.original} does not fit the u64 witness corridor"
            }
            aligned.toLong()
        }
    }

    private fun trimmedNumeric(amount: String): TrimmedNumeric {
        val numeric = parseNumeric(amount)
        val decimal = BigDecimal(numeric.canonicalString).stripTrailingZeros()
        val scale = decimal.scale().coerceAtLeast(0)
        val mantissa = decimal.movePointRight(scale).toBigIntegerExact()
        return TrimmedNumeric(amount, mantissa, scale)
    }

    private fun checkedSum(values: List<Long>, label: String): Long {
        var sum = BigInteger.ZERO
        for (value in values) {
            sum = sum.add(unsignedLongToBigInteger(value))
            require(sum <= MAX_U64) {
                "Offline V2 $label amount sum overflows u64 witness units"
            }
        }
        return sum.toLong()
    }

    private fun unsignedLongToBigInteger(value: Long): BigInteger {
        val bytes = ByteArray(9)
        for (idx in 0 until 8) {
            bytes[8 - idx] = (value ushr (idx * 8)).toByte()
        }
        return BigInteger(bytes)
    }

    private fun hashLimb0Sum(hashes: List<ByteArray>): Long {
        var sum = 0L
        for (hash in hashes) {
            sum += hashLimb0(hash)
        }
        return sum
    }

    private fun hashLimb0(hash: ByteArray): Long = hashLimbsLE(hash)[0]

    private fun hashLimbsLE(hash: ByteArray): LongArray {
        require(hash.size == 32) { "hash must be 32 bytes" }
        val limbs = LongArray(4)
        for (idx in 0 until 4) {
            val start = idx * 8
            var value = 0L
            for (offset in 0 until 8) {
                value = value or ((hash[start + offset].toLong() and 0xffL) shl (offset * 8))
            }
            limbs[idx] = value
        }
        return limbs
    }

    private fun requireCertificateCore(version: Int, accountId: String, publicKey: ByteArray, oneUse: Boolean) {
        require(version == KEY_CERTIFICATE_VERSION) {
            "Offline Note V2 key certificate version must be $KEY_CERTIFICATE_VERSION"
        }
        require(oneUse) { "Offline Note V2 key certificate must be one-use" }
        require(publicKey.size == 32) { "Offline Note V2 note public key must be 32 bytes" }
        encodeAccountIdPayload(accountId)
    }

    private fun requireHash(value: ByteArray, field: String) {
        require(value.size == 32) { "$field must be 32 bytes" }
        require((value[value.size - 1].toInt() and 1) == 1) {
            "$field must carry the Iroha prehash marker"
        }
    }

    private fun requireHashes(values: List<ByteArray>, field: String) {
        require(values.isNotEmpty()) { "$field must not be empty" }
        for (i in values.indices) {
            requireHash(values[i], "$field[$i]")
        }
    }

    private fun requireNonBlank(value: String, field: String): String {
        val trimmed = value.trim()
        require(trimmed.isNotEmpty()) { "$field must not be empty" }
        return trimmed
    }

    private fun requireNonBlankUnpadded(value: String, field: String): String {
        require(value.trim().isNotEmpty()) { "$field must not be empty" }
        require(value.trim() == value) { "$field must not contain surrounding whitespace" }
        return value
    }

    private fun canonicalSortKey(member: MultisigMemberPayload): ByteArray {
        val algorithm = algorithmForCurveId(member.curveId)
            ?: throw IllegalArgumentException("unknown multisig curve id")
        val algorithmBytes = algorithm.toByteArray(StandardCharsets.UTF_8)
        val keyBytes = member.publicKey
        val sortKey = ByteArray(algorithmBytes.size + 1 + keyBytes.size)
        System.arraycopy(algorithmBytes, 0, sortKey, 0, algorithmBytes.size)
        sortKey[algorithmBytes.size] = 0
        System.arraycopy(keyBytes, 0, sortKey, algorithmBytes.size + 1, keyBytes.size)
        return sortKey
    }

    private fun compareUnsigned(a: ByteArray, b: ByteArray): Int {
        val len = minOf(a.size, b.size)
        for (i in 0 until len) {
            val cmp = (a[i].toInt() and 0xFF) - (b[i].toInt() and 0xFF)
            if (cmp != 0) return cmp
        }
        return a.size.compareTo(b.size)
    }

    private fun compact(encoder: NoritoEncoder): Boolean =
        (encoder.flags and NoritoHeader.COMPACT_LEN) != 0

    private fun compact(decoder: NoritoDecoder): Boolean =
        (decoder.flags and NoritoHeader.COMPACT_LEN) != 0

    private fun hexLower(bytes: ByteArray): String =
        bytes.joinToString(separator = "") { "%02x".format(it.toInt() and 0xFF) }

    private class ParsedAssetId(
        val accountId: String,
        val definitionBytes: ByteArray,
        val dataspaceId: Long?,
    )

    private class NumericValue(
        val mantissaBytes: ByteArray,
        val scale: Int,
        val canonicalString: String,
    )

    private class TrimmedNumeric(
        val original: String,
        val mantissa: BigInteger,
        val scale: Int,
    )
}
