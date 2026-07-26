package org.hyperledger.iroha.sdk.nexus

/** Immutable view over `/v1/space-directory/uaids/{uaid}` responses. */
class UaidBindingsResponse(
    @JvmField val uaid: String,
    dataspaces: List<UaidBindingsDataspace>,
) {
    @JvmField val dataspaces: List<UaidBindingsDataspace> = dataspaces.toList()

    /** Dataspace binding entry returned by Torii. */
    class UaidBindingsDataspace(
        @JvmField val dataspaceId: Long,
        @JvmField val dataspaceAlias: String?,
        accounts: List<String>,
    ) {
        @JvmField val accounts: List<String> = accounts.toList()
    }
}
