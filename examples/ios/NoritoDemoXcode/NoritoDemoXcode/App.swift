import SwiftUI
#if canImport(IrohaSwift)
import IrohaSwift
#endif

@main
struct NoritoDemoXcodeApp: App {
  init() {
#if canImport(IrohaSwift)
    do {
      try DemoAccelerationConfig.load().apply()
    } catch {
      fatalError("Invalid bundled acceleration configuration: \(error)")
    }
#endif
  }

  var body: some Scene {
    WindowGroup { ContentView() }
  }
}
