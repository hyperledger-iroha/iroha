package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Strict parser for Torii's complete Rust `ContractManifest` JSON shape. */
public final class ContractManifestJsonParser {
  private static final BigInteger MAX_U32 = BigInteger.ONE.shiftLeft(32).subtract(BigInteger.ONE);
  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private static final Pattern MANIFEST_HASH =
      Pattern.compile("^hash:([0-9A-F]{64})#([0-9A-F]{4})$");
  private static final Pattern CONVENIENCE_HASH = Pattern.compile("^[0-9a-f]{64}$");
  // BEGIN GENERATED: kotodama-v1-validator-policy
  private static final Set<String> RESERVED_IDENTIFIERS =
      set(
          "authorize",
          "break",
          "const",
          "continue",
          "else",
          "enum",
          "error",
          "false",
          "fn",
          "for",
          "hajimari",
          "始まり",
          "if",
          "in",
          "kaizen",
          "改善",
          "kotoage",
          "言挙げ",
          "let",
          "match",
          "module",
          "return",
          "seiyaku",
          "誓約",
          "state",
          "struct",
          "trigger",
          "true",
          "var",
          "view",
          "Amount");
  private static final Set<String> RESERVED_DECLARATION_NAMES =
      set(
          "int",
          "decimal",
          "quantity",
          "bool",
          "string",
          "bytes",
          "Json",
          "AccountId",
          "AssetDefinitionId",
          "AssetId",
          "DomainId",
          "Name",
          "NftId",
          "DataSpaceId",
          "Option",
          "Result",
          "List",
          "StateMap",
          "Secret",
          "AccountView",
          "AssetView",
          "AssetDefinitionView",
          "DomainView",
          "NftView",
          "QueryPage",
          "AxtDescriptor",
          "AssetHandle",
          "ProofBlob",
          "SoracloudRequest",
          "SoracloudResponse",
          "state_map_get",
          "__kotodama_list_len",
          "__kotodama_list_get",
          "__kotodama_list_try_set",
          "__kotodama_list_try_push",
          "__kotodama_list_pop",
          "__kotodama_list_contains",
          "__kotodama_list_take",
          "__kotodama_list_enumerate",
          "__kotodama_decimal_div_round",
          "__kotodama_quantity_div_round",
          "__kotodama_quantity_ratio_round",
          "__kotodama_decimal_to_int_trunc",
          "__kotodama_decimal_to_int_round",
          "is_some",
          "is_none",
          "is_ok",
          "is_err",
          "unwrap_or",
          "unwrap_err_or",
          "Amount");
  private static final Set<String> RETIRED_NUMERIC_TYPE_NAMES =
      set(
          "i8",
          "i16",
          "i32",
          "i64",
          "i128",
          "isize",
          "u8",
          "u16",
          "u32",
          "u64",
          "u128",
          "usize",
          "num",
          "Int",
          "Integer",
          "float",
          "f32",
          "f64",
          "Decimal",
          "Fixed",
          "FixedPoint",
          "Amount",
          "amount",
          "money",
          "Quantity",
          "number");
  private static final Set<String> STATE_MAP_KEY_TYPE_NAMES =
      set(
          "int",
          "decimal",
          "quantity",
          "bool",
          "string",
          "bytes",
          "DataSpaceId",
          "AccountId",
          "AssetDefinitionId",
          "AssetId",
          "NftId",
          "DomainId",
          "Name");
  private static final Set<String> DYNAMIC_ACCESS_BOUND_KINDS =
      set(
          "range",
          "take");
  private static final BigInteger MAX_DYNAMIC_ACCESS_KEYS = BigInteger.valueOf(64);
  // END GENERATED: kotodama-v1-validator-policy
  private static final Set<String> STATE_SCALAR_TYPE_NAMES =
      set(
          "int",
          "decimal",
          "quantity",
          "bool",
          "string",
          "bytes",
          "DataSpaceId",
          "AccountId",
          "AssetDefinitionId",
          "AssetId",
          "NftId",
          "DomainId",
          "Name",
          "Json");
  private static final int MAX_STATE_TYPE_DEPTH = 256;
  private static final int MAX_STATE_TYPE_NODES = 256;
  private static final Map<String, ContractManifest.ValueKindV1> VALUE_KINDS = valueKinds();

  private ContractManifestJsonParser() {}

  /** Parse and validate one complete contract-manifest route response. */
  public static ContractManifestRecord parseRecord(final byte[] payload) {
    final Map<String, Object> root =
        object(parse(payload, "contract manifest response"), "contract manifest response");
    exactKeys(root, set("manifest", "code_hash", "abi_hash"), "contract manifest response");
    final ContractManifest manifest =
        parseManifest(
            object(
                required(root, "manifest", "contract manifest response"),
                "contract manifest response.manifest"));
    final String codeHash =
        optionalConvenienceHash(root, "code_hash", "contract manifest response.code_hash");
    final String abiHash =
        optionalConvenienceHash(root, "abi_hash", "contract manifest response.abi_hash");
    check(
        equalsNullable(codeHash, manifest.codeHashHex()),
        "contract manifest response.code_hash must exactly match manifest.code_hash");
    check(
        equalsNullable(abiHash, manifest.abiHashHex()),
        "contract manifest response.abi_hash must exactly match manifest.abi_hash");
    return new ContractManifestRecord(manifest, codeHash, abiHash);
  }

  /** Parse and validate one complete Rust `ContractManifest` object. */
  public static ContractManifest parseManifest(final Map<String, Object> root) {
    exactKeys(
        root,
        set(
            "seiyaku_name",
            "code_hash",
            "abi_hash",
            "compiler_fingerprint",
            "features_bitmap",
            "access_set_hints",
            "entrypoints",
            "states",
            "error_codes",
            "kotoba",
            "provenance"),
        "manifest");
    final String seiyakuName = optionalExactString(root, "seiyaku_name", "manifest.seiyaku_name");
    if (seiyakuName != null) {
      check(
          canonicalTypeDeclarationIdentifier(seiyakuName),
          "manifest.seiyaku_name must be a canonical Kotodama identifier");
    }
    final String codeHash = optionalManifestHash(root, "code_hash", "manifest.code_hash");
    final String abiHash = optionalManifestHash(root, "abi_hash", "manifest.abi_hash");
    final String compilerFingerprint =
        optionalExactString(root, "compiler_fingerprint", "manifest.compiler_fingerprint");
    final BigInteger featuresBitmap =
        root.containsKey("features_bitmap") && root.get("features_bitmap") != null
            ? unsignedInteger(root.get("features_bitmap"), MAX_U64, "manifest.features_bitmap")
            : null;
    check(
        featuresBitmap == null || featuresBitmap.compareTo(BigInteger.valueOf(3)) <= 0,
        "manifest.features_bitmap contains unsupported Kotodama V1 bits");
    final ContractManifest.AccessSetHints accessSetHints =
        root.containsKey("access_set_hints") && root.get("access_set_hints") != null
            ? parseAccessSetHints(object(root.get("access_set_hints"), "manifest.access_set_hints"))
            : null;
    final List<ContractManifest.EntrypointDescriptor> entrypoints =
        optionalObjectList(root, "entrypoints", "manifest.entrypoints", ENTRYPOINT_PARSER);
    final List<ContractManifest.StateDescriptor> states =
        optionalObjectList(root, "states", "manifest.states", STATE_PARSER);
    final List<ContractManifest.ErrorCodeDescriptor> errorCodes =
        optionalObjectList(root, "error_codes", "manifest.error_codes", ERROR_CODE_PARSER);
    final List<ContractManifest.KotobaTranslationEntry> kotoba =
        optionalObjectList(root, "kotoba", "manifest.kotoba", KOTOBA_ENTRY_PARSER);
    final ContractManifest.Provenance provenance =
        root.containsKey("provenance") && root.get("provenance") != null
            ? parseProvenance(object(root.get("provenance"), "manifest.provenance"))
            : null;

    if (entrypoints != null) {
      final List<String> names = new ArrayList<>();
      int hajimariCount = 0;
      int kaizenCount = 0;
      for (final ContractManifest.EntrypointDescriptor descriptor : entrypoints) {
        names.add(descriptor.name());
        if (descriptor.kind() == ContractManifest.EntrypointKind.HAJIMARI) {
          hajimariCount++;
        } else if (descriptor.kind() == ContractManifest.EntrypointKind.KAIZEN) {
          kaizenCount++;
        }
      }
      unique(names, "manifest.entrypoints");
      check(hajimariCount <= 1, "manifest.entrypoints must not declare multiple hajimari entrypoints");
      check(kaizenCount <= 1, "manifest.entrypoints must not declare multiple kaizen entrypoints");
      final Map<String, ContractManifest.EntrypointKind> declared = new HashMap<>();
      final Set<String> triggerIds = new HashSet<>();
      for (final ContractManifest.EntrypointDescriptor descriptor : entrypoints) {
        declared.put(descriptor.name(), descriptor.kind());
      }
      for (final ContractManifest.EntrypointDescriptor descriptor : entrypoints) {
        for (final ContractManifest.TriggerDescriptor trigger : descriptor.triggers()) {
          check(triggerIds.add(trigger.id()), "manifest trigger ids must be globally unique");
          check(
              trigger.callback().namespace() != null
                  || declared.get(trigger.callback().entrypoint())
                      == ContractManifest.EntrypointKind.KOTOAGE,
              "manifest local trigger callback must name a declared kotoage entrypoint");
        }
      }
    }
    if (states != null) {
      final List<String> names = new ArrayList<>();
      for (final ContractManifest.StateDescriptor descriptor : states) {
        names.add(descriptor.name());
      }
      unique(names, "manifest.states");
    }
    validateDynamicAccessHintStateMaps(accessSetHints, states);
    if (errorCodes != null) {
      final List<String> paths = new ArrayList<>();
      final List<String> codes = new ArrayList<>();
      for (final ContractManifest.ErrorCodeDescriptor descriptor : errorCodes) {
        paths.add(descriptor.namespace() + "::" + descriptor.name());
        codes.add(Long.toString(descriptor.code()));
      }
      unique(paths, "manifest.error_codes");
      unique(codes, "manifest.error_codes.code");
    }
    if (kotoba != null) {
      final List<String> ids = new ArrayList<>();
      for (final ContractManifest.KotobaTranslationEntry entry : kotoba) {
        ids.add(entry.messageId());
      }
      unique(ids, "manifest.kotoba");
    }
    return new ContractManifest(
        seiyakuName,
        codeHash,
        abiHash,
        compilerFingerprint,
        featuresBitmap,
        accessSetHints,
        entrypoints,
        states,
        errorCodes,
        kotoba,
        provenance);
  }

