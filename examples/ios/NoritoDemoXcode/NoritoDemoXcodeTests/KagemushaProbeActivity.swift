// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import Foundation

/// Test-host observation DATA, never a monetary or Archive operation.
struct KagemushaProbeActivity: Equatable, Sendable {
  enum Battery: Equatable { case unknown, unplugged, charging, full }
  let foreground: Bool
  let charging: Bool
  static let inactive = Self(foreground: false, charging: false)

  static func observed(appInBackground: Bool, foregroundScene: Bool, battery: Battery) -> Self {
    Self(
      foreground: !appInBackground && foregroundScene,
      charging: battery == .charging || battery == .full)
  }
}

/// One exact wallet lifetime. The queue is independent of the probe actor's synchronous calls.
final class KagemushaProbeActivityBinding: @unchecked Sendable {
  enum Failure: Error { case retired, initialPublicationMissing }
  struct Retirement {
    let closed: Bool
    let activityFailure: Error?
    let closeFailure: Error?
    func check() throws {
      if let closeFailure { throw closeFailure }
      if let activityFailure { throw activityFailure }
    }
  }
  typealias Job = @Sendable () -> Void
  private let enqueue: (@escaping Job) -> Void
  private let apply: @Sendable (KagemushaProbeActivity) throws -> Void
  private let closeOwner: @Sendable () throws -> Void
  private let lock = NSLock()
  private var accepting = true
  private var firstFailure: Error?
  private var published = false
  // Only the serial control executor accesses these two fields.
  private var closeAttempted = false
  private var ownerClosed = false

  /// An injected executor must be serial, FIFO and must never run a submitted job inline.
  init(
    enqueue: ((@escaping Job) -> Void)? = nil,
    apply: @escaping @Sendable (KagemushaProbeActivity) throws -> Void,
    closeOwner: @escaping @Sendable () throws -> Void
  ) {
    let queue = DispatchQueue(label: "org.hyperledger.iroha.probe.activity." + UUID().uuidString)
    self.enqueue = enqueue ?? { job in queue.async(execute: job) }
    self.apply = apply
    self.closeOwner = closeOwner
  }

  func publish(_ activity: KagemushaProbeActivity) {
    lock.lock()
    defer { lock.unlock() }
    guard accepting else { return }
    // Enqueue under the admission lock so invalidate cannot put its fence ahead of this job.
    enqueue { [self] in
      lock.lock()
      let admitted = accepting && firstFailure == nil
      lock.unlock()
      guard admitted else { return }
      do {
        try apply(activity)
        lock.lock()
        published = true
        lock.unlock()
      } catch { retain(error) }
    }
  }

  private func retain(_ error: Error) {
    lock.lock()
    if firstFailure == nil { firstFailure = error }
    lock.unlock()
  }

  /// Idle mailbox polling reports the sticky error without treating retirement as a new failure.
  func checkFailure() throws {
    lock.lock()
    defer { lock.unlock() }
    if let firstFailure { throw firstFailure }
  }

  func requireHealthy() throws {
    lock.lock()
    defer { lock.unlock() }
    if let firstFailure { throw firstFailure }
    guard accepting else { throw Failure.retired }
    guard published else { throw Failure.initialPublicationMissing }
  }

  /// A barrier always resolves, even if retirement dropped the initial queued update.
  func acknowledgeInitial() async throws {
    try await withCheckedThrowingContinuation { (reply: CheckedContinuation<Void, Error>) in
      enqueue { [self] in
        do { try requireHealthy(); reply.resume() }
        catch { reply.resume(throwing: error) }
      }
    }
  }

  func invalidate() {
    lock.lock()
    accepting = false
    lock.unlock()
  }

  /// Detach OS observations before awaiting this fence. Cleanup is never gated by health.
  func retire(close: Bool) async -> Retirement {
    invalidate()
    return await withCheckedContinuation { reply in
      enqueue { [self] in
        // SDK close permanently retires its lease even on failure. A retry must only close it.
        if !closeAttempted {
          do { try apply(.inactive) }
          catch { retain(error) }
        }
        var closeFailure: Error?
        if close && !ownerClosed {
          closeAttempted = true
          do { try closeOwner(); ownerClosed = true }
          catch { closeFailure = error }
        }
        lock.lock()
        let failure = firstFailure
        lock.unlock()
        reply.resume(returning: Retirement(
          closed: ownerClosed, activityFailure: failure, closeFailure: closeFailure))
      }
    }
  }
}

