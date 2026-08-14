package org.hyperledger.iroha.sdk.client

import java.security.KeyPairGenerator
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys

private val applicationKeyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()

internal fun applicationAuth(
    accountId: String = AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x33), "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT),
): ToriiCanonicalRequestAuth =
    ToriiCanonicalRequestAuth(
        accountId,
        applicationKeyPair.private,
        1_700_000_000_123L,
        "application-post-auth",
    )

internal fun noncanonicalStandardBase64PadBitAlias(encoded: String): String {
    require(encoded.endsWith("==")) { "64-byte signatures encode with == padding" }
    val alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    val chars = encoded.toCharArray()
    val index = chars.size - 3
    val value = alphabet.indexOf(chars[index])
    require(value >= 0) { "standard base64 alphabet" }
    chars[index] = alphabet[value xor 0x01]
    return String(chars)
}

internal fun ramLfeExecuteResponseJson(): String =
    """
        {
          "program_id": "identifier_lookup_retail",
          "opaque_hash": "${"11".repeat(32)}",
          "receipt_hash": "${"22".repeat(32)}",
          "output_ciphertext": "abcd",
          "output_hash": "${"44".repeat(32)}",
          "associated_data_hash": "${"55".repeat(32)}",
          "executed_at_ms": 42,
          "expires_at_ms": 142,
          "backend": "bfv-programmed-sha3-256-v1",
          "verification_mode": "signed",
          "receipt": {
            "payload": {
              "program_id": {"name": "identifier_lookup_retail"},
              "program_digest": "hash:${"11".repeat(32).uppercase()}#ABCD",
              "backend": "bfv-programmed-sha3-256-v1",
              "verification_mode": {"mode": "Signed", "value": null},
              "output_hash": "hash:${"22".repeat(32).uppercase()}#BCDE",
              "associated_data_hash": "hash:${"33".repeat(32).uppercase()}#CDEF",
              "executed_at_ms": 42,
              "expires_at_ms": 142
            },
            "signature": "${"aa".repeat(64)}"
          }
        }
    """.trimIndent()

internal fun ramLfeReceiptVerifyResponseJson(): String =
    """
        {
          "valid": true,
          "program_id": "identifier_lookup_retail",
          "backend": "bfv-programmed-sha3-256-v1",
          "verification_mode": "signed",
          "output_hash": "${"44".repeat(32)}",
          "associated_data_hash": "${"55".repeat(32)}",
          "output_hash_matches": true
        }
    """.trimIndent()