  private static ContractManifest.AccessSetHints parseAccessSetHints(
      final Map<String, Object> root) {
    exactKeys(
        root,
        set("read_keys", "write_keys", "dynamic_reads", "dynamic_writes"),
        "manifest.access_set_hints");
    return new ContractManifest.AccessSetHints(
        stringList(
            required(root, "read_keys", "manifest.access_set_hints"),
            "manifest.access_set_hints.read_keys"),
        stringList(
            required(root, "write_keys", "manifest.access_set_hints"),
            "manifest.access_set_hints.write_keys"),
        objectList(
            root.containsKey("dynamic_reads")
                ? root.get("dynamic_reads")
                : new ArrayList<Object>(),
            "manifest.access_set_hints.dynamic_reads",
            DYNAMIC_HINT_PARSER),
        objectList(
            root.containsKey("dynamic_writes")
                ? root.get("dynamic_writes")
                : new ArrayList<Object>(),
            "manifest.access_set_hints.dynamic_writes",
            DYNAMIC_HINT_PARSER));
  }

  private static ContractManifest.DynamicAccessHint parseDynamicHint(
      final Map<String, Object> root) {
    exactKeys(
        root, set("base_key", "key_type", "bound_kind", "max_keys"), "dynamic access hint");
    final String baseKey =
        exactString(required(root, "base_key", "dynamic access hint"), "dynamic access hint.base_key");
    final String stateName =
        baseKey.startsWith("state:") ? baseKey.substring("state:".length()) : "";
    check(
        baseKey.startsWith("state:") && canonicalDeclarationIdentifier(stateName),
        "dynamic access hint.base_key must be state: followed by one canonical state declaration"
            + " identifier");
    final String keyType =
        exactString(
            required(root, "key_type", "dynamic access hint"), "dynamic access hint.key_type");
    check(
        STATE_MAP_KEY_TYPE_NAMES.contains(keyType),
        "dynamic access hint.key_type must be an exact canonical StateMap key type");
    final String boundKind =
        exactString(
            required(root, "bound_kind", "dynamic access hint"),
            "dynamic access hint.bound_kind");
    check(
        DYNAMIC_ACCESS_BOUND_KINDS.contains(boundKind),
        "dynamic access hint.bound_kind must be `take` or `range`");
    final long maxKeys =
        unsignedInteger(
                required(root, "max_keys", "dynamic access hint"),
                MAX_DYNAMIC_ACCESS_KEYS,
                "dynamic access hint.max_keys")
            .longValueExact();
    check(maxKeys > 0, "dynamic access hint.max_keys must be in 1..64");
    return new ContractManifest.DynamicAccessHint(
        baseKey,
        keyType,
        boundKind,
        maxKeys);
  }

  private static void validateDynamicAccessHintStateMaps(
      final ContractManifest.AccessSetHints accessSetHints,
      final List<ContractManifest.StateDescriptor> states) {
    if (accessSetHints == null) {
      return;
    }
    final Map<String, String> stateMapKeyTypes = new HashMap<>();
    if (states != null) {
      for (final ContractManifest.StateDescriptor state : states) {
        final String keyType = topLevelStateMapKeyType(state.typeName());
        if (keyType != null) {
          stateMapKeyTypes.put(state.name(), keyType);
        }
      }
    }
    validateDynamicAccessHintList(
        "manifest.access_set_hints.dynamic_reads",
        accessSetHints.dynamicReads(),
        stateMapKeyTypes);
    validateDynamicAccessHintList(
        "manifest.access_set_hints.dynamic_writes",
        accessSetHints.dynamicWrites(),
        stateMapKeyTypes);
  }

  private static void validateDynamicAccessHintList(
      final String field,
      final List<ContractManifest.DynamicAccessHint> hints,
      final Map<String, String> stateMapKeyTypes) {
    final Set<List<Object>> unique = new HashSet<>();
    for (final ContractManifest.DynamicAccessHint hint : hints) {
      check(
          unique.add(
              Arrays.asList(
                  hint.baseKey(), hint.keyType(), hint.boundKind(), hint.maxKeys())),
          field + " must not contain duplicate hints for `" + hint.baseKey() + "`");
      final String stateName = hint.baseKey().substring("state:".length());
      final String expectedKeyType = stateMapKeyTypes.get(stateName);
      check(
          expectedKeyType != null,
          field
              + " hint `"
              + hint.baseKey()
              + "` must reference a declared top-level StateMap");
      check(
          hint.keyType().equals(expectedKeyType),
          field
              + " hint `"
              + hint.baseKey()
              + "` declares key_type `"
              + hint.keyType()
              + "` but its StateMap key type is `"
              + expectedKeyType
              + "`");
    }
  }

  private static String topLevelStateMapKeyType(final String typeName) {
    final String prefix = "StateMap<";
    if (!typeName.startsWith(prefix)) {
      return null;
    }
    final int separator = typeName.indexOf(", ", prefix.length());
    if (separator < 0) {
      return null;
    }
    final String keyType = typeName.substring(prefix.length(), separator);
    return STATE_MAP_KEY_TYPE_NAMES.contains(keyType) ? keyType : null;
  }

