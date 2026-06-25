package org.hyperledger.iroha.sdk.offline.wallet

import java.math.BigDecimal
import java.security.MessageDigest
import java.time.Instant
import java.util.Locale
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject

@Serializable
data class OfflineCompactKeyCertificate(
    val version: Int = 1,
    val platform: String,
    @SerialName("key_id") val keyId: String,
    @SerialName("device_id") val deviceId: String,
    @SerialName("account_id") val accountId: String,
    @SerialName("public_key") val publicKey: String,
    @SerialName("assertion_scheme") val assertionScheme: String = "android-keymint-ecdsa-p256-usage-limit-v1",
    @SerialName("assertion_key_algorithm") val assertionKeyAlgorithm: String = "ecdsa-p256-sha256",
    @SerialName("assertion_public_key") val assertionPublicKey: String = publicKey,
    @SerialName("assertion_usage_count_limit") val assertionUsageCountLimit: Int? = null,
    @SerialName("one_use") val oneUse: Boolean = true,
    @SerialName("issued_at_ms") val issuedAtMs: Long? = null,
    @SerialName("expires_at_ms") val expiresAtMs: Long? = null,
    @SerialName("app_attest_public_key_base64") val appAttestPublicKeyBase64: String? = null,
    @SerialName("ios_team_id") val iosTeamId: String? = null,
    @SerialName("ios_bundle_id") val iosBundleId: String? = null,
    @SerialName("ios_environment") val iosEnvironment: String? = null,
    @SerialName("issuer_signature_base64") val issuerSignatureBase64: String,
    @SerialName("issuer_signature_payload_base64") val issuerSignaturePayloadBase64: String? = null
)

@Serializable
data class OfflineRecursiveProof(
    @SerialName("verifier_key_backend") val verifierKeyBackend: String = "halo2/ipa",
    @SerialName("verifier_key_id")
    @Serializable(with = OfflineVerifierKeyIdSerializer::class)
    val verifierKeyId: String,
    @SerialName("proof_backend") val proofBackend: String = verifierKeyBackend,
    @SerialName("public_inputs_hash_hex") val publicInputsHashHex: String,
    @SerialName("proof_bytes_base64") val proofBytesBase64: String
)

private object OfflineVerifierKeyIdSerializer : KSerializer<String> {
    override val descriptor: SerialDescriptor =
        PrimitiveSerialDescriptor("OfflineVerifierKeyId", PrimitiveKind.STRING)

    override fun serialize(encoder: Encoder, value: String) {
        encoder.encodeString(value)
    }

    override fun deserialize(decoder: Decoder): String {
        val jsonDecoder = decoder as? JsonDecoder
            ?: return normalize(decoder.decodeString())
        return when (val element = jsonDecoder.decodeJsonElement()) {
            is JsonPrimitive -> normalize(element.content)
            is JsonObject -> {
                val name = element["name"] as? JsonPrimitive
                    ?: throw SerializationException("verifier_key_id object must contain name")
                normalize(name.content)
            }
            else -> throw SerializationException("verifier_key_id must be a string or object")
        }
    }

    private fun normalize(value: String): String {
        val trimmed = value.trim()
        val separator = trimmed.indexOf(':')
        return if (separator >= 0 && separator < trimmed.lastIndex) {
            trimmed.substring(separator + 1)
        } else {
            trimmed
        }
    }
}

@Serializable
data class OfflinePaymentTokenInputClaim(
    val domain: String,
    @SerialName("note_commitment") val noteCommitment: String,
    @SerialName("key_certificate_payload_hash") val keyCertificatePayloadHash: String,
    @SerialName("asset_id") val assetId: String,
    val amount: String,
    @SerialName("claim_hash") val claimHash: String? = null
)

@Serializable
data class OfflineDeviceBinding(
    val platform: String,
    @SerialName("attestation_key_id") val attestationKeyId: String,
    @SerialName("device_id") val deviceId: String,
    @SerialName("offline_public_key") val offlinePublicKey: String,
    @SerialName("attestation_report_base64") val attestationReportBase64: String,
    @SerialName("attestation_receipt") val attestationReceipt: OfflineAttestationReceipt? = null,
    @SerialName("device_key_algorithm") val deviceKeyAlgorithm: String? = null,
    @SerialName("device_public_key") val devicePublicKey: String? = null,
    @SerialName("device_attestation_report_base64") val deviceAttestationReportBase64: String? = null,
    @SerialName("assertion_scheme") val assertionScheme: String? = null,
    @SerialName("assertion_key_id") val assertionKeyId: String? = null,
    @SerialName("assertion_key_algorithm") val assertionKeyAlgorithm: String? = null,
    @SerialName("assertion_public_key") val assertionPublicKey: String? = null,
    @SerialName("assertion_attestation_report_base64") val assertionAttestationReportBase64: String? = null,
    @SerialName("assertion_usage_count_limit") val assertionUsageCountLimit: Int? = null,
    @SerialName("usage_count_limit") val usageCountLimit: Int? = null,
    @SerialName("ios_team_id") val iosTeamId: String? = null,
    @SerialName("ios_bundle_id") val iosBundleId: String? = null,
    @SerialName("ios_environment") val iosEnvironment: String? = null
)

