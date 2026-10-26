// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import Foundation
import XCTest

final class KagemushaProbeActivityTests: XCTestCase {
  private typealias Activity = KagemushaProbeActivity
  private typealias Binding = KagemushaProbeActivityBinding
  private enum Injected: Error, Equatable { case activity, close }

  /// Deterministic FIFO seam: submission never executes Native-shaped work inline.
  private final class Manual: @unchecked Sendable {
    private let lock = NSLock()
    private var jobs: [Binding.Job] = []
    private var waiting: CheckedContinuation<Void, Never>?
    func enqueue(_ job: @escaping Binding.Job) {
      lock.lock()
      jobs.append(job)
      let reply = waiting
      waiting = nil
      lock.unlock()
      reply?.resume()
    }
    func waitForJob() async {
      await withCheckedContinuation { reply in
        lock.lock()
        if !jobs.isEmpty { lock.unlock(); reply.resume() }
        else { waiting = reply; lock.unlock() }
      }
    }
    func drain() {
      while true {
        lock.lock()
        let job = jobs.isEmpty ? nil : jobs.removeFirst()
        lock.unlock()
        guard let job else { return }
        job()
      }
    }
  }
  private final class Recorder: @unchecked Sendable {
    private let lock = NSLock()
    private var recorded: [Activity] = []
    private var closes = 0
    var failActivity = false
    var failCloses = 0
    func apply(_ value: Activity) throws {
      lock.lock()
      defer { lock.unlock() }
      recorded.append(value)
      if failActivity { throw Injected.activity }
    }
    func close() throws {
      lock.lock()
      defer { lock.unlock() }
      closes += 1
      if closes <= failCloses { throw Injected.close }
    }
    var values: [Activity] {
      lock.lock(); defer { lock.unlock() }; return recorded
    }
    var closeCount: Int {
      lock.lock(); defer { lock.unlock() }; return closes
    }
    func binding(_ executor: Manual) -> Binding {
      Binding(enqueue: executor.enqueue, apply: apply, closeOwner: close)
    }
  }
  @MainActor private func retire(_ binding: Binding, _ executor: Manual, close: Bool = true) async
    -> Binding.Retirement
  {
    let task = Task { await binding.retire(close: close) }
    await executor.waitForJob()
    executor.drain()
    return await task.value
  }

  func testOSPolicyKeepsForegroundInactiveAndOnlyChargingOrFull() {
    let batteries: [Activity.Battery] = [.unknown, .unplugged, .charging, .full]
    for battery in batteries {
      let charging = battery == .charging || battery == .full
      // The adapter includes both foregroundActive and foregroundInactive scenes.
      XCTAssertEqual(Activity.observed(appInBackground: false, foregroundScene: true, battery: battery),
        Activity(foreground: true, charging: charging))
      XCTAssertEqual(Activity.observed(appInBackground: true, foregroundScene: true, battery: battery),
        Activity(foreground: false, charging: charging))
      XCTAssertEqual(Activity.observed(appInBackground: false, foregroundScene: false, battery: battery),
        Activity(foreground: false, charging: charging))
    }
  }

  @MainActor func testInitialPublicationMustBeAppliedBeforeAcknowledgementAndCommands() async throws {
    let executor = Manual(), record = Recorder()
    let binding = record.binding(executor)
    XCTAssertThrowsError(try binding.requireHealthy())
    XCTAssertNoThrow(try binding.checkFailure())
    binding.publish(Activity(foreground: true, charging: false))
    XCTAssertTrue(record.values.isEmpty)
    XCTAssertThrowsError(try binding.requireHealthy())
    executor.drain()
    let acknowledgement = Task { try await binding.acknowledgeInitial() }
    await executor.waitForJob()
    executor.drain()
    try await acknowledgement.value
    XCTAssertNoThrow(try binding.requireHealthy())
    XCTAssertEqual(record.values, [Activity(foreground: true, charging: false)])
  }

  @MainActor func testSubscribeBeforeSampleAndRapidEdgesKeepFullPairs() async {
    let executor = Manual(), record = Recorder()
    let binding = record.binding(executor)
    var callback: (@MainActor @Sendable () -> Void)?
    var current = Activity(foreground: true, charging: true)
    var subscribed = false
    var detached = false
    let observation = KagemushaProbeActivityObservation(
      binding: binding,
      subscribe: { changed in
        subscribed = true
        callback = changed
        // Change during registration, before the constructor's initial snapshot.
        current = Activity(foreground: false, charging: true)
        changed()
        return { detached = true }
      },
      sample: { XCTAssertTrue(subscribed); return current })
    current = .inactive
    callback?()
    current = Activity(foreground: true, charging: false)
    callback?()
    XCTAssertTrue(record.values.isEmpty)
    executor.drain()
    XCTAssertEqual(record.values, [
      Activity(foreground: false, charging: true), Activity(foreground: false, charging: true),
      .inactive, Activity(foreground: true, charging: false),
    ])
    observation.stop()
    XCTAssertTrue(detached)
    callback?() // NotificationCenter may already have admitted a callback when removed.
    _ = await retire(binding, executor)
    XCTAssertEqual(record.values.last, .inactive)
    XCTAssertEqual(record.values.count, 5)
  }