  private static ContractManifest.EntrypointDescriptor parseEntrypoint(
      final Map<String, Object> root) {
    exactKeys(
        root,
        set(
            "name",
            "kind",
            "params",
            "argument_schema",
            "return_type",
            "return_schema",
            "permission",
            "read_keys",
            "write_keys",
            "access_hints_complete",
            "access_hints_skipped",
            "triggers"),
        "entrypoint descriptor");
    final String name =
        exactString(required(root, "name", "entrypoint descriptor"), "entrypoint descriptor.name");
    check(
        canonicalEntrypointName(name),
        "entrypoint descriptor.name must be a canonical Kotodama identifier or branded selector");
    final ContractManifest.EntrypointKind kind =
        parseEntrypointKind(
            object(required(root, "kind", "entrypoint descriptor"), "entrypoint descriptor.kind"));
    final boolean lifecycleMatches =
        (kind == ContractManifest.EntrypointKind.HAJIMARI
                && ("hajimari".equals(name) || "始まり".equals(name)))
            || (kind == ContractManifest.EntrypointKind.KAIZEN
                && ("kaizen".equals(name) || "改善".equals(name)))
            || ((kind == ContractManifest.EntrypointKind.KOTOAGE
                    || kind == ContractManifest.EntrypointKind.VIEW)
                && !"hajimari".equals(name)
                && !"始まり".equals(name)
                && !"kaizen".equals(name)
                && !"改善".equals(name));
    check(lifecycleMatches, "entrypoint descriptor kind does not match its branded selector");
    final List<ContractManifest.EntrypointParameter> parameters =
        objectList(
            root.containsKey("params") ? root.get("params") : new ArrayList<Object>(),
            "entrypoint descriptor.params",
            PARAMETER_PARSER);
    check(parameters.size() <= 13, "entrypoint descriptor.params exceeds the V1 limit");
    final List<String> parameterNames = new ArrayList<>();
    for (final ContractManifest.EntrypointParameter parameter : parameters) {
      parameterNames.add(parameter.name());
    }
    unique(parameterNames, "entrypoint descriptor.params");
    final ContractManifest.ArgumentSchemaV1 argumentSchema =
        root.containsKey("argument_schema") && root.get("argument_schema") != null
            ? parseArgumentSchema(
                object(root.get("argument_schema"), "entrypoint descriptor.argument_schema"))
            : null;
    boolean exactArguments = parameters.isEmpty() && argumentSchema == null;
    if (!parameters.isEmpty()
        && argumentSchema != null
        && parameters.size() == argumentSchema.fields().size()) {
      exactArguments = true;
      for (int index = 0; index < parameters.size(); index++) {
        final ContractManifest.EntrypointParameter parameter = parameters.get(index);
        final ContractManifest.ArgumentFieldV1 field = argumentSchema.fields().get(index);
        if (!parameter.name().equals(field.name())
            || !parameter.typeName().equals(field.valueType().canonicalTypeName())) {
          exactArguments = false;
          break;
        }
      }
    }
    check(exactArguments, "entrypoint descriptor argument schema does not exactly match params");
    final String returnType =
        optionalTypeName(root, "return_type", "entrypoint descriptor.return_type");
    final ContractManifest.ValueTypeV1 returnSchema =
        root.containsKey("return_schema") && root.get("return_schema") != null
            ? parseValueType(object(root.get("return_schema"), "entrypoint descriptor.return_schema"))
            : null;
    check(
        (returnType == null) == (returnSchema == null),
        "entrypoint descriptor return_type and return_schema must be present together");
    check(
        returnSchema == null
            || (returnSchema.wordCount() <= 13
                && returnSchema.canonicalTypeName().equals(returnType)),
        "entrypoint descriptor return schema does not exactly match return_type");
    final String permission =
        optionalExactString(root, "permission", "entrypoint descriptor.permission");
    check(
        kind != ContractManifest.EntrypointKind.KOTOAGE || permission != null,
        "kotoage entrypoint descriptor must declare permission");
    check(
        (kind != ContractManifest.EntrypointKind.HAJIMARI
                && kind != ContractManifest.EntrypointKind.KAIZEN)
            || permission == null,
        "hajimari and kaizen entrypoints use runtime-defined authorization");
    final List<String> readKeys =
        stringList(
            root.containsKey("read_keys") ? root.get("read_keys") : new ArrayList<Object>(),
            "entrypoint descriptor.read_keys");
    final List<String> writeKeys =
        stringList(
            root.containsKey("write_keys") ? root.get("write_keys") : new ArrayList<Object>(),
            "entrypoint descriptor.write_keys");
    final Boolean complete =
        optionalBoolean(root, "access_hints_complete", "entrypoint descriptor.access_hints_complete");
    final List<String> skipped =
        stringList(
            root.containsKey("access_hints_skipped")
                ? root.get("access_hints_skipped")
                : new ArrayList<Object>(),
            "entrypoint descriptor.access_hints_skipped");
    check(!Boolean.TRUE.equals(complete) || skipped.isEmpty(), "complete access hints must not contain skipped reasons");
    check(!Boolean.FALSE.equals(complete) || !skipped.isEmpty(), "incomplete access hints must contain a skipped reason");
    final List<ContractManifest.TriggerDescriptor> triggers =
        objectList(
            root.containsKey("triggers") ? root.get("triggers") : new ArrayList<Object>(),
            "entrypoint descriptor.triggers",
            TRIGGER_PARSER);
    return new ContractManifest.EntrypointDescriptor(
        name,
        kind,
        parameters,
        argumentSchema,
        returnType,
        returnSchema,
        permission,
        readKeys,
        writeKeys,
        complete,
        skipped,
        triggers);
  }

  private static ContractManifest.EntrypointKind parseEntrypointKind(
      final Map<String, Object> root) {
    exactKeys(root, set("kind", "value"), "entrypoint descriptor.kind");
    check(
        root.containsKey("value") && root.get("value") == null,
        "entrypoint descriptor.kind.value must be null");
    final String kind =
        exactString(
            required(root, "kind", "entrypoint descriptor.kind"),
            "entrypoint descriptor.kind.kind");
    switch (kind) {
      case "Kotoage":
        return ContractManifest.EntrypointKind.KOTOAGE;
      case "View":
        return ContractManifest.EntrypointKind.VIEW;
      case "Hajimari":
        return ContractManifest.EntrypointKind.HAJIMARI;
      case "Kaizen":
        return ContractManifest.EntrypointKind.KAIZEN;
      default:
        throw new IllegalStateException("unsupported branded Kotodama entrypoint kind");
    }
  }

  private static ContractManifest.EntrypointParameter parseParameter(
      final Map<String, Object> root) {
    exactKeys(root, set("name", "type_name"), "entrypoint parameter");
    final String name =
        exactString(required(root, "name", "entrypoint parameter"), "entrypoint parameter.name");
    check(
        canonicalSourceIdentifier(name),
        "entrypoint parameter.name must be a canonical Kotodama identifier");
    return new ContractManifest.EntrypointParameter(
        name,
        currentTypeName(
            exactString(
                required(root, "type_name", "entrypoint parameter"),
                "entrypoint parameter.type_name"),
            "entrypoint parameter.type_name"));
  }

  private static ContractManifest.ArgumentSchemaV1 parseArgumentSchema(
      final Map<String, Object> root) {
    exactKeys(root, set("fields"), "entrypoint argument schema");
    final List<ContractManifest.ArgumentFieldV1> fields =
        objectList(
            required(root, "fields", "entrypoint argument schema"),
            "entrypoint argument schema.fields",
            ARGUMENT_FIELD_PARSER);
    check(
        !fields.isEmpty() && fields.size() <= 13,
        "entrypoint argument schema must contain 1..13 fields");
    final List<String> names = new ArrayList<>();
    int words = 0;
    for (final ContractManifest.ArgumentFieldV1 field : fields) {
      names.add(field.name());
      words += field.valueType().wordCount();
    }
    unique(names, "entrypoint argument schema.fields");
    check(words <= 13, "entrypoint argument schema exceeds the V1 register window");
    return new ContractManifest.ArgumentSchemaV1(fields, words);
  }

  private static ContractManifest.ArgumentFieldV1 parseArgumentField(
      final Map<String, Object> root) {
    exactKeys(root, set("name", "ty"), "entrypoint argument field");
    final String name =
        exactString(required(root, "name", "entrypoint argument field"), "entrypoint argument field.name");
    check(
        canonicalSourceIdentifier(name),
        "entrypoint argument field.name must be a canonical Kotodama identifier");
    return new ContractManifest.ArgumentFieldV1(
        name,
        parseValueType(
            object(
                required(root, "ty", "entrypoint argument field"),
                "entrypoint argument field.ty")));
  }

  private static ContractManifest.ValueTypeV1 parseValueType(final Map<String, Object> root) {
    exactKeys(root, set("nodes"), "entrypoint value type");
    final List<ContractManifest.ValueTypeNodeV1> nodes =
        objectList(
            required(root, "nodes", "entrypoint value type"),
            "entrypoint value type.nodes",
            VALUE_TYPE_NODE_PARSER);
    check(
        !nodes.isEmpty() && nodes.size() <= 256,
        "entrypoint value type must contain 1..256 nodes");
    final TypeAnalysis analysis = analyze(nodes);
    check(
        analysis.nextIndex == nodes.size()
            && analysis.nodeCount <= 256
            && validateReservedNominalShapes(nodes),
        "entrypoint value type is not one canonical flat preorder V1 schema");
    return new ContractManifest.ValueTypeV1(nodes, analysis.wordCount, analysis.typeName);
  }

