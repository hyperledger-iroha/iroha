package org.hyperledger.iroha.sdk.connect

/** Result of decoding a raw Connect frame. */
class DecodedFrame internal constructor(
    sessionId: ByteArray,
    @JvmField val direction: ConnectDirection,
    @JvmField val sequence: Long,
    @JvmField val type: FrameType,
    @JvmField val open: OpenControl?,
    @JvmField val reject: RejectControl?,
    @JvmField val close: CloseControl?,
    @JvmField val ciphertext: ConnectCiphertext?,
) {
    private val _sessionId: ByteArray = sessionId.copyOf()

    fun sessionId(): ByteArray = _sessionId.clone()
}
