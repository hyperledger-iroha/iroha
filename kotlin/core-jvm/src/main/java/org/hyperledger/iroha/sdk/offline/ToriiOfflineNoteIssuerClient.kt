package org.hyperledger.iroha.sdk.offline

import java.net.URI
import java.nio.charset.StandardCharsets
import java.time.Duration
import java.util.Base64
import java.util.Collections
import java.util.LinkedHashMap
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.function.LongSupplier
import org.hyperledger.iroha.sdk.client.CanonicalRequestSigner
import org.hyperledger.iroha.sdk.client.ClientObserver
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.HttpErrorMessageExtractor
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.PlatformHttpTransportExecutor
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth
import org.hyperledger.iroha.sdk.client.transport.TransportRequest

/** Device-binding material required by the Torii Offline Note issuer endpoints. */
class OfflineNoteIssuerDeviceBinding(
    val deviceId: String,
    val offlinePublicKey: String,
    deviceBinding: Map<String, Any?>,
) {
    private val _deviceBinding = deepCopyObject(deviceBinding)

    init {
        require(deviceId.trim().isNotEmpty()) { "deviceId must not be blank" }
        require(offlinePublicKey.trim().isNotEmpty()) { "offlinePublicKey must not be blank" }
        (_deviceBinding["device_id"] as? String)?.let {
            require(it == deviceId) { "device_binding.device_id must match deviceId" }
        }
        (_deviceBinding["offline_public_key"] as? String)?.let {
            require(it == offlinePublicKey) {
                "device_binding.offline_public_key must match offlinePublicKey"
            }
        }
    }

    fun attestationKeyId(): String =
        (_deviceBinding["attestation_key_id"] as? String)
            ?.trim()
            ?.takeIf { it.isNotEmpty() }
            ?: throw IllegalStateException("device_binding.attestation_key_id is required")

    fun deviceBinding(): Map<String, Any?> = deepCopyObject(_deviceBinding)
}

/** Supplies the current issuer device binding and attestation receipt. */
interface OfflineNoteIssuerDeviceBindingProvider {
    fun currentDeviceBinding(
        chainId: String,
        accountId: String,
        assetDefinitionId: String,
    ): OfflineNoteIssuerDeviceBinding
}

