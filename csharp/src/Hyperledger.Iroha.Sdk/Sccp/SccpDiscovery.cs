using System.Text.Json;

namespace Hyperledger.Iroha.Sccp;

public sealed record SccpOutboundProofCapability(
    string MessageBundlePath,
    string ProofArtifactPath,
    string ProofJobPath,
    string RecentMessagesPath,
    string ManifestPath);

public sealed record SccpCodecCapability(SccpCodecV1 Codec, string Description);

public sealed record SccpBrowserProverManifestRef(
    Uri ModuleUrl,
    string? ModuleSpecifier,
    string ModuleHash,
    string ManifestHash,
    IReadOnlyList<string> ExpectedExports,
    string BoundRouteHash,
    string BoundProofHash);

public sealed record SccpNativeAdmissionCapability(
    SccpNativeBackendV1 Backend,
    string BackendLabel,
    string TrustAnchorHash);

public sealed record SccpSourceIdentityV1(SccpLaneIdV1 Lane, SccpSourceEmitterV1 Emitter);

public sealed record SccpExactInboundLaneCapability(
    SccpLaneIdV1 Lane,
    string SourceIdentityHash,
    SccpSourceIdentityV1 SourceIdentity,
    bool AdmissionEnabled,
    SccpNativeAdmissionCapability? NativeAdmission,
    SccpBrowserProverManifestRef? NativeProofBuilder);

public sealed record SccpCapabilities(
    byte Version,
    string RegistryRevision,
    string? NativeMessageSubmitPath,
    SccpOutboundProofCapability Outbound,
    IReadOnlyList<SccpPayloadKindV1> MessagePayloadKinds,
    IReadOnlyList<SccpCodecCapability> Codecs,
    IReadOnlyList<SccpExactInboundLaneCapability> InboundLanes)
{
    public static SccpCapabilities Parse(ReadOnlyMemory<byte> json) =>
        SccpDiscoveryParser.ParseCapabilities(json);
}

public enum SccpDestinationVerifierPlanV1
{
    EvmGroth16Bn254Adapter,
    SolanaProgramNativeRecursive,
    TonContractNativeRecursive,
    TronContractGroth16Bn254,
}

public sealed record SccpOutboundDestinationRoute(
    SccpLaneIdV1 Lane,
    string RouteId,
    string AssetKey,
    SccpDestinationVerifierPlanV1 VerifierPlan,
    string VerifierIdentity,
    string VerifierCodeHash,
    string? VerifierKeyHash,
    string? ProofArtifactHash,
    string? ProvingKeyHash,
    string DestinationBindingKey,
    string DestinationBindingHash,
    SccpBrowserProverManifestRef? BrowserProver);

public sealed record SccpProofManifestSet(
    byte Version,
    string RegistryRevision,
    IReadOnlyList<SccpExactInboundLaneCapability> InboundNativeLanes,
    IReadOnlyList<SccpOutboundDestinationRoute> OutboundDestinationRoutes)
{
    public static SccpProofManifestSet Parse(ReadOnlyMemory<byte> json) =>
        SccpDiscoveryParser.ParseManifests(json);
}

public sealed record SccpRecentMessageLinks(string BundlePath, string ArtifactPath, string JobPath);

public sealed record SccpRecentMessage(
    ulong Height,
    string MessageIdHex,
    SccpPayloadKindV1 Kind,
    SccpLaneIdV1 Lane,
    string DestinationBindingHash,
    string? AssetId,
    string? RouteId,
    string? Recipient,
    string? Amount,
    string? PayloadProjectionJson,
    SccpRecentMessageLinks Links);

public sealed record SccpRecentMessages(IReadOnlyList<SccpRecentMessage> Items)
{
    public static SccpRecentMessages Parse(ReadOnlyMemory<byte> json) =>
        SccpDiscoveryParser.ParseRecent(json);
}

