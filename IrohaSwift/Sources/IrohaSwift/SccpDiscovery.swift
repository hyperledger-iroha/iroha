import Foundation

public struct SccpOutboundProofCapability: Equatable, Sendable {
    public let messageBundlePath: String
    public let proofArtifactPath: String
    public let proofJobPath: String
    public let recentMessagesPath: String
    public let manifestPath: String
}

public struct SccpCodecCapability: Equatable, Sendable {
    public let codec: SccpCodecV1
    public let description: String
}

public struct SccpBrowserProverManifestRef: Equatable, Sendable {
    public let moduleURL: URL
    public let moduleSpecifier: String?
    public let moduleHash: String
    public let manifestHash: String
    public let expectedExports: [String]
    public let boundRouteHash: String
    public let boundProofHash: String
}

public struct SccpNativeAdmissionCapability: Equatable, Sendable {
    public let backend: SccpNativeBackendV1
    public let backendLabel: String
    public let trustAnchorHash: String
}

public struct SccpSourceIdentityV1: Equatable, Sendable {
    public let lane: SccpLaneIdV1
    public let emitter: SccpSourceEmitterV1
}

public struct SccpExactInboundLaneCapability: Equatable, Sendable {
    public let lane: SccpLaneIdV1
    public let sourceIdentityHash: String
    public let sourceIdentity: SccpSourceIdentityV1
    public let admissionEnabled: Bool
    public let nativeAdmission: SccpNativeAdmissionCapability?
    public let nativeProofBuilder: SccpBrowserProverManifestRef?
}

public struct SccpCapabilities: Equatable, Sendable {
    public let version: UInt8
    public let registryRevision: String
    public let nativeMessageSubmitPath: String?
    public let outbound: SccpOutboundProofCapability
    public let messagePayloadKinds: [SccpPayloadKindV1]
    public let codecs: [SccpCodecCapability]
    public let inboundLanes: [SccpExactInboundLaneCapability]

    public static func parse(_ data: Data) throws -> Self {
        try SccpDiscoveryParser.capabilities(data)
    }
}

public enum SccpDestinationVerifierPlanV1: String, CaseIterable, Sendable {
    case evmGroth16Bn254Adapter = "EvmGroth16Bn254Adapter"
    case solanaProgramNativeRecursive = "SolanaProgramNativeRecursive"
    case tonContractNativeRecursive = "TonContractNativeRecursive"
    case tronContractGroth16Bn254 = "TronContractGroth16Bn254"
}

public struct SccpOutboundDestinationRoute: Equatable, Sendable {
    public let lane: SccpLaneIdV1
    public let routeId: String
    public let assetKey: String
    public let verifierPlan: SccpDestinationVerifierPlanV1
    public let verifierIdentity: String
    public let verifierCodeHash: String
    public let verifierKeyHash: String?
    public let proofArtifactHash: String?
    public let provingKeyHash: String?
    public let destinationBindingKey: String
    public let destinationBindingHash: String
    public let browserProver: SccpBrowserProverManifestRef?
}

public struct SccpProofManifestSet: Equatable, Sendable {
    public let version: UInt8
    public let registryRevision: String
    public let inboundNativeLanes: [SccpExactInboundLaneCapability]
    public let outboundDestinationRoutes: [SccpOutboundDestinationRoute]

    public static func parse(_ data: Data) throws -> Self {
        try SccpDiscoveryParser.manifests(data)
    }
}

public struct SccpRecentMessageLinks: Equatable, Sendable {
    public let bundlePath: String
    public let artifactPath: String
    public let jobPath: String
}

public struct SccpRecentMessage: Equatable, Sendable {
    public let height: UInt64
    public let messageIdHex: String
    public let kind: SccpPayloadKindV1
    public let lane: SccpLaneIdV1
    public let destinationBindingHash: String
    public let assetId: String?
    public let routeId: String?
    public let recipient: String?
    public let amount: String?
    public let payloadProjectionJSON: Data?
    public let links: SccpRecentMessageLinks
}