/** Torii-backed issuer client for Offline Note wallet loads. */
class ToriiOfflineNoteIssuerClient @JvmOverloads constructor(
    private val canonicalAuth: ToriiCanonicalRequestAuth,
    private val deviceBindingProvider: OfflineNoteIssuerDeviceBindingProvider,
    private val executor: HttpTransportExecutor = PlatformHttpTransportExecutor.createDefault(),
    private val baseUri: URI = URI.create("http://localhost:8080"),
    private val timeout: Duration? = Duration.ofSeconds(15),
    defaultHeaders: Map<String, String> = emptyMap(),
    observers: List<ClientObserver> = emptyList(),
    private val clock: LongSupplier = LongSupplier { System.currentTimeMillis() },
    private val nonceGenerator: OfflineNoteIdGenerator = UuidOfflineNoteIdGenerator(),
) : OfflineNoteIssuerClient {
    private val defaultHeaders: Map<String, String> =
        Collections.unmodifiableMap(LinkedHashMap(defaultHeaders))
    private val observers: List<ClientObserver> = observers.toList()
    private val pendingLoads = LinkedHashMap<String, PendingLoad>()
    private val lineageStates = LinkedHashMap<String, StoredLineageState>()

    override fun prepareLoad(
        chainId: String,
        accountId: String,
        assetDefinitionId: String,
        amount: String,
    ): CompletableFuture<OfflineNoteLoadContext> {
        require(canonicalAuth.accountId == accountId) {
            "canonical auth accountId must match wallet accountId"
        }
        val binding = deviceBindingProvider.currentDeviceBinding(chainId, accountId, assetDefinitionId)
        val lineageKey = lineageKey(accountId, assetDefinitionId, binding)
        val cached = synchronized(this) { lineageStates[lineageKey]?.takeIf { !it.isExpired(clock.getAsLong()) } }
        return if (cached == null) {
            refillKeys(chainId, accountId, assetDefinitionId, binding, lineageKey)
        } else {
            val operationId = nonceGenerator.nextId("offline-load")
            val pending = PendingLoad(
                operationId = operationId,
                lineageKey = lineageKey,
                lineageId = cached.lineageId,
                preIssueRevision = cached.revision,
                localBalance = cached.balance,
                keyCertificate = cached.keyCertificate,
                lineageState = cached.lineageState,
                deviceBinding = binding,
            )
            synchronized(this) { pendingLoads[operationId] = pending }
            CompletableFuture.completedFuture(pending.context())
        }
    }

    override fun issueNote(request: OfflineNoteIssueRequest): CompletableFuture<OfflineNoteIssueResponse> {
        val pending = synchronized(this) { pendingLoads[request.loadContext.operationId] }
            ?: return failedFuture(OfflineToriiException("Missing Offline Note load context for operation ${request.loadContext.operationId}."))
        val body = linkedMapOf<String, Any?>(
            "account_id" to request.accountId,
            "operation_id" to pending.operationId,
            "device_id" to pending.deviceBinding.deviceId,
            "offline_public_key" to pending.deviceBinding.offlinePublicKey,
            "asset_definition_id" to request.assetDefinitionId,
            "device_binding" to pending.deviceBinding.deviceBinding(),
            "lineage_id" to pending.lineageId,
            "lineage_state" to deepCopyObject(pending.lineageState),
            "amount" to request.amount,
            "local_balance" to pending.localBalance,
            "local_revision" to pending.preIssueRevision,
            "note_commitment" to request.noteCommitmentHex(),
        )
        return executePost(NOTES_ISSUE_PATH, body) { payload ->
            val response = expectObject(parseJson(payload), "notes issue response")
            val commitment = hexToBytes(requiredString(response, "issued_note_commitment"), "issued_note_commitment")
            val lineageState = expectObject(requiredValue(response, "lineage_state"), "lineage_state")
            val localRevision = requiredLong(response, "local_revision")
            val keyCertificate = parseKeyCertificate(expectObject(requiredValue(response, "key_certificate"), "key_certificate"))
            val settlementEntryHash = optionalObject(response["settlement"])
                ?.let { optionalString(it["entry_hash"]) }
            val stored = StoredLineageState(
                lineageId = requiredString(lineageState, "lineage_id"),
                revision = requiredLong(lineageState, "server_revision"),
                balance = requiredString(lineageState, "balance"),
                authorizationExpiresAtMs = optionalObject(lineageState["authorization"])
                    ?.let { optionalLong(it["expires_at_ms"]) },
                keyCertificateExpiresAtMs = optionalLong(
                    expectObject(requiredValue(response, "key_certificate"), "key_certificate")["expires_at_ms"]
                ),
                keyCertificate = keyCertificate,
                lineageState = lineageState,
            )
            synchronized(this) {
                pendingLoads.remove(pending.operationId)
                lineageStates[pending.lineageKey] = stored
            }
            OfflineNoteIssueResponse(
                noteCommitment = commitment,
                operationId = requiredString(response, "operation_id"),
                lineageId = stored.lineageId,
                localRevision = localRevision,
                keyCertificate = keyCertificate,
                settlementEntryHashHex = settlementEntryHash,
            )
        }
    }

    private fun refillKeys(
        chainId: String,
        accountId: String,
        assetDefinitionId: String,
        binding: OfflineNoteIssuerDeviceBinding,
        lineageKey: String,
    ): CompletableFuture<OfflineNoteLoadContext> {
        val operationId = nonceGenerator.nextId("offline-key-refill")
        val existing = synchronized(this) { lineageStates[lineageKey] }
        val body = linkedMapOf<String, Any?>(
            "account_id" to accountId,
            "operation_id" to operationId,
            "device_id" to binding.deviceId,
            "offline_public_key" to binding.offlinePublicKey,
            "attestation_key_id" to binding.attestationKeyId(),
            "asset_definition_id" to assetDefinitionId,
            "local_revision" to (existing?.revision ?: 0L),
            "local_state_hash" to ((existing?.lineageState?.get("server_state_hash") as? String)?.trim() ?: ""),
            "device_binding" to binding.deviceBinding(),
        )
        if (existing != null) {
            body["existing_lineage_id"] = existing.lineageId
            body["lineage_state"] = deepCopyObject(existing.lineageState)
        }
        return executePost(KEYS_REFILL_PATH, body) { payload ->
            val response = expectObject(parseJson(payload), "keys refill response")
            val lineageState = expectObject(requiredValue(response, "lineage_state"), "lineage_state")
            val keyCertificate = parseKeyCertificate(expectObject(requiredValue(response, "key_certificate"), "key_certificate"))
            val preIssueRevision = requiredLong(lineageState, "server_revision")
            val pending = PendingLoad(
                operationId = requiredString(response, "operation_id"),
                lineageKey = lineageKey,
                lineageId = requiredString(lineageState, "lineage_id"),
                preIssueRevision = preIssueRevision,
                localBalance = requiredString(lineageState, "balance"),
                keyCertificate = keyCertificate,
                lineageState = lineageState,
                deviceBinding = binding,
            )
            synchronized(this) { pendingLoads[pending.operationId] = pending }
            pending.context()
        }
    }

    private fun <T> executePost(
        path: String,
        bodyFields: Map<String, Any?>,
        parser: (ByteArray) -> T,
    ): CompletableFuture<T> {
        val target = resolvePath(path)
        val signedBody = signedBody("POST", target, bodyFields)
        val request = buildPostRequest(target, signedBody)
        notifyRequest(request)
        val future = CompletableFuture<T>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                val error = OfflineToriiException("Offline issuer request failed: ${summarizeCauseMessage(cause)}", cause)
                notifyFailure(request, error)
                future.completeExceptionally(error)
                return@whenComplete
            }
            val rejectCode = HttpErrorMessageExtractor.extractRejectCode(
                response.headers,
                "x-iroha-reject-code",
                response.body,
            )
            val bodyPreview = HttpErrorMessageExtractor.extractMessage(response.body)
            val clientResponse = ClientResponse(response.statusCode, response.body, response.message, null, rejectCode)
            if (response.statusCode < 200 || response.statusCode >= 300) {
                val error = OfflineToriiException(
                    "Offline issuer request failed with HTTP ${response.statusCode} on ${request.uri.path}" +
                        (if (rejectCode.isNullOrBlank()) "" else ". reject_code=$rejectCode") +
                        (if (bodyPreview.isNullOrBlank()) "" else ". body=$bodyPreview"),
                    response.statusCode,
                    rejectCode,
                    bodyPreview,
                )
                notifyFailure(request, error)
                future.completeExceptionally(error)
                return@whenComplete
            }
            try {
                val parsed = parser(response.body)
                notifyResponse(request, clientResponse)
                future.complete(parsed)
            } catch (ex: RuntimeException) {
                val error = OfflineToriiException("Failed to parse Offline Note issuer response for ${request.uri.path}.", ex, response.statusCode, rejectCode, bodyPreview)
                notifyFailure(request, error)
                future.completeExceptionally(error)
            }
        }
        return future
    }

    private fun signedBody(method: String, target: URI, bodyFields: Map<String, Any?>): ByteArray {
        val timestampMs = canonicalAuth.timestampMs ?: clock.getAsLong()
        val nonce = canonicalAuth.nonce ?: nonceGenerator.nextId("offline-auth")
        val signed = CanonicalRequestSigner.withBodySignature(
            method,
            target,
            bodyFields,
            canonicalAuth.accountId,
            canonicalAuth.privateKey,
            timestampMs,
            nonce,
        )
        return JsonEncoder.encode(signed).toByteArray(StandardCharsets.UTF_8)
    }

    private fun buildPostRequest(target: URI, body: ByteArray): TransportRequest {
        val headers = LinkedHashMap(defaultHeaders)
        ensureHeader(headers, "Content-Type", "application/json")
        ensureHeader(headers, "Accept", "application/json")
        val builder = TransportRequest.builder()
            .setUri(target)
            .setMethod("POST")
            .setBody(body)
            .setTimeout(timeout)
        for ((key, value) in headers) builder.addHeader(key, value)
        return builder.build()
    }

    private fun resolvePath(path: String): URI {
        if (path.startsWith("http://") || path.startsWith("https://")) return URI.create(path)
        val normalized = if (path.startsWith("/")) path.substring(1) else path
        val base = baseUri.toString()
        return URI.create(if (base.endsWith("/")) base + normalized else "$base/$normalized")
    }

    private fun notifyRequest(request: TransportRequest) {
        for (observer in observers) observer.onRequest(request)
    }

    private fun notifyResponse(request: TransportRequest, response: ClientResponse) {
        for (observer in observers) observer.onResponse(request, response)
    }

    private fun notifyFailure(request: TransportRequest, error: Throwable) {
        for (observer in observers) observer.onFailure(request, error)
    }

    private data class PendingLoad(
        val operationId: String,
        val lineageKey: String,
        val lineageId: String,
        val preIssueRevision: Long,
        val localBalance: String,
        val keyCertificate: OfflineNote.KeyCertificate,
        val lineageState: Map<String, Any?>,
        val deviceBinding: OfflineNoteIssuerDeviceBinding,
    ) {
        fun context(): OfflineNoteLoadContext =
            OfflineNoteLoadContext(
                operationId = operationId,
                lineageId = lineageId,
                localRevision = preIssueRevision + 1,
                keyCertificate = keyCertificate,
            )
    }

    private data class StoredLineageState(
        val lineageId: String,
        val revision: Long,
        val balance: String,
        val authorizationExpiresAtMs: Long?,
        val keyCertificateExpiresAtMs: Long?,
        val keyCertificate: OfflineNote.KeyCertificate,
        val lineageState: Map<String, Any?>,
    ) {
        fun isExpired(nowMs: Long): Boolean =
            authorizationExpiresAtMs?.let { it <= nowMs } == true ||
                keyCertificateExpiresAtMs?.let { it <= nowMs } == true
    }

    companion object {
        private const val KEYS_REFILL_PATH = "/v1/offline/keys/refill"
        private const val NOTES_ISSUE_PATH = "/v1/offline/notes/issue"
    }
}

