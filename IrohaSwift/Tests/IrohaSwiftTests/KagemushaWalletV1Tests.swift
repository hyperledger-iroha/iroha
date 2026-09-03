import XCTest

@testable import IrohaSwift

final class KagemushaWalletV1Tests: XCTestCase {
  func testStagingDispositionHasOnlyDurableOutcomes() {
    XCTAssertEqual(KagemushaHardwareStageDispositionV1.staged, .staged)
    XCTAssertEqual(KagemushaHardwareStageDispositionV1.exactDuplicate, .exactDuplicate)
  }

  func testMintStageResultRequiresAndRetainsAnExactCreditID() throws {
    for disposition in [KagemushaHardwareStageDispositionV1.staged, .exactDuplicate] {
      var creditID = digest(0xc1)
      let staged = try KagemushaHardwareMintStageV1(
        disposition: disposition, creditID: creditID)
      creditID[0] ^= 1
      XCTAssertEqual(staged.disposition, disposition)
      XCTAssertEqual(staged.creditID, digest(0xc1))
      for invalidID in [
        Data(), Data(repeating: 1, count: 31),
        Data(repeating: 1, count: 33), Data(repeating: 0, count: 32),
      ] {
        XCTAssertThrowsError(
          try KagemushaHardwareMintStageV1(disposition: disposition, creditID: invalidID))
      }
    }
  }

  func testProviderSurfaceIsTheTicketedHardwareAuthority() {
    func requireProvider(_ provider: any KagemushaHardwareProviderV1) {
      _ = provider
    }
    _ = requireProvider

    XCTAssertEqual(
      KagemushaDeviceLifecycleOperationV1.allCases.map(\.rawValue),
      Array(1...24))
    XCTAssertEqual(
      KagemushaDeviceLifecycleOperationV1.foldReceiveBatch.rawValue, 22)
    XCTAssertEqual(
      KagemushaDeviceLifecycleOperationV1.rotateHardwareEpoch.rawValue, 24)
  }

  func testSuiteUpgradeAndRotationRemainDistinctRelations() {
    XCTAssertEqual(KagemushaOperationKindV1.suiteUpgrade.rawValue, 5)
    XCTAssertEqual(KagemushaOperationKindV1.rotate.rawValue, 6)
    XCTAssertNotEqual(
      KagemushaOperationKindV1.suiteUpgrade,
      KagemushaOperationKindV1.rotate)
  }

  func testPaymentAndRedemptionInstallAuthoritativeSuccessors() throws {
    let fixture = try peerFixture()
    let qualification = try makeQualification(for: fixture.request)
    let state0 = try makeState(
      for: fixture.request, qualification: qualification, sequence: 0,
      commitmentTag: 0xd0)
    let state1 = try makeState(
      for: fixture.request, qualification: qualification, sequence: 1,
      commitmentTag: 0xd1)
    let state2 = try makeState(
      for: fixture.request, qualification: qualification, sequence: 2,
      commitmentTag: 0xd2)
    let voucher = try redemptionVoucher(for: fixture.request)
    let provider = TerminalProvider(
      qualification: qualification,
      initialState: try KagemushaNoritoV1.encodeAggregateStateShape(state0),
      payment: fixture.paymentRaw,
      paymentState: try KagemushaNoritoV1.encodeAggregateStateShape(state1),
      redemption: try KagemushaNoritoV1.encodeRedemptionVoucherShape(voucher),
      redemptionState: try KagemushaNoritoV1.encodeAggregateStateShape(state2))
    let wallet = try KagemushaWalletV1.open(provider: provider)

    let payment = try wallet.commitPayment(
      request: fixture.request, intent: fixture.intent, ticket: fixture.ticket)
    XCTAssertEqual(payment, fixture.payment)
    XCTAssertEqual(wallet.aggregateState(), state1)
    XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1(1))

