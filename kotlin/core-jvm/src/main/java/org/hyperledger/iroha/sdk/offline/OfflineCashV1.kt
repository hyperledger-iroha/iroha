package org.hyperledger.iroha.sdk.offline

import java.nio.charset.StandardCharsets

/** Exact canonical receiver request for the clean-slate Offline Cash V1 wire contract. */
class OfflineCashPaymentRequestV1(canonicalNorito: ByteArray) {
    private val canonical = OfflineCashNativeV1.canonicalizePaymentRequest(canonicalNorito)

    fun encodeCanonical(): ByteArray = canonical.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineCashPaymentRequestV1 && canonical.contentEquals(other.canonical)

    override fun hashCode(): Int = canonical.contentHashCode()

    companion object {
        const val MAX_CANONICAL_BYTES: Int = 768

        @JvmStatic
        fun decodeCanonical(canonicalNorito: ByteArray): OfflineCashPaymentRequestV1 =
            OfflineCashPaymentRequestV1(canonicalNorito)
    }
}

/** Exact canonical sender response, validated against its signed receiver request. */
class OfflineCashPaymentV1(
    val request: OfflineCashPaymentRequestV1,
    canonicalNorito: ByteArray,
) {
    private val canonical = OfflineCashNativeV1.canonicalizePayment(
        request.encodeCanonical(),
        canonicalNorito,
    )

    fun encodeCanonical(): ByteArray = canonical.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineCashPaymentV1 &&
            request == other.request && canonical.contentEquals(other.canonical)

    override fun hashCode(): Int = 31 * request.hashCode() + canonical.contentHashCode()

    companion object {
        const val MAX_CANONICAL_BYTES: Int = 7_936

        @JvmStatic
        fun decodeCanonical(
            request: OfflineCashPaymentRequestV1,
            canonicalNorito: ByteArray,
        ): OfflineCashPaymentV1 = OfflineCashPaymentV1(request, canonicalNorito)
    }
}

/** Exact canonical receiver acknowledgement, validated against one complete handoff. */
class OfflineCashAcknowledgementV1(
    val request: OfflineCashPaymentRequestV1,
    val payment: OfflineCashPaymentV1,
    canonicalNorito: ByteArray,
) {
    private val canonical = OfflineCashNativeV1.canonicalizeAcknowledgement(
        request.encodeCanonical(),
        payment.encodeCanonical(),
        canonicalNorito,
    )

    fun encodeCanonical(): ByteArray = canonical.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineCashAcknowledgementV1 &&
            request == other.request &&
            payment == other.payment &&
            canonical.contentEquals(other.canonical)

    override fun hashCode(): Int =
        31 * (31 * request.hashCode() + payment.hashCode()) + canonical.contentHashCode()

    companion object {
        const val MAX_CANONICAL_BYTES: Int = 256

        @JvmStatic
        fun decodeCanonical(
            request: OfflineCashPaymentRequestV1,
            payment: OfflineCashPaymentV1,
            canonicalNorito: ByteArray,
        ): OfflineCashAcknowledgementV1 =
            OfflineCashAcknowledgementV1(request, payment, canonicalNorito)
    }
}

/** Authenticated native release identity required before an Offline Cash V1 session can exist. */
class OfflineCashReleaseStatusV1 internal constructor(
    val available: Boolean,
    val nativeBridgeAbiVersion: Int?,
    installedReleaseId: ByteArray?,
    installedArtifactManifestSHA256: ByteArray?,
    val blocker: String?,
) {
    private val releaseId = installedReleaseId?.copyOf()
    private val artifactManifest = installedArtifactManifestSHA256?.copyOf()

    val installedReleaseId: ByteArray?
        get() = releaseId?.copyOf()

    val installedArtifactManifestSHA256: ByteArray?
        get() = artifactManifest?.copyOf()

    fun matches(
        expectedReleaseId: ByteArray,
        expectedArtifactManifestSHA256: ByteArray,
    ): Boolean = available &&
        expectedReleaseId.size == 32 &&
        expectedArtifactManifestSHA256.size == 32 &&
        releaseId?.contentEquals(expectedReleaseId) == true &&
        artifactManifest?.contentEquals(expectedArtifactManifestSHA256) == true

    companion object {
        const val REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 22
        const val AUTHENTICATED_RELEASE_UNAVAILABLE: String =
            "offline-cash-v1-authenticated-release-unavailable"
        const val NATIVE_ABI22_UNAVAILABLE: String = "offline-cash-v1-native-abi22-unavailable"

        @JvmStatic
        fun installed(): OfflineCashReleaseStatusV1 = OfflineCashNativeV1.releaseStatus()
    }
}

