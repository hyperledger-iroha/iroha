package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Full on-chain Kotodama V1 contract manifest returned by Torii. */
public final class ContractManifest {
  /** Branded Kotodama V1 entrypoint categories. */
  public enum EntrypointKind {
    KOTOAGE,
    VIEW,
    HAJIMARI,
    KAIZEN
  }

  /** Scalar and pointer leaves supported by the V1 boundary ABI. */
  public enum ValueKindV1 {
    INT,
    U128,
    BOOL,
    STRING,
    AMOUNT,
    JSON,
    NAME,
    ACCOUNT_ID,
    ASSET_DEFINITION_ID,
    ASSET_ID,
    DOMAIN_ID,
    NFT_ID,
    DATA_SPACE_ID,
    BLOB
  }

  /** Tree-node categories encoded on a flat preorder V1 boundary-schema tape. */
  public enum ValueTypeNodeKindV1 {
    STRUCT,
    TUPLE,
    OPTION,
    RESULT,
    LIST,
    LEAF
  }

  /** Named product metadata for a flat preorder schema node. */
  public static final class StructTypeNodeV1 {
    private final String name;
    private final List<String> fields;

    StructTypeNodeV1(final String name, final List<String> fields) {
      this.name = name;
      this.fields = immutableList(fields);
    }

    public String name() {
      return name;
    }

    public List<String> fields() {
      return fields;
    }
  }

  /** Bounded-list metadata whose element subtree immediately follows in the enclosing node tape. */
  public static final class ListTypeNodeV1 {
    private final int capacity;

    ListTypeNodeV1(final int capacity) {
      this.capacity = capacity;
    }

    public int capacity() {
      return capacity;
    }
  }

  /** One validated preorder node in an exact Kotodama V1 boundary schema. */
  public static final class ValueTypeNodeV1 {
    private final ValueTypeNodeKindV1 kind;
    private final StructTypeNodeV1 structValue;
    private final Integer tupleArity;
    private final ListTypeNodeV1 listValue;
    private final ValueKindV1 leafKind;

    ValueTypeNodeV1(
        final ValueTypeNodeKindV1 kind,
        final StructTypeNodeV1 structValue,
        final Integer tupleArity,
        final ListTypeNodeV1 listValue,
        final ValueKindV1 leafKind) {
      this.kind = kind;
      this.structValue = structValue;
      this.tupleArity = tupleArity;
      this.listValue = listValue;
      this.leafKind = leafKind;
    }

    public ValueTypeNodeKindV1 kind() {
      return kind;
    }

    public StructTypeNodeV1 structValue() {
      return structValue;
    }

    public Integer tupleArity() {
      return tupleArity;
    }

    public ListTypeNodeV1 listValue() {
      return listValue;
    }

    public ValueKindV1 leafKind() {
      return leafKind;
    }
  }

  /** Exact flat preorder value schema used at a Kotodama V1 public boundary. */
  public static final class ValueTypeV1 {
    private final List<ValueTypeNodeV1> nodes;
    private final int wordCount;
    private final String canonicalTypeName;

    ValueTypeV1(
        final List<ValueTypeNodeV1> nodes,
        final int wordCount,
        final String canonicalTypeName) {
      this.nodes = immutableList(nodes);
      this.wordCount = wordCount;
      this.canonicalTypeName = canonicalTypeName;
    }

    public List<ValueTypeNodeV1> nodes() {
      return nodes;
    }

    public int wordCount() {
      return wordCount;
    }

    public String canonicalTypeName() {
      return canonicalTypeName;
    }
  }

  /** One named field in a canonical V1 argument record. */
  public static final class ArgumentFieldV1 {
    private final String name;
    private final ValueTypeV1 valueType;

    ArgumentFieldV1(final String name, final ValueTypeV1 valueType) {
      this.name = name;
      this.valueType = valueType;
    }

    public String name() {
      return name;
    }

    public ValueTypeV1 valueType() {
      return valueType;
    }
  }