    let redeemed = try wallet.commitRedemption(
      amount: voucher.statement.amount, beneficiary: voucher.statement.beneficiary)
    XCTAssertEqual(redeemed, voucher)
    XCTAssertEqual(wallet.aggregateState(), state2)
    XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1(2))
  }

  func testNonadvancingRevisionPreservesCacheAndRecoveryRejectsRollback() throws {
    let fixture = try peerFixture()
    let qualification = try makeQualification(for: fixture.request)
    let state0 = try makeState(
      for: fixture.request, qualification: qualification, sequence: 0,
      commitmentTag: 0xe0)
    let state1 = try makeState(
      for: fixture.request, qualification: qualification, sequence: 1,
      commitmentTag: 0xe1)
    let state0Bytes = try KagemushaNoritoV1.encodeAggregateStateShape(state0)
    let state1Bytes = try KagemushaNoritoV1.encodeAggregateStateShape(state1)
    let voucher = try redemptionVoucher(for: fixture.request)
    let provider = TerminalProvider(
      qualification: qualification,
      initialState: state0Bytes,
      payment: fixture.paymentRaw,
      paymentState: state1Bytes,
      redemption: try KagemushaNoritoV1.encodeRedemptionVoucherShape(voucher),
      redemptionState: state1Bytes)
    provider.nextPaymentRevisionIncrement = 0
    let wallet = try KagemushaWalletV1.open(provider: provider)

    XCTAssertThrowsError(
      try wallet.commitPayment(
        request: fixture.request, intent: fixture.intent, ticket: fixture.ticket))
    XCTAssertEqual(wallet.aggregateState(), state0)
    XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1.zero)

    provider.installForRecovery(state: state1Bytes, revision: KagemushaUInt128V1(2))
    let recovered = try wallet.recover()
    XCTAssertEqual(recovered.journalRevision, KagemushaUInt128V1(2))
    XCTAssertEqual(wallet.aggregateState(), state1)

    provider.installForRecovery(state: state0Bytes, revision: .zero)
    XCTAssertThrowsError(try wallet.recover())
    XCTAssertEqual(wallet.aggregateState(), state1)
    XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1(2))
  }

  func testTerminalSuccessorsAllowRequiredFoldsAndIndependentJournalProgress() throws {
    let fixture = try peerFixture()
    let qualification = try makeQualification(for: fixture.request)
    let state0 = try makeState(
      for: fixture.request, qualification: qualification, sequence: 0, commitmentTag: 0xd0)
    let paymentState = try makeState(
      for: fixture.request, qualification: qualification, sequence: 3, commitmentTag: 0xd3)
    let redemptionState = try makeState(
      for: fixture.request, qualification: qualification, sequence: 6, commitmentTag: 0xd6)
    let voucher = try redemptionVoucher(for: fixture.request)
    let provider = TerminalProvider(
      qualification: qualification,
      initialState: try KagemushaNoritoV1.encodeAggregateStateShape(state0),
      payment: fixture.paymentRaw,
      paymentState: try KagemushaNoritoV1.encodeAggregateStateShape(paymentState),
      redemption: try KagemushaNoritoV1.encodeRedemptionVoucherShape(voucher),
      redemptionState: try KagemushaNoritoV1.encodeAggregateStateShape(redemptionState))
    provider.nextPaymentRevisionIncrement = 5
    provider.nextRedemptionRevisionIncrement = 4
    let wallet = try KagemushaWalletV1.open(provider: provider)

    _ = try wallet.commitPayment(
      request: fixture.request, intent: fixture.intent, ticket: fixture.ticket)
    XCTAssertEqual(wallet.aggregateState(), paymentState)
    XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1(5))
    _ = try wallet.commitRedemption(
      amount: voucher.statement.amount, beneficiary: voucher.statement.beneficiary)
    XCTAssertEqual(wallet.aggregateState(), redemptionState)
    XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1(9))
  }

  func testRotationAtSaturatedCountersPreservesPendingInboxWithoutFolding() throws {
    let fixture = try peerFixture()
    let qualification = try makeQualification(for: fixture.request)
    let maximum = try KagemushaUInt128V1(littleEndianBytes: Data(repeating: 0xff, count: 16))
    let previous = try makeState(
      for: fixture.request, qualification: qualification, sequence: 0,
      commitmentTag: 0xe0, sequence128: maximum)
    let rotatedQualification = try replacingQualification(
      qualification, epochID: digest(0xe2), keyReference: digest(0xe3),
      epochGeneration: qualification.credential.hardwareEpochGeneration + 1)
    let rotated = try makeState(
      for: fixture.request, qualification: rotatedQualification, sequence: 0,
      commitmentTag: 0xe4)
    let previousBytes = try KagemushaNoritoV1.encodeAggregateStateShape(previous)
    let rotatedBytes = try KagemushaNoritoV1.encodeAggregateStateShape(rotated)
    let provider = TerminalProvider(
      qualification: qualification, initialState: previousBytes,
      payment: fixture.paymentRaw, paymentState: previousBytes,
      redemption: fixture.paymentRaw, redemptionState: previousBytes)
    provider.installForRecovery(state: previousBytes, revision: maximum)
    provider.pendingCredits = KagemushaUInt128V1(17)
    provider.rotationState = rotatedBytes
    provider.rotationQualification = rotatedQualification
    let wallet = try KagemushaWalletV1.open(provider: provider)

    XCTAssertEqual(try wallet.rotateHardwareEpoch(), rotated)
    XCTAssertEqual(wallet.journalRevision(), .zero)
    XCTAssertEqual(wallet.qualification(), rotatedQualification)
    XCTAssertEqual(provider.foldCallCount, 0)
    XCTAssertEqual(provider.pendingCredits, KagemushaUInt128V1(17))

    let invalidProvider = TerminalProvider(
      qualification: qualification, initialState: previousBytes,
      payment: fixture.paymentRaw, paymentState: previousBytes,
      redemption: fixture.paymentRaw, redemptionState: previousBytes)
    invalidProvider.installForRecovery(state: previousBytes, revision: maximum)
    invalidProvider.rotationState = rotatedBytes
    invalidProvider.rotationQualification = rotatedQualification
    invalidProvider.rotationRevision = KagemushaUInt128V1(1)
    let invalidWallet = try KagemushaWalletV1.open(provider: invalidProvider)
    XCTAssertThrowsError(try invalidWallet.rotateHardwareEpoch())
    XCTAssertEqual(invalidWallet.aggregateState(), previous)
    XCTAssertEqual(invalidWallet.journalRevision(), maximum)
    XCTAssertEqual(invalidWallet.qualification(), qualification)
  }

  func testExhaustedEpochGenerationRejectsBeforeCallingProvider() throws {
    let fixture = try peerFixture()
    let qualification = try replacingQualification(
      makeQualification(for: fixture.request), epochGeneration: UInt64.max)
    let state = try makeState(
      for: fixture.request, qualification: qualification, sequence: 0,
      commitmentTag: 0xe5)
    let stateBytes = try KagemushaNoritoV1.encodeAggregateStateShape(state)
    let provider = TerminalProvider(
      qualification: qualification, initialState: stateBytes,
      payment: fixture.paymentRaw, paymentState: stateBytes,
      redemption: fixture.paymentRaw, redemptionState: stateBytes)
    let wallet = try KagemushaWalletV1.open(provider: provider)

    XCTAssertThrowsError(try wallet.rotateHardwareEpoch()) { error in
      XCTAssertEqual(
        error as? KagemushaWalletErrorV1,
        .invalidHardwareResult("hardware epoch generation exhausted"))
    }
    XCTAssertEqual(provider.rotationCallCount, 0)
    XCTAssertEqual(provider.foldCallCount, 0)
    XCTAssertEqual(wallet.aggregateState(), state)
    XCTAssertEqual(wallet.qualification(), qualification)
    XCTAssertEqual(wallet.journalRevision(), .zero)
  }

  func testRecoveryAcceptsLostRotationResponseButRejectsStaleOrReusedEpochs() throws {
    let fixture = try peerFixture()
    let qualification = try replacingQualification(
      makeQualification(for: fixture.request), epochGeneration: 7)
    let maximum = try KagemushaUInt128V1(littleEndianBytes: Data(repeating: 0xff, count: 16))
    let previous = try makeState(
      for: fixture.request, qualification: qualification, sequence: 0,
      commitmentTag: 0xc0, sequence128: maximum)
    let rotatedQualification = try replacingQualification(
      qualification, epochID: digest(0xc1), keyReference: digest(0xc2), epochGeneration: 8)
    let rotated = try makeState(
      for: fixture.request, qualification: rotatedQualification, sequence: 0, commitmentTag: 0xc3)
    let previousBytes = try KagemushaNoritoV1.encodeAggregateStateShape(previous)
    let rotatedBytes = try KagemushaNoritoV1.encodeAggregateStateShape(rotated)
    let provider = TerminalProvider(
      qualification: qualification, initialState: previousBytes,
      payment: fixture.paymentRaw, paymentState: previousBytes,
      redemption: fixture.paymentRaw, redemptionState: previousBytes)
    provider.installForRecovery(state: previousBytes, revision: maximum)
    provider.pendingCredits = KagemushaUInt128V1(17)
    provider.rotationQualification = rotatedQualification
    provider.rotationState = rotatedBytes
    provider.failNextRotationAfterCommit = true
    let wallet = try KagemushaWalletV1.open(provider: provider)
    XCTAssertThrowsError(try wallet.rotateHardwareEpoch())
    XCTAssertEqual(wallet.aggregateState(), previous)
    XCTAssertEqual(wallet.journalRevision(), maximum)
    XCTAssertEqual(wallet.qualification(), qualification)

    let recovered = try wallet.recover()
    XCTAssertEqual(recovered.aggregateState, rotatedBytes)
    XCTAssertEqual(recovered.journalRevision, .zero)
    XCTAssertEqual(recovered.pendingCreditCount, KagemushaUInt128V1(17))
    XCTAssertEqual(wallet.aggregateState(), rotated)
    XCTAssertEqual(wallet.qualification(), rotatedQualification)

    let sameGeneration = try replacingQualification(rotatedQualification, epochID: digest(0xc4))
    let reusedEpochID = try replacingQualification(rotatedQualification, epochGeneration: 9)
    let differentLane = try replacingQualification(
      rotatedQualification, epochID: digest(0xc5), epochGeneration: 9, laneCommitment: digest(0xc6))
    for candidateQualification in [qualification, sameGeneration, reusedEpochID, differentLane] {
      let candidate = try makeState(
        for: fixture.request, qualification: candidateQualification, sequence: 0,
        commitmentTag: 0xc7)
      provider.installForRecovery(
        state: try KagemushaNoritoV1.encodeAggregateStateShape(candidate), revision: maximum,
        qualification: candidateQualification)
      XCTAssertThrowsError(try wallet.recover())
      XCTAssertEqual(wallet.aggregateState(), rotated)
      XCTAssertEqual(wallet.journalRevision(), .zero)
      XCTAssertEqual(wallet.qualification(), rotatedQualification)
    }
    let forwardQualification = try replacingQualification(
      rotatedQualification, epochID: digest(0xc8), epochGeneration: 9)
    let changedAssetScope = try makeState(
      for: fixture.request, qualification: forwardQualification, sequence: 0,
      commitmentTag: 0xc9, scale: fixture.request.scale + 1)
    provider.installForRecovery(
      state: try KagemushaNoritoV1.encodeAggregateStateShape(changedAssetScope), revision: .zero,
      qualification: forwardQualification)
    XCTAssertThrowsError(try wallet.recover())
    XCTAssertEqual(wallet.aggregateState(), rotated)
    XCTAssertEqual(wallet.journalRevision(), .zero)
    XCTAssertEqual(wallet.qualification(), rotatedQualification)
  }

  func testOpenAndRecoverPreflightQualificationThenRefreshAfterNativeRecovery() throws {
    let fixture = try peerFixture()
    let qualification = try replacingQualification(
      makeQualification(for: fixture.request), epochGeneration: 7)
    let state0 = try makeState(
      for: fixture.request, qualification: qualification, sequence: 5, commitmentTag: 0xb0)
    let state0Bytes = try KagemushaNoritoV1.encodeAggregateStateShape(state0)
    let provider = TerminalProvider(
      qualification: qualification, initialState: state0Bytes,
      payment: fixture.paymentRaw, paymentState: state0Bytes,
      redemption: fixture.paymentRaw, redemptionState: state0Bytes)
    let qualification1 = try replacingQualification(
      qualification, epochID: digest(0xb1), keyReference: digest(0xb2), epochGeneration: 8)
    let state1 = try makeState(
      for: fixture.request, qualification: qualification1, sequence: 0, commitmentTag: 0xb3)
    provider.pendingRecoveryState = try KagemushaNoritoV1.encodeAggregateStateShape(state1)
    provider.pendingRecoveryQualification = qualification1
    provider.failQualification = true
    XCTAssertThrowsError(try KagemushaWalletV1.open(provider: provider))
    XCTAssertEqual(provider.recoveryCallCount, 0)
    provider.failQualification = false
    let wallet = try KagemushaWalletV1.open(provider: provider)
    XCTAssertEqual(wallet.aggregateState(), state1)
    XCTAssertEqual(wallet.qualification(), qualification1)
    XCTAssertEqual(provider.recoveryCallCount, 1)

    let qualification2 = try replacingQualification(
      qualification1, epochID: digest(0xb4), keyReference: digest(0xb5), epochGeneration: 9)
    let state2 = try makeState(
      for: fixture.request, qualification: qualification2, sequence: 0, commitmentTag: 0xb6)
    provider.pendingRecoveryState = try KagemushaNoritoV1.encodeAggregateStateShape(state2)
    provider.pendingRecoveryQualification = qualification2
    provider.failQualification = true
    XCTAssertThrowsError(try wallet.recover())
    XCTAssertEqual(provider.recoveryCallCount, 1)
    XCTAssertEqual(wallet.aggregateState(), state1)
    XCTAssertEqual(wallet.qualification(), qualification1)
    provider.failQualification = false
    _ = try wallet.recover()
    XCTAssertEqual(provider.recoveryCallCount, 2)
    XCTAssertEqual(wallet.aggregateState(), state2)
    XCTAssertEqual(wallet.qualification(), qualification2)
    XCTAssertEqual(wallet.journalRevision(), .zero)
  }

  func testOpenCorroboratesBootstrapUsingThePostBootstrapRevision() throws {
    // Core currently bootstraps at revision zero. An advancing revision exercises the
    // provider boundary: the authoritative post-bootstrap snapshot must win in either case.
    for revision in [KagemushaUInt128V1.zero, KagemushaUInt128V1(7)] {
      let fixture = try bootstrapFixture()
      let provider = fixture.provider
      let bytes = fixture.bytes
      provider.recoveryResultOverride = { [unowned provider] state, currentRevision in
        try KagemushaHardwareRecoveryV1(
          aggregateState: provider.bootstrapCallCount == 0 ? nil : state,
          journalRevision: currentRevision,
          pendingCreditCount: .zero, retryOutboxCount: .zero)
      }
      provider.bootstrapHandler = { [unowned provider] in
        provider.installForRecovery(state: bytes, revision: revision)
        return bytes
      }

      let wallet = try KagemushaWalletV1.open(provider: provider)
      XCTAssertEqual(wallet.aggregateState(), fixture.state)
      XCTAssertEqual(wallet.journalRevision(), revision)
      XCTAssertEqual(provider.bootstrapCallCount, 1)
      XCTAssertEqual(provider.recoveryCallCount, 2)
    }
  }

  func testOpenRefreshesQualificationAfterBootstrapPersistence() throws {
    let fixture = try bootstrapFixture()
    let provider = fixture.provider
    let qualification = try replacingQualification(
      fixture.qualification, epochID: digest(0xa1), keyReference: digest(0xa2),
      epochGeneration: fixture.qualification.credential.hardwareEpochGeneration + 1)
    let state = try makeState(
      for: fixture.request, qualification: qualification, sequence: 0, commitmentTag: 0xa3)
    let bytes = try KagemushaNoritoV1.encodeAggregateStateShape(state)
    provider.recoveryResultOverride = { [unowned provider] currentState, revision in
      try KagemushaHardwareRecoveryV1(
        aggregateState: provider.bootstrapCallCount == 0 ? nil : currentState,
        journalRevision: revision, pendingCreditCount: .zero, retryOutboxCount: .zero)
    }
    provider.bootstrapHandler = { [unowned provider] in
      provider.installForRecovery(state: bytes, revision: .zero, qualification: qualification)
      return bytes
    }

    let wallet = try KagemushaWalletV1.open(provider: provider)
    XCTAssertEqual(wallet.aggregateState(), state)
    XCTAssertEqual(wallet.qualification(), qualification)
    XCTAssertEqual(provider.bootstrapCallCount, 1)
    XCTAssertEqual(provider.recoveryCallCount, 2)
  }

  func testOpenRequiresPostRecoveryQualificationBeforeBootstrap() throws {
    let provider = try bootstrapFixture().provider
    provider.recoveryResultOverride = { [unowned provider] _, revision in
      provider.failQualification = true
      return try KagemushaHardwareRecoveryV1(
        aggregateState: nil, journalRevision: revision,
        pendingCreditCount: .zero, retryOutboxCount: .zero)
    }

    XCTAssertThrowsError(try KagemushaWalletV1.open(provider: provider))
    XCTAssertEqual(provider.recoveryCallCount, 1)
    XCTAssertEqual(provider.bootstrapCallCount, 0)
  }

  func testOpenRejectsMissingOrDifferentPersistedBootstrapState() throws {
    for missing in [true, false] {
      let fixture = try bootstrapFixture()
      let provider = fixture.provider
      let different = try makeState(
        for: fixture.request, qualification: fixture.qualification,
        sequence: 0, commitmentTag: 0xa4)
      let differentBytes = try KagemushaNoritoV1.encodeAggregateStateShape(different)
      provider.recoveryResultOverride = { [unowned provider] _, revision in
        try KagemushaHardwareRecoveryV1(
          aggregateState: provider.bootstrapCallCount == 0 || missing ? nil : differentBytes,
          journalRevision: revision, pendingCreditCount: .zero, retryOutboxCount: .zero)
      }

      XCTAssertThrowsError(try KagemushaWalletV1.open(provider: provider)) { error in
        XCTAssertEqual(error as? KagemushaWalletErrorV1,
          .invalidHardwareResult("bootstrap state was not durably recovered"))
      }
      XCTAssertEqual(provider.bootstrapCallCount, 1)
      XCTAssertEqual(provider.recoveryCallCount, 2)
    }
  }

  func testOpenRejectsPostBootstrapLiveJournalMismatch() throws {
    let provider = try bootstrapFixture().provider
    provider.recoveryResultOverride = { [unowned provider] state, _ in
      try KagemushaHardwareRecoveryV1(
        aggregateState: provider.bootstrapCallCount == 0 ? nil : state,
        journalRevision: KagemushaUInt128V1(1),
        pendingCreditCount: .zero, retryOutboxCount: .zero)
    }

    XCTAssertThrowsError(try KagemushaWalletV1.open(provider: provider)) { error in
      XCTAssertEqual(error as? KagemushaWalletErrorV1,
        .invalidHardwareResult("recovery returned an inconsistent journal"))
    }
    XCTAssertEqual(provider.bootstrapCallCount, 1)
    XCTAssertEqual(provider.recoveryCallCount, 2)
  }

  func testRecoverNeverBootstrapsMissingDurableStateAndPreservesCache() throws {
    let fixture = try bootstrapFixture()
    let provider = fixture.provider
    let wallet = try KagemushaWalletV1.open(provider: provider)
    provider.recoveryResultOverride = { _, revision in
      try KagemushaHardwareRecoveryV1(
        aggregateState: nil, journalRevision: revision,
        pendingCreditCount: .zero, retryOutboxCount: .zero)
    }

    XCTAssertThrowsError(try wallet.recover()) { error in
      XCTAssertEqual(error as? KagemushaWalletErrorV1,
        .invalidHardwareResult("recovery lost the durable aggregate state"))
    }
    XCTAssertEqual(provider.bootstrapCallCount, 0)
    XCTAssertEqual(wallet.aggregateState(), fixture.state)
    XCTAssertEqual(wallet.qualification(), fixture.qualification)
    XCTAssertEqual(wallet.journalRevision(), .zero)
  }

  func testRecoverRejectsLiveJournalMismatchAndPreservesCache() throws {
    let fixture = try bootstrapFixture()
    let provider = fixture.provider
    let wallet = try KagemushaWalletV1.open(provider: provider)
    provider.recoveryResultOverride = { state, _ in
      try KagemushaHardwareRecoveryV1(
        aggregateState: state, journalRevision: KagemushaUInt128V1(1),
        pendingCreditCount: .zero, retryOutboxCount: .zero)
    }

    XCTAssertThrowsError(try wallet.recover())
    XCTAssertEqual(provider.bootstrapCallCount, 0)
    XCTAssertEqual(wallet.aggregateState(), fixture.state)
    XCTAssertEqual(wallet.qualification(), fixture.qualification)
    XCTAssertEqual(wallet.journalRevision(), .zero)
  }

  func testSuiteUpgradeRefreshesQualificationForDistinctRelease() throws {
    let fixture = try peerFixture()
    let qualification = try makeQualification(for: fixture.request)
    let successorQualification = try replacingQualification(
      qualification, releaseID: digest(0xf1), suiteID: digest(0xf2))
    let previous = try makeState(
      for: fixture.request, qualification: qualification, sequence: 0, commitmentTag: 0xf3)
    let successor = try makeState(
      for: fixture.request, qualification: successorQualification, sequence: 1,
      commitmentTag: 0xf4)
    let previousBytes = try KagemushaNoritoV1.encodeAggregateStateShape(previous)
    let successorBytes = try KagemushaNoritoV1.encodeAggregateStateShape(successor)
    let provider = TerminalProvider(
      qualification: qualification, initialState: previousBytes,
      payment: fixture.paymentRaw, paymentState: previousBytes,
      redemption: fixture.paymentRaw, redemptionState: previousBytes)
    provider.suiteUpgradeState = successorBytes
    provider.suiteUpgradeQualification = successorQualification
    let wallet = try KagemushaWalletV1.open(provider: provider)
    XCTAssertEqual(try wallet.upgradeSuite(authorizationDigest: digest(0xf5)), successor)
    XCTAssertEqual(wallet.qualification(), successorQualification)
    XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1(1))

    let staleProvider = TerminalProvider(
      qualification: qualification, initialState: previousBytes,
      payment: fixture.paymentRaw, paymentState: previousBytes,
      redemption: fixture.paymentRaw, redemptionState: previousBytes)
    staleProvider.suiteUpgradeState = successorBytes
    let staleWallet = try KagemushaWalletV1.open(provider: staleProvider)
    XCTAssertThrowsError(try staleWallet.upgradeSuite(authorizationDigest: digest(0xf5)))
    XCTAssertEqual(staleWallet.aggregateState(), previous)
    XCTAssertEqual(staleWallet.qualification(), qualification)
    XCTAssertEqual(staleWallet.journalRevision(), .zero)
  }

  func testStagingLeavesMonetaryJournalUnchangedAndLostResponseRetriesExactly() throws {
    let fixture = try peerFixture()
    let qualification = try makeQualification(for: fixture.request)
    let state = try makeState(
      for: fixture.request, qualification: qualification, sequence: 0, commitmentTag: 0xd0)
    let stateBytes = try KagemushaNoritoV1.encodeAggregateStateShape(state)
    let acknowledgement = try fixtureHex(
      XCTUnwrap(loadCanonicalFixture()["acknowledgement"] as? [String: Any]))
    let (credit, authorization) = try KagemushaWireV1Tests.MintFixture.make(
      request: fixture.request)
    for loseResponse in [false, true] {
      let provider = TerminalProvider(
        qualification: qualification, initialState: stateBytes,
        payment: fixture.paymentRaw, paymentState: stateBytes,
        redemption: fixture.paymentRaw, redemptionState: stateBytes)
      provider.stagingAcknowledgement = acknowledgement
      let wallet = try KagemushaWalletV1.open(provider: provider)
      // Observe native immediately before staging, not only the earlier host snapshot.
      provider.installForRecovery(state: stateBytes, revision: KagemushaUInt128V1(7))
      provider.failNextStageAfterPersistence = loseResponse
      if loseResponse {
        XCTAssertThrowsError(
          try wallet.stageInboundPayment(
            request: fixture.request, intent: fixture.intent, ticket: fixture.ticket,
            payment: fixture.payment))
        XCTAssertEqual(wallet.journalRevision(), .zero)
      }
      let staged = try wallet.stageInboundPayment(
        request: fixture.request, intent: fixture.intent, ticket: fixture.ticket,
        payment: fixture.payment)
      XCTAssertEqual(staged.disposition, loseResponse ? .exactDuplicate : .staged)
      XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1(7))
      XCTAssertEqual(wallet.aggregateState(), state)

      provider.failNextStageAfterPersistence = loseResponse
      if loseResponse {
        XCTAssertThrowsError(
          try wallet.stageMintCredit(authorization: authorization, credit: credit))
        XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1(7))
      }
      XCTAssertEqual(
        try wallet.stageMintCredit(authorization: authorization, credit: credit),
        loseResponse ? .exactDuplicate : .staged)
      XCTAssertEqual(
        try wallet.stageMintCredit(authorization: authorization, credit: credit), .exactDuplicate)
      XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1(7))
      XCTAssertEqual(wallet.aggregateState(), state)

      provider.nextStageRevisionIncrement = 1
      XCTAssertThrowsError(
        try wallet.stageInboundPayment(
          request: fixture.request, intent: fixture.intent, ticket: fixture.ticket,
          payment: fixture.payment))
      XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1(7))
      XCTAssertThrowsError(try wallet.stageMintCredit(authorization: authorization, credit: credit))
      XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1(7))
      XCTAssertEqual(wallet.aggregateState(), state)
    }
  }

  func testMintStagingRejectsAnotherCreditIDWithoutUpdatingHostCache() throws {
    let fixture = try peerFixture()
    let qualification = try makeQualification(for: fixture.request)
    let state = try makeState(
      for: fixture.request, qualification: qualification, sequence: 0, commitmentTag: 0xd0)
    let stateBytes = try KagemushaNoritoV1.encodeAggregateStateShape(state)
    let (credit, authorization) = try KagemushaWireV1Tests.MintFixture.make(
      request: fixture.request)
    for exactDuplicate in [false, true] {
      let provider = TerminalProvider(
        qualification: qualification, initialState: stateBytes,
        payment: fixture.paymentRaw, paymentState: stateBytes,
        redemption: fixture.paymentRaw, redemptionState: stateBytes)
      let wallet = try KagemushaWalletV1.open(provider: provider)
      if exactDuplicate {
        XCTAssertEqual(
          try wallet.stageMintCredit(authorization: authorization, credit: credit), .staged)
      }
      // A rejected result must not publish even a newer native journal read to the host cache.
      provider.installForRecovery(state: stateBytes, revision: KagemushaUInt128V1(7))
      var wrongCreditID = credit.statement.lifecycle.creditID
      wrongCreditID[0] ^= 1
      provider.mintStageCreditID = wrongCreditID
      XCTAssertThrowsError(try wallet.stageMintCredit(authorization: authorization, credit: credit))
      {
        XCTAssertEqual(
          $0 as? KagemushaWalletErrorV1,
          .invalidHardwareResult("mint staging credit ID mismatch"))
      }
      XCTAssertEqual(wallet.journalRevision(), .zero)
      XCTAssertEqual(wallet.aggregateState(), state)
      XCTAssertEqual(wallet.qualification(), qualification)

      // Persistence may have completed before a bad/lost response; the exact retry remains usable.
      provider.mintStageCreditID = nil
      XCTAssertEqual(
        try wallet.stageMintCredit(authorization: authorization, credit: credit), .exactDuplicate)
      XCTAssertEqual(wallet.journalRevision(), KagemushaUInt128V1(7))
      XCTAssertEqual(wallet.aggregateState(), state)
    }
  }

  func testDrainYieldsToCoveredSendAndThrowingForegroundAndRejectsRotatedWatermark() throws {
    for action in ["payment", "throwingPayment", "rotation"] {
      let fixture = try peerFixture()
      let qualification = try makeQualification(for: fixture.request)
      let state0 = try makeState(
        for: fixture.request, qualification: qualification, sequence: 0, commitmentTag: 0xd0)
      let stateBytes = try KagemushaNoritoV1.encodeAggregateStateShape(state0)
      let provider = TerminalProvider(
        qualification: qualification, initialState: stateBytes,
        payment: fixture.paymentRaw, paymentState: stateBytes,
        redemption: fixture.paymentRaw, redemptionState: stateBytes)
      let events = WalletTestEvents()
      let firstBatchEntered = DispatchSemaphore(value: 0)
      let releaseFirstBatch = DispatchSemaphore(value: 0)
      let foregroundStarted = DispatchSemaphore(value: 0)
      let oldWatermark = KagemushaUInt128V1(1_024)
      provider.pendingCredits = oldWatermark
      provider.foldHandler = { current, call, watermark in
        XCTAssertEqual(watermark, oldWatermark)
        if call == 1 {
          firstBatchEntered.signal()
          guard releaseFirstBatch.wait(timeout: .now() + 5) == .success else {
            throw NSError(domain: "drain-test-timeout", code: 1)
          }
        }
        guard call <= 64 else { return nil }
        events.record("batch")
        let previous = try KagemushaNoritoV1.decodeAggregateStateShapeExact(current)
        return try KagemushaNoritoV1.encodeAggregateStateShape(
          self.makeState(
            for: fixture.request, qualification: qualification, sequence: 0,
            commitmentTag: UInt8(call), sequence128: previous.sequence.adding(1)))
      }
      provider.paymentStateTransform = { current in
        events.record("foreground")
        if action == "throwingPayment" {
          throw NSError(domain: "expected-payment-failure", code: 1)
        }
        let previous = try KagemushaNoritoV1.decodeAggregateStateShapeExact(current)
        return try KagemushaNoritoV1.encodeAggregateStateShape(
          self.makeState(
            for: fixture.request, qualification: qualification, sequence: 0,
            commitmentTag: 0xfe, sequence128: previous.sequence.adding(1)))
      }
      if action == "rotation" {
        let rotatedQualification = try replacingQualification(
          qualification, epochID: digest(0xe2), keyReference: digest(0xe3),
          epochGeneration: qualification.credential.hardwareEpochGeneration + 1)
        provider.rotationQualification = rotatedQualification
        provider.rotationState = try KagemushaNoritoV1.encodeAggregateStateShape(
          makeState(
            for: fixture.request, qualification: rotatedQualification, sequence: 0,
            commitmentTag: 0xe4))
        provider.onRotate = { events.record("foreground") }
      }
      let wallet = try KagemushaWalletV1.open(provider: provider)
      let drainFinished = expectation(description: "\(action) drain completed")
      let foregroundFinished = expectation(description: "\(action) foreground completed")
      DispatchQueue.global().async {
        defer { drainFinished.fulfill() }
        do {
          let batches = try wallet.drainStagedCredits()
          XCTAssertNotEqual(action, "rotation")
          XCTAssertEqual(batches, KagemushaUInt128V1(64))
        } catch {
          XCTAssertEqual(action, "rotation", "\(error)")
          XCTAssertEqual(
            error as? KagemushaWalletErrorV1,
            .invalidHardwareResult(
              "hardware epoch changed during inbox drain; start a new drain pass"))
          events.record("interrupted")
        }
      }
      XCTAssertEqual(firstBatchEntered.wait(timeout: .now() + 5), .success)
      DispatchQueue.global().async {
        defer { foregroundFinished.fulfill() }
        foregroundStarted.signal()
        do {
          if action == "rotation" {
            _ = try wallet.rotateHardwareEpoch()
          } else {
            _ = try wallet.commitPayment(
              request: fixture.request, intent: fixture.intent, ticket: fixture.ticket)
          }
          XCTAssertNotEqual(action, "throwingPayment")
        } catch {
          XCTAssertEqual(action, "throwingPayment", "\(error)")
        }
      }
      XCTAssertEqual(foregroundStarted.wait(timeout: .now() + 5), .success)
      releaseFirstBatch.signal()
      wait(for: [foregroundFinished, drainFinished], timeout: 15)
      let recorded = events.snapshot()
      let foregroundIndex = try XCTUnwrap(recorded.firstIndex(of: "foreground"))
      XCTAssertLessThan(
        foregroundIndex, 64, "Covered foreground work must not wait for the whole backlog")
      if action == "rotation" {
        XCTAssertEqual(Array(recorded.dropFirst(foregroundIndex + 1)), ["interrupted"])
        XCTAssertEqual(wallet.journalRevision(), .zero)
        XCTAssertEqual(
          wallet.aggregateState().hardwareEpochID,
          provider.rotationQualification?.credential.hardwareEpochID)
      } else {
        XCTAssertEqual(recorded.filter { $0 == "batch" }.count, 64)
      }
    }
  }

  private typealias PeerFixture = (
    request: KagemushaPaymentRequestV1,
    intent: KagemushaAcceptanceIntentV1,
    ticket: KagemushaAcceptanceTicketV1,
    payment: KagemushaPaymentV1,
    paymentRaw: Data
  )

  private func bootstrapFixture() throws -> (
    provider: TerminalProvider, request: KagemushaPaymentRequestV1,
    qualification: KagemushaHardwareQualificationV1,
    state: KagemushaAggregateStateCommitmentV1, bytes: Data
  ) {
    let fixture = try peerFixture()
    let qualification = try makeQualification(for: fixture.request)
    let state = try makeState(
      for: fixture.request, qualification: qualification, sequence: 0, commitmentTag: 0xa0)
    let bytes = try KagemushaNoritoV1.encodeAggregateStateShape(state)
    return (
      TerminalProvider(qualification: qualification, initialState: bytes,
        payment: fixture.paymentRaw, paymentState: bytes,
        redemption: fixture.paymentRaw, redemptionState: bytes),
      fixture.request, qualification, state, bytes)
  }

  private func peerFixture() throws -> PeerFixture {
    let root = try loadCanonicalFixture()
    let requestRaw = try fixtureHex(try XCTUnwrap(root["payment_request"] as? [String: Any]))
    let intentRaw = try fixtureHex(
      try XCTUnwrap(root["acceptance_intent"] as? [String: Any]))
    let ticketRaw = try fixtureHex(try XCTUnwrap(root["acceptance_ticket"] as? [String: Any]))
    let paymentRaw = try fixtureHex(try XCTUnwrap(root["payment"] as? [String: Any]))
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(requestRaw)
    let intent = try KagemushaNoritoV1.decodeAcceptanceIntentShapeExact(
      intentRaw, against: request)
    let ticket = try KagemushaNoritoV1.decodeAcceptanceTicketShapeExact(
      ticketRaw, against: request, intent: intent)
    let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
      paymentRaw, against: request, intent: intent, ticket: ticket)
    return (request, intent, ticket, payment, paymentRaw)
  }

  private func makeQualification(
    for request: KagemushaPaymentRequestV1
  ) throws -> KagemushaHardwareQualificationV1 {
    let credential = request.hardwareCredential
    let profile = try KagemushaHardwareProfileV1(
      hardwareProfileID: credential.hardwareProfileID,
      providerID: digest(0xc0),
      platformClass: .appleOEMService,
      productClassDigest: digest(0xc1),
      firmwarePolicyDigest: credential.firmwarePolicyDigest,
      enrollmentAttestationVerifierDigest: digest(0xc2),
      attestationTrustRootsDigest: digest(0xc3),
      allowedSuiteCommitment: digest(0xc4),
      policyEpoch: credential.policyEpoch,
      governanceCredentialPublicKey: credential.devicePublicKey,
      capabilityMask: KagemushaWireV1.requiredHardwareCapabilityMask,
      qualificationReportDigest: digest(0xc5),
      validFromMS: 0,
      expiresAtMS: UInt64.max)
    return try KagemushaHardwareQualificationV1(
      releaseID: request.releaseID,
      hardwarePolicyDigest: digest(0xc6),
      profile: profile,
      credential: credential)
  }

  private func makeState(
    for request: KagemushaPaymentRequestV1,
    qualification: KagemushaHardwareQualificationV1,
    sequence: UInt64,
    commitmentTag: UInt8,
    sequence128: KagemushaUInt128V1? = nil,
    scale: UInt32? = nil
  ) throws -> KagemushaAggregateStateCommitmentV1 {
    try KagemushaAggregateStateCommitmentV1(
      releaseID: qualification.releaseID,
      networkID: request.networkID,
      asset: request.asset,
      assetIncarnation: request.assetIncarnation,
      scale: scale ?? request.scale,
      liabilityPoolID: request.liabilityPoolID,
      laneID: qualification.credential.laneCommitment,
      hardwareEpochID: qualification.credential.hardwareEpochID,
      keyReference: qualification.credential.deviceKeyReference,
      hardwarePolicyID: qualification.hardwarePolicyDigest,
      sequence: sequence128 ?? KagemushaUInt128V1(sequence),
      stateCommitment: digest(commitmentTag))
  }

  private func replacingQualification(
    _ value: KagemushaHardwareQualificationV1,
    releaseID: Data? = nil,
    suiteID: Data? = nil,
    epochID: Data? = nil,
    keyReference: Data? = nil,
    epochGeneration: UInt64? = nil,
    laneCommitment: Data? = nil
  ) throws -> KagemushaHardwareQualificationV1 {
    let credential = value.credential
    return try KagemushaHardwareQualificationV1(
      releaseID: releaseID ?? value.releaseID,
      hardwarePolicyDigest: value.hardwarePolicyDigest,
      profile: value.profile,
      credential: KagemushaHardwareCredentialV1(
        credentialID: credential.credentialID, networkID: credential.networkID,
        hardwareProfileID: credential.hardwareProfileID, suiteID: suiteID ?? credential.suiteID,
        firmwarePolicyDigest: credential.firmwarePolicyDigest, policyEpoch: credential.policyEpoch,
        laneCommitment: laneCommitment ?? credential.laneCommitment,
        hardwareEpochID: epochID ?? credential.hardwareEpochID,
        hardwareEpochGeneration: epochGeneration ?? credential.hardwareEpochGeneration,
        devicePublicKey: credential.devicePublicKey,
        deviceKeyReference: keyReference ?? credential.deviceKeyReference,
        issuedAtMS: credential.issuedAtMS, expiresAtMS: credential.expiresAtMS,
        governanceSignature: credential.governanceSignature))
  }

  private func redemptionVoucher(
    for request: KagemushaPaymentRequestV1
  ) throws -> KagemushaRedemptionVoucherV1 {
    let lifecycle = try KagemushaLifecycleBindingV1(
      networkID: request.networkID,
      suiteID: request.hardwareCredential.suiteID,
      vkDigest: digest(0xa3),
      releaseID: request.releaseID,
      asset: request.asset,
      assetIncarnation: request.assetIncarnation,
      scale: request.scale,
      liabilityPoolID: request.liabilityPoolID,
      hardwareProfileID: request.hardwareCredential.hardwareProfileID,
      policyEpoch: request.hardwareCredential.policyEpoch,
      operationKind: .redeemSplit,
      requestID: Data(repeating: 0, count: 32),
      acceptanceTicketID: Data(repeating: 0, count: 32),
      creditID: Data(repeating: 0, count: 32),
      ciphertextDigest: Data(repeating: 0, count: 32))
    let evidence = KagemushaCommitEvidenceV1.trustedTime(
      try KagemushaTrustedCommitTimeV1(timeEvidenceCommitment: digest(0xa9)))
    func statement(_ redemptionID: Data) throws -> KagemushaRedemptionStatementV1 {
      try KagemushaRedemptionStatementV1(
        lifecycle: lifecycle,
        amount: KagemushaUInt128V1(12),
        beneficiary: request.recipient,
        terminalNullifier: digest(0xa2),
        redemptionCommitment: digest(0xa8),
        redemptionID: redemptionID,
        commitEvidence: evidence)
    }
    let provisional = try statement(digest(0xaa))
    let finalStatement = try statement(KagemushaNoritoV1.redemptionIDShape(provisional))
    func certificate(_ certificateID: Data) throws -> KagemushaCommitCertificateV1 {
      try KagemushaCommitCertificateV1(
        certificateID: certificateID,
        candidateEnvelopeDigest: digest(0xab),
        lifecycleBindingDigest: KagemushaNoritoV1.lifecycleBindingDigestShape(lifecycle),
        transitionNullifier: finalStatement.terminalNullifier,
        outboxReservationCommitment: digest(0xac),
        commitEvidence: evidence,
        hardwareProfileID: lifecycle.hardwareProfileID,
        policyEpoch: lifecycle.policyEpoch,
        hardwareTerminalCommitment: digest(0xad))
    }
    let provisionalCertificate = try certificate(digest(0xae))
    let commitCertificate = try certificate(
      KagemushaNoritoV1.commitCertificateIDShape(provisionalCertificate))
    let proof = try KagemushaRedemptionProofV1(
      eqProtocolDigest: digest(0xb0),
      epProtocolDigest: digest(0xb1),
      semanticDigest: KagemushaNoritoV1.redemptionStatementDigestShape(finalStatement),
      candidateEnvelopeDigest: commitCertificate.candidateEnvelopeDigest,
      commitCertificateDigest: KagemushaNoritoV1.commitCertificateDigestShape(commitCertificate),
      eqDeferredAudit: digest(0xb2),
      epDeferredAudit: digest(0xb3),
      eqProof: Data([0xb4]),
      epProof: Data([0xb5]),
      eqHistory: Data(repeating: 0xb6, count: KagemushaWireV1.historyAccumulatorBytes),
      epHistory: Data(repeating: 0xb7, count: KagemushaWireV1.historyAccumulatorBytes))
    return try KagemushaRedemptionVoucherV1(
      statement: finalStatement,
      commitCertificate: commitCertificate,
      proof: proof,
      artifactManifestDigest: digest(0xb8))
  }

  private func loadCanonicalFixture() throws -> [String: Any] {
    var current = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while current.path != "/" {
      let candidate = current.appendingPathComponent("fixtures/offline/kagemusha_v1.json")
      if FileManager.default.fileExists(atPath: candidate.path) {
        return try XCTUnwrap(
          JSONSerialization.jsonObject(with: Data(contentsOf: candidate)) as? [String: Any])
      }
      current.deleteLastPathComponent()
    }
    throw NSError(
      domain: "KagemushaWalletV1Tests", code: 1,
      userInfo: [NSLocalizedDescriptionKey: "KAGEMUSHA fixture was not found"])
  }

  private func fixtureHex(_ section: [String: Any]) throws -> Data {
    let hex = try XCTUnwrap(section["norito_hex"] as? String)
    var result = Data()
    var index = hex.startIndex
    while index != hex.endIndex {
      let next = hex.index(index, offsetBy: 2)
      result.append(try XCTUnwrap(UInt8(hex[index..<next], radix: 16)))
      index = next
    }
    return result
  }

  private func digest(_ byte: UInt8) -> Data {
    Data(repeating: byte, count: 32)
  }
}

