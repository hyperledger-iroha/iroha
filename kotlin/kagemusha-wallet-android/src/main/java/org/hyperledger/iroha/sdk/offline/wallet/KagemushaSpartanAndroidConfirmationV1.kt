// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import android.content.Context
import android.os.Build
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyInfo
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import java.security.AlgorithmParameters
import java.security.KeyFactory
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PrivateKey
import java.security.Signature
import java.security.interfaces.ECPublicKey
import java.security.spec.ECGenParameterSpec
import java.security.spec.ECParameterSpec
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference

/** A local debit can be unavailable while enrollment and the online account remain usable. */
enum class KagemushaSpartanDebitUnavailableReasonV1 {
    STRONG_BIOMETRIC_REQUIRED_ON_ANDROID_26_TO_29,
    DEVICE_AUTHENTICATION_UNAVAILABLE,
    HARDWARE_CONFIRMATION_KEY_UNAVAILABLE,
    CONFIRMATION_KEY_INVALIDATED,
}

class KagemushaSpartanDebitUnavailableV1(val reason: KagemushaSpartanDebitUnavailableReasonV1,
    val platformCode: Int? = null, cause: Throwable? = null) : IllegalStateException(reason.name, cause)

internal fun spartanAuthenticatorsV1(api: Int): Int = Authenticators.BIOMETRIC_STRONG or
    (if (api >= 30) Authenticators.DEVICE_CREDENTIAL else 0)

internal fun requireSpartanAuthenticatorV1(api: Int, status: Int) {
    if (status == BiometricManager.BIOMETRIC_SUCCESS) return
    val reason = if (api < 30 && status in setOf(BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED,
            BiometricManager.BIOMETRIC_ERROR_NO_HARDWARE, BiometricManager.BIOMETRIC_ERROR_UNSUPPORTED))
        KagemushaSpartanDebitUnavailableReasonV1.STRONG_BIOMETRIC_REQUIRED_ON_ANDROID_26_TO_29
    else KagemushaSpartanDebitUnavailableReasonV1.DEVICE_AUTHENTICATION_UNAVAILABLE
    throw KagemushaSpartanDebitUnavailableV1(reason, status)
}

internal fun requireSpartanAuthenticatorV1(context: Context) = requireSpartanAuthenticatorV1(Build.VERSION.SDK_INT,
    BiometricManager.from(context).canAuthenticate(spartanAuthenticatorsV1(Build.VERSION.SDK_INT)))

/** OS key used only for local Spartan confirmation. Every signing operation needs its own
 * authenticated CryptoObject. A cached screen unlock is insufficient. */
internal object KagemushaSpartanAndroidConfirmationV1 {
    private const val STORE = "AndroidKeyStore"
    private const val PREFIX = "kgm-spartan-user-presence-v1-"
    private val hex = "0123456789abcdef"
    internal fun alias(slot: ByteArray): String {
        require(slot.size == 32 && slot.any { it != 0.toByte() })
        return buildString {
            append(PREFIX)
            for (byte in slot) { val n = byte.toInt() and 255; append(hex[n ushr 4]); append(hex[n and 15]) }
        }
    }

    private fun store() = KeyStore.getInstance(STORE).apply { load(null) }

    @Synchronized fun provision(slot: ByteArray, preferStrongBox: Boolean) {
        val alias = alias(slot)
        if (!store().containsAlias(alias)) {
            if (Build.VERSION.SDK_INT >= 28 && preferStrongBox) {
                try { generate(alias, true) }
                catch (failure: StrongBoxUnavailableException) {
                    // Never replace an existing or uncertain key, even after a provider error.
                    if (store().containsAlias(alias)) throw failure
                    generate(alias, false)
                }
            } else generate(alias, false)
        }
        material(slot)
    }

