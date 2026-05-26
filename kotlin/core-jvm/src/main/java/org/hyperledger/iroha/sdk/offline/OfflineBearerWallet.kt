package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.math.BigDecimal
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.Base64
import java.util.LinkedHashSet
import org.bouncycastle.asn1.ASN1Integer
import org.bouncycastle.asn1.ASN1Sequence
import org.bouncycastle.asn1.x9.ECNamedCurveTable
import org.bouncycastle.crypto.params.ECDomainParameters
import org.bouncycastle.crypto.params.ECPublicKeyParameters
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.signers.ECDSASigner
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.TypeAdapter

const val OFFLINE_BEARER_SIGNATURE_ALGORITHM_ED25519: String = "ed25519"
const val OFFLINE_BEARER_SIGNATURE_ALGORITHM_ECDSA_P256_SHA256: String = "ecdsa_p256_sha256"
const val OFFLINE_BEARER_PUBLIC_KEY_ENCODING_RAW_ED25519: String = "raw_ed25519"
const val OFFLINE_BEARER_PUBLIC_KEY_ENCODING_X963_P256: String = "x963_uncompressed_p256"

/** Capabilities reported by a hardware-backed Offline Bearer purse provider. */
class OfflineBearerSecureElementCapabilities @JvmOverloads constructor(
    val hardwareBacked: Boolean,
    val statefulPurse: Boolean,
    val hardwareClass: String,
    val attestationKeyId: String? = null,
    val signatureAlgorithm: String = OFFLINE_BEARER_SIGNATURE_ALGORITHM_ED25519,
    val publicKeyEncoding: String = OFFLINE_BEARER_PUBLIC_KEY_ENCODING_RAW_ED25519,
    val rollbackResistantState: Boolean = false,
    attestationEvidence: ByteArray = ByteArray(0),
) {
    private val attestationEvidenceBytes = attestationEvidence.copyOf()

    init {
        require(hardwareClass.trim().isNotEmpty()) { "hardwareClass must not be blank" }
        requireSupportedSignatureAlgorithm(signatureAlgorithm)
        requireSupportedPublicKeyEncoding(publicKeyEncoding)
    }

    fun attestationEvidence(): ByteArray = attestationEvidenceBytes.copyOf()
}

/** V2 certificate binding an Offline Bearer purse to an issuer-approved hardware key. */
class OfflineBearerCertificateV2(
    val certificateId: String,
    val chainId: String,
    val issuerId: String,
    val purseId: String,
    val accountId: String,
    val assetDefinitionId: String,
    val deviceId: String,
    val keyId: String,
    val hardwareClass: String,
    val signatureAlgorithm: String = OFFLINE_BEARER_SIGNATURE_ALGORITHM_ED25519,
    val publicKeyEncoding: String = OFFLINE_BEARER_PUBLIC_KEY_ENCODING_RAW_ED25519,
    publicKey: ByteArray,
    val issuedAtMs: Long,
    val expiresAtMs: Long,
    val policyId: String,
    val policyHashHex: String,
    issuerSignature: ByteArray,
) {
    private val publicKeyBytes = publicKey.copyOf()
    private val issuerSignatureBytes = issuerSignature.copyOf()

    init {
        requireNonBlank(certificateId, "certificateId")
        requireNonBlank(chainId, "chainId")
        requireNonBlank(issuerId, "issuerId")
        requireNonBlank(purseId, "purseId")
        requireNonBlank(accountId, "accountId")
        requireNonBlank(assetDefinitionId, "assetDefinitionId")
        requireNonBlank(deviceId, "deviceId")
        requireNonBlank(keyId, "keyId")
        requireNonBlank(hardwareClass, "hardwareClass")
        requireSupportedSignatureAlgorithm(signatureAlgorithm)
        requireSupportedPublicKeyEncoding(publicKeyEncoding)
        require(publicKeyBytes.isNotEmpty()) { "publicKey must not be empty" }
        require(issuedAtMs >= 0) { "issuedAtMs must be non-negative" }
        require(expiresAtMs > issuedAtMs) { "expiresAtMs must be after issuedAtMs" }
        requireNonBlank(policyId, "policyId")
        requireHexLike(policyHashHex, "policyHashHex")
        require(issuerSignatureBytes.isNotEmpty()) { "issuerSignature must not be empty" }
    }

    fun publicKey(): ByteArray = publicKeyBytes.copyOf()

    fun issuerSignature(): ByteArray = issuerSignatureBytes.copyOf()
}

/** Signed policy bundle used by correct apps to gate offline bearer acceptance. */
class OfflineBearerAssetSendLimitV2(
    val assetDefinitionId: String,
    maxTransactionAmount: String,
    dailySendLimit: String,
    monthlySendLimit: String,
) {
    val maxTransactionAmount: String = OfflineCashCodec.canonicalAmountString(maxTransactionAmount)
    val dailySendLimit: String = OfflineCashCodec.canonicalAmountString(dailySendLimit)
    val monthlySendLimit: String = OfflineCashCodec.canonicalAmountString(monthlySendLimit)

    init {
        requireNonBlank(assetDefinitionId, "assetDefinitionId")
        requirePositiveAmount(this.maxTransactionAmount, "maxTransactionAmount")
        requirePositiveAmount(this.dailySendLimit, "dailySendLimit")
        requirePositiveAmount(this.monthlySendLimit, "monthlySendLimit")
    }
}

class OfflineBearerPolicyBundleV2 @JvmOverloads constructor(
    val policyId: String,
    val policyHashHex: String,
    val issuerId: String,
    val issuedAtMs: Long,
    val expiresAtMs: Long,
    val maxCertificateAgeMs: Long = DEFAULT_MAX_CERTIFICATE_AGE_MS,
    val maxPolicyAgeMs: Long = DEFAULT_MAX_POLICY_AGE_MS,
    val maxTokenAgeMs: Long = DEFAULT_MAX_TOKEN_AGE_MS,
    maxOfflineBalance: String,
    maxTransactionAmount: String,
    allowedHardwareClasses: Collection<String>,
    blacklistedAccountIds: Collection<String> = emptyList(),
    blacklistedDeviceIds: Collection<String> = emptyList(),
    blacklistedKeyIds: Collection<String> = emptyList(),
    val signatureAlgorithm: String = OFFLINE_BEARER_SIGNATURE_ALGORITHM_ED25519,
    issuerSignature: ByteArray = byteArrayOf(1),
    val policyEpoch: Long = 0L,
    val policySource: String = "middleware",
    revokedCertificateIds: Collection<String> = emptyList(),
    revokedTransferIds: Collection<String> = emptyList(),
    assetSendLimits: Collection<OfflineBearerAssetSendLimitV2> = emptyList(),
) {
    val maxOfflineBalance: String = OfflineCashCodec.canonicalAmountString(maxOfflineBalance)
    val maxTransactionAmount: String = OfflineCashCodec.canonicalAmountString(maxTransactionAmount)
    val allowedHardwareClasses: Set<String> = normalizedSet(allowedHardwareClasses)
    val blacklistedAccountIds: Set<String> = normalizedSet(blacklistedAccountIds)
    val blacklistedDeviceIds: Set<String> = normalizedSet(blacklistedDeviceIds)
    val blacklistedKeyIds: Set<String> = normalizedSet(blacklistedKeyIds)
    val revokedCertificateIds: Set<String> = normalizedSet(revokedCertificateIds)
    val revokedTransferIds: Set<String> = normalizedSet(revokedTransferIds)
    val assetSendLimits: List<OfflineBearerAssetSendLimitV2> = assetSendLimits.toList()
    private val issuerSignatureBytes = issuerSignature.copyOf()

    init {
        requireNonBlank(policyId, "policyId")
        requireHexLike(policyHashHex, "policyHashHex")
        requireNonBlank(issuerId, "issuerId")
        require(issuedAtMs >= 0) { "issuedAtMs must be non-negative" }
        require(expiresAtMs > issuedAtMs) { "expiresAtMs must be after issuedAtMs" }
        require(maxCertificateAgeMs > 0) { "maxCertificateAgeMs must be positive" }
        require(maxPolicyAgeMs > 0) { "maxPolicyAgeMs must be positive" }
        require(maxTokenAgeMs > 0) { "maxTokenAgeMs must be positive" }
        require(policyEpoch >= 0L) { "policyEpoch must be non-negative" }
        requireNonBlank(policySource, "policySource")
        requireSupportedSignatureAlgorithm(signatureAlgorithm)
        require(this.allowedHardwareClasses.isNotEmpty()) { "allowedHardwareClasses must not be empty" }
        require(issuerSignatureBytes.isNotEmpty()) { "issuerSignature must not be empty" }
        requirePositiveAmount(this.maxOfflineBalance, "maxOfflineBalance")
        requirePositiveAmount(this.maxTransactionAmount, "maxTransactionAmount")
    }

    fun issuerSignature(): ByteArray = issuerSignatureBytes.copyOf()

    companion object {
        const val DEFAULT_MAX_CERTIFICATE_AGE_MS: Long = 24L * 60L * 60L * 1000L
        const val DEFAULT_MAX_POLICY_AGE_MS: Long = 12L * 60L * 60L * 1000L
        const val DEFAULT_MAX_TOKEN_AGE_MS: Long = 5L * 60L * 1000L
    }
}

/** Current value and sequence tracked by a stateful Offline Bearer purse. */
class OfflineBearerPurseStateV2(
    val chainId: String,
    val accountId: String,
    val assetDefinitionId: String,
    val purseId: String,
    val balance: String,
    val sequence: Long,
    val policyHashHex: String,
    val updatedAtMs: Long,
) {
    init {
        requireNonBlank(chainId, "chainId")
        requireNonBlank(accountId, "accountId")
        requireNonBlank(assetDefinitionId, "assetDefinitionId")
        requireNonBlank(purseId, "purseId")
        require(sequence >= 0) { "sequence must be non-negative" }
        requireHexLike(policyHashHex, "policyHashHex")
        require(updatedAtMs >= 0) { "updatedAtMs must be non-negative" }
        requireNonNegativeAmount(balance, "balance")
    }
}

/** Recipient challenge for a v2 offline bearer payment. */
class OfflineBearerReceiveRequestV2(
    val version: Int,
    val chainId: String,
    val paymentRequestId: String,
    val recipientCertificate: OfflineBearerCertificateV2,
    val assetDefinitionId: String,
    val amount: String,
    val createdAtMs: Long,
    val expiresAtMs: Long,
    val policyHashHex: String,
    val signatureAlgorithm: String = recipientCertificate.signatureAlgorithm,
    challengeSignature: ByteArray,
) {
    private val challengeSignatureBytes = challengeSignature.copyOf()

    init {
        require(version == VERSION) { "unsupported Offline Bearer receive request version" }
        requireNonBlank(chainId, "chainId")
        requireNonBlank(paymentRequestId, "paymentRequestId")
        requireNonBlank(assetDefinitionId, "assetDefinitionId")
        requirePositiveAmount(amount, "amount")
        require(expiresAtMs > createdAtMs) { "expiresAtMs must be after createdAtMs" }
        requireHexLike(policyHashHex, "policyHashHex")
        requireSupportedSignatureAlgorithm(signatureAlgorithm)
        require(challengeSignatureBytes.isNotEmpty()) { "challengeSignature must not be empty" }
    }

    fun challengeSignature(): ByteArray = challengeSignatureBytes.copyOf()

    companion object {
        const val VERSION: Int = 2
    }
}

