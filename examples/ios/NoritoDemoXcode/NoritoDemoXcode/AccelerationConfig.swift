#if canImport(IrohaSwift)
import Foundation
import IrohaSwift

struct DemoAccelerationConfig {
  static let environmentKey = AccelerationSettingsLoader.defaultEnvironmentKey

  static func load(logger: ((String) -> Void)? = nil) -> AccelerationSettings {
    AccelerationSettingsLoader.load(
      environmentKey: environmentKey,
      environment: ProcessInfo.processInfo.environment,
      bundle: .main,
      logger: { message in
        if let logger {
          logger("NoritoDemo: \(message)")
        }
      }
    )
  }
}
#endif
