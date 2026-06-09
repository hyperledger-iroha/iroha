package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

object KagemushaWireNames {
    const val TRANSFER_INSTRUCTION: String = "iroha_data_model::isi::offline::KagemushaTransfer"
    const val REDEEM_RECURSIVE_INSTRUCTION: String = "iroha_data_model::isi::offline::RedeemKagemushaRecursive"
    const val RECURSIVE_REDEEM_REQUEST: String =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1"
}

enum class KagemushaInstructionType(
    val archiveTypeName: String,
    val wireName: String,
) {
    TRANSFER(
        "KagemushaTransfer",
        KagemushaWireNames.TRANSFER_INSTRUCTION,
    ),
    REDEEM_RECURSIVE(
        "RedeemKagemushaRecursive",
        KagemushaWireNames.REDEEM_RECURSIVE_INSTRUCTION,
    ),
}

object KagemushaInstructionArchives {
    @JvmStatic
    fun instructionBox(
        instructionType: KagemushaInstructionType,
        instructionArchive: ByteArray,
    ): InstructionBox {
        val archive = copyAndValidateInstructionArchive(instructionType, instructionArchive)
        return InstructionBox.fromWirePayload(instructionType.wireName, archive)
    }

    @JvmStatic
    fun recursiveRedeemInstructionBox(instructionArchive: ByteArray): InstructionBox =
        instructionBox(KagemushaInstructionType.REDEEM_RECURSIVE, instructionArchive)

    @JvmStatic
    fun recursiveRedeemInstructionBoxFromRequest(redeemRequestArchive: ByteArray): InstructionBox =
        recursiveRedeemInstructionBox(KagemushaRecursiveSpendProver.redeemSpend(redeemRequestArchive))

    @JvmStatic
    fun transactionPayload(
        instructionType: KagemushaInstructionType,
        instructionArchive: ByteArray,
        chainId: String,
        authority: String,
        creationTimeMs: Long,
        timeToLiveMs: Long? = null,
        nonce: Int? = null,
        metadata: Map<String, String> = emptyMap(),
    ): TransactionPayload = TransactionPayload(
        chainId = chainId,
        authority = authority,
        creationTimeMs = creationTimeMs,
        executable = Executable.instructions(listOf(instructionBox(instructionType, instructionArchive))),
        timeToLiveMs = timeToLiveMs,
        nonce = nonce,
        metadata = metadata,
    )

    @JvmStatic
    fun recursiveRedeemTransactionPayload(
        instructionArchive: ByteArray,
        chainId: String,
        authority: String,
        creationTimeMs: Long,
        timeToLiveMs: Long? = null,
        nonce: Int? = null,
        metadata: Map<String, String> = emptyMap(),
    ): TransactionPayload = transactionPayload(
        instructionType = KagemushaInstructionType.REDEEM_RECURSIVE,
        instructionArchive = instructionArchive,
        chainId = chainId,
        authority = authority,
        creationTimeMs = creationTimeMs,
        timeToLiveMs = timeToLiveMs,
        nonce = nonce,
        metadata = metadata,
    )

    @JvmStatic
    fun recursiveRedeemTransactionPayloadFromRequest(
        redeemRequestArchive: ByteArray,
        chainId: String,
        authority: String,
        creationTimeMs: Long,
        timeToLiveMs: Long? = null,
        nonce: Int? = null,
        metadata: Map<String, String> = emptyMap(),
    ): TransactionPayload = recursiveRedeemTransactionPayload(
        instructionArchive = KagemushaRecursiveSpendProver.redeemSpend(redeemRequestArchive),
        chainId = chainId,
        authority = authority,
        creationTimeMs = creationTimeMs,
        timeToLiveMs = timeToLiveMs,
        nonce = nonce,
        metadata = metadata,
    )

    private fun copyAndValidateInstructionArchive(
        instructionType: KagemushaInstructionType,
        instructionArchive: ByteArray,
    ): ByteArray {
        require(instructionArchive.isNotEmpty()) {
            "Kagemusha instruction archive must not be empty."
        }
        require(instructionArchive.size <= KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES) {
            "Kagemusha instruction archive must not exceed ${KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES} bytes."
        }
        val archive = instructionArchive.copyOf()
        val decoded = try {
            NoritoHeader.decode(archive, SchemaHash.hash16(instructionType.wireName))
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException(
                "Kagemusha instruction archive must be a valid ${instructionType.archiveTypeName} Norito archive.",
                ex,
            )
        }
        require(decoded.header.compression == NoritoHeader.COMPRESSION_NONE) {
            "Kagemusha instruction archive must not be compressed."
        }
        require(decoded.header.payloadLength > 0) {
            "Kagemusha instruction archive must contain a non-empty Norito payload."
        }
        try {
            decoded.header.validateChecksum(decoded.payload)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("Kagemusha instruction archive checksum is invalid.", ex)
        }
        return archive
    }
}
