package org.hyperledger.iroha.android.client;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.sccp.SccpLaneIdV1;
import org.hyperledger.iroha.android.sccp.SccpNetworkV1;

/** Strict parser for exact-lane SCCP public discovery and readback DTOs. */
public final class SccpJsonParser {
  private static final Pattern HASH = Pattern.compile("0x[0-9a-f]{64}");
  private static final Pattern POSITIVE_DECIMAL = Pattern.compile("[1-9][0-9]*");

  private static final Set<String> CAPABILITY_FIELDS =
      Set.of("version", "registry_revision", "native_message_submit_path", "outbound", "message_payload_kinds", "codecs", "inbound_lanes");
  private static final Set<String> OUTBOUND_FIELDS =
      Set.of("message_bundle_path", "proof_artifact_path", "proof_job_path", "recent_messages_path", "manifest_path");
  private static final Set<String> INBOUND_FIELDS =
      Set.of("source_profile", "target_profile", "source_domain", "target_domain", "source_identity_hash", "source_identity", "admission_enabled", "native_admission", "native_proof_builder");
  private static final Set<String> BROWSER_FIELDS =
      Set.of("module_url", "module_specifier", "module_hash", "manifest_hash", "expected_exports", "bound_route_hash", "bound_proof_hash");
  private static final Set<String> MANIFEST_FIELDS =
      Set.of("version", "registry_revision", "inbound_native_lanes", "outbound_destination_routes");
  private static final Set<String> ROUTE_FIELDS =
      Set.of("source_profile", "target_profile", "source_domain", "target_domain", "route_id", "asset_key", "verifier_plan", "verifier_identity", "verifier_code_hash", "verifier_key_hash", "proof_artifact_hash", "proving_key_hash", "destination_binding_key", "destination_binding_hash", "browser_prover");
  private static final Set<String> RECENT_FIELDS =
      Set.of("height", "message_id_hex", "kind", "source_profile", "target_profile", "destination_binding_hash", "target_domain", "counterparty_domain", "asset_id", "route_id", "recipient", "amount", "payload_projection", "links");

  private SccpJsonParser() {}

  public static SccpModels.Capabilities parseCapabilities(final byte[] bytes) {
    final Map<String, Object> root = rootObject(bytes, "SCCP capabilities");
    exactFields(root, CAPABILITY_FIELDS, "SCCP capabilities");
    final List<SccpModels.CodecCapability> codecs = new ArrayList<>();
    final Set<Integer> codecIds = new HashSet<>();
    final List<Object> codecValues = requiredList(root, "codecs");
    for (int index = 0; index < codecValues.size(); index++) {
      final Map<String, Object> value = objectValue(codecValues.get(index), "codecs[" + index + "]");
      exactFields(value, Set.of("id", "key", "description"), "SCCP codec");
      final int id = requiredInt(value, "id", 1, 6);
      if (!codecIds.add(id)) throw new IllegalArgumentException("SCCP codec ids must be unique");
      final SccpModels.CodecV1 codec = SccpModels.CodecV1.fromId(id);
      if (codec == null || !codec.wireKey.equals(requiredNonBlank(value, "key"))) {
        throw new IllegalArgumentException("SCCP codec key does not match its canonical tag");
      }
      codecs.add(new SccpModels.CodecCapability(codec, requiredNonBlank(value, "description")));
    }
    final List<SccpModels.PayloadKindV1> payloadKinds = new ArrayList<>();
    for (final String value : requiredStringList(root, "message_payload_kinds", false)) {
      final SccpModels.PayloadKindV1 kind = SccpModels.PayloadKindV1.fromWireKey(value);
      if (kind == null) {
        throw new IllegalArgumentException(
            "message_payload_kinds contains an unknown or retired kind");
      }
      payloadKinds.add(kind);
    }
    if (new HashSet<>(payloadKinds).size() != payloadKinds.size()) {
      throw new IllegalArgumentException("message_payload_kinds contains duplicates");
    }
    final List<SccpModels.ExactInboundLaneCapability> inbound = new ArrayList<>();
    final List<Object> inboundValues = requiredList(root, "inbound_lanes");
    for (int index = 0; index < inboundValues.size(); index++) {
      inbound.add(parseInbound(objectValue(inboundValues.get(index), "inbound_lanes[" + index + "]")));
    }
    return new SccpModels.Capabilities(
        requiredInt(root, "version", 1, 1),
        requiredHash(root, "registry_revision"),
        optionalPath(root, "native_message_submit_path"),
        parseOutbound(requiredObject(root, "outbound")),
        payloadKinds,
        codecs,
        inbound);
  }

