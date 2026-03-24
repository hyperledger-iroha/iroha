package org.hyperledger.iroha.sdk.client

/** Simple container for responses returned by `IrohaClient` implementations. */
class ClientResponse(
    @JvmField val statusCode: Int,
    body: ByteArray,
    @JvmField val message: String = "",
    transactionHashHex: String? = null,
    rejectCode: String? = null,
) {
    private val _body: ByteArray = body.copyOf()

    @JvmField val transactionHashHex: String? = transactionHashHex
    @JvmField val rejectCode: String? = if (rejectCode.isNullOrBlank()) null else rejectCode

    val body: ByteArray get() = _body.copyOf()

    fun hashHex(): String? = transactionHashHex

    fun rejectCode(): String? = rejectCode
}
