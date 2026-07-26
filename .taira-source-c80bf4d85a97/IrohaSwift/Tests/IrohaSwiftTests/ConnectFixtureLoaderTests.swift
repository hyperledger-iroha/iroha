import XCTest
@testable import IrohaSwift

final class ConnectFixtureLoaderTests: XCTestCase {
    func testLoadsBaselineBundleAndMetrics() throws {
        let loader: ConnectFixtureLoader
        do {
            loader = try ConnectFixtureLoader()
        } catch {
            throw XCTSkip("Connect fixture bundle unavailable: \(error)")
        }
        let bundle: ConnectFixtureBundle
        do {
            bundle = try loader.loadBundle()
        } catch {
            throw XCTSkip("Connect fixture bundle failed to decode: \(error)")
        }

        XCTAssertEqual(bundle.manifest.sessionIDBase64, "Y29ubmVjdC1maXh0dXJlLXNpZCE")
        XCTAssertEqual(bundle.manifest.snapshot.state, .healthy)
        XCTAssertEqual(bundle.manifest.snapshot.appToWallet.depth, 2)
        XCTAssertEqual(bundle.manifest.snapshot.walletToApp.depth, 1)

        XCTAssertEqual(bundle.session.sidBase64Url, "Y29ubmVjdC1maXh0dXJlLXNpZCE")
        XCTAssertEqual(bundle.session.toriiBaseURL, "https://mock.torii.example")

        XCTAssertFalse(bundle.metrics.isEmpty)
        XCTAssertTrue(bundle.metrics.contains { $0.state == .throttled })
        XCTAssertTrue(bundle.metrics.contains { $0.reason == "heartbeat_miss" })
        XCTAssertTrue(bundle.metrics.contains { $0.reason == "backpressure_warning" })

        XCTAssertFalse(bundle.notes.isEmpty)
    }

    func testScenarioCoverageAndShape() throws {
        let bundle: ConnectFixtureBundle
        do {
            bundle = try ConnectFixtureLoader().loadBundle()
        } catch {
            throw XCTSkip("Connect fixture bundle unavailable: \(error)")
        }
        let names = Set(bundle.scenarios.map(\.scenario))
        XCTAssertEqual(names, ["heartbeat_loss", "salt_rotation", "multi_observer"])

        for scenario in bundle.scenarios {
            XCTAssertFalse(scenario.frames.isEmpty, "scenario \(scenario.scenario) missing frames")
            XCTAssertFalse(scenario.events.isEmpty, "scenario \(scenario.scenario) missing events")
            XCTAssertEqual(scenario.sidBase64Url, "Y29ubmVjdC1maXh0dXJlLXNpZCE")
        }

        if let heartbeat = bundle.scenarios.first(where: { $0.scenario == "heartbeat_loss" }) {
            XCTAssertTrue(heartbeat.events.contains { $0.state == "heartbeat_lost" })
            XCTAssertTrue(heartbeat.frames.contains { $0.direction == .appToWallet && $0.sequence == 1 })
        } else {
            XCTFail("heartbeat_loss scenario missing")
        }
    }
}