public struct SccpRecentMessages: Equatable, Sendable {
    public let items: [SccpRecentMessage]

    public static func parse(_ data: Data) throws -> Self {
        try SccpDiscoveryParser.recent(data)
    }
}

private enum SccpDiscoveryParser {
    static func capabilities(_ data: Data) throws -> SccpCapabilities {
        let root = try SccpStrictJSON.object(data, label: "SCCP capabilities")
        try SccpStrictJSON.exactFields(root, [
            "version", "registry_revision", "native_message_submit_path", "outbound",
            "message_payload_kinds", "codecs", "inbound_lanes",
        ], label: "SCCP capabilities")
        guard try SccpStrictJSON.uint64(root, "version", minimum: 1) == 1 else {
            throw SccpV1Error.invalid("unsupported SCCP capability version")
        }
        let revision = try hash(root, "registry_revision")
        let nativePath = try optionalPath(root, "native_message_submit_path")
        if let nativePath, nativePath != "/v1/bridge/messages" {
            throw SccpV1Error.invalid("native_message_submit_path is not the exact V1 endpoint")
        }
        let outboundObject = try object(root, "outbound")
        try SccpStrictJSON.exactFields(outboundObject, [
            "message_bundle_path", "proof_artifact_path", "proof_job_path",
            "recent_messages_path", "manifest_path",
        ], label: "SCCP outbound capability")
        let outbound = SccpOutboundProofCapability(
            messageBundlePath: try path(outboundObject, "message_bundle_path"),
            proofArtifactPath: try path(outboundObject, "proof_artifact_path"),
            proofJobPath: try path(outboundObject, "proof_job_path"),
            recentMessagesPath: try path(outboundObject, "recent_messages_path"),
            manifestPath: try path(outboundObject, "manifest_path")
        )
        guard outbound.messageBundlePath == "/v1/sccp/proofs/message/{message_id}",
              outbound.proofArtifactPath == "/v1/sccp/artifacts/message/{message_id}",
              outbound.proofJobPath == "/v1/sccp/jobs/message/{message_id}",
              outbound.recentMessagesPath == "/v1/sccp/messages/recent",
              outbound.manifestPath == "/v1/sccp/manifests"
        else {
            throw SccpV1Error.invalid("SCCP outbound capability paths do not match V1")
        }
        let kinds = try stringArray(root, "message_payload_kinds").map { value -> SccpPayloadKindV1 in
            guard let kind = SccpPayloadKindV1(rawValue: value) else {
                throw SccpV1Error.invalid("message_payload_kinds contains an unknown or retired kind")
            }
            return kind
        }
        guard Set(kinds.map(\.rawValue)) == Set(SccpPayloadKindV1.allCases.map(\.rawValue)),
              kinds.count == SccpPayloadKindV1.allCases.count
        else {
            throw SccpV1Error.invalid("message_payload_kinds must advertise the exact V1 inventory once")
        }
        let codecs = try array(root, "codecs").enumerated().map { index, raw -> SccpCodecCapability in
            guard let item = raw as? [String: Any] else {
                throw SccpV1Error.invalid("codecs[\(index)] must be an object")
            }
            try SccpStrictJSON.exactFields(item, ["id", "key", "description"], label: "codecs[\(index)]")
            let id = try SccpStrictJSON.uint32(item, "id", minimum: 1, maximum: 6)
            guard let codec = SccpCodecV1(rawValue: UInt8(id)),
                  try SccpStrictJSON.text(item, "key") == codec.wireKey
            else { throw SccpV1Error.invalid("SCCP codec id/key mismatch") }
            return SccpCodecCapability(codec: codec, description: try SccpStrictJSON.text(item, "description"))
        }
        guard codecs.map(\.codec) == SccpCodecV1.allCases else {
            throw SccpV1Error.invalid("codecs must advertise the exact ordered V1 inventory")
        }
        let lanes = try array(root, "inbound_lanes").enumerated().map { index, raw in
            try inbound(raw, label: "inbound_lanes[\(index)]")
        }
        try requireUnique(lanes.map { "\($0.lane.source.rawValue)->\($0.lane.target.rawValue)" }, label: "inbound lanes")
        return SccpCapabilities(
            version: 1,
            registryRevision: revision,
            nativeMessageSubmitPath: nativePath,
            outbound: outbound,
            messagePayloadKinds: kinds,
            codecs: codecs,
            inboundLanes: lanes
        )
    }