  /** Exact canonical V1 schema for one public argument record. */
  public static final class ArgumentSchemaV1 {
    private final List<ArgumentFieldV1> fields;
    private final int wordCount;

    ArgumentSchemaV1(final List<ArgumentFieldV1> fields, final int wordCount) {
      this.fields = immutableList(fields);
      this.wordCount = wordCount;
    }

    public List<ArgumentFieldV1> fields() {
      return fields;
    }

    public int wordCount() {
      return wordCount;
    }
  }

  /** One public parameter advertised by the compiler. */
  public static final class EntrypointParameter {
    private final String name;
    private final String typeName;

    EntrypointParameter(final String name, final String typeName) {
      this.name = name;
      this.typeName = typeName;
    }

    public String name() {
      return name;
    }

    public String typeName() {
      return typeName;
    }
  }

  /** One bounded dynamic state access advertised by the compiler. */
  public static final class DynamicAccessHint {
    private final String baseKey;
    private final String keyType;
    private final String boundKind;
    private final long maxKeys;

    DynamicAccessHint(
        final String baseKey, final String keyType, final String boundKind, final long maxKeys) {
      this.baseKey = baseKey;
      this.keyType = keyType;
      this.boundKind = boundKind;
      this.maxKeys = maxKeys;
    }

    public String baseKey() {
      return baseKey;
    }

    public String keyType() {
      return keyType;
    }

    public String boundKind() {
      return boundKind;
    }

    public long maxKeys() {
      return maxKeys;
    }
  }

  /** Static and bounded-dynamic scheduler hints. */
  public static final class AccessSetHints {
    private final List<String> readKeys;
    private final List<String> writeKeys;
    private final List<DynamicAccessHint> dynamicReads;
    private final List<DynamicAccessHint> dynamicWrites;

    AccessSetHints(
        final List<String> readKeys,
        final List<String> writeKeys,
        final List<DynamicAccessHint> dynamicReads,
        final List<DynamicAccessHint> dynamicWrites) {
      this.readKeys = immutableList(readKeys);
      this.writeKeys = immutableList(writeKeys);
      this.dynamicReads = immutableList(dynamicReads);
      this.dynamicWrites = immutableList(dynamicWrites);
    }

    public List<String> readKeys() {
      return readKeys;
    }

    public List<String> writeKeys() {
      return writeKeys;
    }

    public List<DynamicAccessHint> dynamicReads() {
      return dynamicReads;
    }

    public List<DynamicAccessHint> dynamicWrites() {
      return dynamicWrites;
    }
  }

  /** Trigger repetition policy. */
  public enum TriggerRepeatsKind {
    INDEFINITELY,
    EXACTLY
  }

  /** Exact trigger repetition metadata. */
  public static final class TriggerRepeats {
    private final TriggerRepeatsKind kind;
    private final Long exactly;

    TriggerRepeats(final TriggerRepeatsKind kind, final Long exactly) {
      this.kind = kind;
      this.exactly = exactly;
    }

    public TriggerRepeatsKind kind() {
      return kind;
    }

    public Long exactly() {
      return exactly;
    }
  }

  /** Callback target for one manifest trigger. */
  public static final class TriggerCallback {
    private final String namespace;
    private final String entrypoint;

    TriggerCallback(final String namespace, final String entrypoint) {
      this.namespace = namespace;
      this.entrypoint = entrypoint;
    }

    public String namespace() {
      return namespace;
    }

    public String entrypoint() {
      return entrypoint;
    }
  }

  /** Complete trigger metadata attached to an entrypoint. */
  public static final class TriggerDescriptor {
    private final String id;
    private final TriggerRepeats repeats;
    private final String filterBase64;
    private final String authority;
    private final Map<String, Object> metadata;
    private final TriggerCallback callback;

    TriggerDescriptor(
        final String id,
        final TriggerRepeats repeats,
        final String filterBase64,
        final String authority,
        final Map<String, Object> metadata,
        final TriggerCallback callback) {
      this.id = id;
      this.repeats = repeats;
      this.filterBase64 = filterBase64;
      this.authority = authority;
      this.metadata = immutableJsonObject(metadata);
      this.callback = callback;
    }