  public static SccpModels.ProofManifestSet parseProofManifests(final byte[] bytes) {
    final Map<String, Object> root = rootObject(bytes, "SCCP proof manifests");
    exactFields(root, MANIFEST_FIELDS, "SCCP proof manifests");
    final List<SccpModels.ExactInboundLaneCapability> inbound = new ArrayList<>();
    for (final Object value : requiredList(root, "inbound_native_lanes")) {
      inbound.add(parseInbound(objectValue(value, "inbound_native_lanes item")));
    }
    final List<SccpModels.OutboundDestinationRoute> outbound = new ArrayList<>();
    for (final Object value : requiredList(root, "outbound_destination_routes")) {
      outbound.add(parseRoute(objectValue(value, "outbound_destination_routes item")));
    }
    return new SccpModels.ProofManifestSet(
        requiredInt(root, "version", 1, 1),
        requiredHash(root, "registry_revision"), inbound, outbound);
  }

  public static SccpModels.RecentMessages parseRecentMessages(final byte[] bytes) {
    final Map<String, Object> root = rootObject(bytes, "SCCP recent messages");
    exactFields(root, Set.of("items"), "SCCP recent messages");
    final List<SccpModels.RecentMessage> result = new ArrayList<>();
    long previous = Long.MAX_VALUE;
    for (final Object value : requiredList(root, "items")) {
      final SccpModels.RecentMessage item = parseRecent(objectValue(value, "recent message"));
      if (item.height > previous) {
        throw new IllegalArgumentException("SCCP recent messages must be newest-first");
      }
      previous = item.height;
      result.add(item);
    }
    return new SccpModels.RecentMessages(result);
  }

  private static SccpModels.OutboundProofCapability parseOutbound(final Map<String, Object> value) {
    exactFields(value, OUTBOUND_FIELDS, "SCCP outbound capability");
    return new SccpModels.OutboundProofCapability(
        requiredPath(value, "message_bundle_path"), requiredPath(value, "proof_artifact_path"),
        requiredPath(value, "proof_job_path"), requiredPath(value, "recent_messages_path"),
        requiredPath(value, "manifest_path"));
  }

  private static SccpModels.ExactInboundLaneCapability parseInbound(final Map<String, Object> value) {
    exactFields(value, INBOUND_FIELDS, "SCCP inbound lane");
    final SccpNetworkV1 source = requiredProfile(value, "source_profile");
    final SccpNetworkV1 target = requiredProfile(value, "target_profile");
    final SccpLaneIdV1 lane = new SccpLaneIdV1(source, target);
    if (!lane.isInbound()) throw new IllegalArgumentException("inbound SCCP capability must be external-to-SORA");
    final int sourceDomain = requiredInt(value, "source_domain", 0, 5);
    final int targetDomain = requiredInt(value, "target_domain", 0, 5);
    if (sourceDomain != source.domainId() || targetDomain != target.domainId()) {
      throw new IllegalArgumentException("inbound SCCP profile/domain mismatch");
    }
    final SccpModels.SourceIdentityV1 identity = parseSourceIdentity(requiredObject(value, "source_identity"));
    if (!lane.equals(identity.lane)) throw new IllegalArgumentException("source_identity lane mismatch");
    final Map<String, Object> admissionObject = optionalObject(value, "native_admission");
    final SccpModels.NativeAdmissionCapability admission =
        admissionObject == null ? null : parseNativeAdmission(admissionObject, source);
    final boolean enabled = requiredBoolean(value, "admission_enabled");
    if (enabled && admission == null) {
      throw new IllegalArgumentException("enabled native admission requires verifier metadata");
    }
    final Map<String, Object> builderObject = optionalObject(value, "native_proof_builder");
    return new SccpModels.ExactInboundLaneCapability(
        source.profileKey(), target.profileKey(), sourceDomain, targetDomain,
        requiredHash(value, "source_identity_hash"), identity, enabled, admission,
        builderObject == null ? null : parseBrowserProver(builderObject));
  }

