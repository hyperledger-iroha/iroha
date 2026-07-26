package org.hyperledger.iroha.sdk.core.model

import org.hyperledger.iroha.sdk.core.model.instructions.InstructionKind

interface InstructionPayload {
    val kind: InstructionKind
    val arguments: Map<String, String>
}