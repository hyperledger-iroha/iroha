// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

/** JNI field-array endpoint; production implementations invoke the authenticated native bridge. */
internal interface KagemushaCoreCoordinatorEndpointV1 {
    fun contract(): IntArray?
    fun install(storagePath: String): Int
    fun open(storagePath: String): Long
    fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>?
    fun close(handle: Long): Int
}

/**
 * Serialized transport to the process-owned native coordinator, with no software backend.
 *
 * Contract matching proves ABI compatibility only. A generic bridge refuses [open] until its
 * qualified Rust provider is installed. Only that provider supplies the retained release,
 * policy, durable journal and hardware authority; the JVM intake supplies a storage path.
 * Returned Norito archives stay opaque at this layer.
 * Explicit close and every post-dispatch failure revoke the handle. The native ABI never
 * reopens in the same process.
 */
class KagemushaCoreCoordinatorBridgeV1 private constructor(
    private val endpoint: KagemushaCoreCoordinatorEndpointV1,
    private var handle: Long,
) : AutoCloseable {
    /** Invoke one method only after strict framing; reject substituted response identities. */
    @Synchronized
    fun invoke(method: KagemushaCoreCoordinatorMethodV1, fields: List<ByteArray>): List<ByteArray> =
        invokeOriginal(method, fields, ordinaryBootstrap = false)

    @Synchronized
    internal fun invokeOrdinaryBootstrapApproval(fields: List<ByteArray>): List<ByteArray> =
        invokeOriginal(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL, fields, ordinaryBootstrap = true)

    @Synchronized
    internal fun requireOrdinaryDescriptorOpen() {
        check(handle != 0L) { "KAGEMUSHA native coordinator handle is closed" }
    }

    @Synchronized
    internal fun invokeOrdinaryCurrentControl(endpoint: KagemushaOrdinaryNativeCurrentControlEndpointV1,
        phase: Int, signed: ByteArray, authority: ByteArray): List<ByteArray> {
        // Keep the sole wallet-owned JNI class without introducing a client -> wallet dependency.
        // Its exact name is retained by the wallet consumer rules. Name alone is insufficient:
        // a class with that name in another loader must never receive this private descriptor.
        // These are class metadata checks, with no reflective construction or member lookup.
        requireOrdinaryRuntimeJniOwnerClassV1(endpoint.javaClass)
        // Refusing an offered callback precedes even reading the handle and cannot revoke it.
        requireOrdinaryDescriptorOpen()
        val selected = handle
        return try {
            val response = endpoint.invoke(phase, selected, signed.copyOf(), authority.copyOf())
                ?: error("Actual Native current FI control is unavailable")
            val fields = ordinaryCurrentControlResponseFieldsV1(phase, selected, response)
            check(handle == selected)
            fields
        } catch (error: Throwable) {
            val closing = handle; handle = 0L
            try { this.endpoint.close(closing) } catch (_: Throwable) { }
            if (error is LinkageError) throw IllegalStateException("Actual Native current FI control is unavailable", error)
            throw error
        }
    }

    /** Startup shares the descriptor monitor with close; no owner callback runs here. */
    @Synchronized
    internal fun invokeOrdinaryStartup(endpoint: KagemushaOrdinaryNativeStartupEndpointV1,
        phase: Int, readId: Long): List<ByteArray> {
        // Reject another class/loader before even reading this private descriptor.
        requireOrdinaryRuntimeJniOwnerClassV1(endpoint.javaClass)
        require((phase == 1 && readId == 0L) || (phase == 6 && readId != 0L))
        requireOrdinaryDescriptorOpen()
        val selected = handle
        return try {
            val response = endpoint.startup(phase, readId, ByteArray(0))
                ?: error("Actual Native ordinary session refresh is unavailable")
            val fields = ordinaryNativeStartupResponseFieldsV1(phase, response)
            check(handle == selected)
            fields
        } catch (error: Throwable) {
            val closing = handle; handle = 0L
            try { this.endpoint.close(closing) } catch (_: Throwable) { }
            if (error is LinkageError) throw IllegalStateException("Actual Native ordinary session refresh is unavailable", error)
            throw error
        }
    }

    private fun invokeOriginal(method: KagemushaCoreCoordinatorMethodV1, fields: List<ByteArray>, ordinaryBootstrap: Boolean): List<ByteArray> {
        check(handle != 0L) { "KAGEMUSHA native coordinator handle is closed" }
        val request = if (ordinaryBootstrap) KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalRequest(fields)
            else KagemushaCoreCoordinatorFrameV1.encodeRequest(method, fields)
        val nativeFields = (if (ordinaryBootstrap) KagemushaCoreCoordinatorFrameV1.decodeOrdinaryBootstrapApprovalRequest(request)
            else KagemushaCoreCoordinatorFrameV1.decodeRequest(method, request)).toTypedArray()
        return try {
            val response = endpoint.invoke(handle, method.code, nativeFields)
                ?: throw IllegalStateException("KAGEMUSHA native coordinator rejected or could not execute the method")
            if (ordinaryBootstrap) {
                val responseFrame = KagemushaCoreCoordinatorFrameV1.encodeOrdinaryBootstrapApprovalResponse(request, response.toList())
                KagemushaCoreCoordinatorFrameV1.decodeOrdinaryBootstrapApprovalResponse(request, responseFrame)
            } else {
                val responseFrame = KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, response.toList())
                KagemushaCoreCoordinatorFrameV1.decodeResponse(method, request, responseFrame)
            }
        } catch (error: Throwable) {
            // Dispatch may have advanced hardware even when JNI or response decoding failed.
            // Drop the local handle before asking native Core to revoke its owner.
            val closing = handle
            handle = 0L
            try {
                endpoint.close(closing)
            } catch (_: Throwable) {
                // The original uncertain result remains the failure; no retry is allowed here.
            }
            if (error is LinkageError) {
                throw IllegalStateException("KAGEMUSHA native coordinator is unavailable", error)
            }
            throw error
        }
    }

    /** Revoke this handle before native teardown; reopening requires a fresh process. */
    @Synchronized
    override fun close() {
        val closing = handle
        if (closing == 0L) return
        handle = 0L
        val status = try {
            endpoint.close(closing)
        } catch (error: LinkageError) {
            throw IllegalStateException("KAGEMUSHA native coordinator close is unavailable", error)
        }
        check(status == 0) { "KAGEMUSHA native coordinator close failed: $status" }
    }

    companion object {
        private val expectedContract = intArrayOf(2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21)
        // Process-local ordering only. Native OnceLock/source custody cannot be replaced here.
        private val productionEntry = KagemushaInitialOrdinaryStartupGateV1()

        /** Open the exact native ABI. Missing JNI/backend or a mismatched contract fails closed. */
        @JvmStatic
        fun open(storagePath: String): KagemushaCoreCoordinatorBridgeV1 {
            validatePath(storagePath)
            return productionEntry.open {
                openEndpoint(storagePath, loadOriginalEndpoint())
            }
        }

        /** Run the initial fixed Native account read before any production Core open attempt.
         * Only the exact wallet-owned JNI class may dispatch; no Core descriptor or authority is
         * returned. Missing registered Native root/account, failed read or ABI mismatch refuses.
         * One attempt is allowed per process. Replacement/recovery needs genuine fresh process
         * composition; this is not a reset or a monetary-readiness method.
         */
        @JvmStatic
        fun selectInitialOrdinaryAccount(endpoint: KagemushaOrdinaryNativeStartupEndpointV1) {
            // Reject caller callbacks before loader/probe/startup and before the process fence.
            requireOrdinaryRuntimeJniOwnerClassV1(endpoint.javaClass)
            productionEntry.select {
                loadOriginalEndpoint() // Exact existing library/ABI probe only, without install/open.
                completeInitialOrdinaryStartupV1(endpoint)
            }
        }

        private fun loadOriginalEndpoint(): KagemushaCoreCoordinatorEndpointV1 {
            try {
                System.loadLibrary("connect_norito_bridge")
                requireOriginalContract(KagemushaCoreCoordinatorJniV1)
                return KagemushaCoreCoordinatorJniV1
            } catch (error: LinkageError) {
                throw IllegalStateException("KAGEMUSHA native coordinator is unavailable", error)
            }
        }

        private fun requireOriginalContract(endpoint: KagemushaCoreCoordinatorEndpointV1) {
            check(endpoint.contract()?.contentEquals(expectedContract) == true) {
                "KAGEMUSHA native coordinator contract mismatch"
            }
        }

        internal fun openEndpoint(
            storagePath: String,
            endpoint: KagemushaCoreCoordinatorEndpointV1,
        ): KagemushaCoreCoordinatorBridgeV1 {
            validatePath(storagePath)
            requireOriginalContract(endpoint)
            // Native installation is idempotent only for the same successfully provisioned
            // original owner/path. It refuses unknown preinstalled or uncertain owners.
            val status = endpoint.install(storagePath)
            if (status == -312) {
                throw KagemushaNativeProvisioningUnavailableExceptionV1()
            }
            check(status == 0) { "KAGEMUSHA native provisioning rejected the original owner: $status" }
            val handle = endpoint.open(storagePath)
            check(handle != 0L) { "KAGEMUSHA qualified native coordinator is unavailable" }
            return KagemushaCoreCoordinatorBridgeV1(endpoint, handle)
        }

        private fun validatePath(storagePath: String) {
            require(storagePath.isNotBlank() && '\u0000' !in storagePath) { "invalid coordinator storage path" }
            // Reject malformed UTF-16 rather than silently replacing a path component at JNI.
            var index = 0
            while (index < storagePath.length) {
                val character = storagePath[index++]
                if (Character.isHighSurrogate(character)) {
                    require(index < storagePath.length && Character.isLowSurrogate(storagePath[index++])) { "invalid storage path Unicode" }
                } else {
                    require(!Character.isLowSurrogate(character)) { "invalid storage path Unicode" }
                }
            }
            require(storagePath.toByteArray(Charsets.UTF_8).size <= 4096) { "oversized coordinator storage path" }
        }
    }
}

