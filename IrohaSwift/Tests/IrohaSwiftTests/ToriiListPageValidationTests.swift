import XCTest
@testable import IrohaSwift

final class ToriiListPageValidationTests: XCTestCase {
    func testDomainAndRwaPagesRejectNegativeTotals() {
        let payload = Data(#"{"items":[],"total":-1}"#.utf8)

        XCTAssertThrowsError(try JSONDecoder().decode(ToriiDomainListPage.self, from: payload))
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiRwaListPage.self, from: payload))
    }

    func testDomainAndRwaPagesRetainZeroTotals() throws {
        let payload = Data(#"{"items":[],"total":0}"#.utf8)

        XCTAssertEqual(try JSONDecoder().decode(ToriiDomainListPage.self, from: payload).total, 0)
        XCTAssertEqual(try JSONDecoder().decode(ToriiRwaListPage.self, from: payload).total, 0)
    }

    func testDomainAndRwaPagesRejectTotalsBelowReturnedItemCount() {
        let payload = Data(#"{"items":[{"id":"item-1"}],"total":0}"#.utf8)

        XCTAssertThrowsError(try JSONDecoder().decode(ToriiDomainListPage.self, from: payload))
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiRwaListPage.self, from: payload))
    }

    func testDomainAndRwaPagesRejectExplicitNullTotal() {
        let payload = Data(#"{"items":[],"total":null}"#.utf8)

        XCTAssertThrowsError(try JSONDecoder().decode(ToriiDomainListPage.self, from: payload))
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiRwaListPage.self, from: payload))
    }

    func testDomainAndRwaPagesDefaultOnlyMissingTotal() throws {
        let payload = Data(#"{"items":[{"id":"item-1"}]}"#.utf8)

        XCTAssertEqual(try JSONDecoder().decode(ToriiDomainListPage.self, from: payload).total, 1)
        XCTAssertEqual(try JSONDecoder().decode(ToriiRwaListPage.self, from: payload).total, 1)
    }

    func testDirectListPageInitializersRejectInvalidTotals() throws {
        XCTAssertThrowsError(try ToriiDomainListPage(items: [], total: -1))
        XCTAssertThrowsError(
            try ToriiRwaListPage(items: [ToriiRwaListItem(id: "item-1")], total: 0)
        )
        XCTAssertEqual(try ToriiRwaListPage(items: [], total: 0).total, 0)
    }
}
