package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

/** Request payload for Torii `/v1/multisig/propose`. */
data class MultisigProposeRequest @JvmOverloads constructor(
    val multisigAccountId: String? = null,
    val multisigAccountAlias: String? = null,
    val signerAccountId: String,
    val instructions: List<ByteArray>,
    val publicKeyHex: String? = null,
    val signatureB64: String? = null,
    val creationTimeMs: Long? = null,
    val feeSponsor: String? = null,
    val memo: String? = null,
) {
    companion object {
        /** Builds a request from typed instruction boxes by encoding each box as native Norito. */
        @JvmStatic
        @JvmOverloads
        fun fromInstructionBoxes(
            multisigAccountId: String? = null,
            multisigAccountAlias: String? = null,
            signerAccountId: String,
            instructions: List<InstructionBox>,
            publicKeyHex: String? = null,
            signatureB64: String? = null,
            creationTimeMs: Long? = null,
            feeSponsor: String? = null,
            memo: String? = null,
        ): MultisigProposeRequest = MultisigProposeRequest(
            multisigAccountId = multisigAccountId,
            multisigAccountAlias = multisigAccountAlias,
            signerAccountId = signerAccountId,
            instructions = instructions.map { NoritoJavaCodecAdapter.encodeInstructionBox(it) },
            publicKeyHex = publicKeyHex,
            signatureB64 = signatureB64,
            creationTimeMs = creationTimeMs,
            feeSponsor = feeSponsor,
            memo = memo,
        )
    }
}
