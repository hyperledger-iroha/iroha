import IrohaSwift
import XCTest

final class KagemushaPublicLifecycleAPITests: XCTestCase {
    func testV4LifecycleEntryPointsExposeTheirDocumentedFunctionTypes() {
        let initialize: (
            KagemushaRecursiveSpendInitLocalRequestV4,
            KagemushaRecursiveSpendInstalledArtifactSetV4
        ) throws -> KagemushaRecursiveSpendInitResultV4 =
            KagemushaRecursiveSpend.initSpendV4
        let append: (
            KagemushaRecursiveSpendAppendLocalRequestV4,
            KagemushaVerifiedRecipientPaymentRequest,
            KagemushaRecursiveSpendInstalledArtifactSetV4
        ) throws -> KagemushaRecursiveSpendSplitResultV4 =
            KagemushaRecursiveSpend.appendSpendV4
        let resumeAppend: (
            KagemushaRecursiveSpendPersistedAppendLocalRequestV4,
            KagemushaVerifiedRecipientPaymentRequest,
            KagemushaRecursiveSpendInstalledArtifactSetV4
        ) throws -> KagemushaRecursiveSpendSplitResultV4 =
            KagemushaRecursiveSpend.appendSpendV4
        let verify: (
            KagemushaRecursiveSpendVerifyLocalRequestV4,
            KagemushaRecursiveSpendInstalledArtifactSetV4
        ) throws -> KagemushaRecursiveSpendVerifyResultV4 =
            KagemushaRecursiveSpend.verifySpendV4
        let buildRedeem: (
            KagemushaRecursiveSpendRedeemLocalRequestV4,
            KagemushaRecursiveSpendInstalledArtifactSetV4
        ) throws -> KagemushaRecursiveSpendRedeemBuildResultV4 =
            KagemushaRecursiveSpend.buildRedeemV4

        _ = initialize
        _ = append
        _ = resumeAppend
        _ = verify
        _ = buildRedeem
    }
}
