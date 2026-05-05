package org.hyperledger.iroha.sdk.address

class AccountAddressException(
    @JvmField val code: AccountAddressErrorCode,
    message: String,
) : Exception(message) {

    val codeValue: String get() = code.code
}