internal static class SccpDiscoveryParser
{
    private static readonly HashSet<string> CapabilityFields =
    [
        "version", "registry_revision", "native_message_submit_path", "outbound",
        "message_payload_kinds", "codecs", "inbound_lanes",
    ];
    private static readonly HashSet<string> InboundFields =
    [
        "source_profile", "target_profile", "source_domain", "target_domain",
        "source_identity_hash", "source_identity", "admission_enabled", "native_admission",
        "native_proof_builder",
    ];
    private static readonly HashSet<string> BrowserFields =
    [
        "module_url", "module_specifier", "module_hash", "manifest_hash", "expected_exports",
        "bound_route_hash", "bound_proof_hash",
    ];
    private static readonly HashSet<string> RouteFields =
    [
        "source_profile", "target_profile", "source_domain", "target_domain", "route_id",
        "asset_key", "verifier_plan", "verifier_identity", "verifier_code_hash",
        "verifier_key_hash", "proof_artifact_hash", "proving_key_hash",
        "destination_binding_key", "destination_binding_hash", "browser_prover",
    ];
    private static readonly HashSet<string> RecentFields =
    [
        "height", "message_id_hex", "kind", "source_profile", "target_profile",
        "destination_binding_hash", "target_domain", "counterparty_domain", "asset_id",
        "route_id", "recipient", "amount", "payload_projection", "links",
    ];

    internal static SccpCapabilities ParseCapabilities(ReadOnlyMemory<byte> json)
    {
        using var document = SccpJson.Parse(json, "SCCP capabilities");
        var root = document.RootElement;
        SccpJson.ExactFields(root, CapabilityFields, "SCCP capabilities");
        if (SccpJson.UInt64(root, "version", 1) != 1)
        {
            throw new ArgumentException("Unsupported SCCP capability version.");
        }

        var nativePath = OptionalPath(root, "native_message_submit_path");
        if (nativePath is not null && nativePath != "/v1/bridge/messages")
        {
            throw new ArgumentException("native_message_submit_path is not the exact V1 endpoint.");
        }

        var outboundObject = Object(root, "outbound");
        SccpJson.ExactFields(outboundObject,
        [
            "message_bundle_path", "proof_artifact_path", "proof_job_path",
            "recent_messages_path", "manifest_path",
        ], "SCCP outbound capability");
        var outbound = new SccpOutboundProofCapability(
            Path(outboundObject, "message_bundle_path"),
            Path(outboundObject, "proof_artifact_path"),
            Path(outboundObject, "proof_job_path"),
            Path(outboundObject, "recent_messages_path"),
            Path(outboundObject, "manifest_path"));
        if (outbound != new SccpOutboundProofCapability(
                "/v1/sccp/proofs/message/{message_id}",
                "/v1/sccp/artifacts/message/{message_id}",
                "/v1/sccp/jobs/message/{message_id}",
                "/v1/sccp/messages/recent",
                "/v1/sccp/manifests"))
        {
            throw new ArgumentException("SCCP outbound capability paths do not match V1.");
        }

        var kinds = Array(root, "message_payload_kinds")
            .Select(static item => SccpPayloadKindV1Extensions.ParseWireKey(Text(item, "message_payload_kinds item")))
            .ToArray();
        if (kinds.Length != Enum.GetValues<SccpPayloadKindV1>().Length
            || !kinds.SequenceEqual(Enum.GetValues<SccpPayloadKindV1>()))
        {
            throw new ArgumentException("message_payload_kinds must advertise the exact ordered V1 inventory.");
        }

        var codecs = Array(root, "codecs").Select((item, index) =>
        {
            SccpJson.ExactFields(item, ["id", "key", "description"], $"codecs[{index}]");
            var id = SccpJson.UInt32(item, "id", 1, 6);
            var codec = (SccpCodecV1)id;
            if (SccpJson.Text(item, "key") != codec.WireKey())
            {
                throw new ArgumentException("SCCP codec id/key mismatch.");
            }

            return new SccpCodecCapability(codec, SccpJson.Text(item, "description"));
        }).ToArray();
        if (!codecs.Select(static item => item.Codec).SequenceEqual(Enum.GetValues<SccpCodecV1>()))
        {
            throw new ArgumentException("codecs must advertise the exact ordered V1 inventory.");
        }

        var lanes = Array(root, "inbound_lanes").Select(ParseInbound).ToArray();
        RequireUnique(lanes.Select(static item =>
            $"{item.Lane.Source.ProfileKey()}->{item.Lane.Target.ProfileKey()}"), "inbound lanes");
        return new SccpCapabilities(
            1,
            Hash(root, "registry_revision"),
            nativePath,
            outbound,
            kinds,
            codecs,
            lanes);
    }