/** One private production instance orders initial selection and every production Core open.
 * Internal instances test ordering/failure only; they contain no JNI, descriptor or owner.
 */
internal class KagemushaInitialOrdinaryStartupGateV1 {
    private var selectionAttempted = false
    private var selectionInProgress = false
    private var selectionUnavailable = false
    private var coreOpenAttempted = false

    @Synchronized
    fun select(action: () -> Unit) {
        check(!selectionAttempted && !coreOpenAttempted) {
            "Initial ordinary Native selection requires a fresh process before Core open"
        }
        selectionAttempted = true // Loader/ABI failure also consumes this process-local attempt.
        selectionInProgress = true
        try { action() } catch (failure: Throwable) {
            selectionUnavailable = true
            throw failure
        } finally { selectionInProgress = false }
    }

    @Synchronized
    fun <T> open(action: () -> T): T {
        check(!selectionInProgress && !selectionUnavailable) {
            "Initial ordinary Native selection is incomplete or unavailable"
        }
        coreOpenAttempted = true // No later initial-selection rescue of an uncertain Core open.
        return action()
    }
}

/** Internal transport seam only. Success returns Unit, never S/W, nonce, session ID or authority.
 * Native phase6 authenticates the complete certified S/W originals and installs its same source.
 * Its returned session ID is distinct from the read ID; only nonzero framing is required.
 */
