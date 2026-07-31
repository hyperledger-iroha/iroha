package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Strict JSON codec for the typed Torii DA proof client. */
final class DaJson {
  private static final Pattern HASH_PATTERN =
      Pattern.compile("hash:[0-9A-F]{64}#[0-9A-F]{4}");

  private DaJson() {}

  static byte[] encode(final Object value) {
    final StringBuilder output = new StringBuilder();
    append(value, output);
    return output.toString().getBytes(StandardCharsets.UTF_8);
  }

  static Object parse(final byte[] bytes, final String field) {
    if (bytes == null || bytes.length == 0) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    try {
      return JsonParser.parse(new String(bytes, StandardCharsets.UTF_8));
    } catch (final RuntimeException error) {
      throw new IllegalArgumentException("invalid " + field + " JSON", error);
    }
  }

  static DaModels.ProofPolicyBundle parsePolicyBundle(
      final Object value, final String field) {
    final Map<String, Object> object =
        exactObject(value, field, keys("version", "policy_hash", "policies"));
    final int version = u16(object.get("version"), field + ".version");
    final String policyHash = string(object.get("policy_hash"), field + ".policy_hash");
    requireHash(policyHash, field + ".policy_hash");
    final List<Object> rawPolicies = list(object.get("policies"), field + ".policies");
    final List<DaModels.ProofPolicy> policies = new ArrayList<>(rawPolicies.size());
    for (int index = 0; index < rawPolicies.size(); index++) {
      final String policyField = field + ".policies[" + index + "]";
      final Map<String, Object> policy =
          exactObject(
              rawPolicies.get(index),
              policyField,
              keys("lane_id", "dataspace_id", "alias", "proof_scheme"));
      policies.add(
          new DaModels.ProofPolicy(
              u32(policy.get("lane_id"), policyField + ".lane_id"),
              u64(policy.get("dataspace_id"), policyField + ".dataspace_id"),
              exactNonBlankString(policy.get("alias"), policyField + ".alias"),
              DaModels.ProofScheme.fromJson(
                  policy.get("proof_scheme"), policyField + ".proof_scheme")));
    }
    return new DaModels.ProofPolicyBundle(version, policyHash, policies);
  }

  static DaModels.CommitmentListResponse parseCommitmentList(
      final Object value, final String field) {
    final Map<String, Object> object =
        exactObject(value, field, keys("policies", "commitments", "next_cursor"));
    final List<Object> raw =
        list(object.get("commitments"), field + ".commitments");
    final List<DaModels.CommitmentWithLocation> commitments =
        new ArrayList<>(raw.size());
    for (int index = 0; index < raw.size(); index++) {
      commitments.add(
          parseCommitmentWithLocation(
              raw.get(index), field + ".commitments[" + index + "]"));
    }
    return new DaModels.CommitmentListResponse(
        parsePolicyBundle(object.get("policies"), field + ".policies"),
        commitments,
        object.get("next_cursor") == null
            ? null
            : parseCommitmentListCursor(
                object.get("next_cursor"), field + ".next_cursor"));
  }

  static DaModels.CommitmentProofResponse parseCommitmentProofResponse(
      final Object value, final String field) {
    if (value == null) {
      return null;
    }
    final Map<String, Object> object =
        exactObject(value, field, keys("policies", "proof"));
    return new DaModels.CommitmentProofResponse(
        parsePolicyBundle(object.get("policies"), field + ".policies"),
        parseCommitmentProof(object.get("proof"), field + ".proof"));
  }

  static DaModels.CommitmentProof parseCommitmentProof(
      final Object value, final String field) {
    final Map<String, Object> object =
        exactObject(
            value,
            field,
            keys(
                "commitment",
                "location",
                "bundle_hash",
                "bundle_len",
                "root",
                "path"));
    return new DaModels.CommitmentProof(
        parseCommitmentRecord(object.get("commitment"), field + ".commitment"),
        parseLocation(object.get("location"), field + ".location"),
        string(object.get("bundle_hash"), field + ".bundle_hash"),
        u32(object.get("bundle_len"), field + ".bundle_len"),
        string(object.get("root"), field + ".root"),
        parsePath(object.get("path"), field + ".path"));
  }