/** Constant-size sender debit receipt transferred to the recipient. */
class OfflineBearerDebitReceiptV2(
    val version: Int,
    val transferId: String,
    val chainId: String,
    val paymentRequestId: String,
    val senderCertificate: OfflineBearerCertificateV2,
    val recipientCertificate: OfflineBearerCertificateV2,
    val assetDefinitionId: String,
    val amount: String,
    val senderPreBalance: String,
    val senderPostBalance: String,
    val senderSequence: Long,
    val createdAtMs: Long,
    val expiresAtMs: Long,
    val policyHashHex: String,
    receiveChallengeSignature: ByteArray,
    val signatureAlgorithm: String = senderCertificate.signatureAlgorithm,
    debitSignature: ByteArray,
) {
    private val receiveChallengeSignatureBytes = receiveChallengeSignature.copyOf()
    private val debitSignatureBytes = debitSignature.copyOf()

    init {
        require(version == VERSION) { "unsupported Offline Bearer debit receipt version" }
        requireNonBlank(transferId, "transferId")
        requireNonBlank(chainId, "chainId")
        requireNonBlank(paymentRequestId, "paymentRequestId")
        requireNonBlank(assetDefinitionId, "assetDefinitionId")
        requirePositiveAmount(amount, "amount")
        requireNonNegativeAmount(senderPreBalance, "senderPreBalance")
        requireNonNegativeAmount(senderPostBalance, "senderPostBalance")
        require(senderSequence > 0) { "senderSequence must be positive" }
        require(expiresAtMs > createdAtMs) { "expiresAtMs must be after createdAtMs" }
        requireHexLike(policyHashHex, "policyHashHex")
        requireSupportedSignatureAlgorithm(signatureAlgorithm)
        require(receiveChallengeSignatureBytes.isNotEmpty()) { "receiveChallengeSignature must not be empty" }
        require(debitSignatureBytes.isNotEmpty()) { "debitSignature must not be empty" }
    }

    fun receiveChallengeSignature(): ByteArray = receiveChallengeSignatureBytes.copyOf()

    fun debitSignature(): ByteArray = debitSignatureBytes.copyOf()

    companion object {
        const val VERSION: Int = 2
    }
}

/** Recipient-side credit receipt retained for later settlement. */
class OfflineBearerCreditReceiptV2(
    val version: Int,
    val transferId: String,
    val chainId: String,
    val recipientCertificate: OfflineBearerCertificateV2,
    val amount: String,
    val recipientPreBalance: String,
    val recipientPostBalance: String,
    val recipientSequence: Long,
    val acceptedAtMs: Long,
    val signatureAlgorithm: String = recipientCertificate.signatureAlgorithm,
    creditSignature: ByteArray,
) {
    private val creditSignatureBytes = creditSignature.copyOf()

    init {
        require(version == VERSION) { "unsupported Offline Bearer credit receipt version" }
        requireNonBlank(transferId, "transferId")
        requireNonBlank(chainId, "chainId")
        requirePositiveAmount(amount, "amount")
        requireNonNegativeAmount(recipientPreBalance, "recipientPreBalance")
        requireNonNegativeAmount(recipientPostBalance, "recipientPostBalance")
        require(recipientSequence > 0) { "recipientSequence must be positive" }
        require(acceptedAtMs >= 0) { "acceptedAtMs must be non-negative" }
        requireSupportedSignatureAlgorithm(signatureAlgorithm)
        require(creditSignatureBytes.isNotEmpty()) { "creditSignature must not be empty" }
    }

    fun creditSignature(): ByteArray = creditSignatureBytes.copyOf()

    companion object {
        const val VERSION: Int = 2
    }
}

/** Compact online settlement batch exported from a local v2 purse journal. */
class OfflineBearerSettlementBatchV2(
    val version: Int,
    val chainId: String,
    val purseId: String,
    debitReceipts: List<OfflineBearerDebitReceiptV2>,
    creditReceipts: List<OfflineBearerCreditReceiptV2>,
) {
    private val debitReceiptList = debitReceipts.toList()
    private val creditReceiptList = creditReceipts.toList()

    init {
        require(version == VERSION) { "unsupported Offline Bearer settlement batch version" }
        requireNonBlank(chainId, "chainId")
        requireNonBlank(purseId, "purseId")
    }

    fun debitReceipts(): List<OfflineBearerDebitReceiptV2> = debitReceiptList.toList()

    fun creditReceipts(): List<OfflineBearerCreditReceiptV2> = creditReceiptList.toList()

    companion object {
        const val VERSION: Int = 2
    }
}

/** Supplies the current app/issuer Offline Bearer policy bundle. */
interface OfflineBearerPolicyProvider {
    fun currentPolicy(): OfflineBearerPolicyBundleV2
}

/** Static policy provider useful for tests and apps with externally refreshed policy storage. */
class StaticOfflineBearerPolicyProvider(
    private val policy: OfflineBearerPolicyBundleV2,
) : OfflineBearerPolicyProvider {
    override fun currentPolicy(): OfflineBearerPolicyBundleV2 = policy
}

/** Hardware purse abstraction. Production implementations must fail closed when not stateful. */
interface OfflineBearerSecureElement {
    fun capabilities(): OfflineBearerSecureElementCapabilities
    fun currentCertificate(): OfflineBearerCertificateV2?
    fun currentState(): OfflineBearerPurseStateV2?
    fun installPurse(certificate: OfflineBearerCertificateV2, state: OfflineBearerPurseStateV2)
    fun createReceiveRequest(
        paymentRequestId: String,
        amount: String,
        createdAtMs: Long,
        expiresAtMs: Long,
        policyHashHex: String,
    ): OfflineBearerReceiveRequestV2
    fun debit(
        request: OfflineBearerReceiveRequestV2,
        transferId: String,
        createdAtMs: Long,
        expiresAtMs: Long,
    ): OfflineBearerDebitReceiptV2
    fun credit(receipt: OfflineBearerDebitReceiptV2, acceptedAtMs: Long): OfflineBearerCreditReceiptV2
    fun exportSettlementBatch(maxReceipts: Int): OfflineBearerSettlementBatchV2
    fun pruneSettled(transferIds: Collection<String>)
}

/** Fail-closed secure element for unsupported devices. */
class UnsupportedOfflineBearerSecureElement(
    private val hardwareClass: String = "unsupported",
) : OfflineBearerSecureElement {
    override fun capabilities(): OfflineBearerSecureElementCapabilities =
        OfflineBearerSecureElementCapabilities(
            hardwareBacked = false,
            statefulPurse = false,
            hardwareClass = hardwareClass,
            rollbackResistantState = false,
        )

    override fun currentCertificate(): OfflineBearerCertificateV2? = null

    override fun currentState(): OfflineBearerPurseStateV2? = null

    override fun installPurse(certificate: OfflineBearerCertificateV2, state: OfflineBearerPurseStateV2) {
        throw unsupported()
    }

    override fun createReceiveRequest(
        paymentRequestId: String,
        amount: String,
        createdAtMs: Long,
        expiresAtMs: Long,
        policyHashHex: String,
    ): OfflineBearerReceiveRequestV2 = throw unsupported()

    override fun debit(
        request: OfflineBearerReceiveRequestV2,
        transferId: String,
        createdAtMs: Long,
        expiresAtMs: Long,
    ): OfflineBearerDebitReceiptV2 = throw unsupported()

    override fun credit(receipt: OfflineBearerDebitReceiptV2, acceptedAtMs: Long): OfflineBearerCreditReceiptV2 =
        throw unsupported()

    override fun exportSettlementBatch(maxReceipts: Int): OfflineBearerSettlementBatchV2 = throw unsupported()

    override fun pruneSettled(transferIds: Collection<String>) {
        throw unsupported()
    }

    private fun unsupported(): OfflineBearerPolicyException =
        OfflineBearerPolicyException("Offline Bearer value is disabled on unsupported hardware")
}

/** Policy or hardware rejection raised before local offline value changes. */
class OfflineBearerPolicyException(message: String) : IllegalArgumentException(message)

/** Canonical Norito and text handoff codecs for Offline Bearer v2 app transports. */
object OfflineBearerV2TextCodec {
    const val RECEIVE_REQUEST_TEXT_PREFIX: String = "wallet-offline-bearer-receive:"
    const val PAYMENT_TEXT_PREFIX: String = "wallet-offline-bearer-payment:"
    const val ACK_TEXT_PREFIX: String = "wallet-offline-bearer-ack:"

    private const val POLICY_TYPE =
        "iroha_data_model::offline::model::OfflineBearerPolicyBundleV2"
    private const val CERTIFICATE_TYPE =
        "iroha_data_model::offline::model::OfflineBearerCertificateV2"
    private const val RECEIVE_REQUEST_TYPE =
        "iroha_data_model::offline::model::OfflineBearerReceiveRequestV2"
    private const val DEBIT_RECEIPT_TYPE =
        "iroha_data_model::offline::model::OfflineBearerDebitReceiptV2"
    private const val CREDIT_RECEIPT_TYPE =
        "iroha_data_model::offline::model::OfflineBearerCreditReceiptV2"
    private const val SETTLEMENT_BATCH_TYPE =
        "iroha_data_model::offline::model::OfflineBearerSettlementBatchV2"

    @JvmStatic
    fun encodePolicyBundleNorito(policy: OfflineBearerPolicyBundleV2): ByteArray =
        NoritoCodec.encode(policy, POLICY_TYPE, POLICY_ADAPTER, NoritoHeader.COMPACT_LEN)

    @JvmStatic
    fun decodePolicyBundleNorito(payload: ByteArray): OfflineBearerPolicyBundleV2 =
        NoritoCodec.decode(payload, POLICY_ADAPTER, POLICY_TYPE)

    @JvmStatic
    fun encodeCertificateNorito(certificate: OfflineBearerCertificateV2): ByteArray =
        NoritoCodec.encode(certificate, CERTIFICATE_TYPE, CERTIFICATE_ADAPTER, NoritoHeader.COMPACT_LEN)

    @JvmStatic
    fun decodeCertificateNorito(payload: ByteArray): OfflineBearerCertificateV2 =
        NoritoCodec.decode(payload, CERTIFICATE_ADAPTER, CERTIFICATE_TYPE)

    @JvmStatic
    fun encodeReceiveRequestNorito(request: OfflineBearerReceiveRequestV2): ByteArray =
        NoritoCodec.encode(request, RECEIVE_REQUEST_TYPE, RECEIVE_REQUEST_ADAPTER, NoritoHeader.COMPACT_LEN)

    @JvmStatic
    fun decodeReceiveRequestNorito(payload: ByteArray): OfflineBearerReceiveRequestV2 =
        NoritoCodec.decode(payload, RECEIVE_REQUEST_ADAPTER, RECEIVE_REQUEST_TYPE)

    @JvmStatic
    fun encodeDebitReceiptNorito(receipt: OfflineBearerDebitReceiptV2): ByteArray =
        NoritoCodec.encode(receipt, DEBIT_RECEIPT_TYPE, DEBIT_RECEIPT_ADAPTER, NoritoHeader.COMPACT_LEN)

    @JvmStatic
    fun decodeDebitReceiptNorito(payload: ByteArray): OfflineBearerDebitReceiptV2 =
        NoritoCodec.decode(payload, DEBIT_RECEIPT_ADAPTER, DEBIT_RECEIPT_TYPE)

    @JvmStatic
    fun encodeCreditReceiptNorito(receipt: OfflineBearerCreditReceiptV2): ByteArray =
        NoritoCodec.encode(receipt, CREDIT_RECEIPT_TYPE, CREDIT_RECEIPT_ADAPTER, NoritoHeader.COMPACT_LEN)

    @JvmStatic
    fun decodeCreditReceiptNorito(payload: ByteArray): OfflineBearerCreditReceiptV2 =
        NoritoCodec.decode(payload, CREDIT_RECEIPT_ADAPTER, CREDIT_RECEIPT_TYPE)

