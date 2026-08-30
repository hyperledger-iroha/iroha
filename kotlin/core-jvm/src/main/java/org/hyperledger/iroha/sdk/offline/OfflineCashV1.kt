package org.hyperledger.iroha.sdk.offline

import java.nio.charset.StandardCharsets

/** Exact public byte ceilings for the first Offline Cash V1 transport release. */
object OfflineCashLimitsV1 {
    const val PAYMENT_REQUEST_RAW_MAX_BYTES: Int = 768
    const val PAYMENT_RAW_MAX_BYTES: Int = 7_936
    const val ACKNOWLEDGEMENT_RAW_MAX_BYTES: Int = 256
    const val PAYMENT_REQUEST_TEXT_MAX_BYTES: Int = 1_029
    const val PAYMENT_TEXT_MAX_BYTES: Int = 10_587
    const val ACKNOWLEDGEMENT_TEXT_MAX_BYTES: Int = 347
    const val RAW_SESSION_MAX_BYTES: Int = 9_211
    const val TEXT_SESSION_MAX_BYTES: Int = 12_288
    const val PAIRED_PROOF_MAX_BYTES: Int = 6_400
    const val PARITY_PROOF_MAX_BYTES: Int = 3_200
    const val ENCRYPTED_CREDIT_MAX_BYTES: Int = 384
}

/** Exact canonical receiver request for the clean-slate Offline Cash V1 wire contract. */
class OfflineCashPaymentRequestV1(canonicalNorito: ByteArray) {
    private val canonical = OfflineCashNativeV1.canonicalizePaymentRequest(canonicalNorito)

    fun encodeCanonical(): ByteArray = canonical.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineCashPaymentRequestV1 && canonical.contentEquals(other.canonical)

    override fun hashCode(): Int = canonical.contentHashCode()

    companion object {
        const val MAX_CANONICAL_BYTES: Int =
            OfflineCashLimitsV1.PAYMENT_REQUEST_RAW_MAX_BYTES

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
        const val MAX_CANONICAL_BYTES: Int = OfflineCashLimitsV1.PAYMENT_RAW_MAX_BYTES

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
        const val MAX_CANONICAL_BYTES: Int =
            OfflineCashLimitsV1.ACKNOWLEDGEMENT_RAW_MAX_BYTES

        @JvmStatic
        fun decodeCanonical(
            request: OfflineCashPaymentRequestV1,
            payment: OfflineCashPaymentV1,
            canonicalNorito: ByteArray,
        ): OfflineCashAcknowledgementV1 =
            OfflineCashAcknowledgementV1(request, payment, canonicalNorito)
    }
}

/** Authenticated native release identity required before a verification session can exist. */
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

enum class OfflineCashVerificationSessionStateV1 {
    UNAVAILABLE,
    REQUEST_VERIFIED,
    PAYMENT_VERIFIED,
    ACKNOWLEDGEMENT_VERIFIED,
}

enum class OfflineCashVerificationSessionEventV1 {
    PAYMENT_VERIFIED,
    PAYMENT_VERIFICATION_REPLAY,
    ACKNOWLEDGEMENT_VERIFIED,
    ACKNOWLEDGEMENT_VERIFICATION_REPLAY,
}

/**
 * Opaque fail-closed proof-verification state machine for one receiver-bound Offline Cash V1
 * handoff.
 *
 * Construction cryptographically binds the signed runtime manifest's release id and artifact
 * manifest SHA-256 to the authenticated installed native artifact set. `PAYMENT_VERIFIED` and
 * `ACKNOWLEDGEMENT_VERIFIED` describe only native cryptographic verification and retention in this
 * process. They do not mean that the secure-device journal, exact-next counter, wallet balance,
 * payment outbox, or acknowledgement store was mutated durably. Only the sealed Core lifecycle
 * joined to a qualifying device backend may authorize those effects. There is deliberately no
 * production bypass or app-owned emulator constructor.
 */
