import SwiftUI
#if canImport(IrohaSwift)
import IrohaSwift
#endif

@main
struct NoritoDemoApp: App {
  init() {
#if canImport(IrohaSwift)
    var settings = AccelerationSettings()
    settings.apply()
#endif
  }

  var body: some Scene {
    WindowGroup { ContentView() }
  }
}