enum class OfflineCashWalletSessionStateV1 {
    RECEIVE_REQUEST_READY,
    PAYMENT_COMMITTED,
    ACKNOWLEDGED,
}

enum class OfflineCashWalletSessionEventV1 {
    PAYMENT_COMMITTED,
    PAYMENT_REPLAY,
    ACKNOWLEDGED,
    ACKNOWLEDGEMENT_REPLAY,
}

/**
 * Opaque fail-closed wallet state machine for one receiver-bound Offline Cash V1 handoff.
 *
 * Construction cryptographically binds the signed runtime manifest's release id and artifact
 * manifest SHA-256 to the authenticated installed native artifact set. There is deliberately no
 * production bypass or app-owned emulator constructor.
 */
class OfflineCashWalletSessionV1(
    val request: OfflineCashPaymentRequestV1,
    expectedReleaseId: ByteArray,
    expectedArtifactManifestSHA256: ByteArray,
) {
    private val releaseId = expectedReleaseId.copyOf()
    private val artifactManifest = expectedArtifactManifestSHA256.copyOf()
    private var committedPayment: OfflineCashPaymentV1? = null
    private var acceptedAcknowledgement: OfflineCashAcknowledgementV1? = null

    init {
        require(releaseId.size == 32 && releaseId.any { it.toInt() != 0 }) {
            "expectedReleaseId must be a non-zero 32-byte digest"
        }
        require(artifactManifest.size == 32 && artifactManifest.any { it.toInt() != 0 }) {
            "expectedArtifactManifestSHA256 must be a non-zero 32-byte digest"
        }
        val status = OfflineCashReleaseStatusV1.installed()
        check(status.matches(releaseId, artifactManifest)) {
            status.blocker ?: "offline-cash-v1-installed-release-mismatch"
        }
    }

    val state: OfflineCashWalletSessionStateV1
        @Synchronized get() = when {
            acceptedAcknowledgement != null -> OfflineCashWalletSessionStateV1.ACKNOWLEDGED
            committedPayment != null -> OfflineCashWalletSessionStateV1.PAYMENT_COMMITTED
            else -> OfflineCashWalletSessionStateV1.RECEIVE_REQUEST_READY
        }

    fun expectedReleaseId(): ByteArray = releaseId.copyOf()

    fun expectedArtifactManifestSHA256(): ByteArray = artifactManifest.copyOf()

    @Synchronized
    fun payment(): OfflineCashPaymentV1? = committedPayment

    @Synchronized
    fun acknowledgement(): OfflineCashAcknowledgementV1? = acceptedAcknowledgement

    @Synchronized
    fun acceptPayment(canonicalNorito: ByteArray): OfflineCashWalletSessionEventV1 {
        check(acceptedAcknowledgement == null) { "payment cannot follow acknowledgement" }
        val sessionCanonical = OfflineCashNativeV1.canonicalizePaymentForSession(
            request.encodeCanonical(),
            canonicalNorito,
            artifactManifest,
        )
        val candidate = OfflineCashPaymentV1(request, sessionCanonical)
        committedPayment?.let { existing ->
            if (existing == candidate) return OfflineCashWalletSessionEventV1.PAYMENT_REPLAY
            throw IllegalArgumentException("conflicting Offline Cash V1 payment")
        }
        committedPayment = candidate
        return OfflineCashWalletSessionEventV1.PAYMENT_COMMITTED
    }

    @Synchronized
    fun acceptAcknowledgement(canonicalNorito: ByteArray): OfflineCashWalletSessionEventV1 {
        val payment = checkNotNull(committedPayment) { "acknowledgement requires a payment" }
        val candidate = OfflineCashAcknowledgementV1(request, payment, canonicalNorito)
        acceptedAcknowledgement?.let { existing ->
            if (existing == candidate) {
                return OfflineCashWalletSessionEventV1.ACKNOWLEDGEMENT_REPLAY
            }
            throw IllegalArgumentException("conflicting Offline Cash V1 acknowledgement")
        }
        acceptedAcknowledgement = candidate
        return OfflineCashWalletSessionEventV1.ACKNOWLEDGED
    }
}

