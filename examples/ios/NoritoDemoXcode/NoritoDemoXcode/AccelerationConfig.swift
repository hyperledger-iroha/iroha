#if canImport(IrohaSwift)
import Foundation
import IrohaSwift

struct DemoAccelerationConfig {
  static func load(logger: ((String) -> Void)? = nil) throws -> AccelerationSettings {
    try AccelerationSettingsLoader.load(
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
