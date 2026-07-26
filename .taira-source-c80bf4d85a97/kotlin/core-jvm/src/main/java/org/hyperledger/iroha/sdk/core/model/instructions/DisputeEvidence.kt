package org.hyperledger.iroha.sdk.core.model.instructions

/** Evidence metadata recorded with a capacity dispute. */
class DisputeEvidence(
    @JvmField val digestHex: String,
    @JvmField val mediaType: String? = null,
    @JvmField val uri: String? = null,
    @JvmField val sizeBytes: Long? = null,
) {
    init {
        require(digestHex.isNotBlank()) { "digestHex must not be blank" }
        if (sizeBytes != null) {
            require(sizeBytes >= 0) { "sizeBytes must be non-negative" }
        }
    }

    fun appendArguments(args: MutableMap<String, String>) {
        args["evidence.digest_hex"] = digestHex
        if (!mediaType.isNullOrBlank()) args["evidence.media_type"] = mediaType
        if (!uri.isNullOrBlank()) args["evidence.uri"] = uri
        if (sizeBytes != null) args["evidence.size_bytes"] = sizeBytes.toString()
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is DisputeEvidence) return false
        return digestHex == other.digestHex
            && mediaType == other.mediaType
            && uri == other.uri
            && sizeBytes == other.sizeBytes
    }

    override fun hashCode(): Int = listOf(digestHex, mediaType, uri, sizeBytes).hashCode()
}