    static func manifests(_ data: Data) throws -> SccpProofManifestSet {
        let root = try SccpStrictJSON.object(data, label: "SCCP proof manifests")
        try SccpStrictJSON.exactFields(root, [
            "version", "registry_revision", "inbound_native_lanes", "outbound_destination_routes",
        ], label: "SCCP proof manifests")
        guard try SccpStrictJSON.uint64(root, "version", minimum: 1) == 1 else {
            throw SccpV1Error.invalid("unsupported SCCP manifest version")
        }
        let inboundLanes = try array(root, "inbound_native_lanes").enumerated().map {
            try inbound($0.element, label: "inbound_native_lanes[\($0.offset)]")
        }
        let routes = try array(root, "outbound_destination_routes").enumerated().map { index, raw -> SccpOutboundDestinationRoute in
            guard let item = raw as? [String: Any] else {
                throw SccpV1Error.invalid("outbound_destination_routes[\(index)] must be an object")
            }
            try SccpStrictJSON.exactFields(item, [
                "source_profile", "target_profile", "source_domain", "target_domain", "route_id",
                "asset_key", "verifier_plan", "verifier_identity", "verifier_code_hash",
                "verifier_key_hash", "proof_artifact_hash", "proving_key_hash",
                "destination_binding_key", "destination_binding_hash", "browser_prover",
            ], label: "outbound_destination_routes[\(index)]")
            let lane = try lane(item, requireInbound: false)
            guard let plan = SccpDestinationVerifierPlanV1(rawValue: try SccpStrictJSON.text(item, "verifier_plan")) else {
                throw SccpV1Error.invalid("unknown or retired SCCP verifier plan")
            }
            return SccpOutboundDestinationRoute(
                lane: lane,
                routeId: try SccpStrictJSON.text(item, "route_id"),
                assetKey: try SccpStrictJSON.text(item, "asset_key"),
                verifierPlan: plan,
                verifierIdentity: try SccpStrictJSON.text(item, "verifier_identity"),
                verifierCodeHash: try hash(item, "verifier_code_hash"),
                verifierKeyHash: try optionalHash(item, "verifier_key_hash"),
                proofArtifactHash: try optionalHash(item, "proof_artifact_hash"),
                provingKeyHash: try optionalHash(item, "proving_key_hash"),
                destinationBindingKey: try SccpStrictJSON.text(item, "destination_binding_key"),
                destinationBindingHash: try hash(item, "destination_binding_hash"),
                browserProver: try optionalBrowser(item, "browser_prover")
            )
        }
        try requireUnique(routes.map(\.routeId), label: "outbound route ids")
        try requireUnique(routes.map(\.destinationBindingHash), label: "outbound destination bindings")
        return SccpProofManifestSet(
            version: 1,
            registryRevision: try hash(root, "registry_revision"),
            inboundNativeLanes: inboundLanes,
            outboundDestinationRoutes: routes
        )
    }