    @JvmStatic
    fun encodeSettlementBatchNorito(batch: OfflineBearerSettlementBatchV2): ByteArray =
        NoritoCodec.encode(batch, SETTLEMENT_BATCH_TYPE, SETTLEMENT_BATCH_ADAPTER, NoritoHeader.COMPACT_LEN)

    @JvmStatic
    fun decodeSettlementBatchNorito(payload: ByteArray): OfflineBearerSettlementBatchV2 =
        NoritoCodec.decode(payload, SETTLEMENT_BATCH_ADAPTER, SETTLEMENT_BATCH_TYPE)

    @JvmStatic
    fun encodeReceiveRequestText(request: OfflineBearerReceiveRequestV2): String =
        RECEIVE_REQUEST_TEXT_PREFIX + encodeBase64Url(encodeReceiveRequestNorito(request))

    @JvmStatic
    fun decodeReceiveRequestText(text: String): OfflineBearerReceiveRequestV2 =
        decodeReceiveRequestNorito(decodeTextPayload(text, RECEIVE_REQUEST_TEXT_PREFIX, "receive request"))

    @JvmStatic
    fun encodePaymentText(receipt: OfflineBearerDebitReceiptV2): String =
        PAYMENT_TEXT_PREFIX + encodeBase64Url(encodeDebitReceiptNorito(receipt))

    @JvmStatic
    fun decodePaymentText(text: String): OfflineBearerDebitReceiptV2 =
        decodeDebitReceiptNorito(decodeTextPayload(text, PAYMENT_TEXT_PREFIX, "payment"))

    @JvmStatic
    fun encodeAckText(receipt: OfflineBearerCreditReceiptV2): String =
        ACK_TEXT_PREFIX + encodeBase64Url(encodeCreditReceiptNorito(receipt))

    @JvmStatic
    fun decodeAckText(text: String): OfflineBearerCreditReceiptV2 =
        decodeCreditReceiptNorito(decodeTextPayload(text, ACK_TEXT_PREFIX, "ack"))

    @JvmStatic
    fun payloadKind(text: String): PayloadKind? {
        val trimmed = text.trim()
        return when {
            trimmed.startsWith(RECEIVE_REQUEST_TEXT_PREFIX) -> PayloadKind.RECEIVE_REQUEST
            trimmed.startsWith(PAYMENT_TEXT_PREFIX) -> PayloadKind.PAYMENT
            trimmed.startsWith(ACK_TEXT_PREFIX) -> PayloadKind.ACK
            else -> null
        }
    }

    enum class PayloadKind {
        RECEIVE_REQUEST,
        PAYMENT,
        ACK,
    }

    private val ASSET_SEND_LIMIT_ADAPTER: TypeAdapter<OfflineBearerAssetSendLimitV2> =
        object : TypeAdapter<OfflineBearerAssetSendLimitV2> {
            override fun encode(encoder: NoritoEncoder, value: OfflineBearerAssetSendLimitV2) {
                writeField(encoder) { writeString(it, value.assetDefinitionId) }
                writeField(encoder) { writeString(it, value.maxTransactionAmount) }
                writeField(encoder) { writeString(it, value.dailySendLimit) }
                writeField(encoder) { writeString(it, value.monthlySendLimit) }
            }

            override fun decode(decoder: NoritoDecoder): OfflineBearerAssetSendLimitV2 =
                OfflineBearerAssetSendLimitV2(
                    assetDefinitionId = readField(decoder, ::readString),
                    maxTransactionAmount = readField(decoder, ::readString),
                    dailySendLimit = readField(decoder, ::readString),
                    monthlySendLimit = readField(decoder, ::readString),
                )
        }

    private val POLICY_ADAPTER: TypeAdapter<OfflineBearerPolicyBundleV2> =
        object : TypeAdapter<OfflineBearerPolicyBundleV2> {
            override fun encode(encoder: NoritoEncoder, value: OfflineBearerPolicyBundleV2) {
                writeField(encoder) { writeString(it, value.policyId) }
                writeField(encoder) { writeString(it, value.policyHashHex) }
                writeField(encoder) { writeString(it, value.issuerId) }
                writeField(encoder) { it.writeUInt(value.issuedAtMs, 64) }
                writeField(encoder) { it.writeUInt(value.expiresAtMs, 64) }
                writeField(encoder) { it.writeUInt(value.maxCertificateAgeMs, 64) }
                writeField(encoder) { it.writeUInt(value.maxPolicyAgeMs, 64) }
                writeField(encoder) { it.writeUInt(value.maxTokenAgeMs, 64) }
                writeField(encoder) { writeString(it, value.maxOfflineBalance) }
                writeField(encoder) { writeString(it, value.maxTransactionAmount) }
                writeField(encoder) { writeStringList(it, value.allowedHardwareClasses.sorted()) }
                writeField(encoder) { writeStringList(it, value.blacklistedAccountIds.sorted()) }
                writeField(encoder) { writeStringList(it, value.blacklistedDeviceIds.sorted()) }
                writeField(encoder) { writeStringList(it, value.blacklistedKeyIds.sorted()) }
                writeField(encoder) { writeString(it, value.signatureAlgorithm) }
                writeField(encoder) { writeBytesVec(it, value.issuerSignature()) }
                writeField(encoder) { it.writeUInt(value.policyEpoch, 64) }
                writeField(encoder) { writeString(it, value.policySource) }
                writeField(encoder) { writeStringList(it, value.revokedCertificateIds.sorted()) }
                writeField(encoder) { writeStringList(it, value.revokedTransferIds.sorted()) }
                writeField(encoder) { writeList(it, value.assetSendLimits, ASSET_SEND_LIMIT_ADAPTER) }
            }

            override fun decode(decoder: NoritoDecoder): OfflineBearerPolicyBundleV2 =
                OfflineBearerPolicyBundleV2(
                    policyId = readField(decoder, ::readString),
                    policyHashHex = readField(decoder, ::readString),
                    issuerId = readField(decoder, ::readString),
                    issuedAtMs = readField(decoder) { it.readUInt(64) },
                    expiresAtMs = readField(decoder) { it.readUInt(64) },
                    maxCertificateAgeMs = readField(decoder) { it.readUInt(64) },
                    maxPolicyAgeMs = readField(decoder) { it.readUInt(64) },
                    maxTokenAgeMs = readField(decoder) { it.readUInt(64) },
                    maxOfflineBalance = readField(decoder, ::readString),
                    maxTransactionAmount = readField(decoder, ::readString),
                    allowedHardwareClasses = readField(decoder, ::readStringList),
                    blacklistedAccountIds = readField(decoder, ::readStringList),
                    blacklistedDeviceIds = readField(decoder, ::readStringList),
                    blacklistedKeyIds = readField(decoder, ::readStringList),
                    signatureAlgorithm = readField(decoder, ::readString),
                    issuerSignature = readField(decoder, ::readBytesVec),
                    policyEpoch = readField(decoder) { it.readUInt(64) },
                    policySource = readField(decoder, ::readString),
                    revokedCertificateIds = readField(decoder, ::readStringList),
                    revokedTransferIds = readField(decoder, ::readStringList),
                    assetSendLimits = readField(decoder) { readList(it, ASSET_SEND_LIMIT_ADAPTER) },
                )
        }

    private val CERTIFICATE_ADAPTER: TypeAdapter<OfflineBearerCertificateV2> =
        object : TypeAdapter<OfflineBearerCertificateV2> {
            override fun encode(encoder: NoritoEncoder, value: OfflineBearerCertificateV2) {
                writeField(encoder) { writeString(it, value.certificateId) }
                writeField(encoder) { writeString(it, value.chainId) }
                writeField(encoder) { writeString(it, value.issuerId) }
                writeField(encoder) { writeString(it, value.purseId) }
                writeField(encoder) { writeString(it, value.accountId) }
                writeField(encoder) { writeString(it, value.assetDefinitionId) }
                writeField(encoder) { writeString(it, value.deviceId) }
                writeField(encoder) { writeString(it, value.keyId) }
                writeField(encoder) { writeString(it, value.hardwareClass) }
                writeField(encoder) { writeString(it, value.signatureAlgorithm) }
                writeField(encoder) { writeString(it, value.publicKeyEncoding) }
                writeField(encoder) { writeBytesVec(it, value.publicKey()) }
                writeField(encoder) { it.writeUInt(value.issuedAtMs, 64) }
                writeField(encoder) { it.writeUInt(value.expiresAtMs, 64) }
                writeField(encoder) { writeString(it, value.policyId) }
                writeField(encoder) { writeString(it, value.policyHashHex) }
                writeField(encoder) { writeBytesVec(it, value.issuerSignature()) }
            }

            override fun decode(decoder: NoritoDecoder): OfflineBearerCertificateV2 =
                OfflineBearerCertificateV2(
                    certificateId = readField(decoder, ::readString),
                    chainId = readField(decoder, ::readString),
                    issuerId = readField(decoder, ::readString),
                    purseId = readField(decoder, ::readString),
                    accountId = readField(decoder, ::readString),
                    assetDefinitionId = readField(decoder, ::readString),
                    deviceId = readField(decoder, ::readString),
                    keyId = readField(decoder, ::readString),
                    hardwareClass = readField(decoder, ::readString),
                    signatureAlgorithm = readField(decoder, ::readString),
                    publicKeyEncoding = readField(decoder, ::readString),
                    publicKey = readField(decoder, ::readBytesVec),
                    issuedAtMs = readField(decoder) { it.readUInt(64) },
                    expiresAtMs = readField(decoder) { it.readUInt(64) },
                    policyId = readField(decoder, ::readString),
                    policyHashHex = readField(decoder, ::readString),
                    issuerSignature = readField(decoder, ::readBytesVec),
                )
        }

    private val RECEIVE_REQUEST_ADAPTER: TypeAdapter<OfflineBearerReceiveRequestV2> =
        object : TypeAdapter<OfflineBearerReceiveRequestV2> {
            override fun encode(encoder: NoritoEncoder, value: OfflineBearerReceiveRequestV2) {
                writeField(encoder) { it.writeUInt(value.version.toLong(), 16) }
                writeField(encoder) { writeString(it, value.chainId) }
                writeField(encoder) { writeString(it, value.paymentRequestId) }
                writeField(encoder) { CERTIFICATE_ADAPTER.encode(it, value.recipientCertificate) }
                writeField(encoder) { writeString(it, value.assetDefinitionId) }
                writeField(encoder) { writeString(it, value.amount) }
                writeField(encoder) { it.writeUInt(value.createdAtMs, 64) }
                writeField(encoder) { it.writeUInt(value.expiresAtMs, 64) }
                writeField(encoder) { writeString(it, value.policyHashHex) }
                writeField(encoder) { writeString(it, value.signatureAlgorithm) }
                writeField(encoder) { writeBytesVec(it, value.challengeSignature()) }
            }

            override fun decode(decoder: NoritoDecoder): OfflineBearerReceiveRequestV2 =
                OfflineBearerReceiveRequestV2(
                    version = readField(decoder) { it.readUInt(16).toInt() },
                    chainId = readField(decoder, ::readString),
                    paymentRequestId = readField(decoder, ::readString),
                    recipientCertificate = readField(decoder) { CERTIFICATE_ADAPTER.decode(it) },
                    assetDefinitionId = readField(decoder, ::readString),
                    amount = readField(decoder, ::readString),
                    createdAtMs = readField(decoder) { it.readUInt(64) },
                    expiresAtMs = readField(decoder) { it.readUInt(64) },
                    policyHashHex = readField(decoder, ::readString),
                    signatureAlgorithm = readField(decoder, ::readString),
                    challengeSignature = readField(decoder, ::readBytesVec),
                )
        }

