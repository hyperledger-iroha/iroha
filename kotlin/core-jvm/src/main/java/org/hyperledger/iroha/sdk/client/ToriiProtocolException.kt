package org.hyperledger.iroha.sdk.client

/**
 * A Torii response that does not follow its contract (malformed JSON, a missing required row
 * field, a cursor that does not advance, ...). [code] is always `invalid_response`.
 */
class ToriiProtocolException @JvmOverloads constructor(
    status: Int,
    message: String,
    cause: Throwable? = null,
) : ToriiApiException(status, CODE, message, null, null, cause) {
    companion object {
        /** The [ToriiApiException.code] of every protocol violation. */
        const val CODE: String = "invalid_response"
    }
}