    static func recent(_ data: Data) throws -> SccpRecentMessages {
        let root = try SccpStrictJSON.object(data, label: "SCCP recent messages")
        try SccpStrictJSON.exactFields(root, ["items"], label: "SCCP recent messages")
        let items = try array(root, "items").enumerated().map { index, raw -> SccpRecentMessage in
            guard let item = raw as? [String: Any] else {
                throw SccpV1Error.invalid("items[\(index)] must be an object")
            }
            try SccpStrictJSON.exactFields(item, [
                "height", "message_id_hex", "kind", "source_profile", "target_profile",
                "destination_binding_hash", "target_domain", "counterparty_domain", "asset_id",
                "route_id", "recipient", "amount", "payload_projection", "links",
            ], label: "items[\(index)]")
            let lane = try lane(item, requireInbound: false)
            guard let kind = SccpPayloadKindV1(rawValue: try SccpStrictJSON.text(item, "kind")) else {
                throw SccpV1Error.invalid("recent SCCP message kind is unknown or retired")
            }
            let targetDomain = try SccpStrictJSON.uint32(item, "target_domain", minimum: 1, maximum: 5)
            let counterpartyDomain = try SccpStrictJSON.uint32(item, "counterparty_domain", minimum: 1, maximum: 5)
            guard targetDomain == lane.target.domainId, counterpartyDomain == lane.target.domainId else {
                throw SccpV1Error.invalid("recent SCCP message profile/domain mismatch")
            }
            let amount = try SccpStrictJSON.optionalText(item, "amount")
            if let amount,
               amount.first == "0" || !amount.allSatisfy(\.isNumber)
            {
                throw SccpV1Error.invalid("SCCP amount must be canonical positive decimal")
            }
            let links = try object(item, "links")
            try SccpStrictJSON.exactFields(links, ["bundle_path", "artifact_path", "job_path"], label: "recent message links")
            let projection: Data?
            if item["payload_projection"] is NSNull {
                projection = nil
            } else if let object = item["payload_projection"] as? [String: Any] {
                projection = try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
            } else {
                throw SccpV1Error.invalid("payload_projection must be an object or null")
            }
            return SccpRecentMessage(
                height: try SccpStrictJSON.uint64(item, "height", minimum: 1),
                messageIdHex: try hash(item, "message_id_hex"),
                kind: kind,
                lane: lane,
                destinationBindingHash: try hash(item, "destination_binding_hash"),
                assetId: try SccpStrictJSON.optionalText(item, "asset_id"),
                routeId: try SccpStrictJSON.optionalText(item, "route_id"),
                recipient: try SccpStrictJSON.optionalText(item, "recipient"),
                amount: amount,
                payloadProjectionJSON: projection,
                links: SccpRecentMessageLinks(
                    bundlePath: try path(links, "bundle_path"),
                    artifactPath: try path(links, "artifact_path"),
                    jobPath: try path(links, "job_path")
                )
            )
        }
        try requireUnique(items.map(\.messageIdHex), label: "recent message ids")
        for index in 1..<items.count where items[index].height > items[index - 1].height {
            throw SccpV1Error.invalid("recent SCCP messages must be newest first")
        }
        return SccpRecentMessages(items: items)
    }

    private static func inbound(_ raw: Any, label: String) throws -> SccpExactInboundLaneCapability {
        guard let item = raw as? [String: Any] else { throw SccpV1Error.invalid("\(label) must be an object") }
        try SccpStrictJSON.exactFields(item, [
            "source_profile", "target_profile", "source_domain", "target_domain",
            "source_identity_hash", "source_identity", "admission_enabled", "native_admission",
            "native_proof_builder",
        ], label: label)
        let exactLane = try lane(item, requireInbound: true)
        let identity = try sourceIdentity(object(item, "source_identity"), expectedLane: exactLane)
        let admission = try optionalNativeAdmission(item, source: exactLane.source)
        let enabled = try SccpStrictJSON.boolean(item, "admission_enabled")
        guard !enabled || admission != nil else {
            throw SccpV1Error.invalid("enabled native admission requires verifier metadata")
        }
        return SccpExactInboundLaneCapability(
            lane: exactLane,
            sourceIdentityHash: try hash(item, "source_identity_hash"),
            sourceIdentity: identity,
            admissionEnabled: enabled,
            nativeAdmission: admission,
            nativeProofBuilder: try optionalBrowser(item, "native_proof_builder")
        )
    }

