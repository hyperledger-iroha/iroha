package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Base64
import java.util.Collections
import java.util.LinkedHashMap
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address

/** Branded Kotodama V1 entrypoint categories carried by contract manifests. */
enum class ContractEntrypointKind {
    KOTOAGE,
    VIEW,
    HAJIMARI,
    KAIZEN,
}

/** Scalar and pointer leaves supported by an exact Kotodama V1 boundary schema. */
enum class EntrypointValueKindV1 {
    INT,
    DECIMAL,
    QUANTITY,
    BOOL,
    STRING,
    JSON,
    NAME,
    ACCOUNT_ID,
    ASSET_DEFINITION_ID,
    ASSET_ID,
    DOMAIN_ID,
    NFT_ID,
    DATA_SPACE_ID,
    BLOB,
}

/** Tree-node categories encoded on an exact flat preorder Kotodama V1 schema tape. */
enum class EntrypointValueTypeNodeKindV1 {
    STRUCT,
    TUPLE,
    OPTION,
    RESULT,
    LIST,
    LEAF,
}

/** Named product metadata for one exact boundary-schema node. */
class EntrypointStructTypeNodeV1(
    @JvmField val name: String,
    fields: List<String>,
) {
    @JvmField val fields: List<String> = Collections.unmodifiableList(ArrayList(fields))
}

/** Bounded-list metadata; its element subtree immediately follows in the enclosing node tape. */
class EntrypointListTypeNodeV1(
    @JvmField val capacity: Int,
)

/** One validated preorder node in an exact Kotodama V1 boundary schema. */
class EntrypointValueTypeNodeV1(
    @JvmField val kind: EntrypointValueTypeNodeKindV1,
    @JvmField val structValue: EntrypointStructTypeNodeV1? = null,
    @JvmField val tupleArity: Int? = null,
    @JvmField val listValue: EntrypointListTypeNodeV1? = null,
    @JvmField val leafKind: EntrypointValueKindV1? = null,
)

/** Exact flat preorder value schema used at a Kotodama V1 public boundary. */
class EntrypointValueTypeV1 internal constructor(
    nodes: List<EntrypointValueTypeNodeV1>,
    @JvmField val wordCount: Int,
    @JvmField val canonicalTypeName: String,
) {
    @JvmField val nodes: List<EntrypointValueTypeNodeV1> =
        Collections.unmodifiableList(ArrayList(nodes))
}

/** One named field in a canonical V1 entrypoint argument record. */
class EntrypointArgumentFieldV1(
    @JvmField val name: String,
    @JvmField val valueType: EntrypointValueTypeV1,
)

/** Exact canonical V1 schema for one public entrypoint argument record. */
class EntrypointArgumentSchemaV1(
    fields: List<EntrypointArgumentFieldV1>,
    @JvmField val wordCount: Int,
) {
    @JvmField val fields: List<EntrypointArgumentFieldV1> =
        Collections.unmodifiableList(ArrayList(fields))
}

/** One public parameter advertised by a Kotodama manifest. */
class ContractEntrypointParameter(
    @JvmField val name: String,
    @JvmField val typeName: String,
)

/** One compiler-advertised bounded dynamic state access. */
class ContractDynamicAccessHint(
    @JvmField val baseKey: String,
    @JvmField val keyType: String,
    @JvmField val boundKind: String,
    @JvmField val maxKeys: Long,
)

/** Static and bounded-dynamic scheduler hints in a contract manifest. */
class ContractAccessSetHints(
    readKeys: List<String>,
    writeKeys: List<String>,
    dynamicReads: List<ContractDynamicAccessHint>,
    dynamicWrites: List<ContractDynamicAccessHint>,
) {
    @JvmField val readKeys: List<String> = Collections.unmodifiableList(ArrayList(readKeys))
    @JvmField val writeKeys: List<String> = Collections.unmodifiableList(ArrayList(writeKeys))
    @JvmField val dynamicReads: List<ContractDynamicAccessHint> =
        Collections.unmodifiableList(ArrayList(dynamicReads))
    @JvmField val dynamicWrites: List<ContractDynamicAccessHint> =
        Collections.unmodifiableList(ArrayList(dynamicWrites))
}

/** Trigger repetition policy encoded by the Rust `Repeats` enum. */
enum class ContractTriggerRepeatsKind {
    INDEFINITELY,
    EXACTLY,
}

/** Exact trigger repetition policy in a manifest entrypoint descriptor. */
class ContractTriggerRepeats(
    @JvmField val kind: ContractTriggerRepeatsKind,
    @JvmField val exactly: Long?,
)

/** Callback target for a manifest trigger. */
class ContractTriggerCallback(
    @JvmField val namespace: String?,
    @JvmField val entrypoint: String,
)

/** Complete trigger metadata attached to one manifest entrypoint. */
class ContractTriggerDescriptor(
    @JvmField val id: String,
    @JvmField val repeats: ContractTriggerRepeats,
    @JvmField val filterBase64: String,
    @JvmField val authority: String?,
    metadata: Map<String, Any?>,
    @JvmField val callback: ContractTriggerCallback,
) {
    @JvmField val metadata: Map<String, Any?> = immutableJsonObject(metadata)
}

/** Exact public interface metadata for one Kotodama entrypoint. */
class ContractEntrypointDescriptor(
    @JvmField val name: String,
    @JvmField val kind: ContractEntrypointKind,
    parameters: List<ContractEntrypointParameter>,
    @JvmField val argumentSchema: EntrypointArgumentSchemaV1?,
    @JvmField val returnType: String?,
    @JvmField val returnSchema: EntrypointValueTypeV1?,
    @JvmField val permission: String?,
    readKeys: List<String>,
    writeKeys: List<String>,
    @JvmField val accessHintsComplete: Boolean?,
    accessHintsSkipped: List<String>,
    triggers: List<ContractTriggerDescriptor>,
) {
    @JvmField val parameters: List<ContractEntrypointParameter> =
        Collections.unmodifiableList(ArrayList(parameters))
    @JvmField val readKeys: List<String> = Collections.unmodifiableList(ArrayList(readKeys))
    @JvmField val writeKeys: List<String> = Collections.unmodifiableList(ArrayList(writeKeys))
    @JvmField val accessHintsSkipped: List<String> =
        Collections.unmodifiableList(ArrayList(accessHintsSkipped))
    @JvmField val triggers: List<ContractTriggerDescriptor> =
        Collections.unmodifiableList(ArrayList(triggers))
}

/** One durable state slot advertised by a Kotodama seiyaku. */
class ContractStateDescriptor(
    @JvmField val name: String,
    @JvmField val typeName: String,
)

