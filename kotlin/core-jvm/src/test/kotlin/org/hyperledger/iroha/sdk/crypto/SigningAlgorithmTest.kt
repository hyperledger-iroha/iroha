package org.hyperledger.iroha.sdk.crypto

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class SigningAlgorithmTest {
    @Test
    fun bridgeCodesAndCanonicalWireNamesMatchRustAlgorithms() {
        val expected = mapOf(
            0 to "ed25519",
            1 to "secp256k1",
            2 to "bls_normal",
            3 to "bls_small",
            4 to "ml-dsa",
            5 to "gost3410-2012-256-paramset-a",
            6 to "gost3410-2012-256-paramset-b",
            7 to "gost3410-2012-256-paramset-c",
            8 to "gost3410-2012-512-paramset-a",
            9 to "gost3410-2012-512-paramset-b",
            10 to "sm2",
        )

        assertEquals(expected, SigningAlgorithm.entries.associate { it.bridgeCode to it.wireName })
        expected.forEach { (code, wireName) ->
            assertEquals(wireName, SigningAlgorithm.fromBridgeCode(code).wireName)
        }
    }

    @Test
    fun aliasesNormalizeToCanonicalAlgorithms() {
        assertEquals(SigningAlgorithm.SECP256K1, SigningAlgorithm.fromAlgorithmName("secp-256k1"))
        assertEquals(SigningAlgorithm.BLS_NORMAL, SigningAlgorithm.fromAlgorithmName("bls-normal"))
        assertEquals(SigningAlgorithm.BLS_SMALL, SigningAlgorithm.fromAlgorithmName("bls12-381-g2"))
        assertEquals(SigningAlgorithm.ML_DSA, SigningAlgorithm.fromAlgorithmName("ML_DSA-65"))
        assertEquals(
            SigningAlgorithm.GOST_2012_512_B,
            SigningAlgorithm.fromAlgorithmName("GOST3410-2012-512-PARAMSET-B"),
        )
        assertEquals(SigningAlgorithm.SM2, SigningAlgorithm.fromAlgorithmName("sm-2"))
    }

    @Test
    fun unsupportedAndUnicodeConfusableAliasesFailClosed() {
        for (algorithm in listOf(null, "", "   ")) {
            val error = assertFailsWith<IllegalArgumentException> {
                SigningAlgorithm.fromAlgorithmName(algorithm)
            }
            assertEquals("signing algorithm must be a non-empty string", error.message)
        }

        for (algorithm in listOf(" ed25519", "ed25519 ", "\ted25519")) {
            val error = assertFailsWith<IllegalArgumentException> {
                SigningAlgorithm.fromAlgorithmName(algorithm)
            }
            assertEquals("signing algorithm must not contain surrounding whitespace", error.message)
        }

        for (algorithm in listOf(
            "unknown",
            "ed\t25519",
            "ed\u200B25519",
            "\u0435d25519",
            "ml\uFF0Ddsa",
            "gost3410-2012-512-paramset-\u0432",
        )) {
            assertFailsWith<IllegalArgumentException> {
                SigningAlgorithm.fromAlgorithmName(algorithm)
            }
        }
    }
}
