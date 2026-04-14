package org.hyperledger.iroha.sdk.crypto.keystore

import java.time.Duration
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm

/**
 * Specification for key generation requests made through `KeystoreBackend`.
 *
 * The parameters map closely to Android's `KeyGenParameterSpec` but avoid a direct
 * dependency so the desktop JVM build can compile. Android-specific builders will translate these
 * fields into platform constructs in follow-up revisions.
 */
class KeyGenParameters(
    @JvmField val requireStrongBox: Boolean = false,
    @JvmField val preferStrongBox: Boolean = false,
    @JvmField val allowStrongBoxFallback: Boolean = true,
    @JvmField val userAuthenticationRequired: Boolean = false,
    @JvmField val userAuthenticationTimeout: Duration = Duration.ZERO,
    @JvmField val algorithm: String = "Ed25519",
    attestationChallenge: ByteArray? = null,
) {
    private val _attestationChallenge: ByteArray? = attestationChallenge?.copyOf()

    fun attestationChallenge(): ByteArray? = _attestationChallenge?.copyOf()

    fun signingAlgorithm(): SigningAlgorithm = SigningAlgorithm.fromAlgorithmName(algorithm)

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()
    }

    /**
     * Mutable builder retained for Java interop and the imperative validation
     * logic that couples `requireStrongBox` / `preferStrongBox` / `allowStrongBoxFallback`.
     */
    class Builder {
        private var requireStrongBox = false
        private var preferStrongBox = false
        private var allowStrongBoxFallback = true
        private var userAuthenticationRequired = false
        private var userAuthenticationTimeout: Duration = Duration.ZERO
        private var algorithm = "Ed25519"
        private var attestationChallenge: ByteArray? = null

        fun setRequireStrongBox(requireStrongBox: Boolean): Builder = apply {
            this.requireStrongBox = requireStrongBox
            if (requireStrongBox) {
                this.allowStrongBoxFallback = false
            }
        }

        fun setPreferStrongBox(preferStrongBox: Boolean): Builder = apply {
            this.preferStrongBox = preferStrongBox
            if (preferStrongBox && !requireStrongBox) {
                this.allowStrongBoxFallback = true
            }
        }

        fun setAllowStrongBoxFallback(allowStrongBoxFallback: Boolean): Builder = apply {
            this.allowStrongBoxFallback = allowStrongBoxFallback
            if (!allowStrongBoxFallback) {
                this.requireStrongBox = true
            }
        }

        fun setUserAuthenticationRequired(userAuthenticationRequired: Boolean): Builder = apply {
            this.userAuthenticationRequired = userAuthenticationRequired
        }

        fun setUserAuthenticationTimeout(userAuthenticationTimeout: Duration?): Builder = apply {
            if (userAuthenticationTimeout != null) {
                this.userAuthenticationTimeout = userAuthenticationTimeout
            }
        }

        fun setAlgorithm(algorithm: String?): Builder = apply {
            if (!algorithm.isNullOrBlank()) {
                this.algorithm = SigningAlgorithm.fromAlgorithmName(algorithm).providerName
            }
        }

        fun setSigningAlgorithm(signingAlgorithm: SigningAlgorithm?): Builder = apply {
            if (signingAlgorithm != null) {
                this.algorithm = signingAlgorithm.providerName
            }
        }

        fun setAttestationChallenge(attestationChallenge: ByteArray?): Builder = apply {
            if (attestationChallenge != null) {
                this.attestationChallenge = attestationChallenge.copyOf()
            }
        }

        fun build(): KeyGenParameters = KeyGenParameters(
            requireStrongBox = requireStrongBox,
            preferStrongBox = preferStrongBox,
            allowStrongBoxFallback = allowStrongBoxFallback,
            userAuthenticationRequired = userAuthenticationRequired,
            userAuthenticationTimeout = userAuthenticationTimeout,
            algorithm = algorithm,
            attestationChallenge = attestationChallenge,
        )
    }

    fun toBuilder(): Builder = Builder()
        .setRequireStrongBox(requireStrongBox)
        .setPreferStrongBox(preferStrongBox)
        .setAllowStrongBoxFallback(allowStrongBoxFallback)
        .setUserAuthenticationRequired(userAuthenticationRequired)
        .setUserAuthenticationTimeout(userAuthenticationTimeout)
        .setSigningAlgorithm(signingAlgorithm())
        .setAttestationChallenge(_attestationChallenge)
}
