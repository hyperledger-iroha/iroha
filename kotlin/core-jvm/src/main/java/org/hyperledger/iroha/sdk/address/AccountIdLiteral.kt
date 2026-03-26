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
        "$field must use canonical I105 encoded account without @domain"
    }
    val parsed = try {
        AccountAddress.parseEncoded(value, null)
    } catch (ex: AccountAddressException) {
        throw IllegalArgumentException(
            "$field must use a canonical I105 encoded account literal",
            ex,
        )
    }
    require(parsed.format == AccountAddressFormat.I105) {
        "$field must use a canonical I105 encoded account literal"
    }
    val discriminant = AccountAddress.detectI105Discriminant(value)
        ?: throw IllegalArgumentException("$field must use a canonical I105 encoded account literal")
    val canonical = try {
        parsed.address.toI105(discriminant)
    } catch (ex: AccountAddressException) {
        throw IllegalArgumentException(
            "$field must use a canonical I105 encoded account literal",
            ex,
        )
    }
    require(value == canonical) {
        "$field must use a canonical I105 encoded account literal"
    }
    return canonical
}