internal fun completeInitialOrdinaryStartupV1(endpoint: KagemushaOrdinaryNativeStartupEndpointV1) {
    try {
        val reservation = ordinaryNativeStartupResponseFieldsV1(1,
            endpoint.startup(1, 0L, ByteArray(0))
                ?: error("Actual initial ordinary Native account read is unavailable"))
        val rawId = reservation[2]
        var readId = 0L
        for (index in rawId.indices) readId = readId or ((rawId[index].toLong() and 255L) shl (index * 8))
        ordinaryNativeStartupResponseFieldsV1(6,
            endpoint.startup(6, readId, ByteArray(0))
                ?: error("Actual initial ordinary Native account selection is unavailable"))
    } catch (error: LinkageError) {
        throw IllegalStateException("Actual initial ordinary Native account selection is unavailable", error)
    }
}

/** Exact JNI class metadata only; no descriptor, member lookup or construction is exposed. */
internal fun requireOrdinaryRuntimeJniOwnerClassV1(endpointClass: Class<*>) {
    require(endpointClass.name == "org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryRuntimeJniV1" &&
        (endpointClass.modifiers and 0x0010) != 0 &&
        endpointClass.classLoader === KagemushaCoreCoordinatorBridgeV1::class.java.classLoader) {
        "Ordinary Native dispatch requires the original final SDK wallet JNI owner"
    }
}