private fun parseKeyCertificate(value: Map<String, Any?>): OfflineNote.KeyCertificate =
    OfflineNote.KeyCertificate(
        version = requiredKeyCertificateVersion(value),
        platform = requiredString(value, "platform"),
        keyId = requiredString(value, "key_id"),
        deviceId = requiredString(value, "device_id"),
        accountId = requiredString(value, "account_id"),
        publicKey = decodeBase64(requiredString(value, "public_key"), "public_key"),
        assertionScheme = requiredString(value, "assertion_scheme"),
        assertionKeyAlgorithm = requiredString(value, "assertion_key_algorithm"),
        assertionPublicKey = decodeBase64(requiredString(value, "assertion_public_key"), "assertion_public_key"),
        assertionUsageCountLimit = optionalAssertionUsageCountLimit(value["assertion_usage_count_limit"]),
        oneUse = requiredBoolean(value, "one_use"),
        issuerSignature = decodeBase64(requiredString(value, "issuer_signature_base64"), "issuer_signature_base64"),
    )

private fun parseJson(payload: ByteArray): Any? =
    JsonParser.parse(String(payload, StandardCharsets.UTF_8))

private fun expectObject(value: Any?, path: String): Map<String, Any?> {
    require(value is Map<*, *>) { "$path must be a JSON object" }
    val result = LinkedHashMap<String, Any?>()
    for ((key, item) in value) {
        require(key is String) { "$path keys must be strings" }
        result[key] = normalizeJsonValue(item)
    }
    return result
}

