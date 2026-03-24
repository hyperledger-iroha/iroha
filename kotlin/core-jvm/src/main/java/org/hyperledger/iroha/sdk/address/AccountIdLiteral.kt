package org.hyperledger.iroha.sdk.address

/**
 * Extracts the IH58 address portion from a `"<address>@<domain>"` account identifier.
 *
 * @param accountId account identifier string
 * @return IH58 address portion without the domain suffix
 */
fun extractIh58Address(accountId: String): String {
    val value = accountId.trim()
    require(value.isNotEmpty()) { "accountId must not be blank" }
    val atIndex = value.lastIndexOf('@')
    require(atIndex > 0 && atIndex != value.length - 1) { "Invalid account ID format: $value" }
    return value.substring(0, atIndex)
}

/**
 * Backward-compatible alias for callers that still use the older I105 naming.
 */
fun extractI105Address(accountId: String): String = extractIh58Address(accountId)