private final class TerminalProvider: KagemushaHardwareProviderV1 {
  private var qualificationValue: KagemushaHardwareQualificationV1
  private var state: Data
  private var revision = KagemushaUInt128V1.zero
  private let payment: Data
  private let paymentState: Data
  private let redemption: Data
  private let redemptionState: Data
  var nextPaymentRevisionIncrement: UInt8 = 1
  var nextRedemptionRevisionIncrement: UInt8 = 1
  var pendingCredits = KagemushaUInt128V1.zero
  private(set) var foldCallCount = 0
  private(set) var rotationCallCount = 0
  var rotationState: Data?
  var rotationQualification: KagemushaHardwareQualificationV1?
  var rotationRevision = KagemushaUInt128V1.zero
  var suiteUpgradeState: Data?
  var suiteUpgradeQualification: KagemushaHardwareQualificationV1?
  var stagingAcknowledgement: Data?
  var nextStageRevisionIncrement: UInt8 = 0
  var failNextStageAfterPersistence = false
  var mintStageCreditID: Data?
  private var paymentStaged = false
  private var mintStaged = false
  var foldHandler: ((Data, Int, KagemushaUInt128V1) throws -> Data?)?
  var paymentStateTransform: ((Data) throws -> Data)?
  var onRotate: (() -> Void)?
  var failNextRotationAfterCommit = false
  var failQualification = false
  var pendingRecoveryState: Data?
  var pendingRecoveryQualification: KagemushaHardwareQualificationV1?
  private(set) var recoveryCallCount = 0
  var recoveryResultOverride: ((Data, KagemushaUInt128V1) throws -> KagemushaHardwareRecoveryV1)?
  var bootstrapHandler: (() throws -> Data)?
  private(set) var bootstrapCallCount = 0