/** One stable application error code declared by a Kotodama seiyaku. */
class ContractErrorCodeDescriptor(
    @JvmField val namespace: String,
    @JvmField val name: String,
    @JvmField val code: Long,
)

/** One localized text in a `kotoba` manifest table. */
class ContractKotobaTranslation(
    @JvmField val language: String,
    @JvmField val text: String,
)

/** One stable message identifier and all of its localized texts. */
class ContractKotobaTranslationEntry(
    @JvmField val messageId: String,
    translations: List<ContractKotobaTranslation>,
) {
    @JvmField val translations: List<ContractKotobaTranslation> =
        Collections.unmodifiableList(ArrayList(translations))
}

/** Signature metadata binding a manifest to its approved signer. */
class ContractManifestProvenance(
    @JvmField val signer: String,
    @JvmField val signature: String,
)

/** Full on-chain `ContractManifest` returned by Torii. */
class ContractManifest(
    @JvmField val seiyakuName: String?,
    @JvmField val codeHashHex: String?,
    @JvmField val abiHashHex: String?,
    @JvmField val compilerFingerprint: String?,
    @JvmField val featuresBitmap: BigInteger?,
    @JvmField val accessSetHints: ContractAccessSetHints?,
    entrypoints: List<ContractEntrypointDescriptor>?,
    states: List<ContractStateDescriptor>?,
    errorCodes: List<ContractErrorCodeDescriptor>?,
    kotoba: List<ContractKotobaTranslationEntry>?,
    @JvmField val provenance: ContractManifestProvenance?,
) {
    @JvmField val entrypoints: List<ContractEntrypointDescriptor>? = entrypoints?.let {
        Collections.unmodifiableList(ArrayList(it))
    }
    @JvmField val states: List<ContractStateDescriptor>? = states?.let {
        Collections.unmodifiableList(ArrayList(it))
    }
    @JvmField val errorCodes: List<ContractErrorCodeDescriptor>? = errorCodes?.let {
        Collections.unmodifiableList(ArrayList(it))
    }
    @JvmField val kotoba: List<ContractKotobaTranslationEntry>? = kotoba?.let {
        Collections.unmodifiableList(ArrayList(it))
    }
}

/** Full response from `GET /v1/contracts/code/{code_hash}`. */
class ContractManifestRecord(
    @JvmField val manifest: ContractManifest,
    @JvmField val codeHashHex: String?,
    @JvmField val abiHashHex: String?,
)

/** Strict parser for the full Rust `ContractManifest` JSON shape. */
object ContractManifestJsonParser {
    private val maxU64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val reservedIdentifiers = setOf(
        "authorize", "break", "const", "continue", "else", "enum", "error", "false",
        "fn", "for", "hajimari", "if", "in", "int", "decimal", "quantity", "kaizen", "kotoage", "let", "match", "module",
        "return", "seiyaku", "state", "struct", "trigger", "true", "var", "view",
    )
    private val reservedDeclarationNames = setOf(
        "int", "decimal", "quantity", "bool", "string", "bytes", "Json", "AccountId",
        "AssetDefinitionId", "AssetId", "DomainId", "Name", "NftId", "DataSpaceId",
        "Option", "Result", "List", "StateMap", "Secret", "AccountView", "AssetView",
        "AssetDefinitionView", "DomainView", "NftView", "QueryPage", "AxtDescriptor",
        "AssetHandle", "ProofBlob", "SoracloudRequest", "SoracloudResponse",
        "state_map_get", "__kotodama_list_len", "__kotodama_list_get",
        "__kotodama_list_try_set", "__kotodama_list_try_push", "__kotodama_list_pop",
        "__kotodama_list_contains", "__kotodama_list_take", "__kotodama_list_enumerate",
        "__kotodama_decimal_div_round", "__kotodama_quantity_div_round",
        "__kotodama_quantity_ratio_round", "__kotodama_decimal_to_int_trunc",
        "__kotodama_decimal_to_int_round",
    )
    private val valueKindByWire = mapOf(
        "Int" to EntrypointValueKindV1.INT,
        "Decimal" to EntrypointValueKindV1.DECIMAL,
        "Quantity" to EntrypointValueKindV1.QUANTITY,
        "Bool" to EntrypointValueKindV1.BOOL,
        "String" to EntrypointValueKindV1.STRING,
        "Json" to EntrypointValueKindV1.JSON,
        "Name" to EntrypointValueKindV1.NAME,
        "AccountId" to EntrypointValueKindV1.ACCOUNT_ID,
        "AssetDefinitionId" to EntrypointValueKindV1.ASSET_DEFINITION_ID,
        "AssetId" to EntrypointValueKindV1.ASSET_ID,
        "DomainId" to EntrypointValueKindV1.DOMAIN_ID,
        "NftId" to EntrypointValueKindV1.NFT_ID,
        "DataSpaceId" to EntrypointValueKindV1.DATA_SPACE_ID,
        "Blob" to EntrypointValueKindV1.BLOB,
    )

    /** Parse and validate one complete Torii contract-manifest record. */
    @JvmStatic
    fun parseRecord(payload: ByteArray): ContractManifestRecord {
        val root = objectValue(parse(payload, "contract manifest response"), "contract manifest response")
        exactKeys(root, setOf("manifest", "code_hash", "abi_hash"), "contract manifest response")
        val manifest = parseManifest(
            objectValue(required(root, "manifest", "contract manifest response"), "contract manifest response.manifest"),
        )
        val codeHash = optionalConvenienceHash(root, "code_hash", "contract manifest response.code_hash")
        val abiHash = optionalConvenienceHash(root, "abi_hash", "contract manifest response.abi_hash")
        check(codeHash == manifest.codeHashHex) {
            "contract manifest response.code_hash must exactly match manifest.code_hash"
        }
        check(abiHash == manifest.abiHashHex) {
            "contract manifest response.abi_hash must exactly match manifest.abi_hash"
        }
        return ContractManifestRecord(manifest, codeHash, abiHash)
    }