    private val DEBIT_RECEIPT_ADAPTER: TypeAdapter<OfflineBearerDebitReceiptV2> =
        object : TypeAdapter<OfflineBearerDebitReceiptV2> {
            override fun encode(encoder: NoritoEncoder, value: OfflineBearerDebitReceiptV2) {
                writeField(encoder) { it.writeUInt(value.version.toLong(), 16) }
                writeField(encoder) { writeString(it, value.transferId) }
                writeField(encoder) { writeString(it, value.chainId) }
                writeField(encoder) { writeString(it, value.paymentRequestId) }
                writeField(encoder) { CERTIFICATE_ADAPTER.encode(it, value.senderCertificate) }
                writeField(encoder) { CERTIFICATE_ADAPTER.encode(it, value.recipientCertificate) }
                writeField(encoder) { writeString(it, value.assetDefinitionId) }
                writeField(encoder) { writeString(it, value.amount) }
                writeField(encoder) { writeString(it, value.senderPreBalance) }
                writeField(encoder) { writeString(it, value.senderPostBalance) }
                writeField(encoder) { it.writeUInt(value.senderSequence, 64) }
                writeField(encoder) { it.writeUInt(value.createdAtMs, 64) }
                writeField(encoder) { it.writeUInt(value.expiresAtMs, 64) }
                writeField(encoder) { writeString(it, value.policyHashHex) }
                writeField(encoder) { writeBytesVec(it, value.receiveChallengeSignature()) }
                writeField(encoder) { writeString(it, value.signatureAlgorithm) }
                writeField(encoder) { writeBytesVec(it, value.debitSignature()) }
            }

            override fun decode(decoder: NoritoDecoder): OfflineBearerDebitReceiptV2 =
                OfflineBearerDebitReceiptV2(
                    version = readField(decoder) { it.readUInt(16).toInt() },
                    transferId = readField(decoder, ::readString),
                    chainId = readField(decoder, ::readString),
                    paymentRequestId = readField(decoder, ::readString),
                    senderCertificate = readField(decoder) { CERTIFICATE_ADAPTER.decode(it) },
                    recipientCertificate = readField(decoder) { CERTIFICATE_ADAPTER.decode(it) },
                    assetDefinitionId = readField(decoder, ::readString),
                    amount = readField(decoder, ::readString),
                    senderPreBalance = readField(decoder, ::readString),
                    senderPostBalance = readField(decoder, ::readString),
                    senderSequence = readField(decoder) { it.readUInt(64) },
                    createdAtMs = readField(decoder) { it.readUInt(64) },
                    expiresAtMs = readField(decoder) { it.readUInt(64) },
                    policyHashHex = readField(decoder, ::readString),
                    receiveChallengeSignature = readField(decoder, ::readBytesVec),
                    signatureAlgorithm = readField(decoder, ::readString),
                    debitSignature = readField(decoder, ::readBytesVec),
                )
        }

    private val CREDIT_RECEIPT_ADAPTER: TypeAdapter<OfflineBearerCreditReceiptV2> =
        object : TypeAdapter<OfflineBearerCreditReceiptV2> {
            override fun encode(encoder: NoritoEncoder, value: OfflineBearerCreditReceiptV2) {
                writeField(encoder) { it.writeUInt(value.version.toLong(), 16) }
                writeField(encoder) { writeString(it, value.transferId) }
                writeField(encoder) { writeString(it, value.chainId) }
                writeField(encoder) { CERTIFICATE_ADAPTER.encode(it, value.recipientCertificate) }
                writeField(encoder) { writeString(it, value.amount) }
                writeField(encoder) { writeString(it, value.recipientPreBalance) }
                writeField(encoder) { writeString(it, value.recipientPostBalance) }
                writeField(encoder) { it.writeUInt(value.recipientSequence, 64) }
                writeField(encoder) { it.writeUInt(value.acceptedAtMs, 64) }
                writeField(encoder) { writeString(it, value.signatureAlgorithm) }
                writeField(encoder) { writeBytesVec(it, value.creditSignature()) }
            }

            override fun decode(decoder: NoritoDecoder): OfflineBearerCreditReceiptV2 =
                OfflineBearerCreditReceiptV2(
                    version = readField(decoder) { it.readUInt(16).toInt() },
                    transferId = readField(decoder, ::readString),
                    chainId = readField(decoder, ::readString),
                    recipientCertificate = readField(decoder) { CERTIFICATE_ADAPTER.decode(it) },
                    amount = readField(decoder, ::readString),
                    recipientPreBalance = readField(decoder, ::readString),
                    recipientPostBalance = readField(decoder, ::readString),
                    recipientSequence = readField(decoder) { it.readUInt(64) },
                    acceptedAtMs = readField(decoder) { it.readUInt(64) },
                    signatureAlgorithm = readField(decoder, ::readString),
                    creditSignature = readField(decoder, ::readBytesVec),
                )
        }

    private val SETTLEMENT_BATCH_ADAPTER: TypeAdapter<OfflineBearerSettlementBatchV2> =
        object : TypeAdapter<OfflineBearerSettlementBatchV2> {
            override fun encode(encoder: NoritoEncoder, value: OfflineBearerSettlementBatchV2) {
                writeField(encoder) { it.writeUInt(value.version.toLong(), 16) }
                writeField(encoder) { writeString(it, value.chainId) }
                writeField(encoder) { writeString(it, value.purseId) }
                writeField(encoder) { writeList(it, value.debitReceipts(), DEBIT_RECEIPT_ADAPTER) }
                writeField(encoder) { writeList(it, value.creditReceipts(), CREDIT_RECEIPT_ADAPTER) }
            }

            override fun decode(decoder: NoritoDecoder): OfflineBearerSettlementBatchV2 =
                OfflineBearerSettlementBatchV2(
                    version = readField(decoder) { it.readUInt(16).toInt() },
                    chainId = readField(decoder, ::readString),
                    purseId = readField(decoder, ::readString),
                    debitReceipts = readField(decoder) { readList(it, DEBIT_RECEIPT_ADAPTER) },
                    creditReceipts = readField(decoder) { readList(it, CREDIT_RECEIPT_ADAPTER) },
                )
        }

    private fun encodeBase64Url(payload: ByteArray): String =
        Base64.getUrlEncoder().withoutPadding().encodeToString(payload)

    private fun decodeTextPayload(text: String, prefix: String, label: String): ByteArray {
        val trimmed = text.trim()
        require(trimmed.startsWith(prefix)) { "Offline Bearer $label prefix missing" }
        return Base64.getUrlDecoder().decode(trimmed.substring(prefix.length))
    }

    private fun writeField(encoder: NoritoEncoder, write: (NoritoEncoder) -> Unit) {
        val child = encoder.childEncoder()
        write(child)
        val payload = child.toByteArray()
        encoder.writeLength(payload.size.toLong(), compact = true)
        encoder.writeBytes(payload)
    }

    private fun <T> readField(decoder: NoritoDecoder, read: (NoritoDecoder) -> T): T {
        val length = decoder.readLength(compact = true)
        require(length <= Int.MAX_VALUE) { "Offline Bearer field length overflow" }
        val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags, decoder.flagsHint)
        val value = read(child)
        require(child.remaining() == 0) { "Trailing bytes after Offline Bearer field decode" }
        return value
    }

    private fun writeString(encoder: NoritoEncoder, value: String) {
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        encoder.writeLength(bytes.size.toLong(), compact = true)
        encoder.writeBytes(bytes)
    }

    private fun readString(decoder: NoritoDecoder): String {
        val length = decoder.readLength(compact = true)
        require(length <= Int.MAX_VALUE) { "Offline Bearer string length overflow" }
        return String(decoder.readBytes(length.toInt()), StandardCharsets.UTF_8)
    }

    private fun writeBytesVec(encoder: NoritoEncoder, value: ByteArray) {
        encoder.writeUInt(value.size.toLong(), 64)
        encoder.writeBytes(value)
    }

    private fun readBytesVec(decoder: NoritoDecoder): ByteArray {
        val length = decoder.readUInt(64)
        require(length <= Int.MAX_VALUE) { "Offline Bearer bytes length overflow" }
        return decoder.readBytes(length.toInt())
    }

    private fun writeStringList(encoder: NoritoEncoder, values: List<String>) {
        encoder.writeLength(values.size.toLong(), compact = false)
        values.forEach { value ->
            writeField(encoder) { writeString(it, value) }
        }
    }

    private fun readStringList(decoder: NoritoDecoder): List<String> =
        readList(
            decoder,
            object : TypeAdapter<String> {
                override fun encode(encoder: NoritoEncoder, value: String) {
                    writeString(encoder, value)
                }

                override fun decode(decoder: NoritoDecoder): String = readString(decoder)
            },
        )

    private fun <T> writeList(encoder: NoritoEncoder, values: List<T>, adapter: TypeAdapter<T>) {
        encoder.writeLength(values.size.toLong(), compact = false)
        values.forEach { value ->
            writeField(encoder) { adapter.encode(it, value) }
        }
    }

    private fun <T> readList(decoder: NoritoDecoder, adapter: TypeAdapter<T>): List<T> {
        val count = decoder.readLength(compact = false)
        require(count <= Int.MAX_VALUE) { "Offline Bearer list length overflow" }
        val values = ArrayList<T>(count.toInt())
        repeat(count.toInt()) {
            values.add(readField(decoder) { adapter.decode(it) })
        }
        return values
    }
}

/** Canonical domain-separated unsigned payloads for Offline Bearer v2 signatures. */
object OfflineBearerV2Payloads {
    private const val POLICY_PAYLOAD_TYPE =
        "iroha_data_model::offline::model::OfflineBearerPolicyBundlePayloadV2"
    private const val POLICY_TYPE =
        "iroha_data_model::offline::model::OfflineBearerPolicyBundleV2"
    private const val CERTIFICATE_PAYLOAD_TYPE =
        "iroha_data_model::offline::model::OfflineBearerCertificatePayloadV2"
    private const val CERTIFICATE_TYPE =
        "iroha_data_model::offline::model::OfflineBearerCertificateV2"
    private const val RECEIVE_REQUEST_PAYLOAD_TYPE =
        "iroha_data_model::offline::model::OfflineBearerReceiveRequestPayloadV2"
    private const val RECEIVE_REQUEST_TYPE =
        "iroha_data_model::offline::model::OfflineBearerReceiveRequestV2"
    private const val DEBIT_RECEIPT_PAYLOAD_TYPE =
        "iroha_data_model::offline::model::OfflineBearerDebitReceiptPayloadV2"
    private const val DEBIT_RECEIPT_TYPE =
        "iroha_data_model::offline::model::OfflineBearerDebitReceiptV2"
    private const val CREDIT_RECEIPT_PAYLOAD_TYPE =
        "iroha_data_model::offline::model::OfflineBearerCreditReceiptPayloadV2"
    private const val CREDIT_RECEIPT_TYPE =
        "iroha_data_model::offline::model::OfflineBearerCreditReceiptV2"
    private const val SETTLEMENT_BATCH_PAYLOAD_TYPE =
        "iroha_data_model::offline::model::OfflineBearerSettlementBatchPayloadV2"
    private const val SETTLEMENT_BATCH_TYPE =
        "iroha_data_model::offline::model::OfflineBearerSettlementBatchV2"

    private const val POLICY_DOMAIN = "iroha:offline-bearer-v2:policy-bundle"
    private const val CERTIFICATE_DOMAIN = "iroha:offline-bearer-v2:certificate"
    private const val RECEIVE_REQUEST_DOMAIN = "iroha:offline-bearer-v2:receive-request"
    private const val DEBIT_RECEIPT_DOMAIN = "iroha:offline-bearer-v2:debit-receipt"
    private const val CREDIT_RECEIPT_DOMAIN = "iroha:offline-bearer-v2:credit-receipt"
    private const val SETTLEMENT_BATCH_DOMAIN = "iroha:offline-bearer-v2:settlement-batch"