    public String id() {
      return id;
    }

    public TriggerRepeats repeats() {
      return repeats;
    }

    public String filterBase64() {
      return filterBase64;
    }

    public String authority() {
      return authority;
    }

    public Map<String, Object> metadata() {
      return metadata;
    }

    public TriggerCallback callback() {
      return callback;
    }
  }

  /** Exact public interface metadata for one Kotodama entrypoint. */
  public static final class EntrypointDescriptor {
    private final String name;
    private final EntrypointKind kind;
    private final List<EntrypointParameter> parameters;
    private final ArgumentSchemaV1 argumentSchema;
    private final String returnType;
    private final ValueTypeV1 returnSchema;
    private final String permission;
    private final List<String> readKeys;
    private final List<String> writeKeys;
    private final Boolean accessHintsComplete;
    private final List<String> accessHintsSkipped;
    private final List<TriggerDescriptor> triggers;

    EntrypointDescriptor(
        final String name,
        final EntrypointKind kind,
        final List<EntrypointParameter> parameters,
        final ArgumentSchemaV1 argumentSchema,
        final String returnType,
        final ValueTypeV1 returnSchema,
        final String permission,
        final List<String> readKeys,
        final List<String> writeKeys,
        final Boolean accessHintsComplete,
        final List<String> accessHintsSkipped,
        final List<TriggerDescriptor> triggers) {
      this.name = name;
      this.kind = kind;
      this.parameters = immutableList(parameters);
      this.argumentSchema = argumentSchema;
      this.returnType = returnType;
      this.returnSchema = returnSchema;
      this.permission = permission;
      this.readKeys = immutableList(readKeys);
      this.writeKeys = immutableList(writeKeys);
      this.accessHintsComplete = accessHintsComplete;
      this.accessHintsSkipped = immutableList(accessHintsSkipped);
      this.triggers = immutableList(triggers);
    }

    public String name() {
      return name;
    }

    public EntrypointKind kind() {
      return kind;
    }

    public List<EntrypointParameter> parameters() {
      return parameters;
    }

    public ArgumentSchemaV1 argumentSchema() {
      return argumentSchema;
    }

    public String returnType() {
      return returnType;
    }

    public ValueTypeV1 returnSchema() {
      return returnSchema;
    }

    public String permission() {
      return permission;
    }

    public List<String> readKeys() {
      return readKeys;
    }

    public List<String> writeKeys() {
      return writeKeys;
    }

    public Boolean accessHintsComplete() {
      return accessHintsComplete;
    }

    public List<String> accessHintsSkipped() {
      return accessHintsSkipped;
    }

    public List<TriggerDescriptor> triggers() {
      return triggers;
    }
  }

  /** One durable state slot advertised by a Kotodama seiyaku. */
  public static final class StateDescriptor {
    private final String name;
    private final String typeName;

    StateDescriptor(final String name, final String typeName) {
      this.name = name;
      this.typeName = typeName;
    }

    public String name() {
      return name;
    }

    public String typeName() {
      return typeName;
    }
  }

  /** One stable application error code. */
  public static final class ErrorCodeDescriptor {
    private final String namespace;
    private final String name;
    private final long code;

    ErrorCodeDescriptor(final String namespace, final String name, final long code) {
      this.namespace = namespace;
      this.name = name;
      this.code = code;
    }

    public String namespace() {
      return namespace;
    }

    public String name() {
      return name;
    }

    public long code() {
      return code;
    }
  }

  /** One localized text in a `kotoba` table. */
  public static final class KotobaTranslation {
    private final String language;
    private final String text;

    KotobaTranslation(final String language, final String text) {
      this.language = language;
      this.text = text;
    }

    public String language() {
      return language;
    }

    public String text() {
      return text;
    }
  }

  /** One stable message id and all of its localized texts. */
  public static final class KotobaTranslationEntry {
    private final String messageId;
    private final List<KotobaTranslation> translations;