    internal static SccpProofManifestSet ParseManifests(ReadOnlyMemory<byte> json)
    {
        using var document = SccpJson.Parse(json, "SCCP proof manifests");
        var root = document.RootElement;
        SccpJson.ExactFields(root,
        [
            "version", "registry_revision", "inbound_native_lanes", "outbound_destination_routes",
        ], "SCCP proof manifests");
        if (SccpJson.UInt64(root, "version", 1) != 1)
        {
            throw new ArgumentException("Unsupported SCCP manifest version.");
        }

        var inbound = Array(root, "inbound_native_lanes").Select(ParseInbound).ToArray();
        var routes = Array(root, "outbound_destination_routes").Select((item, index) =>
        {
            SccpJson.ExactFields(item, RouteFields, $"outbound_destination_routes[{index}]");
            var lane = ParseLane(item, requireInbound: false);
            if (!Enum.TryParse<SccpDestinationVerifierPlanV1>(
                    SccpJson.Text(item, "verifier_plan"),
                    ignoreCase: false,
                    out var plan))
            {
                throw new ArgumentException("Unknown or retired SCCP verifier plan.");
            }

            return new SccpOutboundDestinationRoute(
                lane,
                SccpJson.Text(item, "route_id"),
                SccpJson.Text(item, "asset_key"),
                plan,
                SccpJson.Text(item, "verifier_identity"),
                Hash(item, "verifier_code_hash"),
                OptionalHash(item, "verifier_key_hash"),
                OptionalHash(item, "proof_artifact_hash"),
                OptionalHash(item, "proving_key_hash"),
                SccpJson.Text(item, "destination_binding_key"),
                Hash(item, "destination_binding_hash"),
                OptionalBrowser(item, "browser_prover"));
        }).ToArray();
        RequireUnique(routes.Select(static item => item.RouteId), "outbound route ids");
        RequireUnique(routes.Select(static item => item.DestinationBindingHash), "outbound destination bindings");
        return new SccpProofManifestSet(1, Hash(root, "registry_revision"), inbound, routes);
    }

    internal static SccpRecentMessages ParseRecent(ReadOnlyMemory<byte> json)
    {
        using var document = SccpJson.Parse(json, "SCCP recent messages");
        var root = document.RootElement;
        SccpJson.ExactFields(root, ["items"], "SCCP recent messages");
        var messages = Array(root, "items").Select((item, index) =>
        {
            SccpJson.ExactFields(item, RecentFields, $"items[{index}]");
            var lane = ParseLane(item, requireInbound: false);
            var targetDomain = SccpJson.UInt32(item, "target_domain", 1, 5);
            var counterpartyDomain = SccpJson.UInt32(item, "counterparty_domain", 1, 5);
            if (targetDomain != lane.Target.DomainId() || counterpartyDomain != lane.Target.DomainId())
            {
                throw new ArgumentException("Recent SCCP message profile/domain mismatch.");
            }

            var amount = SccpJson.OptionalText(item, "amount");
            if (amount is not null
                && (amount[0] == '0' || amount.Any(static character => !char.IsAsciiDigit(character))))
            {
                throw new ArgumentException("SCCP amount must be canonical positive decimal.");
            }

            var links = Object(item, "links");
            SccpJson.ExactFields(links, ["bundle_path", "artifact_path", "job_path"], "recent message links");
            var projection = item.GetProperty("payload_projection").ValueKind switch
            {
                JsonValueKind.Null => null,
                JsonValueKind.Object => item.GetProperty("payload_projection").GetRawText(),
                _ => throw new ArgumentException("payload_projection must be an object or null."),
            };
            return new SccpRecentMessage(
                SccpJson.UInt64(item, "height", 1),
                Hash(item, "message_id_hex"),
                SccpPayloadKindV1Extensions.ParseWireKey(SccpJson.Text(item, "kind")),
                lane,
                Hash(item, "destination_binding_hash"),
                SccpJson.OptionalText(item, "asset_id"),
                SccpJson.OptionalText(item, "route_id"),
                SccpJson.OptionalText(item, "recipient"),
                amount,
                projection,
                new SccpRecentMessageLinks(
                    Path(links, "bundle_path"),
                    Path(links, "artifact_path"),
                    Path(links, "job_path")));
        }).ToArray();
        RequireUnique(messages.Select(static item => item.MessageIdHex), "recent message ids");
        for (var index = 1; index < messages.Length; index++)
        {
            if (messages[index].Height > messages[index - 1].Height)
            {
                throw new ArgumentException("Recent SCCP messages must be newest first.");
            }
        }

        return new SccpRecentMessages(messages);
    }

