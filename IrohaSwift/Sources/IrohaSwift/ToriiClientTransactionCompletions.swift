extension ToriiClient {
    @discardableResult
    public func getTransactions(accountId: String,
                                limit: Int = 50,
                                offset: Int = 0,
                                assetDefinitionId: String? = nil,
                                completion: @escaping (Result<ToriiTxEnvelope, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getTransactions(
                accountId: accountId,
                limit: limit,
                offset: offset,
                assetDefinitionId: assetDefinitionId
            )
        }
    }
}
