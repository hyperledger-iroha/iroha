package org.hyperledger.iroha.sdk.crypto.keystore

import android.content.Context
import com.google.android.gms.tasks.Task
import com.google.android.play.core.integrity.IntegrityManagerFactory
import com.google.android.play.core.integrity.StandardIntegrityException
import com.google.android.play.core.integrity.StandardIntegrityManager
import com.google.android.play.core.integrity.model.StandardIntegrityErrorCode
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.concurrent.ExecutionException

/** Opaque original Google evidence, never a client-verified verdict or qualification. */
class KagemushaAndroidPlayIntegrityTokenOriginalV1 internal constructor(@JvmField val cloudProjectNumber: Long,
    hash: ByteArray, private val token: String) {
    private val request = hash.copyOf()
    fun requestHash(): ByteArray = request.copyOf()
    fun opaqueToken(): String = token
}

/** Shared Android transport for the exact native-owned enrollment/policy-refresh request hash.
 * The independently admitted issuer checks Google, app signing identity, freshness and policy.
 * This adapter never chooses a verdict, retries an uncertain token request or manufactures a grant.
 */
class KagemushaAndroidPlayIntegrityProviderV1 internal constructor(private val backend: KagemushaPlayIntegrityBackendV1) {
    constructor(context: Context) : this(GoogleStandardIntegrityBackendV1(context.applicationContext))
    private val lock = Any()
    private var preparedProject: Long? = null
    private var prepared: CompletableFuture<KagemushaPlayIntegrityPreparedV1>? = null

    /** The publisher's explicit invalid-provider error allows a later user retry, never an automatic repeat. */
    internal fun invalidatedPreparedProviderOriginal(error: Throwable): Boolean = backend.invalidatesPreparedProvider(error)

    fun requestOriginal(cloudProjectNumber: Long, nativeRequestHash: ByteArray,
        requireOriginal: () -> Unit): CompletableFuture<KagemushaAndroidPlayIntegrityTokenOriginalV1> {
        require(cloudProjectNumber > 0 && nativeRequestHash.size == 32 && nativeRequestHash.any { it != 0.toByte() })
        val original = nativeRequestHash.copyOf()
        val result = CompletableFuture<KagemushaAndroidPlayIntegrityTokenOriginalV1>()
        try {
            requireOriginal()
            val warm = synchronized(lock) {
                prepared?.takeIf { preparedProject == cloudProjectNumber } ?: backend.prepare(cloudProjectNumber).also {
                    preparedProject = cloudProjectNumber; prepared = it
                }
            }
            warm.whenComplete { _, error ->
                if (error != null) synchronized(lock) { if (prepared === warm) { preparedProject = null; prepared = null } }
            }
            warm.whenComplete { provider, preparationError ->
                try {
                    requireOriginal()
                    if (preparationError != null) throw preparationError
                    if (!result.isCancelled) {
                        val hashText = Base64.getUrlEncoder().withoutPadding().encodeToString(original)
                        check(hashText.length == 43)
                        requireOriginal()
                        checkNotNull(provider).request(hashText).whenComplete { token, tokenError ->
                            if (tokenError != null && backend.invalidatesPreparedProvider(tokenError)) {
                                synchronized(lock) { if (prepared === warm) { preparedProject = null; prepared = null } }
                            }
                            try {
                                requireOriginal()
                                if (tokenError != null) throw tokenError
                                val raw = checkNotNull(token)
                                check(raw.isNotEmpty() && raw.length <= MAXIMUM_OPAQUE_TOKEN_BYTES && raw.all { it.code in 0x21..0x7e }) {
                                    "Original Google token is outside the supported bound"
                                }
                                val evidence = KagemushaAndroidPlayIntegrityTokenOriginalV1(cloudProjectNumber, original, raw)
                                requireOriginal(); result.complete(evidence)
                            } catch (error: Throwable) { result.completeExceptionally(error) }
                        }
                    }
                } catch (error: Throwable) { result.completeExceptionally(error) }
            }
        } catch (error: Throwable) { result.completeExceptionally(error) }
        return result
    }

    companion object {
        /** The issuer's original encrypted token bound; no decoded verdict is accepted here. */
        const val MAXIMUM_OPAQUE_TOKEN_BYTES: Int = 64 * 1024
    }
}

internal interface KagemushaPlayIntegrityBackendV1 {
    fun prepare(cloudProjectNumber: Long): CompletableFuture<KagemushaPlayIntegrityPreparedV1>
    fun invalidatesPreparedProvider(error: Throwable): Boolean {
        var original = error
        repeat(8) {
            if (original is StandardIntegrityException) {
                return original.errorCode == StandardIntegrityErrorCode.INTEGRITY_TOKEN_PROVIDER_INVALID
            }
            if (original !is CompletionException && original !is ExecutionException) return false
            original = original.cause ?: return false
        }
        return false
    }
}
internal interface KagemushaPlayIntegrityPreparedV1 {
    fun request(originalHashText: String): CompletableFuture<String>
}

private class GoogleStandardIntegrityBackendV1(context: Context) : KagemushaPlayIntegrityBackendV1 {
    private val manager = IntegrityManagerFactory.createStandard(context)
    override fun prepare(cloudProjectNumber: Long): CompletableFuture<KagemushaPlayIntegrityPreparedV1> =
        googleOriginalFutureV1(manager.prepareIntegrityToken(StandardIntegrityManager.PrepareIntegrityTokenRequest.builder()
            .setCloudProjectNumber(cloudProjectNumber).build())).thenApply { provider ->
            object : KagemushaPlayIntegrityPreparedV1 {
                override fun request(originalHashText: String): CompletableFuture<String> = googleOriginalFutureV1(provider.request(
                    StandardIntegrityManager.StandardIntegrityTokenRequest.builder().setRequestHash(originalHashText).build()
                )).thenApply { it.token() }
            }
        }
}
private fun <T> googleOriginalFutureV1(task: Task<T>): CompletableFuture<T> {
    val result = CompletableFuture<T>()
    task.addOnSuccessListener { result.complete(it) }
    task.addOnFailureListener { result.completeExceptionally(it) }
    task.addOnCanceledListener { result.cancel(false) }
    return result
}