    private static func lane(_ item: [String: Any], requireInbound: Bool) throws -> SccpLaneIdV1 {
        guard let source = SccpNetworkV1(rawValue: try SccpStrictJSON.text(item, "source_profile")),
              let target = SccpNetworkV1(rawValue: try SccpStrictJSON.text(item, "target_profile"))
        else { throw SccpV1Error.invalid("SCCP lane contains an unknown exact profile") }
        let result = try SccpLaneIdV1(source: source, target: target)
        guard result.isInbound == requireInbound else {
            throw SccpV1Error.invalid(requireInbound ? "lane must be external-to-SORA" : "lane must be SORA-to-external")
        }
        let sourceDomain = try SccpStrictJSON.uint32(item, "source_domain", minimum: 0, maximum: 5)
        let targetDomain = try SccpStrictJSON.uint32(item, "target_domain", minimum: 0, maximum: 5)
        guard sourceDomain == source.domainId, targetDomain == target.domainId else {
            throw SccpV1Error.invalid("SCCP lane profile/domain mismatch")
        }
        return result
    }

    private static func sourceIdentity(
        _ item: [String: Any],
        expectedLane: SccpLaneIdV1
    ) throws -> SccpSourceIdentityV1 {
        try SccpStrictJSON.exactFields(item, ["lane", "emitter"], label: "SCCP source identity")
        let laneObject = try object(item, "lane")
        try SccpStrictJSON.exactFields(laneObject, ["source", "target"], label: "SCCP source identity lane")
        let source = try network(object(laneObject, "source"))
        let target = try network(object(laneObject, "target"))
        let lane = try SccpLaneIdV1(source: source, target: target)
        guard lane == expectedLane else { throw SccpV1Error.invalid("source identity lane mismatch") }
        return SccpSourceIdentityV1(lane: lane, emitter: try emitter(object(item, "emitter"), source: source))
    }

    private static func network(_ item: [String: Any]) throws -> SccpNetworkV1 {
        try SccpStrictJSON.exactFields(item, ["network", "profile"], label: "SCCP network")
        guard item["profile"] is NSNull else {
            throw SccpV1Error.invalid("unit SCCP network profile content must be null")
        }
        let wire = try SccpStrictJSON.text(item, "network")
        guard wire.utf8.allSatisfy({ (97...122).contains($0) || $0 == 95 }),
              let profile = SccpNetworkV1(rawValue: wire.replacingOccurrences(of: "_", with: "-"))
        else { throw SccpV1Error.invalid("unsupported SCCP network profile") }
        return profile
    }