  init(
    qualification: KagemushaHardwareQualificationV1,
    initialState: Data,
    payment: Data,
    paymentState: Data,
    redemption: Data,
    redemptionState: Data
  ) {
    qualificationValue = qualification
    state = initialState
    self.payment = payment
    self.paymentState = paymentState
    self.redemption = redemption
    self.redemptionState = redemptionState
  }

  func installForRecovery(
    state: Data, revision: KagemushaUInt128V1,
    qualification: KagemushaHardwareQualificationV1? = nil
  ) {
    self.state = state
    self.revision = revision
    if let qualification { qualificationValue = qualification }
  }

  func qualification() throws -> KagemushaHardwareQualificationV1 {
    if failQualification { throw unused() }
    return qualificationValue
  }

  func recover() throws -> KagemushaHardwareRecoveryV1 {
    recoveryCallCount += 1
    if let pendingRecoveryState, let pendingRecoveryQualification {
      state = pendingRecoveryState
      qualificationValue = pendingRecoveryQualification
      revision = .zero
      self.pendingRecoveryState = nil
      self.pendingRecoveryQualification = nil
    }
    if let recoveryResultOverride {
      return try recoveryResultOverride(state, revision)
    }
    return try KagemushaHardwareRecoveryV1(
      aggregateState: state,
      journalRevision: revision,
      pendingCreditCount: pendingCredits,
      retryOutboxCount: .zero)
  }

