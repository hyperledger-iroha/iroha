package org.hyperledger.iroha.sdk.core.model.instructions

private const val ARG_ACTION = "action"
private const val ARG_ESCROW_ID = "escrow_id"
private const val ARG_ASSET_DEFINITION = "asset_definition"
private const val ARG_AMOUNT = "amount"
private const val ARG_BUYER_AMOUNT = "buyer_amount"
private const val ARG_SELLER_AMOUNT = "seller_amount"
private const val ARG_EVIDENCE_HASHES = "evidence_hashes"

private fun requireEscrowArgument(arguments: Map<String, String>, key: String): String {
    val value = arguments[key]
    require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
    return value
}

private fun validatedEscrowValue(value: String, name: String): String {
    val trimmed = value.trim()
    require(trimmed.isNotEmpty()) { "$name must not be blank" }
    return trimmed
}

private fun normalizedEscrowEvidenceHashes(values: List<String>): List<String> =
    values.mapIndexed { index, value ->
        validatedEscrowValue(value, "evidenceHashes[$index]")
    }

private fun appendEscrowEvidence(args: MutableMap<String, String>, evidenceHashes: List<String>) {
    if (evidenceHashes.isNotEmpty()) {
        args[ARG_EVIDENCE_HASHES] = evidenceHashes.joinToString(",")
    }
}

private fun parseEscrowEvidenceHashes(raw: String?): List<String> =
    raw?.split(',')
        ?.map { it.trim() }
        ?.filter { it.isNotEmpty() }
        ?: emptyList()

private fun escrowIdArguments(action: String, escrowId: String): Map<String, String> =
    linkedMapOf(ARG_ACTION to action, ARG_ESCROW_ID to escrowId)

private fun openEscrowArguments(
    escrowId: String,
    assetDefinition: String,
    amount: String,
    evidenceHashes: List<String>,
): Map<String, String> {
    val args = linkedMapOf(
        ARG_ACTION to OpenAssetEscrowInstruction.ACTION,
        ARG_ESCROW_ID to validatedEscrowValue(escrowId, "escrowId"),
        ARG_ASSET_DEFINITION to validatedEscrowValue(assetDefinition, "assetDefinition"),
        ARG_AMOUNT to validatedEscrowValue(amount, "amount"),
    )
    appendEscrowEvidence(args, normalizedEscrowEvidenceHashes(evidenceHashes))
    return args
}

private fun openDisputeArguments(
    escrowId: String,
    evidenceHashes: List<String>,
): Map<String, String> {
    val args = linkedMapOf(
        ARG_ACTION to OpenEscrowDisputeInstruction.ACTION,
        ARG_ESCROW_ID to validatedEscrowValue(escrowId, "escrowId"),
    )
    appendEscrowEvidence(args, normalizedEscrowEvidenceHashes(evidenceHashes))
    return args
}

private fun resolveDisputeArguments(
    escrowId: String,
    buyerAmount: String,
    sellerAmount: String,
    evidenceHashes: List<String>,
): Map<String, String> {
    val args = linkedMapOf(
        ARG_ACTION to ResolveEscrowDisputeInstruction.ACTION,
        ARG_ESCROW_ID to validatedEscrowValue(escrowId, "escrowId"),
        ARG_BUYER_AMOUNT to validatedEscrowValue(buyerAmount, "buyerAmount"),
        ARG_SELLER_AMOUNT to validatedEscrowValue(sellerAmount, "sellerAmount"),
    )
    appendEscrowEvidence(args, normalizedEscrowEvidenceHashes(evidenceHashes))
    return args
}

/** Typed representation of the `OpenAssetEscrow` instruction. */
class OpenAssetEscrowInstruction private constructor(
    @JvmField val escrowId: String,
    @JvmField val assetDefinition: String,
    @JvmField val amount: String,
    @JvmField val evidenceHashes: List<String>,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    constructor(
        escrowId: String,
        assetDefinition: String,
        amount: String,
        evidenceHashes: List<String> = emptyList(),
    ) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        assetDefinition = validatedEscrowValue(assetDefinition, "assetDefinition"),
        amount = validatedEscrowValue(amount, "amount"),
        evidenceHashes = normalizedEscrowEvidenceHashes(evidenceHashes),
        arguments = openEscrowArguments(escrowId, assetDefinition, amount, evidenceHashes),
    )

    constructor(
        escrowId: String,
        assetDefinition: String,
        amount: Number,
        evidenceHashes: List<String> = emptyList(),
    ) : this(escrowId, assetDefinition, amount.toString(), evidenceHashes)

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is OpenAssetEscrowInstruction) return false
        return escrowId == other.escrowId
            && assetDefinition == other.assetDefinition
            && amount == other.amount
            && evidenceHashes == other.evidenceHashes
    }

    override fun hashCode(): Int = listOf(escrowId, assetDefinition, amount, evidenceHashes).hashCode()

    companion object {
        const val ACTION: String = "OpenAssetEscrow"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): OpenAssetEscrowInstruction =
            OpenAssetEscrowInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                assetDefinition = requireEscrowArgument(arguments, ARG_ASSET_DEFINITION),
                amount = requireEscrowArgument(arguments, ARG_AMOUNT),
                evidenceHashes = parseEscrowEvidenceHashes(arguments[ARG_EVIDENCE_HASHES]),
                arguments = LinkedHashMap(arguments),
            )
    }
}

