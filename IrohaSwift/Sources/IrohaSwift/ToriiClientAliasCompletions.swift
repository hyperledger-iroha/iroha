extension ToriiClient {
    @discardableResult
    public func getAssets(accountId: String,
                          limit: Int = 100,
                          asset: String? = nil,
                          scope: String? = nil,
                          completion: @escaping (Result<[ToriiAssetBalance], Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getAssets(accountId: accountId, limit: limit, asset: asset, scope: scope) }
    }

    @discardableResult
    public func resolveAssetAlias(_ alias: String,
                                  completion: @escaping (Result<ToriiAssetAliasResolution?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.resolveAssetAlias(alias) }
    }

    @discardableResult
    public func resolveAccountAlias(_ alias: String,
                                    completion: @escaping (Result<ToriiAccountAliasResolution?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.resolveAccountAlias(alias) }
    }

    /// Resolve a restricted alias with canonical account/signature/timestamp/nonce headers.
    @discardableResult
    public func resolveAccountAlias(_ alias: String,
                                    canonicalAuth: ToriiCanonicalRequestAuth,
                                    completion: @escaping (Result<ToriiAccountAliasResolution?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.resolveAccountAlias(alias, canonicalAuth: canonicalAuth)
        }
    }

    /// Resolve one visible deterministic alias index without authentication.
    @discardableResult
    public func resolveAccountAliasIndex(
        _ index: UInt64,
        completion: @escaping (Result<ToriiAliasIndexResolution?, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.resolveAccountAliasIndex(index) }
    }

    /// Resolve one deterministic alias index with canonical request authentication.
    @discardableResult
    public func resolveAccountAliasIndex(
        _ index: UInt64,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiAliasIndexResolution?, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.resolveAccountAliasIndex(index, canonicalAuth: canonicalAuth)
        }
    }

    /// List visible aliases for one canonical account without authentication.
    @discardableResult
    public func aliasesByAccount(
        _ lookup: ToriiAliasesByAccountRequest,
        completion: @escaping (Result<ToriiAliasesByAccountResponse?, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.aliasesByAccount(lookup) }
    }

    /// List visible aliases for one canonical account with canonical authentication.
    @discardableResult
    public func aliasesByAccount(
        _ lookup: ToriiAliasesByAccountRequest,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiAliasesByAccountResponse?, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.aliasesByAccount(lookup, canonicalAuth: canonicalAuth)
        }
    }

    /// Request a read-only, canonical-account-signed atomic alias setup plan.
    public func planAliasSetup(
        _ setup: AliasSetupPlanRequestV1,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<AliasTransactionPlanV1, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.planAliasSetup(setup, canonicalAuth: canonicalAuth)
        }
    }

    /// Request a canonical-account-signed, read-only lease-renewal plan.
    public func planAliasLeaseRenewal(
        _ renewal: AliasLeaseRenewPlanRequestV1,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<AliasLifecycleTransactionPlanV1, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.planAliasLeaseRenewal(renewal, canonicalAuth: canonicalAuth)
        }
    }

    /// Request a canonical-account-signed, read-only auto-renew configuration plan.
    public func planAliasAutoRenew(
        _ configuration: AliasAutoRenewPlanRequestV1,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<AliasLifecycleTransactionPlanV1, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.planAliasAutoRenew(configuration, canonicalAuth: canonicalAuth)
        }
    }
}