  func bootstrapState() throws -> Data {
    bootstrapCallCount += 1
    return try bootstrapHandler?() ?? state
  }

  func journalRevision() throws -> KagemushaUInt128V1 { revision }

  func createPaymentRequest(
    recipient: KagemushaAccountIDV1,
    requestMode: KagemushaPaymentRequestModeV1,
    validityWindowMS: UInt64
  ) throws -> Data { throw unused() }

  func prepareAcceptanceIntent(
    canonicalRequest: Data,
    exactAmount: KagemushaUInt128V1
  ) throws -> Data { throw unused() }

  func recoverAcceptanceIntent(intentID: Data) throws -> Data? { nil }

  func validateIntentReserveInboxAndIssueAcceptanceTicket(
    canonicalRequest: Data,
    canonicalIntent: Data
  ) throws -> Data { throw unused() }

  func recoverAcceptanceTicket(acceptanceTicketID: Data) throws -> Data? { nil }

  func prepareProveCommitPayment(
    canonicalRequest: Data,
    canonicalIntent: Data,
    canonicalTicket: Data
  ) throws -> KagemushaHardwareTerminalResultV1 {
    let successor = try paymentStateTransform?(state) ?? paymentState
    revision = try revision.adding(nextPaymentRevisionIncrement)
    state = successor
    return try KagemushaHardwareTerminalResultV1(
      canonicalEnvelope: payment, aggregateState: successor)
  }