@Serializable
data class OfflineAttestationReceipt(
    val version: Long,
    val platform: String,
    @SerialName("account_id") val accountId: String,
    @SerialName("device_id") val deviceId: String,
    @SerialName("offline_public_key_base64") val offlinePublicKeyBase64: String,
    @SerialName("assertion_public_key_base64") val assertionPublicKeyBase64: String,
    @SerialName("assertion_scheme") val assertionScheme: String,
    @SerialName("assertion_key_algorithm") val assertionKeyAlgorithm: String,
    @SerialName("attestation_key_id") val attestationKeyId: String,
    @SerialName("hardware_one_use") val hardwareOneUse: Boolean,
    @SerialName("attestation_report_hash_hex") val attestationReportHashHex: String,
    @SerialName("issued_at_ms") val issuedAtMs: Long,
    @SerialName("expires_at_ms") val expiresAtMs: Long,
    @SerialName("signature_base64") val signatureBase64: String
)

@Serializable
data class OfflineDeviceProof(
    val platform: String,
    @SerialName("attestation_key_id") val attestationKeyId: String,
    @SerialName("challenge_hash_hex") val challengeHashHex: String,
    @SerialName("assertion_base64") val assertionBase64: String,
    val counter: Long? = null
)

@Serializable
data class OfflineSpendAuthorization(
    @SerialName("authorization_id") val authorizationId: String,
    @SerialName("lineage_id") val lineageId: String,
    @SerialName("account_id") val accountId: String,
    @SerialName("verdict_id") val verdictId: String,
    @SerialName("max_balance") val policyMaxBalance: String,
    @SerialName("max_tx_value") val policyMaxTxValue: String,
    @SerialName("issued_at_ms") val issuedAtMs: Long,
    @SerialName("refresh_at_ms") val refreshAtMs: Long,
    @SerialName("expires_at_ms") val expiresAtMs: Long,
    @SerialName("device_binding") val deviceBinding: OfflineDeviceBinding,
    @SerialName("key_certificate") val keyCertificate: OfflineCompactKeyCertificate? = null,
    @SerialName("next_key_certificate") val nextKeyCertificate: OfflineCompactKeyCertificate? = null,
    @SerialName("issuer_signature_base64") val issuerSignatureBase64: String
)

@Serializable
data class OfflineCashStatePayload(
    @SerialName("lineage_id") val lineageId: String,
    @SerialName("account_id") val accountId: String,
    @SerialName("device_id") val deviceId: String,
    @SerialName("offline_public_key") val offlinePublicKey: String,
    @SerialName("asset_definition_id") val assetDefinitionId: String,
    val balance: String,
    @SerialName("locked_balance") val lockedBalance: String,
    @SerialName("server_revision") val serverRevision: Long,
    @SerialName("server_state_hash") val serverStateHash: String,
    @SerialName("pending_local_revision") val pendingLocalRevision: Long,
    val authorization: OfflineSpendAuthorization,
    @SerialName("issuer_signature_base64") val issuerSignatureBase64: String
)

@Serializable
data class OfflineRevocationBundlePayload(
    @SerialName("issued_at_ms") val issuedAtMs: Long,
    @SerialName("expires_at_ms") val expiresAtMs: Long,
    @SerialName("verdict_ids") val verdictIds: List<String> = emptyList(),
    @SerialName("blacklisted_account_ids") val blacklistedAccountIds: List<String> = emptyList(),
    @SerialName("asset_send_limits") val assetSendLimits: List<OfflineAssetSendLimit> = emptyList(),
    @SerialName("issuer_signature_base64") val issuerSignatureBase64: String
)

@Serializable
data class OfflineAssetSendLimit(
    @SerialName("asset_definition_id") val assetDefinitionId: String,
    @SerialName("daily_send_limit") val dailySendLimit: String,
    @SerialName("monthly_send_limit") val monthlySendLimit: String
)

@Serializable
data class OfflineTransferReceipt(
    val version: Int = 1,
    @SerialName("transfer_id") val transferId: String,
    val direction: String,
    @SerialName("lineage_id") val lineageId: String,
    @SerialName("account_id") val accountId: String,
    @SerialName("device_id") val deviceId: String,
    @SerialName("offline_public_key") val offlinePublicKey: String,
    @SerialName("pre_balance") val preBalance: String,
    @SerialName("post_balance") val postBalance: String,
    @SerialName("pre_locked_balance") val preLockedBalance: String,
    @SerialName("post_locked_balance") val postLockedBalance: String,
    @SerialName("pre_state_hash") val preStateHash: String,
    @SerialName("post_state_hash") val postStateHash: String,
    @SerialName("local_revision") val localRevision: Long,
    @SerialName("counterparty_lineage_id") val counterpartyLineageId: String,
    @SerialName("counterparty_account_id") val counterpartyAccountId: String,
    @SerialName("counterparty_device_id") val counterpartyDeviceId: String,
    @SerialName("counterparty_offline_public_key") val counterpartyOfflinePublicKey: String,
    val amount: String,
    val authorization: OfflineSpendAuthorization? = null,
    @SerialName("device_proof") val deviceProof: OfflineDeviceProof,
    @SerialName("sender_signature_base64") val senderSignatureBase64: String,
    @SerialName("created_at_ms") val createdAtMs: Long
)