  private static SccpModels.SourceIdentityV1 parseSourceIdentity(final Map<String, Object> value) {
    exactFields(value, Set.of("lane", "emitter"), "SCCP source identity");
    final Map<String, Object> laneObject = requiredObject(value, "lane");
    exactFields(laneObject, Set.of("source", "target"), "SCCP source identity lane");
    final SccpLaneIdV1 lane =
        new SccpLaneIdV1(parseNetwork(requiredObject(laneObject, "source")), parseNetwork(requiredObject(laneObject, "target")));
    if (!lane.isInbound()) throw new IllegalArgumentException("source identity must use an inbound lane");
    return new SccpModels.SourceIdentityV1(lane, parseEmitter(requiredObject(value, "emitter"), lane.source()));
  }

  private static SccpNetworkV1 parseNetwork(final Map<String, Object> value) {
    exactFields(value, Set.of("network", "profile"), "SCCP network");
    if (!value.containsKey("profile") || value.get("profile") != null) {
      throw new IllegalArgumentException("unit SCCP network profile content must be null");
    }
    final SccpNetworkV1 result = SccpNetworkV1.fromProfileKey(requiredNonBlank(value, "network").replace('_', '-'));
    if (result == null) throw new IllegalArgumentException("unsupported SCCP network profile");
    return result;
  }

  private static SccpModels.SourceEmitterV1 parseEmitter(
      final Map<String, Object> value, final SccpNetworkV1 source) {
    exactFields(value, Set.of("emitter", "identity"), "SCCP source emitter");
    final SccpModels.SourceEmitterFamilyV1 family =
        SccpModels.SourceEmitterFamilyV1.fromWireKey(requiredNonBlank(value, "emitter"));
    if (family == null || family != expectedFamily(source)) {
      throw new IllegalArgumentException("source emitter family does not match exact profile");
    }
    final Map<String, Object> identity = requiredObject(value, "identity");
    final Map<String, Object> normalized = new LinkedHashMap<>();
    switch (family) {
      case EVM, TRON -> {
        exactFields(identity, Set.of("address", "runtime_code_hash", "route_config_hash"), "SCCP EVM/TRON emitter");
        final String address = requiredFixedUpperHex(identity, "address", 20);
        final String runtime = requiredFixedUpperHex(identity, "runtime_code_hash", 32);
        final String routeConfig = requiredFixedUpperHex(identity, "route_config_hash", 32);
        if (runtime.equals(routeConfig)) throw new IllegalArgumentException("source emitter runtime and route-config hashes must differ");
        normalized.put("address", address); normalized.put("runtime_code_hash", runtime); normalized.put("route_config_hash", routeConfig);
      }
      case SOLANA -> {
        exactFields(identity, Set.of("program_id", "executable_hash", "authorized_emitter"), "SCCP Solana emitter");
        final List<String> roles = Arrays.asList(
            requiredFixedUpperHex(identity, "program_id", 32),
            requiredFixedUpperHex(identity, "executable_hash", 32),
            requiredFixedUpperHex(identity, "authorized_emitter", 32));
        if (new HashSet<>(roles).size() != roles.size()) throw new IllegalArgumentException("Solana emitter roles must be distinct");
        normalized.put("program_id", roles.get(0)); normalized.put("executable_hash", roles.get(1)); normalized.put("authorized_emitter", roles.get(2));
      }
      case TON -> {
        exactFields(identity, Set.of("workchain", "account_id", "code_hash", "immutable_config_hash"), "SCCP TON emitter");
        final int workchain = requiredInt(identity, "workchain", 0, 0);
        final List<String> roles = Arrays.asList(
            requiredFixedUpperHex(identity, "account_id", 32),
            requiredFixedUpperHex(identity, "code_hash", 32),
            requiredFixedUpperHex(identity, "immutable_config_hash", 32));
        if (new HashSet<>(roles).size() != roles.size()) throw new IllegalArgumentException("TON emitter roles must be distinct");
        normalized.put("workchain", workchain); normalized.put("account_id", roles.get(0)); normalized.put("code_hash", roles.get(1)); normalized.put("immutable_config_hash", roles.get(2));
      }
    }
    return new SccpModels.SourceEmitterV1(family, normalized);
  }