private fun optionalObject(value: Any?): Map<String, Any?>? =
    if (value == null) null else expectObject(value, "object")

private fun requiredValue(value: Map<String, Any?>, field: String): Any? =
    if (value.containsKey(field)) value[field] else throw IllegalStateException("$field is required")

private fun requiredString(value: Map<String, Any?>, field: String): String =
    optionalString(requiredValue(value, field))
        ?: throw IllegalStateException("$field must be a string")

private fun optionalString(value: Any?): String? = value as? String

private fun requiredBoolean(value: Map<String, Any?>, field: String): Boolean =
    requiredValue(value, field) as? Boolean
        ?: throw IllegalStateException("$field must be a boolean")

private fun requiredLong(value: Map<String, Any?>, field: String): Long =
    optionalLong(requiredValue(value, field))
        ?: throw IllegalStateException("$field must be an integer")

private fun requiredKeyCertificateVersion(value: Map<String, Any?>): Int {
    val version = requiredLong(value, "version")
    require(version == OfflineNote.KEY_CERTIFICATE_VERSION.toLong()) {
        "version must be ${OfflineNote.KEY_CERTIFICATE_VERSION}"
    }
    return OfflineNote.KEY_CERTIFICATE_VERSION
}

private fun optionalAssertionUsageCountLimit(value: Any?): Int? {
    val limit = optionalLong(value) ?: return null
    require(limit == 1L) { "assertion_usage_count_limit must be exactly 1" }
    return 1
}

