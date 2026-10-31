package org.hyperledger.iroha.sdk.privacy

import java.security.MessageDigest

/** Stable signing-preparation failure; messages contain no statement or key bytes. */
class ZkX509SigningPreparationException(@JvmField val code: Int) : IllegalStateException(
    when (code) {
        -1 -> "X.509 signing input or output pointer is null"
        -2 -> "X.509 signing statement is empty"
        -3 -> "X.509 signing statement exceeds the native byte limit"
        -4 -> "X.509 signing statement is not a canonical Norito statement"
        -5 -> "Signing preparation requires the X.509 statement variant"
        -6 -> "Canonical X.509 ownership-signing preparation failed"
        -10 -> "X.509 signing output allocation failed"
        -101 -> "Native X.509 signing preparation is unavailable"
        -102 -> "Native X.509 signing preparation returned invalid output"
        else -> "X.509 signing preparation failed internally"
    },
)

/**
 * The native holder's exact public message and SHA-256 prehash for one immutable statement.
 *
 * A P-256 `SHA256withECDSA` message signer signs [message]; a prehash signer signs [digest].
 * Message-signing the digest hashes twice and produces an invalid ownership signature.
 * Normalize the returned signature to low-s fixed 64-byte `r || s` for witness construction.
 * Subject keys stay with the application's signer and never enter this API.
 *
 * Preparation checks canonical input, not authoritative State, host pairing, proof validity,
 * transaction authorization or release readiness. The application must review and authenticate
 * the exact original statement before requesting a signature.
 */
class ZkX509SigningPreparationV1 private constructor(message: ByteArray, digest: ByteArray) {
    private val messageBytes = message.copyOf()
    private val digestBytes = digest.copyOf()

    /** Public message for a P-256 signer that applies SHA-256 internally exactly once. */
    val message: ByteArray get() = messageBytes.copyOf()
    /** SHA-256 of [message], only for an explicit prehash-signing API. */
    val digest: ByteArray get() = digestBytes.copyOf()

    override fun toString(): String = "ZkX509SigningPreparationV1(public signing bytes)"

    companion object {
        /** Local native input bound; does not replace committed transaction admission rules. */
        const val MAX_STATEMENT_BYTES: Int = 256 * 1024

        /** Prepare one canonical Norito PrivacyStatementV1 with the X.509 variant. */
        @JvmStatic
        fun prepareOwnershipSigningV1(statementNorito: ByteArray): ZkX509SigningPreparationV1 =
            prepareWithNative(statementNorito, { PrivacyNativeBridge.isNativeAvailable() }) {
                ZkX509SigningPreparationNative.prepare(it)
            }

        internal fun prepareWithNative(
            statementNorito: ByteArray,
            available: () -> Boolean,
            native: (ByteArray) -> Array<ByteArray?>?,
        ): ZkX509SigningPreparationV1 {
            if (statementNorito.isEmpty()) throw ZkX509SigningPreparationException(-2)
            if (statementNorito.size > MAX_STATEMENT_BYTES) throw ZkX509SigningPreparationException(-3)
            if (!available()) throw ZkX509SigningPreparationException(-101)
            // Borrow no application-owned mutable array across the JNI call.
            val outputs = try { native(statementNorito.copyOf()) }
            catch (_: LinkageError) { throw ZkX509SigningPreparationException(-101) }
            if (outputs == null || outputs.size != 2) throw ZkX509SigningPreparationException(-102)
            val message = outputs[0]
            val digest = outputs[1]
            if (message == null || message.isEmpty() || message.size > MAX_STATEMENT_BYTES ||
                digest == null || digest.size != 32 ||
                !MessageDigest.isEqual(MessageDigest.getInstance("SHA-256").digest(message), digest)
            ) {
                throw ZkX509SigningPreparationException(-102)
            }
            return ZkX509SigningPreparationV1(message, digest)
        }
    }
}

internal object ZkX509SigningPreparationNative {
    @JvmStatic external fun prepare(statementNorito: ByteArray): Array<ByteArray?>?
}
