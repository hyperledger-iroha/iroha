package org.hyperledger.iroha.sdk.core.model

/**
 * Represents the executable payload embedded in a transaction.
 */
sealed class Executable {

    class Instructions(instructions: List<InstructionBox>) : Executable() {

        private val _instructions: List<InstructionBox> =
            java.util.Collections.unmodifiableList(ArrayList(instructions))

        val instructions: List<InstructionBox> get() = _instructions

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is Instructions) return false
            return _instructions == other._instructions
        }

        override fun hashCode(): Int = _instructions.hashCode()
    }

    class Ivm(ivmBytes: ByteArray) : Executable() {

        private val _ivmBytes: ByteArray = ivmBytes.copyOf()

        val ivmBytes: ByteArray get() = _ivmBytes.copyOf()

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is Ivm) return false
            return _ivmBytes.contentEquals(other._ivmBytes)
        }

        override fun hashCode(): Int = _ivmBytes.contentHashCode()
    }

    /** Invokes one deployed contract instance by reference. */
    class ContractCall(@JvmField val invocation: ContractInvocation) : Executable() {
        override fun equals(other: Any?): Boolean =
            this === other || other is ContractCall && invocation == other.invocation

        override fun hashCode(): Int = invocation.hashCode()
    }

    /** Ordered, atomic mix of native instructions and deployed-contract invocations. */
    class Batch(entries: List<ExecutableBatchItem>) : Executable() {
        init {
            require(entries.isNotEmpty()) { "executable batch must contain at least one item" }
        }

        private val _entries: List<ExecutableBatchItem> =
            java.util.Collections.unmodifiableList(ArrayList(entries))

        val entries: List<ExecutableBatchItem> get() = _entries

        override fun equals(other: Any?): Boolean =
            this === other || other is Batch && _entries == other._entries

        override fun hashCode(): Int = _entries.hashCode()
    }

    /** Mutable authoring helper that preserves the exact order in which batch items are added. */
    class BatchBuilder internal constructor() {
        private val entries: MutableList<ExecutableBatchItem> = mutableListOf()

        fun addInstruction(instruction: InstructionBox) = apply {
            entries.add(ExecutableBatchItem.instruction(instruction))
        }

        fun addContractCall(invocation: ContractInvocation) = apply {
            entries.add(ExecutableBatchItem.contractCall(invocation))
        }

        fun addItem(item: ExecutableBatchItem) = apply {
            entries.add(item)
        }

        fun build(): Executable = Batch(entries)
    }

    /** Whether this executable requires a signature-bound gas limit in its fee intent. */
    fun requiresTransactionGasLimit(): Boolean = when (this) {
        is Instructions -> false
        is Ivm, is ContractCall -> true
        is Batch -> entries.any { it is ExecutableBatchItem.ContractCall }
    }

    companion object {
        @JvmStatic
        fun instructions(instructions: List<InstructionBox>): Executable =
            Instructions(instructions)

        @JvmStatic
        fun ivm(ivmBytes: ByteArray): Executable = Ivm(ivmBytes)

        @JvmStatic
        fun contractCall(invocation: ContractInvocation): Executable = ContractCall(invocation)

        @JvmStatic
        fun batch(entries: List<ExecutableBatchItem>): Executable = Batch(entries)

        @JvmStatic
        fun batchBuilder(): BatchBuilder = BatchBuilder()
    }
}
