import SwiftUI
#if canImport(IrohaSwift)
import IrohaSwift
#endif

@main
struct NoritoDemoXcodeApp: App {
  init() {
#if canImport(IrohaSwift)
    DemoAccelerationConfig.load().apply()
#endif
  }

  var body: some Scene {
    WindowGroup { ContentView() }
  }
}