@Serializable
data class OfflineTransferJournalEntry(
    @SerialName("transfer_id") val transferId: String,
    val direction: OfflineTransferDirection,
    @SerialName("counterparty_account_id") val counterpartyAccountId: String,
    val amount: String,
    @SerialName("created_at_ms") val createdAtMs: Long,
    val payload: String,
    @SerialName("sync_receipt") val syncReceipt: OfflineTransferReceipt? = null,
    val receipt: OfflineTransferReceipt? = null,
    @SerialName("receipt_ack_payload") val receiptAckPayload: String? = null,
    @SerialName("receipt_ack_received_at_ms") val receiptAckReceivedAtMs: Long? = null,
    @SerialName("synced_at_ms") val syncedAtMs: Long? = null
)

@Serializable
data class OfflineCashMutationHistoryEntry(
    @SerialName("operation_id") val operationId: String,
    val kind: String,
    val amount: String,
    @SerialName("chain_tx_hash") val chainTxHash: String,
    @SerialName("entry_hash") val entryHash: String,
    @SerialName("block_height") val blockHeight: Long,
    @SerialName("created_at_ms") val createdAtMs: Long
)

@Serializable
data class OfflineAssetTransferUsage(
    @SerialName("asset_definition_id") val assetDefinitionId: String,
    @SerialName("window_kind") val windowKind: String,
    @SerialName("window_key") val windowKey: String,
    val amount: String
)

@Serializable
enum class OfflineNoteRecordSource {
    @SerialName("issued")
    ISSUED,

    @SerialName("received")
    RECEIVED,

    @SerialName("change")
    CHANGE
}

@Serializable
enum class OfflineNoteRecordStatus {
    @SerialName("spendable")
    SPENDABLE,

    @SerialName("pending_redeem")
    PENDING_REDEEM,

    @SerialName("redeemed")
    REDEEMED,

    @SerialName("archived")
    ARCHIVED
}

@Serializable
data class OfflineNoteRecord(
    @SerialName("note_id") val noteId: String,
    @SerialName("commitment") val commitment: String,
    @SerialName("asset_definition_id") val assetDefinitionId: String,
    val amount: String,
    val source: OfflineNoteRecordSource,
    val status: OfflineNoteRecordStatus = OfflineNoteRecordStatus.SPENDABLE,
    @SerialName("issued_at_ms") val issuedAtMs: Long = System.currentTimeMillis(),
    @SerialName("updated_at_ms") val updatedAtMs: Long = System.currentTimeMillis(),
    @SerialName("token_id") val tokenId: String? = null,
    @SerialName("lineage_id") val lineageId: String? = null,
    @SerialName("key_certificate") val keyCertificate: OfflineCompactKeyCertificate? = null,
    @SerialName("input_claim") val inputClaim: OfflinePaymentTokenInputClaim? = null
)

@Serializable
data class OfflineOneUseKeyPoolState(
    @SerialName("capacity") val capacity: Int = 0,
    @SerialName("remaining_capacity") val remainingCapacity: Int = 0,
    @SerialName("available_key_ids") val availableKeyIds: List<String> = emptyList(),
    @SerialName("consumed_key_ids") val consumedKeyIds: List<String> = emptyList(),
    @SerialName("last_refill_at_ms") val lastRefillAtMs: Long? = null
)

@Serializable
data class OfflinePendingOutboxEntry(
    @SerialName("token_id") val tokenId: String,
    @SerialName("recipient_account_id") val recipientAccountId: String,
    @SerialName("asset_definition_id") val assetDefinitionId: String,
    val amount: String,
    val payload: String,
    @SerialName("created_at_ms") val createdAtMs: Long,
    @SerialName("receipt_ack_payload") val receiptAckPayload: String? = null,
    @SerialName("receipt_ack_received_at_ms") val receiptAckReceivedAtMs: Long? = null
)

@Serializable
data class OfflinePendingAuditReceipt(
    @SerialName("receipt_id") val receiptId: String,
    @SerialName("token_id") val tokenId: String,
    @SerialName("payment_token_norito_base64") val paymentTokenNoritoBase64: String? = null,
    @SerialName("bearer_settlement_batch_norito_base64") val bearerSettlementBatchNoritoBase64: String? = null,
    @SerialName("created_at_ms") val createdAtMs: Long,
    @SerialName("synced_at_ms") val syncedAtMs: Long? = null
)

