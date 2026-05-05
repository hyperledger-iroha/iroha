package org.hyperledger.iroha.sdk.core.model.instructions

private const val ARG_ACTION = "action"
private const val ARG_ESCROW_ID = "escrow_id"
private const val ARG_ASSET_DEFINITION = "asset_definition"
private const val ARG_AMOUNT = "amount"
private const val ARG_BUYER_AMOUNT = "buyer_amount"
private const val ARG_SELLER_AMOUNT = "seller_amount"
private const val ARG_EVIDENCE_HASHES = "evidence_hashes"
private const val ARG_FUNDING_NULLIFIERS = "funding_nullifiers"
private const val ARG_ESCROW_COMMITMENT = "escrow_commitment"
private const val ARG_ESCROW_NULLIFIERS = "escrow_nullifiers"
private const val ARG_BUYER_OUTPUT_COMMITMENTS = "buyer_output_commitments"
private const val ARG_SELLER_OUTPUT_COMMITMENTS = "seller_output_commitments"
private const val ARG_PROOF = "proof"
private const val ARG_ROOT_HINT = "root_hint"

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

private fun normalizedEscrowList(values: List<String>, name: String): List<String> =
    values.mapIndexed { index, value ->
        validatedEscrowValue(value, "$name[$index]")
    }

private fun appendEscrowEvidence(args: MutableMap<String, String>, evidenceHashes: List<String>) {
    if (evidenceHashes.isNotEmpty()) {
        args[ARG_EVIDENCE_HASHES] = evidenceHashes.joinToString(",")
    }
}

private fun appendOptionalEscrowValue(args: MutableMap<String, String>, key: String, value: String?) {
    if (value != null) {
        args[key] = validatedEscrowValue(value, key)
    }
}

private fun appendEscrowList(args: MutableMap<String, String>, key: String, values: List<String>) {
    args[key] = values.joinToString(",")
}

private fun parseEscrowEvidenceHashes(raw: String?): List<String> =
    raw?.split(',')
        ?.map { it.trim() }
        ?.filter { it.isNotEmpty() }
        ?: emptyList()

