package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.InstructionPayload

/**
 * Produces an `InstructionBox`. Implementations capture the typed fields for each instruction
 * variant before converting to the argument-map representation used by local helpers. Transaction
 * encoding still requires wire-framed instruction payloads.
 */
interface InstructionTemplate : InstructionPayload {

    fun toInstructionBox(): InstructionBox = InstructionBox.of(this)
}