@Serializable
data class OfflineRestoreQuarantineState(
    val reason: String,
    @SerialName("created_at_ms") val createdAtMs: Long = System.currentTimeMillis(),
    @SerialName("requires_offline_setup") val requiresOfflineSetup: Boolean = true
)

@Serializable
enum class OfflineTransferDirection {
    @SerialName("incoming")
    INCOMING,

    @SerialName("outgoing")
    OUTGOING
}

@Serializable
data class OfflineWalletState(
    @SerialName("schema_version") val schemaVersion: Int = OFFLINE_WALLET_STATE_SCHEMA_VERSION,
    @SerialName("account_id") val accountId: String,
    @SerialName("device_id") val deviceId: String,
    @SerialName("offline_public_key") val offlinePublicKey: String,
    @SerialName("attestation_key_id") val attestationKeyId: String? = null,
    @SerialName("android_device_key_alias") val androidDeviceKeyAlias: String? = null,
    val authorization: OfflineSpendAuthorization? = null,
    @SerialName("server_anchor") val serverAnchor: OfflineCashStatePayload? = null,
    @SerialName("local_balance") val localBalance: String = "0",
    @SerialName("locked_balance") val lockedBalance: String = "0",
    @SerialName("local_revision") val localRevision: Long = 0,
    @SerialName("local_state_hash") val localStateHash: String = "",
    @SerialName("attestation_counter") val attestationCounter: Long = 0,
    @SerialName("received_transfer_ids") val receivedTransferIds: List<String> = emptyList(),
    @SerialName("source_nullifiers") val sourceNullifiers: List<String> = emptyList(),
    val journal: List<OfflineTransferJournalEntry> = emptyList(),
    @SerialName("mutation_history") val mutationHistory: List<OfflineCashMutationHistoryEntry> = emptyList(),
    @SerialName("asset_transfer_usage") val assetTransferUsage: List<OfflineAssetTransferUsage> = emptyList(),
    @SerialName("note_records") val noteRecords: List<OfflineNoteRecord> = emptyList(),
    @SerialName("one_use_key_pool_state") val oneUseKeyPoolState: OfflineOneUseKeyPoolState = OfflineOneUseKeyPoolState(),
    @SerialName("key_certificate_reserve") val keyCertificateReserve: List<OfflineCompactKeyCertificate> = emptyList(),
    @SerialName("pending_outbox") val pendingOutbox: List<OfflinePendingOutboxEntry> = emptyList(),
    @SerialName("pending_audit_receipts") val pendingAuditReceipts: List<OfflinePendingAuditReceipt> = emptyList(),
    @SerialName("restore_quarantine_state") val restoreQuarantineState: OfflineRestoreQuarantineState? = null,
    @SerialName("revocation_bundle") val revocationBundle: OfflineRevocationBundlePayload? = null,
    @SerialName("frozen_reason") val frozenReason: String? = null
)

const val OFFLINE_WALLET_STATE_SCHEMA_VERSION: Int = 5

data class OfflineWalletOverview(
    val state: OfflineWalletState,
    val serverBalance: BigDecimal,
    val localBalance: BigDecimal,
    val spendableAmount: BigDecimal,
    val lockedAmount: BigDecimal,
    val sendEnabled: Boolean,
    val receiveEnabled: Boolean,
    val requiresRefresh: Boolean,
    val revoked: Boolean,
    val revocationFresh: Boolean,
    val authorizationDeadline: Instant?,
    val hasPendingTransfers: Boolean
)

data class OfflineNoteDebitSelection(
    val record: OfflineNoteRecord,
    val debitAmount: String,
    val changeAmount: String
)

object BearerOfflineWalletPolicy {
    private val canonicalJson = Json {
        encodeDefaults = true
        explicitNulls = false
    }

    fun balanceAmount(value: String?): BigDecimal {
        return value?.let(::parseNonNegativeAsciiDecimal) ?: BigDecimal.ZERO
    }

    fun normalizeAmountString(value: String): String {
        val amount = parseNonNegativeAsciiDecimal(value) ?: return "0"
        if (amount.compareTo(BigDecimal.ZERO) == 0) return "0"
        return amount.stripTrailingZeros().toPlainString()
    }

    fun isPositiveAmountString(value: String): Boolean {
        return parseNonNegativeAsciiDecimal(value)?.let { it > BigDecimal.ZERO } == true
    }

    fun isNonNegativeAmountString(value: String): Boolean {
        return parseNonNegativeAsciiDecimal(value) != null
    }

    private fun parseNonNegativeAsciiDecimal(value: String): BigDecimal? {
        val trimmed = value.trim()
        if (trimmed.isBlank() || trimmed.none { it in '0'..'9' }) return null
        if (trimmed.any { it !in '0'..'9' && it != '.' }) return null
        if (trimmed.count { it == '.' } > 1) return null
        val decimalIndex = trimmed.indexOf('.')
        if (decimalIndex == 0 || decimalIndex == trimmed.lastIndex) return null
        return trimmed.toBigDecimalOrNull()?.takeIf { it >= BigDecimal.ZERO }
    }

