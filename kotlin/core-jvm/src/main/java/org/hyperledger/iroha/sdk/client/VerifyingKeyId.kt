package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyBackendTag

/** Exact identifier returned by Torii's verifying-key registry id projection. */
class VerifyingKeyId internal constructor(
    /** Exact verifier-registry backend label. */
    @JvmField val backend: String,
    /** Exact verifying-key registry name. */
    @JvmField val name: String,
) {
    /** Low-level proof engine bound to this exact verifier-registry label. */
    fun engine(): VerifyingKeyBackendTag =
        checkNotNull(VerifyingKeyBackendTag.verifierBackendRegistryTagV1(backend)) {
            "unsupported verifier-registry backend $backend"
        }

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is VerifyingKeyId && backend == other.backend && name == other.name

    override fun hashCode(): Int = 31 * backend.hashCode() + name.hashCode()

    override fun toString(): String = "$backend:$name"
}