    private static func emitter(_ item: [String: Any], source: SccpNetworkV1) throws -> SccpSourceEmitterV1 {
        try SccpStrictJSON.exactFields(item, ["emitter", "identity"], label: "SCCP source emitter")
        let family = try SccpStrictJSON.text(item, "emitter")
        let identity = try object(item, "identity")
        switch family {
        case "evm":
            guard [SccpNetworkV1.ethereumMainnet, .ethereumSepolia, .bscMainnet, .bscTestnet].contains(source) else {
                throw SccpV1Error.invalid("source emitter family does not match exact profile")
            }
            try SccpStrictJSON.exactFields(identity, ["address", "runtime_code_hash", "route_config_hash"], label: "SCCP EVM emitter")
            return try .validatedEvm(
                address: upperHex(identity, "address", bytes: 20),
                runtimeCodeHash: upperHex(identity, "runtime_code_hash", bytes: 32),
                routeConfigHash: upperHex(identity, "route_config_hash", bytes: 32)
            )
        case "solana":
            guard source == .solanaMainnetBeta || source == .solanaTestnet else {
                throw SccpV1Error.invalid("source emitter family does not match exact profile")
            }
            try SccpStrictJSON.exactFields(identity, ["program_id", "executable_hash", "authorized_emitter"], label: "SCCP Solana emitter")
            return try .validatedSolana(
                programId: upperHex(identity, "program_id", bytes: 32),
                executableHash: upperHex(identity, "executable_hash", bytes: 32),
                authorizedEmitter: upperHex(identity, "authorized_emitter", bytes: 32)
            )
        case "ton":
            guard source == .tonMainnet || source == .tonTestnet else {
                throw SccpV1Error.invalid("source emitter family does not match exact profile")
            }
            try SccpStrictJSON.exactFields(identity, ["workchain", "account_id", "code_hash", "immutable_config_hash"], label: "SCCP TON emitter")
            return try .validatedTon(
                workchain: try int32(identity, "workchain"),
                accountId: upperHex(identity, "account_id", bytes: 32),
                codeHash: upperHex(identity, "code_hash", bytes: 32),
                immutableConfigHash: upperHex(identity, "immutable_config_hash", bytes: 32)
            )
        case "tron":
            guard source == .tronMainnet || source == .tronNile || source == .tronShasta else {
                throw SccpV1Error.invalid("source emitter family does not match exact profile")
            }
            try SccpStrictJSON.exactFields(identity, ["address", "runtime_code_hash", "route_config_hash"], label: "SCCP TRON emitter")
            return try .validatedTron(
                address: upperHex(identity, "address", bytes: 20),
                runtimeCodeHash: upperHex(identity, "runtime_code_hash", bytes: 32),
                routeConfigHash: upperHex(identity, "route_config_hash", bytes: 32)
            )
        default:
            throw SccpV1Error.invalid("unsupported SCCP source emitter")
        }
    }

    private static func optionalNativeAdmission(
        _ item: [String: Any],
        source: SccpNetworkV1
    ) throws -> SccpNativeAdmissionCapability? {
        guard let raw = item["native_admission"], !(raw is NSNull), let value = raw as? [String: Any] else {
            if let raw = item["native_admission"], !(raw is NSNull) {
                throw SccpV1Error.invalid("native_admission must be an object or null")
            }
            return nil
        }
        try SccpStrictJSON.exactFields(value, ["backend", "backend_label", "trust_anchor_hash"], label: "SCCP native admission")
        let backendObject = try object(value, "backend")
        try SccpStrictJSON.exactFields(backendObject, ["backend", "protocol"], label: "SCCP native backend")
        guard backendObject["protocol"] is NSNull,
              let backend = SccpNativeBackendV1(rawValue: try SccpStrictJSON.text(backendObject, "backend")),
              backend.supports(source),
              try SccpStrictJSON.text(value, "backend_label") == backend.backendLabel
        else { throw SccpV1Error.invalid("native backend does not match its exact source profile or label") }
        return SccpNativeAdmissionCapability(
            backend: backend,
            backendLabel: backend.backendLabel,
            trustAnchorHash: try hash(value, "trust_anchor_hash")
        )
    }

