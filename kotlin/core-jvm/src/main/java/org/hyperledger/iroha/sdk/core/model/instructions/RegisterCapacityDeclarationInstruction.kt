@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model.instructions

import java.util.Collections
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

private const val CAPACITY_ACTION = "RegisterCapacityDeclaration"
private const val MAX_DECLARATION_BYTES = 256 * 1024

/**
 * One canonical Norito capacity declaration. Consensus validates its contents and derives the
 * registered epoch, provider, capacity, validity and metadata; callers cannot supply projections.
 * This local argument representation does not itself encode a wire-framed instruction.
 */
class RegisterCapacityDeclarationInstruction(declaration: ByteArray) : InstructionTemplate {
    @JvmField val declarationBase64: String
    private val canonicalArguments: Map<String, String>

    init {
        require(declaration.isNotEmpty() && declaration.size <= MAX_DECLARATION_BYTES) {
            "declaration must contain 1..$MAX_DECLARATION_BYTES bytes"
        }
        declarationBase64 = Base64.encode(declaration.copyOf())
        canonicalArguments = Collections.unmodifiableMap(linkedMapOf(
            "action" to CAPACITY_ACTION,
            "declaration_b64" to declarationBase64,
        ))
    }

    override val kind: InstructionKind get() = InstructionKind.REGISTER
    override val arguments: Map<String, String> get() = canonicalArguments

    /** Return a fresh copy of the bounded canonical declaration material. */
    fun declarationBytes(): ByteArray = Base64.decode(declarationBase64)

    override fun equals(other: Any?): Boolean =
        other is RegisterCapacityDeclarationInstruction && declarationBase64 == other.declarationBase64

    override fun hashCode(): Int = declarationBase64.hashCode()

    /** Java and Kotlin builder for the sole declaration field. */
    class Builder internal constructor() {
        private var declaration: ByteArray? = null

        fun setDeclarationBytes(bytes: ByteArray) = apply {
            require(bytes.isNotEmpty() && bytes.size <= MAX_DECLARATION_BYTES) {
                "declaration must contain 1..$MAX_DECLARATION_BYTES bytes"
            }
            declaration = bytes.copyOf()
        }

        fun setDeclarationBase64(value: String) = apply { declaration = decodeDeclaration(value) }

        fun build(): RegisterCapacityDeclarationInstruction = RegisterCapacityDeclarationInstruction(
            checkNotNull(declaration) { "declaration must be provided" },
        )
    }

    companion object {
        @JvmStatic fun builder(): Builder = Builder()

        /** Reject retired projection fields, unknown arguments and noncanonical Base64. */
        @JvmStatic fun fromArguments(arguments: Map<String, String>): RegisterCapacityDeclarationInstruction {
            require(arguments.keys == setOf("action", "declaration_b64")) {
                "capacity declaration arguments must contain only action and declaration_b64"
            }
            require(arguments["action"] == CAPACITY_ACTION) { "invalid capacity declaration action" }
            val payload = requireNotNull(arguments["declaration_b64"]) { "declaration_b64 is required" }
            return RegisterCapacityDeclarationInstruction(decodeDeclaration(payload))
        }

        private fun decodeDeclaration(value: String): ByteArray {
            val maximumEncodedLength = ((MAX_DECLARATION_BYTES + 2) / 3) * 4
            require(value.isNotEmpty() && value.length <= maximumEncodedLength) {
                "declaration Base64 exceeds its bounded length or is empty"
            }
            val decoded = try { Base64.decode(value) } catch (error: IllegalArgumentException) {
                throw IllegalArgumentException("declaration must be Base64", error)
            }
            require(decoded.isNotEmpty() && decoded.size <= MAX_DECLARATION_BYTES) {
                "declaration must encode 1..$MAX_DECLARATION_BYTES bytes"
            }
            require(Base64.encode(decoded) == value) { "declaration must use canonical padded Base64" }
            return decoded
        }
    }
}
