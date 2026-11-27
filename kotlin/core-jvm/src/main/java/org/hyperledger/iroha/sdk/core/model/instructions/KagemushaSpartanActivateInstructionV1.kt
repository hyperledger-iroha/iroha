// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

/** Account-signed registration of the exact hardware-authorized Bootstrap claim under its selected World manifest. */
class KagemushaSpartanActivateInstructionV1(
    scheme: ByteArray, asset: ByteArray, manifestDigest: ByteArray, wallet: ByteArray, activation: ByteArray,
) : InstructionTemplate {
    private val frame: ByteArray
    init {
        val scope = listOf(scheme, asset, manifestDigest, wallet)
        require(scope.all { it.size == 32 && it.any { byte -> byte != 0.toByte() } })
        require(activation.size in 1..32_768)
        frame = spartanControlFrame("iroha.kagemusha.spartan.Activate.v1", scope, activation)
    }
    override val kind = InstructionKind.CUSTOM
    override val arguments: Map<String, String> get() = toInstructionBox().arguments
    fun concreteFrame(): ByteArray = frame.copyOf()
    override fun toInstructionBox() = InstructionBox.fromWirePayload(WIRE_ID, concreteFrame())

    companion object {
        const val WIRE_ID = "iroha.kagemusha.spartan.activate.v1"
    }
}

internal fun spartanControlFrame(schema: String, scopes: List<ByteArray>, claim: ByteArray): ByteArray {
    val root = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
    fun field(bytes: ByteArray) {
        root.writeLength(bytes.size.toLong(), true)
        root.writeBytes(bytes)
    }
    scopes.forEach(::field)
    val original = root.childEncoder()
    NoritoAdapters.rawByteVecAdapter().encode(original, claim)
    field(original.toByteArray())
    val payload = root.toByteArray()
    val header = NoritoHeader(SchemaHash.hash16(schema),
        payload.size, CRC64.compute(payload), NoritoCodec.DEFAULT_FLAGS, NoritoHeader.COMPRESSION_NONE)
    return header.encode() + payload
}