    KotobaTranslationEntry(
        final String messageId, final List<KotobaTranslation> translations) {
      this.messageId = messageId;
      this.translations = immutableList(translations);
    }

    public String messageId() {
      return messageId;
    }

    public List<KotobaTranslation> translations() {
      return translations;
    }
  }

  /** Signature metadata binding the manifest to an approved signer. */
  public static final class Provenance {
    private final String signer;
    private final String signature;

    Provenance(final String signer, final String signature) {
      this.signer = signer;
      this.signature = signature;
    }

    public String signer() {
      return signer;
    }

    public String signature() {
      return signature;
    }
  }

  private final String seiyakuName;
  private final String codeHashHex;
  private final String abiHashHex;
  private final String compilerFingerprint;
  private final BigInteger featuresBitmap;
  private final AccessSetHints accessSetHints;
  private final List<EntrypointDescriptor> entrypoints;
  private final List<StateDescriptor> states;
  private final List<ErrorCodeDescriptor> errorCodes;
  private final List<KotobaTranslationEntry> kotoba;
  private final Provenance provenance;

  ContractManifest(
      final String seiyakuName,
      final String codeHashHex,
      final String abiHashHex,
      final String compilerFingerprint,
      final BigInteger featuresBitmap,
      final AccessSetHints accessSetHints,
      final List<EntrypointDescriptor> entrypoints,
      final List<StateDescriptor> states,
      final List<ErrorCodeDescriptor> errorCodes,
      final List<KotobaTranslationEntry> kotoba,
      final Provenance provenance) {
    this.seiyakuName = seiyakuName;
    this.codeHashHex = codeHashHex;
    this.abiHashHex = abiHashHex;
    this.compilerFingerprint = compilerFingerprint;
    this.featuresBitmap = featuresBitmap;
    this.accessSetHints = accessSetHints;
    this.entrypoints = immutableNullableList(entrypoints);
    this.states = immutableNullableList(states);
    this.errorCodes = immutableNullableList(errorCodes);
    this.kotoba = immutableNullableList(kotoba);
    this.provenance = provenance;
  }

  public String seiyakuName() {
    return seiyakuName;
  }

  public String codeHashHex() {
    return codeHashHex;
  }

  public String abiHashHex() {
    return abiHashHex;
  }

  public String compilerFingerprint() {
    return compilerFingerprint;
  }

  public BigInteger featuresBitmap() {
    return featuresBitmap;
  }

  public AccessSetHints accessSetHints() {
    return accessSetHints;
  }

  public List<EntrypointDescriptor> entrypoints() {
    return entrypoints;
  }

  public List<StateDescriptor> states() {
    return states;
  }

  public List<ErrorCodeDescriptor> errorCodes() {
    return errorCodes;
  }

  public List<KotobaTranslationEntry> kotoba() {
    return kotoba;
  }

  public Provenance provenance() {
    return provenance;
  }

  private static <T> List<T> immutableList(final List<T> source) {
    return Collections.unmodifiableList(new ArrayList<>(source));
  }

  private static <T> List<T> immutableNullableList(final List<T> source) {
    return source == null ? null : immutableList(source);
  }

  @SuppressWarnings("unchecked")
  private static Object immutableJsonValue(final Object value) {
    if (value instanceof Map<?, ?>) {
      return immutableJsonObject((Map<String, Object>) value);
    }
    if (value instanceof List<?>) {
      final List<Object> copy = new ArrayList<>();
      for (final Object item : (List<?>) value) {
        copy.add(immutableJsonValue(item));
      }
      return Collections.unmodifiableList(copy);
    }
    return value;
  }

  private static Map<String, Object> immutableJsonObject(final Map<String, Object> source) {
    final Map<String, Object> copy = new LinkedHashMap<>();
    for (final Map.Entry<String, Object> entry : source.entrySet()) {
      copy.put(entry.getKey(), immutableJsonValue(entry.getValue()));
    }
    return Collections.unmodifiableMap(copy);
  }
}
