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
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Native JVM implementation of Iroha Offline Note V2 canonical Norito encodings. */
object OfflineNoteV2 {
    const val KEY_CERTIFICATE_PAYLOAD_DOMAIN: String =
        "iroha:offline-note-v2:key-certificate-payload:v1"
    const val ISSUED_CLAIM_DOMAIN: String = "iroha:offline-note-v2:issued-claim:v1"
    const val REDEEM_PUBLIC_INPUTS_DOMAIN: String =
        "iroha:offline-note-v2:redeem-public-inputs:v1"
    const val AUDIT_PUBLIC_INPUTS_DOMAIN: String =
        "iroha:offline-note-v2:audit-public-inputs:v1"
    const val RECURSIVE_BACKEND: String = "halo2/ipa"
    const val RECURSIVE_VERIFIER_NAME: String = "offline-note-v2-recursive-v1"

    private const val MULTISIG_POLICY_VERSION_V1 = 1
    private const val MAX_NUMERIC_SCALE = 28
    private const val MAX_BIGINT_BYTES = 64

    private const val KEY_CERTIFICATE_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteKeyCertificateV2"
    private const val KEY_CERTIFICATE_PAYLOAD_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteKeyCertificatePayloadV2"
    private const val ISSUE_SCHEMA = "iroha_data_model::offline::model::OfflineNoteIssueV2"
    private const val ISSUED_CLAIM_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteIssuedClaimV2"
    private const val AUDIT_OUTPUT_CLAIM_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteAuditOutputClaimV2"
    private const val REDEEM_SCHEMA = "iroha_data_model::offline::model::OfflineNoteRedeemV2"
    private const val REDEEM_PUBLIC_INPUTS_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteRedeemPublicInputsV2"
    private const val AUDIT_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteAuditBundleV2"
    private const val AUDIT_PUBLIC_INPUTS_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteAuditPublicInputsV2"

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
    fun hash(bytes: ByteArray): ByteArray = IrohaHash.prehash(bytes)

    private fun <T> encodeWithHeader(value: T, schema: String, adapter: TypeAdapter<T>): ByteArray =
        NoritoCodec.encode(value, schema, adapter, NoritoHeader.COMPACT_LEN)

    class VerifyingKeyIdReference @JvmOverloads constructor(
        val backend: String = RECURSIVE_BACKEND,
        val name: String = RECURSIVE_VERIFIER_NAME,
    ) {
        init {
            require(backend.trim().isNotEmpty()) { "verifying key backend must not be empty" }
            require(name.trim().isNotEmpty()) { "verifying key name must not be empty" }
        }
    }

    class ProofBox(val backend: String, bytes: ByteArray) {
        private val _bytes = bytes.copyOf()

        init {
            require(backend.trim().isNotEmpty()) { "proof backend must not be empty" }
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
        val version: Int = 2,
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
        fun noritoEncoded(): ByteArray = encodeAudit(this)
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

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): KeyCertificatePayloadV2 =
            throw UnsupportedOperationException("Offline Note V2 decoding is not supported yet")
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

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): KeyCertificateV2 =
            throw UnsupportedOperationException("Offline Note V2 decoding is not supported yet")
    }

    private object RecursiveProofAdapter : TypeAdapter<RecursiveProofV2> {
        override fun encode(encoder: NoritoEncoder, value: RecursiveProofV2) {
            writeField(encoder) { writeVerifyingKeyId(it, value.verifierKeyId) }
            writeField(encoder) { it.writeBytes(value.publicInputsHash()) }
            writeField(encoder) { writeProofBox(it, value.proof) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): RecursiveProofV2 =
            throw UnsupportedOperationException("Offline Note V2 decoding is not supported yet")
    }

    private object IssueAdapter : TypeAdapter<IssueV2> {
        override fun encode(encoder: NoritoEncoder, value: IssueV2) {
            writeField(encoder) { it.writeBytes(value.noteCommitment()) }
            writeField(encoder) { KeyCertificateAdapter.encode(it, value.keyCertificate) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): IssueV2 =
            throw UnsupportedOperationException("Offline Note V2 decoding is not supported yet")
    }

    private object IssuedClaimAdapter : TypeAdapter<IssuedClaimV2> {
        override fun encode(encoder: NoritoEncoder, value: IssuedClaimV2) {
            writeField(encoder) { writeString(it, value.domain) }
            writeField(encoder) { it.writeBytes(value.noteCommitment()) }
            writeField(encoder) { it.writeBytes(value.keyCertificatePayloadHash()) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): IssuedClaimV2 =
            throw UnsupportedOperationException("Offline Note V2 decoding is not supported yet")
    }

    private object AuditOutputClaimAdapter : TypeAdapter<AuditOutputClaimV2> {
        override fun encode(encoder: NoritoEncoder, value: AuditOutputClaimV2) {
            writeField(encoder) { it.writeBytes(value.noteCommitment()) }
            writeField(encoder) { KeyCertificateAdapter.encode(it, value.keyCertificate) }
            writeField(encoder) { writeAssetId(it, value.assetId) }
            writeField(encoder) { writeNumeric(it, value.canonicalAmount) }
        }

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): AuditOutputClaimV2 =
            throw UnsupportedOperationException("Offline Note V2 decoding is not supported yet")
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

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): RedeemPublicInputsV2 =
            throw UnsupportedOperationException("Offline Note V2 decoding is not supported yet")
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

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): RedeemV2 =
            throw UnsupportedOperationException("Offline Note V2 decoding is not supported yet")
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

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): AuditPublicInputsV2 =
            throw UnsupportedOperationException("Offline Note V2 decoding is not supported yet")
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

        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): AuditBundleV2 =
            throw UnsupportedOperationException("Offline Note V2 decoding is not supported yet")
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
            writeField(encoder) { writeString(it, encodePublicKeyMultihash(single.curveId, single.publicKey)) }
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
                writeField(memberEncoder) {
                    writeString(it, encodePublicKeyMultihash(member.curveId, member.publicKey))
                }
                writeField(memberEncoder) { it.writeUInt(member.weight.toLong(), 16) }
            }
        }
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

    private fun requireCertificateCore(version: Int, accountId: String, publicKey: ByteArray, oneUse: Boolean) {
        require(version == 2) { "Offline Note V2 key certificate version must be 2" }
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
}
