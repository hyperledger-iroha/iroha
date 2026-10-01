// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.address.AccountAddress
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Public account rendering preserves one canonical identity across chain discriminants. */
class KagemushaAccountIdV1Test {
    @Test
    fun renderingAtChainBoundariesPreservesCanonicalIdentity() {
        val address = address()
        val account = KagemushaAccountIdV1.parse(address.toI105(0))
        val canonical = account.canonicalPayload()

        for (discriminant in listOf(0, 1, 0xffff)) {
            val rendered = account.toI105(discriminant)
            assertEquals(address.toI105(discriminant), rendered)
            assertContentEquals(address.canonicalBytes, AccountAddress.parseEncoded(rendered, discriminant).canonicalBytes)
            assertContentEquals(canonical, KagemushaAccountIdV1.parse(rendered).canonicalPayload())
        }
        assertContentEquals(canonical, account.canonicalPayload())
    }

    @Test
    fun invalidChainDiscriminantsCannotChangeTheAccount() {
        val account = KagemushaAccountIdV1.parse(address().toI105(0))
        val canonical = account.canonicalPayload()
        for (discriminant in listOf(Int.MIN_VALUE, -1, 0x10000, Int.MAX_VALUE)) {
            assertFailsWith<IllegalArgumentException> { account.toI105(discriminant) }
        }
        assertContentEquals(canonical, account.canonicalPayload())
    }

    @Test
    fun renderingOwnsItsCanonicalPayload() {
        val original = KagemushaAccountIdV1.parse(address().toI105(0))
        val input = original.canonicalPayload()
        val restored = KagemushaAccountIdV1.fromCanonicalPayload(input)
        input.fill(0)
        restored.canonicalPayload().fill(0)
        assertEquals(original.toI105(0xffff), restored.toI105(0xffff))
        assertEquals(original, restored)
        assertContentEquals(original.canonicalPayload(), restored.canonicalPayload())
    }

    private fun address(): AccountAddress {
        val encoded = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
        val publicKey = encoded.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
        return AccountAddress.fromAccount(publicKey, "ed25519")
    }
}