    fun normalizeState(state: OfflineWalletState): OfflineWalletState {
        val anchor = state.serverAnchor
        val authorization = state.authorization ?: anchor?.authorization
        val normalizedBalance = when {
            state.localBalance.trim().isNotBlank() -> normalizeAmountString(state.localBalance)
            anchor != null -> normalizeAmountString(anchor.balance)
            else -> "0"
        }
        val normalizedLockedBalance = when {
            state.lockedBalance.trim().isNotBlank() -> normalizeAmountString(state.lockedBalance)
            anchor != null -> normalizeAmountString(anchor.lockedBalance)
            else -> "0"
        }
        val normalizedRevision = when {
            state.localRevision > 0L -> state.localRevision
            anchor != null -> anchor.pendingLocalRevision
            else -> 0L
        }
        val normalizedStateHash = when {
            state.localStateHash.trim().isNotBlank() -> state.localStateHash.trim().lowercase(Locale.ROOT)
            anchor != null -> anchor.serverStateHash.trim().lowercase(Locale.ROOT)
            else -> ""
        }
        val normalizedUsage = state.assetTransferUsage
            .map { usage ->
                usage.copy(
                    assetDefinitionId = usage.assetDefinitionId.trim(),
                    windowKind = usage.windowKind.trim().lowercase(Locale.ROOT),
                    windowKey = usage.windowKey.trim(),
                    amount = normalizeAmountString(usage.amount)
                )
            }
            .filter { usage ->
                usage.assetDefinitionId.isNotBlank() &&
                    usage.windowKind.isNotBlank() &&
                    usage.windowKey.isNotBlank()
            }
            .distinctBy { usage -> Triple(usage.assetDefinitionId, usage.windowKind, usage.windowKey) }
        val normalizedOneUseKeys = state.oneUseKeyPoolState.let { pool ->
            val available = pool.availableKeyIds.map { it.trim() }.filter { it.isNotBlank() }.distinct()
            val consumed = pool.consumedKeyIds.map { it.trim() }.filter { it.isNotBlank() }.distinct()
            pool.copy(
                capacity = maxOf(pool.capacity, available.size + consumed.size),
                remainingCapacity = available.size,
                availableKeyIds = available,
                consumedKeyIds = consumed
            )
        }
        val activeCertificateIds = listOfNotNull(
            authorization?.keyCertificate?.keyId,
            authorization?.nextKeyCertificate?.keyId
        )
            .map { it.trim() }
            .filter { it.isNotBlank() }
            .toSet()
        val consumedCertificateIds = normalizedOneUseKeys.consumedKeyIds.toSet()
        val normalizedReserve = state.keyCertificateReserve
            .filter { certificate ->
                val keyId = certificate.keyId.trim()
                keyId.isNotBlank() &&
                    keyId !in activeCertificateIds &&
                    keyId !in consumedCertificateIds &&
                    accountIdsMatchForOffline(certificate.accountId, state.accountId) &&
                    certificate.deviceId == state.deviceId &&
                    certificate.publicKey == state.offlinePublicKey
            }
            .distinctBy { it.keyId.trim() }
            .takeLast(64)
        return state.copy(
            schemaVersion = OFFLINE_WALLET_STATE_SCHEMA_VERSION,
            attestationKeyId = state.attestationKeyId?.trim()?.takeIf { it.isNotBlank() }
                ?: authorization?.deviceBinding?.attestationKeyId?.trim()?.takeIf { it.isNotBlank() },
            androidDeviceKeyAlias = state.androidDeviceKeyAlias?.trim()?.takeIf { it.isNotBlank() },
            authorization = authorization,
            localBalance = normalizedBalance,
            lockedBalance = normalizedLockedBalance,
            localRevision = normalizedRevision,
            localStateHash = normalizedStateHash,
            receivedTransferIds = state.receivedTransferIds.distinct(),
            sourceNullifiers = state.sourceNullifiers
                .map { it.trim().lowercase(Locale.ROOT) }
                .filter { it.isNotBlank() }
                .distinct(),
            mutationHistory = state.mutationHistory
                .sortedByDescending { it.createdAtMs }
                .distinctBy { it.operationId }
                .take(20),
            assetTransferUsage = normalizedUsage,
            noteRecords = state.noteRecords
                .filter { it.noteId.isNotBlank() && it.commitment.isNotBlank() }
                .distinctBy { it.noteId }
                .takeLast(512),
            oneUseKeyPoolState = normalizedOneUseKeys,
            keyCertificateReserve = normalizedReserve,
            pendingOutbox = state.pendingOutbox
                .filter { it.tokenId.isNotBlank() }
                .distinctBy { it.tokenId }
                .takeLast(512),
            pendingAuditReceipts = state.pendingAuditReceipts
                .filter { it.receiptId.isNotBlank() }
                .distinctBy { it.receiptId }
                .takeLast(512)
        )
    }

