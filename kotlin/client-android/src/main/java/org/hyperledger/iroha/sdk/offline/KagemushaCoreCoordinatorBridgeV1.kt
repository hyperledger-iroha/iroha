// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

/** JNI field-array endpoint; production implementations invoke the authenticated native bridge. */
internal interface KagemushaCoreCoordinatorEndpointV1 {
    fun contract(): IntArray?
    fun open(storagePath: String): Long
    fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>?
}

/**
 * Serialized transport to the process-owned native coordinator, with no software backend.
 *
 * Contract matching proves ABI compatibility only. A generic bridge refuses [open] until its
 * qualified Rust backend is installed. Returned Norito archives stay opaque at this layer.
 * The native ABI owns handles for the process lifetime and exposes no close/reset operation.
 */
class KagemushaCoreCoordinatorBridgeV1 private constructor(
    private val endpoint: KagemushaCoreCoordinatorEndpointV1,
    private val handle: Long,
) {
    /** Invoke one method only after strict framing; reject substituted response identities. */
    @Synchronized
    fun invoke(method: KagemushaCoreCoordinatorMethodV1, fields: List<ByteArray>): List<ByteArray> {
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, fields)
        val nativeFields = KagemushaCoreCoordinatorFrameV1.decodeRequest(method, request).toTypedArray()
        val response = try {
            endpoint.invoke(handle, method.code, nativeFields)
                ?: throw IllegalStateException("KAGEMUSHA native coordinator rejected or could not execute the method")
        } catch (error: LinkageError) {
            throw IllegalStateException("KAGEMUSHA native coordinator is unavailable", error)
        }
        val responseFrame = KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, response.toList())
        return KagemushaCoreCoordinatorFrameV1.decodeResponse(method, request, responseFrame)
    }

    companion object {
        private val expectedContract = intArrayOf(2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff)

        /** Open the exact native ABI. Missing JNI/backend or a mismatched contract fails closed. */
        @JvmStatic
        fun open(storagePath: String): KagemushaCoreCoordinatorBridgeV1 {
            validatePath(storagePath)
            try {
                System.loadLibrary("connect_norito_bridge")
                return openEndpoint(storagePath, KagemushaCoreCoordinatorJniV1)
            } catch (error: LinkageError) {
                throw IllegalStateException("KAGEMUSHA native coordinator is unavailable", error)
            }
        }

        internal fun openEndpoint(
            storagePath: String,
            endpoint: KagemushaCoreCoordinatorEndpointV1,
        ): KagemushaCoreCoordinatorBridgeV1 {
            validatePath(storagePath)
            check(endpoint.contract()?.contentEquals(expectedContract) == true) {
                "KAGEMUSHA native coordinator contract mismatch"
            }
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

/** Exact JNI owner; these methods have no Java/Kotlin monetary implementation. */
internal object KagemushaCoreCoordinatorJniV1 : KagemushaCoreCoordinatorEndpointV1 {
    override fun contract(): IntArray? = nativeContractV1()
    override fun open(storagePath: String): Long = nativeOpenV1(storagePath)
    override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? =
        nativeInvokeV1(handle, method, fields)

    @JvmStatic private external fun nativeContractV1(): IntArray?
    @JvmStatic private external fun nativeOpenV1(storagePath: String): Long
    @JvmStatic private external fun nativeInvokeV1(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>?
}
