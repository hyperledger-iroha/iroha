package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral

internal fun requirePublicKeyLiteral(value: String, fieldName: String): String {
    require(value.isNotBlank()) { "$fieldName must not be empty" }
    require(value.trim() == value) { "$fieldName must not contain surrounding whitespace" }
    require(decodePublicKeyLiteral(value) != null) { "$fieldName is not a valid public key literal" }
    return value
}
