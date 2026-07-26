package org.hyperledger.iroha.sdk.client

/** Preferred Torii wire format for request negotiation. */
enum class WireFormatPreference(private val header: String) {
    /** Prefer Norito while allowing JSON responses from peers that negotiate it. */
    NORITO_PREFERRED("application/x-norito, application/json;q=0.8"),

    /** Prefer JSON while allowing Norito responses. */
    JSON_PREFERRED("application/json, application/x-norito;q=0.8"),

    /** Request only Norito responses. */
    NORITO_ONLY("application/x-norito"),

    /** Request only JSON responses. */
    JSON_ONLY("application/json");

    /** HTTP Accept header value for this preference. */
    fun acceptHeader(): String = header
}
