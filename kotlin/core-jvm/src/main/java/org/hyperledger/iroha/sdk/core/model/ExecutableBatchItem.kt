package org.hyperledger.iroha.sdk.core.model

/** One ordered native-instruction or deployed-contract-call item in an executable batch. */
sealed class ExecutableBatchItem {

    /** A native Iroha Special Instruction batch item. */
    class Instruction(@JvmField val instruction: InstructionBox) : ExecutableBatchItem() {
        override fun equals(other: Any?): Boolean =
            this === other || other is Instruction && instruction == other.instruction

        override fun hashCode(): Int = instruction.hashCode()
    }

    /** A deployed-contract invocation batch item. */
    class ContractCall(@JvmField val invocation: ContractInvocation) : ExecutableBatchItem() {
        override fun equals(other: Any?): Boolean =
            this === other || other is ContractCall && invocation == other.invocation

        override fun hashCode(): Int = invocation.hashCode()
    }

    companion object {
        /** Wraps a native instruction as a batch item. */
        @JvmStatic
        fun instruction(instruction: InstructionBox): ExecutableBatchItem = Instruction(instruction)

        /** Wraps a deployed-contract call as a batch item. */
        @JvmStatic
        fun contractCall(invocation: ContractInvocation): ExecutableBatchItem = ContractCall(invocation)
    }
}
