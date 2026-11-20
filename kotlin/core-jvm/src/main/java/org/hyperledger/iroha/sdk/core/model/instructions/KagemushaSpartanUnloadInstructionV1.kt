// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.InstructionBox

/** Exact charged Control claim and selected World identity for the ordinary account transaction signer. */
class KagemushaSpartanUnloadInstructionV1(
    scheme: ByteArray,
    asset: ByteArray,
    manifestDigest: ByteArray,
    claim: ByteArray,
) : InstructionTemplate {
    private val frame: ByteArray
    init {
        val scopes = listOf(scheme, asset, manifestDigest)
        require(scopes.all { it.size == 32 && it.any { byte -> byte.toInt() != 0 } }) {
            "scheme, asset and manifestDigest must be nonzero 32-byte identities"
        }
        require(claim.size in 1..16_384) { "Unload claim must contain 1..16384 bytes" }
        frame = spartanControlFrame("iroha.kagemusha.spartan.Unload.v1", scopes, claim)
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM
    override val arguments: Map<String, String> get() = toInstructionBox().arguments

    /** Canonical standalone Norito original, suitable for exact-byte audit or recovery. */
    fun concreteFrame(): ByteArray = frame.copyOf()

    override fun toInstructionBox(): InstructionBox = InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

    companion object {
        const val WIRE_ID: String = "iroha.kagemusha.spartan.unload.v1"
    }
}
