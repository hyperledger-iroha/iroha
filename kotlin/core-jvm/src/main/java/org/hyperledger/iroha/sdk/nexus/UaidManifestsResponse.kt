package org.hyperledger.iroha.sdk.nexus

import org.hyperledger.iroha.sdk.client.JsonParser
import java.util.Collections

/** Immutable view over `/v1/space-directory/uaids/{uaid}/manifests` responses. */
class UaidManifestsResponse(
    @JvmField val uaid: String,
    @JvmField val total: Long,
    @JvmField val hasMore: Boolean,
    @JvmField val countMode: UaidManifestCountMode,
    manifests: List<UaidManifestRecord>,
) {
    init {
        UaidLiteral.canonicalize(uaid, "uaid")
        require(total >= 0) { "total must be non-negative" }
    }

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
        init {
            require(dataspaceId >= 0) { "dataspaceId must be non-negative" }
            require(manifestHash.matches(Regex("[0-9a-f]{64}"))) {
                "manifestHash must be exactly 64 lowercase hexadecimal characters"
            }
            require(manifestJson.isNotBlank()) { "manifestJson must be a JSON object" }
        }

        @JvmField val accounts: List<String> = accounts.toList()

        /**
         * Parses the manifest JSON into an immutable map representation.
         *
         * @throws IllegalStateException if the payload is not a JSON object
         */
        @Suppress("UNCHECKED_CAST")
        fun manifestAsMap(): Map<String, Any> {
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
    ) {
        init {
            require(activatedEpoch == null || activatedEpoch >= 0) {
                "activatedEpoch must be non-negative"
            }
            require(expiredEpoch == null || expiredEpoch >= 0) {
                "expiredEpoch must be non-negative"
            }
        }
    }

    /** Revocation metadata bundled with the lifecycle. */
    class UaidManifestRevocation(
        @JvmField val epoch: Long,
        @JvmField val reason: String?,
    ) {
        init {
            require(epoch >= 0) { "epoch must be non-negative" }
        }
    }

    /** Manifest status as emitted by Torii. */
    enum class UaidManifestStatus {
        PENDING,
        ACTIVE,
        EXPIRED,
        REVOKED,
    }
}