    @Suppress("DEPRECATION")
    private fun generate(alias: String, strongBox: Boolean) {
        val spec = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN)
            .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
            .setDigests(KeyProperties.DIGEST_SHA256)
            .setUserAuthenticationRequired(true)
            .setInvalidatedByBiometricEnrollment(false)
        if (Build.VERSION.SDK_INT >= 30) {
            spec.setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG or KeyProperties.AUTH_DEVICE_CREDENTIAL)
        } else {
            spec.setUserAuthenticationValidityDurationSeconds(-1)
        }
        if (Build.VERSION.SDK_INT >= 28 && strongBox) spec.setIsStrongBoxBacked(true)
        KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, STORE).apply {
            initialize(spec.build())
        }.generateKeyPair()
    }

    private data class Material(val key: PrivateKey, val publicKey: ECPublicKey, val authenticators: Int)
    @Suppress("DEPRECATION")
    private fun material(slot: ByteArray): Material {
        val alias = alias(slot)
        val ks = store()
        val key = ks.getKey(alias, null) as? PrivateKey ?: error("Confirmation key unavailable")
        val publicKey = ks.getCertificate(alias)?.publicKey as? ECPublicKey ?: error("Confirmation key unavailable")
        val info = KeyFactory.getInstance(KeyProperties.KEY_ALGORITHM_EC, STORE).getKeySpec(key, KeyInfo::class.java)
        val hardware = if (Build.VERSION.SDK_INT >= 31) {
            info.securityLevel == KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT ||
                info.securityLevel == KeyProperties.SECURITY_LEVEL_STRONGBOX
        } else info.isInsideSecureHardware
        val policy = if (Build.VERSION.SDK_INT >= 30) info.userAuthenticationType else KeyProperties.AUTH_BIOMETRIC_STRONG
        val authenticators = when (policy) {
            KeyProperties.AUTH_BIOMETRIC_STRONG -> Authenticators.BIOMETRIC_STRONG
            (KeyProperties.AUTH_BIOMETRIC_STRONG or KeyProperties.AUTH_DEVICE_CREDENTIAL) ->
                Authenticators.BIOMETRIC_STRONG or Authenticators.DEVICE_CREDENTIAL
            else -> error("Unsupported hardware confirmation authenticators")
        }
        val p256 = AlgorithmParameters.getInstance("EC").apply { init(ECGenParameterSpec("secp256r1")) }
            .getParameterSpec(ECParameterSpec::class.java)
        val curve = publicKey.params
        check(info.keystoreAlias == alias && key.encoded == null && hardware &&
            info.origin == KeyProperties.ORIGIN_GENERATED && info.keySize == 256 &&
            info.purposes == KeyProperties.PURPOSE_SIGN && info.digests.toSet() == setOf(KeyProperties.DIGEST_SHA256) &&
            info.isUserAuthenticationRequired && info.isUserAuthenticationRequirementEnforcedBySecureHardware &&
            info.userAuthenticationValidityDurationSeconds == -1 && !info.isUserAuthenticationValidWhileOnBody &&
            curve.curve == p256.curve && curve.generator == p256.generator &&
            curve.order == p256.order && curve.cofactor == p256.cofactor) { "Hardware confirmation policy unavailable" }
        return Material(key, publicKey, authenticators)
    }

    fun probe(slot: ByteArray): ByteArray {
        val publicKey = material(slot).publicKey
        fun coordinate(n: java.math.BigInteger): ByteArray {
            require(n.signum() >= 0 && n.bitLength() <= 256)
            val value = n.toByteArray().takeLast(32).toByteArray()
            return ByteArray(32 - value.size) + value
        }
        return byteArrayOf(4) + coordinate(publicKey.w.affineX) + coordinate(publicKey.w.affineY)
    }

    fun operation(slot: ByteArray): Pair<Signature, Int> {
        val key = material(slot)
        return try {
            Signature.getInstance("SHA256withECDSA").apply { initSign(key.key) } to key.authenticators
        } catch (failure: android.security.keystore.KeyPermanentlyInvalidatedException) {
            throw KagemushaSpartanDebitUnavailableV1(KagemushaSpartanDebitUnavailableReasonV1.CONFIRMATION_KEY_INVALIDATED,
                cause = failure)
        }
    }
}