private fun parseEscrowList(raw: String?): List<String> = parseEscrowEvidenceHashes(raw)

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
    action: String,
    escrowId: String,
    evidenceHashes: List<String>,
): Map<String, String> {
    val args = linkedMapOf(
        ARG_ACTION to action,
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

private fun openAnonymousEscrowArguments(
    escrowId: String,
    assetDefinition: String,
    fundingNullifiers: List<String>,
    escrowCommitment: String,
    proof: String,
    rootHint: String?,
    evidenceHashes: List<String>,
): Map<String, String> {
    val args = linkedMapOf(
        ARG_ACTION to OpenAnonymousAssetEscrowInstruction.ACTION,
        ARG_ESCROW_ID to validatedEscrowValue(escrowId, "escrowId"),
        ARG_ASSET_DEFINITION to validatedEscrowValue(assetDefinition, "assetDefinition"),
        ARG_ESCROW_COMMITMENT to validatedEscrowValue(escrowCommitment, "escrowCommitment"),
        ARG_PROOF to validatedEscrowValue(proof, "proof"),
    )
    appendEscrowList(args, ARG_FUNDING_NULLIFIERS, normalizedEscrowList(fundingNullifiers, "fundingNullifiers"))
    appendOptionalEscrowValue(args, ARG_ROOT_HINT, rootHint)
    appendEscrowEvidence(args, normalizedEscrowEvidenceHashes(evidenceHashes))
    return args
}

private fun releaseAnonymousEscrowArguments(
    action: String,
    escrowId: String,
    escrowNullifiers: List<String>,
    outputKey: String,
    outputName: String,
    outputCommitments: List<String>,
    proof: String,
    rootHint: String?,
): Map<String, String> {
    val args = linkedMapOf(
        ARG_ACTION to action,
        ARG_ESCROW_ID to validatedEscrowValue(escrowId, "escrowId"),
        ARG_PROOF to validatedEscrowValue(proof, "proof"),
    )
    appendEscrowList(args, ARG_ESCROW_NULLIFIERS, normalizedEscrowList(escrowNullifiers, "escrowNullifiers"))
    appendEscrowList(args, outputKey, normalizedEscrowList(outputCommitments, outputName))
    appendOptionalEscrowValue(args, ARG_ROOT_HINT, rootHint)
    return args
}

private fun resolveAnonymousEscrowArguments(
    escrowId: String,
    escrowNullifiers: List<String>,
    buyerOutputCommitments: List<String>,
    sellerOutputCommitments: List<String>,
    proof: String,
    rootHint: String?,
    evidenceHashes: List<String>,
): Map<String, String> {
    val args = linkedMapOf(
        ARG_ACTION to ResolveAnonymousEscrowDisputeInstruction.ACTION,
        ARG_ESCROW_ID to validatedEscrowValue(escrowId, "escrowId"),
        ARG_PROOF to validatedEscrowValue(proof, "proof"),
    )
    appendEscrowList(args, ARG_ESCROW_NULLIFIERS, normalizedEscrowList(escrowNullifiers, "escrowNullifiers"))
    appendEscrowList(
        args,
        ARG_BUYER_OUTPUT_COMMITMENTS,
        normalizedEscrowList(buyerOutputCommitments, "buyerOutputCommitments"),
    )
    appendEscrowList(
        args,
        ARG_SELLER_OUTPUT_COMMITMENTS,
        normalizedEscrowList(sellerOutputCommitments, "sellerOutputCommitments"),
    )
    appendOptionalEscrowValue(args, ARG_ROOT_HINT, rootHint)
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
        arguments = openDisputeArguments(ACTION, escrowId, evidenceHashes),
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

/** Typed representation of the `OpenAnonymousAssetEscrow` instruction. */
class OpenAnonymousAssetEscrowInstruction private constructor(
    @JvmField val escrowId: String,
    @JvmField val assetDefinition: String,
    @JvmField val fundingNullifiers: List<String>,
    @JvmField val escrowCommitment: String,
    @JvmField val proof: String,
    @JvmField val rootHint: String?,
    @JvmField val evidenceHashes: List<String>,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    constructor(
        escrowId: String,
        assetDefinition: String,
        fundingNullifiers: List<String>,
        escrowCommitment: String,
        proof: String,
        rootHint: String? = null,
        evidenceHashes: List<String> = emptyList(),
    ) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        assetDefinition = validatedEscrowValue(assetDefinition, "assetDefinition"),
        fundingNullifiers = normalizedEscrowList(fundingNullifiers, "fundingNullifiers"),
        escrowCommitment = validatedEscrowValue(escrowCommitment, "escrowCommitment"),
        proof = validatedEscrowValue(proof, "proof"),
        rootHint = rootHint?.let { validatedEscrowValue(it, "rootHint") },
        evidenceHashes = normalizedEscrowEvidenceHashes(evidenceHashes),
        arguments = openAnonymousEscrowArguments(
            escrowId,
            assetDefinition,
            fundingNullifiers,
            escrowCommitment,
            proof,
            rootHint,
            evidenceHashes,
        ),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is OpenAnonymousAssetEscrowInstruction) return false
        return escrowId == other.escrowId
            && assetDefinition == other.assetDefinition
            && fundingNullifiers == other.fundingNullifiers
            && escrowCommitment == other.escrowCommitment
            && proof == other.proof
            && rootHint == other.rootHint
            && evidenceHashes == other.evidenceHashes
    }

    override fun hashCode(): Int =
        listOf(escrowId, assetDefinition, fundingNullifiers, escrowCommitment, proof, rootHint, evidenceHashes)
            .hashCode()

    companion object {
        const val ACTION: String = "OpenAnonymousAssetEscrow"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): OpenAnonymousAssetEscrowInstruction =
            OpenAnonymousAssetEscrowInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                assetDefinition = requireEscrowArgument(arguments, ARG_ASSET_DEFINITION),
                fundingNullifiers = parseEscrowList(arguments[ARG_FUNDING_NULLIFIERS]),
                escrowCommitment = requireEscrowArgument(arguments, ARG_ESCROW_COMMITMENT),
                proof = requireEscrowArgument(arguments, ARG_PROOF),
                rootHint = arguments[ARG_ROOT_HINT],
                evidenceHashes = parseEscrowEvidenceHashes(arguments[ARG_EVIDENCE_HASHES]),
                arguments = LinkedHashMap(arguments),
            )
    }
}

/** Typed representation of the `AcceptAnonymousAssetEscrow` instruction. */
class AcceptAnonymousAssetEscrowInstruction private constructor(
    @JvmField val escrowId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    constructor(escrowId: String) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        arguments = escrowIdArguments(ACTION, validatedEscrowValue(escrowId, "escrowId")),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean =
        other is AcceptAnonymousAssetEscrowInstruction && escrowId == other.escrowId

    override fun hashCode(): Int = escrowId.hashCode()

    companion object {
        const val ACTION: String = "AcceptAnonymousAssetEscrow"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): AcceptAnonymousAssetEscrowInstruction =
            AcceptAnonymousAssetEscrowInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                arguments = LinkedHashMap(arguments),
            )
    }
}