class OfflineCashVerificationSessionV1(
    val request: OfflineCashPaymentRequestV1,
    expectedReleaseId: ByteArray,
    expectedArtifactManifestSHA256: ByteArray,
    val expectedNetworkIdLiteral: String,
    val expectedAssetDefinitionId: String,
) : AutoCloseable {
    private val releaseId = expectedReleaseId.copyOf()
    private val artifactManifest = expectedArtifactManifestSHA256.copyOf()
    private var verifiedPayment: OfflineCashPaymentV1? = null
    private var verifiedAcknowledgement: OfflineCashAcknowledgementV1? = null
    private var nativeHandle: Long = 0

    init {
        require(releaseId.size == 32 && releaseId.any { it.toInt() != 0 }) {
            "expectedReleaseId must be a non-zero 32-byte digest"
        }
        require(artifactManifest.size == 32 && artifactManifest.any { it.toInt() != 0 }) {
            "expectedArtifactManifestSHA256 must be a non-zero 32-byte digest"
        }
        require(
            expectedNetworkIdLiteral.length == 64 &&
                expectedNetworkIdLiteral.all { it in '0'..'9' || it in 'a'..'f' },
        ) { "expectedNetworkIdLiteral must be an exact lowercase 64-hex NetworkId" }
        require(
            expectedAssetDefinitionId.isNotEmpty() &&
                expectedAssetDefinitionId.length <= 64 &&
                expectedAssetDefinitionId.all { character ->
                    character in '1'..'9' ||
                        character in 'A'..'H' ||
                        character in 'J'..'N' ||
                        character in 'P'..'Z' ||
                        character in 'a'..'k' ||
                        character in 'm'..'z'
                },
        ) { "expectedAssetDefinitionId must be a bounded canonical Base58 literal" }
        val status = OfflineCashReleaseStatusV1.installed()
        check(status.matches(releaseId, artifactManifest)) {
            status.blocker ?: "offline-cash-v1-installed-release-mismatch"
        }
        val networkIdBytes = expectedNetworkIdLiteral.toByteArray(StandardCharsets.UTF_8)
        val assetDefinitionIdBytes = expectedAssetDefinitionId.toByteArray(StandardCharsets.UTF_8)
        nativeHandle = try {
            OfflineCashNativeV1.verificationSessionOpenBound(
                request.encodeCanonical(),
                releaseId,
                artifactManifest,
                networkIdBytes,
                assetDefinitionIdBytes,
            )
        } finally {
            networkIdBytes.fill(0)
            assetDefinitionIdBytes.fill(0)
        }
        check(nativeHandle > 0) {
            "native Offline Cash V1 verification session did not return a handle"
        }
    }

    val state: OfflineCashVerificationSessionStateV1
        @Synchronized get() {
            if (nativeHandle == 0L) return OfflineCashVerificationSessionStateV1.UNAVAILABLE
            return runCatching { OfflineCashNativeV1.verificationSessionState(nativeHandle) }
                .map { state ->
                    when (state) {
                        1 -> OfflineCashVerificationSessionStateV1.REQUEST_VERIFIED
                        2 -> OfflineCashVerificationSessionStateV1.PAYMENT_VERIFIED
                        3 -> OfflineCashVerificationSessionStateV1.ACKNOWLEDGEMENT_VERIFIED
                        else -> OfflineCashVerificationSessionStateV1.UNAVAILABLE
                    }
                }
                .getOrDefault(OfflineCashVerificationSessionStateV1.UNAVAILABLE)
        }

    fun expectedReleaseId(): ByteArray = releaseId.copyOf()

    fun expectedArtifactManifestSHA256(): ByteArray = artifactManifest.copyOf()

    @Synchronized
    fun validatedPayment(): OfflineCashPaymentV1? = verifiedPayment

    @Synchronized
    fun validatedAcknowledgement(): OfflineCashAcknowledgementV1? = verifiedAcknowledgement

    @Synchronized
    fun verifyPayment(canonicalNorito: ByteArray): OfflineCashVerificationSessionEventV1 {
        check(nativeHandle != 0L) { "Offline Cash V1 verification session is closed" }
        val observedNowMilliseconds = System.currentTimeMillis()
        check(observedNowMilliseconds > 0) { "system time precedes the Unix epoch" }
        val verificationCanonical = OfflineCashNativeV1.verificationSessionVerifyPayment(
            nativeHandle,
            canonicalNorito,
            observedNowMilliseconds,
        )
        val candidate = OfflineCashPaymentV1(request, verificationCanonical)
        verifiedPayment?.let { existing ->
            if (existing == candidate) {
                return OfflineCashVerificationSessionEventV1.PAYMENT_VERIFICATION_REPLAY
            }
            throw IllegalArgumentException("conflicting Offline Cash V1 payment")
        }
        check(verifiedAcknowledgement == null) { "payment cannot follow acknowledgement" }
        verifiedPayment = candidate
        return OfflineCashVerificationSessionEventV1.PAYMENT_VERIFIED
    }

    @Synchronized
    fun verifyAcknowledgement(
        canonicalNorito: ByteArray,
    ): OfflineCashVerificationSessionEventV1 {
        check(nativeHandle != 0L) { "Offline Cash V1 verification session is closed" }
        val payment = checkNotNull(verifiedPayment) { "acknowledgement requires a payment" }
        val verificationCanonical = OfflineCashNativeV1.verificationSessionVerifyAcknowledgement(
            nativeHandle,
            canonicalNorito,
        )
        val candidate = OfflineCashAcknowledgementV1(request, payment, verificationCanonical)
        verifiedAcknowledgement?.let { existing ->
            if (existing == candidate) {
                return OfflineCashVerificationSessionEventV1.ACKNOWLEDGEMENT_VERIFICATION_REPLAY
            }
            throw IllegalArgumentException("conflicting Offline Cash V1 acknowledgement")
        }
        verifiedAcknowledgement = candidate
        return OfflineCashVerificationSessionEventV1.ACKNOWLEDGEMENT_VERIFIED
    }

    @Synchronized
    override fun close() {
        val handle = nativeHandle
        if (handle == 0L) return
        OfflineCashNativeV1.verificationSessionClose(handle)
        nativeHandle = 0
    }
}