/// Actor-owned gate covers suspension during Native open, initial publication and teardown.
struct KagemushaProbeWalletLifecycle {
  enum Phase { case empty, opening, ready, retiring }
  enum Failure: Error { case transition, notReady }
  private(set) var phase: Phase = .empty
  private var transitioning = false
  func requireIdle() throws {
    guard !transitioning else { throw Failure.transition }
  }
  func requireReady() throws {
    try requireIdle()
    guard phase == .ready else { throw Failure.notReady }
  }
  mutating func beginOpen() throws {
    try requireIdle()
    guard phase == .empty else { throw Failure.notReady }
    phase = .opening
    transitioning = true
  }
  mutating func finishOpen(ownerRetained: Bool, ready: Bool) {
    phase = ready ? .ready : (ownerRetained ? .retiring : .empty)
    transitioning = false
  }
  mutating func beginRetirement() throws {
    try requireIdle()
    phase = .retiring
    transitioning = true
  }
  mutating func finishRetirement(closed: Bool) {
    phase = closed ? .empty : .retiring
    transitioning = false
  }
}

/// Boolean battery monitoring has no platform lease API; share one lease in this test host.
@MainActor final class KagemushaProbeBatteryMonitoring {
  private var users = 0
  private var previous = false
  private let enabled: () -> Bool
  private let setEnabled: (Bool) -> Void
  init(enabled: @escaping () -> Bool, setEnabled: @escaping (Bool) -> Void) {
    self.enabled = enabled
    self.setEnabled = setEnabled
  }
  func acquire() {
    if users == 0 {
      previous = enabled()
      setEnabled(true)
    }
    users += 1
  }
  func release() {
    precondition(users > 0)
    users -= 1
    if users == 0 { setEnabled(previous) }
  }
}

/// Subscribe and snapshot on MainActor; each callback captures a complete pair before returning.
@MainActor final class KagemushaProbeActivityObservation {
  private let binding: KagemushaProbeActivityBinding
  private let sample: () -> KagemushaProbeActivity
  private var detach: (() -> Void)?
  private var active = true

  init(
    binding: KagemushaProbeActivityBinding,
    subscribe: (@escaping @MainActor @Sendable () -> Void) -> () -> Void,
    sample: @escaping () -> KagemushaProbeActivity
  ) {
    self.binding = binding
    self.sample = sample
    detach = subscribe { [weak self] in self?.changed() }
    // Subscription precedes the snapshot; any synchronous subscription callback is also ordered.
    changed()
  }
  private func changed() {
    guard active else { return }
    binding.publish(sample())
  }
  func stop() {
    active = false
    binding.invalidate()
    detach?()
    detach = nil
  }
}

#if os(iOS) && !targetEnvironment(macCatalyst)
  import UIKit

  extension KagemushaProbeActivityObservation {
    private static let battery = KagemushaProbeBatteryMonitoring(
      enabled: { UIDevice.current.isBatteryMonitoringEnabled },
      setEnabled: { UIDevice.current.isBatteryMonitoringEnabled = $0 })

    static func phone(binding: KagemushaProbeActivityBinding) -> KagemushaProbeActivityObservation {
      Self(
        binding: binding,
        subscribe: { callback in
          let center = NotificationCenter.default
          let names: [Notification.Name] = [
            UIApplication.didEnterBackgroundNotification, UIApplication.willEnterForegroundNotification,
            UIApplication.didBecomeActiveNotification, UIApplication.willResignActiveNotification,
            UIScene.willConnectNotification, UIScene.didDisconnectNotification,
            UIScene.didActivateNotification, UIScene.willDeactivateNotification,
            UIScene.willEnterForegroundNotification, UIScene.didEnterBackgroundNotification,
            UIDevice.batteryStateDidChangeNotification,
          ]
          let tokens = names.map { name in
            center.addObserver(forName: name, object: nil, queue: .main) { _ in
              // The specified main queue supplies isolation. Do not defer sampling in a Task:
              // a background/unplugged edge followed by foreground must reach Native in order.
              MainActor.assumeIsolated { callback() }
            }
          }
          battery.acquire()
          return {
            tokens.forEach { center.removeObserver($0) }
            battery.release()
          }
        },
        sample: {
          let charging: KagemushaProbeActivity.Battery
          switch UIDevice.current.batteryState {
          case .charging: charging = .charging
          case .full: charging = .full
          case .unplugged: charging = .unplugged
          case .unknown: charging = .unknown
          @unknown default: charging = .unknown
          }
          let foregroundScene = UIApplication.shared.connectedScenes.contains {
            $0.activationState == .foregroundActive || $0.activationState == .foregroundInactive
          }
          return .observed(
            appInBackground: UIApplication.shared.applicationState == .background,
            foregroundScene: foregroundScene, battery: charging)
        })
    }
  }
#endif