    private static func optionalBrowser(_ item: [String: Any], _ field: String) throws -> SccpBrowserProverManifestRef? {
        guard let raw = item[field], !(raw is NSNull), let value = raw as? [String: Any] else {
            if let raw = item[field], !(raw is NSNull) {
                throw SccpV1Error.invalid("\(field) must be an object or null")
            }
            return nil
        }
        try SccpStrictJSON.exactFields(value, [
            "module_url", "module_specifier", "module_hash", "manifest_hash", "expected_exports",
            "bound_route_hash", "bound_proof_hash",
        ], label: "SCCP browser prover")
        let urlText = try SccpStrictJSON.text(value, "module_url")
        guard let url = URL(string: urlText), url.absoluteString == urlText,
              ["http", "https"].contains(url.scheme), url.host != nil
        else { throw SccpV1Error.invalid("module_url must be an absolute HTTP(S) URL") }
        let exports = try stringArray(value, "expected_exports")
        try requireUnique(exports, label: "browser prover exports")
        guard !exports.isEmpty else { throw SccpV1Error.invalid("browser prover exports must not be empty") }
        return SccpBrowserProverManifestRef(
            moduleURL: url,
            moduleSpecifier: try SccpStrictJSON.optionalText(value, "module_specifier"),
            moduleHash: try hash(value, "module_hash"),
            manifestHash: try hash(value, "manifest_hash"),
            expectedExports: exports,
            boundRouteHash: try hash(value, "bound_route_hash"),
            boundProofHash: try hash(value, "bound_proof_hash")
        )
    }

    private static func object(_ item: [String: Any], _ field: String) throws -> [String: Any] {
        guard let value = item[field] as? [String: Any] else {
            throw SccpV1Error.invalid("\(field) must be an object")
        }
        return value
    }

    private static func array(_ item: [String: Any], _ field: String) throws -> [Any] {
        guard let value = item[field] as? [Any] else {
            throw SccpV1Error.invalid("\(field) must be an array")
        }
        return value
    }

    private static func stringArray(_ item: [String: Any], _ field: String) throws -> [String] {
        try array(item, field).enumerated().map { index, raw in
            guard let value = raw as? String, !value.isEmpty,
                  value == value.trimmingCharacters(in: .whitespacesAndNewlines)
            else { throw SccpV1Error.invalid("\(field)[\(index)] must be canonical nonempty text") }
            return value
        }
    }

    private static func path(_ item: [String: Any], _ field: String) throws -> String {
        let value = try SccpStrictJSON.text(item, field)
        guard value.first == "/", !value.contains("//"), !value.contains("?"), !value.contains("#") else {
            throw SccpV1Error.invalid("\(field) must be a canonical absolute Torii path")
        }
        return value
    }

    private static func optionalPath(_ item: [String: Any], _ field: String) throws -> String? {
        guard !(item[field] is NSNull) else { return nil }
        return try path(item, field)
    }

    private static func hash(_ item: [String: Any], _ field: String) throws -> String {
        let value = try SccpStrictJSON.text(item, field)
        guard value.count == 66, value.hasPrefix("0x") else {
            throw SccpV1Error.invalid("\(field) must be canonical lowercase nonzero 32-byte hex")
        }
        _ = try SccpSubmitValidation.responseHash(String(value.dropFirst(2)), field: field)
        return value
    }

    private static func optionalHash(_ item: [String: Any], _ field: String) throws -> String? {
        guard !(item[field] is NSNull) else { return nil }
        return try hash(item, field)
    }

    private static func upperHex(_ item: [String: Any], _ field: String, bytes: Int) throws -> Data {
        let value = try SccpStrictJSON.text(item, field)
        guard value.count == bytes * 2,
              value.utf8.allSatisfy({ (48...57).contains($0) || (65...70).contains($0) }),
              value.contains(where: { $0 != "0" }),
              let data = Data(hexString: value)
        else { throw SccpV1Error.invalid("\(field) must be canonical uppercase nonzero fixed hex") }
        return data
    }

    private static func int32(_ item: [String: Any], _ field: String) throws -> Int32 {
        guard let number = item[field] as? NSNumber,
              number.doubleValue.rounded(.towardZero) == number.doubleValue,
              number.doubleValue >= Double(Int32.min), number.doubleValue <= Double(Int32.max)
        else { throw SccpV1Error.invalid("\(field) must fit i32") }
        return number.int32Value
    }

    private static func requireUnique(_ values: [String], label: String) throws {
        guard Set(values).count == values.count else {
            throw SccpV1Error.invalid("\(label) must be unique")
        }
    }
}