/** Typed representation of the `AcceptAssetEscrow` instruction. */
class AcceptAssetEscrowInstruction private constructor(
    @JvmField val escrowId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    constructor(escrowId: String) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        arguments = escrowIdArguments(ACTION, validatedEscrowValue(escrowId, "escrowId")),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean = other is AcceptAssetEscrowInstruction && escrowId == other.escrowId

    override fun hashCode(): Int = escrowId.hashCode()

    companion object {
        const val ACTION: String = "AcceptAssetEscrow"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): AcceptAssetEscrowInstruction =
            AcceptAssetEscrowInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                arguments = LinkedHashMap(arguments),
            )
    }
}

/** Typed representation of the `MarkEscrowPaymentSent` instruction. */
class MarkEscrowPaymentSentInstruction private constructor(
    @JvmField val escrowId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    constructor(escrowId: String) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        arguments = escrowIdArguments(ACTION, validatedEscrowValue(escrowId, "escrowId")),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean = other is MarkEscrowPaymentSentInstruction && escrowId == other.escrowId

    override fun hashCode(): Int = escrowId.hashCode()

    companion object {
        const val ACTION: String = "MarkEscrowPaymentSent"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): MarkEscrowPaymentSentInstruction =
            MarkEscrowPaymentSentInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                arguments = LinkedHashMap(arguments),
            )
    }
}

/** Typed representation of the `ReleaseAssetEscrow` instruction. */
class ReleaseAssetEscrowInstruction private constructor(
    @JvmField val escrowId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    constructor(escrowId: String) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        arguments = escrowIdArguments(ACTION, validatedEscrowValue(escrowId, "escrowId")),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean = other is ReleaseAssetEscrowInstruction && escrowId == other.escrowId

    override fun hashCode(): Int = escrowId.hashCode()

    companion object {
        const val ACTION: String = "ReleaseAssetEscrow"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ReleaseAssetEscrowInstruction =
            ReleaseAssetEscrowInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                arguments = LinkedHashMap(arguments),
            )
    }
}

/** Typed representation of the `CancelAssetEscrow` instruction. */
class CancelAssetEscrowInstruction private constructor(
    @JvmField val escrowId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    constructor(escrowId: String) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        arguments = escrowIdArguments(ACTION, validatedEscrowValue(escrowId, "escrowId")),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean = other is CancelAssetEscrowInstruction && escrowId == other.escrowId

    override fun hashCode(): Int = escrowId.hashCode()

    companion object {
        const val ACTION: String = "CancelAssetEscrow"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): CancelAssetEscrowInstruction =
            CancelAssetEscrowInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                arguments = LinkedHashMap(arguments),
            )
    }
}

/** Typed representation of the `OpenEscrowDispute` instruction. */
class OpenEscrowDisputeInstruction private constructor(
    @JvmField val escrowId: String,
    @JvmField val evidenceHashes: List<String>,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    constructor(
        escrowId: String,
        evidenceHashes: List<String> = emptyList(),
    ) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        evidenceHashes = normalizedEscrowEvidenceHashes(evidenceHashes),
        arguments = openDisputeArguments(escrowId, evidenceHashes),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean =
        other is OpenEscrowDisputeInstruction && escrowId == other.escrowId && evidenceHashes == other.evidenceHashes

    override fun hashCode(): Int = listOf(escrowId, evidenceHashes).hashCode()

    companion object {
        const val ACTION: String = "OpenEscrowDispute"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): OpenEscrowDisputeInstruction =
            OpenEscrowDisputeInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                evidenceHashes = parseEscrowEvidenceHashes(arguments[ARG_EVIDENCE_HASHES]),
                arguments = LinkedHashMap(arguments),
            )
    }
}

/** Typed representation of the `ResolveEscrowDispute` instruction. */
class ResolveEscrowDisputeInstruction private constructor(
    @JvmField val escrowId: String,
    @JvmField val buyerAmount: String,
    @JvmField val sellerAmount: String,
    @JvmField val evidenceHashes: List<String>,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    constructor(
        escrowId: String,
        buyerAmount: String,
        sellerAmount: String,
        evidenceHashes: List<String> = emptyList(),
    ) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        buyerAmount = validatedEscrowValue(buyerAmount, "buyerAmount"),
        sellerAmount = validatedEscrowValue(sellerAmount, "sellerAmount"),
        evidenceHashes = normalizedEscrowEvidenceHashes(evidenceHashes),
        arguments = resolveDisputeArguments(escrowId, buyerAmount, sellerAmount, evidenceHashes),
    )

    constructor(
        escrowId: String,
        buyerAmount: Number,
        sellerAmount: Number,
        evidenceHashes: List<String> = emptyList(),
    ) : this(escrowId, buyerAmount.toString(), sellerAmount.toString(), evidenceHashes)

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ResolveEscrowDisputeInstruction) return false
        return escrowId == other.escrowId
            && buyerAmount == other.buyerAmount
            && sellerAmount == other.sellerAmount
            && evidenceHashes == other.evidenceHashes
    }

    override fun hashCode(): Int = listOf(escrowId, buyerAmount, sellerAmount, evidenceHashes).hashCode()

    companion object {
        const val ACTION: String = "ResolveEscrowDispute"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ResolveEscrowDisputeInstruction =
            ResolveEscrowDisputeInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                buyerAmount = requireEscrowArgument(arguments, ARG_BUYER_AMOUNT),
                sellerAmount = requireEscrowArgument(arguments, ARG_SELLER_AMOUNT),
                evidenceHashes = parseEscrowEvidenceHashes(arguments[ARG_EVIDENCE_HASHES]),
                arguments = LinkedHashMap(arguments),
            )
    }
}
