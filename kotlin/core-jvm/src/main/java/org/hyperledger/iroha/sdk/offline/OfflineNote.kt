package org.hyperledger.iroha.sdk.offline

import java.math.BigDecimal
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.MultisigMemberPayload
import org.hyperledger.iroha.sdk.address.MultisigPolicyPayload
import org.hyperledger.iroha.sdk.address.PublicKeyPayload
import org.hyperledger.iroha.sdk.address.algorithmForCurveId
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Native JVM implementation of Iroha Offline Note canonical Norito encodings. */
object OfflineNote {
    const val KEY_CERTIFICATE_PAYLOAD_DOMAIN: String =
        "iroha:offline-note:key-certificate-payload"
    const val ISSUED_CLAIM_DOMAIN: String = "iroha:offline-note:issued-claim"
    const val REDEEM_PUBLIC_INPUTS_DOMAIN: String =
        "iroha:offline-note:redeem-public-inputs"
    const val AUDIT_PUBLIC_INPUTS_DOMAIN: String =
        "iroha:offline-note:audit-public-inputs"
    const val NOTE_COMMITMENT_DOMAIN: String =
        "iroha:offline-note:note-commitment"
    const val INPUT_NULLIFIER_DOMAIN: String =
        "iroha:offline-note:input-nullifier"
    const val PAYMENT_TOKEN_ID_DOMAIN: String =
        "iroha:offline-note:payment-token-id"
    const val RECURSIVE_BACKEND: String = "halo2/ipa"
    const val RECURSIVE_VERIFIER_NAME: String = "offline-note-recursive"
    const val RECURSIVE_PUBLIC_INPUTS_SCHEMA: String =
        "{\"schema\":\"offline_note_recursive\",\"public_inputs\":[\"public_inputs_hash_limb0\",\"public_inputs_hash_limb1\",\"public_inputs_hash_limb2\",\"public_inputs_hash_limb3\",\"proof_mode\",\"input_count\",\"output_count\",\"input_amount_sum\",\"output_amount_sum\",\"input_nullifier_sum_limb0\",\"output_commitment_sum_limb0\",\"key_certificate_payload_hash_limb0\",\"source_or_token_limb0\",\"input_claim_hash_sum_limb0\",\"output_claim_hash_sum_limb0\",\"reserved_zero\"]}"
    const val KEY_CERTIFICATE_VERSION: Int = 1

    private const val MULTISIG_POLICY_VERSION = 1
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
    private const val NOTE_COMMITMENT_PREIMAGE_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteCommitmentPreimage"
    private const val INPUT_NULLIFIER_PREIMAGE_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteInputNullifierPreimage"
    private const val PAYMENT_TOKEN_ID_PREIMAGE_SCHEMA =
        "iroha_data_model::offline::model::OfflineNotePaymentTokenIdPreimage"
    const val ISSUE_INSTRUCTION_SCHEMA: String =
        "iroha_data_model::isi::offline::IssueOfflineNote"
    const val REDEEM_INSTRUCTION_SCHEMA: String =
        "iroha_data_model::isi::offline::RedeemOfflineNote"
    const val AUDIT_INSTRUCTION_SCHEMA: String =
        "iroha_data_model::isi::offline::AuditOfflineNote"

    @JvmStatic
    fun encodeCertificatePayload(value: KeyCertificatePayload): ByteArray =
        encodeWithHeader(value, KEY_CERTIFICATE_PAYLOAD_SCHEMA, KeyCertificatePayloadAdapter)

    @JvmStatic
    fun encodeCertificate(value: KeyCertificate): ByteArray =
        encodeWithHeader(value, KEY_CERTIFICATE_SCHEMA, KeyCertificateAdapter)

    @JvmStatic
    fun encodeIssue(value: Issue): ByteArray =
        encodeWithHeader(value, ISSUE_SCHEMA, IssueAdapter)

    @JvmStatic
    fun encodeIssuedClaim(value: IssuedClaim): ByteArray =
        encodeWithHeader(value, ISSUED_CLAIM_SCHEMA, IssuedClaimAdapter)

    @JvmStatic
    fun encodeRedeem(value: Redeem): ByteArray =
        encodeWithHeader(value, REDEEM_SCHEMA, RedeemAdapter)

    @JvmStatic
    fun encodeRedeemPublicInputs(value: RedeemPublicInputs): ByteArray =
        encodeWithHeader(value, REDEEM_PUBLIC_INPUTS_SCHEMA, RedeemPublicInputsAdapter)

    @JvmStatic
    fun encodeAudit(value: AuditBundle): ByteArray =
        encodeWithHeader(value, AUDIT_SCHEMA, AuditAdapter)

    @JvmStatic
    fun encodeAuditPublicInputs(value: AuditPublicInputs): ByteArray =
        encodeWithHeader(value, AUDIT_PUBLIC_INPUTS_SCHEMA, AuditPublicInputsAdapter)

    @JvmStatic
    fun encodeNoteCommitmentPreimage(value: NoteCommitmentPreimage): ByteArray =
        encodeWithHeader(value, NOTE_COMMITMENT_PREIMAGE_SCHEMA, NoteCommitmentPreimageAdapter)

    @JvmStatic
    fun encodeInputNullifierPreimage(value: InputNullifierPreimage): ByteArray =
        encodeWithHeader(value, INPUT_NULLIFIER_PREIMAGE_SCHEMA, InputNullifierPreimageAdapter)

    @JvmStatic
    fun encodePaymentTokenIdPreimage(value: PaymentTokenIdPreimage): ByteArray =
        encodeWithHeader(value, PAYMENT_TOKEN_ID_PREIMAGE_SCHEMA, PaymentTokenIdPreimageAdapter)

    @JvmStatic
    fun decodeCertificatePayload(bytes: ByteArray): KeyCertificatePayload =
        decodeWithHeader(bytes, KEY_CERTIFICATE_PAYLOAD_SCHEMA, KeyCertificatePayloadAdapter)

    @JvmStatic
    fun decodeCertificate(bytes: ByteArray): KeyCertificate =
        decodeWithHeader(bytes, KEY_CERTIFICATE_SCHEMA, KeyCertificateAdapter)