  static DaModels.PinIntentListResponse parsePinIntentList(
      final Object value, final String field) {
    final Map<String, Object> object =
        exactObject(value, field, keys("intents", "next_cursor"));
    final List<Object> raw = list(object.get("intents"), field + ".intents");
    final List<DaModels.PinIntentWithLocation> intents = new ArrayList<>(raw.size());
    for (int index = 0; index < raw.size(); index++) {
      intents.add(
          parsePinIntentWithLocation(
              raw.get(index), field + ".intents[" + index + "]"));
    }
    return new DaModels.PinIntentListResponse(
        intents,
        object.get("next_cursor") == null
            ? null
            : parsePinIntentListCursor(
                object.get("next_cursor"), field + ".next_cursor"));
  }

  static DaModels.PinIntentProof parsePinIntentProof(
      final Object value, final String field) {
    if (value == null) {
      return null;
    }
    final Map<String, Object> object =
        exactObject(
            value,
            field,
            keys("intent", "location", "bundle_hash", "bundle_len", "root", "path"));
    return new DaModels.PinIntentProof(
        parsePinIntent(object.get("intent"), field + ".intent"),
        parseLocation(object.get("location"), field + ".location"),
        string(object.get("bundle_hash"), field + ".bundle_hash"),
        u32(object.get("bundle_len"), field + ".bundle_len"),
        string(object.get("root"), field + ".root"),
        parsePath(object.get("path"), field + ".path"));
  }

  static DaModels.VerifyResponse parseVerifyResponse(
      final Object value, final String field) {
    final Map<String, Object> object =
        exactObject(value, field, keys("valid", "error"));
    final Object rawValid = object.get("valid");
    if (!(rawValid instanceof Boolean)) {
      throw new IllegalArgumentException(field + ".valid must be a boolean");
    }
    final String error =
        object.get("error") == null
            ? null
            : string(object.get("error"), field + ".error");
    if (error != null && error.isEmpty()) {
      throw new IllegalArgumentException(field + ".error must be non-empty");
    }
    return new DaModels.VerifyResponse(((Boolean) rawValid).booleanValue(), error);
  }

  private static DaModels.CommitmentRecord parseCommitmentRecord(
      final Object value, final String field) {
    final Map<String, Object> record =
        exactObject(
            value,
            field,
            keys(
                "lane_id",
                "epoch",
                "sequence",
                "client_blob_id",
                "manifest_hash",
                "proof_scheme",
                "chunk_root",
                "proof_digest",
                "retention_class",
                "storage_ticket",
                "acknowledgement_sig"));
    return new DaModels.CommitmentRecord(
        u32(record.get("lane_id"), field + ".lane_id"),
        u64(record.get("epoch"), field + ".epoch"),
        u64(record.get("sequence"), field + ".sequence"),
        DaModels.Digest32.fromJson(
            record.get("client_blob_id"), field + ".client_blob_id"),
        DaModels.Digest32.fromJson(
            record.get("manifest_hash"), field + ".manifest_hash"),
        DaModels.ProofScheme.fromJson(
            record.get("proof_scheme"), field + ".proof_scheme"),
        string(record.get("chunk_root"), field + ".chunk_root"),
        record.get("proof_digest") == null
            ? null
            : string(record.get("proof_digest"), field + ".proof_digest"),
        parseRetention(record.get("retention_class"), field + ".retention_class"),
        DaModels.Digest32.fromJson(
            record.get("storage_ticket"), field + ".storage_ticket"),
        string(
            record.get("acknowledgement_sig"),
            field + ".acknowledgement_sig"));
  }