    fun authorizationDeadline(authorization: OfflineSpendAuthorization?): Instant? {
        return authorization?.expiresAtMs?.takeIf { it > 0L }?.let(Instant::ofEpochMilli)
    }

    fun isRevoked(
        authorization: OfflineSpendAuthorization?,
        revocationBundle: OfflineRevocationBundlePayload?,
    ): Boolean {
        val verdictId = authorization?.verdictId?.trim().orEmpty()
        if (verdictId.isBlank()) return false
        return revocationBundle?.verdictIds.orEmpty().any { it.equals(verdictId, ignoreCase = true) }
    }

    fun hasFreshRevocationBundle(
        revocationBundle: OfflineRevocationBundlePayload?,
        now: Instant = Instant.now()
    ): Boolean {
        val bundle = revocationBundle ?: return false
        return bundle.expiresAtMs > now.toEpochMilli()
    }

    fun isSendActive(
        authorization: OfflineSpendAuthorization?,
        revocationBundle: OfflineRevocationBundlePayload?,
        now: Instant = Instant.now()
    ): Boolean {
        val auth = authorization ?: return false
        val nowMs = now.toEpochMilli()
        if (!isAuthorizationWindowActive(auth, nowMs)) return false
        if (revocationBundle != null) {
            if (!hasFreshRevocationBundle(revocationBundle, now)) return false
            if (isRevoked(auth, revocationBundle)) return false
            if (isAccountBlacklisted(auth.accountId, revocationBundle)) return false
        }
        return true
    }

    fun isAuthorizationWindowActive(
        authorization: OfflineSpendAuthorization?,
        now: Instant = Instant.now()
    ): Boolean = authorization?.let { isAuthorizationWindowActive(it, now.toEpochMilli()) } ?: false

    fun isAuthorizationWindowActive(
        authorization: OfflineSpendAuthorization,
        nowMs: Long
    ): Boolean = authorization.issuedAtMs <= nowMs && nowMs < authorization.expiresAtMs

    fun isAccountBlacklisted(
        accountId: String?,
        revocationBundle: OfflineRevocationBundlePayload?
    ): Boolean {
        val normalized = accountId?.trim().orEmpty()
        if (normalized.isBlank()) return false
        return revocationBundle?.blacklistedAccountIds.orEmpty().any { it == normalized }
    }

    fun assetSendLimit(
        assetDefinitionId: String?,
        revocationBundle: OfflineRevocationBundlePayload?,
        now: Instant = Instant.now()
    ): OfflineAssetSendLimit? {
        if (!hasFreshRevocationBundle(revocationBundle, now)) return null
        val normalized = assetDefinitionId?.trim().orEmpty()
        if (normalized.isBlank()) return null
        return revocationBundle?.assetSendLimits.orEmpty().firstOrNull {
            it.assetDefinitionId == normalized
        }
    }

    fun recordOutgoingUsage(
        state: OfflineWalletState,
        assetDefinitionId: String,
        amount: String,
        createdAtMs: Long
    ): List<OfflineAssetTransferUsage> {
        val normalizedAmount = normalizeAmountString(amount)
        val utcTime = Instant.ofEpochMilli(createdAtMs).atOffset(java.time.ZoneOffset.UTC)
        val windows = listOf(
            "daily" to utcTime.toLocalDate().toString(),
            "monthly" to String.format(
                Locale.ROOT,
                "%04d-%02d",
                utcTime.year,
                utcTime.monthValue
            )
        )
        val usageMap = state.assetTransferUsage.associateBy {
            Triple(it.assetDefinitionId, it.windowKind, it.windowKey)
        }.toMutableMap()
        windows.forEach { (windowKind, windowKey) ->
            val usageKey = Triple(assetDefinitionId, windowKind, windowKey)
            val existing = usageMap[usageKey]
            val currentAmount = balanceAmount(existing?.amount)
            usageMap[usageKey] = OfflineAssetTransferUsage(
                assetDefinitionId = assetDefinitionId,
                windowKind = windowKind,
                windowKey = windowKey,
                amount = normalizeAmountString(
                    currentAmount.add(balanceAmount(normalizedAmount)).toPlainString()
                )
            )
        }
        return usageMap.values.sortedWith(
            compareBy<OfflineAssetTransferUsage>({ it.assetDefinitionId }, { it.windowKind }, { it.windowKey })
        )
    }

    fun usageAmountForWindow(
        state: OfflineWalletState,
        assetDefinitionId: String,
        windowKind: String,
        windowKey: String
    ): BigDecimal {
        return state.assetTransferUsage.firstOrNull {
            it.assetDefinitionId == assetDefinitionId &&
                it.windowKind == windowKind &&
                it.windowKey == windowKey
        }?.amount?.let(::balanceAmount) ?: BigDecimal.ZERO
    }