  func recoverPayment(creditID: Data) throws -> Data? { payment }

  func verifyAndStageInboundPayment(
    canonicalRequest: Data,
    canonicalIntent: Data,
    canonicalTicket: Data,
    canonicalPayment: Data
  ) throws -> (KagemushaHardwareStageDispositionV1, Data) {
    guard let stagingAcknowledgement else { throw unused() }
    let disposition: KagemushaHardwareStageDispositionV1 = paymentStaged ? .exactDuplicate : .staged
    paymentStaged = true
    try finishStage()
    return (disposition, stagingAcknowledgement)
  }

  func releasePaymentOutbox(
    canonicalRequest: Data,
    canonicalIntent: Data,
    canonicalTicket: Data,
    canonicalPayment: Data,
    canonicalAcknowledgement: Data
  ) throws { throw unused() }

  func prepareMintAuthorization(
    operationID: Data,
    amount: KagemushaUInt128V1,
    payer: KagemushaAccountIDV1,
    recipient: KagemushaAccountIDV1
  ) throws -> Data { throw unused() }

  func verifyAuthorizationAndStageMintCredit(
    canonicalAuthorization: Data,
    canonicalMintCredit: Data
  ) throws -> KagemushaHardwareMintStageV1 {
    let authorization = try KagemushaNoritoV1.decodeMintAuthorizationShapeExact(
      canonicalAuthorization)
    let credit = try KagemushaNoritoV1.decodeMintCreditShapeExact(
      canonicalMintCredit, against: authorization)
    let disposition: KagemushaHardwareStageDispositionV1 = mintStaged ? .exactDuplicate : .staged
    mintStaged = true
    try finishStage()
    return try KagemushaHardwareMintStageV1(
      disposition: disposition, creditID: mintStageCreditID ?? credit.statement.lifecycle.creditID)
  }