    @JvmStatic
    fun decodeIssue(bytes: ByteArray): Issue =
        decodeWithHeader(bytes, ISSUE_SCHEMA, IssueAdapter)

    @JvmStatic
    fun decodeIssuedClaim(bytes: ByteArray): IssuedClaim =
        decodeWithHeader(bytes, ISSUED_CLAIM_SCHEMA, IssuedClaimAdapter)

    @JvmStatic
    fun decodeRecursiveProof(bytes: ByteArray): RecursiveProof =
        decodeWithHeader(bytes, RECURSIVE_PROOF_SCHEMA, RecursiveProofAdapter)

    @JvmStatic
    fun decodeRedeem(bytes: ByteArray): Redeem =
        decodeWithHeader(bytes, REDEEM_SCHEMA, RedeemAdapter)

    @JvmStatic
    fun decodeRedeemPublicInputs(bytes: ByteArray): RedeemPublicInputs =
        decodeWithHeader(bytes, REDEEM_PUBLIC_INPUTS_SCHEMA, RedeemPublicInputsAdapter)

    @JvmStatic
    fun decodeAudit(bytes: ByteArray): AuditBundle =
        decodeWithHeader(bytes, AUDIT_SCHEMA, AuditAdapter)

    @JvmStatic
    fun decodeAuditPublicInputs(bytes: ByteArray): AuditPublicInputs =
        decodeWithHeader(bytes, AUDIT_PUBLIC_INPUTS_SCHEMA, AuditPublicInputsAdapter)

    @JvmStatic
    fun decodeNoteCommitmentPreimage(bytes: ByteArray): NoteCommitmentPreimage =
        decodeWithHeader(bytes, NOTE_COMMITMENT_PREIMAGE_SCHEMA, NoteCommitmentPreimageAdapter)

    @JvmStatic
    fun decodeInputNullifierPreimage(bytes: ByteArray): InputNullifierPreimage =
        decodeWithHeader(bytes, INPUT_NULLIFIER_PREIMAGE_SCHEMA, InputNullifierPreimageAdapter)

    @JvmStatic
    fun decodePaymentTokenIdPreimage(bytes: ByteArray): PaymentTokenIdPreimage =
        decodeWithHeader(bytes, PAYMENT_TOKEN_ID_PREIMAGE_SCHEMA, PaymentTokenIdPreimageAdapter)

    @JvmStatic
    fun decodeIssueInstruction(bytes: ByteArray): Issue =
        decodeInstructionModel(
            bytes,
            ISSUE_INSTRUCTION_SCHEMA,
            ISSUE_SCHEMA,
            IssueAdapter,
        )

    @JvmStatic
    fun decodeRedeemInstruction(bytes: ByteArray): Redeem =
        decodeInstructionModel(
            bytes,
            REDEEM_INSTRUCTION_SCHEMA,
            REDEEM_SCHEMA,
            RedeemAdapter,
        )

    @JvmStatic
    fun decodeAuditInstruction(bytes: ByteArray): AuditBundle =
        decodeInstructionModel(
            bytes,
            AUDIT_INSTRUCTION_SCHEMA,
            AUDIT_SCHEMA,
            AuditAdapter,
        )

    @JvmStatic
    fun issueInstruction(value: Issue): InstructionBox =
        InstructionBox.fromWirePayload(
            ISSUE_INSTRUCTION_SCHEMA,
            encodeInstructionWrapper(ISSUE_INSTRUCTION_SCHEMA, encodeIssue(value)),
        )

    @JvmStatic
    fun redeemInstruction(value: Redeem): InstructionBox {
        value.validateProofBinding()
        return InstructionBox.fromWirePayload(
            REDEEM_INSTRUCTION_SCHEMA,
            encodeInstructionWrapper(REDEEM_INSTRUCTION_SCHEMA, encodeRedeem(value)),
        )
    }

    @JvmStatic
    fun auditInstruction(value: AuditBundle): InstructionBox {
        value.validateProofBinding()
        return InstructionBox.fromWirePayload(
            AUDIT_INSTRUCTION_SCHEMA,
            encodeInstructionWrapper(AUDIT_INSTRUCTION_SCHEMA, encodeAudit(value)),
        )
    }

    @JvmStatic
    fun deriveNoteCommitment(value: NoteCommitmentPreimage): ByteArray =
        hash(encodeNoteCommitmentPreimage(value))

    @JvmStatic
    fun deriveInputNullifier(value: InputNullifierPreimage): ByteArray =
        hash(encodeInputNullifierPreimage(value))

    @JvmStatic
    fun derivePaymentTokenId(value: PaymentTokenIdPreimage): ByteArray =
        hash(encodePaymentTokenIdPreimage(value))

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

    private fun encodeInstructionWrapper(schema: String, modelPayload: ByteArray): ByteArray =
        NoritoCodec.encode(modelPayload, schema, InstructionWrapperAdapter, 0)

    private fun <T> decodeInstructionModel(
        bytes: ByteArray,
        instructionSchema: String,
        modelSchema: String,
        modelAdapter: TypeAdapter<T>,
    ): T {
        val wirePayload = extractInstructionWirePayload(bytes, instructionSchema)
        val modelPayload = NoritoCodec.decode(
            wirePayload,
            InstructionWrapperPayloadAdapter,
            instructionSchema,
        )
        return decodeModelPayload(modelPayload.bytes, modelSchema, modelAdapter, modelPayload.flags)
    }

    private fun extractInstructionWirePayload(bytes: ByteArray, expectedWireName: String): ByteArray {
        if (isNoritoFrame(bytes)) return bytes.copyOf()
        tryDecodeInstructionPair(bytes, expectedWireName, NoritoHeader.COMPACT_LEN)?.let { return it }
        tryDecodeInstructionPair(bytes, expectedWireName, 0)?.let { return it }
        throw IllegalArgumentException("Offline Note instruction envelope is invalid")
    }

