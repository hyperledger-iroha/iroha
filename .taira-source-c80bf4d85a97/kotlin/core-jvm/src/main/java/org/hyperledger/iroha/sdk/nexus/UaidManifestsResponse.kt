package org.hyperledger.iroha.sdk.nexus

import org.hyperledger.iroha.sdk.client.JsonParser
import java.util.Collections

/** Immutable view over `/v1/space-directory/uaids/{uaid}/manifests` responses. */
class UaidManifestsResponse(
    @JvmField val uaid: String,
    total: Long,
    manifests: List<UaidManifestRecord>,
) {
    @JvmField val total: Long = maxOf(0L, total)
    @JvmField val manifests: List<UaidManifestRecord> = manifests.toList()

    /** Manifest entry with lifecycle metadata and bound accounts. */
    class UaidManifestRecord(
        @JvmField val dataspaceId: Long,
        @JvmField val dataspaceAlias: String?,
        @JvmField val manifestHash: String,
        @JvmField val status: UaidManifestStatus,
        @JvmField val lifecycle: UaidManifestLifecycle,
        accounts: List<String>,
        @JvmField val manifestJson: String,
    ) {
        @JvmField val accounts: List<String> = accounts.toList()

        /**
         * Parses the manifest JSON into an immutable map representation.
         *
         * @throws IllegalStateException if the payload is not a JSON object
         */
        @Suppress("UNCHECKED_CAST")
        fun manifestAsMap(): Map<String, Any> {
            if (manifestJson.isBlank()) return emptyMap()
            val parsed = JsonParser.parse(manifestJson)
            check(parsed is Map<*, *>) { "manifest is not a JSON object" }
            return Collections.unmodifiableMap(parsed as Map<String, Any>)
        }
    }

    /** Lifecycle metadata attached to a manifest. */
    class UaidManifestLifecycle(
        @JvmField val activatedEpoch: Long?,
        @JvmField val expiredEpoch: Long?,
        @JvmField val revocation: UaidManifestRevocation?,
    )

    /** Revocation metadata bundled with the lifecycle. */
    class UaidManifestRevocation(
        @JvmField val epoch: Long,
        @JvmField val reason: String?,
    )

    /** Manifest status as emitted by Torii. */
    enum class UaidManifestStatus {
        PENDING,
        ACTIVE,
        EXPIRED,
        REVOKED,
    }
}
