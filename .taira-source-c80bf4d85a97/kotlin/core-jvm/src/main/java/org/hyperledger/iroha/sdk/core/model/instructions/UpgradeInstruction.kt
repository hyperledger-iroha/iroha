@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

/** Typed representation of `Upgrade` instructions carrying executor bytecode. */
class UpgradeInstruction private constructor(
    bytecode: ByteArray,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    init {
        require(bytecode.isNotEmpty()) { "bytecode must not be empty" }
    }

    private val _bytecode: ByteArray = bytecode.copyOf()

    /** Returns the executor bytecode that will replace the active executor once applied. */
    val bytecode: ByteArray get() = _bytecode.copyOf()

    constructor(bytecode: ByteArray) : this(
        bytecode = bytecode,
        arguments = linkedMapOf(
            "action" to "UpgradeExecutor",
            "bytecode" to Base64.encode(bytecode),
        ),
    )

    override val kind: InstructionKind get() = InstructionKind.UPGRADE

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is UpgradeInstruction) return false
        return _bytecode.contentEquals(other._bytecode)
    }

    override fun hashCode(): Int = _bytecode.contentHashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): UpgradeInstruction {
            val encoded = require(arguments, "bytecode")
            val decoded = try {
                Base64.decode(encoded)
            } catch (ex: IllegalArgumentException) {
                throw IllegalArgumentException("bytecode argument must be base64", ex)
            }
            return UpgradeInstruction(
                bytecode = decoded,
                arguments = LinkedHashMap(arguments),
            )
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