    fun spendableAmount(state: OfflineWalletState, now: Instant = Instant.now()): BigDecimal {
        if (!isSendActive(state.authorization, state.revocationBundle, now)) return BigDecimal.ZERO
        val local = balanceAmount(state.localBalance)
        val locked = lockedAmount(state, now)
        return local.subtract(locked).max(BigDecimal.ZERO)
    }

    fun selectSpendableNoteRecordsForBalance(
        state: OfflineWalletState,
        balance: String,
        maxInputs: Int = 4
    ): List<OfflineNoteRecord>? {
        val target = balanceAmount(balance)
        if (target <= BigDecimal.ZERO) return emptyList()
        val spendableRecords = state.noteRecords
            .filter { it.status == OfflineNoteRecordStatus.SPENDABLE }
            .mapNotNull { record ->
                val amount = balanceAmount(record.amount)
                if (amount > BigDecimal.ZERO) record to amount else null
            }
            .sortedBy { (record, _) -> record.updatedAtMs }
        if (spendableRecords.isEmpty()) return emptyList()
        spendableRecords.firstOrNull { (_, amount) -> amount.compareTo(target) == 0 }?.let { (record, _) ->
            return listOf(record)
        }

        val boundedMaxInputs = maxInputs.coerceAtLeast(1)
        val selectionsByAmount = LinkedHashMap<String, List<OfflineNoteRecord>>()
        selectionsByAmount[normalizeAmountString(BigDecimal.ZERO.toPlainString())] = emptyList()
        spendableRecords.forEach { (record, amount) ->
            val snapshot = selectionsByAmount.toList()
            snapshot.forEach { (sumKey, selectedRecords) ->
                if (selectedRecords.size >= boundedMaxInputs) return@forEach
                val sum = balanceAmount(sumKey)
                val nextSum = sum.add(amount)
                if (nextSum > target) return@forEach
                val nextKey = normalizeAmountString(nextSum.toPlainString())
                if (nextKey !in selectionsByAmount) {
                    selectionsByAmount[nextKey] = selectedRecords + record
                }
            }
        }
        return selectionsByAmount[normalizeAmountString(target.toPlainString())]
    }

    fun selectSpendableNoteRecordForDebit(
        state: OfflineWalletState,
        amount: String
    ): OfflineNoteDebitSelection? {
        val target = balanceAmount(amount)
        if (target <= BigDecimal.ZERO) return null
        return state.noteRecords
            .filter { it.status == OfflineNoteRecordStatus.SPENDABLE }
            .mapNotNull { record ->
                val noteAmount = balanceAmount(record.amount)
                if (noteAmount >= target) record to noteAmount else null
            }
            .sortedWith(
                compareBy<Pair<OfflineNoteRecord, BigDecimal>>(
                    { (_, noteAmount) -> noteAmount },
                    { (record, _) -> record.updatedAtMs }
                )
            )
            .firstOrNull()
            ?.let { (record, noteAmount) ->
                OfflineNoteDebitSelection(
                    record = record,
                    debitAmount = normalizeAmountString(target.toPlainString()),
                    changeAmount = normalizeAmountString(noteAmount.subtract(target).toPlainString())
                )
            }
    }

    fun lockedAmount(state: OfflineWalletState, now: Instant = Instant.now()): BigDecimal {
        val normalized = normalizeState(state)
        val local = balanceAmount(normalized.localBalance)
        val stored = balanceAmount(normalized.lockedBalance)
        val authorization = normalized.authorization
        val required = when {
            authorization == null -> local
            !isSendActive(authorization, normalized.revocationBundle, now) -> local
            else -> {
                val maxBalance = balanceAmount(authorization.policyMaxBalance)
                local.subtract(maxBalance).max(BigDecimal.ZERO)
            }
        }
        return stored.max(required).min(local)
    }

    fun applyOutgoingBalance(state: OfflineWalletState, amount: String): OfflineWalletState {
        val normalizedState = normalizeState(state)
        val requested = balanceAmount(amount)
        val current = balanceAmount(normalizedState.localBalance)
        if (requested <= BigDecimal.ZERO || requested > current) {
            throw IllegalStateException("Amount exceeds remaining offline balance")
        }
        val nextBalance = current.subtract(requested)
        val nextRevision = normalizedState.localRevision + 1L
        val lineageId = normalizedState.serverAnchor?.lineageId
            ?: normalizedState.authorization?.lineageId
            ?: throw IllegalStateException("Offline cash is not initialized")
        val nextLockedBalance = normalizeAmountString(
            lockedAmount(
                normalizedState.copy(localBalance = normalizeAmountString(nextBalance.toPlainString()))
            ).toPlainString()
        )
        return normalizedState.copy(
            localBalance = normalizeAmountString(nextBalance.toPlainString()),
            lockedBalance = nextLockedBalance,
            localRevision = nextRevision,
            localStateHash = nextLocalStateHash(
                lineageId = lineageId,
                previousStateHash = normalizedState.localStateHash,
                transferId = "",
                direction = "outgoing",
                counterpartyLineageId = "",
                amount = normalizeAmountString(requested.toPlainString()),
                localRevision = nextRevision,
                postBalance = normalizeAmountString(nextBalance.toPlainString()),
                postLockedBalance = nextLockedBalance
            )
        )
    }