private fun optionalLong(value: Any?): Long? = when (value) {
    null -> null
    is Long -> value
    is Int -> value.toLong()
    is Short -> value.toLong()
    is Byte -> value.toLong()
    is java.math.BigInteger -> value.longValueExact()
    is Double -> {
        require(value.isFinite() && value % 1.0 == 0.0) { "number must be integral" }
        value.toLong()
    }
    is Float -> optionalLong(value.toDouble())
    else -> throw IllegalStateException("value must be an integer")
}

private fun decodeBase64(value: String, field: String): ByteArray =
    try {
        Base64.getDecoder().decode(value)
    } catch (ex: IllegalArgumentException) {
        throw IllegalStateException("$field must be base64", ex)
    }

private fun hexToBytes(value: String, field: String): ByteArray {
    require(value.length == 64) { "$field must be 64 lowercase hex characters" }
    val out = ByteArray(32)
    for (i in out.indices) {
        val hi = Character.digit(value[i * 2], 16)
        val lo = Character.digit(value[i * 2 + 1], 16)
        require(hi >= 0 && lo >= 0) { "$field must be hex" }
        out[i] = ((hi shl 4) or lo).toByte()
    }
    return out
}

private fun ensureHeader(headers: MutableMap<String, String>, name: String, value: String) {
    headers[headers.keys.firstOrNull { it.equals(name, ignoreCase = true) } ?: name] = value
}

private fun lineageKey(
    accountId: String,
    assetDefinitionId: String,
    binding: OfflineNoteIssuerDeviceBinding,
): String = "$accountId\n$assetDefinitionId\n${binding.deviceId}\n${binding.offlinePublicKey}"

private fun summarizeCauseMessage(cause: Throwable?): String =
    cause?.message?.takeIf { it.isNotBlank() } ?: cause?.javaClass?.simpleName ?: "unknown transport error"

private fun <T> failedFuture(error: Throwable): CompletableFuture<T> {
    val future = CompletableFuture<T>()
    future.completeExceptionally(error)
    return future
}

@Suppress("UNCHECKED_CAST")
private fun normalizeJsonValue(value: Any?): Any? = when (value) {
    null, is String, is Number, is Boolean -> value
    is Map<*, *> -> expectObject(value, "object")
    is List<*> -> value.map { normalizeJsonValue(it) }
    else -> throw IllegalStateException("Unsupported JSON value: ${value::class.java}")
}

private fun deepCopyObject(value: Map<String, Any?>): LinkedHashMap<String, Any?> {
    val copy = LinkedHashMap<String, Any?>()
    for ((key, item) in value) copy[key] = normalizeJsonValue(item)
    return copy
}