    private static SccpExactInboundLaneCapability ParseInbound(JsonElement item)
    {
        SccpJson.ExactFields(item, InboundFields, "SCCP inbound lane");
        var lane = ParseLane(item, requireInbound: true);
        var identity = ParseSourceIdentity(Object(item, "source_identity"), lane);
        var admission = OptionalNativeAdmission(item, lane.Source);
        var enabled = SccpJson.Boolean(item, "admission_enabled");
        if (enabled && admission is null)
        {
            throw new ArgumentException("Enabled native admission requires verifier metadata.");
        }

        return new SccpExactInboundLaneCapability(
            lane,
            Hash(item, "source_identity_hash"),
            identity,
            enabled,
            admission,
            OptionalBrowser(item, "native_proof_builder"));
    }

    private static SccpLaneIdV1 ParseLane(JsonElement item, bool requireInbound)
    {
        var source = SccpNetworkV1Extensions.ParseProfileKey(SccpJson.Text(item, "source_profile"));
        var target = SccpNetworkV1Extensions.ParseProfileKey(SccpJson.Text(item, "target_profile"));
        var lane = new SccpLaneIdV1(source, target);
        if (lane.IsInbound != requireInbound)
        {
            throw new ArgumentException(requireInbound
                ? "Lane must be external-to-SORA."
                : "Lane must be SORA-to-external.");
        }

        if (SccpJson.UInt32(item, "source_domain", 0, 5) != source.DomainId()
            || SccpJson.UInt32(item, "target_domain", 0, 5) != target.DomainId())
        {
            throw new ArgumentException("SCCP lane profile/domain mismatch.");
        }

        return lane;
    }

    private static SccpSourceIdentityV1 ParseSourceIdentity(JsonElement item, SccpLaneIdV1 expectedLane)
    {
        SccpJson.ExactFields(item, ["lane", "emitter"], "SCCP source identity");
        var laneObject = Object(item, "lane");
        SccpJson.ExactFields(laneObject, ["source", "target"], "SCCP source identity lane");
        var lane = new SccpLaneIdV1(
            ParseNetwork(Object(laneObject, "source")),
            ParseNetwork(Object(laneObject, "target")));
        if (lane != expectedLane)
        {
            throw new ArgumentException("Source identity lane mismatch.");
        }

        return new SccpSourceIdentityV1(lane, ParseEmitter(Object(item, "emitter"), lane.Source));
    }

    private static SccpNetworkV1 ParseNetwork(JsonElement item)
    {
        SccpJson.ExactFields(item, ["network", "profile"], "SCCP network");
        if (item.GetProperty("profile").ValueKind != JsonValueKind.Null)
        {
            throw new ArgumentException("Unit SCCP network profile content must be null.");
        }

        var wire = SccpJson.Text(item, "network");
        if (wire.Any(static item => item is not (>= 'a' and <= 'z') and not '_'))
        {
            throw new ArgumentException("Unsupported SCCP network profile.");
        }

        return SccpNetworkV1Extensions.ParseProfileKey(wire.Replace('_', '-'));
    }