  @MainActor func testRetirementDropsQueuedInitialUpdateAndResolvesAcknowledgement() async {
    let executor = Manual(), record = Recorder()
    let binding = record.binding(executor)
    binding.publish(Activity(foreground: true, charging: false))
    binding.invalidate()
    executor.drain()
    let acknowledgement = Task { try await binding.acknowledgeInitial() }
    await executor.waitForJob()
    executor.drain()
    do { try await acknowledgement.value; XCTFail("retired setup must fail") }
    catch { XCTAssertTrue(error is Binding.Failure) }
    XCTAssertTrue(record.values.isEmpty)
    let result = await retire(binding, executor)
    XCTAssertTrue(result.closed)
    XCTAssertEqual(record.values, [.inactive])
  }

  @MainActor func testStickyInitialFailureStillPublishesInactiveAndCloses() async {
    let executor = Manual(), record = Recorder()
    record.failActivity = true
    let binding = record.binding(executor)
    binding.publish(Activity(foreground: true, charging: false))
    executor.drain()
    let acknowledgement = Task { try await binding.acknowledgeInitial() }
    await executor.waitForJob()
    executor.drain()
    do { try await acknowledgement.value; XCTFail("initial setter failed") }
    catch { XCTAssertEqual(error as? Injected, .activity) }
    binding.publish(Activity(foreground: true, charging: true))
    executor.drain()
    XCTAssertEqual(record.values.count, 1)
    let result = await retire(binding, executor)
    XCTAssertTrue(result.closed)
    XCTAssertNil(result.closeFailure)
    XCTAssertEqual(result.activityFailure as? Injected, .activity)
    XCTAssertThrowsError(try result.check()) { XCTAssertEqual($0 as? Injected, .activity) }
    XCTAssertEqual(record.values, [Activity(foreground: true, charging: false), .inactive])
    XCTAssertEqual(record.closeCount, 1)
    XCTAssertThrowsError(try binding.requireHealthy()) { XCTAssertEqual($0 as? Injected, .activity) }
    XCTAssertThrowsError(try binding.checkFailure()) { XCTAssertEqual($0 as? Injected, .activity) }
  }

  @MainActor func testFinalInactiveFailureCannotSkipClose() async {
    let executor = Manual(), record = Recorder()
    let binding = record.binding(executor)
    binding.publish(Activity(foreground: true, charging: false))
    executor.drain()
    record.failActivity = true
    let result = await retire(binding, executor)
    XCTAssertTrue(result.closed)
    XCTAssertEqual(record.closeCount, 1)
    XCTAssertEqual(result.activityFailure as? Injected, .activity)
  }

  @MainActor func testLaterCallbackFailureRemainsVisibleToIdlePollingAndCommands() async {
    let executor = Manual(), record = Recorder()
    let binding = record.binding(executor)
    binding.publish(Activity(foreground: true, charging: false))
    executor.drain()
    XCTAssertNoThrow(try binding.requireHealthy())
    record.failActivity = true
    binding.publish(.inactive)
    executor.drain()
    record.failActivity = false
    binding.publish(Activity(foreground: true, charging: true))
    executor.drain()
    XCTAssertThrowsError(try binding.checkFailure()) { XCTAssertEqual($0 as? Injected, .activity) }
    XCTAssertThrowsError(try binding.requireHealthy()) { XCTAssertEqual($0 as? Injected, .activity) }
    XCTAssertEqual(record.values, [Activity(foreground: true, charging: false), .inactive])
    let result = await retire(binding, executor)
    XCTAssertTrue(result.closed)
    XCTAssertEqual(result.activityFailure as? Injected, .activity)
  }

  @MainActor func testCloseFailureRetainsRetiredOwnerAndRetriesWithoutActivityOnClosedLease() async {
    let executor = Manual(), record = Recorder()
    record.failCloses = 1
    let binding = record.binding(executor)
    binding.publish(Activity(foreground: true, charging: false))
    // Queued active work must not overtake the final inactive/close fence.
    binding.invalidate()
    executor.drain()
    let first = await retire(binding, executor)
    XCTAssertFalse(first.closed)
    XCTAssertEqual(first.closeFailure as? Injected, .close)
    XCTAssertThrowsError(try first.check()) { XCTAssertEqual($0 as? Injected, .close) }
    binding.publish(Activity(foreground: true, charging: true))
    let second = await retire(binding, executor)
    XCTAssertTrue(second.closed)
    XCTAssertNoThrow(try second.check())
    XCTAssertEqual(record.closeCount, 2)
    XCTAssertEqual(record.values, [.inactive])
  }

  @MainActor func testSetupRetirementKeepsOwnerUntilExplicitClose() async {
    let executor = Manual(), record = Recorder()
    let binding = record.binding(executor)
    let setup = await retire(binding, executor, close: false)
    XCTAssertFalse(setup.closed)
    XCTAssertEqual(record.closeCount, 0)
    XCTAssertThrowsError(try binding.requireHealthy())
    let cleanup = await retire(binding, executor)
    XCTAssertTrue(cleanup.closed)
    XCTAssertEqual(record.closeCount, 1)
    XCTAssertEqual(record.values, [.inactive, .inactive])
  }