    /** Parse and validate one full Rust `ContractManifest` object. */
    @JvmStatic
    fun parseManifest(root: Map<String, Any?>): ContractManifest {
        exactKeys(
            root,
            setOf(
                "seiyaku_name", "code_hash", "abi_hash", "compiler_fingerprint",
                "features_bitmap", "access_set_hints", "entrypoints", "states", "error_codes",
                "kotoba", "provenance",
            ),
            "manifest",
        )
        val seiyakuName = optionalExactString(root, "seiyaku_name", "manifest.seiyaku_name")
        if (seiyakuName != null) {
            check(canonicalDeclarationIdentifier(seiyakuName)) {
                "manifest.seiyaku_name must be a canonical Kotodama identifier"
            }
        }
        val codeHash = optionalManifestHash(root, "code_hash", "manifest.code_hash")
        val abiHash = optionalManifestHash(root, "abi_hash", "manifest.abi_hash")
        val compilerFingerprint = optionalExactString(
            root,
            "compiler_fingerprint",
            "manifest.compiler_fingerprint",
        )
        val featuresBitmap = if (!root.containsKey("features_bitmap") || root["features_bitmap"] == null) {
            null
        } else {
            unsignedInteger(root["features_bitmap"], maxU64, "manifest.features_bitmap")
        }
        check(featuresBitmap == null || featuresBitmap <= BigInteger.valueOf(3)) {
            "manifest.features_bitmap contains unsupported Kotodama V1 bits"
        }
        val accessSetHints = optionalObject(root, "access_set_hints", "manifest.access_set_hints")
            ?.let(::parseAccessSetHints)
        val entrypoints = optionalObjectList(root, "entrypoints", "manifest.entrypoints", ::parseEntrypoint)
        val states = optionalObjectList(root, "states", "manifest.states", ::parseState)
        val errorCodes = optionalObjectList(root, "error_codes", "manifest.error_codes", ::parseErrorCode)
        val kotoba = optionalObjectList(root, "kotoba", "manifest.kotoba", ::parseKotobaEntry)
        val provenance = optionalObject(root, "provenance", "manifest.provenance")?.let(::parseProvenance)

        entrypoints?.let {
            val names = it.map { descriptor -> descriptor.name }
            requireUnique(names, "manifest.entrypoints")
            check(it.count { descriptor -> descriptor.kind == ContractEntrypointKind.HAJIMARI } <= 1) {
                "manifest.entrypoints must not declare multiple hajimari entrypoints"
            }
            check(it.count { descriptor -> descriptor.kind == ContractEntrypointKind.KAIZEN } <= 1) {
                "manifest.entrypoints must not declare multiple kaizen entrypoints"
            }
            val declared = it.associate { descriptor -> descriptor.name to descriptor.kind }
            val triggerIds = mutableSetOf<String>()
            it.flatMap { descriptor -> descriptor.triggers }.forEach { trigger ->
                check(triggerIds.add(trigger.id)) { "manifest trigger ids must be globally unique" }
                check(
                    trigger.callback.namespace != null ||
                        declared[trigger.callback.entrypoint] == ContractEntrypointKind.KOTOAGE,
                ) {
                    "manifest local trigger callback must name a declared kotoage entrypoint"
                }
            }
        }
        states?.let { requireUnique(it.map { descriptor -> descriptor.name }, "manifest.states") }
        errorCodes?.let {
            requireUnique(it.map { descriptor -> "${descriptor.namespace}::${descriptor.name}" }, "manifest.error_codes")
            requireUnique(it.map { descriptor -> descriptor.code.toString() }, "manifest.error_codes.code")
        }
        kotoba?.let { requireUnique(it.map { entry -> entry.messageId }, "manifest.kotoba") }

        return ContractManifest(
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
            provenance,
        )
    }

    private fun parseAccessSetHints(root: Map<String, Any?>): ContractAccessSetHints {
        exactKeys(root, setOf("read_keys", "write_keys", "dynamic_reads", "dynamic_writes"), "manifest.access_set_hints")
        return ContractAccessSetHints(
            stringList(required(root, "read_keys", "manifest.access_set_hints"), "manifest.access_set_hints.read_keys"),
            stringList(required(root, "write_keys", "manifest.access_set_hints"), "manifest.access_set_hints.write_keys"),
            objectList(root["dynamic_reads"] ?: emptyList<Any?>(), "manifest.access_set_hints.dynamic_reads", ::parseDynamicHint),
            objectList(root["dynamic_writes"] ?: emptyList<Any?>(), "manifest.access_set_hints.dynamic_writes", ::parseDynamicHint),
        )
    }

    private fun parseDynamicHint(root: Map<String, Any?>): ContractDynamicAccessHint {
        exactKeys(root, setOf("base_key", "key_type", "bound_kind", "max_keys"), "dynamic access hint")
        val baseKey = exactString(required(root, "base_key", "dynamic access hint"), "dynamic access hint.base_key")
        check(baseKey.startsWith("state:") && baseKey != "state:*") {
            "dynamic access hint.base_key must be a concrete state: key"
        }
        val maxKeys = unsignedInteger(
            required(root, "max_keys", "dynamic access hint"),
            BigInteger.valueOf(0xffff_ffffL),
            "dynamic access hint.max_keys",
        ).longValueExact()
        check(maxKeys > 0) { "dynamic access hint.max_keys must be positive" }
        return ContractDynamicAccessHint(
            baseKey,
            exactString(required(root, "key_type", "dynamic access hint"), "dynamic access hint.key_type"),
            exactString(required(root, "bound_kind", "dynamic access hint"), "dynamic access hint.bound_kind"),
            maxKeys,
        )
    }

