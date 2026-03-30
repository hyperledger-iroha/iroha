package org.hyperledger.iroha.sdk.address

import org.hyperledger.iroha.sdk.crypto.SoftwareKeyProvider
import kotlin.test.Test
import kotlin.test.assertEquals

class I105CompatTest {

    @Test
    fun `fromAccount produces 36 canonical bytes for ed25519`() {
        // Use SoftwareKeyProvider like the E2E test does
        val keyProvider = SoftwareKeyProvider(SoftwareKeyProvider.ProviderPolicy.BOUNCY_CASTLE_REQUIRED)
        val keyPair = keyProvider.generateEphemeral()
        val spki = keyPair.public.encoded
        println("SPKI size: ${spki.size}")
        println("SPKI hex: ${spki.joinToString("") { "%02x".format(it) }}")

        // Extract raw 32-byte key (skip 12-byte SPKI prefix)
        val prefix = byteArrayOf(
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65,
            0x70, 0x03, 0x21, 0x00
        )
        val rawKey = spki.copyOfRange(prefix.size, spki.size)
        println("Raw key size: ${rawKey.size}")
        println("Raw key hex: ${rawKey.joinToString("") { "%02x".format(it) }}")

        val address = AccountAddress.fromAccount(rawKey, "ed25519")
        val canonical = address.canonicalHex()
        println("Canonical: $canonical")
        println("Canonical bytes: ${canonical.length / 2 - 1}") // minus 0x

        val i105 = address.toI105Default()
        println("I105: $i105")
        println("I105 length: ${i105.length}")

        // Canonical must be 36 bytes: header(1) + ctrl_tag(1) + curve(1) + keylen(1) + key(32)
        assertEquals(36, (canonical.length - 2) / 2, "Canonical should be 36 bytes")
    }

    @Test
    fun `fromAccount produces same I105 for known key`() {
        // Alice's key - known to produce correct I105
        val aliceHex = "CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
        val aliceKey = aliceHex.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
        val aliceAddr = AccountAddress.fromAccount(aliceKey, "ed25519")
        val aliceI105 = aliceAddr.toI105Default()
        println("Alice I105: $aliceI105")
        println("Alice canonical: ${aliceAddr.canonicalHex()}")
        assertEquals("sorauロ1PノウヌmEエWオebHム6ヤルイヰiwuCWErJ7uスoPGアヤnjムKヒTCW2PV", aliceI105)
    }
}