  private static DaModels.PinIntent parsePinIntent(
      final Object value, final String field) {
    final Map<String, Object> intent =
        exactObject(
            value,
            field,
            keys(
                "lane_id",
                "epoch",
                "sequence",
                "storage_ticket",
                "manifest_hash",
                "alias",
                "owner"));
    return new DaModels.PinIntent(
        u32(intent.get("lane_id"), field + ".lane_id"),
        u64(intent.get("epoch"), field + ".epoch"),
        u64(intent.get("sequence"), field + ".sequence"),
        DaModels.Digest32.fromJson(
            intent.get("storage_ticket"), field + ".storage_ticket"),
        DaModels.Digest32.fromJson(
            intent.get("manifest_hash"), field + ".manifest_hash"),
        intent.get("alias") == null
            ? null
            : string(intent.get("alias"), field + ".alias"),
        intent.get("owner") == null
            ? null
            : AccountIdLiteral.requireCanonicalI105Address(
                exactNonBlankString(intent.get("owner"), field + ".owner"),
                field + ".owner"));
  }

  static void requireHash(final String value, final String field) {
    final boolean canonicalShape =
        value != null && HASH_PATTERN.matcher(value).matches();
    final String body = canonicalShape ? value.substring(5, 69) : "";
    final int checksum =
        canonicalShape ? Integer.parseInt(value.substring(70, 74), 16) : -1;
    final boolean marked =
        canonicalShape
            && (Integer.parseInt(body.substring(62, 64), 16) & 1) == 1;
    if (!canonicalShape || !marked || checksum != hashChecksum(body)) {
      throw new IllegalArgumentException(
          field + " must be a canonical checksummed Iroha hash");
    }
  }

  static String taggedUnit(
      final Object value,
      final String field,
      final String discriminator,
      final Set<String> allowed) {
    final Map<String, Object> object =
        exactObject(value, field, keys(discriminator, "value"));
    final String type =
        string(object.get(discriminator), field + "." + discriminator);
    if (!allowed.contains(type)) {
      throw new IllegalArgumentException(
          field + "." + discriminator + " is unsupported");
    }
    if (object.get("value") != null) {
      throw new IllegalArgumentException(field + ".value must be null");
    }
    return type;
  }

  static List<Object> list(final Object value, final String field) {
    if (!(value instanceof List<?>)) {
      throw new IllegalArgumentException(field + " must be an array");
    }
    @SuppressWarnings("unchecked")
    final List<Object> list = (List<Object>) value;
    return list;
  }

  static int u8(final Object value, final String field) {
    final BigInteger integer = integer(value, field);
    if (integer.signum() < 0 || integer.compareTo(BigInteger.valueOf(255)) > 0) {
      throw new IllegalArgumentException(field + " must fit u8");
    }
    return integer.intValue();
  }

  private static DaModels.CommitmentWithLocation parseCommitmentWithLocation(
      final Object value, final String field) {
    final Map<String, Object> object =
        exactObject(value, field, keys("commitment", "location"));
    return new DaModels.CommitmentWithLocation(
        parseCommitmentRecord(object.get("commitment"), field + ".commitment"),
        parseLocation(object.get("location"), field + ".location"));
  }

  private static DaModels.PinIntentWithLocation parsePinIntentWithLocation(
      final Object value, final String field) {
    final Map<String, Object> object =
        exactObject(value, field, keys("intent", "location"));
    return new DaModels.PinIntentWithLocation(
        parsePinIntent(object.get("intent"), field + ".intent"),
        parseLocation(object.get("location"), field + ".location"));
  }

  private static DaModels.ListSnapshot parseListSnapshot(
      final Object value, final String field) {
    final Map<String, Object> object =
        exactObject(value, field, keys("block_height", "block_hash"));
    return new DaModels.ListSnapshot(
        u64(object.get("block_height"), field + ".block_height"),
        object.get("block_hash") == null
            ? null
            : string(object.get("block_hash"), field + ".block_hash"));
  }

  private static DaModels.CommitmentKey parseCommitmentKey(
      final Object value, final String field) {
    final Map<String, Object> object =
        exactObject(value, field, keys("lane_id", "epoch", "sequence"));
    return new DaModels.CommitmentKey(
        u32(object.get("lane_id"), field + ".lane_id"),
        u64(object.get("epoch"), field + ".epoch"),
        u64(object.get("sequence"), field + ".sequence"));
  }