    private const val FLAGS = NoritoHeader.COMPACT_LEN

    @JvmStatic
    fun policyUnsignedPayload(policy: OfflineBearerPolicyBundleV2): ByteArray =
        frame(
            POLICY_PAYLOAD_TYPE,
            policyPayload(policy, includeSignature = false, includeDomain = true),
        )

    @JvmStatic
    fun certificateUnsignedPayload(certificate: OfflineBearerCertificateV2): ByteArray =
        frame(
            CERTIFICATE_PAYLOAD_TYPE,
            certificatePayload(certificate, includeSignature = false, includeDomain = true),
        )

    @JvmStatic
    fun receiveRequestUnsignedPayload(request: OfflineBearerReceiveRequestV2): ByteArray =
        frame(
            RECEIVE_REQUEST_PAYLOAD_TYPE,
            structPayload(
                stringPayload(RECEIVE_REQUEST_DOMAIN),
                u16Payload(request.version),
                stringPayload(request.chainId),
                stringPayload(request.paymentRequestId),
                hashPayload(signedCertificateHash(request.recipientCertificate)),
                stringPayload(request.assetDefinitionId),
                stringPayload(request.amount),
                u64Payload(request.createdAtMs),
                u64Payload(request.expiresAtMs),
                stringPayload(request.policyHashHex),
                stringPayload(request.signatureAlgorithm),
            ),
        )

    @JvmStatic
    fun debitReceiptUnsignedPayload(receipt: OfflineBearerDebitReceiptV2): ByteArray =
        frame(
            DEBIT_RECEIPT_PAYLOAD_TYPE,
            structPayload(
                stringPayload(DEBIT_RECEIPT_DOMAIN),
                u16Payload(receipt.version),
                stringPayload(receipt.transferId),
                stringPayload(receipt.chainId),
                stringPayload(receipt.paymentRequestId),
                hashPayload(signedCertificateHash(receipt.senderCertificate)),
                hashPayload(signedCertificateHash(receipt.recipientCertificate)),
                stringPayload(receipt.assetDefinitionId),
                stringPayload(receipt.amount),
                stringPayload(receipt.senderPreBalance),
                stringPayload(receipt.senderPostBalance),
                u64Payload(receipt.senderSequence),
                u64Payload(receipt.createdAtMs),
                u64Payload(receipt.expiresAtMs),
                stringPayload(receipt.policyHashHex),
                bytesVecPayload(receipt.receiveChallengeSignature()),
                stringPayload(receipt.signatureAlgorithm),
            ),
        )

    @JvmStatic
    fun creditReceiptUnsignedPayload(receipt: OfflineBearerCreditReceiptV2): ByteArray =
        frame(
            CREDIT_RECEIPT_PAYLOAD_TYPE,
            structPayload(
                stringPayload(CREDIT_RECEIPT_DOMAIN),
                u16Payload(receipt.version),
                stringPayload(receipt.transferId),
                stringPayload(receipt.chainId),
                hashPayload(signedCertificateHash(receipt.recipientCertificate)),
                stringPayload(receipt.amount),
                stringPayload(receipt.recipientPreBalance),
                stringPayload(receipt.recipientPostBalance),
                u64Payload(receipt.recipientSequence),
                u64Payload(receipt.acceptedAtMs),
                stringPayload(receipt.signatureAlgorithm),
            ),
        )

    @JvmStatic
    fun settlementBatchUnsignedPayload(batch: OfflineBearerSettlementBatchV2): ByteArray =
        frame(
            SETTLEMENT_BATCH_PAYLOAD_TYPE,
            structPayload(
                stringPayload(SETTLEMENT_BATCH_DOMAIN),
                u16Payload(batch.version),
                stringPayload(batch.chainId),
                stringPayload(batch.purseId),
                hashVecPayload(batch.debitReceipts().map { signedDebitReceiptHash(it) }),
                hashVecPayload(batch.creditReceipts().map { signedCreditReceiptHash(it) }),
            ),
        )

    private fun policyPayload(
        policy: OfflineBearerPolicyBundleV2,
        includeSignature: Boolean,
        includeDomain: Boolean,
    ): ByteArray =
        structPayload(
            buildList {
                if (includeDomain) {
                    add(stringPayload(POLICY_DOMAIN))
                }
                add(stringPayload(policy.policyId))
                add(stringPayload(policy.policyHashHex))
                add(stringPayload(policy.issuerId))
                add(u64Payload(policy.issuedAtMs))
                add(u64Payload(policy.expiresAtMs))
                add(u64Payload(policy.maxCertificateAgeMs))
                add(u64Payload(policy.maxPolicyAgeMs))
                add(u64Payload(policy.maxTokenAgeMs))
                add(stringPayload(policy.maxOfflineBalance))
                add(stringPayload(policy.maxTransactionAmount))
                add(stringVecPayload(policy.allowedHardwareClasses))
                add(stringVecPayload(policy.blacklistedAccountIds))
                add(stringVecPayload(policy.blacklistedDeviceIds))
                add(stringVecPayload(policy.blacklistedKeyIds))
                add(stringPayload(policy.signatureAlgorithm))
                if (includeSignature) {
                    add(bytesVecPayload(policy.issuerSignature()))
                }
                add(u64Payload(policy.policyEpoch))
                add(stringPayload(policy.policySource))
                add(stringVecPayload(policy.revokedCertificateIds))
                add(stringVecPayload(policy.revokedTransferIds))
                add(assetSendLimitVecPayload(policy.assetSendLimits))
            },
        )

    private fun certificatePayload(
        certificate: OfflineBearerCertificateV2,
        includeSignature: Boolean,
        includeDomain: Boolean,
    ): ByteArray =
        structPayload(
            buildList {
                if (includeDomain) {
                    add(stringPayload(CERTIFICATE_DOMAIN))
                }
                add(stringPayload(certificate.certificateId))
                add(stringPayload(certificate.chainId))
                add(stringPayload(certificate.issuerId))
                add(stringPayload(certificate.purseId))
                add(stringPayload(certificate.accountId))
                add(stringPayload(certificate.assetDefinitionId))
                add(stringPayload(certificate.deviceId))
                add(stringPayload(certificate.keyId))
                add(stringPayload(certificate.hardwareClass))
                add(stringPayload(certificate.signatureAlgorithm))
                add(stringPayload(certificate.publicKeyEncoding))
                add(bytesVecPayload(certificate.publicKey()))
                add(u64Payload(certificate.issuedAtMs))
                add(u64Payload(certificate.expiresAtMs))
                add(stringPayload(certificate.policyId))
                add(stringPayload(certificate.policyHashHex))
                if (includeSignature) {
                    add(bytesVecPayload(certificate.issuerSignature()))
                }
            },
        )

    private fun receiveRequestPayload(request: OfflineBearerReceiveRequestV2, includeSignature: Boolean): ByteArray =
        structPayload(
            buildList {
                add(u16Payload(request.version))
                add(stringPayload(request.chainId))
                add(stringPayload(request.paymentRequestId))
                add(certificatePayload(request.recipientCertificate, includeSignature = true, includeDomain = false))
                add(stringPayload(request.assetDefinitionId))
                add(stringPayload(request.amount))
                add(u64Payload(request.createdAtMs))
                add(u64Payload(request.expiresAtMs))
                add(stringPayload(request.policyHashHex))
                add(stringPayload(request.signatureAlgorithm))
                if (includeSignature) {
                    add(bytesVecPayload(request.challengeSignature()))
                }
            },
        )

    private fun debitReceiptPayload(receipt: OfflineBearerDebitReceiptV2, includeSignature: Boolean): ByteArray =
        structPayload(
            buildList {
                add(u16Payload(receipt.version))
                add(stringPayload(receipt.transferId))
                add(stringPayload(receipt.chainId))
                add(stringPayload(receipt.paymentRequestId))
                add(certificatePayload(receipt.senderCertificate, includeSignature = true, includeDomain = false))
                add(certificatePayload(receipt.recipientCertificate, includeSignature = true, includeDomain = false))
                add(stringPayload(receipt.assetDefinitionId))
                add(stringPayload(receipt.amount))
                add(stringPayload(receipt.senderPreBalance))
                add(stringPayload(receipt.senderPostBalance))
                add(u64Payload(receipt.senderSequence))
                add(u64Payload(receipt.createdAtMs))
                add(u64Payload(receipt.expiresAtMs))
                add(stringPayload(receipt.policyHashHex))
                add(bytesVecPayload(receipt.receiveChallengeSignature()))
                add(stringPayload(receipt.signatureAlgorithm))
                if (includeSignature) {
                    add(bytesVecPayload(receipt.debitSignature()))
                }
            },
        )

    private fun creditReceiptPayload(receipt: OfflineBearerCreditReceiptV2, includeSignature: Boolean): ByteArray =
        structPayload(
            buildList {
                add(u16Payload(receipt.version))
                add(stringPayload(receipt.transferId))
                add(stringPayload(receipt.chainId))
                add(certificatePayload(receipt.recipientCertificate, includeSignature = true, includeDomain = false))
                add(stringPayload(receipt.amount))
                add(stringPayload(receipt.recipientPreBalance))
                add(stringPayload(receipt.recipientPostBalance))
                add(u64Payload(receipt.recipientSequence))
                add(u64Payload(receipt.acceptedAtMs))
                add(stringPayload(receipt.signatureAlgorithm))
                if (includeSignature) {
                    add(bytesVecPayload(receipt.creditSignature()))
                }
            },
        )

    @Suppress("unused")
    private fun settlementBatchPayload(batch: OfflineBearerSettlementBatchV2): ByteArray =
        structPayload(
            u16Payload(batch.version),
            stringPayload(batch.chainId),
            stringPayload(batch.purseId),
            vecPayload(batch.debitReceipts().map { debitReceiptPayload(it, includeSignature = true) }),
            vecPayload(batch.creditReceipts().map { creditReceiptPayload(it, includeSignature = true) }),
        )

    private fun signedCertificateHash(certificate: OfflineBearerCertificateV2): ByteArray =
        IrohaHash.prehash(frame(CERTIFICATE_TYPE, certificatePayload(certificate, includeSignature = true, includeDomain = false)))

    private fun signedDebitReceiptHash(receipt: OfflineBearerDebitReceiptV2): ByteArray =
        IrohaHash.prehash(frame(DEBIT_RECEIPT_TYPE, debitReceiptPayload(receipt, includeSignature = true)))

    private fun signedCreditReceiptHash(receipt: OfflineBearerCreditReceiptV2): ByteArray =
        IrohaHash.prehash(frame(CREDIT_RECEIPT_TYPE, creditReceiptPayload(receipt, includeSignature = true)))

    @Suppress("unused")
    private fun signedPolicyBytes(policy: OfflineBearerPolicyBundleV2): ByteArray =
        frame(POLICY_TYPE, policyPayload(policy, includeSignature = true, includeDomain = false))

    @Suppress("unused")
    private fun signedReceiveRequestBytes(request: OfflineBearerReceiveRequestV2): ByteArray =
        frame(RECEIVE_REQUEST_TYPE, receiveRequestPayload(request, includeSignature = true))

    @Suppress("unused")
    private fun signedSettlementBatchBytes(batch: OfflineBearerSettlementBatchV2): ByteArray =
        frame(SETTLEMENT_BATCH_TYPE, settlementBatchPayload(batch))

    private fun assetSendLimitVecPayload(limits: List<OfflineBearerAssetSendLimitV2>): ByteArray =
        vecPayload(
            limits.sortedBy { it.assetDefinitionId }.map { limit ->
                structPayload(
                    stringPayload(limit.assetDefinitionId),
                    stringPayload(limit.maxTransactionAmount),
                    stringPayload(limit.dailySendLimit),
                    stringPayload(limit.monthlySendLimit),
                )
            },
        )

