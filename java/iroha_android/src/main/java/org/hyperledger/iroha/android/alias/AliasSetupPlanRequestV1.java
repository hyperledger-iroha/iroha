package org.hyperledger.iroha.android.alias;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Canonical signed request body for one indivisible alias setup plan. */
public final class AliasSetupPlanRequestV1 extends AliasJsonValue {
  /** Current planner request layout. */
  public static final int VERSION = 1;

  private final int schemaVersion;
  private final List<EnsureAlias> intents;

  /** Constructs a current-version request from exact EnsureAlias intents. */
  public AliasSetupPlanRequestV1(final List<EnsureAlias> intents) {
    this(VERSION, intents);
  }

  /** Constructs an explicitly versioned request. */
  public AliasSetupPlanRequestV1(final int schemaVersion, final List<EnsureAlias> intents) {
    if (schemaVersion != VERSION) {
      throw new IllegalArgumentException("schemaVersion must be " + VERSION);
    }
    if (intents == null || intents.isEmpty()) {
      throw new IllegalArgumentException("intents must not be empty");
    }
    final List<EnsureAlias> copy = new ArrayList<>(intents.size());
    final Set<String> resources = new HashSet<>();
    for (final EnsureAlias intent : intents) {
      if (intent == null) throw new IllegalArgumentException("intents must not contain null");
      final String resource = intent.intent().kind() + "\0" + intent.intent().resourceText();
      if (!resources.add(resource)) {
        throw new IllegalArgumentException(
            "intents must not contain the same resource more than once");
      }
      copy.add(intent);
    }
    this.schemaVersion = schemaVersion;
    this.intents = Collections.unmodifiableList(copy);
  }

  /** Returns the request schema version. */
  public int schemaVersion() {
    return schemaVersion;
  }

  /** Returns the exact setup intents. */
  public List<EnsureAlias> intents() {
    return intents;
  }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("schema_version", schemaVersion);
    final List<Map<String, Object>> values = new ArrayList<>(intents.size());
    for (final EnsureAlias intent : intents) values.add(intent.toJsonMap());
    map.put("intents", values);
    return map;
  }
}