    private fun parseEntrypoint(root: Map<String, Any?>): ContractEntrypointDescriptor {
        exactKeys(
            root,
            setOf(
                "name", "kind", "params", "argument_schema", "return_type", "return_schema",
                "permission", "read_keys", "write_keys", "access_hints_complete",
                "access_hints_skipped", "triggers",
            ),
            "entrypoint descriptor",
        )
        val name = exactString(required(root, "name", "entrypoint descriptor"), "entrypoint descriptor.name")
        check(canonicalEntrypointName(name)) {
            "entrypoint descriptor.name must be a canonical Kotodama identifier or branded lifecycle selector"
        }
        val kind = parseEntrypointKind(
            objectValue(required(root, "kind", "entrypoint descriptor"), "entrypoint descriptor.kind"),
        )
        check(
            (kind == ContractEntrypointKind.HAJIMARI && (name == "hajimari" || name == "始まり")) ||
                (kind == ContractEntrypointKind.KAIZEN && (name == "kaizen" || name == "改善")) ||
                (kind != ContractEntrypointKind.HAJIMARI && kind != ContractEntrypointKind.KAIZEN &&
                    name != "hajimari" && name != "始まり" && name != "kaizen" && name != "改善")
        ) { "entrypoint descriptor kind does not match its branded selector" }
        val parameters = objectList(root["params"] ?: emptyList<Any?>(), "entrypoint descriptor.params", ::parseParameter)
        check(parameters.size <= 13) { "entrypoint descriptor.params exceeds the V1 argument limit" }
        requireUnique(parameters.map { it.name }, "entrypoint descriptor.params")
        val argumentSchema = optionalObject(root, "argument_schema", "entrypoint descriptor.argument_schema")
            ?.let(::parseArgumentSchema)
        check(
            (parameters.isEmpty() && argumentSchema == null) ||
                (parameters.isNotEmpty() && argumentSchema != null &&
                    parameters.size == argumentSchema.fields.size &&
                    parameters.indices.all { index ->
                        parameters[index].name == argumentSchema.fields[index].name &&
                            parameters[index].typeName == argumentSchema.fields[index].valueType.canonicalTypeName
                    })
        ) { "entrypoint descriptor argument schema does not exactly match params" }
        val returnType = optionalExactString(root, "return_type", "entrypoint descriptor.return_type")
        val returnSchema = optionalObject(root, "return_schema", "entrypoint descriptor.return_schema")
            ?.let(::parseValueType)
        check((returnType == null) == (returnSchema == null)) {
            "entrypoint descriptor return_type and return_schema must be present together"
        }
        check(returnSchema == null || (returnSchema.wordCount <= 13 && returnSchema.canonicalTypeName == returnType)) {
            "entrypoint descriptor return schema does not exactly match return_type"
        }
        val permission = optionalExactString(root, "permission", "entrypoint descriptor.permission")
        check(kind != ContractEntrypointKind.KOTOAGE || permission != null) {
            "kotoage entrypoint descriptor must declare permission"
        }
        check((kind != ContractEntrypointKind.HAJIMARI && kind != ContractEntrypointKind.KAIZEN) || permission == null) {
            "hajimari and kaizen entrypoints use runtime-defined authorization"
        }
        val readKeys = stringList(root["read_keys"] ?: emptyList<Any?>(), "entrypoint descriptor.read_keys")
        val writeKeys = stringList(root["write_keys"] ?: emptyList<Any?>(), "entrypoint descriptor.write_keys")
        val complete = optionalBoolean(root, "access_hints_complete", "entrypoint descriptor.access_hints_complete")
        val skipped = stringList(root["access_hints_skipped"] ?: emptyList<Any?>(), "entrypoint descriptor.access_hints_skipped")
        check(complete != true || skipped.isEmpty()) {
            "complete access hints must not contain skipped reasons"
        }
        check(complete != false || skipped.isNotEmpty()) {
            "incomplete access hints must contain a skipped reason"
        }
        val triggers = objectList(root["triggers"] ?: emptyList<Any?>(), "entrypoint descriptor.triggers", ::parseTrigger)
        return ContractEntrypointDescriptor(
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
            triggers,
        )
    }

    private fun parseEntrypointKind(root: Map<String, Any?>): ContractEntrypointKind {
        exactKeys(root, setOf("kind", "value"), "entrypoint descriptor.kind")
        check(root.containsKey("value") && root["value"] == null) {
            "entrypoint descriptor.kind.value must be null"
        }
        return when (exactString(required(root, "kind", "entrypoint descriptor.kind"), "entrypoint descriptor.kind.kind")) {
            "Kotoage" -> ContractEntrypointKind.KOTOAGE
            "View" -> ContractEntrypointKind.VIEW
            "Hajimari" -> ContractEntrypointKind.HAJIMARI
            "Kaizen" -> ContractEntrypointKind.KAIZEN
            else -> error("unsupported branded Kotodama entrypoint kind")
        }
    }

    private fun parseParameter(root: Map<String, Any?>): ContractEntrypointParameter {
        exactKeys(root, setOf("name", "type_name"), "entrypoint parameter")
        val name = exactString(required(root, "name", "entrypoint parameter"), "entrypoint parameter.name")
        check(canonicalSourceIdentifier(name)) { "entrypoint parameter.name must be a canonical Kotodama identifier" }
        return ContractEntrypointParameter(
            name,
            exactString(required(root, "type_name", "entrypoint parameter"), "entrypoint parameter.type_name"),
        )
    }

    private fun parseArgumentSchema(root: Map<String, Any?>): EntrypointArgumentSchemaV1 {
        exactKeys(root, setOf("fields"), "entrypoint argument schema")
        val fields = objectList(required(root, "fields", "entrypoint argument schema"), "entrypoint argument schema.fields", ::parseArgumentField)
        check(fields.isNotEmpty() && fields.size <= 13) {
            "entrypoint argument schema must contain 1..13 fields"
        }
        requireUnique(fields.map { it.name }, "entrypoint argument schema.fields")
        val words = fields.fold(0) { total, field -> total + field.valueType.wordCount }
        check(words <= 13) { "entrypoint argument schema exceeds the V1 register window" }
        return EntrypointArgumentSchemaV1(fields, words)
    }

    private fun parseArgumentField(root: Map<String, Any?>): EntrypointArgumentFieldV1 {
        exactKeys(root, setOf("name", "ty"), "entrypoint argument field")
        val name = exactString(required(root, "name", "entrypoint argument field"), "entrypoint argument field.name")
        check(canonicalSourceIdentifier(name)) { "entrypoint argument field.name must be a canonical Kotodama identifier" }
        return EntrypointArgumentFieldV1(
            name,
            parseValueType(objectValue(required(root, "ty", "entrypoint argument field"), "entrypoint argument field.ty")),
        )
    }

    private fun parseValueType(root: Map<String, Any?>): EntrypointValueTypeV1 {
        exactKeys(root, setOf("nodes"), "entrypoint value type")
        val nodes = objectList(required(root, "nodes", "entrypoint value type"), "entrypoint value type.nodes", ::parseValueTypeNode)
        check(nodes.isNotEmpty() && nodes.size <= 256) { "entrypoint value type must contain 1..256 nodes" }
        val analysis = analyzeValueType(nodes)
        check(
            analysis.nextIndex == nodes.size &&
                analysis.nodeCount <= 256 &&
                validateReservedNominalShapes(nodes),
        ) {
            "entrypoint value type is not one canonical flat preorder V1 schema"
        }
        return EntrypointValueTypeV1(nodes, analysis.wordCount, analysis.typeName)
    }

