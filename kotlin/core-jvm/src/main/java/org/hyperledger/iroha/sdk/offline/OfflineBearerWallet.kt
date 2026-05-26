package org.hyperledger.iroha.sdk.offline

import java.math.BigDecimal
import java.util.LinkedHashSet

/** Capabilities reported by a hardware-backed Offline Bearer purse provider. */
class OfflineBearerSecureElementCapabilities @JvmOverloads constructor(
    val hardwareBacked: Boolean,
    val statefulPurse: Boolean,
    val hardwareClass: String,
    val attestationKeyId: String? = null,
) {
    init {
        require(hardwareClass.trim().isNotEmpty()) { "hardwareClass must not be blank" }
    }
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

/** One-call v2 Offline Bearer wallet facade for hardware-backed local transfers. */
class OfflineBearerWallet @JvmOverloads constructor(
    private val chainId: String,
    private val accountId: String,
    private val secureElement: OfflineBearerSecureElement,
    private val policyProvider: OfflineBearerPolicyProvider,
    private val idGenerator: OfflineNoteIdGenerator = UuidOfflineNoteIdGenerator(),
    private val clock: java.util.function.LongSupplier = java.util.function.LongSupplier { System.currentTimeMillis() },
) {
    init {
        requireNonBlank(chainId, "chainId")
        requireNonBlank(accountId, "accountId")
    }

    fun currentState(): OfflineBearerPurseStateV2? = secureElement.currentState()

    fun installLoadedPurse(certificate: OfflineBearerCertificateV2, state: OfflineBearerPurseStateV2) {
        val policy = policyProvider.currentPolicy()
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
        val policy = policyProvider.currentPolicy()
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
        return secureElement.createReceiveRequest(
            paymentRequestId = idGenerator.nextId("offline-bearer-request"),
            amount = canonicalAmount,
            createdAtMs = now,
            expiresAtMs = expiresAtMs,
            policyHashHex = policy.policyHashHex,
        )
    }

    @JvmOverloads
    fun pay(request: OfflineBearerReceiveRequestV2, ttlMs: Long = defaultTokenTtl()): OfflineBearerDebitReceiptV2 {
        val policy = policyProvider.currentPolicy()
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
        return secureElement.debit(
            request = request,
            transferId = idGenerator.nextId("offline-bearer-transfer"),
            createdAtMs = now,
            expiresAtMs = safeAdd(now, ttlMs.coerceAtMost(policy.maxTokenAgeMs)),
        )
    }

    fun accept(receipt: OfflineBearerDebitReceiptV2): OfflineBearerCreditReceiptV2 {
        val policy = policyProvider.currentPolicy()
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
        return secureElement.credit(receipt, now)
    }

    @JvmOverloads
    fun exportSettlementBatch(maxReceipts: Int = 256): OfflineBearerSettlementBatchV2 {
        val policy = policyProvider.currentPolicy()
        requireHardwareUsable(policy)
        require(maxReceipts > 0) { "maxReceipts must be positive" }
        return secureElement.exportSettlementBatch(maxReceipts)
    }

    fun pruneSettled(transferIds: Collection<String>) {
        requireHardwareUsable(policyProvider.currentPolicy())
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
    }

    private fun requireHardwareUsable(policy: OfflineBearerPolicyBundleV2) {
        val capabilities = secureElement.capabilities()
        if (!capabilities.hardwareBacked || !capabilities.statefulPurse) {
            throw OfflineBearerPolicyException("Offline Bearer requires a hardware-backed stateful purse")
        }
        if (capabilities.attestationKeyId.isNullOrBlank()) {
            throw OfflineBearerPolicyException("Offline Bearer requires a non-extractable hardware attestation key")
        }
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
        policyProvider.currentPolicy().maxTokenAgeMs
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
