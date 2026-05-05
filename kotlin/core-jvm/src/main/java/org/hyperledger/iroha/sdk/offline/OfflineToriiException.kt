package org.hyperledger.iroha.sdk.offline

/** Runtime error raised when offline Torii endpoints reject a request or parsing fails. */
class OfflineToriiException : RuntimeException {

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
        this.rejectCode = if (rejectCode.isNullOrBlank()) null else rejectCode
        this.responseBody = if (responseBody.isNullOrBlank()) null else responseBody
    }

    companion object {
        private const val serialVersionUID: Long = 1L
    }
}