  @MainActor func testOldCallbacksCannotTouchReopenedOwner() async {
    let executor = Manual(), old = Recorder(), fresh = Recorder()
    let oldBinding = old.binding(executor)
    var oldCallback: (@MainActor @Sendable () -> Void)?
    let observation = KagemushaProbeActivityObservation(
      binding: oldBinding, subscribe: { oldCallback = $0; return {} },
      sample: { Activity(foreground: true, charging: false) })
    executor.drain()
    observation.stop()
    _ = await retire(oldBinding, executor)
    let freshBinding = fresh.binding(executor)
    freshBinding.publish(Activity(foreground: false, charging: true))
    oldCallback?()
    oldBinding.publish(Activity(foreground: true, charging: true))
    executor.drain()
    XCTAssertEqual(old.values, [Activity(foreground: true, charging: false), .inactive])
    XCTAssertEqual(fresh.values, [Activity(foreground: false, charging: true)])
    XCTAssertEqual(old.closeCount, 1)
    XCTAssertEqual(fresh.closeCount, 0)
  }

  func testLifecycleGuardsOpeningReadyRetirementAndFailedSetup() throws {
    var gate = KagemushaProbeWalletLifecycle()
    try gate.beginOpen()
    XCTAssertThrowsError(try gate.requireIdle())
    XCTAssertThrowsError(try gate.requireReady())
    XCTAssertThrowsError(try gate.beginRetirement())
    gate.finishOpen(ownerRetained: true, ready: true)
    XCTAssertNoThrow(try gate.requireReady())
    try gate.beginRetirement()
    XCTAssertThrowsError(try gate.requireReady())
    gate.finishRetirement(closed: false)
    XCTAssertThrowsError(try gate.beginOpen())
    XCTAssertThrowsError(try gate.requireReady())
    try gate.beginRetirement()
    gate.finishRetirement(closed: true)
    try gate.beginOpen()
    gate.finishOpen(ownerRetained: true, ready: false)
    XCTAssertThrowsError(try gate.beginOpen())
    try gate.beginRetirement()
    gate.finishRetirement(closed: true)
    try gate.beginOpen()
    gate.finishOpen(ownerRetained: false, ready: false)
    XCTAssertNoThrow(try gate.beginOpen())
  }

  @MainActor func testBatteryLeasePreservesExistingMonitoringAndMultipleOwners() {
    for original in [false, true] {
      var enabled = original
      var writes: [Bool] = []
      let monitoring = KagemushaProbeBatteryMonitoring(
        enabled: { enabled }, setEnabled: { enabled = $0; writes.append($0) })
      monitoring.acquire()
      monitoring.acquire()
      XCTAssertTrue(enabled)
      monitoring.release()
      XCTAssertTrue(enabled)
      XCTAssertEqual(writes, [true])
      monitoring.release()
      XCTAssertEqual(enabled, original)
      XCTAssertEqual(writes, [true, original])
    }
  }

  func testControlQueueDeliversInactiveWhileCommandActorIsBlocked() async throws {
    actor BusyCommand {
      func run(_ entered: XCTestExpectation, _ release: DispatchSemaphore) {
        entered.fulfill()
        release.wait()
      }
    }
    let entered = expectation(description: "synchronous command entered")
    let inactive = expectation(description: "inactive delivered before command returns")
    let release = DispatchSemaphore(value: 0)
    let command = Task { await BusyCommand().run(entered, release) }
    defer { release.signal() }
    await fulfillment(of: [entered], timeout: 2)
    let binding = Binding(
      apply: { value in
        XCTAssertFalse(Thread.isMainThread)
        if value == .inactive { inactive.fulfill() }
      }, closeOwner: { XCTAssertFalse(Thread.isMainThread) })
    binding.publish(.inactive)
    try await binding.acknowledgeInitial()
    await fulfillment(of: [inactive], timeout: 2)
    release.signal()
    await command.value
    // Avoid fulfilling the single-delivery expectation again during cleanup.
    binding.invalidate()
  }

  func testRetirementFencesRunningSetterBeforeInactiveAndClose() async {
    let entered = expectation(description: "setter running")
    let release = DispatchSemaphore(value: 0)
    let record = Recorder()
    let first = Activity(foreground: true, charging: false)
    let binding = Binding(
      apply: { value in
        try record.apply(value)
        if value == first { entered.fulfill(); release.wait() }
      }, closeOwner: { try record.close() })
    binding.publish(first)
    defer { release.signal() }
    await fulfillment(of: [entered], timeout: 2)
    binding.publish(Activity(foreground: true, charging: true))
    binding.invalidate()
    let cleanup = Task { await binding.retire(close: true) }
    XCTAssertEqual(record.closeCount, 0)
    release.signal()
    let result = await cleanup.value
    XCTAssertTrue(result.closed)
    XCTAssertEqual(record.values, [first, .inactive])
    XCTAssertEqual(record.closeCount, 1)
  }
}