  private static DaModels.CommitmentListCursor parseCommitmentListCursor(
      final Object value, final String field) {
    final Map<String, Object> object =
        exactObject(value, field, keys("snapshot", "after"));
    return new DaModels.CommitmentListCursor(
        parseListSnapshot(object.get("snapshot"), field + ".snapshot"),
        parseCommitmentKey(object.get("after"), field + ".after"));
  }

  private static DaModels.PinIntentListCursor parsePinIntentListCursor(
      final Object value, final String field) {
    final Map<String, Object> object =
        exactObject(value, field, keys("snapshot", "after"));
    return new DaModels.PinIntentListCursor(
        parseListSnapshot(object.get("snapshot"), field + ".snapshot"),
        parseLocation(object.get("after"), field + ".after"));
  }

  private static DaModels.Location parseLocation(final Object value, final String field) {
    final Map<String, Object> object =
        exactObject(value, field, keys("block_height", "index_in_bundle"));
    return new DaModels.Location(
        u64(object.get("block_height"), field + ".block_height"),
        u32(object.get("index_in_bundle"), field + ".index_in_bundle"));
  }

  private static List<DaModels.MerklePathItem> parsePath(
      final Object value, final String field) {
    final List<Object> raw = list(value, field);
    if (raw.size() > 32) {
      throw new IllegalArgumentException(field + " exceeds the u32 bundle depth");
    }
    final List<DaModels.MerklePathItem> path = new ArrayList<>(raw.size());
    for (int index = 0; index < raw.size(); index++) {
      final String itemField = field + "[" + index + "]";
      final Map<String, Object> item =
          exactObject(raw.get(index), itemField, keys("sibling", "direction"));
      final String sibling = string(item.get("sibling"), itemField + ".sibling");
      requireHash(sibling, itemField + ".sibling");
      final String direction =
          taggedUnit(
              item.get("direction"),
              itemField + ".direction",
              "direction",
              keys("Left", "Right"));
      path.add(
          new DaModels.MerklePathItem(
              sibling,
              "Left".equals(direction)
                  ? DaModels.MerkleDirection.LEFT
                  : DaModels.MerkleDirection.RIGHT));
    }
    return Collections.unmodifiableList(path);
  }

  private static DaModels.RetentionPolicy parseRetention(
      final Object value, final String field) {
    final Map<String, Object> policy =
        exactObject(
            value,
            field,
            keys(
                "hot_retention_secs",
                "cold_retention_secs",
                "required_replicas",
                "storage_class",
                "governance_tag"));
    final List<Object> tag = list(policy.get("governance_tag"), field + ".governance_tag");
    if (tag.size() != 1) {
      throw new IllegalArgumentException(
          field + ".governance_tag must contain one item");
    }
    return new DaModels.RetentionPolicy(
        u64(policy.get("hot_retention_secs"), field + ".hot_retention_secs"),
        u64(policy.get("cold_retention_secs"), field + ".cold_retention_secs"),
        u16(policy.get("required_replicas"), field + ".required_replicas"),
        DaModels.StorageClass.fromJson(
            policy.get("storage_class"), field + ".storage_class"),
        string(tag.get(0), field + ".governance_tag[0]"));
  }

  private static Map<String, Object> exactObject(
      final Object value, final String field, final Set<String> keys) {
    final Map<String, Object> object = objectMap(value, field);
    if (!object.keySet().equals(keys)) {
      throw new IllegalArgumentException(field + " contains unknown or missing fields");
    }
    return object;
  }