  private func finishStage() throws {
    revision = try revision.adding(nextStageRevisionIncrement)
    if failNextStageAfterPersistence {
      failNextStageAfterPersistence = false
      throw unused()
    }
  }

  func pendingCreditWatermark() throws -> KagemushaUInt128V1 { pendingCredits }

  func foldReceiveBatch(upToInclusive watermark: KagemushaUInt128V1) throws -> Data? {
    foldCallCount += 1
    if let foldHandler {
      guard let next = try foldHandler(state, foldCallCount, watermark) else { return nil }
      revision = try revision.adding(1)
      state = next
      return next
    }
    guard pendingCredits.isZero else { throw unused() }
    return nil
  }

  func prepareProveCommitRedemption(
    amount: KagemushaUInt128V1,
    beneficiary: KagemushaAccountIDV1
  ) throws -> KagemushaHardwareTerminalResultV1 {
    revision = try revision.adding(nextRedemptionRevisionIncrement)
    state = redemptionState
    return try KagemushaHardwareTerminalResultV1(
      canonicalEnvelope: redemption, aggregateState: redemptionState)
  }

  func recoverRedemption(redemptionID: Data) throws -> Data? { redemption }

  func prepareProveCommitSuiteUpgrade(authorizationDigest: Data) throws -> Data {
    guard let suiteUpgradeState else { throw unused() }
    revision = try revision.adding(1)
    state = suiteUpgradeState
    if let suiteUpgradeQualification { qualificationValue = suiteUpgradeQualification }
    return state
  }

  func rotateHardwareEpoch() throws -> Data {
    rotationCallCount += 1
    onRotate?()
    guard let rotationState, let rotationQualification else { throw unused() }
    state = rotationState
    qualificationValue = rotationQualification
    revision = rotationRevision
    if failNextRotationAfterCommit {
      failNextRotationAfterCommit = false
      throw unused()
    }
    return state
  }

  private func unused() -> NSError {
    NSError(
      domain: "TerminalProvider", code: 1,
      userInfo: [NSLocalizedDescriptionKey: "unused provider method"])
  }
}

private final class WalletTestEvents: @unchecked Sendable {
  private let lock = NSLock()
  private var events: [String] = []

  func record(_ event: String) {
    lock.lock()
    defer { lock.unlock() }
    events.append(event)
  }

  func snapshot() -> [String] {
    lock.lock()
    defer { lock.unlock() }
    return events
  }
}
