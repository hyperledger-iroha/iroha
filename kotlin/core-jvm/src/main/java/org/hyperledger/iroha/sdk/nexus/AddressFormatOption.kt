package org.hyperledger.iroha.sdk.nexus

/** Address format preference accepted by Torii account/UAID endpoints. */
enum class AddressFormatOption(val parameterValue: String) {
    IH58("ih58"),
    CANONICAL("canonical"),
    COMPRESSED("compressed"),
}
