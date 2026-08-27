// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.security.MessageDigest

private val TYPE_NAME_SCHEMA_HASH_DOMAIN = "norito:v1:type-name\u0000".toByteArray(Charsets.UTF_8)

/** Computes domain-separated SHA-256 schema hashes matching the Rust implementation. */
object SchemaHash {

    @JvmStatic
    fun hash16(canonicalPath: String): ByteArray {
        return hash16(TYPE_NAME_SCHEMA_HASH_DOMAIN, canonicalPath.toByteArray(Charsets.UTF_8))
    }

    private fun hash16(domain: ByteArray, input: ByteArray): ByteArray {
        val digest = MessageDigest.getInstance("SHA-256")
        digest.update(domain)
        digest.update(input)
        return digest.digest().copyOfRange(0, 16)
    }

}