    private fun stringVecPayload(values: Collection<String>): ByteArray =
        vecPayload(
            values
                .map { it.trim() }
                .filter { it.isNotEmpty() }
                .distinct()
                .sorted()
                .map(::stringPayload),
        )

    private fun hashVecPayload(values: List<ByteArray>): ByteArray =
        vecPayload(values.map(::hashPayload))

    private fun vecPayload(values: List<ByteArray>): ByteArray {
        val encoder = NoritoEncoder(FLAGS)
        encoder.writeLength(values.size.toLong(), compact = true)
        values.forEach { value ->
            encoder.writeLength(value.size.toLong(), compact = true)
            encoder.writeBytes(value)
        }
        return encoder.toByteArray()
    }

    private fun structPayload(vararg fields: ByteArray): ByteArray =
        structPayload(fields.toList())

    private fun structPayload(fields: List<ByteArray>): ByteArray {
        val encoder = NoritoEncoder(FLAGS)
        fields.forEach { payload ->
            encoder.writeLength(payload.size.toLong(), compact = true)
            encoder.writeBytes(payload)
        }
        return encoder.toByteArray()
    }

    private fun stringPayload(value: String): ByteArray {
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        val encoder = NoritoEncoder(FLAGS)
        encoder.writeLength(bytes.size.toLong(), compact = true)
        encoder.writeBytes(bytes)
        return encoder.toByteArray()
    }

    private fun bytesVecPayload(value: ByteArray): ByteArray {
        val encoder = NoritoEncoder(FLAGS)
        encoder.writeLength(value.size.toLong(), compact = true)
        encoder.writeBytes(value)
        return encoder.toByteArray()
    }

    private fun hashPayload(value: ByteArray): ByteArray {
        require(value.size == 32) { "Offline Bearer hash fields must be 32 bytes" }
        return value.copyOf()
    }

    private fun u16Payload(value: Int): ByteArray {
        require(value in 0..UShort.MAX_VALUE.toInt()) { "u16 value is out of range" }
        val encoder = NoritoEncoder(FLAGS)
        encoder.writeUInt(value.toLong(), 16)
        return encoder.toByteArray()
    }

    private fun u64Payload(value: Long): ByteArray {
        require(value >= 0L) { "u64 value must be non-negative" }
        val encoder = NoritoEncoder(FLAGS)
        encoder.writeUInt(value, 64)
        return encoder.toByteArray()
    }

    private fun frame(typeName: String, payload: ByteArray): ByteArray {
        val header = NoritoHeader(
            SchemaHash.hash16(typeName),
            payload.size,
            CRC64.compute(payload),
            FLAGS,
            NoritoHeader.COMPRESSION_NONE,
        ).encode()
        val output = ByteArrayOutputStream(header.size + payload.size)
        output.write(header)
        output.write(payload)
        return output.toByteArray()
    }
}

/** Verifies Offline Bearer v2 issuer and hardware-purse signatures. */
interface OfflineBearerSignatureVerifying {
    fun verifyPolicy(policy: OfflineBearerPolicyBundleV2)
    fun verifyCertificate(certificate: OfflineBearerCertificateV2, policy: OfflineBearerPolicyBundleV2)
    fun verifyReceiveRequest(request: OfflineBearerReceiveRequestV2)
    fun verifyDebitReceipt(receipt: OfflineBearerDebitReceiptV2)
    fun verifyCreditReceipt(receipt: OfflineBearerCreditReceiptV2)
}

/** Fail-closed verifier used when an app has not configured trusted issuer keys. */
class RejectingOfflineBearerSignatureVerifier : OfflineBearerSignatureVerifying {
    override fun verifyPolicy(policy: OfflineBearerPolicyBundleV2) {
        throw OfflineBearerPolicyException("Offline Bearer issuer signature verifier is not configured")
    }

    override fun verifyCertificate(certificate: OfflineBearerCertificateV2, policy: OfflineBearerPolicyBundleV2) {
        throw OfflineBearerPolicyException("Offline Bearer issuer signature verifier is not configured")
    }

    override fun verifyReceiveRequest(request: OfflineBearerReceiveRequestV2) {
        throw OfflineBearerPolicyException("Offline Bearer device signature verifier is not configured")
    }

    override fun verifyDebitReceipt(receipt: OfflineBearerDebitReceiptV2) {
        throw OfflineBearerPolicyException("Offline Bearer device signature verifier is not configured")
    }

    override fun verifyCreditReceipt(receipt: OfflineBearerCreditReceiptV2) {
        throw OfflineBearerPolicyException("Offline Bearer device signature verifier is not configured")
    }
}

/** Ed25519 and P-256 verifier for issuer roots and hardware-purse public keys. */
class OfflineBearerSignatureVerifier(
    trustedIssuerPublicKeys: Collection<ByteArray>,
) : OfflineBearerSignatureVerifying {
    private val trustedIssuerPublicKeys = trustedIssuerPublicKeys.map { it.copyOf() }

    override fun verifyPolicy(policy: OfflineBearerPolicyBundleV2) {
        verifyIssuerSignature(
            policy.signatureAlgorithm,
            OfflineBearerV2Payloads.policyUnsignedPayload(policy),
            policy.issuerSignature(),
            "Offline Bearer policy issuer signature is invalid",
        )
    }

    override fun verifyCertificate(certificate: OfflineBearerCertificateV2, policy: OfflineBearerPolicyBundleV2) {
        if (certificate.issuerId != policy.issuerId ||
            !certificate.policyHashHex.equals(policy.policyHashHex, ignoreCase = true)
        ) {
            throw OfflineBearerPolicyException("Offline Bearer certificate does not match policy")
        }
        verifyIssuerSignature(
            policy.signatureAlgorithm,
            OfflineBearerV2Payloads.certificateUnsignedPayload(certificate),
            certificate.issuerSignature(),
            "Offline Bearer certificate issuer signature is invalid",
        )
    }

    override fun verifyReceiveRequest(request: OfflineBearerReceiveRequestV2) {
        if (request.signatureAlgorithm != request.recipientCertificate.signatureAlgorithm) {
            throw OfflineBearerPolicyException("Offline Bearer receive request algorithm does not match certificate")
        }
        verifyDeviceSignature(
            request.signatureAlgorithm,
            request.recipientCertificate.publicKey(),
            OfflineBearerV2Payloads.receiveRequestUnsignedPayload(request),
            request.challengeSignature(),
            "Offline Bearer receive request signature is invalid",
        )
    }

    override fun verifyDebitReceipt(receipt: OfflineBearerDebitReceiptV2) {
        if (receipt.signatureAlgorithm != receipt.senderCertificate.signatureAlgorithm) {
            throw OfflineBearerPolicyException("Offline Bearer debit receipt algorithm does not match certificate")
        }
        verifyDeviceSignature(
            receipt.signatureAlgorithm,
            receipt.senderCertificate.publicKey(),
            OfflineBearerV2Payloads.debitReceiptUnsignedPayload(receipt),
            receipt.debitSignature(),
            "Offline Bearer debit receipt signature is invalid",
        )
    }

    override fun verifyCreditReceipt(receipt: OfflineBearerCreditReceiptV2) {
        if (receipt.signatureAlgorithm != receipt.recipientCertificate.signatureAlgorithm) {
            throw OfflineBearerPolicyException("Offline Bearer credit receipt algorithm does not match certificate")
        }
        verifyDeviceSignature(
            receipt.signatureAlgorithm,
            receipt.recipientCertificate.publicKey(),
            OfflineBearerV2Payloads.creditReceiptUnsignedPayload(receipt),
            receipt.creditSignature(),
            "Offline Bearer credit receipt signature is invalid",
        )
    }

    private fun verifyIssuerSignature(
        algorithm: String,
        payload: ByteArray,
        signature: ByteArray,
        message: String,
    ) {
        if (trustedIssuerPublicKeys.none { verifySignature(algorithm, it, payload, signature) }) {
            throw OfflineBearerPolicyException(message)
        }
    }

    private fun verifyDeviceSignature(
        algorithm: String,
        publicKey: ByteArray,
        payload: ByteArray,
        signature: ByteArray,
        message: String,
    ) {
        if (!verifySignature(algorithm, publicKey, payload, signature)) {
            throw OfflineBearerPolicyException(message)
        }
    }

    private fun verifySignature(
        algorithm: String,
        publicKey: ByteArray,
        payload: ByteArray,
        signature: ByteArray,
    ): Boolean =
        when (algorithm) {
            OFFLINE_BEARER_SIGNATURE_ALGORITHM_ED25519 -> verifyEd25519(publicKey, payload, signature)
            OFFLINE_BEARER_SIGNATURE_ALGORITHM_ECDSA_P256_SHA256 -> verifyP256(publicKey, payload, signature)
            else -> false
        }

    private fun verifyEd25519(publicKey: ByteArray, payload: ByteArray, signature: ByteArray): Boolean =
        try {
            if (publicKey.size != 32) {
                false
            } else {
                val verifier = Ed25519Signer()
                verifier.init(false, Ed25519PublicKeyParameters(publicKey, 0))
                verifier.update(payload, 0, payload.size)
                verifier.verifySignature(signature)
            }
        } catch (_: RuntimeException) {
            false
        }

    private fun verifyP256(publicKey: ByteArray, payload: ByteArray, signature: ByteArray): Boolean =
        try {
            val params = ECNamedCurveTable.getByName("secp256r1") ?: return false
            val domain = ECDomainParameters(params.curve, params.g, params.n, params.h)
            val point = params.curve.decodePoint(publicKey)
            val key = ECPublicKeyParameters(point, domain)
            val sequence = ASN1Sequence.getInstance(signature)
            if (sequence.size() != 2) {
                return false
            }
            val r = (sequence.getObjectAt(0) as ASN1Integer).positiveValue
            val s = (sequence.getObjectAt(1) as ASN1Integer).positiveValue
            val digest = MessageDigest.getInstance("SHA-256").digest(payload)
            val verifier = ECDSASigner()
            verifier.init(false, key)
            verifier.verifySignature(digest, r, s)
        } catch (_: RuntimeException) {
            false
        } catch (_: IllegalArgumentException) {
            false
        }
}

/** Verifies exported Offline Bearer settlement batches before online submission. */
object OfflineBearerSettlementBatchVerifier {
    @JvmStatic
    @JvmOverloads
    fun verify(
        batch: OfflineBearerSettlementBatchV2,
        policy: OfflineBearerPolicyBundleV2,
        signatureVerifier: OfflineBearerSignatureVerifying,
        now: Long = System.currentTimeMillis(),
    ) {
        signatureVerifier.verifyPolicy(policy)
        requirePolicyFresh(policy, now)
        require(batch.policyHashMatches(policy)) {
            "settlement batch policy hash does not match current policy"
        }
        val creditTransferIds = batch.creditReceipts().map { it.transferId }.toSet()
        batch.debitReceipts().forEach { receipt ->
            verifyDebitReceipt(batch, receipt, policy, signatureVerifier, now, creditTransferIds)
        }
        val debitByTransferId = batch.debitReceipts().associateBy { it.transferId }
        batch.creditReceipts().forEach { receipt ->
            verifyCreditReceipt(batch, receipt, policy, signatureVerifier, now)
            val debit = requireNotNull(debitByTransferId[receipt.transferId]) {
                "settlement credit is missing its accepted debit receipt"
            }
            require(receipt.amount == debit.amount) {
                "settlement credit amount does not match debit receipt"
            }
            require(receipt.chainId == debit.chainId) {
                "settlement credit chainId does not match debit receipt"
            }
            require(receipt.recipientCertificate.purseId == debit.recipientCertificate.purseId) {
                "settlement credit recipient purse does not match debit receipt"
            }
            require(receipt.acceptedAtMs <= debit.expiresAtMs) {
                "settlement credit was accepted after debit receipt expiry"
            }
        }
    }