/** Typed representation of the `MarkAnonymousEscrowPaymentSent` instruction. */
class MarkAnonymousEscrowPaymentSentInstruction private constructor(
    @JvmField val escrowId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    constructor(escrowId: String) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        arguments = escrowIdArguments(ACTION, validatedEscrowValue(escrowId, "escrowId")),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean =
        other is MarkAnonymousEscrowPaymentSentInstruction && escrowId == other.escrowId

    override fun hashCode(): Int = escrowId.hashCode()

    companion object {
        const val ACTION: String = "MarkAnonymousEscrowPaymentSent"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): MarkAnonymousEscrowPaymentSentInstruction =
            MarkAnonymousEscrowPaymentSentInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                arguments = LinkedHashMap(arguments),
            )
    }
}

/** Typed representation of the `ReleaseAnonymousAssetEscrow` instruction. */
class ReleaseAnonymousAssetEscrowInstruction private constructor(
    @JvmField val escrowId: String,
    @JvmField val escrowNullifiers: List<String>,
    @JvmField val buyerOutputCommitments: List<String>,
    @JvmField val proof: String,
    @JvmField val rootHint: String?,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    constructor(
        escrowId: String,
        escrowNullifiers: List<String>,
        buyerOutputCommitments: List<String>,
        proof: String,
        rootHint: String? = null,
    ) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        escrowNullifiers = normalizedEscrowList(escrowNullifiers, "escrowNullifiers"),
        buyerOutputCommitments = normalizedEscrowList(buyerOutputCommitments, "buyerOutputCommitments"),
        proof = validatedEscrowValue(proof, "proof"),
        rootHint = rootHint?.let { validatedEscrowValue(it, "rootHint") },
        arguments = releaseAnonymousEscrowArguments(
            ReleaseAnonymousAssetEscrowInstruction.ACTION,
            escrowId,
            escrowNullifiers,
            ARG_BUYER_OUTPUT_COMMITMENTS,
            "buyerOutputCommitments",
            buyerOutputCommitments,
            proof,
            rootHint,
        ),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ReleaseAnonymousAssetEscrowInstruction) return false
        return escrowId == other.escrowId
            && escrowNullifiers == other.escrowNullifiers
            && buyerOutputCommitments == other.buyerOutputCommitments
            && proof == other.proof
            && rootHint == other.rootHint
    }

    override fun hashCode(): Int = listOf(escrowId, escrowNullifiers, buyerOutputCommitments, proof, rootHint).hashCode()

    companion object {
        const val ACTION: String = "ReleaseAnonymousAssetEscrow"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ReleaseAnonymousAssetEscrowInstruction =
            ReleaseAnonymousAssetEscrowInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                escrowNullifiers = parseEscrowList(arguments[ARG_ESCROW_NULLIFIERS]),
                buyerOutputCommitments = parseEscrowList(arguments[ARG_BUYER_OUTPUT_COMMITMENTS]),
                proof = requireEscrowArgument(arguments, ARG_PROOF),
                rootHint = arguments[ARG_ROOT_HINT],
                arguments = LinkedHashMap(arguments),
            )
    }
}

/** Typed representation of the `CancelAnonymousAssetEscrow` instruction. */
class CancelAnonymousAssetEscrowInstruction private constructor(
    @JvmField val escrowId: String,
    @JvmField val escrowNullifiers: List<String>,
    @JvmField val sellerOutputCommitments: List<String>,
    @JvmField val proof: String,
    @JvmField val rootHint: String?,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    constructor(
        escrowId: String,
        escrowNullifiers: List<String>,
        sellerOutputCommitments: List<String>,
        proof: String,
        rootHint: String? = null,
    ) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        escrowNullifiers = normalizedEscrowList(escrowNullifiers, "escrowNullifiers"),
        sellerOutputCommitments = normalizedEscrowList(sellerOutputCommitments, "sellerOutputCommitments"),
        proof = validatedEscrowValue(proof, "proof"),
        rootHint = rootHint?.let { validatedEscrowValue(it, "rootHint") },
        arguments = releaseAnonymousEscrowArguments(
            CancelAnonymousAssetEscrowInstruction.ACTION,
            escrowId,
            escrowNullifiers,
            ARG_SELLER_OUTPUT_COMMITMENTS,
            "sellerOutputCommitments",
            sellerOutputCommitments,
            proof,
            rootHint,
        ),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is CancelAnonymousAssetEscrowInstruction) return false
        return escrowId == other.escrowId
            && escrowNullifiers == other.escrowNullifiers
            && sellerOutputCommitments == other.sellerOutputCommitments
            && proof == other.proof
            && rootHint == other.rootHint
    }

    override fun hashCode(): Int = listOf(escrowId, escrowNullifiers, sellerOutputCommitments, proof, rootHint).hashCode()

    companion object {
        const val ACTION: String = "CancelAnonymousAssetEscrow"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): CancelAnonymousAssetEscrowInstruction =
            CancelAnonymousAssetEscrowInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                escrowNullifiers = parseEscrowList(arguments[ARG_ESCROW_NULLIFIERS]),
                sellerOutputCommitments = parseEscrowList(arguments[ARG_SELLER_OUTPUT_COMMITMENTS]),
                proof = requireEscrowArgument(arguments, ARG_PROOF),
                rootHint = arguments[ARG_ROOT_HINT],
                arguments = LinkedHashMap(arguments),
            )
    }
}

