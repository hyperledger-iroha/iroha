// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1
import org.hyperledger.iroha.sdk.offline.KagemushaTopUpRequestV1

/**
 * Native first-release instruction for one payer-authorized KAGEMUSHA reserve top-up.
 *
 * Shape validation does not grant monetary authority; Core verifies the embedded authorization
 * before debiting the transaction authority.
 */
class TopUpKagemushaV1Instruction(
    @JvmField val request: KagemushaTopUpRequestV1,
) {
    init {
        KagemushaNoritoV1.encodeTopUpRequestShape(request)
    }

    /** Return the registered, schema-bound instruction ready for a transaction executable. */
    fun toInstructionBox(): InstructionBox =
        InstructionBox.fromWirePayload(
            WIRE_ID,
            KagemushaNoritoV1.encodeTopUpInstructionPayloadShape(request),
        )

    companion object {
        /** Sole first-release dynamic instruction registry identifier. */
        const val WIRE_ID: String = "iroha.kagemusha.v1.top_up"

        /** Exact concrete Rust type whose schema hash binds the instruction payload. */
        const val SCHEMA_NAME: String =
            "iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1"
    }
}