  private static ContractManifest.ValueTypeNodeV1 parseValueTypeNode(
      final Map<String, Object> root) {
    exactKeys(root, set("kind", "value"), "entrypoint value type node");
    check(root.containsKey("value"), "entrypoint value type node.value is required");
    final String kind =
        exactString(
            required(root, "kind", "entrypoint value type node"),
            "entrypoint value type node.kind");
    switch (kind) {
      case "Struct":
        return new ContractManifest.ValueTypeNodeV1(
            ContractManifest.ValueTypeNodeKindV1.STRUCT,
            parseStructNode(object(root.get("value"), "entrypoint struct node")),
            null,
            null,
            null);
      case "Tuple":
        final int arity =
            unsignedInteger(root.get("value"), BigInteger.valueOf(0xffff), "entrypoint tuple arity")
                .intValueExact();
        check(arity >= 2, "entrypoint tuple arity must be in 2..65535");
        return new ContractManifest.ValueTypeNodeV1(
            ContractManifest.ValueTypeNodeKindV1.TUPLE, null, arity, null, null);
      case "Option":
        check(root.get("value") == null, "entrypoint Option node.value must be null");
        return new ContractManifest.ValueTypeNodeV1(
            ContractManifest.ValueTypeNodeKindV1.OPTION, null, null, null, null);
      case "Result":
        check(root.get("value") == null, "entrypoint Result node.value must be null");
        return new ContractManifest.ValueTypeNodeV1(
            ContractManifest.ValueTypeNodeKindV1.RESULT, null, null, null, null);
      case "List":
        return new ContractManifest.ValueTypeNodeV1(
            ContractManifest.ValueTypeNodeKindV1.LIST,
            null,
            null,
            parseListNode(object(root.get("value"), "entrypoint list node")),
            null);
      case "Leaf":
        return new ContractManifest.ValueTypeNodeV1(
            ContractManifest.ValueTypeNodeKindV1.LEAF,
            null,
            null,
            null,
            parseLeafKind(object(root.get("value"), "entrypoint value kind")));
      default:
        throw new IllegalStateException("unsupported Kotodama boundary type node");
    }
  }

  private static ContractManifest.StructTypeNodeV1 parseStructNode(
      final Map<String, Object> root) {
    exactKeys(root, set("name", "fields"), "entrypoint struct node");
    final String name =
        exactString(required(root, "name", "entrypoint struct node"), "entrypoint struct node.name");
    final List<String> fields =
        stringList(
            required(root, "fields", "entrypoint struct node"), "entrypoint struct node.fields");
    check(
        (canonicalTypeDeclarationIdentifier(name)
                || "QueryPage".equals(name)
                || isCoreQueryViewName(name))
            && !fields.isEmpty(),
        "entrypoint struct node must use canonical Kotodama identifiers");
    for (final String field : fields) {
      check(
          canonicalSourceIdentifier(field),
          "entrypoint struct node must use canonical Kotodama identifiers");
    }
    unique(fields, "entrypoint struct node.fields");
    return new ContractManifest.StructTypeNodeV1(name, fields);
  }

  private static ContractManifest.ListTypeNodeV1 parseListNode(final Map<String, Object> root) {
    exactKeys(root, set("capacity"), "entrypoint list node");
    final int capacity =
        unsignedInteger(
                required(root, "capacity", "entrypoint list node"),
                BigInteger.valueOf(64),
                "entrypoint list node.capacity")
            .intValueExact();
    check(capacity >= 1, "entrypoint list node.capacity must be in 1..64");
    return new ContractManifest.ListTypeNodeV1(capacity);
  }

  private static ContractManifest.ValueKindV1 parseLeafKind(final Map<String, Object> root) {
    exactKeys(root, set("kind", "value"), "entrypoint value kind");
    check(
        root.containsKey("value") && root.get("value") == null,
        "entrypoint value kind.value must be null");
    final String label =
        exactString(
            required(root, "kind", "entrypoint value kind"), "entrypoint value kind.kind");
    final ContractManifest.ValueKindV1 kind = VALUE_KINDS.get(label);
    check(kind != null, "unsupported Kotodama boundary value kind");
    return kind;
  }

  private static TypeAnalysis analyze(
      final List<ContractManifest.ValueTypeNodeV1> nodes) {
    final List<TraversalFrame> frames = new ArrayList<>();
    int words = 0;
    int maximumDepth = 0;
    for (int index = 0; index < nodes.size(); index++) {
      while (!frames.isEmpty() && frames.get(frames.size() - 1).remaining == 0) {
        frames.remove(frames.size() - 1);
      }
      final ContractManifest.ValueTypeNodeV1 node = nodes.get(index);
      final boolean suppressWords;
      if (index == 0) {
        suppressWords = false;
      } else {
        check(!frames.isEmpty(), "entrypoint value type contains a trailing preorder node");
        final TraversalFrame parent = frames.get(frames.size() - 1);
        check(parent.remaining > 0, "entrypoint value type contains a trailing preorder node");
        parent.remaining--;
        suppressWords = parent.suppressWords;
      }
      final int depth = frames.size() + 1;
      check(depth <= 256, "entrypoint value type exceeds the V1 nesting depth");
      maximumDepth = Math.max(maximumDepth, depth);

      final boolean handle =
          node.kind() == ContractManifest.ValueTypeNodeKindV1.OPTION
              || node.kind() == ContractManifest.ValueTypeNodeKindV1.RESULT
              || node.kind() == ContractManifest.ValueTypeNodeKindV1.LIST;
      if (!suppressWords
          && (handle || node.kind() == ContractManifest.ValueTypeNodeKindV1.LEAF)) {
        words++;
      }
      final int children = nodeChildCount(node);
      if (children != 0) {
        frames.add(new TraversalFrame(children, suppressWords || handle));
      }
    }
    while (!frames.isEmpty() && frames.get(frames.size() - 1).remaining == 0) {
      frames.remove(frames.size() - 1);
    }
    check(frames.isEmpty(), "entrypoint value type ends before its preorder tree is complete");

    final List<RenderedType> rendered = new ArrayList<>();
    for (int index = nodes.size() - 1; index >= 0; index--) {
      final ContractManifest.ValueTypeNodeV1 node = nodes.get(index);
      final int childCount = nodeChildCount(node);
      check(
          rendered.size() >= childCount,
          "entrypoint value type ends before its preorder tree is complete");
      final List<RenderedType> children = new ArrayList<>();
      for (int child = 0; child < childCount; child++) {
        children.add(rendered.remove(rendered.size() - 1));
      }

      final RenderedType value;
      switch (node.kind()) {
        case STRUCT:
          final ContractManifest.StructTypeNodeV1 struct =
              requiredValue(node.structValue(), "struct metadata");
          if ("QueryPage".equals(struct.name())) {
            final String viewName =
                children.isEmpty() ? null : children.get(0).listElementCoreViewName;
            value =
                new RenderedType(
                    viewName == null ? "struct QueryPage" : "QueryPage<" + viewName + ">",
                    null,
                    null);
          } else if (isCoreQueryViewName(struct.name())) {
            value = new RenderedType(struct.name(), struct.name(), null);
          } else {
            value = new RenderedType("struct " + struct.name(), null, null);
          }
          break;
        case TUPLE:
          final List<String> tupleNames = new ArrayList<>();
          for (final RenderedType child : children) {
            tupleNames.add(child.typeName);
          }
          value = new RenderedType("(" + join(tupleNames, ", ") + ")", null, null);
          break;
        case OPTION:
          value = new RenderedType("Option<" + children.get(0).typeName + ">", null, null);
          break;
        case RESULT:
          value =
              new RenderedType(
                  "Result<" + children.get(0).typeName + ", " + children.get(1).typeName + ">",
                  null,
                  null);
          break;
        case LIST:
          final ContractManifest.ListTypeNodeV1 list =
              requiredValue(node.listValue(), "list metadata");
          final RenderedType element = children.get(0);
          value =
              new RenderedType(
                  "List<" + element.typeName + ", " + list.capacity() + ">",
                  null,
                  element.coreViewName);
          break;
        case LEAF:
          value =
              new RenderedType(
                  canonicalLeafName(requiredValue(node.leafKind(), "leaf kind")), null, null);
          break;
        default:
          throw new IllegalStateException("unsupported boundary type node");
      }
      rendered.add(value);
    }
    check(rendered.size() == 1, "entrypoint value type is not one canonical preorder tree");
    return new TypeAnalysis(
        nodes.size(), nodes.size(), words, maximumDepth, rendered.get(0).typeName);
  }

