package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.instructions.VerifyingKeyInstructionUtils.nonEmptyString
import org.hyperledger.iroha.sdk.core.model.instructions.VerifyingKeyInstructionUtils.parseRecord
import org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyRecordDescription

private const val ACTION = "RegisterVerifyingKey"

/** Typed representation of `RegisterVerifyingKey` instructions. */
class RegisterVerifyingKeyInstruction private constructor(
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
        if (other !is RegisterVerifyingKeyInstruction) return false
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
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterVerifyingKeyInstruction {
            val backend = arguments.nonEmptyString("backend")
            val name = arguments.nonEmptyString("name")
            val record = arguments.parseRecord(backend)
            return RegisterVerifyingKeyInstruction(backend, name, record)
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

    class Builder internal constructor() {
        private var backend: String? = null
        private var name: String? = null
        private var record: VerifyingKeyRecordDescription? = null

        fun setBackend(backend: String) = apply {
            require(backend.trim().isNotEmpty()) { "backend must not be blank" }
            this.backend = backend.trim()
        }

        fun setName(name: String) = apply {
            require(name.trim().isNotEmpty()) { "name must not be blank" }
            this.name = name.trim()
        }

        fun setRecord(record: VerifyingKeyRecordDescription) = apply {
            this.record = requireNotNull(record) { "record" }
        }

        fun build(): RegisterVerifyingKeyInstruction {
            val b = checkNotNull(backend) { "backend must be provided" }
            val n = checkNotNull(name) { "name must be provided" }
            val r = checkNotNull(record) { "record must be provided" }
            return RegisterVerifyingKeyInstruction(b, n, r)
        }
    }
}