/** Stable product state of the fail-closed Offline Cash V1 wallet facade. */
enum class OfflineCashWalletSessionStateV1(val code: Int) {
    UNAVAILABLE(0),
    SETUP_REQUIRED(1),
    EMPTY(2),
    TOP_UP_PENDING(3),
    AVAILABLE(4),
    RECEIVE_REQUEST_READY(5),
    SEND_PREPARING(6),
    PAYMENT_COMMITTED(7),
    AWAITING_ACKNOWLEDGEMENT(8),
    RECEIVED(9),
    REDEEM_PENDING(10),
    RECOVERY_REQUIRED(11),
    ERROR(12),
}

enum class OfflineCashWalletSessionStatusV1(val code: Int) {
    UNAVAILABLE(0),
}

/** High-level action vocabulary only; codes carry no device or monetary authority. */
enum class OfflineCashWalletSessionActionV1(val code: Int) {
    SET_UP(0),
    TOP_UP(1),
    CREATE_RECEIVE_REQUEST(2),
    PREPARE_SEND(3),
    COMMIT_PAYMENT(4),
    RECORD_ACKNOWLEDGEMENT_EVIDENCE(5),
    RECEIVE_PAYMENT(6),
    REDEEM(7),
    RECOVER(8),
}

enum class OfflineCashWalletSessionErrorV1 {
    UNAVAILABLE,
}

class OfflineCashWalletSessionExceptionV1(
    val reason: OfflineCashWalletSessionErrorV1,
) : IllegalStateException("production Offline Cash V1 wallet runtime is unavailable")

/**
 * Opaque fail-closed product facade for one Offline Cash V1 wallet session.
 *
 * This shell exposes no bytes, native handle, caller clock, emulator constructor, balance,
 * device owner, or state-transition owner. A reviewed secure backend must be integrated before
 * [open] or any action can succeed. Proof verification is exposed separately through
 * [OfflineCashVerificationSessionV1].
 */
class OfflineCashWalletSessionV1 private constructor() {
    val status: OfflineCashWalletSessionStatusV1
        get() = OfflineCashWalletSessionStatusV1.UNAVAILABLE

    val state: OfflineCashWalletSessionStateV1
        get() = OfflineCashWalletSessionStateV1.UNAVAILABLE

    fun attempt(@Suppress("UNUSED_PARAMETER") action: OfflineCashWalletSessionActionV1): Nothing {
        throw OfflineCashWalletSessionExceptionV1(OfflineCashWalletSessionErrorV1.UNAVAILABLE)
    }

    companion object {
        @JvmStatic
        fun unavailable(): OfflineCashWalletSessionV1 = OfflineCashWalletSessionV1()

        @JvmStatic
        fun open(): OfflineCashWalletSessionV1 =
            throw OfflineCashWalletSessionExceptionV1(OfflineCashWalletSessionErrorV1.UNAVAILABLE)
    }
}