    private fun parseValueTypeNode(root: Map<String, Any?>): EntrypointValueTypeNodeV1 {
        exactKeys(root, setOf("kind", "value"), "entrypoint value type node")
        check(root.containsKey("value")) { "entrypoint value type node.value is required" }
        return when (exactString(required(root, "kind", "entrypoint value type node"), "entrypoint value type node.kind")) {
            "Struct" -> EntrypointValueTypeNodeV1(
                EntrypointValueTypeNodeKindV1.STRUCT,
                structValue = parseStructNode(objectValue(root["value"], "entrypoint struct node")),
            )
            "Tuple" -> {
                val arity = unsignedInteger(root["value"], BigInteger.valueOf(0xffff), "entrypoint tuple arity").intValueExact()
                check(arity >= 2) { "entrypoint tuple arity must be in 2..65535" }
                EntrypointValueTypeNodeV1(EntrypointValueTypeNodeKindV1.TUPLE, tupleArity = arity)
            }
            "Option" -> {
                check(root["value"] == null) { "entrypoint Option node.value must be null" }
                EntrypointValueTypeNodeV1(EntrypointValueTypeNodeKindV1.OPTION)
            }
            "Result" -> {
                check(root["value"] == null) { "entrypoint Result node.value must be null" }
                EntrypointValueTypeNodeV1(EntrypointValueTypeNodeKindV1.RESULT)
            }
            "List" -> EntrypointValueTypeNodeV1(
                EntrypointValueTypeNodeKindV1.LIST,
                listValue = parseListNode(objectValue(root["value"], "entrypoint list node")),
            )
            "Leaf" -> EntrypointValueTypeNodeV1(
                EntrypointValueTypeNodeKindV1.LEAF,
                leafKind = parseLeafKind(objectValue(root["value"], "entrypoint value kind")),
            )
            else -> error("unsupported Kotodama boundary type node")
        }
    }

    private fun parseStructNode(root: Map<String, Any?>): EntrypointStructTypeNodeV1 {
        exactKeys(root, setOf("name", "fields"), "entrypoint struct node")
        val name = exactString(required(root, "name", "entrypoint struct node"), "entrypoint struct node.name")
        val fields = stringList(required(root, "fields", "entrypoint struct node"), "entrypoint struct node.fields")
        check(canonicalSourceIdentifier(name) && fields.isNotEmpty() && fields.all(::canonicalSourceIdentifier)) {
            "entrypoint struct node must use canonical Kotodama identifiers"
        }
        requireUnique(fields, "entrypoint struct node.fields")
        return EntrypointStructTypeNodeV1(name, fields)
    }

    private fun parseListNode(root: Map<String, Any?>): EntrypointListTypeNodeV1 {
        exactKeys(root, setOf("capacity"), "entrypoint list node")
        val capacity = unsignedInteger(
            required(root, "capacity", "entrypoint list node"),
            BigInteger.valueOf(64),
            "entrypoint list node.capacity",
        ).intValueExact()
        check(capacity >= 1) { "entrypoint list node.capacity must be in 1..64" }
        return EntrypointListTypeNodeV1(capacity)
    }

    private fun parseLeafKind(root: Map<String, Any?>): EntrypointValueKindV1 {
        exactKeys(root, setOf("kind", "value"), "entrypoint value kind")
        check(root.containsKey("value") && root["value"] == null) {
            "entrypoint value kind.value must be null"
        }
        val label = exactString(required(root, "kind", "entrypoint value kind"), "entrypoint value kind.kind")
        return valueKindByWire[label] ?: error("unsupported Kotodama boundary value kind")
    }

    private data class TypeAnalysis(
        val nextIndex: Int,
        val nodeCount: Int,
        val wordCount: Int,
        val maxDepth: Int,
        val typeName: String,
    )

    private data class TraversalFrame(
        var remaining: Int,
        val suppressWords: Boolean,
    )

    private data class RenderedType(
        val typeName: String,
        val coreViewName: String? = null,
        val listElementCoreViewName: String? = null,
    )

    private fun analyzeValueType(nodes: List<EntrypointValueTypeNodeV1>): TypeAnalysis {
        val frames = ArrayList<TraversalFrame>()
        var words = 0
        var maxDepth = 0
        nodes.forEachIndexed { index, node ->
            while (frames.lastOrNull()?.remaining == 0) {
                frames.removeAt(frames.lastIndex)
            }
            val suppressWords = if (index == 0) {
                false
            } else {
                val parent = frames.lastOrNull()
                check(parent != null && parent.remaining > 0) {
                    "entrypoint value type contains a trailing preorder node"
                }
                parent.remaining -= 1
                parent.suppressWords
            }
            val depth = frames.size + 1
            check(depth <= 256) { "entrypoint value type exceeds the V1 nesting depth" }
            maxDepth = maxOf(maxDepth, depth)

            val handle = node.kind == EntrypointValueTypeNodeKindV1.OPTION ||
                node.kind == EntrypointValueTypeNodeKindV1.RESULT ||
                node.kind == EntrypointValueTypeNodeKindV1.LIST
            if (!suppressWords && (handle || node.kind == EntrypointValueTypeNodeKindV1.LEAF)) {
                words += 1
            }
            val children = nodeChildCount(node)
            if (children != 0) {
                frames.add(TraversalFrame(children, suppressWords || handle))
            }
        }
        while (frames.lastOrNull()?.remaining == 0) {
            frames.removeAt(frames.lastIndex)
        }
        check(frames.isEmpty()) { "entrypoint value type ends before its preorder tree is complete" }

        val rendered = ArrayList<RenderedType>()
        nodes.asReversed().forEach { node ->
            val childCount = nodeChildCount(node)
            check(rendered.size >= childCount) {
                "entrypoint value type ends before its preorder tree is complete"
            }
            val children = ArrayList<RenderedType>(childCount)
            repeat(childCount) { children.add(rendered.removeAt(rendered.lastIndex)) }
            val value = when (node.kind) {
                EntrypointValueTypeNodeKindV1.STRUCT -> {
                    val struct = checkNotNull(node.structValue) { "missing struct node metadata" }
                    when {
                        struct.name == "QueryPage" -> children.firstOrNull()?.listElementCoreViewName
                            ?.let { RenderedType("QueryPage<$it>") }
                            ?: RenderedType("struct QueryPage")
                        isCoreQueryViewName(struct.name) ->
                            RenderedType(struct.name, coreViewName = struct.name)
                        else -> RenderedType("struct ${struct.name}")
                    }
                }
                EntrypointValueTypeNodeKindV1.TUPLE ->
                    RenderedType("(${children.joinToString(", ") { it.typeName }})")
                EntrypointValueTypeNodeKindV1.OPTION ->
                    RenderedType("Option<${children.single().typeName}>")
                EntrypointValueTypeNodeKindV1.RESULT ->
                    RenderedType("Result<${children[0].typeName}, ${children[1].typeName}>")
                EntrypointValueTypeNodeKindV1.LIST -> {
                    val list = checkNotNull(node.listValue) { "missing list node metadata" }
                    val child = children.single()
                    RenderedType(
                        "List<${child.typeName}, ${list.capacity}>",
                        listElementCoreViewName = child.coreViewName,
                    )
                }
                EntrypointValueTypeNodeKindV1.LEAF ->
                    RenderedType(canonicalLeafName(checkNotNull(node.leafKind) { "missing leaf kind" }))
            }
            rendered.add(value)
        }
        check(rendered.size == 1) { "entrypoint value type is not one canonical preorder tree" }
        return TypeAnalysis(nodes.size, nodes.size, words, maxDepth, rendered.single().typeName)
    }

