package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.WirePayload

private const val INSTRUCTION_PREFIX = "instruction."
private const val KIND_SUFFIX = ".kind"
private const val ARG_PREFIX = ".arg."
private const val METADATA_PREFIX = "metadata."

/** Internal helpers shared by trigger registration instruction builders. */
internal object TriggerInstructionUtils {

    @JvmStatic
    fun extractMetadata(arguments: Map<String, String>): Map<String, String> {
        val metadata = linkedMapOf<String, String>()
        arguments.forEach { (key, value) ->
            if (key.startsWith(METADATA_PREFIX)) {
                val metadataKey = key.substring(METADATA_PREFIX.length)
                if (metadataKey.isNotEmpty()) {
                    metadata[metadataKey] = value
                }
            }
        }
        return metadata
    }

    @JvmStatic
    fun appendMetadata(metadata: Map<String, String>, target: MutableMap<String, String>) {
        metadata.forEach { (key, value) -> target["$METADATA_PREFIX$key"] = value }
    }

    @JvmStatic
    fun appendInstructions(instructions: List<InstructionBox>, target: MutableMap<String, String>) {
        instructions.forEachIndexed { index, instruction ->
            require(instruction.payload is WirePayload) {
                "Trigger registration requires wire-framed instruction payloads"
            }
            target["$INSTRUCTION_PREFIX$index$KIND_SUFFIX"] = instruction.kind.displayName
            instruction.arguments.forEach { (key, value) ->
                target["$INSTRUCTION_PREFIX$index$ARG_PREFIX$key"] = value
            }
        }
    }

    @JvmStatic
    fun parseInstructions(arguments: Map<String, String>): List<InstructionBox> {
        val grouped = sortedMapOf<Int, InstructionParts>()
        for ((key, value) in arguments) {
            if (!key.startsWith(INSTRUCTION_PREFIX)) continue
            val remainder = key.substring(INSTRUCTION_PREFIX.length)
            val separatorIndex = remainder.indexOf('.')
            require(separatorIndex > 0) { "Malformed trigger instruction key: $key" }
            val index = try {
                remainder.substring(0, separatorIndex).toInt()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("Trigger instruction index must be numeric: $key", ex)
            }
            val parts = grouped.getOrPut(index) { InstructionParts() }
            val suffix = remainder.substring(separatorIndex)
            when {
                suffix == KIND_SUFFIX -> parts.kind = value
                suffix.startsWith(ARG_PREFIX) -> {
                    val argumentKey = suffix.substring(ARG_PREFIX.length)
                    require(argumentKey.isNotEmpty()) { "Trigger instruction argument key must not be empty" }
                    parts.arguments[argumentKey] = value
                }
                else -> throw IllegalArgumentException("Unrecognised trigger instruction key: $key")
            }
        }
        require(grouped.isNotEmpty()) { "Trigger registration must include at least one instruction" }
        return grouped.map { (index, parts) ->
            require(!parts.kind.isNullOrBlank()) {
                "instruction.$index.kind is required for trigger registration"
            }
            require(parts.arguments.containsKey("wire_name") && parts.arguments.containsKey("payload_base64")) {
                "instruction.$index must include wire_name and payload_base64"
            }
            require(parts.arguments.size == 2) {
                "instruction.$index must not include extra arguments when using wire payloads"
            }
            InstructionBox.fromNorito(LinkedHashMap(parts.arguments))
        }
    }

    @JvmStatic
    fun parseRepeats(value: String?): Int? {
        if (value.isNullOrBlank() || value.equals(RegisterTimeTriggerInstruction.REPEATS_INDEFINITE, ignoreCase = true)) {
            return null
        }
        val parsed = try {
            Integer.parseUnsignedInt(value)
        } catch (ex: NumberFormatException) {
            throw IllegalArgumentException("repeats must be an unsigned integer", ex)
        }
        require(parsed > 0) { "repeats must be greater than zero when provided" }
        return parsed
    }

    private class InstructionParts {
        var kind: String? = null
        val arguments: LinkedHashMap<String, String> = linkedMapOf()
    }
}