/** Strict canonical `kgm2:` peer transport adapter, distinct from PKK1 transport. */
object OfflineCashPeerAdapterV1 {
    const val TEXT_PREFIX: String = "kgm2:"
    const val MAX_RAW_SESSION_BYTES: Int = OfflineCashLimitsV1.RAW_SESSION_MAX_BYTES
    const val MAX_TEXT_SESSION_BYTES: Int = OfflineCashLimitsV1.TEXT_SESSION_MAX_BYTES
    const val MAX_PAYMENT_REQUEST_TEXT_BYTES: Int =
        OfflineCashLimitsV1.PAYMENT_REQUEST_TEXT_MAX_BYTES
    const val MAX_PAYMENT_TEXT_BYTES: Int = OfflineCashLimitsV1.PAYMENT_TEXT_MAX_BYTES
    const val MAX_ACKNOWLEDGEMENT_TEXT_BYTES: Int =
        OfflineCashLimitsV1.ACKNOWLEDGEMENT_TEXT_MAX_BYTES

    @JvmStatic
    fun encodePaymentRequest(request: OfflineCashPaymentRequestV1): String =
        requirePeerText(
            OfflineCashNativeV1.peerEncodePaymentRequest(request.encodeCanonical()),
            MAX_PAYMENT_REQUEST_TEXT_BYTES,
        )

    @JvmStatic
    fun decodePaymentRequest(text: String): OfflineCashPaymentRequestV1 =
        OfflineCashPaymentRequestV1(
            OfflineCashNativeV1.peerDecodePaymentRequest(
                requirePeerText(text, MAX_PAYMENT_REQUEST_TEXT_BYTES),
            ),
        )

    @JvmStatic
    fun encodePayment(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
    ): String = requirePeerText(
        OfflineCashNativeV1.peerEncodePayment(
            request.encodeCanonical(),
            payment.encodeCanonical(),
        ),
        MAX_PAYMENT_TEXT_BYTES,
    )

    @JvmStatic
    fun decodePayment(
        request: OfflineCashPaymentRequestV1,
        text: String,
    ): OfflineCashPaymentV1 = OfflineCashPaymentV1(
        request,
        OfflineCashNativeV1.peerDecodePayment(
            request.encodeCanonical(),
            requirePeerText(text, MAX_PAYMENT_TEXT_BYTES),
        ),
    )