    private fun isCoreQueryViewName(name: String): Boolean = name in setOf(
        "AccountView",
        "AssetView",
        "AssetDefinitionView",
        "DomainView",
        "NftView",
    )

    private fun nodeChildCount(node: EntrypointValueTypeNodeV1): Int = when (node.kind) {
        EntrypointValueTypeNodeKindV1.STRUCT -> checkNotNull(node.structValue).fields.size
        EntrypointValueTypeNodeKindV1.TUPLE -> checkNotNull(node.tupleArity)
        EntrypointValueTypeNodeKindV1.OPTION, EntrypointValueTypeNodeKindV1.LIST -> 1
        EntrypointValueTypeNodeKindV1.RESULT -> 2
        EntrypointValueTypeNodeKindV1.LEAF -> 0
    }

    private fun subtreeEnd(nodes: List<EntrypointValueTypeNodeV1>, start: Int): Int? {
        var index = start
        var pending = 1
        while (pending != 0) {
            val node = nodes.getOrNull(index) ?: return null
            index += 1
            pending = pending - 1 + nodeChildCount(node)
        }
        return index
    }

    private data class CoreViewRange(val end: Int)

    private fun coreQueryViewRange(
        nodes: List<EntrypointValueTypeNodeV1>,
        start: Int,
    ): CoreViewRange? {
        val root = nodes.getOrNull(start)
        if (root?.kind != EntrypointValueTypeNodeKindV1.STRUCT) return null
        val struct = root.structValue ?: return null
        val expected = when (struct.name) {
            "AccountView" -> listOf(
                "id" to EntrypointValueKindV1.ACCOUNT_ID,
                "metadata" to EntrypointValueKindV1.JSON,
            )
            "AssetView" -> listOf(
                "id" to EntrypointValueKindV1.ASSET_ID,
                "amount" to EntrypointValueKindV1.QUANTITY,
            )
            "DomainView" -> listOf(
                "id" to EntrypointValueKindV1.DOMAIN_ID,
                "owned_by" to EntrypointValueKindV1.ACCOUNT_ID,
                "metadata" to EntrypointValueKindV1.JSON,
            )
            "NftView" -> listOf(
                "id" to EntrypointValueKindV1.NFT_ID,
                "owned_by" to EntrypointValueKindV1.ACCOUNT_ID,
                "content" to EntrypointValueKindV1.JSON,
            )
            "AssetDefinitionView" -> null
            else -> return null
        }
        if (struct.name == "AssetDefinitionView") {
            if (
                struct.fields != listOf(
                    "id",
                    "name",
                    "description",
                    "owned_by",
                    "total_quantity",
                    "metadata",
                ) ||
                !leafAt(nodes, start + 1, EntrypointValueKindV1.ASSET_DEFINITION_ID) ||
                !leafAt(nodes, start + 2, EntrypointValueKindV1.STRING) ||
                nodes.getOrNull(start + 3)?.kind != EntrypointValueTypeNodeKindV1.OPTION ||
                !leafAt(nodes, start + 4, EntrypointValueKindV1.STRING) ||
                !leafAt(nodes, start + 5, EntrypointValueKindV1.ACCOUNT_ID) ||
                !leafAt(nodes, start + 6, EntrypointValueKindV1.QUANTITY) ||
                !leafAt(nodes, start + 7, EntrypointValueKindV1.JSON) ||
                subtreeEnd(nodes, start) != start + 8
            ) {
                return null
            }
            return CoreViewRange(start + 8)
        }
        val fields = checkNotNull(expected)
        if (struct.fields != fields.map { it.first }) return null
        fields.forEachIndexed { offset, (_, kind) ->
            if (!leafAt(nodes, start + 1 + offset, kind)) return null
        }
        val end = start + 1 + fields.size
        return if (subtreeEnd(nodes, start) == end) CoreViewRange(end) else null
    }

    private fun leafAt(
        nodes: List<EntrypointValueTypeNodeV1>,
        index: Int,
        kind: EntrypointValueKindV1,
    ): Boolean = nodes.getOrNull(index)?.let { node ->
        node.kind == EntrypointValueTypeNodeKindV1.LEAF && node.leafKind == kind
    } == true

    private fun validateReservedNominalShapes(nodes: List<EntrypointValueTypeNodeV1>): Boolean {
        nodes.forEachIndexed { start, node ->
            if (node.kind != EntrypointValueTypeNodeKindV1.STRUCT) return@forEachIndexed
            val struct = node.structValue ?: return false
            if (isCoreQueryViewName(struct.name)) {
                if (coreQueryViewRange(nodes, start) == null) return false
                return@forEachIndexed
            }
            if (struct.name != "QueryPage") return@forEachIndexed
            if (struct.fields != listOf("items", "next_offset")) return false
            val rootEnd = subtreeEnd(nodes, start) ?: return false
            val listStart = start + 1
            val listNode = nodes.getOrNull(listStart) ?: return false
            if (
                listNode.kind != EntrypointValueTypeNodeKindV1.LIST ||
                listNode.listValue?.capacity != 64
            ) {
                return false
            }
            val view = coreQueryViewRange(nodes, listStart + 1) ?: return false
            if (subtreeEnd(nodes, listStart) != view.end) return false
            val nextOffset = view.end
            if (
                nodes.getOrNull(nextOffset)?.kind != EntrypointValueTypeNodeKindV1.OPTION ||
                !leafAt(nodes, nextOffset + 1, EntrypointValueKindV1.INT) ||
                subtreeEnd(nodes, nextOffset) != nextOffset + 2 ||
                rootEnd != nextOffset + 2
            ) {
                return false
            }
        }
        return true
    }

    private fun canonicalLeafName(kind: EntrypointValueKindV1): String = when (kind) {
        EntrypointValueKindV1.INT -> "int"
        EntrypointValueKindV1.DECIMAL -> "decimal"
        EntrypointValueKindV1.QUANTITY -> "quantity"
        EntrypointValueKindV1.BOOL -> "bool"
        EntrypointValueKindV1.STRING -> "string"
        EntrypointValueKindV1.JSON -> "Json"
        EntrypointValueKindV1.NAME -> "Name"
        EntrypointValueKindV1.ACCOUNT_ID -> "AccountId"
        EntrypointValueKindV1.ASSET_DEFINITION_ID -> "AssetDefinitionId"
        EntrypointValueKindV1.ASSET_ID -> "AssetId"
        EntrypointValueKindV1.DOMAIN_ID -> "DomainId"
        EntrypointValueKindV1.NFT_ID -> "NftId"
        EntrypointValueKindV1.DATA_SPACE_ID -> "DataSpaceId"
        EntrypointValueKindV1.BLOB -> "bytes"
    }

