package org.hyperledger.iroha.sdk.explorer

/** Immutable view over `/v1/explorer/accounts/{account_id}/qr` responses. */
class ExplorerAccountQrSnapshot(
    canonicalId: String,
    literal: String,
    addressFormat: String,
    networkPrefix: Int,
    modules: Int,
    qrVersion: Int,
    errorCorrection: String,
    svg: String,
) {
    @JvmField val canonicalId: String = requireNonEmpty(canonicalId, "canonicalId")
    @JvmField val literal: String = requireNonEmpty(literal, "literal")
    @JvmField val addressFormat: String = requireNonEmpty(addressFormat, "addressFormat")
    @JvmField val networkPrefix: Int = requirePositive(networkPrefix, "networkPrefix")
    @JvmField val modules: Int = requirePositive(modules, "modules")
    @JvmField val qrVersion: Int = requirePositive(qrVersion, "qrVersion")
    @JvmField val errorCorrection: String = requireNonEmpty(errorCorrection, "errorCorrection")
    @JvmField val svg: String = requireNonEmpty(svg, "svg")
}

private fun requireNonEmpty(value: String, label: String): String {
    val trimmed = value.trim()
    require(trimmed.isNotEmpty()) { "$label must not be blank" }
    return trimmed
}

private fun requirePositive(value: Int, label: String): Int {
    require(value > 0) { "$label must be positive" }
    return value
}
