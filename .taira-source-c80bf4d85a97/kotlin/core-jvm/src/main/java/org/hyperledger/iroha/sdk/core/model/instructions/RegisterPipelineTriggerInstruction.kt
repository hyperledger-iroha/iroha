package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.InstructionBox

/** Typed builder for `RegisterPipelineTrigger` instructions. */
class RegisterPipelineTriggerInstruction private constructor(
    @JvmField val triggerId: String,
    @JvmField val authority: String,
    @JvmField val filter: PipelineFilter,
    @JvmField val repeats: Int?,
    @JvmField val instructions: List<InstructionBox>,
    @JvmField val metadata: Map<String, String>,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.REGISTER

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterPipelineTriggerInstruction) return false
        return triggerId == other.triggerId
            && authority == other.authority
            && filter == other.filter
            && repeats == other.repeats
            && instructions == other.instructions
            && metadata == other.metadata
    }

    override fun hashCode(): Int {
        var result = triggerId.hashCode()
        result = 31 * result + authority.hashCode()
        result = 31 * result + filter.hashCode()
        result = 31 * result + (repeats?.hashCode() ?: 0)
        result = 31 * result + instructions.hashCode()
        result = 31 * result + metadata.hashCode()
        return result
    }

    class Builder internal constructor() {
        private var triggerId: String? = null
        private var authority: String? = null
        private var filter: PipelineFilter? = null
        private var repeats: Int? = null
        private val instructions: MutableList<InstructionBox> = mutableListOf()
        private val metadata: MutableMap<String, String> = linkedMapOf()

        fun setTriggerId(triggerId: String) = apply {
            require(triggerId.isNotBlank()) { "triggerId must not be blank" }
            this.triggerId = triggerId
        }

        fun setAuthority(authority: String) = apply {
            require(authority.isNotBlank()) { "authority must not be blank" }
            this.authority = authority
        }

        fun setFilter(filter: PipelineFilter) = apply {
            this.filter = requireNotNull(filter) { "filter" }
        }

        fun setRepeats(repeats: Int?) = apply {
            if (repeats != null) {
                require(repeats > 0) { "repeats must be greater than zero when provided" }
            }
            this.repeats = repeats
        }

        fun addInstruction(instruction: InstructionBox) = apply {
            instructions.add(requireNotNull(instruction) { "instruction" })
        }

        fun setInstructions(newInstructions: List<InstructionBox>?) = apply {
            instructions.clear()
            newInstructions?.forEach { addInstruction(it) }
        }

        fun putMetadata(key: String, value: String) = apply {
            metadata[requireNotNull(key) { "metadata key" }] = requireNotNull(value) { "metadata value" }
        }

        fun setMetadata(entries: Map<String, String>?) = apply {
            metadata.clear()
            entries?.forEach { (k, v) -> putMetadata(k, v) }
        }

        fun build(): RegisterPipelineTriggerInstruction {
            val tid = checkNotNull(triggerId) { "triggerId must be set" }
            val auth = checkNotNull(authority) { "authority must be set" }
            val f = checkNotNull(filter) { "filter must be set" }
            check(instructions.isNotEmpty()) { "at least one instruction must be provided" }
            val args = buildCanonicalArguments(tid, auth, f, repeats, instructions, metadata)
            return RegisterPipelineTriggerInstruction(
                tid, auth, f, repeats, instructions.toList(), metadata.toMap(), args,
            )
        }

        private fun buildCanonicalArguments(
            triggerId: String,
            authority: String,
            filter: PipelineFilter,
            repeats: Int?,
            instructions: List<InstructionBox>,
            metadata: Map<String, String>,
        ): Map<String, String> = buildMap {
            put("action", ACTION)
            put("trigger", triggerId)
            put("authority", authority)
            put(
                "repeats",
                if (repeats == null) RegisterTimeTriggerInstruction.REPEATS_INDEFINITE
                else Integer.toUnsignedString(repeats),
            )
            filter.appendArguments(this)
            TriggerInstructionUtils.appendInstructions(instructions, this)
            TriggerInstructionUtils.appendMetadata(metadata, this)
        }
    }

    class PipelineFilter private constructor(@JvmField val filterKind: Kind) {

        var transactionHash: String? = null; private set
        var transactionBlockHeight: Long? = null; private set
        var transactionStatus: String? = null; private set
        var blockHeight: Long? = null; private set
        var blockStatus: String? = null; private set
        var mergeEpochId: Long? = null; private set
        var witnessBlockHash: String? = null; private set
        var witnessHeight: Long? = null; private set
        var witnessView: Long? = null; private set

        fun setTransactionHash(hashHex: String) = apply {
            ensureKind(Kind.TRANSACTION)
            require(hashHex.isNotBlank()) { "transaction hash must not be blank" }
            this.transactionHash = hashHex
        }

        fun setTransactionBlockHeight(blockHeight: Long?) = apply {
            ensureKind(Kind.TRANSACTION)
            if (blockHeight != null) require(blockHeight > 0) { "transaction block height must be positive when provided" }
            this.transactionBlockHeight = blockHeight
        }

        fun setTransactionStatus(status: String?) = apply {
            ensureKind(Kind.TRANSACTION)
            if (status != null) require(status.isNotBlank()) { "transaction status must not be blank when provided" }
            this.transactionStatus = status
        }

        fun setBlockHeight(height: Long?) = apply {
            ensureKind(Kind.BLOCK)
            if (height != null) require(height > 0) { "block height must be positive when provided" }
            this.blockHeight = height
        }

        fun setBlockStatus(status: String?) = apply {
            ensureKind(Kind.BLOCK)
            if (status != null) require(status.isNotBlank()) { "block status must not be blank when provided" }
            this.blockStatus = status
        }

        fun setMergeEpochId(epochId: Long?) = apply {
            ensureKind(Kind.MERGE)
            if (epochId != null) require(epochId >= 0) { "epoch id must be non-negative when provided" }
            this.mergeEpochId = epochId
        }

        fun setWitnessBlockHash(blockHashHex: String) = apply {
            ensureKind(Kind.WITNESS)
            require(blockHashHex.isNotBlank()) { "witness block hash must not be blank" }
            this.witnessBlockHash = blockHashHex
        }

        fun setWitnessHeight(height: Long?) = apply {
            ensureKind(Kind.WITNESS)
            if (height != null) require(height > 0) { "witness height must be positive when provided" }
            this.witnessHeight = height
        }

        fun setWitnessView(view: Long?) = apply {
            ensureKind(Kind.WITNESS)
            if (view != null) require(view >= 0) { "witness view must be non-negative when provided" }
            this.witnessView = view
        }

        private fun ensureKind(expected: Kind) {
            check(filterKind == expected) { "Filter variant mismatch: expected $expected but was $filterKind" }
        }

        internal fun appendArguments(target: MutableMap<String, String>) {
            target["filter.kind"] = filterKind.displayName
            when (filterKind) {
                Kind.TRANSACTION -> {
                    transactionHash?.let { target["filter.transaction.hash"] = it }
                    transactionBlockHeight?.let { target["filter.transaction.block_height"] = java.lang.Long.toUnsignedString(it) }
                    transactionStatus?.let { target["filter.transaction.status"] = it }
                }
                Kind.BLOCK -> {
                    blockHeight?.let { target["filter.block.height"] = java.lang.Long.toUnsignedString(it) }
                    blockStatus?.let { target["filter.block.status"] = it }
                }
                Kind.MERGE -> {
                    mergeEpochId?.let { target["filter.merge.epoch_id"] = java.lang.Long.toUnsignedString(it) }
                }
                Kind.WITNESS -> {
                    witnessBlockHash?.let { target["filter.witness.block_hash"] = it }
                    witnessHeight?.let { target["filter.witness.height"] = java.lang.Long.toUnsignedString(it) }
                    witnessView?.let { target["filter.witness.view"] = java.lang.Long.toUnsignedString(it) }
                }
            }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is PipelineFilter) return false
            return filterKind == other.filterKind
                && transactionHash == other.transactionHash
                && transactionBlockHeight == other.transactionBlockHeight
                && transactionStatus == other.transactionStatus
                && blockHeight == other.blockHeight
                && blockStatus == other.blockStatus
                && mergeEpochId == other.mergeEpochId
                && witnessBlockHash == other.witnessBlockHash
                && witnessHeight == other.witnessHeight
                && witnessView == other.witnessView
        }

        override fun hashCode(): Int {
            var result = filterKind.hashCode()
            result = 31 * result + (transactionHash?.hashCode() ?: 0)
            result = 31 * result + (transactionBlockHeight?.hashCode() ?: 0)
            result = 31 * result + (transactionStatus?.hashCode() ?: 0)
            result = 31 * result + (blockHeight?.hashCode() ?: 0)
            result = 31 * result + (blockStatus?.hashCode() ?: 0)
            result = 31 * result + (mergeEpochId?.hashCode() ?: 0)
            result = 31 * result + (witnessBlockHash?.hashCode() ?: 0)
            result = 31 * result + (witnessHeight?.hashCode() ?: 0)
            result = 31 * result + (witnessView?.hashCode() ?: 0)
            return result
        }

        enum class Kind(@JvmField val displayName: String) {
            TRANSACTION("Transaction"),
            BLOCK("Block"),
            MERGE("Merge"),
            WITNESS("Witness");

            companion object {
                @JvmStatic
                fun fromDisplayName(value: String): Kind =
                    entries.find { it.displayName.equals(value, ignoreCase = true) }
                        ?: throw IllegalArgumentException("Unknown pipeline filter kind: $value")
            }
        }

        companion object {
            @JvmStatic fun transaction(): PipelineFilter = PipelineFilter(Kind.TRANSACTION)
            @JvmStatic fun block(): PipelineFilter = PipelineFilter(Kind.BLOCK)
            @JvmStatic fun merge(): PipelineFilter = PipelineFilter(Kind.MERGE)
            @JvmStatic fun witness(): PipelineFilter = PipelineFilter(Kind.WITNESS)

            @JvmStatic
            fun fromArguments(arguments: Map<String, String>): PipelineFilter {
                val kindValue = arguments["filter.kind"]
                require(!kindValue.isNullOrBlank()) { "filter.kind is required for RegisterPipelineTrigger" }
                val kind = Kind.fromDisplayName(kindValue)
                val filter = PipelineFilter(kind)
                when (kind) {
                    Kind.TRANSACTION -> {
                        arguments["filter.transaction.hash"]?.let { filter.setTransactionHash(it) }
                        arguments["filter.transaction.block_height"]?.let {
                            filter.setTransactionBlockHeight(parseUnsignedLong(it, "filter.transaction.block_height"))
                        }
                        arguments["filter.transaction.status"]?.let { filter.setTransactionStatus(it) }
                    }
                    Kind.BLOCK -> {
                        arguments["filter.block.height"]?.let {
                            filter.setBlockHeight(parseUnsignedLong(it, "filter.block.height"))
                        }
                        arguments["filter.block.status"]?.let { filter.setBlockStatus(it) }
                    }
                    Kind.MERGE -> {
                        arguments["filter.merge.epoch_id"]?.let {
                            filter.setMergeEpochId(parseUnsignedLong(it, "filter.merge.epoch_id"))
                        }
                    }
                    Kind.WITNESS -> {
                        arguments["filter.witness.block_hash"]?.let { filter.setWitnessBlockHash(it) }
                        arguments["filter.witness.height"]?.let {
                            filter.setWitnessHeight(parseUnsignedLong(it, "filter.witness.height"))
                        }
                        arguments["filter.witness.view"]?.let {
                            filter.setWitnessView(parseUnsignedLong(it, "filter.witness.view"))
                        }
                    }
                }
                return filter
            }

            private fun parseUnsignedLong(value: String, field: String): Long = try {
                val parsed = java.lang.Long.parseUnsignedLong(value)
                if (parsed < 0) throw NumberFormatException("negative")
                parsed
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("$field must be an unsigned integer", ex)
            }
        }
    }

    companion object {
        const val ACTION: String = "RegisterPipelineTrigger"

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterPipelineTriggerInstruction {
            val triggerId = requireArg(arguments, "trigger")
            val authority = requireArg(arguments, "authority")
            val filter = PipelineFilter.fromArguments(arguments)
            val instructions = TriggerInstructionUtils.parseInstructions(arguments)
            val metadata = TriggerInstructionUtils.extractMetadata(arguments)
            val repeats = TriggerInstructionUtils.parseRepeats(arguments["repeats"])
            return RegisterPipelineTriggerInstruction(
                triggerId = triggerId,
                authority = authority,
                filter = filter,
                repeats = repeats,
                instructions = instructions,
                metadata = metadata,
                arguments = LinkedHashMap(arguments),
            )
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