/** Pure startup transport framing; it supplies no installed account or session authority. */
internal fun ordinaryNativeStartupResponseFieldsV1(phase: Int, response: Array<ByteArray>): List<ByteArray> {
    require(phase == 1 || phase == 6)
    require(response.size == if (phase == 1) 6 else 3)
    require(response[0].contentEquals(byteArrayOf(1, 0)) &&
        response[1].contentEquals(byteArrayOf(phase.toByte())) &&
        response[2].size == 8 && response[2].any { it != 0.toByte() })
    if (phase == 1) {
        require(response[3].size == 32 && response[3].any { it != 0.toByte() })
        require(response[4].size in 1..4096 && response[5].size in 1..4096)
    }
    return response.map { it.copyOf() }
}

/** Pure transport correlation only. Caller-supplied bytes cannot create a descriptor or authority. */
internal fun ordinaryCurrentControlResponseFieldsV1(
    phase: Int,
    correlation: Long,
    response: Array<ByteArray>,
): List<ByteArray> {
    require(phase in 1..3 && correlation != 0L)
    require(response.size == if (phase == 3) 3 else 5)
    require(response[0].contentEquals(byteArrayOf(1, 0)) &&
        response[1].contentEquals(byteArrayOf(phase.toByte())) &&
        response[2].contentEquals(ByteArray(8) { (correlation ushr (it * 8)).toByte() }))
    if (phase == 3) return emptyList()
    require(response[3].size in 1..KagemushaOrdinaryCurrentControlHttpCodecV1.MAXIMUM_REQUEST_BYTES)
    if (phase == 1) require(response[4].contentEquals(
        "iroha:kagemusha:v1:ordinary-current-fi-control-request\u0000".toByteArray(Charsets.US_ASCII) + response[3]))
    else require(response[4].size == 64)
    return listOf(response[3].copyOf(), response[4].copyOf())
}

/** No independently qualified native platform provider is installed for this process. */
class KagemushaNativeProvisioningUnavailableExceptionV1 : IllegalStateException(
    "KAGEMUSHA qualified native platform provisioning is unavailable",
)

/** Exact JNI owner; these methods have no Java/Kotlin monetary implementation. */
internal object KagemushaCoreCoordinatorJniV1 : KagemushaCoreCoordinatorEndpointV1 {
    override fun contract(): IntArray? = nativeContractV1()
    override fun install(storagePath: String): Int = nativeInstallV1(storagePath)
    override fun open(storagePath: String): Long = nativeOpenV1(storagePath)
    override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? =
        nativeInvokeV1(handle, method, fields)
    override fun close(handle: Long): Int = nativeCloseV1(handle)

    @JvmStatic private external fun nativeContractV1(): IntArray?
    @JvmStatic private external fun nativeInstallV1(storagePath: String): Int
    @JvmStatic private external fun nativeOpenV1(storagePath: String): Long
    @JvmStatic private external fun nativeInvokeV1(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>?
    @JvmStatic private external fun nativeCloseV1(handle: Long): Int
}
