package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.WirePayload
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertIs

/**
 * Parity test for [RegisterAccountWirePayloadEncoder].
 *
 * Runs the Rust `kotlin-fixture-gen register-account` binary that encodes the
 * same `RegisterBox::Account` instruction using the current Rust data model.
 * If either side changes the struct layout, this test fails.
 */
class RegisterAccountWirePayloadEncoderParityTest {

    private val parityPublicKeyHex =
        "CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"

    @Test
    fun `register account encoding matches Rust fixture generator`() {
        val lines = FixtureGeneratorRunner.run("register-account")
        val rustHex = lines[0]

        val rawPublicKey = FixtureGeneratorRunner.hexToBytes(parityPublicKeyHex)
        val address = AccountAddress.fromAccount(rawPublicKey, "ed25519")
        val accountId = address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

        val instruction = RegisterAccountWirePayloadEncoder.encodeRegisterAccount(accountId)

        assertEquals("iroha.register", instruction.name)
        val wirePayload = assertIs<WirePayload>(instruction.payload)
        val kotlinHex = FixtureGeneratorRunner.bytesToHex(wirePayload.payloadBytes)

        assertContentEquals(
            FixtureGeneratorRunner.hexToBytes(rustHex),
            wirePayload.payloadBytes,
            "Kotlin RegisterBox encoding must match Rust. " +
                "If Rust data model changed, update RegisterAccountWirePayloadEncoder.\n" +
                "  Rust:   $rustHex\n" +
                "  Kotlin: $kotlinHex",
        )
    }
}