  private static boolean isCoreQueryViewName(final String name) {
    return "AccountView".equals(name)
        || "AssetView".equals(name)
        || "AssetDefinitionView".equals(name)
        || "DomainView".equals(name)
        || "NftView".equals(name);
  }

  private static int nodeChildCount(final ContractManifest.ValueTypeNodeV1 node) {
    switch (node.kind()) {
      case STRUCT:
        return requiredValue(node.structValue(), "struct metadata").fields().size();
      case TUPLE:
        return requiredValue(node.tupleArity(), "tuple arity");
      case OPTION:
      case LIST:
        return 1;
      case RESULT:
        return 2;
      case LEAF:
        return 0;
      default:
        throw new IllegalStateException("unsupported boundary type node");
    }
  }

  private static Integer subtreeEnd(
      final List<ContractManifest.ValueTypeNodeV1> nodes, final int start) {
    int index = start;
    int pending = 1;
    while (pending != 0) {
      if (index < 0 || index >= nodes.size()) {
        return null;
      }
      final ContractManifest.ValueTypeNodeV1 node = nodes.get(index++);
      pending = pending - 1 + nodeChildCount(node);
    }
    return index;
  }

  private static CoreViewRange coreQueryViewRange(
      final List<ContractManifest.ValueTypeNodeV1> nodes, final int start) {
    if (start < 0 || start >= nodes.size()) {
      return null;
    }
    final ContractManifest.ValueTypeNodeV1 root = nodes.get(start);
    if (root.kind() != ContractManifest.ValueTypeNodeKindV1.STRUCT) {
      return null;
    }
    final ContractManifest.StructTypeNodeV1 struct = requiredValue(root.structValue(), "struct metadata");
    if ("AccountView".equals(struct.name())) {
      return coreViewRange(
          nodes,
          start,
          struct,
          new String[] {"id", "metadata"},
          new ContractManifest.ValueKindV1[] {
            ContractManifest.ValueKindV1.ACCOUNT_ID, ContractManifest.ValueKindV1.JSON
          });
    }
    if ("AssetView".equals(struct.name())) {
      return coreViewRange(
          nodes,
          start,
          struct,
          new String[] {"id", "amount"},
          new ContractManifest.ValueKindV1[] {
            ContractManifest.ValueKindV1.ASSET_ID, ContractManifest.ValueKindV1.QUANTITY
          });
    }
    if ("DomainView".equals(struct.name())) {
      return coreViewRange(
          nodes,
          start,
          struct,
          new String[] {"id", "owned_by", "metadata"},
          new ContractManifest.ValueKindV1[] {
            ContractManifest.ValueKindV1.DOMAIN_ID,
            ContractManifest.ValueKindV1.ACCOUNT_ID,
            ContractManifest.ValueKindV1.JSON
          });
    }
    if ("NftView".equals(struct.name())) {
      return coreViewRange(
          nodes,
          start,
          struct,
          new String[] {"id", "owned_by", "content"},
          new ContractManifest.ValueKindV1[] {
            ContractManifest.ValueKindV1.NFT_ID,
            ContractManifest.ValueKindV1.ACCOUNT_ID,
            ContractManifest.ValueKindV1.JSON
          });
    }
    if (!"AssetDefinitionView".equals(struct.name())
        || !struct.fields().equals(
            Arrays.asList("id", "name", "description", "owned_by", "total_quantity", "metadata"))
        || !leafAt(nodes, start + 1, ContractManifest.ValueKindV1.ASSET_DEFINITION_ID)
        || !leafAt(nodes, start + 2, ContractManifest.ValueKindV1.STRING)
        || !nodeKindAt(nodes, start + 3, ContractManifest.ValueTypeNodeKindV1.OPTION)
        || !leafAt(nodes, start + 4, ContractManifest.ValueKindV1.STRING)
        || !leafAt(nodes, start + 5, ContractManifest.ValueKindV1.ACCOUNT_ID)
        || !leafAt(nodes, start + 6, ContractManifest.ValueKindV1.QUANTITY)
        || !leafAt(nodes, start + 7, ContractManifest.ValueKindV1.JSON)
        || !Integer.valueOf(start + 8).equals(subtreeEnd(nodes, start))) {
      return null;
    }
    return new CoreViewRange(start + 8);
  }

  private static CoreViewRange coreViewRange(
      final List<ContractManifest.ValueTypeNodeV1> nodes,
      final int start,
      final ContractManifest.StructTypeNodeV1 struct,
      final String[] fields,
      final ContractManifest.ValueKindV1[] leafKinds) {
    if (!struct.fields().equals(Arrays.asList(fields))) {
      return null;
    }
    for (int index = 0; index < leafKinds.length; index++) {
      if (!leafAt(nodes, start + 1 + index, leafKinds[index])) {
        return null;
      }
    }
    final int end = start + 1 + leafKinds.length;
    return Integer.valueOf(end).equals(subtreeEnd(nodes, start))
        ? new CoreViewRange(end)
        : null;
  }

  private static boolean leafAt(
      final List<ContractManifest.ValueTypeNodeV1> nodes,
      final int index,
      final ContractManifest.ValueKindV1 kind) {
    return index >= 0
        && index < nodes.size()
        && nodes.get(index).kind() == ContractManifest.ValueTypeNodeKindV1.LEAF
        && nodes.get(index).leafKind() == kind;
  }

  private static boolean nodeKindAt(
      final List<ContractManifest.ValueTypeNodeV1> nodes,
      final int index,
      final ContractManifest.ValueTypeNodeKindV1 kind) {
    return index >= 0 && index < nodes.size() && nodes.get(index).kind() == kind;
  }

  private static boolean validateReservedNominalShapes(
      final List<ContractManifest.ValueTypeNodeV1> nodes) {
    for (int start = 0; start < nodes.size(); start++) {
      final ContractManifest.ValueTypeNodeV1 node = nodes.get(start);
      if (node.kind() != ContractManifest.ValueTypeNodeKindV1.STRUCT) {
        continue;
      }
      final ContractManifest.StructTypeNodeV1 struct = requiredValue(node.structValue(), "struct metadata");
      if (isCoreQueryViewName(struct.name())) {
        if (coreQueryViewRange(nodes, start) == null) {
          return false;
        }
        continue;
      }
      if (!"QueryPage".equals(struct.name())) {
        continue;
      }
      if (!struct.fields().equals(Arrays.asList("items", "next_offset"))) {
        return false;
      }
      final Integer rootEnd = subtreeEnd(nodes, start);
      final int listStart = start + 1;
      if (!nodeKindAt(nodes, listStart, ContractManifest.ValueTypeNodeKindV1.LIST)) {
        return false;
      }
      final ContractManifest.ListTypeNodeV1 list = nodes.get(listStart).listValue();
      if (list == null || list.capacity() != 64) {
        return false;
      }
      final CoreViewRange view = coreQueryViewRange(nodes, listStart + 1);
      if (view == null || !Integer.valueOf(view.end).equals(subtreeEnd(nodes, listStart))) {
        return false;
      }
      final int nextOffset = view.end;
      if (!nodeKindAt(nodes, nextOffset, ContractManifest.ValueTypeNodeKindV1.OPTION)
          || !leafAt(nodes, nextOffset + 1, ContractManifest.ValueKindV1.INT)
          || !Integer.valueOf(nextOffset + 2).equals(subtreeEnd(nodes, nextOffset))
          || !Integer.valueOf(nextOffset + 2).equals(rootEnd)) {
        return false;
      }
    }
    return true;
  }

  private static String canonicalLeafName(final ContractManifest.ValueKindV1 kind) {
    switch (kind) {
      case INT:
        return "int";
      case DECIMAL:
        return "decimal";
      case QUANTITY:
        return "quantity";
      case BOOL:
        return "bool";
      case STRING:
        return "string";
      case JSON:
        return "Json";
      case NAME:
        return "Name";
      case ACCOUNT_ID:
        return "AccountId";
      case ASSET_DEFINITION_ID:
        return "AssetDefinitionId";
      case ASSET_ID:
        return "AssetId";
      case DOMAIN_ID:
        return "DomainId";
      case NFT_ID:
        return "NftId";
      case DATA_SPACE_ID:
        return "DataSpaceId";
      case BLOB:
        return "bytes";
      default:
        throw new IllegalStateException("unsupported leaf kind");
    }
  }