/** Strict canonical `kgm2:` peer transport adapter, distinct from PKK1 transport. */
object OfflineCashPeerAdapterV1 {
    const val TEXT_PREFIX: String = "kgm2:"
    const val MAX_TEXT_SESSION_BYTES: Int = 12_288

    @JvmStatic
    fun encodePaymentRequest(request: OfflineCashPaymentRequestV1): String =
        OfflineCashNativeV1.peerEncodePaymentRequest(request.encodeCanonical())

    @JvmStatic
    fun decodePaymentRequest(text: String): OfflineCashPaymentRequestV1 =
        OfflineCashPaymentRequestV1(OfflineCashNativeV1.peerDecodePaymentRequest(text))

    @JvmStatic
    fun encodePayment(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
    ): String = OfflineCashNativeV1.peerEncodePayment(
        request.encodeCanonical(),
        payment.encodeCanonical(),
    )

    @JvmStatic
    fun decodePayment(
        request: OfflineCashPaymentRequestV1,
        text: String,
    ): OfflineCashPaymentV1 = OfflineCashPaymentV1(
        request,
        OfflineCashNativeV1.peerDecodePayment(request.encodeCanonical(), text),
    )

    @JvmStatic
    fun encodeAcknowledgement(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        acknowledgement: OfflineCashAcknowledgementV1,
    ): String = OfflineCashNativeV1.peerEncodeAcknowledgement(
        request.encodeCanonical(),
        payment.encodeCanonical(),
        acknowledgement.encodeCanonical(),
    )

    @JvmStatic
    fun decodeAcknowledgement(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        text: String,
    ): OfflineCashAcknowledgementV1 = OfflineCashAcknowledgementV1(
        request,
        payment,
        OfflineCashNativeV1.peerDecodeAcknowledgement(
            request.encodeCanonical(),
            payment.encodeCanonical(),
            text,
        ),
    )
}

/** ABI22 JNI boundary. Public callers use the typed wrappers above. */
internal object OfflineCashNativeV1 {
    private const val LIBRARY_NAME = "connect_norito_bridge"
    private val loaded: Boolean = runCatching { System.loadLibrary(LIBRARY_NAME) }.isSuccess

    private fun requireLoaded() {
        check(loaded) { OfflineCashReleaseStatusV1.NATIVE_ABI22_UNAVAILABLE }
    }

    fun canonicalizePaymentRequest(value: ByteArray): ByteArray {
        requireLoaded()
        return nativeCanonicalizePaymentRequestV1(value)
    }

    fun canonicalizePayment(request: ByteArray, payment: ByteArray): ByteArray {
        requireLoaded()
        return nativeCanonicalizePaymentV1(request, payment)
    }

    fun canonicalizePaymentForSession(
        request: ByteArray,
        payment: ByteArray,
        expectedArtifactManifestSHA256: ByteArray,
    ): ByteArray {
        requireLoaded()
        return nativeCanonicalizePaymentForSessionV1(
            request,
            payment,
            expectedArtifactManifestSHA256,
        )
    }

    fun canonicalizeAcknowledgement(
        request: ByteArray,
        payment: ByteArray,
        acknowledgement: ByteArray,
    ): ByteArray {
        requireLoaded()
        return nativeCanonicalizeAcknowledgementV1(request, payment, acknowledgement)
    }

    fun peerEncodePaymentRequest(request: ByteArray): String {
        requireLoaded()
        return String(nativePeerEncodePaymentRequestV1(request), StandardCharsets.UTF_8)
    }

    fun peerDecodePaymentRequest(text: String): ByteArray {
        requireLoaded()
        return nativePeerDecodePaymentRequestV1(text.toByteArray(StandardCharsets.UTF_8))
    }

    fun peerEncodePayment(request: ByteArray, payment: ByteArray): String {
        requireLoaded()
        return String(nativePeerEncodePaymentV1(request, payment), StandardCharsets.UTF_8)
    }

    fun peerDecodePayment(request: ByteArray, text: String): ByteArray {
        requireLoaded()
        return nativePeerDecodePaymentV1(request, text.toByteArray(StandardCharsets.UTF_8))
    }

