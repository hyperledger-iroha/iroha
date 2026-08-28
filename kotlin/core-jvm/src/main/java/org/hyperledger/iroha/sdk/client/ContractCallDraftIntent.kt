package org.hyperledger.iroha.sdk.client

import java.util.Collections
import java.util.LinkedHashMap
import org.hyperledger.iroha.sdk.core.model.ContractInvocation
import org.hyperledger.iroha.sdk.core.model.JsonValue

/**
 * Caller-trusted contract-call intent used to verify an unsigned Torii draft.
 *
 * Construct the invocation from a trusted contract artifact and exact argument schema. The
 * metadata is the complete final transaction metadata the caller intends to sign; it is kept
 * off-wire and is never populated from response echoes.
 */
class ContractCallDraftIntent(
    @JvmField val invocation: ContractInvocation,
    metadata: Map<String, JsonValue>,
) {
    private val _metadata: Map<String, JsonValue> =
        Collections.unmodifiableMap(LinkedHashMap(metadata))

    /** Exact signature-bound transaction metadata authorized by the caller. */
    val metadata: Map<String, JsonValue> get() = _metadata

    init {
        _metadata.keys.forEach { key ->
            require(key.isNotBlank()) { "metadata key must not be blank" }
        }
    }

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is ContractCallDraftIntent &&
            invocation == other.invocation &&
            _metadata == other._metadata

    override fun hashCode(): Int = 31 * invocation.hashCode() + _metadata.hashCode()
}
