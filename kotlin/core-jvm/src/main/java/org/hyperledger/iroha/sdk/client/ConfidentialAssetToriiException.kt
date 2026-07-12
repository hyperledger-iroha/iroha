package org.hyperledger.iroha.sdk.client

/** Runtime error raised when a confidential-asset Torii request fails. */
class ConfidentialAssetToriiException : RuntimeException {
    val statusCode: Int?
    val rejectCode: String?
    val responseBody: String?

    constructor(message: String) : this(message, null, null, null, null)

    constructor(message: String, cause: Throwable?) : this(message, cause, null, null, null)

    constructor(
        message: String,
        statusCode: Int?,
        rejectCode: String?,
        responseBody: String?,
    ) : this(message, null, statusCode, rejectCode, responseBody)

    constructor(
        message: String,
        cause: Throwable?,
        statusCode: Int?,
        rejectCode: String?,
        responseBody: String?,
    ) : super(message, cause) {
        this.statusCode = statusCode
        this.rejectCode = rejectCode?.takeUnless(String::isBlank)
        this.responseBody = responseBody?.takeUnless(String::isBlank)
    }

    companion object {
        private const val serialVersionUID: Long = 1L
    }
}