  private static SccpModels.SourceEmitterFamilyV1 expectedFamily(final SccpNetworkV1 network) {
    return switch (network) {
      case ETHEREUM_MAINNET, ETHEREUM_SEPOLIA, BSC_MAINNET, BSC_TESTNET -> SccpModels.SourceEmitterFamilyV1.EVM;
      case SOLANA_MAINNET_BETA, SOLANA_TESTNET -> SccpModels.SourceEmitterFamilyV1.SOLANA;
      case TON_MAINNET, TON_TESTNET -> SccpModels.SourceEmitterFamilyV1.TON;
      case TRON_MAINNET, TRON_NILE, TRON_SHASTA -> SccpModels.SourceEmitterFamilyV1.TRON;
      default -> throw new IllegalArgumentException("SORA cannot be a source emitter");
    };
  }

  private static SccpModels.NativeAdmissionCapability parseNativeAdmission(
      final Map<String, Object> value, final SccpNetworkV1 source) {
    exactFields(value, Set.of("backend", "backend_label", "trust_anchor_hash"), "SCCP native admission");
    final Map<String, Object> backendObject = requiredObject(value, "backend");
    exactFields(backendObject, Set.of("backend", "protocol"), "SCCP native backend");
    if (!backendObject.containsKey("protocol") || backendObject.get("protocol") != null) {
      throw new IllegalArgumentException("unit native backend content must be null");
    }
    final SccpModels.NativeBackendV1 backend =
        SccpModels.NativeBackendV1.fromWireKey(requiredNonBlank(backendObject, "backend"));
    if (backend == null || !supports(backend, source)) {
      throw new IllegalArgumentException("native backend does not support exact source profile");
    }
    final String label = requiredNonBlank(value, "backend_label");
    if (!label.equals(backend.backendLabel)) throw new IllegalArgumentException("native backend label mismatch");
    return new SccpModels.NativeAdmissionCapability(backend, label, requiredHash(value, "trust_anchor_hash"));
  }

  private static boolean supports(final SccpModels.NativeBackendV1 backend, final SccpNetworkV1 network) {
    return switch (backend) {
      case ETHEREUM_BEACON -> network == SccpNetworkV1.ETHEREUM_MAINNET || network == SccpNetworkV1.ETHEREUM_SEPOLIA;
      case BSC_PARLIA -> network == SccpNetworkV1.BSC_MAINNET || network == SccpNetworkV1.BSC_TESTNET;
      case SOLANA_TOWER -> network == SccpNetworkV1.SOLANA_MAINNET_BETA || network == SccpNetworkV1.SOLANA_TESTNET;
      case TON_MASTERCHAIN -> network == SccpNetworkV1.TON_MAINNET || network == SccpNetworkV1.TON_TESTNET;
      case TRON_DPOS -> network == SccpNetworkV1.TRON_MAINNET || network == SccpNetworkV1.TRON_NILE || network == SccpNetworkV1.TRON_SHASTA;
    };
  }

  private static SccpModels.BrowserProverManifestRef parseBrowserProver(final Map<String, Object> value) {
    exactFields(value, BROWSER_FIELDS, "SCCP browser prover");
    final List<String> exports = requiredStringList(value, "expected_exports", true);
    if (new HashSet<>(exports).size() != exports.size()) throw new IllegalArgumentException("browser prover exports must be unique");
    final String url = requiredNonBlank(value, "module_url");
    if (!url.startsWith("https://")) throw new IllegalArgumentException("module_url must be HTTPS");
    return new SccpModels.BrowserProverManifestRef(
        url, optionalNonBlank(value, "module_specifier"), requiredHash(value, "module_hash"),
        requiredHash(value, "manifest_hash"), exports, requiredHash(value, "bound_route_hash"),
        requiredHash(value, "bound_proof_hash"));
  }