  private static Map<String, Object> objectMap(
      final Object value, final String field) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalArgumentException(field + " must be an object");
    }
    final Map<?, ?> raw = (Map<?, ?>) value;
    final Map<String, Object> checked = new LinkedHashMap<>();
    for (final Map.Entry<?, ?> entry : raw.entrySet()) {
      if (!(entry.getKey() instanceof String)) {
        throw new IllegalArgumentException(field + " object keys must be strings");
      }
      checked.put((String) entry.getKey(), entry.getValue());
    }
    return checked;
  }

  private static String string(final Object value, final String field) {
    if (!(value instanceof String)) {
      throw new IllegalArgumentException(field + " must be a string");
    }
    return (String) value;
  }

  private static String exactNonBlankString(final Object value, final String field) {
    final String text = string(value, field);
    if (text.trim().isEmpty() || !text.equals(text.trim())) {
      throw new IllegalArgumentException(field + " must be exact and non-blank");
    }
    return text;
  }

  private static BigInteger integer(final Object value, final String field) {
    if (value instanceof BigInteger) {
      return (BigInteger) value;
    }
    if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      return BigInteger.valueOf(((Number) value).longValue());
    }
    throw new IllegalArgumentException(field + " must be an integer");
  }

  private static int u16(final Object value, final String field) {
    final BigInteger integer = integer(value, field);
    if (integer.signum() < 0 || integer.compareTo(BigInteger.valueOf(65535)) > 0) {
      throw new IllegalArgumentException(field + " must fit u16");
    }
    return integer.intValue();
  }

  private static long u32(final Object value, final String field) {
    final BigInteger integer = integer(value, field);
    DaModels.requireU32(integer, field);
    return integer.longValue();
  }

  private static BigInteger u64(final Object value, final String field) {
    final BigInteger integer = integer(value, field);
    DaModels.requireU64(integer, field);
    return integer;
  }

  private static void append(final Object value, final StringBuilder output) {
    if (value == null) {
      output.append("null");
    } else if (value instanceof String) {
      appendString((String) value, output);
    } else if (value instanceof Boolean
        || value instanceof BigInteger
        || value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      output.append(value);
    } else if (value instanceof List<?>) {
      output.append('[');
      final List<?> items = (List<?>) value;
      for (int index = 0; index < items.size(); index++) {
        if (index != 0) {
          output.append(',');
        }
        append(items.get(index), output);
      }
      output.append(']');
    } else if (value instanceof Map<?, ?>) {
      final Map<?, ?> map = (Map<?, ?>) value;
      final List<String> keys = new ArrayList<>(map.size());
      for (final Object key : map.keySet()) {
        if (!(key instanceof String)) {
          throw new IllegalArgumentException("JSON object key must be a string");
        }
        keys.add((String) key);
      }
      Collections.sort(keys);
      output.append('{');
      for (int index = 0; index < keys.size(); index++) {
        if (index != 0) {
          output.append(',');
        }
        final String key = keys.get(index);
        appendString(key, output);
        output.append(':');
        append(map.get(key), output);
      }
      output.append('}');
    } else {
      throw new IllegalArgumentException(
          "unsupported JSON value " + value.getClass().getName());
    }
  }

  private static void appendString(final String value, final StringBuilder output) {
    output.append('"');
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      switch (character) {
        case '"':
          output.append("\\\"");
          break;
        case '\\':
          output.append("\\\\");
          break;
        case '\b':
          output.append("\\b");
          break;
        case '\f':
          output.append("\\f");
          break;
        case '\n':
          output.append("\\n");
          break;
        case '\r':
          output.append("\\r");
          break;
        case '\t':
          output.append("\\t");
          break;
        default:
          if (character < 0x20) {
            output.append("\\u");
            final String hex = Integer.toHexString(character);
            for (int pad = hex.length(); pad < 4; pad++) {
              output.append('0');
            }
            output.append(hex);
          } else {
            output.append(character);
          }
      }
    }
    output.append('"');
  }

  private static int hashChecksum(final String body) {
    int crc = 0xffff;
    final byte[] bytes = ("hash:" + body).getBytes(StandardCharsets.US_ASCII);
    for (final byte value : bytes) {
      crc ^= (value & 0xff) << 8;
      for (int bit = 0; bit < 8; bit++) {
        crc =
            (crc & 0x8000) != 0
                ? ((crc << 1) ^ 0x1021) & 0xffff
                : (crc << 1) & 0xffff;
      }
    }
    return crc;
  }

  private static Set<String> keys(final String... values) {
    return new LinkedHashSet<>(Arrays.asList(values));
  }
}
