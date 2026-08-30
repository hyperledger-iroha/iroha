//! Completion-handler facades for authenticated runtime and governance routes.

import Foundation

extension ToriiClient {
    @discardableResult
    public func getOfflineCapability(
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiOfflineStatus, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getOfflineCapability(canonicalAuth: canonicalAuth)
        }
    }

    @discardableResult
    public func getStatusSnapshot(
        completion: @escaping (Result<ToriiStatusSnapshot, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.getStatusSnapshot() }
    }

    @discardableResult
    public func getMetrics(
        asText: Bool = false,
        completion: @escaping (Result<ToriiMetricsResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.getMetrics(asText: asText) }
    }

    @discardableResult
    public func getNodeCapabilities(
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiNodeCapabilities, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getNodeCapabilities(canonicalAuth: canonicalAuth)
        }
    }

    @discardableResult
    public func getRuntimeMetrics(
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiRuntimeMetrics, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getRuntimeMetrics(canonicalAuth: canonicalAuth)
        }
    }

    @discardableResult
    public func getRuntimeAbiActive(
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiRuntimeAbiActive, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getRuntimeAbiActive(canonicalAuth: canonicalAuth)
        }
    }

    @discardableResult
    public func submitGovernanceDeployContractProposal(
        _ requestBody: ToriiGovernanceDeployContractProposalRequest,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiGovernanceProposalResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.submitGovernanceDeployContractProposal(
                requestBody, canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func enactGovernanceProposal(
        _ requestBody: ToriiGovernanceEnactRequest,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiGovernanceEnactResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.enactGovernanceProposal(
                requestBody, canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func getGovernanceProposal(
        idHex: String,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiGovernanceProposalGetResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getGovernanceProposal(
                idHex: idHex, canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func getGovernanceLocks(
        referendumId: String,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiGovernanceLocksResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getGovernanceLocks(
                referendumId: referendumId, canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func getGovernanceReferendum(
        id: String,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiGovernanceReferendumResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getGovernanceReferendum(
                id: id, canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func getGovernanceTally(
        id: String,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiGovernanceTallyResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getGovernanceTally(id: id, canonicalAuth: canonicalAuth)
        }
    }

    @discardableResult
    public func getGovernanceUnlockStats(
        height: UInt64? = nil,
        referendumId: String? = nil,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiGovernanceUnlockStatsResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getGovernanceUnlockStats(
                height: height,
                referendumId: referendumId,
                canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func getZkAssetMerklePaths(
        asset: String,
        commitments: [Data],
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<[ZkAssetMerklePath], Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getZkAssetMerklePaths(
                asset: asset, commitments: commitments, canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func getMerklePathForCommitment(
        asset: String,
        commitment: Data,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ZkAssetMerklePath, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getMerklePathForCommitment(
                asset: asset, commitment: commitment, canonicalAuth: canonicalAuth
            )
        }
    }

    @discardableResult
    public func getRuntimeAbiHash(
        completion: @escaping (Result<ToriiRuntimeAbiHash, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.getRuntimeAbiHash() }
    }

    @discardableResult
    public func listRuntimeUpgrades(
        completion: @escaping (Result<[ToriiRuntimeUpgradeListItem], Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.listRuntimeUpgrades() }
    }

    @discardableResult
    public func proposeRuntimeUpgrade(
        manifest: ToriiRuntimeUpgradeManifest,
        completion: @escaping (Result<ToriiRuntimeUpgradeActionResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.proposeRuntimeUpgrade(manifest: manifest) }
    }

    @discardableResult
    public func activateRuntimeUpgrade(
        idHex: String,
        completion: @escaping (Result<ToriiRuntimeUpgradeActionResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.activateRuntimeUpgrade(idHex: idHex) }
    }

    @discardableResult
    public func cancelRuntimeUpgrade(
        idHex: String,
        completion: @escaping (Result<ToriiRuntimeUpgradeActionResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.cancelRuntimeUpgrade(idHex: idHex) }
    }

    @discardableResult
    public func getVerifyingKey(
        backend: String,
        name: String,
        completion: @escaping (Result<ToriiVerifyingKeyDetail, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.getVerifyingKey(backend: backend, name: name) }
    }

    @discardableResult
    public func listVerifyingKeys(
        query: ToriiVerifyingKeyListQuery? = nil,
        completion: @escaping (Result<[ToriiVerifyingKeyListItem], Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.listVerifyingKeys(query: query) }
    }

    @discardableResult
    public func registerVerifyingKey(
        _ requestBody: ToriiVerifyingKeyRegisterRequest,
        completion: @escaping (Result<ToriiVerifyingKeyTransactionDraft, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.registerVerifyingKey(requestBody) }
    }

    @discardableResult
    public func updateVerifyingKey(
        _ requestBody: ToriiVerifyingKeyUpdateRequest,
        completion: @escaping (Result<ToriiVerifyingKeyTransactionDraft, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.updateVerifyingKey(requestBody) }
    }

    @discardableResult
    public func listProverReports(
        filter: ToriiProverReportsFilter? = nil,
        completion: @escaping (Result<[ToriiProverReport], Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.listProverReports(filter: filter) }
    }

    @discardableResult
    public func getProverReport(
        id: String,
        completion: @escaping (Result<ToriiProverReport, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.getProverReport(id: id) }
    }

    @discardableResult
    public func deleteProverReport(
        id: String,
        completion: @escaping (Result<Void, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.deleteProverReport(id: id) }
    }

    @discardableResult
    public func countProverReports(
        filter: ToriiProverReportsFilter? = nil,
        completion: @escaping (Result<UInt64, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.countProverReports(filter: filter) }
    }

    @discardableResult
    public func fetchContractManifest(
        codeHashHex: String,
        completion: @escaping (Result<ToriiContractManifestRecord, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.fetchContractManifest(codeHashHex: codeHashHex) }
    }

    @discardableResult
    public func callContract(
        _ requestBody: ToriiContractCallRequest,
        completion: @escaping (Result<ToriiContractCallResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.callContract(requestBody) }
    }

    @discardableResult
    public func resolveContractAlias(
        _ contractAlias: String,
        canonicalAuth: ToriiCanonicalRequestAuth,
        completion: @escaping (Result<ToriiContractAliasResolution, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) {
            try await self.resolveContractAlias(contractAlias, canonicalAuth: canonicalAuth)
        }
    }

    @discardableResult
    public func queryContractState(
        _ query: ToriiContractStateQuery,
        completion: @escaping (Result<ToriiContractStateResponse, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.queryContractState(query) }
    }

    @discardableResult
    public func prepareDetachedContractCall(
        _ requestBody: ToriiContractCallRequest,
        completion: @escaping (Result<ToriiContractCallDraft, Swift.Error>) -> Void
    ) -> Task<Void, Never> {
        runTask(completion) { try await self.prepareDetachedContractCall(requestBody) }
    }
}
