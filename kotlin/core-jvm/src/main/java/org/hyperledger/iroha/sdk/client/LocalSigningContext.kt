package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.core.model.NetworkId

/**
 * Immutable local context used to validate server-prepared transaction drafts before signing.
 *
 * The exact network identity is configured by the caller and is never inferred from a server
 * response.
 */
class LocalSigningContext(
    private val networkId: NetworkId,
) {
    /** Exact canonical genesis-hash identity required in every locally signed draft. */
    fun networkId(): NetworkId = networkId

    override fun equals(other: Any?): Boolean =
        this === other || other is LocalSigningContext && networkId == other.networkId

    override fun hashCode(): Int = networkId.hashCode()
}
