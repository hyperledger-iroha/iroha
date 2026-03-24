package org.hyperledger.iroha.sdk.nexus

/** Query parameters for `/v1/space-directory/uaids/{uaid}` bindings endpoint. */
class UaidBindingsQuery(
    val addressFormat: AddressFormatOption? = null,
) {
    fun toQueryParameters(): Map<String, String> = buildMap {
        addressFormat?.let { put("address_format", it.parameterValue) }
    }
}
