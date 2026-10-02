package org.hyperledger.iroha.sdk.offline

/** Test artifact only: exact final named inert transport for metadata/monitor tests.
 * No JNI, runtime registration, account, signature, clock or Native authority is created.
 * This file must remain under src/test; wallet production owns the real main-source class.
 */
internal object KagemushaOrdinaryRuntimeJniV1 : KagemushaOrdinaryNativeStartupEndpointV1,
    KagemushaOrdinaryNativeCurrentControlEndpointV1 {
    @Volatile var startupScript: (Int, Long, ByteArray) -> Array<ByteArray>? =
        { _, _, _ -> error("No inert startup script selected") }
    override fun startup(phase: Int, readId: Long, original: ByteArray): Array<ByteArray>? =
        startupScript(phase, readId, original)
    override fun invoke(phase: Int, coreHandle: Long, signedOriginal: ByteArray,
        authorityOriginal: ByteArray): Array<ByteArray>? = error("No current-FI dispatch in this inert fixture")
}
