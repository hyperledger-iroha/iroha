package org.hyperledger.iroha.sdk.address

/**
 * Requires a canonical encoded account identifier without a trailing `@domain` suffix.
 *
 * @param accountId account identifier string
 * @param field field name used in validation messages
 * @return canonical encoded account identifier
 */
fun requireCanonicalI105Address(accountId: String, field: String): String {
    require(field.isNotBlank()) { "field must not be blank" }
    val value = accountId.trim()
    require(value.isNotEmpty()) { "$field must not be blank" }
    require(value.indexOf('@') < 0) {
        "$field must use canonical i105 encoded account without @domain"
    }
    val parsed = try {
        AccountAddress.parseEncoded(value, null)
    } catch (ex: AccountAddressException) {
        throw IllegalArgumentException(
            "$field must use a canonical i105 encoded account literal",
            ex,
        )
    }
    require(parsed.format == AccountAddressFormat.I105) {
        "$field must use a canonical i105 encoded account literal"
    }
    return value
}
