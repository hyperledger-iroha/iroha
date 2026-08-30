package org.hyperledger.iroha.android.client;

import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyBackendTag;

/** Strict parser for the bounded {@code ids_only=true} verifying-key projection. */
final class VerifyingKeyJsonParser {
  private static final int MAX_IDS = 1_000;
  private static final Set<String> ID_FIELDS = Set.of("backend", "name");

  private VerifyingKeyJsonParser() {}

  static List<VerifyingKeyId> parseActiveIds(final byte[] body) {
    final String json;
    try {
      json =
          StandardCharsets.UTF_8
              .newDecoder()
              .onMalformedInput(CodingErrorAction.REPORT)
              .onUnmappableCharacter(CodingErrorAction.REPORT)
              .decode(ByteBuffer.wrap(body))
              .toString();
    } catch (final CharacterCodingException error) {
      throw new IllegalStateException(
          "active verifying-key response must be exact UTF-8", error);
    }
    final Object root = JsonParser.parse(json);
    if (!(root instanceof List<?>)) {
      throw new IllegalStateException("active verifying-key response must be a JSON array");
    }
    final List<?> rows = (List<?>) root;
    if (rows.size() > MAX_IDS) {
      throw new IllegalStateException("active verifying-key response exceeds 1000 ids");
    }
    final List<VerifyingKeyId> ids = new ArrayList<>(rows.size());
    final Set<VerifyingKeyId> unique = new HashSet<>();
    VerifyingKeyId previous = null;
    for (int index = 0; index < rows.size(); index++) {
      final Object row = rows.get(index);
      if (!(row instanceof Map<?, ?>)) {
        throw new IllegalStateException("active verifying-key id[" + index + "] must be an object");
      }
      final Map<?, ?> fields = (Map<?, ?>) row;
      if (!fields.keySet().equals(ID_FIELDS)) {
        throw new IllegalStateException(
            "active verifying-key id[" + index + "] must contain only backend and name");
      }
      final Object backendValue = fields.get("backend");
      final Object nameValue = fields.get("name");
      if (!(backendValue instanceof String) || !(nameValue instanceof String)) {
        throw new IllegalStateException(
            "active verifying-key id[" + index + "] fields must be strings");
      }
      final String backend = (String) backendValue;
      final String name = (String) nameValue;
      try {
        VerifyingKeyBackendTag.requireVerifierBackendRegistryLabelV1(
            backend, "active verifying-key backend");
      } catch (final IllegalArgumentException error) {
        throw new IllegalStateException(error.getMessage(), error);
      }
      if (!HttpClientTransport.isPortableVerifyingKeyIdField(name)) {
        throw new IllegalStateException(
            "active verifying-key name must use the bounded portable registry grammar");
      }
      final VerifyingKeyId id = new VerifyingKeyId(backend, name);
      if (!unique.add(id)) {
        throw new IllegalStateException("active verifying-key response contains duplicate id " + id);
      }
      if (previous != null
          && (previous.name().compareTo(id.name()) > 0
              || (previous.name().equals(id.name())
                  && previous.backend().compareTo(id.backend()) > 0))) {
        throw new IllegalStateException(
            "active verifying-key response is not in requested ascending order");
      }
      ids.add(id);
      previous = id;
    }
    return Collections.unmodifiableList(ids);
  }
}
