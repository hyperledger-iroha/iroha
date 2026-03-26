package org.hyperledger.iroha.sdk.address

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class AssetIdCodecTest {
    @Test
    fun assetDefinitionIdRoundTripsCanonicalBase58() {
        val bytes = AssetDefinitionIdEncoder.parseAddressBytes(TERTIARY_ASSET_ID)
        val reencoded = AssetDefinitionIdEncoder.encodeFromBytes(bytes)
        assertEquals(TERTIARY_ASSET_ID, reencoded)
        assertTrue(AssetDefinitionIdEncoder.isCanonicalAddress(TERTIARY_ASSET_ID))
    }

    @Test
    fun assetIdEncoderPreservesCanonicalDefinitionAddress() {
        assertEquals(
            PRIMARY_ASSET_ID,
            AssetIdEncoder.encodeAssetIdFromDefinition(PRIMARY_ASSET_ID),
        )
    }

    @Test
    fun assetIdDecoderAcceptsCanonicalPublicAssetIdOnly() {
        val decoded = AssetIdDecoder.decode(PRIMARY_ASSET_ID)
        assertEquals(PRIMARY_ASSET_ID, decoded.definition.address)
        assertEquals("", decoded.accountId)
        assertNull(decoded.dataspaceId)
        assertTrue(AssetIdDecoder.isCanonical(PRIMARY_ASSET_ID))
    }

    @Test
    fun assetIdDecoderRejectsOwnerQualifiedLiteral() {
        assertFalse(AssetIdDecoder.isCanonical("$PRIMARY_ASSET_ID#not-public"))
        val error = assertFailsWith<IllegalArgumentException> {
            AssetIdDecoder.decode("$PRIMARY_ASSET_ID#not-public")
        }
        assertTrue(error.message!!.contains("Base58"))
    }

    @Test
    fun canonicalAddressValidationRejectsMalformedLiteral() {
        val error = assertFailsWith<IllegalArgumentException> {
            AssetDefinitionIdEncoder.parseAddressBytes("not-an-address")
        }
        assertTrue(
            error.message!!.contains("Base58") || error.message!!.contains("canonical"),
        )
    }

    companion object {
        private const val PRIMARY_ASSET_ID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
        private const val TERTIARY_ASSET_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
    }
}