  private static ContractManifest.TriggerDescriptor parseTrigger(
      final Map<String, Object> root) {
    exactKeys(
        root,
        set("id", "repeats", "filter", "authority", "metadata", "callback"),
        "trigger descriptor");
    final String id =
        exactString(required(root, "id", "trigger descriptor"), "trigger descriptor.id");
    check(
        !"Amount".equals(id),
        "trigger descriptor.id must not use retired Kotodama source form Amount");
    final ContractManifest.TriggerRepeats repeats =
        parseRepeats(
            object(
                required(root, "repeats", "trigger descriptor"),
                "trigger descriptor.repeats"));
    final String filter =
        canonicalBase64(
            required(root, "filter", "trigger descriptor"), "trigger descriptor.filter");
    String authority = optionalExactString(root, "authority", "trigger descriptor.authority");
    if (authority != null) {
      try {
        authority =
            AccountIdLiteral.requireCanonicalI105Address(authority, "trigger descriptor.authority");
      } catch (final IllegalArgumentException error) {
        throw new IllegalStateException(
            "trigger descriptor.authority must be a canonical I105 account id", error);
      }
    }
    final Map<String, Object> metadata =
        object(
            required(root, "metadata", "trigger descriptor"), "trigger descriptor.metadata");
    final ContractManifest.TriggerCallback callback =
        parseCallback(
            object(
                required(root, "callback", "trigger descriptor"),
                "trigger descriptor.callback"));
    return new ContractManifest.TriggerDescriptor(id, repeats, filter, authority, metadata, callback);
  }

  private static ContractManifest.TriggerRepeats parseRepeats(final Map<String, Object> root) {
    check(root.size() == 1, "trigger descriptor.repeats must contain exactly one enum variant");
    if (root.containsKey("Indefinitely")) {
      check(root.get("Indefinitely") == null, "Repeats.Indefinitely value must be null");
      return new ContractManifest.TriggerRepeats(
          ContractManifest.TriggerRepeatsKind.INDEFINITELY, null);
    }
    if (root.containsKey("Exactly")) {
      return new ContractManifest.TriggerRepeats(
          ContractManifest.TriggerRepeatsKind.EXACTLY,
          unsignedInteger(root.get("Exactly"), MAX_U32, "Repeats.Exactly").longValueExact());
    }
    throw new IllegalStateException("unsupported trigger repetition policy");
  }

  private static ContractManifest.TriggerCallback parseCallback(final Map<String, Object> root) {
    exactKeys(root, set("namespace", "entrypoint"), "trigger callback");
    final String namespace = optionalExactString(root, "namespace", "trigger callback.namespace");
    check(
        !"Amount".equals(namespace),
        "trigger callback.namespace must not use retired Kotodama source form Amount");
    final String entrypoint =
        exactString(
            required(root, "entrypoint", "trigger callback"), "trigger callback.entrypoint");
    check(
        canonicalEntrypointName(entrypoint),
        "trigger callback.entrypoint must be a canonical Kotodama selector");
    return new ContractManifest.TriggerCallback(namespace, entrypoint);
  }

  private static ContractManifest.StateDescriptor parseState(final Map<String, Object> root) {
    exactKeys(root, set("name", "type_name"), "state descriptor");
    final String name =
        exactString(required(root, "name", "state descriptor"), "state descriptor.name");
    check(
        canonicalDeclarationIdentifier(name),
        "state descriptor.name must be a canonical Kotodama identifier");
    final String typeName =
        exactString(
            required(root, "type_name", "state descriptor"), "state descriptor.type_name");
    check(
        new StateTypeNameParser(typeName).parse(),
        "state descriptor.type_name must be a canonical Kotodama V1 state type");
    return new ContractManifest.StateDescriptor(name, typeName);
  }

  private static ContractManifest.ErrorCodeDescriptor parseErrorCode(
      final Map<String, Object> root) {
    exactKeys(root, set("namespace", "name", "code"), "error code descriptor");
    final String namespace =
        exactString(
            required(root, "namespace", "error code descriptor"),
            "error code descriptor.namespace");
    final String name =
        exactString(required(root, "name", "error code descriptor"), "error code descriptor.name");
    check(
        canonicalTypeDeclarationIdentifier(namespace) && canonicalSourceIdentifier(name),
        "error code namespace and name must be canonical Kotodama identifiers");
    final long code =
        unsignedInteger(
                required(root, "code", "error code descriptor"),
                MAX_U32,
                "error code descriptor.code")
            .longValueExact();
    check(code > 0, "error code descriptor.code must be a non-zero u32");
    return new ContractManifest.ErrorCodeDescriptor(namespace, name, code);
  }

  private static ContractManifest.KotobaTranslationEntry parseKotobaEntry(
      final Map<String, Object> root) {
    exactKeys(root, set("msg_id", "translations"), "kotoba translation entry");
    final String messageId =
        exactString(
            required(root, "msg_id", "kotoba translation entry"),
            "kotoba translation entry.msg_id");
    final List<ContractManifest.KotobaTranslation> translations =
        objectList(
            required(root, "translations", "kotoba translation entry"),
            "kotoba translation entry.translations",
            KOTOBA_TRANSLATION_PARSER);
    final List<String> languages = new ArrayList<>();
    for (final ContractManifest.KotobaTranslation translation : translations) {
      languages.add(translation.language());
    }
    unique(languages, "kotoba translation entry.translations");
    return new ContractManifest.KotobaTranslationEntry(messageId, translations);
  }

  private static ContractManifest.KotobaTranslation parseKotobaTranslation(
      final Map<String, Object> root) {
    exactKeys(root, set("lang", "text"), "kotoba translation");
    final Object text = required(root, "text", "kotoba translation");
    check(text instanceof String, "kotoba translation.text must be a string");
    return new ContractManifest.KotobaTranslation(
        exactString(
            required(root, "lang", "kotoba translation"), "kotoba translation.lang"),
        (String) text);
  }

  private static ContractManifest.Provenance parseProvenance(final Map<String, Object> root) {
    exactKeys(root, set("signer", "signature"), "manifest.provenance");
    return new ContractManifest.Provenance(
        exactString(
            required(root, "signer", "manifest.provenance"), "manifest.provenance.signer"),
        exactString(
            required(root, "signature", "manifest.provenance"),
            "manifest.provenance.signature"));
  }

  private interface ObjectParser<T> {
    T parse(Map<String, Object> value);
  }

  private static final ObjectParser<ContractManifest.DynamicAccessHint> DYNAMIC_HINT_PARSER =
      ContractManifestJsonParser::parseDynamicHint;
  private static final ObjectParser<ContractManifest.EntrypointDescriptor> ENTRYPOINT_PARSER =
      ContractManifestJsonParser::parseEntrypoint;
  private static final ObjectParser<ContractManifest.EntrypointParameter> PARAMETER_PARSER =
      ContractManifestJsonParser::parseParameter;
  private static final ObjectParser<ContractManifest.ArgumentFieldV1> ARGUMENT_FIELD_PARSER =
      ContractManifestJsonParser::parseArgumentField;
  private static final ObjectParser<ContractManifest.ValueTypeNodeV1> VALUE_TYPE_NODE_PARSER =
      ContractManifestJsonParser::parseValueTypeNode;
  private static final ObjectParser<ContractManifest.TriggerDescriptor> TRIGGER_PARSER =
      ContractManifestJsonParser::parseTrigger;
  private static final ObjectParser<ContractManifest.StateDescriptor> STATE_PARSER =
      ContractManifestJsonParser::parseState;
  private static final ObjectParser<ContractManifest.ErrorCodeDescriptor> ERROR_CODE_PARSER =
      ContractManifestJsonParser::parseErrorCode;
  private static final ObjectParser<ContractManifest.KotobaTranslationEntry> KOTOBA_ENTRY_PARSER =
      ContractManifestJsonParser::parseKotobaEntry;
  private static final ObjectParser<ContractManifest.KotobaTranslation> KOTOBA_TRANSLATION_PARSER =
      ContractManifestJsonParser::parseKotobaTranslation;

