extension ToriiClient {
    @discardableResult
    public func listIdentifierPolicies(completion: @escaping (Result<ToriiIdentifierPolicyListResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listIdentifierPolicies() }
    }

    @discardableResult
    public func listRamLfeProgramPolicies(completion: @escaping (Result<ToriiRamLfeProgramPolicyListResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listRamLfeProgramPolicies() }
    }

    @discardableResult
    public func resolveIdentifier(_ requestBody: ToriiIdentifierLookupRequest,
                                  canonicalAuth: ToriiCanonicalRequestAuth,
                                  completion: @escaping (Result<ToriiIdentifierResolutionReceipt?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.resolveIdentifier(requestBody, canonicalAuth: canonicalAuth)
        }
    }

    @discardableResult
    public func resolveIdentifier(policyId: String,
                                  encryptedInputHex: String,
                                  outputOpening: ToriiRamLfeOutputOpening,
                                  canonicalAuth: ToriiCanonicalRequestAuth,
                                  completion: @escaping (Result<ToriiIdentifierResolutionReceipt?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.resolveIdentifier(
                policyId: policyId,
                encryptedInputHex: encryptedInputHex,
                outputOpening: outputOpening,
                canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func getIdentifierClaimByReceiptHash(_ receiptHash: String,
                                                completion: @escaping (Result<ToriiIdentifierClaimRecord?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getIdentifierClaimByReceiptHash(receiptHash) }
    }

    @discardableResult
    public func issueIdentifierClaimReceipt(accountId: String,
                                            requestBody: ToriiIdentifierLookupRequest,
                                            canonicalAuth: ToriiCanonicalRequestAuth,
                                            completion: @escaping (Result<ToriiIdentifierResolutionReceipt?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.issueIdentifierClaimReceipt(
                accountId: accountId,
                requestBody: requestBody,
                canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func issueIdentifierClaimReceipt(accountId: String,
                                            policyId: String,
                                            encryptedInputHex: String,
                                            outputOpening: ToriiRamLfeOutputOpening,
                                            canonicalAuth: ToriiCanonicalRequestAuth,
                                            completion: @escaping (Result<ToriiIdentifierResolutionReceipt?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.issueIdentifierClaimReceipt(
                accountId: accountId,
                policyId: policyId,
                encryptedInputHex: encryptedInputHex,
                outputOpening: outputOpening,
                canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func executeRamLfeProgram(programId: String,
                                     requestBody: ToriiRamLfeExecuteRequest,
                                     canonicalAuth: ToriiCanonicalRequestAuth,
                                     completion: @escaping (Result<ToriiRamLfeExecuteResponse?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.executeRamLfeProgram(
                programId: programId,
                requestBody: requestBody,
                canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func executeRamLfeProgram(programId: String,
                                     encryptedInputHex: String,
                                     canonicalAuth: ToriiCanonicalRequestAuth,
                                     completion: @escaping (Result<ToriiRamLfeExecuteResponse?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.executeRamLfeProgram(
                programId: programId,
                encryptedInputHex: encryptedInputHex,
                canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func verifyRamLfeReceipt(_ requestBody: ToriiRamLfeReceiptVerifyRequest,
                                    canonicalAuth: ToriiCanonicalRequestAuth,
                                    completion: @escaping (Result<ToriiRamLfeReceiptVerifyResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.verifyRamLfeReceipt(requestBody, canonicalAuth: canonicalAuth)
        }
    }

    @discardableResult
    public func verifyRamLfeReceipt(receipt: ToriiRamLfeExecutionReceipt,
                                    outputHex: String? = nil,
                                    canonicalAuth: ToriiCanonicalRequestAuth,
                                    completion: @escaping (Result<ToriiRamLfeReceiptVerifyResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.verifyRamLfeReceipt(
                receipt: receipt,
                outputHex: outputHex,
                canonicalAuth: canonicalAuth
            )
        }
    }
}