    private fun OfflineBearerSettlementBatchV2.policyHashMatches(
        policy: OfflineBearerPolicyBundleV2,
    ): Boolean =
        debitReceipts().all { it.policyHashHex.equals(policy.policyHashHex, ignoreCase = true) } &&
            creditReceipts().all {
                it.recipientCertificate.policyHashHex.equals(policy.policyHashHex, ignoreCase = true)
            }

    private fun verifyDebitReceipt(
        batch: OfflineBearerSettlementBatchV2,
        receipt: OfflineBearerDebitReceiptV2,
        policy: OfflineBearerPolicyBundleV2,
        signatureVerifier: OfflineBearerSignatureVerifying,
        now: Long,
        creditTransferIds: Set<String>,
    ) {
        require(receipt.chainId == batch.chainId) { "debit receipt chainId does not match settlement batch" }
        val senderExport = receipt.senderCertificate.purseId == batch.purseId
        val recipientExport = receipt.recipientCertificate.purseId == batch.purseId
        require(senderExport || recipientExport) {
            "debit receipt purse does not match settlement batch"
        }
        if (recipientExport && !senderExport) {
            require(creditTransferIds.contains(receipt.transferId)) {
                "receiver settlement batch must include a credit receipt for its accepted debit"
            }
        }
        require(receipt.createdAtMs <= now) { "debit receipt is from the future" }
        require(receipt.policyHashHex.equals(policy.policyHashHex, ignoreCase = true)) {
            "debit receipt policy hash does not match current policy"
        }
        require(!policy.revokedTransferIds.contains(receipt.transferId)) {
            "Offline Bearer transfer is revoked"
        }
        require(!policy.revokedTransferIds.contains(receipt.paymentRequestId)) {
            "Offline Bearer receive request is revoked"
        }
        require(receipt.senderCertificate.assetDefinitionId == receipt.assetDefinitionId) {
            "sender certificate asset does not match debit receipt"
        }
        require(receipt.recipientCertificate.assetDefinitionId == receipt.assetDefinitionId) {
            "recipient certificate asset does not match debit receipt"
        }
        requireAmountAtMost(receipt.amount, policy.maxTransactionAmount, "amount exceeds offline transaction policy")
        requireAmountAtMost(
            receipt.amount,
            maxTransactionAmountForAsset(policy, receipt.assetDefinitionId),
            "amount exceeds offline asset transaction policy",
        )
        require(
            decimal(receipt.senderPreBalance)
                .subtract(decimal(receipt.amount))
                .compareTo(decimal(receipt.senderPostBalance)) == 0,
        ) {
            "debit receipt balance transition is invalid"
        }
        enforceCertificatePolicyAt(receipt.senderCertificate, policy, receipt.createdAtMs, signatureVerifier)
        enforceCertificatePolicyAt(receipt.recipientCertificate, policy, receipt.createdAtMs, signatureVerifier)
        signatureVerifier.verifyDebitReceipt(receipt)
    }

    private fun verifyCreditReceipt(
        batch: OfflineBearerSettlementBatchV2,
        receipt: OfflineBearerCreditReceiptV2,
        policy: OfflineBearerPolicyBundleV2,
        signatureVerifier: OfflineBearerSignatureVerifying,
        now: Long,
    ) {
        require(receipt.chainId == batch.chainId) { "credit receipt chainId does not match settlement batch" }
        require(receipt.recipientCertificate.purseId == batch.purseId) {
            "credit receipt recipient purse does not match settlement batch"
        }
        require(receipt.acceptedAtMs <= now) { "credit receipt is from the future" }
        require(!policy.revokedTransferIds.contains(receipt.transferId)) {
            "Offline Bearer transfer is revoked"
        }
        requireAmountAtMost(receipt.amount, policy.maxTransactionAmount, "amount exceeds offline transaction policy")
        requireAmountAtMost(
            receipt.amount,
            maxTransactionAmountForAsset(policy, receipt.recipientCertificate.assetDefinitionId),
            "amount exceeds offline asset transaction policy",
        )
        require(
            decimal(receipt.recipientPreBalance)
                .add(decimal(receipt.amount))
                .compareTo(decimal(receipt.recipientPostBalance)) == 0,
        ) {
            "credit receipt balance transition is invalid"
        }
        enforceCertificatePolicyAt(receipt.recipientCertificate, policy, receipt.acceptedAtMs, signatureVerifier)
        signatureVerifier.verifyCreditReceipt(receipt)
    }

    private fun enforceCertificatePolicyAt(
        certificate: OfflineBearerCertificateV2,
        policy: OfflineBearerPolicyBundleV2,
        eventTimeMs: Long,
        signatureVerifier: OfflineBearerSignatureVerifying,
    ) {
        requirePolicyFresh(policy, eventTimeMs)
        if (certificate.issuedAtMs > eventTimeMs) {
            throw OfflineBearerPolicyException("Offline Bearer certificate is from the future")
        }
        if (certificate.expiresAtMs <= eventTimeMs) {
            throw OfflineBearerPolicyException("Offline Bearer certificate is expired")
        }
        if (eventTimeMs - certificate.issuedAtMs > policy.maxCertificateAgeMs) {
            throw OfflineBearerPolicyException("Offline Bearer certificate is too old")
        }
        if (certificate.issuerId != policy.issuerId) {
            throw OfflineBearerPolicyException("Offline Bearer certificate issuer does not match policy")
        }
        requireSupportedSignatureAlgorithm(certificate.signatureAlgorithm)
        requireSupportedPublicKeyEncoding(certificate.publicKeyEncoding)
        if (!certificate.policyHashHex.equals(policy.policyHashHex, ignoreCase = true)) {
            throw OfflineBearerPolicyException("Offline Bearer certificate policy hash does not match policy")
        }
        requireSupportedSignatureAlgorithm(certificate.signatureAlgorithm)
        requireSupportedPublicKeyEncoding(certificate.publicKeyEncoding)
        if (!policy.allowedHardwareClasses.contains(certificate.hardwareClass)) {
            throw OfflineBearerPolicyException("certificate hardware class is not allowed by policy")
        }
        if (policy.blacklistedAccountIds.contains(certificate.accountId) ||
            policy.blacklistedDeviceIds.contains(certificate.deviceId) ||
            policy.blacklistedKeyIds.contains(certificate.keyId) ||
            policy.revokedCertificateIds.contains(certificate.certificateId) ||
            policy.revokedCertificateIds.contains(certificate.keyId)
        ) {
            throw OfflineBearerPolicyException("Offline Bearer certificate is blacklisted")
        }
        signatureVerifier.verifyCertificate(certificate, policy)
    }

    private fun requirePolicyFresh(policy: OfflineBearerPolicyBundleV2, now: Long) {
        if (policy.issuedAtMs > now) {
            throw OfflineBearerPolicyException("Offline Bearer policy is from the future")
        }
        if (policy.expiresAtMs <= now) {
            throw OfflineBearerPolicyException("Offline Bearer policy is expired")
        }
        if (now - policy.issuedAtMs > policy.maxPolicyAgeMs) {
            throw OfflineBearerPolicyException("Offline Bearer policy is too old")
        }
    }
}