/** Typed representation of the `OpenAnonymousEscrowDispute` instruction. */
class OpenAnonymousEscrowDisputeInstruction private constructor(
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
        arguments = openDisputeArguments(ACTION, escrowId, evidenceHashes),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean =
        other is OpenAnonymousEscrowDisputeInstruction
            && escrowId == other.escrowId
            && evidenceHashes == other.evidenceHashes

    override fun hashCode(): Int = listOf(escrowId, evidenceHashes).hashCode()

    companion object {
        const val ACTION: String = "OpenAnonymousEscrowDispute"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): OpenAnonymousEscrowDisputeInstruction =
            OpenAnonymousEscrowDisputeInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                evidenceHashes = parseEscrowEvidenceHashes(arguments[ARG_EVIDENCE_HASHES]),
                arguments = LinkedHashMap(arguments),
            )
    }
}

/** Typed representation of the `ResolveAnonymousEscrowDispute` instruction. */
class ResolveAnonymousEscrowDisputeInstruction private constructor(
    @JvmField val escrowId: String,
    @JvmField val escrowNullifiers: List<String>,
    @JvmField val buyerOutputCommitments: List<String>,
    @JvmField val sellerOutputCommitments: List<String>,
    @JvmField val proof: String,
    @JvmField val rootHint: String?,
    @JvmField val evidenceHashes: List<String>,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    constructor(
        escrowId: String,
        escrowNullifiers: List<String>,
        buyerOutputCommitments: List<String>,
        sellerOutputCommitments: List<String>,
        proof: String,
        rootHint: String? = null,
        evidenceHashes: List<String> = emptyList(),
    ) : this(
        escrowId = validatedEscrowValue(escrowId, "escrowId"),
        escrowNullifiers = normalizedEscrowList(escrowNullifiers, "escrowNullifiers"),
        buyerOutputCommitments = normalizedEscrowList(buyerOutputCommitments, "buyerOutputCommitments"),
        sellerOutputCommitments = normalizedEscrowList(sellerOutputCommitments, "sellerOutputCommitments"),
        proof = validatedEscrowValue(proof, "proof"),
        rootHint = rootHint?.let { validatedEscrowValue(it, "rootHint") },
        evidenceHashes = normalizedEscrowEvidenceHashes(evidenceHashes),
        arguments = resolveAnonymousEscrowArguments(
            escrowId,
            escrowNullifiers,
            buyerOutputCommitments,
            sellerOutputCommitments,
            proof,
            rootHint,
            evidenceHashes,
        ),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ResolveAnonymousEscrowDisputeInstruction) return false
        return escrowId == other.escrowId
            && escrowNullifiers == other.escrowNullifiers
            && buyerOutputCommitments == other.buyerOutputCommitments
            && sellerOutputCommitments == other.sellerOutputCommitments
            && proof == other.proof
            && rootHint == other.rootHint
            && evidenceHashes == other.evidenceHashes
    }

    override fun hashCode(): Int =
        listOf(
            escrowId,
            escrowNullifiers,
            buyerOutputCommitments,
            sellerOutputCommitments,
            proof,
            rootHint,
            evidenceHashes,
        ).hashCode()

    companion object {
        const val ACTION: String = "ResolveAnonymousEscrowDispute"

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ResolveAnonymousEscrowDisputeInstruction =
            ResolveAnonymousEscrowDisputeInstruction(
                escrowId = requireEscrowArgument(arguments, ARG_ESCROW_ID),
                escrowNullifiers = parseEscrowList(arguments[ARG_ESCROW_NULLIFIERS]),
                buyerOutputCommitments = parseEscrowList(arguments[ARG_BUYER_OUTPUT_COMMITMENTS]),
                sellerOutputCommitments = parseEscrowList(arguments[ARG_SELLER_OUTPUT_COMMITMENTS]),
                proof = requireEscrowArgument(arguments, ARG_PROOF),
                rootHint = arguments[ARG_ROOT_HINT],
                evidenceHashes = parseEscrowEvidenceHashes(arguments[ARG_EVIDENCE_HASHES]),
                arguments = LinkedHashMap(arguments),
            )
    }
}