    private fun parseTrigger(root: Map<String, Any?>): ContractTriggerDescriptor {
        exactKeys(root, setOf("id", "repeats", "filter", "authority", "metadata", "callback"), "trigger descriptor")
        val id = exactString(required(root, "id", "trigger descriptor"), "trigger descriptor.id")
        val repeats = parseRepeats(objectValue(required(root, "repeats", "trigger descriptor"), "trigger descriptor.repeats"))
        val filter = canonicalBase64(required(root, "filter", "trigger descriptor"), "trigger descriptor.filter")
        val authority = optionalExactString(root, "authority", "trigger descriptor.authority")?.let {
            try {
                requireCanonicalI105Address(it, "trigger descriptor.authority")
            } catch (error: IllegalArgumentException) {
                throw IllegalStateException("trigger descriptor.authority must be a canonical I105 account id", error)
            }
        }
        val metadata = objectValue(required(root, "metadata", "trigger descriptor"), "trigger descriptor.metadata")
        val callback = parseCallback(objectValue(required(root, "callback", "trigger descriptor"), "trigger descriptor.callback"))
        return ContractTriggerDescriptor(id, repeats, filter, authority, metadata, callback)
    }

    private fun parseRepeats(root: Map<String, Any?>): ContractTriggerRepeats {
        check(root.size == 1) { "trigger descriptor.repeats must contain exactly one enum variant" }
        return when {
            root.containsKey("Indefinitely") -> {
                check(root["Indefinitely"] == null) { "Repeats.Indefinitely value must be null" }
                ContractTriggerRepeats(ContractTriggerRepeatsKind.INDEFINITELY, null)
            }
            root.containsKey("Exactly") -> ContractTriggerRepeats(
                ContractTriggerRepeatsKind.EXACTLY,
                unsignedInteger(root["Exactly"], BigInteger.valueOf(0xffff_ffffL), "Repeats.Exactly").longValueExact(),
            )
            else -> error("unsupported trigger repetition policy")
        }
    }

    private fun parseCallback(root: Map<String, Any?>): ContractTriggerCallback {
        exactKeys(root, setOf("namespace", "entrypoint"), "trigger callback")
        val namespace = optionalExactString(root, "namespace", "trigger callback.namespace")
        val entrypoint = exactString(required(root, "entrypoint", "trigger callback"), "trigger callback.entrypoint")
        check(canonicalEntrypointName(entrypoint)) { "trigger callback.entrypoint must be a canonical Kotodama selector" }
        return ContractTriggerCallback(namespace, entrypoint)
    }

    private fun parseState(root: Map<String, Any?>): ContractStateDescriptor {
        exactKeys(root, setOf("name", "type_name"), "state descriptor")
        val name = exactString(required(root, "name", "state descriptor"), "state descriptor.name")
        check(canonicalDeclarationIdentifier(name)) { "state descriptor.name must be a canonical Kotodama identifier" }
        return ContractStateDescriptor(
            name,
            exactString(required(root, "type_name", "state descriptor"), "state descriptor.type_name"),
        )
    }

    private fun parseErrorCode(root: Map<String, Any?>): ContractErrorCodeDescriptor {
        exactKeys(root, setOf("namespace", "name", "code"), "error code descriptor")
        val namespace = exactString(required(root, "namespace", "error code descriptor"), "error code descriptor.namespace")
        val name = exactString(required(root, "name", "error code descriptor"), "error code descriptor.name")
        check(canonicalDeclarationIdentifier(namespace) && canonicalSourceIdentifier(name)) {
            "error code namespace and name must be canonical Kotodama identifiers"
        }
        val code = unsignedInteger(
            required(root, "code", "error code descriptor"),
            BigInteger.valueOf(0xffff_ffffL),
            "error code descriptor.code",
        ).longValueExact()
        check(code > 0) { "error code descriptor.code must be a non-zero u32" }
        return ContractErrorCodeDescriptor(namespace, name, code)
    }

    private fun parseKotobaEntry(root: Map<String, Any?>): ContractKotobaTranslationEntry {
        exactKeys(root, setOf("msg_id", "translations"), "kotoba translation entry")
        val messageId = exactString(required(root, "msg_id", "kotoba translation entry"), "kotoba translation entry.msg_id")
        val translations = objectList(
            required(root, "translations", "kotoba translation entry"),
            "kotoba translation entry.translations",
            ::parseKotobaTranslation,
        )
        requireUnique(translations.map { it.language }, "kotoba translation entry.translations")
        return ContractKotobaTranslationEntry(messageId, translations)
    }

    private fun parseKotobaTranslation(root: Map<String, Any?>): ContractKotobaTranslation {
        exactKeys(root, setOf("lang", "text"), "kotoba translation")
        val text = required(root, "text", "kotoba translation")
        check(text is String) { "kotoba translation.text must be a string" }
        return ContractKotobaTranslation(
            exactString(required(root, "lang", "kotoba translation"), "kotoba translation.lang"),
            text,
        )
    }

    private fun parseProvenance(root: Map<String, Any?>): ContractManifestProvenance {
        exactKeys(root, setOf("signer", "signature"), "manifest.provenance")
        return ContractManifestProvenance(
            exactString(required(root, "signer", "manifest.provenance"), "manifest.provenance.signer"),
            exactString(required(root, "signature", "manifest.provenance"), "manifest.provenance.signature"),
        )
    }

    private fun parse(payload: ByteArray?, context: String): Any? {
        check(payload != null && payload.isNotEmpty()) { "$context returned an empty payload" }
        val json = String(payload, StandardCharsets.UTF_8)
        check(json.isNotBlank()) { "$context returned a blank payload" }
        return JsonParser.parse(json)
    }

    private fun required(root: Map<String, Any?>, name: String, context: String): Any? {
        check(root.containsKey(name)) { "$context.$name is required" }
        return root[name]
    }

    private fun optionalObject(root: Map<String, Any?>, name: String, path: String): Map<String, Any?>? {
        if (!root.containsKey(name) || root[name] == null) return null
        return objectValue(root[name], path)
    }

    private fun optionalExactString(root: Map<String, Any?>, name: String, path: String): String? {
        if (!root.containsKey(name) || root[name] == null) return null
        return exactString(root[name], path)
    }