    private static SccpSourceEmitterV1 ParseEmitter(JsonElement item, SccpNetworkV1 source)
    {
        SccpJson.ExactFields(item, ["emitter", "identity"], "SCCP source emitter");
        var family = SccpJson.Text(item, "emitter");
        var identity = Object(item, "identity");
        switch (family)
        {
            case "evm":
                if (source is not (SccpNetworkV1.EthereumMainnet or SccpNetworkV1.EthereumSepolia
                    or SccpNetworkV1.BscMainnet or SccpNetworkV1.BscTestnet))
                {
                    throw new ArgumentException("Source emitter family does not match exact profile.");
                }

                SccpJson.ExactFields(identity,
                    ["address", "runtime_code_hash", "route_config_hash"], "SCCP EVM emitter");
                return new SccpSourceEmitterV1.Evm(
                    UpperHex(identity, "address", 20),
                    UpperHex(identity, "runtime_code_hash", 32),
                    UpperHex(identity, "route_config_hash", 32));
            case "solana":
                if (source is not (SccpNetworkV1.SolanaMainnetBeta or SccpNetworkV1.SolanaTestnet))
                {
                    throw new ArgumentException("Source emitter family does not match exact profile.");
                }

                SccpJson.ExactFields(identity,
                    ["program_id", "executable_hash", "authorized_emitter"], "SCCP Solana emitter");
                return new SccpSourceEmitterV1.Solana(
                    UpperHex(identity, "program_id", 32),
                    UpperHex(identity, "executable_hash", 32),
                    UpperHex(identity, "authorized_emitter", 32));
            case "ton":
                if (source is not (SccpNetworkV1.TonMainnet or SccpNetworkV1.TonTestnet))
                {
                    throw new ArgumentException("Source emitter family does not match exact profile.");
                }

                SccpJson.ExactFields(identity,
                    ["workchain", "account_id", "code_hash", "immutable_config_hash"], "SCCP TON emitter");
                var workchain = Int32(identity, "workchain");
                return new SccpSourceEmitterV1.Ton(
                    workchain,
                    UpperHex(identity, "account_id", 32),
                    UpperHex(identity, "code_hash", 32),
                    UpperHex(identity, "immutable_config_hash", 32));
            case "tron":
                if (source is not (SccpNetworkV1.TronMainnet or SccpNetworkV1.TronNile or SccpNetworkV1.TronShasta))
                {
                    throw new ArgumentException("Source emitter family does not match exact profile.");
                }

                SccpJson.ExactFields(identity,
                    ["address", "runtime_code_hash", "route_config_hash"], "SCCP TRON emitter");
                return new SccpSourceEmitterV1.Tron(
                    UpperHex(identity, "address", 20),
                    UpperHex(identity, "runtime_code_hash", 32),
                    UpperHex(identity, "route_config_hash", 32));
            default:
                throw new ArgumentException("Unsupported SCCP source emitter.");
        }
    }