  private static SccpModels.OutboundDestinationRoute parseRoute(final Map<String, Object> value) {
    exactFields(value, ROUTE_FIELDS, "SCCP outbound destination route");
    final SccpNetworkV1 source = requiredProfile(value, "source_profile");
    final SccpNetworkV1 target = requiredProfile(value, "target_profile");
    final SccpLaneIdV1 lane = new SccpLaneIdV1(source, target);
    if (!lane.isOutbound()) throw new IllegalArgumentException("outbound route must be SORA-to-external");
    final int sourceDomain = requiredInt(value, "source_domain", 0, 5);
    final int targetDomain = requiredInt(value, "target_domain", 0, 5);
    if (sourceDomain != source.domainId() || targetDomain != target.domainId()) throw new IllegalArgumentException("outbound route profile/domain mismatch");
    final SccpModels.DestinationVerifierPlanV1 plan =
        SccpModels.DestinationVerifierPlanV1.fromWireKey(requiredNonBlank(value, "verifier_plan"));
    if (plan == null) throw new IllegalArgumentException("unknown or retired SCCP verifier plan");
    final Map<String, Object> browser = optionalObject(value, "browser_prover");
    return new SccpModels.OutboundDestinationRoute(
        source.profileKey(), target.profileKey(), sourceDomain, targetDomain,
        requiredNonBlank(value, "route_id"), requiredNonBlank(value, "asset_key"), plan,
        requiredNonBlank(value, "verifier_identity"), requiredHash(value, "verifier_code_hash"),
        optionalHash(value, "verifier_key_hash"), optionalHash(value, "proof_artifact_hash"),
        optionalHash(value, "proving_key_hash"), requiredNonBlank(value, "destination_binding_key"),
        requiredHash(value, "destination_binding_hash"), browser == null ? null : parseBrowserProver(browser));
  }

  private static SccpModels.RecentMessage parseRecent(final Map<String, Object> value) {
    exactFields(value, RECENT_FIELDS, "SCCP recent message");
    final SccpNetworkV1 source = requiredProfile(value, "source_profile");
    final SccpNetworkV1 target = requiredProfile(value, "target_profile");
    if (!new SccpLaneIdV1(source, target).isOutbound()) throw new IllegalArgumentException("recent message must use outbound lane");
    final int targetDomain = requiredInt(value, "target_domain", 1, 5);
    final int counterparty = requiredInt(value, "counterparty_domain", 1, 5);
    if (targetDomain != target.domainId() || counterparty != target.domainId()) throw new IllegalArgumentException("recent message profile/domain mismatch");
    final String amount = optionalNonBlank(value, "amount");
    if (amount != null && !POSITIVE_DECIMAL.matcher(amount).matches()) throw new IllegalArgumentException("amount must be canonical positive decimal");
    final Map<String, Object> links = requiredObject(value, "links");
    exactFields(links, Set.of("bundle_path", "artifact_path", "job_path"), "SCCP recent links");
    final SccpModels.PayloadKindV1 kind =
        SccpModels.PayloadKindV1.fromWireKey(requiredNonBlank(value, "kind"));
    if (kind == null) {
      throw new IllegalArgumentException("recent SCCP message kind is unknown or retired");
    }
    return new SccpModels.RecentMessage(
        requiredLong(value, "height", 1), requiredHash(value, "message_id_hex"),
        kind, source.profileKey(), target.profileKey(),
        requiredHash(value, "destination_binding_hash"), targetDomain, counterparty,
        optionalNonBlank(value, "asset_id"), optionalNonBlank(value, "route_id"),
        optionalNonBlank(value, "recipient"), amount, optionalObject(value, "payload_projection"),
        new SccpModels.RecentMessageLinks(requiredPath(links, "bundle_path"), requiredPath(links, "artifact_path"), requiredPath(links, "job_path")));
  }