    private fun tryDecodeInstructionPair(
        bytes: ByteArray,
        expectedWireName: String,
        flags: Int,
    ): ByteArray? = try {
        val decoder = NoritoDecoder(bytes, flags)
        val wireName = readField(decoder) { readString(it) }
        require(wireName == expectedWireName) {
            "Offline Note instruction wire name mismatch: $wireName"
        }
        val wirePayload = readField(decoder) { readBytesVec(it) }
        require(decoder.remaining() == 0) { "Trailing bytes after instruction envelope" }
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
                require(decoder.remaining() == 0) { "Trailing bytes after Offline Note model decode" }
                return value
            } catch (ex: RuntimeException) {
                lastError = ex
            }
        }
        throw IllegalArgumentException("Offline Note instruction model payload is invalid", lastError)
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
        val backend: String
        val name: String

        init {
            val normalizedBackend = backend.trim()
            val normalizedName = name.trim()
            require(normalizedBackend.isNotEmpty()) { "verifying key backend must not be empty" }
            require(normalizedName.isNotEmpty()) { "verifying key name must not be empty" }
            require(!normalizedBackend.contains(':') && !normalizedName.contains(':')) {
                "verifying key backend and name must not contain ':'"
            }
            this.backend = normalizedBackend
            this.name = normalizedName
        }
    }

    class ProofBox(backend: String, bytes: ByteArray) {
        val backend: String
        private val _bytes = bytes.copyOf()

        init {
            val normalizedBackend = backend.trim()
            require(normalizedBackend.isNotEmpty()) { "proof backend must not be empty" }
            require(_bytes.isNotEmpty()) { "proof bytes must not be empty" }
            this.backend = normalizedBackend
        }

        fun bytes(): ByteArray = _bytes.copyOf()
    }

    class RecursiveProof @JvmOverloads constructor(
        val verifierKeyId: VerifyingKeyIdReference = VerifyingKeyIdReference(),
        publicInputsHash: ByteArray,
        val proof: ProofBox,
    ) {
        private val _publicInputsHash = publicInputsHash.copyOf()

        init {
            requireHash(_publicInputsHash, "public_inputs_hash")
        }

        fun publicInputsHash(): ByteArray = _publicInputsHash.copyOf()

        fun validateCanonicalMetadata() {
            require(
                verifierKeyId.backend == RECURSIVE_BACKEND &&
                    verifierKeyId.name == RECURSIVE_VERIFIER_NAME
            ) {
                "recursive proof verifier key must be $RECURSIVE_BACKEND:$RECURSIVE_VERIFIER_NAME"
            }
            require(proof.backend == RECURSIVE_BACKEND) {
                "recursive proof backend must be $RECURSIVE_BACKEND"
            }
        }
    }

    class KeyCertificatePayload @JvmOverloads constructor(
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
            requireCertificateCore(version, accountId, _publicKey, assertionUsageCountLimit, oneUse)
        }

        fun publicKey(): ByteArray = _publicKey.copyOf()
        fun assertionPublicKey(): ByteArray = _assertionPublicKey.copyOf()
        fun noritoEncoded(): ByteArray = encodeCertificatePayload(this)
        fun payloadHash(): ByteArray = hash(noritoEncoded())
    }

    class KeyCertificate @JvmOverloads constructor(
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
            requireCertificateCore(version, accountId, _publicKey, assertionUsageCountLimit, oneUse)
            require(_issuerSignature.size == 64) { "issuer signature must be 64 bytes" }
        }

        fun publicKey(): ByteArray = _publicKey.copyOf()
        fun assertionPublicKey(): ByteArray = _assertionPublicKey.copyOf()
        fun issuerSignature(): ByteArray = _issuerSignature.copyOf()

        fun signingPayload(): KeyCertificatePayload = KeyCertificatePayload(
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

    sealed class CommitmentOrigin {
        class IssuerLoad(
            val operationId: String,
            val lineageId: String,
            val localRevision: Long,
        ) : CommitmentOrigin() {
            init {
                require(operationId.trim().isNotEmpty()) { "operation_id must not be empty" }
                require(lineageId.trim().isNotEmpty()) { "lineage_id must not be empty" }
                require(localRevision >= 0) { "local_revision must be non-negative" }
            }
        }

        class P2pOutput(
            val paymentRequestId: String,
            val outputIndex: Int,
        ) : CommitmentOrigin() {
            init {
                require(paymentRequestId.trim().isNotEmpty()) { "payment_request_id must not be empty" }
                require(outputIndex >= 0) { "output_index must be non-negative" }
            }
        }
    }

    class NoteCommitmentPreimage @JvmOverloads constructor(
        val domain: String = NOTE_COMMITMENT_DOMAIN,
        val chainId: String,
        ownerKeyCertificatePayloadHash: ByteArray,
        val assetId: String,
        val amount: String,
        noteSecret: ByteArray,
        val origin: CommitmentOrigin,
    ) {
        private val _ownerKeyCertificatePayloadHash = ownerKeyCertificatePayloadHash.copyOf()
        private val _noteSecret = noteSecret.copyOf()
        val canonicalAmount: String = parseNumeric(amount).canonicalString

        init {
            require(domain == NOTE_COMMITMENT_DOMAIN) { "unsupported note commitment domain" }
            require(chainId.trim().isNotEmpty()) { "chain_id must not be empty" }
            requireHash(_ownerKeyCertificatePayloadHash, "owner_key_certificate_payload_hash")
            parseAssetId(assetId)
            requireRandomBytes(_noteSecret, "note_secret")
        }

        fun ownerKeyCertificatePayloadHash(): ByteArray = _ownerKeyCertificatePayloadHash.copyOf()
        fun noteSecret(): ByteArray = _noteSecret.copyOf()
        fun noritoEncoded(): ByteArray = encodeNoteCommitmentPreimage(this)
        fun deriveNoteCommitment(): ByteArray = OfflineNote.deriveNoteCommitment(this)
    }

    class InputNullifierPreimage @JvmOverloads constructor(
        val domain: String = INPUT_NULLIFIER_DOMAIN,
        val chainId: String,
        sourceNoteCommitment: ByteArray,
        ownerKeyCertificatePayloadHash: ByteArray,
        noteSecret: ByteArray,
    ) {
        private val _sourceNoteCommitment = sourceNoteCommitment.copyOf()
        private val _ownerKeyCertificatePayloadHash = ownerKeyCertificatePayloadHash.copyOf()
        private val _noteSecret = noteSecret.copyOf()

        init {
            require(domain == INPUT_NULLIFIER_DOMAIN) { "unsupported input nullifier domain" }
            require(chainId.trim().isNotEmpty()) { "chain_id must not be empty" }
            requireHash(_sourceNoteCommitment, "source_note_commitment")
            requireHash(_ownerKeyCertificatePayloadHash, "owner_key_certificate_payload_hash")
            requireRandomBytes(_noteSecret, "note_secret")
        }

        fun sourceNoteCommitment(): ByteArray = _sourceNoteCommitment.copyOf()
        fun ownerKeyCertificatePayloadHash(): ByteArray = _ownerKeyCertificatePayloadHash.copyOf()
        fun noteSecret(): ByteArray = _noteSecret.copyOf()
        fun noritoEncoded(): ByteArray = encodeInputNullifierPreimage(this)
        fun deriveInputNullifier(): ByteArray = OfflineNote.deriveInputNullifier(this)
    }

    class PaymentTokenIdPreimage @JvmOverloads constructor(
        val domain: String = PAYMENT_TOKEN_ID_DOMAIN,
        val chainId: String,
        val paymentRequestId: String,
        val createdAtMs: Long,
        tokenNonce: ByteArray,
        senderKeyCertificatePayloadHash: ByteArray,
        inputNullifiers: List<ByteArray>,
        outputCommitments: List<ByteArray>,
    ) {
        private val _tokenNonce = tokenNonce.copyOf()
        private val _senderKeyCertificatePayloadHash = senderKeyCertificatePayloadHash.copyOf()
        private val _inputNullifiers = inputNullifiers.map { it.copyOf() }
        private val _outputCommitments = outputCommitments.map { it.copyOf() }

        init {
            require(domain == PAYMENT_TOKEN_ID_DOMAIN) { "unsupported payment token id domain" }
            require(chainId.trim().isNotEmpty()) { "chain_id must not be empty" }
            require(paymentRequestId.trim().isNotEmpty()) { "payment_request_id must not be empty" }
            requireRandomBytes(_tokenNonce, "token_nonce")
            requireHash(_senderKeyCertificatePayloadHash, "sender_key_certificate_payload_hash")
            requireHashes(_inputNullifiers, "input_nullifiers")
            requireHashes(_outputCommitments, "output_commitments")
        }

        fun tokenNonce(): ByteArray = _tokenNonce.copyOf()
        fun senderKeyCertificatePayloadHash(): ByteArray = _senderKeyCertificatePayloadHash.copyOf()
        fun inputNullifiers(): List<ByteArray> = _inputNullifiers.map { it.copyOf() }
        fun outputCommitments(): List<ByteArray> = _outputCommitments.map { it.copyOf() }
        fun noritoEncoded(): ByteArray = encodePaymentTokenIdPreimage(this)
        fun derivePaymentTokenId(): ByteArray = OfflineNote.derivePaymentTokenId(this)
    }

    class Issue(
        noteCommitment: ByteArray,
        val keyCertificate: KeyCertificate,
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
        fun issuedClaim(): IssuedClaim = IssuedClaim(
            noteCommitment = noteCommitment(),
            keyCertificatePayloadHash = keyCertificate.payloadHash(),
            assetId = assetId,
            amount = canonicalAmount,
        )
        fun noritoEncoded(): ByteArray = encodeIssue(this)
    }

    class IssuedClaim @JvmOverloads constructor(
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
            requireHash(_noteCommitment, "note_commitment")
            requireHash(_keyCertificatePayloadHash, "key_certificate_payload_hash")
            parseAssetId(assetId)
        }

        fun noteCommitment(): ByteArray = _noteCommitment.copyOf()
        fun keyCertificatePayloadHash(): ByteArray = _keyCertificatePayloadHash.copyOf()
        fun noritoEncoded(): ByteArray = encodeIssuedClaim(this)
        fun claimHash(): ByteArray = hash(noritoEncoded())
    }

    class AuditOutputClaim(
        noteCommitment: ByteArray,
        val keyCertificate: KeyCertificate,
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
        fun issuedClaim(): IssuedClaim = IssuedClaim(
            noteCommitment = noteCommitment(),
            keyCertificatePayloadHash = keyCertificate.payloadHash(),
            assetId = assetId,
            amount = canonicalAmount,
        )
    }

    class RedeemPublicInputs @JvmOverloads constructor(
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

    class Redeem(
        sourceNoteCommitment: ByteArray,
        inputNullifiers: List<ByteArray>,
        val senderKeyCertificate: KeyCertificate,
        val recipient: String,
        val assetId: String,
        val amount: String,
        val recursiveProof: RecursiveProof,
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
        fun publicInputs(): RedeemPublicInputs = RedeemPublicInputs(
            sourceNoteCommitment = sourceNoteCommitment(),
            inputNullifiers = inputNullifiers(),
            keyCertificatePayloadHash = senderKeyCertificate.payloadHash(),
            recipient = recipient,
            assetId = assetId,
            amount = canonicalAmount,
        )
        fun publicInputsHash(): ByteArray = publicInputs().publicInputsHash()
        fun validateProofBinding() {
            recursiveProof.validateCanonicalMetadata()
            require(recursiveProof.publicInputsHash().contentEquals(publicInputsHash())) {
                "recursive proof public inputs hash mismatch"
            }
        }

        fun replacingRecursiveProof(recursiveProof: RecursiveProof): Redeem = Redeem(
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

    class AuditPublicInputs @JvmOverloads constructor(
        val domain: String = AUDIT_PUBLIC_INPUTS_DOMAIN,
        tokenId: ByteArray,
        keyCertificatePayloadHash: ByteArray,
        inputNullifiers: List<ByteArray>,
        val inputClaims: List<IssuedClaim>,
        outputCommitments: List<ByteArray>,
        val outputClaims: List<IssuedClaim>,
    ) {
        private val _tokenId = tokenId.copyOf()
        private val _keyCertificatePayloadHash = keyCertificatePayloadHash.copyOf()
        private val _inputNullifiers = inputNullifiers.map { it.copyOf() }
        private val _outputCommitments = outputCommitments.map { it.copyOf() }

        init {
            requireHash(_tokenId, "token_id")
            requireHash(_keyCertificatePayloadHash, "key_certificate_payload_hash")
            requireHashes(_inputNullifiers, "input_nullifiers")
            require(inputClaims.isNotEmpty()) { "input claims must not be empty" }
            require(inputClaims.size == _inputNullifiers.size) {
                "input nullifier count must match input claim count"
            }
            requireHashes(_outputCommitments, "output_commitments")
            require(outputClaims.isNotEmpty()) { "output claims must not be empty" }
            require(outputClaims.size == _outputCommitments.size) {
                "output claim count must match output commitment count"
            }
            for ((commitment, claim) in _outputCommitments.zip(outputClaims)) {
                require(claim.noteCommitment().contentEquals(commitment)) {
                    "audit output claims must be ordered one-to-one with output commitments"
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

    class AuditBundle(
        tokenId: ByteArray,
        val senderKeyCertificate: KeyCertificate,
        inputNullifiers: List<ByteArray>,
        val inputClaims: List<IssuedClaim>,
        outputCommitments: List<ByteArray>,
        val outputClaims: List<AuditOutputClaim>,
        val recursiveProof: RecursiveProof,
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
            require(outputClaims.size == _outputCommitments.size) {
                "output claim count must match output commitment count"
            }
            for ((commitment, claim) in _outputCommitments.zip(outputClaims)) {
                require(claim.noteCommitment().contentEquals(commitment)) {
                    "audit output claims must be ordered one-to-one with output commitments"
                }
            }
        }

        fun tokenId(): ByteArray = _tokenId.copyOf()
        fun inputNullifiers(): List<ByteArray> = _inputNullifiers.map { it.copyOf() }
        fun outputCommitments(): List<ByteArray> = _outputCommitments.map { it.copyOf() }
        fun outputClaimForNoteCommitment(noteCommitment: ByteArray): AuditOutputClaim? =
            outputClaims.firstOrNull { it.noteCommitment().contentEquals(noteCommitment) }
        fun containsOutputNoteCommitment(noteCommitment: ByteArray): Boolean =
            outputClaimForNoteCommitment(noteCommitment) != null
        fun publicInputs(): AuditPublicInputs = AuditPublicInputs(
            tokenId = tokenId(),
            keyCertificatePayloadHash = senderKeyCertificate.payloadHash(),
            inputNullifiers = inputNullifiers(),
            inputClaims = inputClaims,
            outputCommitments = outputCommitments(),
            outputClaims = outputClaims.map { it.issuedClaim() },
        )
        fun publicInputsHash(): ByteArray = publicInputs().publicInputsHash()
        fun validateProofBinding() {
            recursiveProof.validateCanonicalMetadata()
            require(recursiveProof.publicInputsHash().contentEquals(publicInputsHash())) {
                "recursive proof public inputs hash mismatch"
            }
        }

        fun replacingRecursiveProof(recursiveProof: RecursiveProof): AuditBundle = AuditBundle(
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
                "Offline public instance count must be $PUBLIC_VALUE_COUNT"
            }
            require(_inputAmounts.size == MAX_INPUT_AMOUNTS) {
                "Offline input amount witness count must be $MAX_INPUT_AMOUNTS"
            }
            require(_outputAmounts.size == MAX_OUTPUT_AMOUNTS) {
                "Offline output amount witness count must be $MAX_OUTPUT_AMOUNTS"
            }
        }

        fun publicValues(): LongArray = _publicValues.copyOf()
        fun inputAmounts(): LongArray = _inputAmounts.copyOf()
        fun outputAmounts(): LongArray = _outputAmounts.copyOf()
        fun publicInstanceColumns(): List<ByteArray> = _publicValues.map { instanceScalarBytes(it) }
    }

    object InstanceBuilder {
        @JvmStatic
        fun redeemInstanceValues(redemption: Redeem): InstanceValues {
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
            val issuedClaimHash = IssuedClaim(
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
        fun auditInstanceValues(audit: AuditBundle): InstanceValues {
            val inputCount = validateCount(audit.inputClaims.size, MAX_INPUT_AMOUNTS, "audit input")
            val outputCount = validateCount(audit.outputClaims.size, MAX_OUTPUT_AMOUNTS, "audit output")
            require(audit.inputNullifiers().size == audit.inputClaims.size) {
                "audit input nullifier count must match input claim count"
            }
            require(audit.outputCommitments().size == audit.outputClaims.size) {
                "audit output claim count must match output commitment count"
            }
            for ((commitment, claim) in audit.outputCommitments().zip(audit.outputClaims)) {
                require(claim.noteCommitment().contentEquals(commitment)) {
                    "audit output claims must be ordered one-to-one with output commitments"
                }
            }
            val senderCertificateHash = audit.senderKeyCertificate.payloadHash()
            require(audit.inputClaims.all { it.keyCertificatePayloadHash().contentEquals(senderCertificateHash) }) {
                "audit input claims must match sender key certificate"
            }
            val inputDefinition = parseAssetId(audit.inputClaims.first().assetId).definitionBytes
            val inputAssetsMatch = audit.inputClaims.all {
                parseAssetId(it.assetId).definitionBytes.contentEquals(inputDefinition)
            }
            val outputAssetsMatch = audit.outputClaims.all {
                parseAssetId(it.assetId).definitionBytes.contentEquals(inputDefinition)
            }
            require(inputAssetsMatch && outputAssetsMatch) {
                "audit input and output asset definitions must match"
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
                "Offline audit amounts are not conserved"
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

    private object InstructionWrapperAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            writeField(encoder) { it.writeBytes(value) }
        }

        override fun decode(decoder: NoritoDecoder): ByteArray =
            readField(decoder) { it.readBytes(it.remaining()) }
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

    private object KeyCertificatePayloadAdapter : TypeAdapter<KeyCertificatePayload> {
        override fun encode(encoder: NoritoEncoder, value: KeyCertificatePayload) {
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

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): KeyCertificatePayload =
            KeyCertificatePayload(
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

    private object KeyCertificateAdapter : TypeAdapter<KeyCertificate> {
        override fun encode(encoder: NoritoEncoder, value: KeyCertificate) {
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

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): KeyCertificate =
            KeyCertificate(
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

    private object RecursiveProofAdapter : TypeAdapter<RecursiveProof> {
        override fun encode(encoder: NoritoEncoder, value: RecursiveProof) {
            writeField(encoder) { writeVerifyingKeyId(it, value.verifierKeyId) }
            writeField(encoder) { it.writeBytes(value.publicInputsHash()) }
            writeField(encoder) { writeProofBox(it, value.proof) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): RecursiveProof =
            RecursiveProof(
                verifierKeyId = readField(decoder) { readVerifyingKeyId(it) },
                publicInputsHash = readField(decoder) { readHash(it, "public_inputs_hash") },
                proof = readField(decoder) { readProofBox(it) },
            )
    }

    private object IssueAdapter : TypeAdapter<Issue> {
        override fun encode(encoder: NoritoEncoder, value: Issue) {
            writeField(encoder) { it.writeBytes(value.noteCommitment()) }
            writeField(encoder) { KeyCertificateAdapter.encode(it, value.keyCertificate) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): Issue =
            Issue(
                noteCommitment = readField(decoder) { readHash(it, "note_commitment") },
                keyCertificate = readField(decoder) { KeyCertificateAdapter.decode(it) },
                assetId = readField(decoder) { readAssetId(it) },
                amount = readField(decoder) { readNumeric(it) },
            )
    }

    private object IssuedClaimAdapter : TypeAdapter<IssuedClaim> {
        override fun encode(encoder: NoritoEncoder, value: IssuedClaim) {
            writeField(encoder) { writeString(it, value.domain) }
            writeField(encoder) { it.writeBytes(value.noteCommitment()) }
            writeField(encoder) { it.writeBytes(value.keyCertificatePayloadHash()) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): IssuedClaim =
            IssuedClaim(
                domain = readField(decoder) { readString(it) },
                noteCommitment = readField(decoder) { readHash(it, "note_commitment") },
                keyCertificatePayloadHash = readField(decoder) { readHash(it, "key_certificate_payload_hash") },
                assetId = readField(decoder) { readAssetId(it) },
                amount = readField(decoder) { readNumeric(it) },
            )
    }

    private object AuditOutputClaimAdapter : TypeAdapter<AuditOutputClaim> {
        override fun encode(encoder: NoritoEncoder, value: AuditOutputClaim) {
            writeField(encoder) { it.writeBytes(value.noteCommitment()) }
            writeField(encoder) { KeyCertificateAdapter.encode(it, value.keyCertificate) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): AuditOutputClaim =
            AuditOutputClaim(
                noteCommitment = readField(decoder) { readHash(it, "note_commitment") },
                keyCertificate = readField(decoder) { KeyCertificateAdapter.decode(it) },
                assetId = readField(decoder) { readAssetId(it) },
                amount = readField(decoder) { readNumeric(it) },
            )
    }

    private object RedeemPublicInputsAdapter : TypeAdapter<RedeemPublicInputs> {
        override fun encode(encoder: NoritoEncoder, value: RedeemPublicInputs) {
            writeField(encoder) { writeString(it, value.domain) }
            writeField(encoder) { it.writeBytes(value.sourceNoteCommitment()) }
            writeField(encoder) { writeVec(it, value.inputNullifiers()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { it.writeBytes(value.keyCertificatePayloadHash()) }
            writeField(encoder) { writeAccountId(it, value.recipient) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): RedeemPublicInputs =
            RedeemPublicInputs(
                domain = readField(decoder) { readString(it) },
                sourceNoteCommitment = readField(decoder) { readHash(it, "source_note_commitment") },
                inputNullifiers = readField(decoder) { readVec(it) { child -> readHash(child, "input_nullifier") } },
                keyCertificatePayloadHash = readField(decoder) { readHash(it, "key_certificate_payload_hash") },
                recipient = readField(decoder) { readAccountId(it) },
                assetId = readField(decoder) { readAssetId(it) },
                amount = readField(decoder) { readNumeric(it) },
            )
    }

    private object RedeemAdapter : TypeAdapter<Redeem> {
        override fun encode(encoder: NoritoEncoder, value: Redeem) {
            writeField(encoder) { it.writeBytes(value.sourceNoteCommitment()) }
            writeField(encoder) { writeVec(it, value.inputNullifiers()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { KeyCertificateAdapter.encode(it, value.senderKeyCertificate) }
            writeField(encoder) { writeAccountId(it, value.recipient) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
            writeField(encoder) { RecursiveProofAdapter.encode(it, value.recursiveProof) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): Redeem =
            Redeem(
                sourceNoteCommitment = readField(decoder) { readHash(it, "source_note_commitment") },
                inputNullifiers = readField(decoder) { readVec(it) { child -> readHash(child, "input_nullifier") } },
                senderKeyCertificate = readField(decoder) { KeyCertificateAdapter.decode(it) },
                recipient = readField(decoder) { readAccountId(it) },
                assetId = readField(decoder) { readAssetId(it) },
                amount = readField(decoder) { readNumeric(it) },
                recursiveProof = readField(decoder) { RecursiveProofAdapter.decode(it) },
            )
    }

    private object AuditPublicInputsAdapter : TypeAdapter<AuditPublicInputs> {
        override fun encode(encoder: NoritoEncoder, value: AuditPublicInputs) {
            writeField(encoder) { writeString(it, value.domain) }
            writeField(encoder) { it.writeBytes(value.tokenId()) }
            writeField(encoder) { it.writeBytes(value.keyCertificatePayloadHash()) }
            writeField(encoder) { writeVec(it, value.inputNullifiers()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { writeVec(it, value.inputClaims) { out, claim -> IssuedClaimAdapter.encode(out, claim) } }
            writeField(encoder) { writeVec(it, value.outputCommitments()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { writeVec(it, value.outputClaims) { out, claim -> IssuedClaimAdapter.encode(out, claim) } }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): AuditPublicInputs =
            AuditPublicInputs(
                domain = readField(decoder) { readString(it) },
                tokenId = readField(decoder) { readHash(it, "token_id") },
                keyCertificatePayloadHash = readField(decoder) { readHash(it, "key_certificate_payload_hash") },
                inputNullifiers = readField(decoder) { readVec(it) { child -> readHash(child, "input_nullifier") } },
                inputClaims = readField(decoder) { readVec(it) { child -> IssuedClaimAdapter.decode(child) } },
                outputCommitments = readField(decoder) { readVec(it) { child -> readHash(child, "output_commitment") } },
                outputClaims = readField(decoder) { readVec(it) { child -> IssuedClaimAdapter.decode(child) } },
            )
    }

    private object AuditAdapter : TypeAdapter<AuditBundle> {
        override fun encode(encoder: NoritoEncoder, value: AuditBundle) {
            writeField(encoder) { it.writeBytes(value.tokenId()) }
            writeField(encoder) { KeyCertificateAdapter.encode(it, value.senderKeyCertificate) }
            writeField(encoder) { writeVec(it, value.inputNullifiers()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { writeVec(it, value.inputClaims) { out, claim -> IssuedClaimAdapter.encode(out, claim) } }
            writeField(encoder) { writeVec(it, value.outputCommitments()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { writeVec(it, value.outputClaims) { out, claim -> AuditOutputClaimAdapter.encode(out, claim) } }
            writeField(encoder) { RecursiveProofAdapter.encode(it, value.recursiveProof) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): AuditBundle =
            AuditBundle(
                tokenId = readField(decoder) { readHash(it, "token_id") },
                senderKeyCertificate = readField(decoder) { KeyCertificateAdapter.decode(it) },
                inputNullifiers = readField(decoder) { readVec(it) { child -> readHash(child, "input_nullifier") } },
                inputClaims = readField(decoder) { readVec(it) { child -> IssuedClaimAdapter.decode(child) } },
                outputCommitments = readField(decoder) { readVec(it) { child -> readHash(child, "output_commitment") } },
                outputClaims = readField(decoder) { readVec(it) { child -> AuditOutputClaimAdapter.decode(child) } },
                recursiveProof = readField(decoder) { RecursiveProofAdapter.decode(it) },
            )
    }

    private object NoteCommitmentPreimageAdapter : TypeAdapter<NoteCommitmentPreimage> {
        override fun encode(encoder: NoritoEncoder, value: NoteCommitmentPreimage) {
            writeField(encoder) { writeString(it, value.domain) }
            writeField(encoder) { writeChainId(it, value.chainId) }
            writeField(encoder) { it.writeBytes(value.ownerKeyCertificatePayloadHash()) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
            writeField(encoder) { writeBytesVec(it, value.noteSecret()) }
            writeField(encoder) { writeCommitmentOrigin(it, value.origin) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): NoteCommitmentPreimage =
            NoteCommitmentPreimage(
                domain = readField(decoder) { readString(it) },
                chainId = readField(decoder) { readChainId(it) },
                ownerKeyCertificatePayloadHash = readField(decoder) {
                    readHash(it, "owner_key_certificate_payload_hash")
                },
                assetId = readField(decoder) { readAssetId(it) },
                amount = readField(decoder) { readNumeric(it) },
                noteSecret = readField(decoder) { readBytesVec(it) },
                origin = readField(decoder) { readCommitmentOrigin(it) },
            )
    }

    private object InputNullifierPreimageAdapter : TypeAdapter<InputNullifierPreimage> {
        override fun encode(encoder: NoritoEncoder, value: InputNullifierPreimage) {
            writeField(encoder) { writeString(it, value.domain) }
            writeField(encoder) { writeChainId(it, value.chainId) }
            writeField(encoder) { it.writeBytes(value.sourceNoteCommitment()) }
            writeField(encoder) { it.writeBytes(value.ownerKeyCertificatePayloadHash()) }
            writeField(encoder) { writeBytesVec(it, value.noteSecret()) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): InputNullifierPreimage =
            InputNullifierPreimage(
                domain = readField(decoder) { readString(it) },
                chainId = readField(decoder) { readChainId(it) },
                sourceNoteCommitment = readField(decoder) { readHash(it, "source_note_commitment") },
                ownerKeyCertificatePayloadHash = readField(decoder) {
                    readHash(it, "owner_key_certificate_payload_hash")
                },
                noteSecret = readField(decoder) { readBytesVec(it) },
            )
    }

    private object PaymentTokenIdPreimageAdapter : TypeAdapter<PaymentTokenIdPreimage> {
        override fun encode(encoder: NoritoEncoder, value: PaymentTokenIdPreimage) {
            writeField(encoder) { writeString(it, value.domain) }
            writeField(encoder) { writeChainId(it, value.chainId) }
            writeField(encoder) { writeString(it, value.paymentRequestId) }
            writeField(encoder) { it.writeUInt(value.createdAtMs, 64) }
            writeField(encoder) { writeBytesVec(it, value.tokenNonce()) }
            writeField(encoder) { it.writeBytes(value.senderKeyCertificatePayloadHash()) }
            writeField(encoder) { writeVec(it, value.inputNullifiers()) { out, bytes -> out.writeBytes(bytes) } }
            writeField(encoder) { writeVec(it, value.outputCommitments()) { out, bytes -> out.writeBytes(bytes) } }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): PaymentTokenIdPreimage =
            PaymentTokenIdPreimage(
                domain = readField(decoder) { readString(it) },
                chainId = readField(decoder) { readChainId(it) },
                paymentRequestId = readField(decoder) { readString(it) },
                createdAtMs = readField(decoder) { it.readUInt(64) },
                tokenNonce = readField(decoder) { readBytesVec(it) },
                senderKeyCertificatePayloadHash = readField(decoder) {
                    readHash(it, "sender_key_certificate_payload_hash")
                },
                inputNullifiers = readField(decoder) { readVec(it) { child -> readHash(child, "input_nullifier") } },
                outputCommitments = readField(decoder) { readVec(it) { child -> readHash(child, "output_commitment") } },
            )
    }

    private fun <T> readField(parent: NoritoDecoder, readPayload: (NoritoDecoder) -> T): T {
        val length = checkedLength(parent.readLength(compact(parent)), "field length")
        val child = NoritoDecoder(parent.readBytes(length), parent.flags, parent.flagsHint)
        val value = readPayload(child)
        require(child.remaining() == 0) { "Trailing bytes after Offline Note field decode" }
        return value
    }

    private fun readString(decoder: NoritoDecoder): String {
        val length = checkedLength(decoder.readLength(compact(decoder)), "string length")
        val bytes = decoder.readBytes(length)
        return String(bytes, StandardCharsets.UTF_8)
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

    private fun readChainId(decoder: NoritoDecoder): String =
        readField(decoder) { readString(it) }

    private fun readCommitmentOrigin(decoder: NoritoDecoder): CommitmentOrigin =
        when (val tag = decoder.readUInt(32)) {
            0L -> readField(decoder) { payload ->
                CommitmentOrigin.IssuerLoad(
                    operationId = readField(payload) { readString(it) },
                    lineageId = readField(payload) { readString(it) },
                    localRevision = readField(payload) { it.readUInt(64) },
                )
            }
            1L -> readField(decoder) { payload ->
                CommitmentOrigin.P2pOutput(
                    paymentRequestId = readField(payload) { readString(it) },
                    outputIndex = readField(payload) { it.readUInt(32).toInt() },
                )
            }
            else -> throw IllegalArgumentException("unsupported commitment origin tag: $tag")
        }

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
                val policy = readMultisigPolicy(payload)
                try {
                    AccountAddress.fromMultisigPolicy(policy).toI105Default()
                } catch (ex: AccountAddressException) {
                    throw IllegalArgumentException("invalid decoded multisig account id", ex)
                }
            }
            else -> throw IllegalArgumentException("unsupported account controller tag: $tag")
        }

    private fun readPublicKeyPayload(decoder: NoritoDecoder): PublicKeyPayload {
        val payload = readConstVec(decoder)
        require(payload.isNotEmpty()) { "public key payload must not be empty" }
        val curveId = compactTagToCurveId(payload[0].toInt() and 0xFF)
        return PublicKeyPayload(curveId, payload.copyOfRange(1, payload.size))
    }

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
        val mantissa = bigIntegerFromLittleEndianTwosComplement(mantissaBytes)
        return canonicalNumericString(mantissa, scale)
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

    private fun compactTagToCurveId(tag: Int): Int =
        when (tag) {
            0 -> 0x01
            1 -> 0x04
            2 -> 0x03
            3 -> 0x05
            4 -> 0x02
            5 -> 0x0A
            6 -> 0x0B
            7 -> 0x0C
            8 -> 0x0D
            9 -> 0x0E
            10 -> 0x0F
            else -> throw IllegalArgumentException("unsupported public key compact tag: $tag")
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

    private fun writeCommitmentOrigin(encoder: NoritoEncoder, origin: CommitmentOrigin) {
        when (origin) {
            is CommitmentOrigin.IssuerLoad -> {
                encoder.writeUInt(0, 32)
                writeField(encoder) { payload ->
                    writeField(payload) { writeString(it, origin.operationId) }
                    writeField(payload) { writeString(it, origin.lineageId) }
                    writeField(payload) { it.writeUInt(origin.localRevision, 64) }
                }
            }
            is CommitmentOrigin.P2pOutput -> {
                encoder.writeUInt(1, 32)
                writeField(encoder) { payload ->
                    writeField(payload) { writeString(it, origin.paymentRequestId) }
                    writeField(payload) { it.writeUInt(origin.outputIndex.toLong(), 32) }
                }
            }
        }
    }

    private fun writeAccountId(encoder: NoritoEncoder, accountId: String) {
        encoder.writeBytes(encodeAccountIdPayload(accountId))
    }

    private fun writeChainId(encoder: NoritoEncoder, chainId: String) {
        writeField(encoder) { writeString(it, chainId) }
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
        require(policy.version == MULTISIG_POLICY_VERSION) { "unsupported multisig policy version" }
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
        writeConstVec(encoder, publicKeyCompactPayload(curveId, publicKey))
    }

    private fun publicKeyCompactPayload(curveId: Int, publicKey: ByteArray): ByteArray {
        val tag = when (curveId) {
            0x01 -> 0
            0x04 -> 1
            0x03 -> 2
            0x05 -> 3
            0x02 -> 4
            0x0A -> 5
            0x0B -> 6
            0x0C -> 7
            0x0D -> 8
            0x0E -> 9
            0x0F -> 10
            else -> throw IllegalArgumentException("Unsupported curve id: $curveId")
        }
        val bytes = ByteArray(1 + publicKey.size)
        bytes[0] = tag.toByte()
        System.arraycopy(publicKey, 0, bytes, 1, publicKey.size)
        return bytes
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
            "Offline $label count $count must be in 1..$max"
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
                "Offline amount ${numeric.original} must not be negative"
            }
            val scaleDelta = targetScale - numeric.scale
            val aligned = numeric.mantissa.multiply(BigInteger.TEN.pow(scaleDelta))
            require(aligned.bitLength() <= 64) {
                "Offline amount ${numeric.original} does not fit the u64 witness corridor"
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
                "Offline $label amount sum overflows u64 witness units"
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

    private fun requireCertificateCore(
        version: Int,
        accountId: String,
        publicKey: ByteArray,
        assertionUsageCountLimit: Int?,
        oneUse: Boolean,
    ) {
        require(version == KEY_CERTIFICATE_VERSION) { "Offline Note key certificate format is unsupported" }
        require(oneUse && (assertionUsageCountLimit == null || assertionUsageCountLimit == 1)) {
            "Offline Note key certificate must be one-use with usage limit 1 when present"
        }
        require(publicKey.size == 32) { "Offline Note note public key must be 32 bytes" }
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

    private fun requireRandomBytes(value: ByteArray, field: String) {
        require(value.size == 32) { "$field must be exactly 32 bytes" }
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