    fun peerEncodeAcknowledgement(
        request: ByteArray,
        payment: ByteArray,
        acknowledgement: ByteArray,
    ): String {
        requireLoaded()
        return String(
            nativePeerEncodeAcknowledgementV1(request, payment, acknowledgement),
            StandardCharsets.UTF_8,
        )
    }

    fun peerDecodeAcknowledgement(
        request: ByteArray,
        payment: ByteArray,
        text: String,
    ): ByteArray {
        requireLoaded()
        return nativePeerDecodeAcknowledgementV1(
            request,
            payment,
            text.toByteArray(StandardCharsets.UTF_8),
        )
    }

    fun releaseStatus(): OfflineCashReleaseStatusV1 {
        if (!loaded) {
            return OfflineCashReleaseStatusV1(
                available = false,
                nativeBridgeAbiVersion = null,
                installedReleaseId = null,
                installedArtifactManifestSHA256 = null,
                blocker = OfflineCashReleaseStatusV1.NATIVE_ABI22_UNAVAILABLE,
            )
        }
        val fields = runCatching { nativeReleaseProbeV1() }.getOrElse {
            return OfflineCashReleaseStatusV1(
                available = false,
                nativeBridgeAbiVersion = null,
                installedReleaseId = null,
                installedArtifactManifestSHA256 = null,
                blocker = OfflineCashReleaseStatusV1.NATIVE_ABI22_UNAVAILABLE,
            )
        }
        if (fields.size != 4 || fields[0].size != 1 || fields[1].size != 32 ||
            fields[2].size != 32 || fields[3].size != 4
        ) {
            return OfflineCashReleaseStatusV1(
                available = false,
                nativeBridgeAbiVersion = null,
                installedReleaseId = null,
                installedArtifactManifestSHA256 = null,
                blocker = OfflineCashReleaseStatusV1.NATIVE_ABI22_UNAVAILABLE,
            )
        }
        val abi = fields[3].fold(0) { value, byte -> (value shl 8) or (byte.toInt() and 0xff) }
        val available = fields[0][0].toInt() == 1 &&
            abi == OfflineCashReleaseStatusV1.REQUIRED_NATIVE_BRIDGE_ABI_VERSION &&
            fields[1].any { it.toInt() != 0 } && fields[2].any { it.toInt() != 0 }
        return OfflineCashReleaseStatusV1(
            available = available,
            nativeBridgeAbiVersion = abi,
            installedReleaseId = fields[1].takeIf { available },
            installedArtifactManifestSHA256 = fields[2].takeIf { available },
            blocker = if (available) null else
                OfflineCashReleaseStatusV1.AUTHENTICATED_RELEASE_UNAVAILABLE,
        )
    }

    @JvmStatic private external fun nativeCanonicalizePaymentRequestV1(request: ByteArray): ByteArray
    @JvmStatic private external fun nativeCanonicalizePaymentV1(
        request: ByteArray,
        payment: ByteArray,
    ): ByteArray
    @JvmStatic private external fun nativeCanonicalizePaymentForSessionV1(
        request: ByteArray,
        payment: ByteArray,
        expectedArtifactManifestSHA256: ByteArray,
    ): ByteArray
    @JvmStatic private external fun nativeCanonicalizeAcknowledgementV1(
        request: ByteArray,
        payment: ByteArray,
        acknowledgement: ByteArray,
    ): ByteArray
    @JvmStatic private external fun nativePeerEncodePaymentRequestV1(request: ByteArray): ByteArray
    @JvmStatic private external fun nativePeerDecodePaymentRequestV1(text: ByteArray): ByteArray
    @JvmStatic private external fun nativePeerEncodePaymentV1(
        request: ByteArray,
        payment: ByteArray,
    ): ByteArray
    @JvmStatic private external fun nativePeerDecodePaymentV1(
        request: ByteArray,
        text: ByteArray,
    ): ByteArray
    @JvmStatic private external fun nativePeerEncodeAcknowledgementV1(
        request: ByteArray,
        payment: ByteArray,
        acknowledgement: ByteArray,
    ): ByteArray
    @JvmStatic private external fun nativePeerDecodeAcknowledgementV1(
        request: ByteArray,
        payment: ByteArray,
        text: ByteArray,
    ): ByteArray
    @JvmStatic private external fun nativeReleaseProbeV1(): Array<ByteArray>
}