    @JvmStatic
    fun encodeAcknowledgement(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        acknowledgement: OfflineCashAcknowledgementV1,
    ): String = requirePeerText(
        OfflineCashNativeV1.peerEncodeAcknowledgement(
            request.encodeCanonical(),
            payment.encodeCanonical(),
            acknowledgement.encodeCanonical(),
        ),
        MAX_ACKNOWLEDGEMENT_TEXT_BYTES,
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
            requirePeerText(text, MAX_ACKNOWLEDGEMENT_TEXT_BYTES),
        ),
    )

    private fun requirePeerText(text: String, maximumTextBytes: Int): String {
        val bytes = text.toByteArray(StandardCharsets.UTF_8)
        try {
            require(bytes.size <= maximumTextBytes && text.startsWith(TEXT_PREFIX)) {
                "Offline Cash V1 peer text exceeds its kind bound or prefix is invalid"
            }
            return text
        } finally {
            bytes.fill(0)
        }
    }
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

    fun verificationSessionOpenBound(
        request: ByteArray,
        expectedReleaseId: ByteArray,
        expectedArtifactManifestSHA256: ByteArray,
        expectedNetworkId: ByteArray,
        expectedAssetDefinitionId: ByteArray,
    ): Long {
        requireLoaded()
        return nativeVerificationSessionOpenBoundV1(
            request,
            expectedReleaseId,
            expectedArtifactManifestSHA256,
            expectedNetworkId,
            expectedAssetDefinitionId,
        )
    }

    fun verificationSessionVerifyPayment(
        handle: Long,
        payment: ByteArray,
        observedNowMilliseconds: Long,
    ): ByteArray {
        requireLoaded()
        return nativeVerificationSessionVerifyPaymentV1(handle, payment, observedNowMilliseconds)
    }

    fun verificationSessionVerifyAcknowledgement(
        handle: Long,
        acknowledgement: ByteArray,
    ): ByteArray {
        requireLoaded()
        return nativeVerificationSessionVerifyAcknowledgementV1(handle, acknowledgement)
    }

    fun verificationSessionState(handle: Long): Int {
        requireLoaded()
        return nativeVerificationSessionStateV1(handle)
    }

    fun verificationSessionClose(handle: Long) {
        requireLoaded()
        nativeVerificationSessionCloseV1(handle)
    }

    fun artifactBegin(manifest: ByteArray, role: Int): Long {
        requireLoaded()
        return nativeArtifactBeginV1(manifest, role)
    }

    fun artifactWrite(handle: Long, chunk: ByteArray) {
        requireLoaded()
        nativeArtifactWriteV1(handle, chunk)
    }

    fun artifactFinalize(handle: Long) {
        requireLoaded()
        nativeArtifactFinalizeV1(handle)
    }

    fun artifactCancel(handle: Long) {
        requireLoaded()
        nativeArtifactCancelV1(handle)
    }

    fun artifactSetInstall(
        manifest: ByteArray,
        expectedManifestSHA256: ByteArray,
        validationReceipt: ByteArray,
        trustedPolicy: ByteArray,
        releaseAttestation: ByteArray,
        handles: LongArray,
    ) {
        requireLoaded()
        nativeArtifactSetInstallV1(
            manifest,
            expectedManifestSHA256,
            validationReceipt,
            trustedPolicy,
            releaseAttestation,
            handles,
        )
    }

    fun artifactSetUninstall(
        expectedReleaseId: ByteArray,
        expectedManifestSHA256: ByteArray,
    ) {
        requireLoaded()
        nativeArtifactSetUninstallV1(expectedReleaseId, expectedManifestSHA256)
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
    @JvmStatic private external fun nativeVerificationSessionOpenV1(
        request: ByteArray,
        expectedReleaseId: ByteArray,
        expectedArtifactManifestSHA256: ByteArray,
    ): Long
    @JvmStatic private external fun nativeVerificationSessionOpenBoundV1(
        request: ByteArray,
        expectedReleaseId: ByteArray,
        expectedArtifactManifestSHA256: ByteArray,
        expectedNetworkId: ByteArray,
        expectedAssetDefinitionId: ByteArray,
    ): Long
    @JvmStatic private external fun nativeVerificationSessionVerifyPaymentV1(
        handle: Long,
        payment: ByteArray,
        observedNowMilliseconds: Long,
    ): ByteArray
    @JvmStatic private external fun nativeVerificationSessionVerifyAcknowledgementV1(
        handle: Long,
        acknowledgement: ByteArray,
    ): ByteArray
    @JvmStatic private external fun nativeVerificationSessionStateV1(handle: Long): Int
    @JvmStatic private external fun nativeVerificationSessionCloseV1(handle: Long)
    // Disjoint fail-closed product shell. ABI22 exports these symbols for
    // explicit status/ABI honesty, but no SDK path may retain the zero handle
    // or treat symbol presence as production enablement.
    @JvmStatic private external fun nativeWalletRuntimeSessionOpenV1(): Long
    @JvmStatic private external fun nativeWalletRuntimeSessionStatusV1(): ByteArray
    @JvmStatic private external fun nativeWalletRuntimeSessionAttemptV1(handle: Long, action: Int)
    @JvmStatic private external fun nativeWalletRuntimeSessionCloseV1(handle: Long)
    @JvmStatic private external fun nativeArtifactBeginV1(manifest: ByteArray, role: Int): Long
    @JvmStatic private external fun nativeArtifactWriteV1(handle: Long, chunk: ByteArray)
    @JvmStatic private external fun nativeArtifactFinalizeV1(handle: Long)
    @JvmStatic private external fun nativeArtifactCancelV1(handle: Long)
    @JvmStatic private external fun nativeArtifactSetInstallV1(
        manifest: ByteArray,
        expectedManifestSHA256: ByteArray,
        validationReceipt: ByteArray,
        trustedPolicy: ByteArray,
        releaseAttestation: ByteArray,
        handles: LongArray,
    )
    @JvmStatic private external fun nativeArtifactSetUninstallV1(
        expectedReleaseId: ByteArray,
        expectedManifestSHA256: ByteArray,
    )
}