    private fun optionalBoolean(root: Map<String, Any?>, name: String, path: String): Boolean? {
        if (!root.containsKey(name) || root[name] == null) return null
        val value = root[name]
        check(value is Boolean) { "$path must be a boolean" }
        return value
    }

    private fun optionalManifestHash(root: Map<String, Any?>, name: String, path: String): String? {
        if (!root.containsKey(name) || root[name] == null) return null
        return manifestHash(root[name], path)
    }

    private fun optionalConvenienceHash(root: Map<String, Any?>, name: String, path: String): String? {
        if (!root.containsKey(name) || root[name] == null) return null
        val value = root[name]
        check(value is String && value.matches(Regex("^[0-9a-f]{64}$"))) {
            "$path must be canonical lowercase 64-hex"
        }
        check(markerBitIsSet(value)) { "$path must set the Iroha Hash marker bit" }
        return value
    }

    private fun manifestHash(value: Any?, path: String): String {
        check(value is String) { "$path must be a canonical checksummed Norito Hash literal" }
        val match = Regex("^hash:([0-9A-F]{64})#([0-9A-F]{4})$").matchEntire(value)
            ?: error("$path must be a canonical checksummed Norito Hash literal")
        val body = match.groupValues[1]
        val supplied = match.groupValues[2].toInt(16)
        check(supplied == crc16("hash:$body".toByteArray(StandardCharsets.US_ASCII))) {
            "$path has an invalid Norito Hash checksum"
        }
        val normalized = body.lowercase(java.util.Locale.ROOT)
        check(markerBitIsSet(normalized)) { "$path must set the Iroha Hash marker bit" }
        return normalized
    }

    private fun crc16(bytes: ByteArray): Int {
        var crc = 0xffff
        for (byte in bytes) {
            crc = crc xor ((byte.toInt() and 0xff) shl 8)
            repeat(8) {
                crc = if (crc and 0x8000 != 0) ((crc shl 1) xor 0x1021) and 0xffff else (crc shl 1) and 0xffff
            }
        }
        return crc
    }

    private fun markerBitIsSet(hex: String): Boolean =
        hex.substring(hex.length - 2).toInt(16) and 1 == 1

    private fun canonicalBase64(value: Any?, path: String): String {
        val text = exactString(value, path)
        val bytes = try {
            Base64.getDecoder().decode(text)
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException("$path must be canonical base64", error)
        }
        check(bytes.isNotEmpty() && Base64.getEncoder().encodeToString(bytes) == text) {
            "$path must be non-empty canonical base64"
        }
        return text
    }

    private fun exactString(value: Any?, path: String): String {
        check(value is String && value.isNotBlank()) { "$path must be a non-empty string" }
        check(value.trim() == value) { "$path must not contain surrounding whitespace" }
        check(value.none { it.isISOControl() }) { "$path must not contain control characters" }
        return value
    }

    private fun stringList(value: Any?, path: String): List<String> =
        listValue(value, path).mapIndexed { index, item -> exactString(item, "$path[$index]") }

    private fun <T> optionalObjectList(
        root: Map<String, Any?>,
        name: String,
        path: String,
        parser: (Map<String, Any?>) -> T,
    ): List<T>? {
        if (!root.containsKey(name) || root[name] == null) return null
        return objectList(root[name], path, parser)
    }

    private fun <T> objectList(value: Any?, path: String, parser: (Map<String, Any?>) -> T): List<T> =
        listValue(value, path).mapIndexed { index, item -> parser(objectValue(item, "$path[$index]")) }

    private fun listValue(value: Any?, path: String): List<Any?> {
        check(value is List<*>) { "$path must be an array" }
        return value
    }

    @Suppress("UNCHECKED_CAST")
    private fun objectValue(value: Any?, path: String): Map<String, Any?> {
        check(value is Map<*, *>) { "$path must be an object" }
        check(value.keys.all { it is String }) { "$path must use string object keys" }
        return value as Map<String, Any?>
    }

    private fun exactKeys(root: Map<String, Any?>, allowed: Set<String>, path: String) {
        val unknown = root.keys.firstOrNull { it !in allowed }
        check(unknown == null) { "$path contains unknown field `$unknown`" }
    }

    private fun unsignedInteger(value: Any?, maximum: BigInteger, path: String): BigInteger {
        val integer = when (value) {
            is BigInteger -> value
            is Byte -> BigInteger.valueOf(value.toLong())
            is Short -> BigInteger.valueOf(value.toLong())
            is Int -> BigInteger.valueOf(value.toLong())
            is Long -> BigInteger.valueOf(value)
            else -> error("$path must be an unsigned integer")
        }
        check(integer.signum() >= 0 && integer <= maximum) { "$path is outside its unsigned integer range" }
        return integer
    }

    private fun canonicalIdentifierSyntax(value: String): Boolean {
        if (value.isEmpty() || !(value[0] == '_' || value[0] in 'A'..'Z' || value[0] in 'a'..'z')) return false
        return value.drop(1).all { it == '_' || it in 'A'..'Z' || it in 'a'..'z' || it in '0'..'9' }
    }

    private fun canonicalSourceIdentifier(value: String): Boolean =
        canonicalIdentifierSyntax(value) && value !in reservedIdentifiers

    private fun canonicalDeclarationIdentifier(value: String): Boolean =
        canonicalSourceIdentifier(value) &&
            value !in reservedDeclarationNames &&
            !value.startsWith("__kotodama_link_")

    private fun canonicalEntrypointName(value: String): Boolean =
        value == "hajimari" || value == "始まり" || value == "kaizen" || value == "改善" || canonicalSourceIdentifier(value)

    private fun requireUnique(values: List<String>, path: String) {
        check(values.toSet().size == values.size) { "$path must not contain duplicate identifiers" }
    }
}

private fun immutableJsonObject(source: Map<String, Any?>): Map<String, Any?> {
    val copy = LinkedHashMap<String, Any?>(source.size)
    source.forEach { (key, value) -> copy[key] = immutableJsonValue(value) }
    return Collections.unmodifiableMap(copy)
}

private fun immutableJsonValue(value: Any?): Any? = when (value) {
    is Map<*, *> -> {
        val copy = LinkedHashMap<String, Any?>(value.size)
        value.forEach { (key, nested) ->
            check(key is String) { "metadata JSON objects must use string keys" }
            copy[key] = immutableJsonValue(nested)
        }
        Collections.unmodifiableMap(copy)
    }
    is List<*> -> Collections.unmodifiableList(value.map(::immutableJsonValue))
    else -> value
}