  private static Object parse(final byte[] payload, final String context) {
    check(payload != null && payload.length > 0, context + " returned an empty payload");
    final String json = new String(payload, StandardCharsets.UTF_8);
    check(!json.trim().isEmpty(), context + " returned a blank payload");
    return JsonParser.parse(json);
  }

  private static Object required(
      final Map<String, Object> root, final String name, final String context) {
    check(root.containsKey(name), context + "." + name + " is required");
    return root.get(name);
  }

  private static String optionalExactString(
      final Map<String, Object> root, final String name, final String path) {
    return !root.containsKey(name) || root.get(name) == null
        ? null
        : exactString(root.get(name), path);
  }

  private static Boolean optionalBoolean(
      final Map<String, Object> root, final String name, final String path) {
    if (!root.containsKey(name) || root.get(name) == null) {
      return null;
    }
    check(root.get(name) instanceof Boolean, path + " must be a boolean");
    return (Boolean) root.get(name);
  }

  private static String optionalManifestHash(
      final Map<String, Object> root, final String name, final String path) {
    return !root.containsKey(name) || root.get(name) == null
        ? null
        : manifestHash(root.get(name), path);
  }

  private static String optionalConvenienceHash(
      final Map<String, Object> root, final String name, final String path) {
    if (!root.containsKey(name) || root.get(name) == null) {
      return null;
    }
    final Object value = root.get(name);
    check(
        value instanceof String && CONVENIENCE_HASH.matcher((String) value).matches(),
        path + " must be canonical lowercase 64-hex");
    check(markerBitIsSet((String) value), path + " must set the Iroha Hash marker bit");
    return (String) value;
  }

  private static String manifestHash(final Object value, final String path) {
    check(
        value instanceof String,
        path + " must be a canonical checksummed Norito Hash literal");
    final Matcher match = MANIFEST_HASH.matcher((String) value);
    check(match.matches(), path + " must be a canonical checksummed Norito Hash literal");
    final String body = match.group(1);
    final int supplied = Integer.parseInt(match.group(2), 16);
    check(
        supplied == crc16(("hash:" + body).getBytes(StandardCharsets.US_ASCII)),
        path + " has an invalid Norito Hash checksum");
    final String normalized = body.toLowerCase(java.util.Locale.ROOT);
    check(markerBitIsSet(normalized), path + " must set the Iroha Hash marker bit");
    return normalized;
  }

