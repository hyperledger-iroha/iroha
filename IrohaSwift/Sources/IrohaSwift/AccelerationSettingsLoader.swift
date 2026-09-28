import Foundation

/// Loads caller-selected or bundled canonical `iroha_config` files.
/// Invalid present files throw; they cannot silently enable default acceleration.
public enum AccelerationSettingsLoader {
    /// Configuration files searched when the caller supplies no explicit URL.
    public static let defaultBundleCandidates: [ResourceCandidate] = [
        ResourceCandidate(name: "acceleration", fileExtension: "json"),
        ResourceCandidate(name: "acceleration", fileExtension: "toml"),
        ResourceCandidate(name: "client", fileExtension: "json"),
        ResourceCandidate(name: "client", fileExtension: "toml")
    ]

    public struct ResourceCandidate: Sendable {
        public let name: String
        public let fileExtension: String?

        public init(name: String, fileExtension: String?) {
            self.name = name
            self.fileExtension = fileExtension
        }
    }

    /// Load explicit file policy first, then a bundled file, or enabled defaults when absent.
    public static func load(
        configurationURL: URL? = nil,
        bundle: Bundle? = .main,
        resourceCandidates: [ResourceCandidate] = defaultBundleCandidates,
        logger: ((String) -> Void)? = nil
    ) throws -> AccelerationSettings {
        if let configurationURL {
            return try loadConfig(at: configurationURL, logger: logger)
        }
        if let bundle,
           let settings = try loadFromBundle(bundle: bundle, candidates: resourceCandidates, logger: logger) {
            return settings
        }
        logger?("AccelerationSettingsLoader: no configuration file; using defaults.")
        return AccelerationSettings()
    }

    /// Return nil only when no candidate exists. Malformed file policy throws.
    public static func loadFromBundle(
        bundle: Bundle,
        candidates: [ResourceCandidate] = defaultBundleCandidates,
        logger: ((String) -> Void)? = nil
    ) throws -> AccelerationSettings? {
        for candidate in candidates {
            if let url = bundle.url(forResource: candidate.name, withExtension: candidate.fileExtension) {
                return try loadConfig(at: url, logger: logger)
            }
        }
        return nil
    }

    private static func loadConfig(at url: URL, logger: ((String) -> Void)?) throws -> AccelerationSettings {
        let settings = try AccelerationSettings.fromIrohaConfigFile(at: url)
        logger?("AccelerationSettingsLoader: loaded \(url.lastPathComponent).")
        return settings
    }
}
