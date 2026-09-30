import Foundation
import IrohaSwift

struct DemoAccelerationConfig {
  static func load(configurationURL: URL? = nil,
                   bundle: Bundle? = .main,
                   logger: ((String) -> Void)? = nil) throws -> AccelerationSettings {
    try AccelerationSettingsLoader.load(
      configurationURL: configurationURL,
      bundle: bundle,
      logger: { message in
        if let logger {
          logger("NoritoDemo: \(message)")
        }
      }
    )
  }
}
