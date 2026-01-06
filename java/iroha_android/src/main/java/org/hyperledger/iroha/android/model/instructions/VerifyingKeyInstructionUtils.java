package org.hyperledger.iroha.android.model.instructions;

import java.util.Base64;
import java.util.Map;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyBackendTag;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyRecordDescription;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyStatus;

final class VerifyingKeyInstructionUtils {

  private VerifyingKeyInstructionUtils() {}

  static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value.trim();
  }

  static String optional(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null) {
      return null;
    }
    final String trimmed = value.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }

  static Integer parseOptionalInt(final Map<String, String> arguments, final String key) {
    final String value = optional(arguments, key);
    if (value == null) {
      return null;
    }
    return Integer.valueOf(value);
  }

  static Long parseOptionalLong(final Map<String, String> arguments, final String key) {
    final String value = optional(arguments, key);
    if (value == null) {
      return null;
    }
    return Long.valueOf(value);
  }

  static VerifyingKeyRecordDescription parseRecord(
      final Map<String, String> arguments, final String backend) {
    final VerifyingKeyRecordDescription.Builder builder = VerifyingKeyRecordDescription.builder();
    builder.setVersion(Integer.parseUnsignedInt(require(arguments, "record.version")));
    builder.setCircuitId(require(arguments, "record.circuit_id"));
    builder.setBackendTag(
        VerifyingKeyBackendTag.parse(require(arguments, "record.backend_tag")));
    builder.setCurve(optional(arguments, "record.curve"));
    builder.setSchemaHashHex(require(arguments, "record.public_inputs_schema_hash_hex"));
    builder.setCommitmentHex(optional(arguments, "record.commitment_hex"));
    final String vkBytesB64 = optional(arguments, "record.vk_bytes_b64");
    if (vkBytesB64 != null) {
      try {
        builder.setInlineKeyBytes(Base64.getDecoder().decode(vkBytesB64));
      } catch (final IllegalArgumentException ex) {
        throw new IllegalArgumentException("record.vk_bytes_b64 is not valid base64", ex);
      }
    }
    builder.setVkLength(parseOptionalInt(arguments, "record.vk_len"));
    builder.setMaxProofBytes(parseOptionalInt(arguments, "record.max_proof_bytes"));
    builder.setGasScheduleId(require(arguments, "record.gas_schedule_id"));
    builder.setMetadataUriCid(optional(arguments, "record.metadata_uri_cid"));
    builder.setVkBytesCid(optional(arguments, "record.vk_bytes_cid"));
    builder.setActivationHeight(parseOptionalLong(arguments, "record.activation_height"));
    final Long withdrawHeight = parseOptionalLong(arguments, "record.withdraw_height");
    final Long deprecationHeight = parseOptionalLong(arguments, "record.deprecation_height");
    if (withdrawHeight != null
        && deprecationHeight != null
        && !withdrawHeight.equals(deprecationHeight)) {
      throw new IllegalArgumentException(
          "record.deprecation_height must match record.withdraw_height when both are set");
    }
    builder.setWithdrawHeight(withdrawHeight != null ? withdrawHeight : deprecationHeight);
    final String statusValue = optional(arguments, "record.status");
    if (statusValue != null) {
      builder.setStatus(VerifyingKeyStatus.parse(statusValue));
    }
    return builder.build(backend);
  }
}