/** One-call v2 Offline Bearer wallet facade for hardware-backed local transfers. */
class OfflineBearerWallet @JvmOverloads constructor(
    private val chainId: String,
    private val accountId: String,
    private val secureElement: OfflineBearerSecureElement,
    private val policyProvider: OfflineBearerPolicyProvider,
    private val idGenerator: OfflineNoteIdGenerator = UuidOfflineNoteIdGenerator(),
    private val clock: java.util.function.LongSupplier = java.util.function.LongSupplier { System.currentTimeMillis() },
    private val signatureVerifier: OfflineBearerSignatureVerifying = RejectingOfflineBearerSignatureVerifier(),
) {
    init {
        requireNonBlank(chainId, "chainId")
        requireNonBlank(accountId, "accountId")
    }

    fun currentState(): OfflineBearerPurseStateV2? = secureElement.currentState()

    fun installLoadedPurse(certificate: OfflineBearerCertificateV2, state: OfflineBearerPurseStateV2) {
        val policy = currentVerifiedPolicy()
        val now = clock.getAsLong()
        requireHardwareUsable(policy)
        require(certificate.chainId == chainId) { "certificate chainId does not match wallet chainId" }
        require(certificate.accountId == accountId) { "certificate accountId does not match wallet accountId" }
        require(state.chainId == chainId) { "purse state chainId does not match wallet chainId" }
        require(state.accountId == accountId) { "purse state accountId does not match wallet accountId" }
        require(state.purseId == certificate.purseId) { "purse state does not match certificate purse" }
        require(state.assetDefinitionId == certificate.assetDefinitionId) {
            "purse state assetDefinitionId does not match certificate"
        }
        require(state.policyHashHex.equals(policy.policyHashHex, ignoreCase = true)) {
            "purse state policy hash does not match current policy"
        }
        require(certificate.policyHashHex.equals(policy.policyHashHex, ignoreCase = true)) {
            "certificate policy hash does not match current policy"
        }
        enforceCertificatePolicy(certificate, policy, now)
        requireAmountAtMost(state.balance, policy.maxOfflineBalance, "offline purse balance exceeds policy limit")
        secureElement.installPurse(certificate, state)
    }

    @JvmOverloads
    fun prepareReceive(assetDefinitionId: String, amount: String, ttlMs: Long = defaultTokenTtl()): OfflineBearerReceiveRequestV2 {
        val policy = currentVerifiedPolicy()
        val now = clock.getAsLong()
        requireHardwareUsable(policy)
        requirePolicyFresh(policy, now)
        val certificate = requireCurrentCertificate()
        val state = requireCurrentState()
        enforceCertificatePolicy(certificate, policy, now)
        require(assetDefinitionId == state.assetDefinitionId) { "assetDefinitionId does not match purse asset" }
        val canonicalAmount = OfflineCashCodec.canonicalAmountString(amount)
        requirePositiveAmount(canonicalAmount, "amount")
        requireAmountAtMost(canonicalAmount, policy.maxTransactionAmount, "amount exceeds offline transaction policy")
        requireAmountAtMost(
            canonicalAmount,
            maxTransactionAmountForAsset(policy, assetDefinitionId),
            "amount exceeds offline asset transaction policy",
        )
        val expiresAtMs = safeAdd(now, ttlMs.coerceAtMost(policy.maxTokenAgeMs))
        val request = secureElement.createReceiveRequest(
            paymentRequestId = idGenerator.nextId("offline-bearer-request"),
            amount = canonicalAmount,
            createdAtMs = now,
            expiresAtMs = expiresAtMs,
            policyHashHex = policy.policyHashHex,
        )
        signatureVerifier.verifyReceiveRequest(request)
        return request
    }

    @JvmOverloads
    fun pay(request: OfflineBearerReceiveRequestV2, ttlMs: Long = defaultTokenTtl()): OfflineBearerDebitReceiptV2 {
        val policy = currentVerifiedPolicy()
        val now = clock.getAsLong()
        requireHardwareUsable(policy)
        requirePolicyFresh(policy, now)
        validateReceiveRequest(request, policy, now)
        val senderCertificate = requireCurrentCertificate()
        enforceCertificatePolicy(senderCertificate, policy, now)
        require(senderCertificate.assetDefinitionId == request.assetDefinitionId) {
            "sender purse asset does not match receive request"
        }
        requireAmountAtMost(request.amount, policy.maxTransactionAmount, "amount exceeds offline transaction policy")
        requireAmountAtMost(
            request.amount,
            maxTransactionAmountForAsset(policy, request.assetDefinitionId),
            "amount exceeds offline asset transaction policy",
        )
        if (policy.revokedTransferIds.contains(request.paymentRequestId)) {
            throw OfflineBearerPolicyException("Offline Bearer receive request is revoked")
        }
        val receipt = secureElement.debit(
            request = request,
            transferId = idGenerator.nextId("offline-bearer-transfer"),
            createdAtMs = now,
            expiresAtMs = safeAdd(now, ttlMs.coerceAtMost(policy.maxTokenAgeMs)),
        )
        signatureVerifier.verifyDebitReceipt(receipt)
        return receipt
    }

    fun accept(receipt: OfflineBearerDebitReceiptV2): OfflineBearerCreditReceiptV2 {
        val policy = currentVerifiedPolicy()
        val now = clock.getAsLong()
        requireHardwareUsable(policy)
        requirePolicyFresh(policy, now)
        validateDebitReceipt(receipt, policy, now)
        val certificate = requireCurrentCertificate()
        val state = requireCurrentState()
        require(receipt.recipientCertificate.purseId == certificate.purseId) {
            "debit receipt is not addressed to this purse"
        }
        require(state.purseId == certificate.purseId && state.accountId == accountId) {
            "current purse state does not match wallet certificate"
        }
        require(state.assetDefinitionId == receipt.assetDefinitionId) {
            "current purse asset does not match debit receipt"
        }
        require(state.policyHashHex.equals(policy.policyHashHex, ignoreCase = true)) {
            "current purse policy hash does not match policy"
        }
        enforceCertificatePolicy(certificate, policy, now)
        requireAmountAtMost(
            decimal(state.balance).add(decimal(receipt.amount)).toPlainString(),
            policy.maxOfflineBalance,
            "offline purse balance exceeds policy limit",
        )
        val creditReceipt = secureElement.credit(receipt, now)
        signatureVerifier.verifyCreditReceipt(creditReceipt)
        return creditReceipt
    }

    @JvmOverloads
    fun exportSettlementBatch(maxReceipts: Int = 256): OfflineBearerSettlementBatchV2 {
        val policy = currentVerifiedPolicy()
        requireHardwareUsable(policy)
        require(maxReceipts > 0) { "maxReceipts must be positive" }
        return secureElement.exportSettlementBatch(maxReceipts).also { batch ->
            batch.debitReceipts().forEach(signatureVerifier::verifyDebitReceipt)
            batch.creditReceipts().forEach(signatureVerifier::verifyCreditReceipt)
        }
    }

    fun pruneSettled(transferIds: Collection<String>) {
        requireHardwareUsable(currentVerifiedPolicy())
        secureElement.pruneSettled(transferIds)
    }

    private fun validateReceiveRequest(
        request: OfflineBearerReceiveRequestV2,
        policy: OfflineBearerPolicyBundleV2,
        now: Long,
    ) {
        require(request.chainId == chainId) { "receive request chainId does not match wallet chainId" }
        require(request.expiresAtMs > now) { "receive request is expired" }
        require(request.createdAtMs <= now) { "receive request is from the future" }
        require(now - request.createdAtMs <= policy.maxTokenAgeMs) { "receive request is too old" }
        require(request.policyHashHex.equals(policy.policyHashHex, ignoreCase = true)) {
            "receive request policy hash does not match current policy"
        }
        enforceCertificatePolicy(request.recipientCertificate, policy, now)
        signatureVerifier.verifyReceiveRequest(request)
    }

    private fun validateDebitReceipt(
        receipt: OfflineBearerDebitReceiptV2,
        policy: OfflineBearerPolicyBundleV2,
        now: Long,
    ) {
        require(receipt.chainId == chainId) { "debit receipt chainId does not match wallet chainId" }
        require(receipt.expiresAtMs > now) { "debit receipt is expired" }
        require(receipt.createdAtMs <= now) { "debit receipt is from the future" }
        require(now - receipt.createdAtMs <= policy.maxTokenAgeMs) { "debit receipt is too old" }
        require(receipt.policyHashHex.equals(policy.policyHashHex, ignoreCase = true)) {
            "debit receipt policy hash does not match current policy"
        }
        require(receipt.recipientCertificate.accountId == accountId) {
            "debit receipt recipient account does not match wallet account"
        }
        if (policy.revokedTransferIds.contains(receipt.transferId)) {
            throw OfflineBearerPolicyException("Offline Bearer transfer is revoked")
        }
        require(receipt.senderCertificate.assetDefinitionId == receipt.assetDefinitionId) {
            "sender certificate asset does not match debit receipt"
        }
        require(receipt.recipientCertificate.assetDefinitionId == receipt.assetDefinitionId) {
            "recipient certificate asset does not match debit receipt"
        }
        enforceCertificatePolicy(receipt.senderCertificate, policy, now)
        enforceCertificatePolicy(receipt.recipientCertificate, policy, now)
        requireAmountAtMost(receipt.amount, policy.maxTransactionAmount, "amount exceeds offline transaction policy")
        signatureVerifier.verifyDebitReceipt(receipt)
    }

    private fun requireHardwareUsable(policy: OfflineBearerPolicyBundleV2) {
        val capabilities = secureElement.capabilities()
        if (!capabilities.hardwareBacked || !capabilities.statefulPurse) {
            throw OfflineBearerPolicyException("Offline Bearer requires a hardware-backed stateful purse")
        }
        if (capabilities.attestationKeyId.isNullOrBlank()) {
            throw OfflineBearerPolicyException("Offline Bearer requires a non-extractable hardware attestation key")
        }
        if (!capabilities.rollbackResistantState) {
            throw OfflineBearerPolicyException("Offline Bearer requires rollback-resistant purse state")
        }
        if (capabilities.attestationEvidence().isEmpty()) {
            throw OfflineBearerPolicyException("Offline Bearer requires secure-element attestation evidence")
        }
        requireSupportedSignatureAlgorithm(capabilities.signatureAlgorithm)
        requireSupportedPublicKeyEncoding(capabilities.publicKeyEncoding)
        if (!policy.allowedHardwareClasses.contains(capabilities.hardwareClass)) {
            throw OfflineBearerPolicyException("hardware class is not allowed by current Offline Bearer policy")
        }
    }

    private fun enforceCertificatePolicy(
        certificate: OfflineBearerCertificateV2,
        policy: OfflineBearerPolicyBundleV2,
        now: Long,
    ) {
        requirePolicyFresh(policy, now)
        if (certificate.issuedAtMs > now) {
            throw OfflineBearerPolicyException("Offline Bearer certificate is from the future")
        }
        if (certificate.expiresAtMs <= now) {
            throw OfflineBearerPolicyException("Offline Bearer certificate is expired")
        }
        if (now - certificate.issuedAtMs > policy.maxCertificateAgeMs) {
            throw OfflineBearerPolicyException("Offline Bearer certificate is too old")
        }
        if (certificate.issuerId != policy.issuerId) {
            throw OfflineBearerPolicyException("Offline Bearer certificate issuer does not match policy")
        }
        if (!certificate.policyHashHex.equals(policy.policyHashHex, ignoreCase = true)) {
            throw OfflineBearerPolicyException("Offline Bearer certificate policy hash does not match policy")
        }
        if (!policy.allowedHardwareClasses.contains(certificate.hardwareClass)) {
            throw OfflineBearerPolicyException("certificate hardware class is not allowed by policy")
        }
        if (policy.blacklistedAccountIds.contains(certificate.accountId) ||
            policy.blacklistedDeviceIds.contains(certificate.deviceId) ||
            policy.blacklistedKeyIds.contains(certificate.keyId) ||
            policy.revokedCertificateIds.contains(certificate.certificateId) ||
            policy.revokedCertificateIds.contains(certificate.keyId)
        ) {
            throw OfflineBearerPolicyException("Offline Bearer certificate is blacklisted")
        }
        signatureVerifier.verifyCertificate(certificate, policy)
    }

    private fun requirePolicyFresh(policy: OfflineBearerPolicyBundleV2, now: Long) {
        if (policy.issuedAtMs > now) {
            throw OfflineBearerPolicyException("Offline Bearer policy is from the future")
        }
        if (policy.expiresAtMs <= now) {
            throw OfflineBearerPolicyException("Offline Bearer policy is expired")
        }
        if (now - policy.issuedAtMs > policy.maxPolicyAgeMs) {
            throw OfflineBearerPolicyException("Offline Bearer policy is too old")
        }
    }

    private fun requireCurrentCertificate(): OfflineBearerCertificateV2 =
        secureElement.currentCertificate()
            ?: throw OfflineBearerPolicyException("Offline Bearer purse certificate is not installed")

    private fun requireCurrentState(): OfflineBearerPurseStateV2 =
        secureElement.currentState()
            ?: throw OfflineBearerPolicyException("Offline Bearer purse state is not installed")

    private fun defaultTokenTtl(): Long =
        currentVerifiedPolicy().maxTokenAgeMs

    private fun currentVerifiedPolicy(): OfflineBearerPolicyBundleV2 =
        policyProvider.currentPolicy().also(signatureVerifier::verifyPolicy)
}

private fun maxTransactionAmountForAsset(
    policy: OfflineBearerPolicyBundleV2,
    assetDefinitionId: String,
): String =
    policy.assetSendLimits
        .firstOrNull { it.assetDefinitionId == assetDefinitionId }
        ?.maxTransactionAmount
        ?: policy.maxTransactionAmount

private fun normalizedSet(values: Collection<String>): Set<String> {
    val set = LinkedHashSet<String>()
    values.forEach { value ->
        val trimmed = value.trim()
        if (trimmed.isNotEmpty()) {
            set.add(trimmed)
        }
    }
    return set
}

private fun requireNonBlank(value: String, field: String) {
    require(value.trim().isNotEmpty()) { "$field must not be blank" }
}

private fun requireHexLike(value: String, field: String) {
    require(value.trim().isNotEmpty()) { "$field must not be blank" }
    require(value.length % 2 == 0) { "$field must have an even number of hex characters" }
    require(value.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) {
        "$field must be hex"
    }
}

private fun requireSupportedSignatureAlgorithm(value: String) {
    require(
        value == OFFLINE_BEARER_SIGNATURE_ALGORITHM_ED25519 ||
            value == OFFLINE_BEARER_SIGNATURE_ALGORITHM_ECDSA_P256_SHA256,
    ) {
        "unsupported Offline Bearer signature algorithm"
    }
}

private fun requireSupportedPublicKeyEncoding(value: String) {
    require(
        value == OFFLINE_BEARER_PUBLIC_KEY_ENCODING_RAW_ED25519 ||
            value == OFFLINE_BEARER_PUBLIC_KEY_ENCODING_X963_P256,
    ) {
        "unsupported Offline Bearer public key encoding"
    }
}

private fun requirePositiveAmount(value: String, field: String) {
    require(decimal(value).signum() > 0) { "$field must be positive" }
}

private fun requireNonNegativeAmount(value: String, field: String) {
    require(decimal(value).signum() >= 0) { "$field must be non-negative" }
}

private fun requireAmountAtMost(value: String, max: String, message: String) {
    if (decimal(value).compareTo(decimal(max)) > 0) {
        throw OfflineBearerPolicyException(message)
    }
}

private fun decimal(value: String): BigDecimal =
    BigDecimal(OfflineCashCodec.canonicalAmountString(value))

private fun safeAdd(lhs: Long, rhs: Long): Long {
    require(rhs > 0) { "duration must be positive" }
    val maxDelta = Long.MAX_VALUE - lhs
    return if (rhs > maxDelta) Long.MAX_VALUE else lhs + rhs
}
