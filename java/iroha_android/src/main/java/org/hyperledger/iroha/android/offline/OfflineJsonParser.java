package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

public final class OfflineJsonParser {

  private OfflineJsonParser() {}

  public static OfflineV2Readiness parseOfflineV2Readiness(final byte[] payload) {
    final Object root = parse(payload);
    final Map<String, Object> object = expectObject(root, "root");
    return new OfflineV2Readiness(
        asBoolean(object.get("offline_note_v2"), "offline_note_v2"),
        asBoolean(object.get("offline_one_use_keys"), "offline_one_use_keys"),
        asBoolean(object.get("offline_recursive_note_proof"), "offline_recursive_note_proof"),
        asBoolean(object.get("offline_fountain_qr_v1"), "offline_fountain_qr_v1"),
        asBoolean(object.get("offline_sync_optional"), "offline_sync_optional"),
        asBoolean(object.get("offline_telemetry"), "offline_telemetry"));
  }

  public static String canonicalJson(final byte[] payload) {
    return JsonEncoder.encode(parse(payload));
  }

  private static Object parse(final byte[] payload) {
    final String json = new String(payload, StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      throw new IllegalStateException("Empty JSON payload");
    }
    return JsonParser.parse(json);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> expectObject(final Object value, final String path) {
    if (!(value instanceof Map<?, ?> map)) {
      throw new IllegalStateException(path + " is not a JSON object");
    }
    return (Map<String, Object>) map;
  }

  private static boolean asBoolean(final Object value, final String path) {
    if (!(value instanceof Boolean bool)) {
      throw new IllegalStateException(path + " must be a boolean");
    }
    return bool.booleanValue();
  }
}
