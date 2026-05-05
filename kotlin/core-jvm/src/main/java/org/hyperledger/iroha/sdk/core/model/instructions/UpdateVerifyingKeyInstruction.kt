package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.instructions.VerifyingKeyInstructionUtils.nonEmptyString
import org.hyperledger.iroha.sdk.core.model.instructions.VerifyingKeyInstructionUtils.parseRecord
import org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyRecordDescription

private const val ACTION = "UpdateVerifyingKey"

/** Typed representation of `UpdateVerifyingKey` instructions. */
class UpdateVerifyingKeyInstruction private constructor(
    @JvmField val backend: String,
    @JvmField val name: String,
    @JvmField val record: VerifyingKeyRecordDescription,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    constructor(
        backend: String,
        name: String,
        record: VerifyingKeyRecordDescription,
    ) : this(
        backend = validatedNotBlank(backend, "backend"),
        name = validatedNotBlank(name, "name"),
        record = record,
        arguments = buildArguments(backend.trim(), name.trim(), record),
    )

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is UpdateVerifyingKeyInstruction) return false
        return backend == other.backend
            && name == other.name
            && record == other.record
    }

    override fun hashCode(): Int {
        var result = backend.hashCode()
        result = 31 * result + name.hashCode()
        result = 31 * result + record.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): UpdateVerifyingKeyInstruction {
            val backend = arguments.nonEmptyString("backend")
            val name = arguments.nonEmptyString("name")
            val record = arguments.parseRecord(backend)
            return UpdateVerifyingKeyInstruction(backend, name, record)
        }

        private fun buildArguments(
            backend: String,
            name: String,
            record: VerifyingKeyRecordDescription,
        ): Map<String, String> {
            val args = linkedMapOf<String, String>()
            args["action"] = ACTION
            args["backend"] = backend
            args["name"] = name
            args.putAll(record.toArguments(backend))
            return args
        }

        private fun validatedNotBlank(value: String, name: String): String {
            val trimmed = value.trim()
            require(trimmed.isNotEmpty()) { "$name must not be blank" }
            return trimmed
        }
    }
}