    private static SccpNativeAdmissionCapability? OptionalNativeAdmission(
        JsonElement item,
        SccpNetworkV1 source)
    {
        var property = item.GetProperty("native_admission");
        if (property.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        if (property.ValueKind != JsonValueKind.Object)
        {
            throw new ArgumentException("native_admission must be an object or null.");
        }

        SccpJson.ExactFields(property,
            ["backend", "backend_label", "trust_anchor_hash"], "SCCP native admission");
        var backendObject = Object(property, "backend");
        SccpJson.ExactFields(backendObject, ["backend", "protocol"], "SCCP native backend");
        if (backendObject.GetProperty("protocol").ValueKind != JsonValueKind.Null)
        {
            throw new ArgumentException("Unit SCCP native backend content must be null.");
        }

        var backend = SccpNativeBackendV1Extensions.ParseWireKey(SccpJson.Text(backendObject, "backend"));
        if (!backend.Supports(source)
            || SccpJson.Text(property, "backend_label") != backend.BackendLabel())
        {
            throw new ArgumentException("Native backend does not match its source profile or label.");
        }

        return new SccpNativeAdmissionCapability(
            backend,
            backend.BackendLabel(),
            Hash(property, "trust_anchor_hash"));
    }

    private static SccpBrowserProverManifestRef? OptionalBrowser(JsonElement item, string field)
    {
        var property = item.GetProperty(field);
        if (property.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        if (property.ValueKind != JsonValueKind.Object)
        {
            throw new ArgumentException($"{field} must be an object or null.");
        }

        SccpJson.ExactFields(property, BrowserFields, "SCCP browser prover");
        var urlText = SccpJson.Text(property, "module_url");
        if (!Uri.TryCreate(urlText, UriKind.Absolute, out var url)
            || url.Scheme is not ("http" or "https")
            || url.AbsoluteUri != urlText)
        {
            throw new ArgumentException("module_url must be an absolute HTTP(S) URL.");
        }

        var exports = Array(property, "expected_exports")
            .Select(static item => Text(item, "expected_exports item"))
            .ToArray();
        if (exports.Length == 0)
        {
            throw new ArgumentException("Browser prover exports must not be empty.");
        }

        RequireUnique(exports, "browser prover exports");
        return new SccpBrowserProverManifestRef(
            url,
            SccpJson.OptionalText(property, "module_specifier"),
            Hash(property, "module_hash"),
            Hash(property, "manifest_hash"),
            exports,
            Hash(property, "bound_route_hash"),
            Hash(property, "bound_proof_hash"));
    }

    private static JsonElement Object(JsonElement item, string field)
    {
        var value = item.GetProperty(field);
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new ArgumentException($"{field} must be an object.");
        }

        return value;
    }

    private static JsonElement[] Array(JsonElement item, string field)
    {
        var value = item.GetProperty(field);
        if (value.ValueKind != JsonValueKind.Array)
        {
            throw new ArgumentException($"{field} must be an array.");
        }

        return value.EnumerateArray().ToArray();
    }

    private static string Text(JsonElement item, string label)
    {
        if (item.ValueKind != JsonValueKind.String)
        {
            throw new ArgumentException($"{label} must be canonical nonempty text.");
        }

        var result = item.GetString()!;
        if (result.Length == 0 || result != result.Trim())
        {
            throw new ArgumentException($"{label} must be canonical nonempty text.");
        }

        return result;
    }

    private static string Path(JsonElement item, string field)
    {
        var value = SccpJson.Text(item, field);
        if (!value.StartsWith("/", StringComparison.Ordinal)
            || value.Contains("//", StringComparison.Ordinal)
            || value.Contains("?", StringComparison.Ordinal)
            || value.Contains("#", StringComparison.Ordinal))
        {
            throw new ArgumentException($"{field} must be a canonical absolute Torii path.");
        }

        return value;
    }

    private static string? OptionalPath(JsonElement item, string field) =>
        item.GetProperty(field).ValueKind == JsonValueKind.Null ? null : Path(item, field);

    private static string Hash(JsonElement item, string field)
    {
        var value = SccpJson.Text(item, field);
        if (!value.StartsWith("0x", StringComparison.Ordinal) || value.Length != 66)
        {
            throw new ArgumentException($"{field} must be canonical lowercase nonzero 32-byte hex.");
        }

        SccpSubmitValidation.ResponseHash(value[2..], field);
        return value;
    }

    private static string? OptionalHash(JsonElement item, string field) =>
        item.GetProperty(field).ValueKind == JsonValueKind.Null ? null : Hash(item, field);

    private static byte[] UpperHex(JsonElement item, string field, int bytes)
    {
        var value = SccpJson.Text(item, field);
        if (value.Length != bytes * 2
            || value.Any(static character => !char.IsAsciiDigit(character) && character is not (>= 'A' and <= 'F'))
            || value.All(static character => character == '0'))
        {
            throw new ArgumentException($"{field} must be canonical uppercase nonzero fixed hex.");
        }

        return Convert.FromHexString(value);
    }

    private static int Int32(JsonElement item, string field)
    {
        var property = item.GetProperty(field);
        if (property.ValueKind != JsonValueKind.Number || !property.TryGetInt32(out var result)
            || property.GetRawText() != result.ToString(System.Globalization.CultureInfo.InvariantCulture))
        {
            throw new ArgumentException($"{field} must be a canonical i32.");
        }

        return result;
    }

    private static void RequireUnique(IEnumerable<string> values, string label)
    {
        var observed = new HashSet<string>(StringComparer.Ordinal);
        foreach (var value in values)
        {
            if (!observed.Add(value))
            {
                throw new ArgumentException($"{label} must be unique.");
            }
        }
    }
}