    fun applyIncomingBalance(state: OfflineWalletState, amount: String): OfflineWalletState {
        val normalizedState = normalizeState(state)
        val incoming = balanceAmount(amount)
        if (incoming <= BigDecimal.ZERO) {
            throw IllegalStateException("Invalid amount")
        }
        val nextBalance = balanceAmount(normalizedState.localBalance).add(incoming)
        val nextRevision = normalizedState.localRevision + 1L
        val lineageId = normalizedState.serverAnchor?.lineageId
            ?: normalizedState.authorization?.lineageId
            ?: throw IllegalStateException("Offline cash is not initialized")
        val nextLockedBalance = normalizeAmountString(
            lockedAmount(
                normalizedState.copy(localBalance = normalizeAmountString(nextBalance.toPlainString()))
            ).toPlainString()
        )
        return normalizedState.copy(
            localBalance = normalizeAmountString(nextBalance.toPlainString()),
            lockedBalance = nextLockedBalance,
            localRevision = nextRevision,
            localStateHash = nextLocalStateHash(
                lineageId = lineageId,
                previousStateHash = normalizedState.localStateHash,
                transferId = "",
                direction = "incoming",
                counterpartyLineageId = "",
                amount = normalizeAmountString(incoming.toPlainString()),
                localRevision = nextRevision,
                postBalance = normalizeAmountString(nextBalance.toPlainString()),
                postLockedBalance = nextLockedBalance
            )
        )
    }

    fun nextLocalStateHash(
        lineageId: String,
        previousStateHash: String,
        transferId: String,
        direction: String,
        counterpartyLineageId: String,
        amount: String,
        localRevision: Long,
        postBalance: String,
        postLockedBalance: String
    ): String {
        val payload = buildJsonObject {
            put("amount", JsonPrimitive(normalizeAmountString(amount)))
            put("counterparty_lineage_id", JsonPrimitive(counterpartyLineageId))
            put("direction", JsonPrimitive(direction))
            put("lineage_id", JsonPrimitive(lineageId))
            put("local_revision", JsonPrimitive(localRevision))
            put("post_balance", JsonPrimitive(normalizeAmountString(postBalance)))
            put("post_locked_balance", JsonPrimitive(normalizeAmountString(postLockedBalance)))
            put("previous_state_hash", JsonPrimitive(previousStateHash.lowercase(Locale.ROOT)))
            put("transfer_id", JsonPrimitive(transferId))
        }
        return sha256Hex(canonicalJsonBytes(payload))
    }

    fun challengeHashHex(
        lineageId: String,
        transferId: String,
        amount: String,
        direction: String,
        counterpartyLineageId: String,
        accountId: String
    ): String {
        val operation = if (direction == "incoming") "receive" else "send"
        val transferPayload = buildJsonObject {
            put("amount", JsonPrimitive(normalizeAmountString(amount)))
            put("lineage_id", JsonPrimitive(lineageId))
            put("transfer_id", JsonPrimitive(transferId))
            if (direction == "incoming") {
                put("sender_lineage_id", JsonPrimitive(counterpartyLineageId))
            } else {
                put("receiver_lineage_id", JsonPrimitive(counterpartyLineageId))
            }
        }
        val seed = buildJsonObject {
            put("account_id", JsonPrimitive(accountId))
            put("lineage_id", JsonPrimitive(lineageId))
            put("operation", JsonPrimitive(operation))
            put("payload_hash", JsonPrimitive(sha256Hex(canonicalJsonBytes(transferPayload))))
        }
        return sha256Hex(canonicalJsonBytes(seed))
    }

    private fun sha256Hex(bytes: ByteArray): String {
        return MessageDigest.getInstance("SHA-256")
            .digest(bytes)
            .joinToString(separator = "") { byte -> "%02x".format(byte) }
    }

    private fun canonicalJsonBytes(value: kotlinx.serialization.json.JsonElement): ByteArray {
        return canonicalJson.encodeToString(
            kotlinx.serialization.json.JsonElement.serializer(),
            sortJson(value)
        ).toByteArray(Charsets.UTF_8)
    }

    private fun sortJson(value: kotlinx.serialization.json.JsonElement): kotlinx.serialization.json.JsonElement {
        return when (value) {
            is JsonObject -> JsonObject(value.entries.sortedBy { it.key }.associate { (key, child) ->
                key to sortJson(child)
            })
            is JsonArray -> JsonArray(value.map(::sortJson))
            else -> value
        }
    }
}

private fun accountIdsMatchForOffline(lhs: String?, rhs: String?): Boolean {
    val left = lhs?.trim().orEmpty()
    val right = rhs?.trim().orEmpty()
    return left.isNotBlank() && right.isNotBlank() && left == right
}