  private static Map<String, Object> rootObject(final byte[] bytes, final String label) {
    final String text = new String(bytes, StandardCharsets.UTF_8);
    if (!Arrays.equals(text.getBytes(StandardCharsets.UTF_8), bytes)) throw new IllegalArgumentException(label + " must be UTF-8 JSON");
    return objectValue(JsonParser.parse(text), label);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> objectValue(final Object value, final String label) {
    if (!(value instanceof Map<?, ?>)) throw new IllegalArgumentException(label + " must be a JSON object");
    for (final Object key : ((Map<?, ?>) value).keySet()) if (!(key instanceof String)) throw new IllegalArgumentException(label + " keys must be strings");
    return (Map<String, Object>) value;
  }

  private static void exactFields(final Map<String, Object> value, final Set<String> allowed, final String label) {
    for (final String field : value.keySet()) if (!allowed.contains(field)) throw new IllegalArgumentException(label + " contains unknown field `" + field + "`");
  }
  private static Map<String, Object> requiredObject(final Map<String, Object> value, final String field) { return objectValue(value.get(field), field); }
  private static Map<String, Object> optionalObject(final Map<String, Object> value, final String field) { return value.get(field) == null ? null : objectValue(value.get(field), field); }
  @SuppressWarnings("unchecked")
  private static List<Object> requiredList(final Map<String, Object> value, final String field) {
    if (!(value.get(field) instanceof List<?>)) throw new IllegalArgumentException(field + " must be an array");
    return (List<Object>) value.get(field);
  }
  private static boolean requiredBoolean(final Map<String, Object> value, final String field) {
    if (!(value.get(field) instanceof Boolean)) throw new IllegalArgumentException(field + " must be a boolean");
    return (Boolean) value.get(field);
  }
  private static String requiredNonBlank(final Map<String, Object> value, final String field) {
    if (!(value.get(field) instanceof String)) throw new IllegalArgumentException(field + " must be a string");
    final String result = (String) value.get(field);
    if (result.isEmpty() || !result.equals(result.trim())) throw new IllegalArgumentException(field + " must be canonical nonblank text");
    return result;
  }
  private static String optionalNonBlank(final Map<String, Object> value, final String field) { return value.get(field) == null ? null : requiredNonBlank(value, field); }
  private static String requiredPath(final Map<String, Object> value, final String field) {
    final String path = requiredNonBlank(value, field);
    if (!path.startsWith("/") || path.contains("//")) throw new IllegalArgumentException(field + " must be an absolute Torii path");
    return path;
  }
  private static String optionalPath(final Map<String, Object> value, final String field) { return value.get(field) == null ? null : requiredPath(value, field); }
  private static int requiredInt(final Map<String, Object> value, final String field, final int minimum, final int maximum) {
    if (!(value.get(field) instanceof Number)) throw new IllegalArgumentException(field + " must be an integer");
    final Number number = (Number) value.get(field); final long result = number.longValue();
    if (!number.toString().equals(Long.toString(result)) || result < minimum || result > maximum) throw new IllegalArgumentException(field + " is out of range");
    return (int) result;
  }
  private static long requiredLong(final Map<String, Object> value, final String field, final long minimum) {
    if (!(value.get(field) instanceof Number)) throw new IllegalArgumentException(field + " must be an integer");
    final Number number = (Number) value.get(field); final long result = number.longValue();
    if (!number.toString().equals(Long.toString(result)) || result < minimum) throw new IllegalArgumentException(field + " is out of range");
    return result;
  }
  private static List<String> requiredStringList(final Map<String, Object> value, final String field, final boolean nonempty) {
    final List<String> result = new ArrayList<>();
    for (final Object item : requiredList(value, field)) {
      if (!(item instanceof String) || ((String) item).isEmpty() || !item.equals(((String) item).trim())) throw new IllegalArgumentException(field + " must contain canonical text");
      result.add((String) item);
    }
    if (nonempty && result.isEmpty()) throw new IllegalArgumentException(field + " must not be empty");
    return result;
  }
  private static String requiredHash(final Map<String, Object> value, final String field) {
    final String result = requiredNonBlank(value, field);
    if (!HASH.matcher(result).matches() || result.substring(2).chars().allMatch(item -> item == '0')) throw new IllegalArgumentException(field + " must be canonical lowercase nonzero 32-byte hex");
    return result;
  }
  private static String optionalHash(final Map<String, Object> value, final String field) { return value.get(field) == null ? null : requiredHash(value, field); }
  private static String requiredFixedUpperHex(final Map<String, Object> value, final String field, final int bytes) {
    final String result = requiredNonBlank(value, field);
    if (!result.matches("[0-9A-F]{" + (bytes * 2) + "}") || result.chars().allMatch(item -> item == '0')) throw new IllegalArgumentException(field + " must be canonical uppercase nonzero fixed hex");
    return result;
  }
  private static SccpNetworkV1 requiredProfile(final Map<String, Object> value, final String field) {
    final SccpNetworkV1 result = SccpNetworkV1.fromProfileKey(requiredNonBlank(value, field));
    if (result == null) throw new IllegalArgumentException(field + " is not an exact SCCP profile");
    return result;
  }
}
