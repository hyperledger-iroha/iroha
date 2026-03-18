import Foundation

/// Helper for loading `AccelerationSettings` from operator-provided
/// `iroha_config` documents or bundled defaults.
public enum AccelerationSettingsLoader {
    /// Default environment key mirroring the Norito demo and CI harnesses.
    public static let defaultEnvironmentKey = "NORITO_ACCEL_CONFIG_PATH"

    /// Resource candidates searched inside bundles when no environment override is present.
    public static let defaultBundleCandidates: [ResourceCandidate] = [
        ResourceCandidate(name: "acceleration", fileExtension: "json"),
        ResourceCandidate(name: "acceleration", fileExtension: "toml"),
        ResourceCandidate(name: "client", fileExtension: "json"),
        ResourceCandidate(name: "client", fileExtension: "toml")
    ]

    /// Represents a bundle resource to probe when loading acceleration settings.
    public struct ResourceCandidate: Sendable {
        public let name: String
        public let fileExtension: String?

        public init(name: String, fileExtension: String?) {
            self.name = name
            self.fileExtension = fileExtension
        }
    }

    /// Resolve acceleration settings by checking the environment first and then bundled configs.
    /// Falls back to the default `AccelerationSettings` when no valid manifest is found.
    public static func load(
        environmentKey: String = defaultEnvironmentKey,
        environment: [String: String] = ProcessInfo.processInfo.environment,
        bundle: Bundle? = .main,
        resourceCandidates: [ResourceCandidate] = defaultBundleCandidates,
        logger: ((String) -> Void)? = nil
    ) -> AccelerationSettings {
        if let settings = loadFromEnvironment(
            key: environmentKey,
            environment: environment,
            logger: logger
        ) {
            return settings
        }
        if let bundle,
           let settings = loadFromBundle(
               bundle: bundle,
               candidates: resourceCandidates,
               logger: logger
           ) {
            return settings
        }
        logger?("AccelerationSettingsLoader: no acceleration config found; using defaults.")
        return AccelerationSettings()
    }

    /// Attempt to load the acceleration config from an environment-provided file path.
    /// Returns `nil` when the key is missing or the manifest cannot be parsed.
    public static func loadFromEnvironment(
        key: String = defaultEnvironmentKey,
        environment: [String: String] = ProcessInfo.processInfo.environment,
        logger: ((String) -> Void)? = nil
    ) -> AccelerationSettings? {
        guard let rawPath = environment[key]?.trimmingCharacters(in: .whitespacesAndNewlines),
              !rawPath.isEmpty,
              let url = resolve(path: rawPath) else {
            return nil
        }
        return loadConfig(at: url, sourceDescription: "environment key \(key)", logger: logger)
    }

    /// Attempt to load the acceleration config from bundle resources.
    /// Returns `nil` when no candidate file is available or parsing fails.
    public static func loadFromBundle(
        bundle: Bundle,
        candidates: [ResourceCandidate] = defaultBundleCandidates,
        logger: ((String) -> Void)? = nil
    ) -> AccelerationSettings? {
        for candidate in candidates {
            if let url = bundle.url(forResource: candidate.name, withExtension: candidate.fileExtension),
               let settings = loadConfig(
                   at: url,
                   sourceDescription: candidate.description,
                   logger: logger
               ) {
                return settings
            }
        }
        return nil
    }

    private static func loadConfig(
        at url: URL,
        sourceDescription: String,
        logger: ((String) -> Void)?
    ) -> AccelerationSettings? {
        do {
            let settings = try AccelerationSettings.fromIrohaConfigFile(at: url)
            logger?("AccelerationSettingsLoader: loaded acceleration settings from \(sourceDescription).")
            return settings
        } catch {
            logger?("AccelerationSettingsLoader: failed to parse \(url.path): \(error)")
            return nil
        }
    }

    private static func resolve(path: String) -> URL? {
        let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        if trimmed.hasPrefix("file://"), let url = URL(string: trimmed) {
            return url
        }
        let expanded = (trimmed as NSString).expandingTildeInPath
        return URL(fileURLWithPath: expanded)
    }
}

private extension AccelerationSettingsLoader.ResourceCandidate {
    var description: String {
        if let fileExtension {
            return "\(name).\(fileExtension)"
        }
        return name
    }
}