/** One SDK-owned OS prompt with terms derived from the same Native review as the challenge. */
class KagemushaSpartanDebitPromptV1 internal constructor(
    private val signature: Signature, private val message: ByteArray, private val terms: String,
    private val retain: (ByteArray, ByteArray) -> Unit,
    private val discard: (ByteArray) -> Unit,
    private val allowed: Int = Authenticators.BIOMETRIC_STRONG,
) {
    private val consumed = AtomicBoolean(false)
    private val presenting = AtomicBoolean(false)
    private val cancelUi = AtomicReference<(() -> Unit)?>(null)
    /** Present this exact operation. Cancellation retires the Native challenge without signing. */
    suspend fun authenticate(activity: FragmentActivity): Boolean = withContext(Dispatchers.Main.immediate) {
        if (consumed.get() || !presenting.compareAndSet(false, true)) return@withContext false
        try {
            check(!activity.isFinishing && !activity.isDestroyed && !activity.supportFragmentManager.isStateSaved)
            if (!reviewTerms(activity) || consumed.get()) return@withContext false
            currentCoroutineContext().ensureActive()
            val authenticated = suspendCancellableCoroutine<Signature?> { continuation ->
                val executor = ContextCompat.getMainExecutor(activity)
                lateinit var observer: LifecycleEventObserver
                val prompt = BiometricPrompt(activity, executor, object : BiometricPrompt.AuthenticationCallback() {
                    override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                        activity.lifecycle.removeObserver(observer)
                        if (continuation.isActive) continuation.resume(result.cryptoObject?.signature)
                    }
                    override fun onAuthenticationError(code: Int, message: CharSequence) {
                        activity.lifecycle.removeObserver(observer)
                        if (!continuation.isActive) return
                        if (code == BiometricPrompt.ERROR_CANCELED || code == BiometricPrompt.ERROR_USER_CANCELED ||
                            code == BiometricPrompt.ERROR_NEGATIVE_BUTTON || code == BiometricPrompt.ERROR_TIMEOUT) continuation.resume(null)
                        else continuation.resumeWithException(KagemushaSpartanDebitUnavailableV1(
                            KagemushaSpartanDebitUnavailableReasonV1.DEVICE_AUTHENTICATION_UNAVAILABLE, code))
                    }
                })
                val stop: () -> Unit = { executor.execute {
                    if (continuation.isActive) continuation.resume(null)
                    prompt.cancelAuthentication()
                    activity.lifecycle.removeObserver(observer)
                } }
                observer = LifecycleEventObserver { _, event -> if (event == Lifecycle.Event.ON_DESTROY) stop() }
                cancelUi.set(stop)
                activity.lifecycle.addObserver(observer)
                continuation.invokeOnCancellation { stop() }
                if (consumed.get() || !continuation.isActive) { stop(); return@suspendCancellableCoroutine }
                val info = BiometricPrompt.PromptInfo.Builder().setTitle("Approve offline wallet debit").setDescription(terms)
                    .setAllowedAuthenticators(allowed).setConfirmationRequired(true)
                if (allowed and Authenticators.DEVICE_CREDENTIAL == 0) info.setNegativeButtonText(activity.getString(android.R.string.cancel))
                try { prompt.authenticate(info.build(), BiometricPrompt.CryptoObject(signature)) }
                catch (error: Exception) {
                    activity.lifecycle.removeObserver(observer)
                    if (continuation.isActive) continuation.resumeWithException(error)
                }
            } ?: return@withContext false
            currentCoroutineContext().ensureActive()
            withContext(NonCancellable + Dispatchers.IO) { complete(authenticated, cancelWins = true) }
        } finally {
            cancelUi.set(null)
            withContext(NonCancellable + Dispatchers.IO) { cancel() }
        }
    }
    private suspend fun reviewTerms(activity: FragmentActivity): Boolean =
        suspendCancellableCoroutine { continuation ->
            val text = android.widget.TextView(activity).apply {
                this.text = terms
                setTextIsSelectable(true)
                val inset = (20 * resources.displayMetrics.density).toInt()
                setPadding(inset, inset, inset, inset)
            }
            val scroll = android.widget.ScrollView(activity).apply { addView(text) }
            val dialog = android.app.AlertDialog.Builder(activity).setTitle("Review offline wallet debit").setView(scroll)
                .setNegativeButton(android.R.string.cancel) { _, _ -> if (continuation.isActive) continuation.resume(false) }
                .setPositiveButton(android.R.string.ok) { _, _ -> if (continuation.isActive) continuation.resume(true) }
                .setOnCancelListener { if (continuation.isActive) continuation.resume(false) }.create()
            val stop: () -> Unit = { ContextCompat.getMainExecutor(activity).execute {
                if (continuation.isActive) continuation.resume(false)
                dialog.dismiss()
            } }
            val observer = LifecycleEventObserver { _, event ->
                if (event == Lifecycle.Event.ON_STOP || event == Lifecycle.Event.ON_DESTROY) stop()
            }
            cancelUi.set(stop)
            dialog.setOnDismissListener {
                activity.lifecycle.removeObserver(observer)
                if (continuation.isActive) continuation.resume(false)
            }
            activity.lifecycle.addObserver(observer)
            continuation.invokeOnCancellation { stop() }
            if (consumed.get() || !continuation.isActive) stop() else dialog.show()
        }

    @JvmSynthetic internal fun complete(authenticatedSignature: Signature, cancelWins: Boolean = false): Boolean {
        require(authenticatedSignature === signature)
        if (!consumed.compareAndSet(false, true)) {
            if (cancelWins) return false
            error("Confirmation already consumed")
        }
        try {
            authenticatedSignature.update(message)
            retain(message, authenticatedSignature.sign())
            return true
        } catch (error: Exception) {
            try { discard(message) } catch (cleanup: Exception) { error.addSuppressed(cleanup) }
            throw error
        }
    }
    fun cancel() {
        if (consumed.compareAndSet(false, true)) {
            cancelUi.getAndSet(null)?.invoke()
            discard(message)
        }
    }
}
