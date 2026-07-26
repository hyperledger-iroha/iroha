package org.hyperledger.iroha.sdk.address

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull

class AccountAddressTest {
    @Test
    fun mixedI105LiteralRoundTripsToOriginalCanonicalPayload() {
        val literal =
            "sorauﾛ1PﾜdﾎｼﾋﾉNｸdﾁﾑkiﾇ3ｵﾓaPBQDTｲKqｼqｵrﾗｶwSQ1ﾌﾅQU61Y7"
        val address = AccountAddress.fromI105(literal, AccountAddress.DEFAULT_I105_DISCRIMINANT)
        assertEquals(
            "0x02000120bc717326224e4b4119298e7b1db8133cb27d6cdf6b3e04d75a6d27b29a34c1cf",
            address.canonicalHex(),
        )
        assertEquals(literal, address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT))
    }

    @Test
    fun rejectsNonCanonicalFullwidthKanaPayload() {
        val literal =
            "sorauﾛ1PﾜdﾎｼﾋﾉNｸdﾁﾑkiﾇ3ｵﾓaPBQDTｲKqｼqｵrﾗｶwSQ1ﾌﾅQU61Y7"
        val nonCanonical = literal.replaceFirst("ﾛ", "ロ")

        val error = assertFailsWith<AccountAddressException> {
            AccountAddress.fromI105(nonCanonical, AccountAddress.DEFAULT_I105_DISCRIMINANT)
        }
        assertEquals(AccountAddressErrorCode.INVALID_I105_CHAR, error.code)
    }

    @Test
    fun curveRegistryCoversAllCryptoAlgorithms() {
        assertEquals("secp256k1", algorithmForCurveId(0x04))
        assertEquals("bls_normal", algorithmForCurveId(0x03))
        assertEquals("bls_small", algorithmForCurveId(0x05))

        val secpKey = ByteArray(33) { 0x02 }
        val secpAddress = AccountAddress.fromAccount(secpKey, "secp256k1")
        assertEquals(0x04, secpAddress.singleKeyPayload()?.curveId)

        val blsKey = ByteArray(48) { 0x03 }
        assertFailsWith<AccountAddressException> {
            AccountAddress.fromAccount(blsKey, "bls_normal")
        }
        try {
            AccountAddress.configureCurveSupport(CurveSupportConfig.builder().allowBls(true).build())
            val blsAddress = AccountAddress.fromAccount(blsKey, "bls-normal")
            assertEquals(0x03, blsAddress.singleKeyPayload()?.curveId)
        } finally {
            AccountAddress.configureCurveSupport(CurveSupportConfig.ed25519Only())
        }

        val encoded = encodePublicKeyMultihash(0x04, secpKey)
        val decoded = assertNotNull(decodePublicKeyLiteral(encoded))
        assertEquals(0x04, decoded.curveId)
        assertEquals(secpKey.toList(), decoded.keyBytes.toList())

        val compact = compactPublicKeyPayload(0x04, secpKey)
        assertEquals(1, compact[0].toInt())
        val decodedCompact = assertNotNull(decodeCompactPublicKeyPayload(compact))
        assertEquals(0x04, decodedCompact.curveId)
        assertEquals(secpKey.toList(), decodedCompact.keyBytes.toList())
    }

    @Test
    fun fromAccountRejectsBlankOrPaddedCurveAlgorithmAliases() {
        val key = ByteArray(32) { 0x11 }
        for (algorithm in listOf(
            "",
            " ",
            " ed25519",
            "ed25519 ",
            "\ted25519",
            "ed25519\n",
            "\u00A0ed25519",
            "ed25519\u00A0",
        )) {
            val error = assertFailsWith<AccountAddressException> {
                AccountAddress.fromAccount(key, algorithm)
            }
            assertEquals(AccountAddressErrorCode.UNSUPPORTED_ALGORITHM, error.code)
        }
    }

    @Test
    fun fromAccountRejectsControlAndUnicodeConfusableCurveAlgorithmAliases() {
        val key = ByteArray(32) { 0x11 }
        for (algorithm in listOf(
            "future-curve",
            "ed\t25519",
            "ed\u200B25519",
            "\u0435d25519",
            "ml\uFF0Ddsa",
            "gost256\u0430",
        )) {
            val error = assertFailsWith<AccountAddressException> {
                AccountAddress.fromAccount(key, algorithm)
            }
            assertEquals(AccountAddressErrorCode.UNSUPPORTED_ALGORITHM, error.code)
        }
    }

    @Test
    fun longGostLabelsAreAcceptedWhenGostSupportIsEnabled() {
        val key = ByteArray(64) { 0x0A }
        try {
            AccountAddress.configureCurveSupport(CurveSupportConfig.builder().allowGost(true).build())
            val address = AccountAddress.fromAccount(key, "gost3410-2012-256-paramset-a")
            assertEquals(0x0A, address.singleKeyPayload()?.curveId)
        } finally {
            AccountAddress.configureCurveSupport(CurveSupportConfig.ed25519Only())
        }
    }
}