  private static int crc16(final byte[] bytes) {
    int crc = 0xffff;
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

  private static boolean markerBitIsSet(final String hex) {
    return (Integer.parseInt(hex.substring(hex.length() - 2), 16) & 1) == 1;
  }

  private static String canonicalBase64(final Object value, final String path) {
    final String text = exactString(value, path);
    final byte[] bytes;
    try {
      bytes = Base64.getDecoder().decode(text);
    } catch (final IllegalArgumentException error) {
      throw new IllegalStateException(path + " must be canonical base64", error);
    }
    check(
        bytes.length > 0 && Base64.getEncoder().encodeToString(bytes).equals(text),
        path + " must be non-empty canonical base64");
    return text;
  }

  private static String exactString(final Object value, final String path) {
    check(value instanceof String, path + " must be a non-empty string");
    final String text = (String) value;
    check(!text.trim().isEmpty(), path + " must be a non-empty string");
    check(text.trim().equals(text), path + " must not contain surrounding whitespace");
    for (int index = 0; index < text.length(); index++) {
      check(!Character.isISOControl(text.charAt(index)), path + " must not contain control characters");
    }
    return text;
  }

  private static List<String> stringList(final Object value, final String path) {
    final List<Object> items = list(value, path);
    final List<String> result = new ArrayList<>();
    for (int index = 0; index < items.size(); index++) {
      result.add(exactString(items.get(index), path + "[" + index + "]"));
    }
    return result;
  }

  private static <T> List<T> optionalObjectList(
      final Map<String, Object> root,
      final String name,
      final String path,
      final ObjectParser<T> parser) {
    return !root.containsKey(name) || root.get(name) == null
        ? null
        : objectList(root.get(name), path, parser);
  }

  private static <T> List<T> objectList(
      final Object value, final String path, final ObjectParser<T> parser) {
    final List<Object> items = list(value, path);
    final List<T> result = new ArrayList<>();
    for (int index = 0; index < items.size(); index++) {
      result.add(parser.parse(object(items.get(index), path + "[" + index + "]")));
    }
    return result;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> list(final Object value, final String path) {
    check(value instanceof List<?>, path + " must be an array");
    return (List<Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Object value, final String path) {
    check(value instanceof Map<?, ?>, path + " must be an object");
    final Map<?, ?> raw = (Map<?, ?>) value;
    for (final Object key : raw.keySet()) {
      check(key instanceof String, path + " must use string object keys");
    }
    return (Map<String, Object>) raw;
  }

  private static void exactKeys(
      final Map<String, Object> root, final Set<String> allowed, final String path) {
    for (final String key : root.keySet()) {
      check(allowed.contains(key), path + " contains unknown field `" + key + "`");
    }
  }

  private static BigInteger unsignedInteger(
      final Object value, final BigInteger maximum, final String path) {
    final BigInteger integer;
    if (value instanceof BigInteger) {
      integer = (BigInteger) value;
    } else if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      integer = BigInteger.valueOf(((Number) value).longValue());
    } else {
      throw new IllegalStateException(path + " must be an unsigned integer");
    }
    check(
        integer.signum() >= 0 && integer.compareTo(maximum) <= 0,
        path + " is outside its unsigned integer range");
    return integer;
  }

  private static boolean canonicalIdentifierSyntax(final String value) {
    if (value.isEmpty()) {
      return false;
    }
    final char first = value.charAt(0);
    if (!(first == '_' || first >= 'A' && first <= 'Z' || first >= 'a' && first <= 'z')) {
      return false;
    }
    for (int index = 1; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!(character == '_'
          || character >= 'A' && character <= 'Z'
          || character >= 'a' && character <= 'z'
          || character >= '0' && character <= '9')) {
        return false;
      }
    }
    return true;
  }

  private static boolean canonicalSourceIdentifier(final String value) {
    return canonicalIdentifierSyntax(value) && !RESERVED_IDENTIFIERS.contains(value);
  }

  private static boolean canonicalDeclarationIdentifier(final String value) {
    return canonicalSourceIdentifier(value)
        && !RESERVED_DECLARATION_NAMES.contains(value)
        && !value.startsWith("__kotodama_link_");
  }

  private static boolean canonicalTypeDeclarationIdentifier(final String value) {
    return canonicalDeclarationIdentifier(value) && !RETIRED_NUMERIC_TYPE_NAMES.contains(value);
  }

  private static final class StateTypeNameParser {
    private static final String AGGREGATE_TYPE = "aggregate";

    private final String value;
    private int cursor;
    private int nodes;

    private StateTypeNameParser(final String value) {
      this.value = value;
    }

    private boolean parse() {
      return !value.isEmpty() && parseType(true, 1) != null && cursor == value.length();
    }

    private String parseType(final boolean allowStateMap, final int depth) {
      nodes++;
      if (depth > MAX_STATE_TYPE_DEPTH || nodes > MAX_STATE_TYPE_NODES) {
        return null;
      }

      if (consume("(")) {
        if (parseType(false, depth + 1) == null || !consume(", ")) {
          return null;
        }
        if (parseType(false, depth + 1) == null) {
          return null;
        }
        while (consume(", ")) {
          if (parseType(false, depth + 1) == null) {
            return null;
          }
        }
        return consume(")") ? AGGREGATE_TYPE : null;
      }

      final String name = identifier();
      if (name == null) {
        return null;
      }
      if (STATE_SCALAR_TYPE_NAMES.contains(name)) {
        return name;
      }
      if ("Option".equals(name)) {
        if (!consume("<") || parseType(false, depth + 1) == null || !consume(">")) {
          return null;
        }
        return AGGREGATE_TYPE;
      }
      if ("Result".equals(name)) {
        if (!consume("<")
            || parseType(false, depth + 1) == null
            || !consume(", ")
            || parseType(false, depth + 1) == null
            || !consume(">")) {
          return null;
        }
        return AGGREGATE_TYPE;
      }
      if ("List".equals(name)) {
        if (!consume("<")
            || parseType(false, depth + 1) == null
            || !consume(", ")
            || !listCapacity()
            || !consume(">")) {
          return null;
        }
        return AGGREGATE_TYPE;
      }
      if ("StateMap".equals(name)) {
        if (!allowStateMap || !consume("<")) {
          return null;
        }
        // The map wrapper and scalar key are outside the stored value schema
        // node budget, but the wrapper still counts in CNTR descriptor depth.
        nodes--;
        final String keyType = identifier();
        if (!STATE_MAP_KEY_TYPE_NAMES.contains(keyType)
            || !consume(", ")
            || parseType(false, depth + 1) == null
            || !consume(">")) {
          return null;
        }
        return AGGREGATE_TYPE;
      }

      if (!canonicalTypeDeclarationIdentifier(name) || !consume("{")) {
        return null;
      }
      final Set<String> fields = new HashSet<>();
      while (true) {
        final String field = identifier();
        if (field == null
            || !canonicalSourceIdentifier(field)
            || field.startsWith("__kotodama_link_")
            || !fields.add(field)
            || !consume(": ")) {
          return null;
        }
        if (parseType(false, depth + 1) == null) {
          return null;
        }
        if (consume("}")) {
          return AGGREGATE_TYPE;
        }
        if (!consume(", ")) {
          return null;
        }
      }
    }

    private boolean consume(final String literal) {
      if (!value.startsWith(literal, cursor)) {
        return false;
      }
      cursor += literal.length();
      return true;
    }

    private String identifier() {
      if (cursor >= value.length() || !isTypeIdentifierStart(value.charAt(cursor))) {
        return null;
      }
      final int start = cursor++;
      while (cursor < value.length() && isTypeIdentifierPart(value.charAt(cursor))) {
        cursor++;
      }
      return value.substring(start, cursor);
    }

    private boolean listCapacity() {
      final int start = cursor;
      int capacity = 0;
      while (cursor < value.length()) {
        final char character = value.charAt(cursor);
        if (character < '0' || character > '9') {
          break;
        }
        capacity = Math.min(65, capacity * 10 + character - '0');
        cursor++;
      }
      if (cursor == start || cursor - start > 1 && value.charAt(start) == '0') {
        return false;
      }
      return capacity >= 1 && capacity <= 64;
    }
  }

  private static String optionalTypeName(
      final Map<String, Object> root, final String key, final String path) {
    final String value = optionalExactString(root, key, path);
    return value == null ? null : currentTypeName(value, path);
  }

  private static String currentTypeName(final String value, final String path) {
    final List<Integer> braceGenericDepths = new ArrayList<>();
    int genericDepth = 0;
    int index = 0;
    while (index < value.length()) {
      final char character = value.charAt(index);
      if (character == '{') {
        braceGenericDepths.add(genericDepth);
        index++;
      } else if (character == '}') {
        if (!braceGenericDepths.isEmpty()) {
          braceGenericDepths.remove(braceGenericDepths.size() - 1);
        }
        index++;
      } else if (character == '<') {
        genericDepth++;
        index++;
      } else if (character == '>') {
        genericDepth = Math.max(0, genericDepth - 1);
        index++;
      } else if (isTypeIdentifierStart(character)) {
        final int start = index++;
        while (index < value.length() && isTypeIdentifierPart(value.charAt(index))) {
          index++;
        }
        final String identifier = value.substring(start, index);
        int next = index;
        while (next < value.length() && Character.isWhitespace(value.charAt(next))) {
          next++;
        }
        int afterColon = next + 1;
        while (afterColon < value.length()
            && Character.isWhitespace(value.charAt(afterColon))) {
          afterColon++;
        }
        int previous = start - 1;
        while (previous >= 0 && Character.isWhitespace(value.charAt(previous))) {
          previous--;
        }
        final boolean isStructField =
            !braceGenericDepths.isEmpty()
                && braceGenericDepths.get(braceGenericDepths.size() - 1) == genericDepth
                && previous >= 0
                && (value.charAt(previous) == '{' || value.charAt(previous) == ',')
                && next < value.length()
                && value.charAt(next) == ':'
                && (afterColon >= value.length() || value.charAt(afterColon) != ':');
        check(
            isStructField || !RETIRED_NUMERIC_TYPE_NAMES.contains(identifier),
            path + " must not use retired Kotodama numeric type name `" + identifier + "`");
      } else {
        check(
            character <= 0x7f,
            path + " must use ASCII Kotodama type identifiers");
        index++;
      }
    }
    return value;
  }

  private static boolean isTypeIdentifierStart(final char value) {
    return value == '_' || value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z';
  }

  private static boolean isTypeIdentifierPart(final char value) {
    return isTypeIdentifierStart(value) || value >= '0' && value <= '9';
  }

  private static boolean canonicalEntrypointName(final String value) {
    return "hajimari".equals(value)
        || "始まり".equals(value)
        || "kaizen".equals(value)
        || "改善".equals(value)
        || canonicalDeclarationIdentifier(value);
  }

  private static void unique(final List<String> values, final String path) {
    check(
        new HashSet<>(values).size() == values.size(),
        path + " must not contain duplicate identifiers");
  }

  private static String join(final List<String> values, final String delimiter) {
    final StringBuilder result = new StringBuilder();
    for (int index = 0; index < values.size(); index++) {
      if (index > 0) {
        result.append(delimiter);
      }
      result.append(values.get(index));
    }
    return result.toString();
  }

  private static boolean equalsNullable(final Object left, final Object right) {
    return left == null ? right == null : left.equals(right);
  }

  private static <T> T requiredValue(final T value, final String field) {
    check(value != null, "missing " + field);
    return value;
  }

  private static void check(final boolean condition, final String message) {
    if (!condition) {
      throw new IllegalStateException(message);
    }
  }

  private static Set<String> set(final String... values) {
    return new HashSet<>(Arrays.asList(values));
  }

  private static Map<String, ContractManifest.ValueKindV1> valueKinds() {
    final Map<String, ContractManifest.ValueKindV1> kinds = new HashMap<>();
    kinds.put("Int", ContractManifest.ValueKindV1.INT);
    kinds.put("Decimal", ContractManifest.ValueKindV1.DECIMAL);
    kinds.put("Quantity", ContractManifest.ValueKindV1.QUANTITY);
    kinds.put("Bool", ContractManifest.ValueKindV1.BOOL);
    kinds.put("String", ContractManifest.ValueKindV1.STRING);
    kinds.put("Json", ContractManifest.ValueKindV1.JSON);
    kinds.put("Name", ContractManifest.ValueKindV1.NAME);
    kinds.put("AccountId", ContractManifest.ValueKindV1.ACCOUNT_ID);
    kinds.put("AssetDefinitionId", ContractManifest.ValueKindV1.ASSET_DEFINITION_ID);
    kinds.put("AssetId", ContractManifest.ValueKindV1.ASSET_ID);
    kinds.put("DomainId", ContractManifest.ValueKindV1.DOMAIN_ID);
    kinds.put("NftId", ContractManifest.ValueKindV1.NFT_ID);
    kinds.put("DataSpaceId", ContractManifest.ValueKindV1.DATA_SPACE_ID);
    kinds.put("Blob", ContractManifest.ValueKindV1.BLOB);
    return kinds;
  }

  private static final class TypeAnalysis {
    private final int nextIndex;
    private final int nodeCount;
    private final int wordCount;
    private final int maxDepth;
    private final String typeName;

    private TypeAnalysis(
        final int nextIndex,
        final int nodeCount,
        final int wordCount,
        final int maxDepth,
        final String typeName) {
      this.nextIndex = nextIndex;
      this.nodeCount = nodeCount;
      this.wordCount = wordCount;
      this.maxDepth = maxDepth;
      this.typeName = typeName;
    }
  }

  private static final class TraversalFrame {
    private int remaining;
    private final boolean suppressWords;

    private TraversalFrame(final int remaining, final boolean suppressWords) {
      this.remaining = remaining;
      this.suppressWords = suppressWords;
    }
  }

  private static final class RenderedType {
    private final String typeName;
    private final String coreViewName;
    private final String listElementCoreViewName;

    private RenderedType(
        final String typeName,
        final String coreViewName,
        final String listElementCoreViewName) {
      this.typeName = typeName;
      this.coreViewName = coreViewName;
      this.listElementCoreViewName = listElementCoreViewName;
    }
  }

  private static final class CoreViewRange {
    private final int end;

    private CoreViewRange(final int end) {
      this.end = end;
    }
  }
}
