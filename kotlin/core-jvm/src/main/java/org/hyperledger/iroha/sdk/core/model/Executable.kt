package org.hyperledger.iroha.sdk.core.model

/**
 * Represents the executable payload embedded in a transaction. Mirrors the Rust enum
 * `Executable::{Instructions, Ivm}`.
 */
sealed class Executable {

    class Instructions(instructions: List<InstructionBox>) : Executable() {

        private val _instructions: List<InstructionBox> = instructions.toList()

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

    companion object {
        @JvmStatic
        fun instructions(instructions: List<InstructionBox>): Executable =
            Instructions(instructions)

        @JvmStatic
        fun ivm(ivmBytes: ByteArray): Executable = Ivm(ivmBytes)
    }
}
