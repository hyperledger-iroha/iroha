import Foundation

public let ToriiPdpCommitmentHeader = "sora-pdp-commitment"

public func decodePdpCommitmentHeader(_ headers: [String: String]) throws -> Data? {
    guard !headers.isEmpty else { return nil }
    for (key, value) in headers {
        if key.caseInsensitiveCompare(ToriiPdpCommitmentHeader) == .orderedSame {
            guard let decoded = Data(base64Encoded: value) else {
                throw ToriiClientError.invalidPayload(
                    "\(ToriiPdpCommitmentHeader) header was not valid base64"
                )
            }
            return decoded
        }
    }
    return nil
}

public func decodePdpCommitmentHeader(from response: HTTPURLResponse) throws -> Data? {
    var normalized: [String: String] = [:]
    for (key, value) in response.allHeaderFields {
        guard let keyString = key as? String else { continue }
        if let stringValue = value as? String {
            normalized[keyString] = stringValue
        } else if let dataValue = value as? Data,
                  let rendered = String(data: dataValue, encoding: .utf8)
        {
            normalized[keyString] = rendered
        }
    }
    return try decodePdpCommitmentHeader(normalized)
}

public struct ToriiAssetBalance: Decodable, Sendable {
    public let asset_id: String
    public let quantity: String
}

public enum PipelineEndpointMode: Sendable, Equatable {
    case pipeline
}

public struct ToriiTxItem: Decodable, Sendable {
    public let authority: String?
    public let timestamp_ms: UInt64?
    public let entrypoint_hash: String
    public let result_ok: Bool
}

public struct ToriiTxEnvelope: Decodable, Sendable {
    public let items: [ToriiTxItem]
    public let total: UInt64
}

public struct ToriiAccountOnboardingRequest: Encodable, Sendable {
    public let alias: String
    public let accountId: String
    public let identity: [String: String]?

    private enum CodingKeys: String, CodingKey {
        case alias
        case accountId = "account_id"
        case identity
    }

    public init(alias: String, accountId: String, identity: [String: String]? = nil) {
        self.alias = alias
        self.accountId = accountId
        self.identity = identity
    }
}

extension ToriiClient {
    static func prettyPrintedJSON(from data: Data) throws -> Data {
        let object = try JSONSerialization.jsonObject(with: data, options: [])
        return try JSONSerialization.data(withJSONObject: object,
                                          options: [.prettyPrinted, .sortedKeys])
    }

    static func persistDaRequestArtifacts(body: Data,
                                          directory: URL,
                                          fileManager: FileManager) throws {
        try fileManager.createDirectory(at: directory, withIntermediateDirectories: true, attributes: nil)
        let requestURL = directory.appendingPathComponent("da_request.json")
        let rendered = try prettyPrintedJSON(from: body)
        try rendered.write(to: requestURL, options: .atomic)
    }

    static func persistDaReceiptArtifacts(responseBody: Data,
                                          pdpHeader: String?,
                                          directory: URL,
                                          fileManager: FileManager) throws {
        try fileManager.createDirectory(at: directory, withIntermediateDirectories: true, attributes: nil)
        let receiptURL = directory.appendingPathComponent("da_receipt.json")
        let rendered = try prettyPrintedJSON(from: responseBody)
        try rendered.write(to: receiptURL, options: .atomic)
        if let header = pdpHeader {
            let headerURL = directory.appendingPathComponent("da_response_headers.json")
            let headersObject = [ToriiPdpCommitmentHeader: header]
            let headerData = try JSONSerialization.data(withJSONObject: headersObject,
                                                        options: [.prettyPrinted, .sortedKeys])
            try headerData.write(to: headerURL, options: .atomic)
        }
    }

    static func sanitizeDaLabel(_ raw: String?) throws -> String {
        guard var label = raw?.trimmingCharacters(in: .whitespacesAndNewlines), !label.isEmpty else {
            throw ToriiClientError.invalidPayload("DA ticket label must not be empty")
        }
        if label.hasPrefix("0x") || label.hasPrefix("0X") {
            label = String(label.dropFirst(2))
        }
        label = label.lowercased()
        let allowed = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyz0123456789-_")
        if label.rangeOfCharacter(from: allowed.inverted) != nil {
            throw ToriiClientError.invalidPayload("DA ticket label contains unsupported characters")
        }
        return label
    }

    static func persistDaManifestBundle(_ bundle: ToriiDaManifestBundle,
                                        outputDir: URL,
                                        label: String?,
                                        fileManager: FileManager) throws -> ToriiDaManifestPersistedPaths {
        let sanitized = try sanitizeDaLabel(label ?? bundle.storageTicketHex)
        try fileManager.createDirectory(at: outputDir, withIntermediateDirectories: true, attributes: nil)
        let manifestURL = outputDir.appendingPathComponent("manifest_\(sanitized).norito")
        let manifestJsonURL = outputDir.appendingPathComponent("manifest_\(sanitized).json")
        let chunkPlanURL = outputDir.appendingPathComponent("chunk_plan_\(sanitized).json")
        let samplingPlanURL: URL?
        if bundle.samplingPlan != nil {
            samplingPlanURL = outputDir.appendingPathComponent("sampling_plan_\(sanitized).json")
        } else {
            samplingPlanURL = nil
        }

        try bundle.manifestBytes.write(to: manifestURL, options: Data.WritingOptions.atomic)
        if let manifestJson = bundle.manifestJson {
            let data = try manifestJson.encodedData(prettyPrinted: true)
            try data.write(to: manifestJsonURL, options: Data.WritingOptions.atomic)
        } else {
            try Data("null\n".utf8).write(to: manifestJsonURL, options: Data.WritingOptions.atomic)
        }
        let chunkPlanString = try bundle.chunkPlanJSONString(prettyPrinted: true)
        if let chunkData = chunkPlanString.data(using: .utf8) {
            try chunkData.write(to: chunkPlanURL, options: Data.WritingOptions.atomic)
        }
        if let samplingPlan = bundle.samplingPlan, let samplingPlanURL {
            let data = try samplingPlan.jsonPayload().encodedData(prettyPrinted: true)
            try data.write(to: samplingPlanURL, options: Data.WritingOptions.atomic)
        }

        return ToriiDaManifestPersistedPaths(manifestURL: manifestURL,
                                             manifestJsonURL: manifestJsonURL,
                                             chunkPlanURL: chunkPlanURL,
                                             samplingPlanURL: samplingPlanURL,
                                             label: sanitized)
    }
}

public struct ToriiAccountOnboardingResponse: Decodable, Sendable {
    public let accountId: String
    public let uaid: String
    public let txHashHex: String
    public let status: String

    private enum CodingKeys: String, CodingKey {
        case accountId = "account_id"
        case uaid
        case txHashHex = "tx_hash_hex"
        case status
    }
}

/// Share-ready QR payload returned by `/v1/explorer/accounts/{account_id}/qr`.
public struct ToriiExplorerAccountQr: Decodable, Sendable {
    public let canonicalId: String
    public let literal: String
    public let addressFormat: String
    public let networkPrefix: UInt16
    public let errorCorrection: String
    public let modules: UInt32
    public let qrVersion: UInt8
    public let svg: String

    /// Convenience view that maps the metric label into an `AccountAddressFormat`.
    public var preferredFormat: AccountAddressFormat? {
        switch addressFormat.lowercased() {
        case "ih58": return .ih58
        case "compressed": return .compressed
        default: return nil
        }
    }

    private enum CodingKeys: String, CodingKey {
        case canonicalId = "canonical_id"
        case literal
        case addressFormat = "address_format"
        case networkPrefix = "network_prefix"
        case errorCorrection = "error_correction"
        case modules
        case qrVersion = "qr_version"
        case svg
    }
}

/// Pagination metadata returned by explorer list endpoints.
public struct ToriiExplorerPaginationMeta: Decodable, Sendable, Equatable {
    public let page: UInt64
    public let perPage: UInt64
    public let totalPages: UInt64
    public let totalItems: UInt64

    private enum CodingKeys: String, CodingKey {
        case page
        case perPage = "per_page"
        case totalPages = "total_pages"
        case totalItems = "total_items"
    }
}

/// Instruction payload wrapper returned by `/v1/explorer/instructions`.
public struct ToriiExplorerInstructionBox: Decodable, Sendable, Equatable {
    public let scale: String
    public let json: ToriiJSONValue
}

/// Instruction projection returned by `/v1/explorer/instructions`.
public struct ToriiExplorerInstructionItem: Decodable, Sendable, Equatable {
    public let authority: String
    public let createdAt: String
    public let kind: String
    public let box: ToriiExplorerInstructionBox
    public let transactionHash: String
    public let transactionStatus: String
    public let block: UInt64
    public let index: UInt32

    private enum CodingKeys: String, CodingKey {
        case authority
        case createdAt = "created_at"
        case kind
        case box = "box"
        case transactionHash = "transaction_hash"
        case transactionStatus = "transaction_status"
        case block
        case index
    }
}

/// Paginated instruction list returned by `/v1/explorer/instructions`.
public struct ToriiExplorerInstructionsPage: Decodable, Sendable, Equatable {
    public let pagination: ToriiExplorerPaginationMeta
    public let items: [ToriiExplorerInstructionItem]
}

/// Transaction summary returned by `/v1/explorer/transactions`.
public struct ToriiExplorerTransactionItem: Decodable, Sendable, Equatable {
    public let authority: String
    public let hash: String
    public let block: UInt64
    public let createdAt: String
    public let executable: String
    public let status: String

    private enum CodingKeys: String, CodingKey {
        case authority
        case hash
        case block
        case createdAt = "created_at"
        case executable
        case status
    }
}

/// Paginated transaction list returned by `/v1/explorer/transactions`.
public struct ToriiExplorerTransactionsPage: Decodable, Sendable, Equatable {
    public let pagination: ToriiExplorerPaginationMeta
    public let items: [ToriiExplorerTransactionItem]
}

/// Explorer duration wrapper (milliseconds).
public struct ToriiExplorerDuration: Decodable, Sendable, Equatable {
    public let ms: UInt64
}

/// Rejection details for explorer transaction detail payloads.
public struct ToriiExplorerTransactionRejection: Decodable, Sendable, Equatable {
    public let scale: String
    public let json: ToriiJSONValue
}

/// Transaction detail returned by `/v1/explorer/transactions/{hash}`.
public struct ToriiExplorerTransactionDetail: Decodable, Sendable, Equatable {
    public let authority: String
    public let hash: String
    public let block: UInt64
    public let createdAt: String
    public let executable: String
    public let status: String
    public let rejectionReason: ToriiExplorerTransactionRejection?
    public let metadata: ToriiJSONValue
    public let nonce: UInt64?
    public let signature: String
    public let timeToLive: ToriiExplorerDuration?

    private enum CodingKeys: String, CodingKey {
        case authority
        case hash
        case block
        case createdAt = "created_at"
        case executable
        case status
        case rejectionReason = "rejection_reason"
        case metadata
        case nonce
        case signature
        case timeToLive = "time_to_live"
    }
}

/// Transfer record derived from explorer instruction payloads.
public struct ToriiExplorerTransferRecord: Sendable, Equatable {
    public let instruction: ToriiExplorerInstructionItem
    public let details: ToriiExplorerTransferDetails
}

/// Direction of a transfer relative to an account.
public enum ToriiExplorerTransferDirection: Sendable, Equatable {
    case incoming
    case outgoing
    case selfTransfer
    case unknown
}

/// Flattened transfer summary derived from explorer instruction payloads.
public struct ToriiExplorerTransferSummary: Sendable, Equatable, Identifiable {
    public let transactionHash: String
    public let block: UInt64
    public let createdAt: String
    public let status: String
    public let authority: String
    public let instructionIndex: UInt32
    /// Index of the transfer entry within the instruction payload (0 for single-asset transfers).
    public let transferIndex: UInt32
    public let senderAccountId: String
    public let receiverAccountId: String
    public let assetDefinitionId: String
    public let amount: String
    public let direction: ToriiExplorerTransferDirection

    public var isIncoming: Bool {
        direction == .incoming
    }

    public var isOutgoing: Bool {
        direction == .outgoing
    }

    public var isSelfTransfer: Bool {
        direction == .selfTransfer
    }

    /// Construct the source asset id from the asset definition and sender account when possible.
    public var sourceAssetId: String? {
        ToriiExplorerTransferSummary.assetId(assetDefinitionId: assetDefinitionId,
                                             accountId: senderAccountId)
    }

    /// Construct the destination asset id from the asset definition and receiver account when possible.
    public var destinationAssetId: String? {
        ToriiExplorerTransferSummary.assetId(assetDefinitionId: assetDefinitionId,
                                             accountId: receiverAccountId)
    }

    /// Stable identifier for SwiftUI lists (`transactionHash|instructionIndex|transferIndex`).
    public var id: String {
        "\(transactionHash)|\(instructionIndex)|\(transferIndex)"
    }

    /// Recompute direction relative to a specific account identifier.
    public func direction(relativeTo accountId: String?) -> ToriiExplorerTransferDirection {
        let trimmed = accountId?.trimmingCharacters(in: .whitespacesAndNewlines)
        let relative = (trimmed?.isEmpty ?? true) ? nil : trimmed
        return ToriiExplorerTransferSummary.direction(sender: senderAccountId,
                                                      receiver: receiverAccountId,
                                                      relativeTo: relative)
    }

    /// Return the counterparty account id relative to a specific account identifier.
    public func counterpartyAccountId(relativeTo accountId: String?) -> String? {
        let trimmed = accountId?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let trimmed, !trimmed.isEmpty else {
            return nil
        }
        if senderAccountId == trimmed && receiverAccountId == trimmed {
            return trimmed
        }
        if senderAccountId == trimmed {
            return receiverAccountId
        }
        if receiverAccountId == trimmed {
            return senderAccountId
        }
        return nil
    }

    public func isIncoming(relativeTo accountId: String?) -> Bool {
        direction(relativeTo: accountId) == .incoming
    }

    public func isOutgoing(relativeTo accountId: String?) -> Bool {
        direction(relativeTo: accountId) == .outgoing
    }

    public func isSelfTransfer(relativeTo accountId: String?) -> Bool {
        direction(relativeTo: accountId) == .selfTransfer
    }

    /// Return the asset id scoped to the provided account (source for sender, destination for receiver).
    public func assetId(relativeTo accountId: String?) -> String? {
        let trimmed = accountId?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let trimmed, !trimmed.isEmpty else {
            return nil
        }
        if senderAccountId == trimmed && receiverAccountId == trimmed {
            return sourceAssetId ?? destinationAssetId
        }
        if senderAccountId == trimmed {
            return sourceAssetId
        }
        if receiverAccountId == trimmed {
            return destinationAssetId
        }
        return nil
    }

    /// Return the counterparty asset id relative to the provided account (destination for sender, source for receiver).
    public func counterpartyAssetId(relativeTo accountId: String?) -> String? {
        let trimmed = accountId?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let trimmed, !trimmed.isEmpty else {
            return nil
        }
        if senderAccountId == trimmed && receiverAccountId == trimmed {
            return sourceAssetId ?? destinationAssetId
        }
        if senderAccountId == trimmed {
            return destinationAssetId
        }
        if receiverAccountId == trimmed {
            return sourceAssetId
        }
        return nil
    }

    /// Return the amount with a sign relative to the provided account (`+` for incoming, `-` for outgoing).
    public func signedAmount(relativeTo accountId: String?) -> String {
        let trimmedAmount = amount.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedAmount.isEmpty else {
            return amount
        }
        let hasSign = trimmedAmount.hasPrefix("-") || trimmedAmount.hasPrefix("+")
        switch direction(relativeTo: accountId) {
        case .incoming:
            return hasSign ? trimmedAmount : "+\(trimmedAmount)"
        case .outgoing:
            return hasSign ? trimmedAmount : "-\(trimmedAmount)"
        case .selfTransfer, .unknown:
            return trimmedAmount
        }
    }

    public init(transactionHash: String,
                block: UInt64,
                createdAt: String,
                status: String,
                authority: String,
                instructionIndex: UInt32,
                senderAccountId: String,
                receiverAccountId: String,
                assetDefinitionId: String,
                amount: String,
                direction: ToriiExplorerTransferDirection,
                transferIndex: UInt32 = 0) {
        self.transactionHash = transactionHash
        self.block = block
        self.createdAt = createdAt
        self.status = status
        self.authority = authority
        self.instructionIndex = instructionIndex
        self.transferIndex = transferIndex
        self.senderAccountId = senderAccountId
        self.receiverAccountId = receiverAccountId
        self.assetDefinitionId = assetDefinitionId
        self.amount = amount
        self.direction = direction
    }
}

/// Parsed transfer details derived from explorer instruction payloads.
public struct ToriiExplorerTransferAsset: Sendable, Equatable {
    /// Full asset identifier literal (includes the source account).
    public let sourceAssetId: String
    /// Destination account literal.
    public let destinationAccountId: String
    /// Amount transferred (string numeric).
    public let amount: String
    /// Source account literal (derived from the asset id when possible).
    public let senderAccountId: String?
    /// Asset definition literal (derived from the asset id when possible).
    public let assetDefinitionId: String?
}

/// Parsed transfer entry derived from explorer batch transfer payloads.
public struct ToriiExplorerTransferBatchEntry: Sendable, Equatable {
    /// Source account literal.
    public let senderAccountId: String
    /// Destination account literal.
    public let receiverAccountId: String
    /// Asset definition literal.
    public let assetDefinitionId: String
    /// Amount transferred (string numeric).
    public let amount: String
}

/// Role of an account within a transfer instruction.
public enum ToriiExplorerTransferRole: Sendable, Equatable {
    case sender
    case receiver
    case both
}

/// Transfer details derived from explorer instruction payloads.
public enum ToriiExplorerTransferDetails: Sendable, Equatable {
    case asset(ToriiExplorerTransferAsset)
    case assetBatch([ToriiExplorerTransferBatchEntry])
}

public extension ToriiExplorerInstructionItem {
    /// Extract transfer details from the instruction payload when possible.
    func transferDetails() -> ToriiExplorerTransferDetails? {
        let normalizedKind = kind.trimmingCharacters(in: .whitespacesAndNewlines)
        guard normalizedKind.lowercased() == "transfer" else {
            return nil
        }
        return ToriiExplorerTransferDetails.parse(from: box.json)
    }
}

public extension ToriiExplorerTransferDetails {
    /// Determine how an account participates in this transfer.
    func role(for accountId: String) -> ToriiExplorerTransferRole? {
        let trimmed = accountId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return nil
        }
        switch self {
        case .asset(let asset):
            let senderMatches = asset.senderAccountId == trimmed
            let receiverMatches = asset.destinationAccountId == trimmed
            if senderMatches && receiverMatches {
                return .both
            } else if senderMatches {
                return .sender
            } else if receiverMatches {
                return .receiver
            } else {
                return nil
            }
        case .assetBatch(let entries):
            var senderMatch = false
            var receiverMatch = false
            for entry in entries {
                if entry.senderAccountId == trimmed {
                    senderMatch = true
                }
                if entry.receiverAccountId == trimmed {
                    receiverMatch = true
                }
                if senderMatch && receiverMatch {
                    return .both
                }
            }
            if senderMatch {
                return .sender
            }
            if receiverMatch {
                return .receiver
            }
            return nil
        }
    }

    /// Check whether a transfer involves the provided account.
    func involvesAccount(_ accountId: String) -> Bool {
        return role(for: accountId) != nil
    }

    /// Check whether a transfer involves the provided asset definition.
    func involvesAssetDefinition(_ assetDefinitionId: String) -> Bool {
        let trimmed = assetDefinitionId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return false
        }
        switch self {
        case .asset(let asset):
            return asset.assetDefinitionId == trimmed
        case .assetBatch(let entries):
            return entries.contains { $0.assetDefinitionId == trimmed }
        }
    }

    /// Check whether a transfer involves the provided asset id (source asset literal).
    func involvesAssetId(_ assetId: String) -> Bool {
        let trimmed = assetId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return false
        }
        switch self {
        case .asset(let asset):
            return asset.sourceAssetId == trimmed
        case .assetBatch(let entries):
            return entries.contains {
                ToriiExplorerTransferSummary.assetId(assetDefinitionId: $0.assetDefinitionId,
                                                     accountId: $0.senderAccountId) == trimmed
            }
        }
    }
}

extension ToriiExplorerTransferSummary {
    fileprivate static func direction(sender: String,
                                      receiver: String,
                                      relativeTo accountId: String?) -> ToriiExplorerTransferDirection {
        guard let accountId, !accountId.isEmpty else {
            return .unknown
        }
        if sender == accountId && receiver == accountId {
            return .selfTransfer
        }
        if sender == accountId {
            return .outgoing
        }
        if receiver == accountId {
            return .incoming
        }
        return .unknown
    }

    fileprivate static func assetId(assetDefinitionId: String, accountId: String) -> String? {
        let trimmedDefinition = assetDefinitionId.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedAccount = accountId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedDefinition.isEmpty, !trimmedAccount.isEmpty else {
            return nil
        }
        guard let hashIndex = trimmedDefinition.firstIndex(of: "#") else {
            return nil
        }
        let name = String(trimmedDefinition[..<hashIndex])
        guard !name.isEmpty else {
            return nil
        }
        return "\(name)##\(trimmedAccount)"
    }
}

public extension ToriiExplorerTransferRecord {
    /// Convert this transfer record into flattened transfer summaries.
    func summaries(relativeTo accountId: String? = nil) -> [ToriiExplorerTransferSummary] {
        let relative = accountId?.trimmingCharacters(in: .whitespacesAndNewlines)
        let relativeAccount = (relative?.isEmpty ?? true) ? nil : relative
        switch details {
        case .asset(let asset):
            guard let sender = asset.senderAccountId,
                  let assetDefinition = asset.assetDefinitionId else {
                return []
            }
            let receiver = asset.destinationAccountId
            let direction = ToriiExplorerTransferSummary.direction(sender: sender,
                                                                    receiver: receiver,
                                                                    relativeTo: relativeAccount)
            return [
                ToriiExplorerTransferSummary(transactionHash: instruction.transactionHash,
                                             block: instruction.block,
                                             createdAt: instruction.createdAt,
                                             status: instruction.transactionStatus,
                                             authority: instruction.authority,
                                             instructionIndex: instruction.index,
                                             senderAccountId: sender,
                                             receiverAccountId: receiver,
                                             assetDefinitionId: assetDefinition,
                                             amount: asset.amount,
                                             direction: direction,
                                             transferIndex: 0),
            ]
        case .assetBatch(let entries):
            return entries.enumerated().map { offset, entry in
                let direction = ToriiExplorerTransferSummary.direction(sender: entry.senderAccountId,
                                                                        receiver: entry.receiverAccountId,
                                                                        relativeTo: relativeAccount)
                return ToriiExplorerTransferSummary(transactionHash: instruction.transactionHash,
                                                    block: instruction.block,
                                                    createdAt: instruction.createdAt,
                                                    status: instruction.transactionStatus,
                                                    authority: instruction.authority,
                                                    instructionIndex: instruction.index,
                                                    senderAccountId: entry.senderAccountId,
                                                    receiverAccountId: entry.receiverAccountId,
                                                    assetDefinitionId: entry.assetDefinitionId,
                                                    amount: entry.amount,
                                                    direction: direction,
                                                    transferIndex: UInt32(offset))
            }
        }
    }
}

public extension ToriiExplorerInstructionsPage {
    /// Extract flattened transfer summaries from this page, optionally filtering by account, asset definition, or asset id.
    func transferSummaries(matchingAccount accountId: String? = nil,
                           assetDefinitionId: String? = nil,
                           assetId: String? = nil,
                           relativeTo relativeAccountId: String? = nil) -> [ToriiExplorerTransferSummary] {
        let relative = relativeAccountId ?? accountId
        let records = transferRecords(matchingAccount: accountId,
                                      assetDefinitionId: assetDefinitionId,
                                      assetId: assetId)
        return records.flatMap { $0.summaries(relativeTo: relative) }
    }
}

public extension ToriiExplorerInstructionsPage {
    /// Extract transfer records from this page, optionally filtering by account, asset definition, or asset id.
    func transferRecords(matchingAccount accountId: String? = nil,
                         assetDefinitionId: String? = nil,
                         assetId: String? = nil) -> [ToriiExplorerTransferRecord] {
        let accountFilter = accountId?.trimmingCharacters(in: .whitespacesAndNewlines)
        let assetFilter = assetDefinitionId?.trimmingCharacters(in: .whitespacesAndNewlines)
        let assetIdFilter = assetId?.trimmingCharacters(in: .whitespacesAndNewlines)
        var records: [ToriiExplorerTransferRecord] = []
        records.reserveCapacity(items.count)
        for item in items {
            guard let details = item.transferDetails() else {
                continue
            }
            if let accountFilter, !accountFilter.isEmpty, !details.involvesAccount(accountFilter) {
                continue
            }
            if let assetFilter, !assetFilter.isEmpty, !details.involvesAssetDefinition(assetFilter) {
                continue
            }
            if let assetIdFilter, !assetIdFilter.isEmpty, !details.involvesAssetId(assetIdFilter) {
                continue
            }
            records.append(ToriiExplorerTransferRecord(instruction: item, details: details))
        }
        return records
    }
}

/// Query parameters accepted by `/v1/explorer/instructions`.
public struct ToriiExplorerInstructionsParams: Sendable, Equatable {
    public var page: UInt64?
    public var perPage: UInt64?
    public var addressFormat: AccountAddressFormat?
    public var authority: String?
    public var transactionHash: String?
    public var transactionStatus: String?
    public var block: UInt64?
    public var kind: String?
    public var assetId: String?

    public init(page: UInt64? = nil,
                perPage: UInt64? = nil,
                addressFormat: AccountAddressFormat? = nil,
                authority: String? = nil,
                transactionHash: String? = nil,
                transactionStatus: String? = nil,
                block: UInt64? = nil,
                kind: String? = nil,
                assetId: String? = nil) {
        self.page = page
        self.perPage = perPage
        self.addressFormat = addressFormat
        self.authority = authority
        self.transactionHash = transactionHash
        self.transactionStatus = transactionStatus
        self.block = block
        self.kind = kind
        self.assetId = assetId
    }

    public func queryItems() throws -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let page {
            guard page > 0 else {
                throw ToriiClientError.invalidPayload("page must be at least 1.")
            }
            items.append(URLQueryItem(name: "page", value: String(page)))
        }
        if let perPage {
            guard perPage > 0 else {
                throw ToriiClientError.invalidPayload("perPage must be at least 1.")
            }
            items.append(URLQueryItem(name: "per_page", value: String(perPage)))
        }
        if let addressFormat {
            let value = try ToriiClient.explorerAddressFormatQueryValue(addressFormat,
                                                                        context: "explorer instructions")
            items.append(URLQueryItem(name: "address_format", value: value))
        }
        if let authority = authority?.trimmingCharacters(in: .whitespacesAndNewlines),
           !authority.isEmpty {
            items.append(URLQueryItem(name: "authority", value: authority))
        }
        if let transactionHash = transactionHash?.trimmingCharacters(in: .whitespacesAndNewlines),
           !transactionHash.isEmpty {
            items.append(URLQueryItem(name: "transaction_hash", value: transactionHash))
        }
        if let transactionStatus = transactionStatus?.trimmingCharacters(in: .whitespacesAndNewlines),
           !transactionStatus.isEmpty {
            items.append(URLQueryItem(name: "transaction_status", value: transactionStatus))
        }
        if let block {
            guard block > 0 else {
                throw ToriiClientError.invalidPayload("block must be at least 1.")
            }
            items.append(URLQueryItem(name: "block", value: String(block)))
        }
        if let kind = kind?.trimmingCharacters(in: .whitespacesAndNewlines),
           !kind.isEmpty {
            items.append(URLQueryItem(name: "kind", value: kind))
        }
        if let assetId = assetId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !assetId.isEmpty {
            items.append(URLQueryItem(name: "asset_id", value: assetId))
        }
        return items.isEmpty ? nil : items
    }
}

/// Query parameters accepted by `/v1/explorer/transactions`.
public struct ToriiExplorerTransactionsParams: Sendable, Equatable {
    public var page: UInt64?
    public var perPage: UInt64?
    public var addressFormat: AccountAddressFormat?
    public var authority: String?
    public var block: UInt64?
    public var status: String?
    public var assetId: String?

    public init(page: UInt64? = nil,
                perPage: UInt64? = nil,
                addressFormat: AccountAddressFormat? = nil,
                authority: String? = nil,
                block: UInt64? = nil,
                status: String? = nil,
                assetId: String? = nil) {
        self.page = page
        self.perPage = perPage
        self.addressFormat = addressFormat
        self.authority = authority
        self.block = block
        self.status = status
        self.assetId = assetId
    }

    public func queryItems() throws -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let page {
            guard page > 0 else {
                throw ToriiClientError.invalidPayload("page must be at least 1.")
            }
            items.append(URLQueryItem(name: "page", value: String(page)))
        }
        if let perPage {
            guard perPage > 0 else {
                throw ToriiClientError.invalidPayload("perPage must be at least 1.")
            }
            items.append(URLQueryItem(name: "per_page", value: String(perPage)))
        }
        if let addressFormat {
            let value = try ToriiClient.explorerAddressFormatQueryValue(addressFormat,
                                                                        context: "explorer transactions")
            items.append(URLQueryItem(name: "address_format", value: value))
        }
        if let authority = authority?.trimmingCharacters(in: .whitespacesAndNewlines),
           !authority.isEmpty {
            items.append(URLQueryItem(name: "authority", value: authority))
        }
        if let block {
            guard block > 0 else {
                throw ToriiClientError.invalidPayload("block must be at least 1.")
            }
            items.append(URLQueryItem(name: "block", value: String(block)))
        }
        if let status = status?.trimmingCharacters(in: .whitespacesAndNewlines),
           !status.isEmpty {
            items.append(URLQueryItem(name: "status", value: status))
        }
        if let assetId = assetId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !assetId.isEmpty {
            items.append(URLQueryItem(name: "asset_id", value: assetId))
        }
        return items.isEmpty ? nil : items
    }
}

extension ToriiExplorerTransferDetails {
    fileprivate static func parse(from json: ToriiJSONValue) -> ToriiExplorerTransferDetails? {
        guard let payload = payloadObject(from: json) else {
            return nil
        }
        guard let variantRaw = stringValue(payload["variant"]) else {
            return nil
        }
        let variant = variantRaw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard let value = payload["value"] else {
            return nil
        }
        switch variant {
        case "asset":
            return parseAssetTransfer(value)
        case "assetbatch":
            return parseAssetBatchTransfer(value)
        default:
            return nil
        }
    }

    fileprivate static func payloadObject(from json: ToriiJSONValue) -> [String: ToriiJSONValue]? {
        guard case let .object(root) = json else {
            return nil
        }
        if let payload = root["payload"], case let .object(map) = payload {
            return map
        }
        if root["variant"] != nil, root["value"] != nil {
            return root
        }
        return nil
    }

    fileprivate static func parseAssetTransfer(_ value: ToriiJSONValue) -> ToriiExplorerTransferDetails? {
        guard case let .object(map) = value else {
            return nil
        }
        guard let source = stringValue(map["source"]),
              let destination = stringValue(map["destination"]),
              let amount = stringValue(map["object"]) else {
            return nil
        }
        let (definition, sender) = parseAssetId(source)
        let details = ToriiExplorerTransferAsset(sourceAssetId: source,
                                                  destinationAccountId: destination,
                                                  amount: amount,
                                                  senderAccountId: sender,
                                                  assetDefinitionId: definition)
        return .asset(details)
    }

    fileprivate static func parseAssetBatchTransfer(_ value: ToriiJSONValue) -> ToriiExplorerTransferDetails? {
        guard case let .object(map) = value else {
            return nil
        }
        guard case let .array(entriesValue) = map["entries"] else {
            return nil
        }
        var entries: [ToriiExplorerTransferBatchEntry] = []
        entries.reserveCapacity(entriesValue.count)
        for entry in entriesValue {
            guard case let .object(entryMap) = entry else {
                return nil
            }
            guard let sender = stringValue(entryMap["from"]),
                  let receiver = stringValue(entryMap["to"]),
                  let assetDefinition = stringValue(entryMap["asset_definition"]),
                  let amount = stringValue(entryMap["amount"]) else {
                return nil
            }
            entries.append(ToriiExplorerTransferBatchEntry(senderAccountId: sender,
                                                           receiverAccountId: receiver,
                                                           assetDefinitionId: assetDefinition,
                                                           amount: amount))
        }
        guard !entries.isEmpty else {
            return nil
        }
        return .assetBatch(entries)
    }

    fileprivate static func stringValue(_ value: ToriiJSONValue?) -> String? {
        guard let value else { return nil }
        switch value {
        case .string(let string):
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? nil : trimmed
        case .number(let number):
            guard number.isFinite else { return nil }
            return String(number)
        default:
            return nil
        }
    }

    fileprivate static func parseAssetId(_ value: String) -> (definition: String?, account: String?) {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return (nil, nil)
        }
        let account = trimmed.split(separator: "#").last.map(String.init)
        if let range = trimmed.range(of: "##") {
            let name = String(trimmed[..<range.lowerBound])
            let accountLiteral = String(trimmed[range.upperBound...])
            if let atIndex = accountLiteral.lastIndex(of: "@") {
                let domain = String(accountLiteral[accountLiteral.index(after: atIndex)...])
                if !name.isEmpty, !domain.isEmpty {
                    return ("\(name)#\(domain)", account)
                }
            }
            return (nil, account)
        }
        let parts = trimmed.split(separator: "#")
        if parts.count >= 3 {
            let name = String(parts[0])
            let domain = String(parts[1])
            if !name.isEmpty, !domain.isEmpty {
                return ("\(name)#\(domain)", account)
            }
        }
        return (nil, account)
    }
}

public struct ToriiDomainRecord: Decodable, Sendable {
    public let id: String
    public let ownedBy: String?
    public let metadata: [String: ToriiJSONValue]
    public let raw: [String: ToriiJSONValue]

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let raw = try container.decode([String: ToriiJSONValue].self)
        guard case let .string(identifier)? = raw["id"], !identifier.isEmpty else {
            throw ToriiClientError.invalidPayload("domain record missing string `id` field")
        }
        let owner: String?
        if let ownedValue = raw["owned_by"] {
            switch ownedValue {
            case .string(let value):
                owner = value
            case .null:
                owner = nil
            default:
                throw ToriiClientError.invalidPayload("domain record `owned_by` must be a string when present")
            }
        } else {
            owner = nil
        }
        let metadata: [String: ToriiJSONValue]
        if let metadataValue = raw["metadata"] {
            switch metadataValue {
            case .object(let object):
                metadata = object
            case .null:
                metadata = [:]
            default:
                throw ToriiClientError.invalidPayload("domain record `metadata` must be an object when present")
            }
        } else {
            metadata = [:]
        }
        id = identifier
        ownedBy = owner
        self.metadata = metadata
        self.raw = raw
    }
}

public struct ToriiDomainListPage: Decodable, Sendable, ToriiListPageProtocol {
    public let items: [ToriiDomainRecord]
    public let total: Int

    private enum CodingKeys: String, CodingKey {
        case items
        case total
    }

    public init(items: [ToriiDomainRecord], total: Int) {
        self.items = items
        self.total = total
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        items = try container.decodeIfPresent([ToriiDomainRecord].self, forKey: .items) ?? []
        if let explicitTotal = try container.decodeIfPresent(Int.self, forKey: .total) {
            total = explicitTotal
        } else {
            total = items.count
        }
    }
}

public protocol ToriiListPageProtocol: Sendable {
    associatedtype Item: Sendable
    var items: [Item] { get }
    var total: Int { get }
}

public enum ToriiListFilter: Sendable, Equatable {
    case expression(String)
    case json(ToriiJSONValue)

    func encodedValue() throws -> String? {
        switch self {
        case .expression(let value):
            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? nil : trimmed
        case .json(let json):
            let data = try json.encodedData()
            return String(data: data, encoding: .utf8)
        }
    }
}

public enum ToriiListSort: Sendable, Equatable {
    case expression(String)
    case fields([String])

    func encodedValue() -> String? {
        switch self {
        case .expression(let value):
            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? nil : trimmed
        case .fields(let values):
            let rendered = values
                .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                .filter { !$0.isEmpty }
                .joined(separator: ",")
            return rendered.isEmpty ? nil : rendered
        }
    }
}

public struct ToriiListOptions: Sendable, Equatable {
    public var filter: ToriiListFilter?
    public var sort: ToriiListSort?
    public var limit: Int?
    public var offset: Int?

    public init(filter: ToriiListFilter? = nil,
                sort: ToriiListSort? = nil,
                limit: Int? = nil,
                offset: Int? = nil) {
        self.filter = filter
        self.sort = sort
        self.limit = limit
        self.offset = offset
    }
}

public enum ToriiQueryOrder: String, Codable, Sendable {
    case asc
    case desc
}

public struct ToriiQuerySortKey: Codable, Sendable, Equatable {
    public var key: String
    public var order: ToriiQueryOrder?

    public init(key: String, order: ToriiQueryOrder? = nil) {
        self.key = key
        self.order = order
    }
}

public struct ToriiQueryPagination: Codable, Sendable, Equatable {
    public var limit: UInt64?
    public var offset: UInt64

    public init(limit: UInt64? = nil, offset: UInt64 = 0) {
        self.limit = limit
        self.offset = offset
    }
}

public struct ToriiQueryEnvelope: Codable, Sendable, Equatable {
    public var query: String?
    public var filter: ToriiJSONValue?
    public var select: [String]?
    public var sort: [ToriiQuerySortKey]
    public var pagination: ToriiQueryPagination
    public var fetchSize: UInt64?
    public var addressFormat: String?

    private enum CodingKeys: String, CodingKey {
        case query
        case filter
        case select
        case sort
        case pagination
        case fetchSize = "fetch_size"
        case addressFormat = "address_format"
    }

    public init(query: String? = nil,
                filter: ToriiJSONValue? = nil,
                select: [String]? = nil,
                sort: [ToriiQuerySortKey] = [],
                pagination: ToriiQueryPagination = ToriiQueryPagination(),
                fetchSize: UInt64? = nil,
                addressFormat: String? = nil) {
        self.query = query
        self.filter = filter
        self.select = select
        self.sort = sort
        self.pagination = pagination
        self.fetchSize = fetchSize
        self.addressFormat = addressFormat
    }
}

fileprivate enum ToriiConnectJSON {
    static func normalizedString(_ value: ToriiJSONValue?) -> String? {
        guard let value else { return nil }
        switch value {
        case .string(let string):
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? nil : trimmed
        case .number(let number):
            guard number.isFinite else { return nil }
            if number.rounded(.towardZero) == number {
                guard number >= Double(Int.min), number <= Double(Int.max) else {
                    return nil
                }
                return String(Int(number))
            }
            return String(number)
        case .bool(let bool):
            return bool ? "true" : "false"
        default:
            return nil
        }
    }

    static func optionalString(_ record: [String: ToriiJSONValue],
                               key: String) -> String? {
        normalizedString(record[key])
    }

    static func requireString(_ record: [String: ToriiJSONValue],
                              key: String,
                              field: String) throws -> String {
        if let value = optionalString(record, key: key) {
            return value
        }
        throw ToriiClientError.invalidPayload("\(field) field was missing or empty")
    }

    static func optionalBool(_ record: [String: ToriiJSONValue],
                             key: String) -> Bool? {
        guard let value = record[key] else { return nil }
        switch value {
        case .bool(let bool):
            return bool
        case .string(let string):
            let lowercased = string.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if lowercased == "true" { return true }
            if lowercased == "false" { return false }
        default:
            break
        }
        return nil
    }

    static func optionalUInt64(_ record: [String: ToriiJSONValue],
                               key: String) -> UInt64? {
        guard let value = record[key] else { return nil }
        return value.normalizedUInt64
    }

    static func requireUInt64(_ record: [String: ToriiJSONValue],
                              key: String,
                              field: String) throws -> UInt64 {
        if let value = optionalUInt64(record, key: key) {
            return value
        }
        throw ToriiClientError.invalidPayload("\(field) field was missing or invalid")
    }

    static func requireUInt64(_ record: [String: ToriiJSONValue],
                              key: String,
                              field: String,
                              allowZero: Bool) throws -> UInt64 {
        let value = try requireUInt64(record, key: key, field: field)
        if !allowZero && value == 0 {
            throw ToriiClientError.invalidPayload("\(field) must be greater than zero")
        }
        return value
    }

    static func optionalInt(_ record: [String: ToriiJSONValue],
                            key: String) -> Int? {
        guard let value = record[key] else { return nil }
        guard let parsed = value.normalizedInt64 else { return nil }
        guard parsed >= Int64(Int.min), parsed <= Int64(Int.max) else {
            return nil
        }
        return Int(parsed)
    }

    static func objectsArray(_ record: [String: ToriiJSONValue],
                             key: String,
                             field: String) throws -> [[String: ToriiJSONValue]] {
        guard let value = record[key] else { return [] }
        guard case .array(let array) = value else {
            throw ToriiClientError.invalidPayload("\(field) must be an array")
        }
        return try array.map { value in
            guard case .object(let object) = value else {
                throw ToriiClientError.invalidPayload("\(field) entries must be objects")
            }
            return object
        }
    }

    static func requireObject(_ record: [String: ToriiJSONValue],
                              key: String,
                              field: String) throws -> [String: ToriiJSONValue] {
        guard let value = record[key] else {
            throw ToriiClientError.invalidPayload("\(field) field was missing or invalid")
        }
        if case .object(let object) = value {
            return object
        }
        throw ToriiClientError.invalidPayload("\(field) must be an object")
    }

    static func optionalObject(_ record: [String: ToriiJSONValue],
                               key: String) -> [String: ToriiJSONValue]? {
        guard let value = record[key] else { return nil }
        if case .object(let object) = value {
            return object
        }
        return nil
    }

    static func stringArray(_ value: ToriiJSONValue?,
                            field: String) throws -> [String] {
        guard let value else { return [] }
        guard case .array(let values) = value else {
            throw ToriiClientError.invalidPayload("\(field) must be an array")
        }
        var result: [String] = []
        for item in values {
            guard case .string(let raw) = item else {
                throw ToriiClientError.invalidPayload("\(field) entries must be strings")
            }
            let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                throw ToriiClientError.invalidPayload("\(field) entries must not be empty")
            }
            result.append(trimmed)
        }
        return result
    }

    static func mergeExtra(record: [String: ToriiJSONValue],
                           knownKeys: Set<String>) -> [String: ToriiJSONValue] {
        var extra: [String: ToriiJSONValue] = [:]
        for (key, value) in record where !knownKeys.contains(key) {
            extra[key] = value
        }
        return extra
    }

    static func encodePayload(_ payload: [String: ToriiJSONValue]) throws -> Data {
        try ToriiJSONValue.object(payload).encodedData()
    }

    static func trimmedNonEmpty(_ value: String,
                                field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("\(field) must be a non-empty string")
        }
        return trimmed
    }
}

public enum ToriiConnectRole: String, Sendable {
    case app
    case wallet
}

public struct ToriiConnectPerIpSessions: Decodable, Sendable, Equatable {
    public let ip: String
    public let sessions: UInt64
    public let raw: [String: ToriiJSONValue]

    public init(raw: [String: ToriiJSONValue]) throws {
        self.raw = raw
        self.ip = try ToriiConnectJSON.requireString(raw, key: "ip", field: "ip")
        self.sessions = try ToriiConnectJSON.requireUInt64(raw, key: "sessions", field: "sessions")
    }

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode([String: ToriiJSONValue].self)
        try self.init(raw: raw)
    }
}

public struct ToriiConnectStatusPolicySnapshot: Decodable, Sendable, Equatable {
    public let wsMaxSessions: UInt64?
    public let wsPerIpMaxSessions: UInt64?
    public let wsRatePerIpPerMin: UInt64?
    public let sessionTtlMs: UInt64?
    public let frameMaxBytes: UInt64?
    public let sessionBufferMaxBytes: UInt64?
    public let relayEnabled: Bool?
    public let heartbeatIntervalMs: UInt64?
    public let heartbeatMissTolerance: UInt64?
    public let heartbeatMinIntervalMs: UInt64?
    public let extra: [String: ToriiJSONValue]
    public let raw: [String: ToriiJSONValue]

    public init(raw: [String: ToriiJSONValue]) {
        self.raw = raw
        wsMaxSessions = ToriiConnectJSON.optionalUInt64(raw, key: "ws_max_sessions")
        wsPerIpMaxSessions = ToriiConnectJSON.optionalUInt64(raw, key: "ws_per_ip_max_sessions")
        wsRatePerIpPerMin = ToriiConnectJSON.optionalUInt64(raw, key: "ws_rate_per_ip_per_min")
        sessionTtlMs = ToriiConnectJSON.optionalUInt64(raw, key: "session_ttl_ms")
        frameMaxBytes = ToriiConnectJSON.optionalUInt64(raw, key: "frame_max_bytes")
        sessionBufferMaxBytes = ToriiConnectJSON.optionalUInt64(raw, key: "session_buffer_max_bytes")
        relayEnabled = ToriiConnectJSON.optionalBool(raw, key: "relay_enabled")
        heartbeatIntervalMs = ToriiConnectJSON.optionalUInt64(raw, key: "heartbeat_interval_ms")
        heartbeatMissTolerance = ToriiConnectJSON.optionalUInt64(raw, key: "heartbeat_miss_tolerance")
        heartbeatMinIntervalMs = ToriiConnectJSON.optionalUInt64(raw, key: "heartbeat_min_interval_ms")
        let known: Set<String> = [
            "ws_max_sessions",
            "ws_per_ip_max_sessions",
            "ws_rate_per_ip_per_min",
            "session_ttl_ms",
            "frame_max_bytes",
            "session_buffer_max_bytes",
            "relay_enabled",
            "heartbeat_interval_ms",
            "heartbeat_miss_tolerance",
            "heartbeat_min_interval_ms"
        ]
        extra = ToriiConnectJSON.mergeExtra(record: raw, knownKeys: known)
    }

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode([String: ToriiJSONValue].self)
        self.init(raw: raw)
    }
}

public struct ToriiConnectStatusSnapshot: Decodable, Sendable, Equatable {
    public let enabled: Bool
    public let sessionsTotal: UInt64
    public let sessionsActive: UInt64
    public let perIpSessions: [ToriiConnectPerIpSessions]
    public let bufferedSessions: UInt64
    public let totalBufferBytes: UInt64
    public let dedupeSize: UInt64
    public let policy: ToriiConnectStatusPolicySnapshot?
    public let framesInTotal: UInt64
    public let framesOutTotal: UInt64
    public let ciphertextTotal: UInt64
    public let dedupeDropsTotal: UInt64
    public let bufferDropsTotal: UInt64
    public let plaintextControlDropsTotal: UInt64
    public let monotonicDropsTotal: UInt64
    public let pingMissTotal: UInt64
    public let raw: [String: ToriiJSONValue]

    public init(raw: [String: ToriiJSONValue]) throws {
        self.raw = raw
        enabled = ToriiConnectJSON.optionalBool(raw, key: "enabled") ?? false
        sessionsTotal = try ToriiConnectJSON.requireUInt64(raw, key: "sessions_total", field: "sessions_total")
        sessionsActive = try ToriiConnectJSON.requireUInt64(raw, key: "sessions_active", field: "sessions_active")
        bufferedSessions = try ToriiConnectJSON.requireUInt64(raw, key: "buffered_sessions", field: "buffered_sessions")
        totalBufferBytes = try ToriiConnectJSON.requireUInt64(raw, key: "total_buffer_bytes", field: "total_buffer_bytes")
        dedupeSize = try ToriiConnectJSON.requireUInt64(raw, key: "dedupe_size", field: "dedupe_size")
        framesInTotal = try ToriiConnectJSON.requireUInt64(raw, key: "frames_in_total", field: "frames_in_total")
        framesOutTotal = try ToriiConnectJSON.requireUInt64(raw, key: "frames_out_total", field: "frames_out_total")
        ciphertextTotal = try ToriiConnectJSON.requireUInt64(raw, key: "ciphertext_total", field: "ciphertext_total")
        dedupeDropsTotal = try ToriiConnectJSON.requireUInt64(raw, key: "dedupe_drops_total", field: "dedupe_drops_total")
        bufferDropsTotal = try ToriiConnectJSON.requireUInt64(raw, key: "buffer_drops_total", field: "buffer_drops_total")
        plaintextControlDropsTotal = try ToriiConnectJSON.requireUInt64(raw, key: "plaintext_control_drops_total", field: "plaintext_control_drops_total")
        monotonicDropsTotal = try ToriiConnectJSON.requireUInt64(raw, key: "monotonic_drops_total", field: "monotonic_drops_total")
        pingMissTotal = try ToriiConnectJSON.requireUInt64(raw, key: "ping_miss_total", field: "ping_miss_total")
        let perIpRaw = try ToriiConnectJSON.objectsArray(raw,
                                                         key: "per_ip_sessions",
                                                         field: "per_ip_sessions")
        perIpSessions = try perIpRaw.map { try ToriiConnectPerIpSessions(raw: $0) }
        if let policyObject = ToriiConnectJSON.optionalObject(raw, key: "policy") {
            policy = ToriiConnectStatusPolicySnapshot(raw: policyObject)
        } else {
            policy = nil
        }
    }

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode([String: ToriiJSONValue].self)
        try self.init(raw: raw)
    }
}

public struct ToriiConnectSessionResponse: Decodable, Sendable, Equatable {
    public let sid: String
    public let walletURI: String
    public let appURI: String
    public let tokenApp: String
    public let tokenWallet: String
    public let extra: [String: ToriiJSONValue]
    public let raw: [String: ToriiJSONValue]

    public init(raw: [String: ToriiJSONValue]) throws {
        self.raw = raw
        sid = try ToriiConnectJSON.requireString(raw, key: "sid", field: "sid")
        walletURI = try ToriiConnectJSON.requireString(raw, key: "wallet_uri", field: "wallet_uri")
        appURI = try ToriiConnectJSON.requireString(raw, key: "app_uri", field: "app_uri")
        tokenApp = try ToriiConnectJSON.requireString(raw, key: "token_app", field: "token_app")
        tokenWallet = try ToriiConnectJSON.requireString(raw, key: "token_wallet", field: "token_wallet")
        let known: Set<String> = [
            "sid",
            "wallet_uri",
            "app_uri",
            "token_app",
            "token_wallet"
        ]
        extra = ToriiConnectJSON.mergeExtra(record: raw, knownKeys: known)
    }

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode([String: ToriiJSONValue].self)
        try self.init(raw: raw)
    }
}

public struct ToriiConnectAppRecord: Decodable, Sendable, Equatable {
    public let appId: String
    public let displayName: String?
    public let description: String?
    public let iconURL: String?
    public let namespaces: [String]
    public let metadata: [String: ToriiJSONValue]
    public let policy: [String: ToriiJSONValue]
    public let extra: [String: ToriiJSONValue]
    public let raw: [String: ToriiJSONValue]

    public init(raw: [String: ToriiJSONValue]) throws {
        self.raw = raw
        appId = try ToriiConnectJSON.requireString(raw, key: "app_id", field: "app_id")
        displayName = ToriiConnectJSON.optionalString(raw, key: "display_name")
        description = ToriiConnectJSON.optionalString(raw, key: "description")
        iconURL = ToriiConnectJSON.optionalString(raw, key: "icon_url")
        if let namespaceValue = raw["namespaces"] {
            namespaces = try ToriiConnectJSON.stringArray(namespaceValue, field: "namespaces")
        } else {
            namespaces = []
        }
        if let metadataValue = raw["metadata"], case .object(let object) = metadataValue {
            metadata = object
        } else {
            metadata = [:]
        }
        if let policyValue = raw["policy"], case .object(let object) = policyValue {
            policy = object
        } else {
            policy = [:]
        }
        let known: Set<String> = [
            "app_id",
            "display_name",
            "description",
            "icon_url",
            "namespaces",
            "metadata",
            "policy"
        ]
        extra = ToriiConnectJSON.mergeExtra(record: raw, knownKeys: known)
    }

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode([String: ToriiJSONValue].self)
        try self.init(raw: raw)
    }
}

public struct ToriiConnectAppRegistryPage: Decodable, Sendable, Equatable {
    public let items: [ToriiConnectAppRecord]
    public let total: UInt64?
    public let nextCursor: String?
    public let extra: [String: ToriiJSONValue]
    public let raw: [String: ToriiJSONValue]

    public init(raw: [String: ToriiJSONValue]) throws {
        self.raw = raw
        let itemsArray = try ToriiConnectJSON.objectsArray(raw, key: "items", field: "items")
        items = try itemsArray.map { try ToriiConnectAppRecord(raw: $0) }
        total = ToriiConnectJSON.optionalUInt64(raw, key: "total")
        nextCursor = ToriiConnectJSON.optionalString(raw, key: "next_cursor")
        let known: Set<String> = ["items", "total", "next_cursor"]
        extra = ToriiConnectJSON.mergeExtra(record: raw, knownKeys: known)
    }

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode([String: ToriiJSONValue].self)
        try self.init(raw: raw)
    }
}

public struct ToriiConnectAppPolicyControls: Decodable, Sendable, Equatable {
    public let relayEnabled: Bool?
    public let wsMaxSessions: UInt64?
    public let wsPerIpMaxSessions: UInt64?
    public let wsRatePerIpPerMin: UInt64?
    public let sessionTtlMs: UInt64?
    public let frameMaxBytes: UInt64?
    public let sessionBufferMaxBytes: UInt64?
    public let pingIntervalMs: UInt64?
    public let pingMissTolerance: UInt64?
    public let pingMinIntervalMs: UInt64?
    public let extra: [String: ToriiJSONValue]
    public let raw: [String: ToriiJSONValue]

    public init(raw: [String: ToriiJSONValue]) {
        if let policy = ToriiConnectJSON.optionalObject(raw, key: "policy") {
            self.raw = policy
        } else {
            self.raw = raw
        }
        relayEnabled = ToriiConnectJSON.optionalBool(self.raw, key: "relay_enabled")
        wsMaxSessions = ToriiConnectJSON.optionalUInt64(self.raw, key: "ws_max_sessions")
        wsPerIpMaxSessions = ToriiConnectJSON.optionalUInt64(self.raw, key: "ws_per_ip_max_sessions")
        wsRatePerIpPerMin = ToriiConnectJSON.optionalUInt64(self.raw, key: "ws_rate_per_ip_per_min")
        sessionTtlMs = ToriiConnectJSON.optionalUInt64(self.raw, key: "session_ttl_ms")
        frameMaxBytes = ToriiConnectJSON.optionalUInt64(self.raw, key: "frame_max_bytes")
        sessionBufferMaxBytes = ToriiConnectJSON.optionalUInt64(self.raw, key: "session_buffer_max_bytes")
        pingIntervalMs = ToriiConnectJSON.optionalUInt64(self.raw, key: "ping_interval_ms")
        pingMissTolerance = ToriiConnectJSON.optionalUInt64(self.raw, key: "ping_miss_tolerance")
        pingMinIntervalMs = ToriiConnectJSON.optionalUInt64(self.raw, key: "ping_min_interval_ms")
        let known: Set<String> = [
            "relay_enabled",
            "ws_max_sessions",
            "ws_per_ip_max_sessions",
            "ws_rate_per_ip_per_min",
            "session_ttl_ms",
            "frame_max_bytes",
            "session_buffer_max_bytes",
            "ping_interval_ms",
            "ping_miss_tolerance",
            "ping_min_interval_ms"
        ]
        extra = ToriiConnectJSON.mergeExtra(record: self.raw, knownKeys: known)
    }

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode([String: ToriiJSONValue].self)
        self.init(raw: raw)
    }
}

public struct ToriiConnectAdmissionManifestEntry: Decodable, Sendable, Equatable {
    public let appId: String
    public let namespaces: [String]
    public let metadata: [String: ToriiJSONValue]
    public let policy: [String: ToriiJSONValue]
    public let extra: [String: ToriiJSONValue]
    public let raw: [String: ToriiJSONValue]

    public init(raw: [String: ToriiJSONValue]) throws {
        self.raw = raw
        appId = try ToriiConnectJSON.requireString(raw, key: "app_id", field: "app_id")
        if let namespaceValue = raw["namespaces"] {
            namespaces = try ToriiConnectJSON.stringArray(namespaceValue, field: "namespaces")
        } else {
            namespaces = []
        }
        if let metadataValue = raw["metadata"], case .object(let object) = metadataValue {
            metadata = object
        } else {
            metadata = [:]
        }
        if let policyValue = raw["policy"], case .object(let object) = policyValue {
            policy = object
        } else {
            policy = [:]
        }
        let known: Set<String> = ["app_id", "namespaces", "metadata", "policy"]
        extra = ToriiConnectJSON.mergeExtra(record: raw, knownKeys: known)
    }

    public init(appId: String,
                namespaces: [String] = [],
                metadata: [String: ToriiJSONValue] = [:],
                policy: [String: ToriiJSONValue] = [:],
                extra: [String: ToriiJSONValue] = [:]) throws {
        let trimmedAppId = try ToriiConnectJSON.trimmedNonEmpty(appId, field: "appId")
        let normalizedNamespaces = try namespaces.map {
            try ToriiConnectJSON.trimmedNonEmpty($0, field: "namespaces")
        }
        var manifest: [String: ToriiJSONValue] = [
            "app_id": .string(trimmedAppId),
            "namespaces": .array(normalizedNamespaces.map { .string($0) }),
            "metadata": .object(metadata),
            "policy": .object(policy)
        ]
        for (key, value) in extra {
            manifest[key] = value
        }
        self.raw = manifest
        self.appId = trimmedAppId
        self.namespaces = normalizedNamespaces
        self.metadata = metadata
        self.policy = policy
        self.extra = extra
    }

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode([String: ToriiJSONValue].self)
        try self.init(raw: raw)
    }
}

public struct ToriiConnectAdmissionManifest: Decodable, Sendable, Equatable {
    public let version: Int?
    public let entries: [ToriiConnectAdmissionManifestEntry]
    public let manifestHash: String?
    public let updatedAt: String?
    public let extra: [String: ToriiJSONValue]
    public let raw: [String: ToriiJSONValue]

    public init(raw: [String: ToriiJSONValue]) throws {
        let manifest = ToriiConnectJSON.optionalObject(raw, key: "manifest") ?? raw
        self.raw = manifest
        version = ToriiConnectJSON.optionalInt(manifest, key: "version")
        manifestHash = ToriiConnectJSON.optionalString(manifest, key: "manifest_hash")
        updatedAt = ToriiConnectJSON.optionalString(manifest, key: "updated_at")
        let entriesRaw = try ToriiConnectJSON.objectsArray(manifest, key: "entries", field: "entries")
        entries = try entriesRaw.map { try ToriiConnectAdmissionManifestEntry(raw: $0) }
        let known: Set<String> = ["entries", "version", "manifest_hash", "updated_at"]
        extra = ToriiConnectJSON.mergeExtra(record: manifest, knownKeys: known)
    }

    public init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode([String: ToriiJSONValue].self)
        try self.init(raw: raw)
    }
}

public struct ToriiConnectAppUpsertInput: Sendable, Equatable {
    public var appId: String
    public var displayName: String?
    public var description: String?
    public var iconURL: String?
    public var namespaces: [String]
    public var metadata: [String: ToriiJSONValue]
    public var policy: [String: ToriiJSONValue]
    public var extra: [String: ToriiJSONValue]

    public init(appId: String,
                displayName: String? = nil,
                description: String? = nil,
                iconURL: String? = nil,
                namespaces: [String] = [],
                metadata: [String: ToriiJSONValue] = [:],
                policy: [String: ToriiJSONValue] = [:],
                extra: [String: ToriiJSONValue] = [:]) {
        self.appId = appId
        self.displayName = displayName
        self.description = description
        self.iconURL = iconURL
        self.namespaces = namespaces
        self.metadata = metadata
        self.policy = policy
        self.extra = extra
    }

    fileprivate func payload() throws -> [String: ToriiJSONValue] {
        var record: [String: ToriiJSONValue] = [:]
        record["app_id"] = .string(try ToriiConnectJSON.trimmedNonEmpty(appId, field: "appId"))
        record["namespaces"] = .array(try namespaces.map {
            let trimmed = try ToriiConnectJSON.trimmedNonEmpty($0, field: "namespaces")
            return .string(trimmed)
        })
        record["metadata"] = .object(metadata)
        record["policy"] = .object(policy)
        if let displayName, !displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            record["display_name"] = .string(displayName)
        }
        if let description, !description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            record["description"] = .string(description)
        }
        if let iconURL, !iconURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            record["icon_url"] = .string(iconURL)
        }
        for (key, value) in extra {
            record[key] = value
        }
        return record
    }
}

public struct ToriiConnectAppPolicyUpdate: Sendable, Equatable {
    public var relayEnabled: Bool?
    public var wsMaxSessions: UInt64?
    public var wsPerIpMaxSessions: UInt64?
    public var wsRatePerIpPerMin: UInt64?
    public var sessionTtlMs: UInt64?
    public var frameMaxBytes: UInt64?
    public var sessionBufferMaxBytes: UInt64?
    public var pingIntervalMs: UInt64?
    public var pingMissTolerance: UInt64?
    public var pingMinIntervalMs: UInt64?
    public var extra: [String: ToriiJSONValue]

    public init(relayEnabled: Bool? = nil,
                wsMaxSessions: UInt64? = nil,
                wsPerIpMaxSessions: UInt64? = nil,
                wsRatePerIpPerMin: UInt64? = nil,
                sessionTtlMs: UInt64? = nil,
                frameMaxBytes: UInt64? = nil,
                sessionBufferMaxBytes: UInt64? = nil,
                pingIntervalMs: UInt64? = nil,
                pingMissTolerance: UInt64? = nil,
                pingMinIntervalMs: UInt64? = nil,
                extra: [String: ToriiJSONValue] = [:]) {
        self.relayEnabled = relayEnabled
        self.wsMaxSessions = wsMaxSessions
        self.wsPerIpMaxSessions = wsPerIpMaxSessions
        self.wsRatePerIpPerMin = wsRatePerIpPerMin
        self.sessionTtlMs = sessionTtlMs
        self.frameMaxBytes = frameMaxBytes
        self.sessionBufferMaxBytes = sessionBufferMaxBytes
        self.pingIntervalMs = pingIntervalMs
        self.pingMissTolerance = pingMissTolerance
        self.pingMinIntervalMs = pingMinIntervalMs
        self.extra = extra
    }

    fileprivate func payload() -> [String: ToriiJSONValue] {
        var record: [String: ToriiJSONValue] = extra
        if let relayEnabled { record["relay_enabled"] = .bool(relayEnabled) }
        if let wsMaxSessions { record["ws_max_sessions"] = .number(Double(wsMaxSessions)) }
        if let wsPerIpMaxSessions { record["ws_per_ip_max_sessions"] = .number(Double(wsPerIpMaxSessions)) }
        if let wsRatePerIpPerMin { record["ws_rate_per_ip_per_min"] = .number(Double(wsRatePerIpPerMin)) }
        if let sessionTtlMs { record["session_ttl_ms"] = .number(Double(sessionTtlMs)) }
        if let frameMaxBytes { record["frame_max_bytes"] = .number(Double(frameMaxBytes)) }
        if let sessionBufferMaxBytes { record["session_buffer_max_bytes"] = .number(Double(sessionBufferMaxBytes)) }
        if let pingIntervalMs { record["ping_interval_ms"] = .number(Double(pingIntervalMs)) }
        if let pingMissTolerance { record["ping_miss_tolerance"] = .number(Double(pingMissTolerance)) }
        if let pingMinIntervalMs { record["ping_min_interval_ms"] = .number(Double(pingMinIntervalMs)) }
        return record
    }
}

public struct ToriiConnectAdmissionManifestInput: Sendable, Equatable {
    public var version: Int?
    public var entries: [ToriiConnectAdmissionManifestEntry]
    public var manifestHash: String?
    public var updatedAt: String?
    public var extra: [String: ToriiJSONValue]

    public init(version: Int? = nil,
                entries: [ToriiConnectAdmissionManifestEntry],
                manifestHash: String? = nil,
                updatedAt: String? = nil,
                extra: [String: ToriiJSONValue] = [:]) {
        self.version = version
        self.entries = entries
        self.manifestHash = manifestHash
        self.updatedAt = updatedAt
        self.extra = extra
    }

    fileprivate func payload() throws -> [String: ToriiJSONValue] {
        var manifest: [String: ToriiJSONValue] = extra
        manifest["entries"] = .array(try entries.map { entry in
            let normalizedNamespaces = try entry.namespaces.map {
                try ToriiConnectJSON.trimmedNonEmpty($0, field: "namespaces")
            }
            var object: [String: ToriiJSONValue] = [
                "app_id": .string(entry.appId),
                "namespaces": .array(normalizedNamespaces.map { .string($0) }),
                "metadata": .object(entry.metadata),
                "policy": .object(entry.policy)
            ]
            for (key, value) in entry.extra {
                object[key] = value
            }
            return .object(object)
        })
        if let version {
            manifest["version"] = .number(Double(version))
        }
        if let manifestHash, !manifestHash.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            manifest["manifest_hash"] = .string(manifestHash)
        }
        if let updatedAt, !updatedAt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            manifest["updated_at"] = .string(updatedAt)
        }
        return manifest
    }
}

public struct ToriiConnectAppListOptions: Sendable, Equatable {
    public var limit: Int?
    public var cursor: String?

    public init(limit: Int? = nil, cursor: String? = nil) {
        self.limit = limit
        self.cursor = cursor
    }

    public func queryItems() throws -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let limit {
            guard limit > 0 else {
                throw ToriiClientError.invalidPayload("limit must be positive")
            }
            items.append(URLQueryItem(name: "limit", value: String(limit)))
        }
        if let cursor, !cursor.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            items.append(URLQueryItem(name: "cursor", value: cursor))
        }
        return items.isEmpty ? nil : items
    }
}

public typealias ToriiSubscriptionPlan = [String: ToriiJSONValue]
public typealias ToriiSubscriptionState = [String: ToriiJSONValue]
public typealias ToriiSubscriptionInvoice = [String: ToriiJSONValue]

public enum ToriiSubscriptionStatus: String, Codable, Sendable, CaseIterable {
    case active
    case paused
    case pastDue = "past_due"
    case canceled
    case suspended
}

public enum ToriiSubscriptionCancelMode: String, Codable, Sendable, CaseIterable {
    case immediate
    case periodEnd = "period_end"
}

public struct ToriiSubscriptionPlanCreateRequest: Encodable, Sendable {
    public var authority: String
    public var privateKey: String
    public var planId: String
    public var plan: ToriiSubscriptionPlan

    public init(authority: String,
                privateKey: String,
                planId: String,
                plan: ToriiSubscriptionPlan) {
        self.authority = authority
        self.privateKey = privateKey
        self.planId = planId
        self.plan = plan
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case privateKey = "private_key"
        case planId = "plan_id"
        case plan
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedAuthority = try ToriiRequestValidation.normalizedNonEmpty(authority,
                                                                                field: "authority")
        let normalizedPrivateKey = try ToriiRequestValidation.normalizedNonEmpty(privateKey,
                                                                                  field: "private_key")
        let normalizedPlanId = try ToriiRequestValidation.normalizedNonEmpty(planId,
                                                                             field: "plan_id")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedAuthority, forKey: .authority)
        try container.encode(normalizedPrivateKey, forKey: .privateKey)
        try container.encode(normalizedPlanId, forKey: .planId)
        try container.encode(plan, forKey: .plan)
    }
}

public struct ToriiSubscriptionPlanCreateResponse: Decodable, Sendable {
    public let ok: Bool
    public let planId: String
    public let txHashHex: String

    private enum CodingKeys: String, CodingKey {
        case ok
        case planId = "plan_id"
        case txHashHex = "tx_hash_hex"
    }
}

public struct ToriiSubscriptionPlanListParams: Sendable, Equatable {
    public var provider: String?
    public var limit: UInt64?
    public var offset: UInt64?

    public init(provider: String? = nil,
                limit: UInt64? = nil,
                offset: UInt64? = nil) {
        self.provider = provider
        self.limit = limit
        self.offset = offset
    }

    public func queryItems() throws -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let provider = provider?.trimmingCharacters(in: .whitespacesAndNewlines),
           !provider.isEmpty {
            items.append(URLQueryItem(name: "provider", value: provider))
        }
        if let limit {
            items.append(URLQueryItem(name: "limit", value: String(limit)))
        }
        if let offset {
            items.append(URLQueryItem(name: "offset", value: String(offset)))
        }
        return items.isEmpty ? nil : items
    }
}

public struct ToriiSubscriptionPlanListItem: Decodable, Sendable {
    public let planId: String
    public let plan: ToriiSubscriptionPlan

    private enum CodingKeys: String, CodingKey {
        case planId = "plan_id"
        case plan
    }
}

public struct ToriiSubscriptionPlanListResponse: Decodable, Sendable {
    public let items: [ToriiSubscriptionPlanListItem]
    public let total: UInt64
}

public struct ToriiSubscriptionCreateRequest: Encodable, Sendable {
    public var authority: String
    public var privateKey: String
    public var subscriptionId: String
    public var planId: String
    public var billingTriggerId: String?
    public var usageTriggerId: String?
    public var firstChargeMs: UInt64?
    public var grantUsageToProvider: Bool?

    public init(authority: String,
                privateKey: String,
                subscriptionId: String,
                planId: String,
                billingTriggerId: String? = nil,
                usageTriggerId: String? = nil,
                firstChargeMs: UInt64? = nil,
                grantUsageToProvider: Bool? = nil) {
        self.authority = authority
        self.privateKey = privateKey
        self.subscriptionId = subscriptionId
        self.planId = planId
        self.billingTriggerId = billingTriggerId
        self.usageTriggerId = usageTriggerId
        self.firstChargeMs = firstChargeMs
        self.grantUsageToProvider = grantUsageToProvider
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case privateKey = "private_key"
        case subscriptionId = "subscription_id"
        case planId = "plan_id"
        case billingTriggerId = "billing_trigger_id"
        case usageTriggerId = "usage_trigger_id"
        case firstChargeMs = "first_charge_ms"
        case grantUsageToProvider = "grant_usage_to_provider"
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedAuthority = try ToriiRequestValidation.normalizedNonEmpty(authority,
                                                                                field: "authority")
        let normalizedPrivateKey = try ToriiRequestValidation.normalizedNonEmpty(privateKey,
                                                                                  field: "private_key")
        let normalizedSubscriptionId = try ToriiRequestValidation.normalizedNonEmpty(subscriptionId,
                                                                                     field: "subscription_id")
        let normalizedPlanId = try ToriiRequestValidation.normalizedNonEmpty(planId,
                                                                             field: "plan_id")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedAuthority, forKey: .authority)
        try container.encode(normalizedPrivateKey, forKey: .privateKey)
        try container.encode(normalizedSubscriptionId, forKey: .subscriptionId)
        try container.encode(normalizedPlanId, forKey: .planId)
        if let billingTriggerId = try ToriiRequestValidation.normalizedOptionalNonEmpty(billingTriggerId,
                                                                                        field: "billing_trigger_id") {
            try container.encode(billingTriggerId, forKey: .billingTriggerId)
        }
        if let usageTriggerId = try ToriiRequestValidation.normalizedOptionalNonEmpty(usageTriggerId,
                                                                                      field: "usage_trigger_id") {
            try container.encode(usageTriggerId, forKey: .usageTriggerId)
        }
        try container.encodeIfPresent(firstChargeMs, forKey: .firstChargeMs)
        try container.encodeIfPresent(grantUsageToProvider, forKey: .grantUsageToProvider)
    }
}

public struct ToriiSubscriptionCreateResponse: Decodable, Sendable {
    public let ok: Bool
    public let subscriptionId: String
    public let billingTriggerId: String
    public let usageTriggerId: String?
    public let firstChargeMs: UInt64
    public let txHashHex: String

    private enum CodingKeys: String, CodingKey {
        case ok
        case subscriptionId = "subscription_id"
        case billingTriggerId = "billing_trigger_id"
        case usageTriggerId = "usage_trigger_id"
        case firstChargeMs = "first_charge_ms"
        case txHashHex = "tx_hash_hex"
    }
}

public struct ToriiSubscriptionListParams: Sendable, Equatable {
    public var ownedBy: String?
    public var provider: String?
    public var status: ToriiSubscriptionStatus?
    public var limit: UInt64?
    public var offset: UInt64?

    public init(ownedBy: String? = nil,
                provider: String? = nil,
                status: ToriiSubscriptionStatus? = nil,
                limit: UInt64? = nil,
                offset: UInt64? = nil) {
        self.ownedBy = ownedBy
        self.provider = provider
        self.status = status
        self.limit = limit
        self.offset = offset
    }

    public func queryItems() throws -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let ownedBy = ownedBy?.trimmingCharacters(in: .whitespacesAndNewlines),
           !ownedBy.isEmpty {
            items.append(URLQueryItem(name: "owned_by", value: ownedBy))
        }
        if let provider = provider?.trimmingCharacters(in: .whitespacesAndNewlines),
           !provider.isEmpty {
            items.append(URLQueryItem(name: "provider", value: provider))
        }
        if let status {
            items.append(URLQueryItem(name: "status", value: status.rawValue))
        }
        if let limit {
            items.append(URLQueryItem(name: "limit", value: String(limit)))
        }
        if let offset {
            items.append(URLQueryItem(name: "offset", value: String(offset)))
        }
        return items.isEmpty ? nil : items
    }
}

public struct ToriiSubscriptionRecord: Decodable, Sendable {
    public let subscriptionId: String
    public let subscription: ToriiSubscriptionState
    public let invoice: ToriiSubscriptionInvoice?
    public let plan: ToriiSubscriptionPlan?

    private enum CodingKeys: String, CodingKey {
        case subscriptionId = "subscription_id"
        case subscription
        case invoice
        case plan
    }
}

public struct ToriiSubscriptionListResponse: Decodable, Sendable {
    public let items: [ToriiSubscriptionRecord]
    public let total: UInt64
}

public struct ToriiSubscriptionActionRequest: Encodable, Sendable {
    public var authority: String
    public var privateKey: String
    public var chargeAtMs: UInt64?
    public var cancelMode: ToriiSubscriptionCancelMode?

    public init(authority: String,
                privateKey: String,
                chargeAtMs: UInt64? = nil,
                cancelMode: ToriiSubscriptionCancelMode? = nil) {
        self.authority = authority
        self.privateKey = privateKey
        self.chargeAtMs = chargeAtMs
        self.cancelMode = cancelMode
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case privateKey = "private_key"
        case chargeAtMs = "charge_at_ms"
        case cancelMode = "cancel_mode"
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedAuthority = try ToriiRequestValidation.normalizedNonEmpty(authority,
                                                                                field: "authority")
        let normalizedPrivateKey = try ToriiRequestValidation.normalizedNonEmpty(privateKey,
                                                                                  field: "private_key")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedAuthority, forKey: .authority)
        try container.encode(normalizedPrivateKey, forKey: .privateKey)
        try container.encodeIfPresent(chargeAtMs, forKey: .chargeAtMs)
        try container.encodeIfPresent(cancelMode, forKey: .cancelMode)
    }
}

public struct ToriiSubscriptionUsageRequest: Encodable, Sendable {
    public var authority: String
    public var privateKey: String
    public var unitKey: String
    public var delta: String
    public var usageTriggerId: String?

    public init(authority: String,
                privateKey: String,
                unitKey: String,
                delta: String,
                usageTriggerId: String? = nil) {
        self.authority = authority
        self.privateKey = privateKey
        self.unitKey = unitKey
        self.delta = delta
        self.usageTriggerId = usageTriggerId
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case privateKey = "private_key"
        case unitKey = "unit_key"
        case delta
        case usageTriggerId = "usage_trigger_id"
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedAuthority = try ToriiRequestValidation.normalizedNonEmpty(authority,
                                                                                field: "authority")
        let normalizedPrivateKey = try ToriiRequestValidation.normalizedNonEmpty(privateKey,
                                                                                  field: "private_key")
        let normalizedUnitKey = try ToriiRequestValidation.normalizedNonEmpty(unitKey, field: "unit_key")
        let normalizedDelta = try ToriiRequestValidation.normalizedNonEmpty(delta, field: "delta")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedAuthority, forKey: .authority)
        try container.encode(normalizedPrivateKey, forKey: .privateKey)
        try container.encode(normalizedUnitKey, forKey: .unitKey)
        try container.encode(normalizedDelta, forKey: .delta)
        if let usageTriggerId = try ToriiRequestValidation.normalizedOptionalNonEmpty(usageTriggerId,
                                                                                      field: "usage_trigger_id") {
            try container.encode(usageTriggerId, forKey: .usageTriggerId)
        }
    }
}

public struct ToriiSubscriptionActionResponse: Decodable, Sendable {
    public let ok: Bool
    public let subscriptionId: String
    public let txHashHex: String

    private enum CodingKeys: String, CodingKey {
        case ok
        case subscriptionId = "subscription_id"
        case txHashHex = "tx_hash_hex"
    }
}

public struct ToriiOfflineAllowanceItem: Decodable, Sendable {
    public let certificateIdHex: String
    public let controllerId: String
    public let controllerDisplay: String
    public let assetId: String
    public let registeredAtMs: UInt64
    public let expiresAtMs: UInt64
    public let policyExpiresAtMs: UInt64
    public let refreshAtMs: UInt64?
    public let verdictIdHex: String?
    public let attestationNonceHex: String?
    public let remainingAmount: String
    public let deadlineKind: String?
    public let deadlineState: String?
    public let deadlineMs: UInt64?
    public let deadlineMsRemaining: Int64?
    public let record: ToriiJSONValue

    public func decodeRecord<T: Decodable>(as type: T.Type = T.self,
                                           decoder: JSONDecoder = JSONDecoder()) throws -> T {
        try record.decode(as: type, decoder: decoder)
    }

    public init(certificateIdHex: String,
                controllerId: String,
                controllerDisplay: String,
                assetId: String,
                registeredAtMs: UInt64,
                expiresAtMs: UInt64,
                policyExpiresAtMs: UInt64,
                refreshAtMs: UInt64?,
                verdictIdHex: String?,
                attestationNonceHex: String?,
                remainingAmount: String,
                deadlineKind: String? = nil,
                deadlineState: String? = nil,
                deadlineMs: UInt64? = nil,
                deadlineMsRemaining: Int64? = nil,
                record: ToriiJSONValue) {
        self.certificateIdHex = certificateIdHex
        self.controllerId = controllerId
        self.controllerDisplay = controllerDisplay
        self.assetId = assetId
        self.registeredAtMs = registeredAtMs
        self.expiresAtMs = expiresAtMs
        self.policyExpiresAtMs = policyExpiresAtMs
        self.refreshAtMs = refreshAtMs
        self.verdictIdHex = verdictIdHex
        self.attestationNonceHex = attestationNonceHex
        self.remainingAmount = remainingAmount
        self.deadlineKind = deadlineKind
        self.deadlineState = deadlineState
        self.deadlineMs = deadlineMs
        self.deadlineMsRemaining = deadlineMsRemaining
        self.record = record
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let rootValue = try container.decode(ToriiJSONValue.self)
        guard case let .object(object) = rootValue else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "Offline allowance entry must be a JSON object"
            )
        }

        func value(_ key: String) -> ToriiJSONValue? {
            object[key]
        }

        guard let recordValue = object["record"] else {
            throw DecodingError.keyNotFound(
                CodingKeys.record,
                .init(codingPath: decoder.codingPath, debugDescription: "missing record payload")
            )
        }

        let recordObject: [String: ToriiJSONValue]
        if case let .object(payload) = recordValue {
            recordObject = payload
        } else {
            recordObject = [:]
        }

        func string(_ key: String, field: String) throws -> String {
            if let string = ToriiOfflineAllowanceItem.normalizedString(value(key)) {
                return string
            }
            throw DecodingError.dataCorrupted(
                .init(codingPath: decoder.codingPath, debugDescription: "missing \(field)")
            )
        }

        func optionalString(_ key: String) -> String? {
            ToriiOfflineAllowanceItem.normalizedString(value(key))
        }

        func uint64(_ key: String, field: String, defaultValue: UInt64? = nil) throws -> UInt64 {
            if let number = ToriiOfflineAllowanceItem.normalizedUInt64(value(key)) {
                return number
            }
            if let fallback = defaultValue {
                return fallback
            }
            throw DecodingError.dataCorrupted(
                .init(codingPath: decoder.codingPath,
                      debugDescription: "missing \(field)")
            )
        }

        func optionalUInt64(_ key: String) -> UInt64? {
            ToriiOfflineAllowanceItem.normalizedUInt64(value(key))
        }

        func optionalInt64(_ key: String) -> Int64? {
            ToriiOfflineAllowanceItem.normalizedInt64(value(key))
        }

        func remainingAmountValue() -> String {
            if let topLevel = ToriiOfflineAllowanceItem.normalizedString(
                value("remaining_amount")
            ) {
                return topLevel
            }
            if let nested = ToriiOfflineAllowanceItem.normalizedString(
                ToriiOfflineAllowanceItem.lookup(
                    key: "remaining_amount",
                    in: recordObject
                )
            ) {
                return nested
            }
            return "0"
        }

        self.certificateIdHex = try string("certificate_id_hex", field: "certificate_id_hex")
        self.controllerId = try string("controller_id", field: "controller_id")
        self.controllerDisplay = try string("controller_display", field: "controller_display")
        self.assetId = try string("asset_id", field: "asset_id")
        self.registeredAtMs = try uint64("registered_at_ms", field: "registered_at_ms")
        let expires = try uint64(
            "expires_at_ms",
            field: "expires_at_ms",
            defaultValue: 0
        )
        self.expiresAtMs = expires
        self.policyExpiresAtMs = try uint64(
            "policy_expires_at_ms",
            field: "policy_expires_at_ms",
            defaultValue: expires
        )
        self.refreshAtMs = optionalUInt64("refresh_at_ms")
        self.verdictIdHex = optionalString("verdict_id_hex")
        self.attestationNonceHex = optionalString("attestation_nonce_hex")
        self.remainingAmount = remainingAmountValue()
        self.deadlineKind = optionalString("deadline_kind")
        self.deadlineState = optionalString("deadline_state")
        self.deadlineMs = optionalUInt64("deadline_ms")
        self.deadlineMsRemaining = optionalInt64("deadline_ms_remaining")
        self.record = recordValue
    }

    private static func lookup(key: String,
                               in object: [String: ToriiJSONValue]) -> ToriiJSONValue? {
        object[key]
    }

    private static func normalizedString(_ value: ToriiJSONValue?) -> String? {
        guard let value else { return nil }
        return value.normalizedString
    }

    private static func normalizedUInt64(_ value: ToriiJSONValue?) -> UInt64? {
        guard let value else { return nil }
        return value.normalizedUInt64
    }

    private static func normalizedInt64(_ value: ToriiJSONValue?) -> Int64? {
        guard let value else { return nil }
        return value.normalizedInt64
    }

    private enum CodingKeys: String, CodingKey {
        case record
    }
}

public struct ToriiOfflineAllowanceList: Decodable, Sendable {
    public let items: [ToriiOfflineAllowanceItem]
    public let total: UInt64
}

public struct ToriiOfflineSummaryItem: Decodable, Sendable {
    public let certificateIdHex: String
    public let controllerId: String
    public let controllerDisplay: String
    public let summaryHashHex: String
    public let appleKeyCounters: [String: UInt64]
    public let androidSeriesCounters: [String: UInt64]

    private enum CodingKeys: String, CodingKey {
        case certificateIdHex = "certificate_id_hex"
        case controllerId = "controller_id"
        case controllerDisplay = "controller_display"
        case summaryHashHex = "summary_hash_hex"
        case appleKeyCounters = "apple_key_counters"
        case androidSeriesCounters = "android_series_counters"
    }
}

public struct ToriiOfflineSummaryList: Decodable, Sendable {
    public let items: [ToriiOfflineSummaryItem]
    public let total: UInt64
}

public struct ToriiOfflineVerdictRevocation: Decodable, Sendable {
    public let verdictIdHex: String
    public let issuerId: String
    public let issuerDisplay: String
    public let revokedAtMs: UInt64
    public let reason: String
    public let note: String?
    public let metadata: ToriiJSONValue?
    public let record: ToriiJSONValue

    private enum CodingKeys: String, CodingKey {
        case verdictIdHex = "verdict_id_hex"
        case issuerId = "issuer_id"
        case issuerDisplay = "issuer_display"
        case revokedAtMs = "revoked_at_ms"
        case reason
        case note
        case metadata
        case record
    }

    public func decodeRecord<T: Decodable>(as type: T.Type = T.self,
                                           decoder: JSONDecoder = JSONDecoder()) throws -> T {
        try record.decode(as: type, decoder: decoder)
    }
}

public struct ToriiOfflineRevocationList: Decodable, Sendable {
    public let items: [ToriiOfflineVerdictRevocation]
    public let total: UInt64
}

/// Lightweight proof status response for an offline bundle.
public struct ToriiOfflineBundleProofStatus: Decodable, Sendable, Equatable {
    /// Proof status label returned by Torii (e.g., `fresh`, `match`, `expired`).
    public let proofStatus: String?
    /// Receipts root advertised by the bundle.
    public let receiptsRootHex: String?
    /// Aggregate proof root if present in the bundle or admission pipeline.
    public let aggregateProofRootHex: String?
    /// Indicates whether Torii computed the same receipts root as the bundle reported.
    public let receiptsRootMatches: Bool?
    /// Optional summary payload returned by Torii.
    public let proofSummary: ToriiJSONValue?
    /// Full payload for forward compatibility.
    public let fields: [String: ToriiJSONValue]

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let raw = try container.decode([String: ToriiJSONValue].self)
        fields = raw
        proofStatus = Self.normalizedString(raw["proof_status"])
        receiptsRootHex = Self.normalizedString(raw["receipts_root_hex"])
        aggregateProofRootHex = Self.normalizedString(raw["aggregate_proof_root_hex"])
        receiptsRootMatches = Self.normalizedBool(raw["receipts_root_matches"])
        proofSummary = raw["proof_summary"]
    }

    /// Attempt to decode the proof summary into the typed view if present.
    public func decodeProofSummary() throws -> ToriiOfflineBundleProofSummary? {
        guard let proofSummary else { return nil }
        return try proofSummary.decode(as: ToriiOfflineBundleProofSummary.self)
    }

    /// Access an arbitrary raw field by name.
    public subscript(field name: String) -> ToriiJSONValue? {
        fields[name]
    }

    private static func normalizedString(_ value: ToriiJSONValue?) -> String? {
        guard let value else { return nil }
        return value.normalizedString
    }

    private static func normalizedBool(_ value: ToriiJSONValue?) -> Bool? {
        guard let value else { return nil }
        switch value {
        case .bool(let flag):
            return flag
        case .string(let string):
            let lowered = string.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if lowered == "true" { return true }
            if lowered == "false" { return false }
            return nil
        default:
            return nil
        }
    }
}

/// Typed view of the optional proof summary payload returned by `/v1/offline/bundle/proof_status`.
public struct ToriiOfflineBundleProofSummary: Decodable, Sendable, Equatable {
    public let version: UInt16
    public let proofSumBytes: UInt64?
    public let proofCounterBytes: UInt64?
    public let proofReplayBytes: UInt64?
    public let metadataKeys: [String]?

    private enum CodingKeys: String, CodingKey {
        case version
        case proofSumBytes = "proof_sum_bytes"
        case proofCounterBytes = "proof_counter_bytes"
        case proofReplayBytes = "proof_replay_bytes"
        case metadataKeys = "metadata_keys"
    }
}

public struct ToriiOfflineCertificateIssueRequest: Encodable, Sendable {
    public let certificate: ToriiJSONValue

    public init(certificate: ToriiJSONValue) {
        self.certificate = certificate
    }
}

public struct ToriiOfflineCertificateIssueResponse: Decodable, Sendable, Equatable {
    public let certificateIdHex: String
    public let certificate: ToriiJSONValue

    private enum CodingKeys: String, CodingKey {
        case certificateIdHex = "certificate_id_hex"
        case certificate
    }

    public func decodeCertificate() throws -> OfflineWalletCertificate {
        try OfflineWalletCertificate(toriiValue: certificate)
    }
}

public struct ToriiOfflineAllowanceRegisterRequest: Encodable, Sendable {
    public let authority: String
    public let privateKey: String
    public let certificate: ToriiJSONValue

    private enum CodingKeys: String, CodingKey {
        case authority
        case privateKey = "private_key"
        case certificate
    }

    public init(authority: String, privateKey: String, certificate: ToriiJSONValue) {
        self.authority = authority
        self.privateKey = privateKey
        self.certificate = certificate
    }
}

public struct ToriiOfflineAllowanceRegisterResponse: Decodable, Sendable, Equatable {
    public let certificateIdHex: String

    private enum CodingKeys: String, CodingKey {
        case certificateIdHex = "certificate_id_hex"
    }
}

/// Aggregated result for the offline top-up helper (issue + register/renew).
public struct ToriiOfflineTopUpResponse: Sendable, Equatable {
    public let certificate: ToriiOfflineCertificateIssueResponse
    public let registration: ToriiOfflineAllowanceRegisterResponse

    public init(certificate: ToriiOfflineCertificateIssueResponse,
                registration: ToriiOfflineAllowanceRegisterResponse) {
        self.certificate = certificate
        self.registration = registration
    }
}

public struct ToriiOfflineSpendReceiptsSubmitRequest: Encodable, Sendable {
    public let receipts: [ToriiJSONValue]

    public init(receipts: [ToriiJSONValue]) {
        self.receipts = receipts
    }
}

public struct ToriiOfflineSpendReceiptsSubmitResponse: Decodable, Sendable, Equatable {
    public let receiptsRootHex: String
    public let receiptCount: UInt64
    public let totalAmount: String
    public let assetId: String?

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let raw = try container.decode([String: ToriiJSONValue].self)
        guard let root = raw["receipts_root_hex"]?.normalizedString else {
            throw DecodingError.dataCorruptedError(in: container,
                                                   debugDescription: "missing receipts_root_hex")
        }
        guard let count = raw["receipt_count"]?.normalizedUInt64 else {
            throw DecodingError.dataCorruptedError(in: container,
                                                   debugDescription: "missing receipt_count")
        }
        guard let total = raw["total_amount"]?.normalizedString else {
            throw DecodingError.dataCorruptedError(in: container,
                                                   debugDescription: "missing total_amount")
        }
        self.receiptsRootHex = root
        self.receiptCount = count
        self.totalAmount = total
        self.assetId = raw["asset_id"]?.normalizedString
    }
}

public struct ToriiOfflineSettlementSubmitRequest: Encodable, Sendable {
    public let authority: String
    public let privateKey: String
    public let transfer: ToriiJSONValue

    private enum CodingKeys: String, CodingKey {
        case authority
        case privateKey = "private_key"
        case transfer
    }

    public init(authority: String, privateKey: String, transfer: ToriiJSONValue) {
        self.authority = authority
        self.privateKey = privateKey
        self.transfer = transfer
    }
}

public struct ToriiOfflineSettlementSubmitResponse: Decodable, Sendable, Equatable {
    public let bundleIdHex: String

    private enum CodingKeys: String, CodingKey {
        case bundleIdHex = "bundle_id_hex"
    }
}

public struct ToriiOfflineTransferProofRequest: Encodable, Sendable {
    public let transfer: ToriiJSONValue
    public let kind: String
    public let counterCheckpoint: UInt64?
    public let replayLogHeadHex: String?
    public let replayLogTailHex: String?

    private enum CodingKeys: String, CodingKey {
        case transfer
        case kind
        case counterCheckpoint = "counter_checkpoint"
        case replayLogHeadHex = "replay_log_head_hex"
        case replayLogTailHex = "replay_log_tail_hex"
    }

    public init(transfer: ToriiJSONValue,
                kind: String,
                counterCheckpoint: UInt64? = nil,
                replayLogHeadHex: String? = nil,
                replayLogTailHex: String? = nil) {
        self.transfer = transfer
        self.kind = kind
        self.counterCheckpoint = counterCheckpoint
        self.replayLogHeadHex = replayLogHeadHex
        self.replayLogTailHex = replayLogTailHex
    }

    public init(transfer: OfflineToOnlineTransfer,
                kind: String,
                counterCheckpoint: UInt64? = nil,
                replayLogHeadHex: String? = nil,
                replayLogTailHex: String? = nil) throws {
        self.init(transfer: try transfer.toriiJSON(),
                  kind: kind,
                  counterCheckpoint: counterCheckpoint,
                  replayLogHeadHex: replayLogHeadHex,
                  replayLogTailHex: replayLogTailHex)
    }
}

public struct ToriiOfflineTransferItem: Codable, Sendable {
    public let bundleIdHex: String
    public let controllerId: String
    public let controllerDisplay: String
    public let receiverId: String
    public let receiverDisplay: String
    public let depositAccountId: String
    public let depositAccountDisplay: String
    public let assetId: String?
    public let receiptCount: UInt64
    public let totalAmount: String
    public let claimedDelta: String
    public let status: String
    public let recordedAtMs: UInt64
    public let recordedAtHeight: UInt64
    public let archivedAtHeight: UInt64?
    public let certificateIdHex: String?
    public let certificateExpiresAtMs: UInt64?
    public let policyExpiresAtMs: UInt64?
    public let refreshAtMs: UInt64?
    public let verdictIdHex: String?
    public let attestationNonceHex: String?
    public let platformPolicy: ToriiPlatformPolicy?
    public let platformTokenSnapshot: ToriiOfflinePlatformTokenSnapshot?
    public let transfer: ToriiJSONValue

    private enum CodingKeys: String, CodingKey {
        case bundleIdHex = "bundle_id_hex"
        case controllerId = "controller_id"
        case controllerDisplay = "controller_display"
        case receiverId = "receiver_id"
        case receiverDisplay = "receiver_display"
        case depositAccountId = "deposit_account_id"
        case depositAccountDisplay = "deposit_account_display"
        case assetId = "asset_id"
        case receiptCount = "receipt_count"
        case totalAmount = "total_amount"
        case claimedDelta = "claimed_delta"
        case status
        case recordedAtMs = "recorded_at_ms"
        case recordedAtHeight = "recorded_at_height"
        case archivedAtHeight = "archived_at_height"
        case certificateIdHex = "certificate_id_hex"
        case certificateExpiresAtMs = "certificate_expires_at_ms"
        case policyExpiresAtMs = "policy_expires_at_ms"
        case refreshAtMs = "refresh_at_ms"
        case verdictIdHex = "verdict_id_hex"
        case attestationNonceHex = "attestation_nonce_hex"
        case platformPolicy = "platform_policy"
        case platformTokenSnapshot = "platform_token_snapshot"
        case transfer
    }

    public init(bundleIdHex: String,
                controllerId: String,
                controllerDisplay: String,
                receiverId: String,
                receiverDisplay: String,
                depositAccountId: String,
                depositAccountDisplay: String,
                assetId: String?,
                receiptCount: UInt64,
                totalAmount: String,
                claimedDelta: String,
                status: String,
                recordedAtMs: UInt64,
                recordedAtHeight: UInt64,
                archivedAtHeight: UInt64?,
                certificateIdHex: String?,
                certificateExpiresAtMs: UInt64?,
                policyExpiresAtMs: UInt64?,
                refreshAtMs: UInt64?,
                verdictIdHex: String?,
                attestationNonceHex: String?,
                platformPolicy: ToriiPlatformPolicy?,
                platformTokenSnapshot: ToriiOfflinePlatformTokenSnapshot?,
                transfer: ToriiJSONValue) {
        self.bundleIdHex = bundleIdHex
        self.controllerId = controllerId
        self.controllerDisplay = controllerDisplay
        self.receiverId = receiverId
        self.receiverDisplay = receiverDisplay
        self.depositAccountId = depositAccountId
        self.depositAccountDisplay = depositAccountDisplay
        self.assetId = assetId
        self.receiptCount = receiptCount
        self.totalAmount = totalAmount
        self.claimedDelta = claimedDelta
        self.status = status
        self.recordedAtMs = recordedAtMs
        self.recordedAtHeight = recordedAtHeight
        self.archivedAtHeight = archivedAtHeight
        self.certificateIdHex = certificateIdHex
        self.certificateExpiresAtMs = certificateExpiresAtMs
        self.policyExpiresAtMs = policyExpiresAtMs
        self.refreshAtMs = refreshAtMs
        self.verdictIdHex = verdictIdHex
        self.attestationNonceHex = attestationNonceHex
        self.platformPolicy = platformPolicy
        self.platformTokenSnapshot = platformTokenSnapshot
        self.transfer = transfer
    }

    public func decodeTransfer<T: Decodable>(as type: T.Type = T.self,
                                             decoder: JSONDecoder = JSONDecoder()) throws -> T {
        try transfer.decode(as: type, decoder: decoder)
    }
}

public struct ToriiOfflineTransferList: Decodable, Sendable {
    public let items: [ToriiOfflineTransferItem]
    public let total: UInt64
}

public struct ToriiOfflineReceiptListItem: Decodable, Sendable, Equatable {
    public let bundleIdHex: String
    public let txIdHex: String
    public let certificateIdHex: String
    public let controllerId: String
    public let controllerDisplay: String
    public let receiverId: String
    public let receiverDisplay: String
    public let assetId: String
    public let amount: String
    public let invoiceId: String
    public let counter: UInt64
    public let recordedAtMs: UInt64
    public let recordedAtHeight: UInt64

    private enum CodingKeys: String, CodingKey {
        case bundleIdHex = "bundle_id_hex"
        case txIdHex = "tx_id_hex"
        case certificateIdHex = "certificate_id_hex"
        case controllerId = "controller_id"
        case controllerDisplay = "controller_display"
        case receiverId = "receiver_id"
        case receiverDisplay = "receiver_display"
        case assetId = "asset_id"
        case amount
        case invoiceId = "invoice_id"
        case counter
        case recordedAtMs = "recorded_at_ms"
        case recordedAtHeight = "recorded_at_height"
    }
}

public struct ToriiOfflineReceiptList: Decodable, Sendable, Equatable {
    public let items: [ToriiOfflineReceiptListItem]
    public let total: UInt64
}

public struct ToriiOfflineStateResponse: Decodable, Sendable, Equatable {
    public let allowances: [ToriiJSONValue]
    public let transfers: [ToriiJSONValue]
    public let summaries: [ToriiJSONValue]
    public let revocations: [ToriiJSONValue]
    public let nowMs: UInt64

    private enum CodingKeys: String, CodingKey {
        case allowances
        case transfers
        case summaries
        case revocations
        case nowMs = "now_ms"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let allowancesValue = try container.decodeIfPresent(ToriiJSONValue.self, forKey: .allowances) ?? .array([])
        let transfersValue = try container.decodeIfPresent(ToriiJSONValue.self, forKey: .transfers) ?? .array([])
        let summariesValue = try container.decodeIfPresent(ToriiJSONValue.self, forKey: .summaries) ?? .array([])
        let revocationsValue = try container.decodeIfPresent(ToriiJSONValue.self, forKey: .revocations) ?? .array([])

        guard case let .array(allowances) = allowancesValue else {
            throw DecodingError.dataCorruptedError(forKey: .allowances,
                                                   in: container,
                                                   debugDescription: "allowances must be an array")
        }
        guard case let .array(transfers) = transfersValue else {
            throw DecodingError.dataCorruptedError(forKey: .transfers,
                                                   in: container,
                                                   debugDescription: "transfers must be an array")
        }
        guard case let .array(summaries) = summariesValue else {
            throw DecodingError.dataCorruptedError(forKey: .summaries,
                                                   in: container,
                                                   debugDescription: "summaries must be an array")
        }
        guard case let .array(revocations) = revocationsValue else {
            throw DecodingError.dataCorruptedError(forKey: .revocations,
                                                   in: container,
                                                   debugDescription: "revocations must be an array")
        }

        self.allowances = allowances
        self.transfers = transfers
        self.summaries = summaries
        self.revocations = revocations
        self.nowMs = try container.decode(UInt64.self, forKey: .nowMs)
    }
}

public struct ToriiOfflinePlatformTokenSnapshot: Codable, Sendable, Equatable {
    public let policy: ToriiPlatformPolicy
    public let attestationJwsB64: String

    private enum CodingKeys: String, CodingKey {
        case policy
        case attestationJwsB64 = "attestation_jws_b64"
    }

    public init(policy: ToriiPlatformPolicy, attestationJwsB64: String) {
        self.policy = policy
        self.attestationJwsB64 = attestationJwsB64
    }
}

public struct ToriiOfflineReceiptSummary: Sendable, Equatable {
    public let senderId: String
    public let receiverId: String
    public let assetId: String?
    public let amount: String
}

public extension ToriiOfflineTransferItem {
    /// Returns a lightweight projection of the first receipt embedded in the transfer payload.
    func firstReceiptSummary() -> ToriiOfflineReceiptSummary? {
        guard case let .object(payload) = transfer else {
            return nil
        }
        guard case let .array(receipts) = payload["receipts"],
              let first = receipts.first,
              case let .object(receipt) = first else {
            return nil
        }
        guard let sender = receipt["from"]?.normalizedString,
              let receiver = receipt["to"]?.normalizedString,
              let amount = receipt["amount"]?.normalizedString else {
            return nil
        }
        let asset = receipt["asset"]?.normalizedString
        return ToriiOfflineReceiptSummary(senderId: sender,
                                          receiverId: receiver,
                                          assetId: asset,
                                          amount: amount)
    }
}

extension ToriiJSONValue {
    public var normalizedString: String? {
        switch self {
        case .string(let string):
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? nil : trimmed
        case .number(let number):
            guard number.isFinite else {
                return nil
            }
            if number.rounded(.towardZero) == number {
                guard number >= Double(Int.min), number <= Double(Int.max) else {
                    return nil
                }
                return String(Int(number))
            }
            return String(number)
        case .bool(let value):
            return value ? "true" : "false"
        case .null:
            return nil
        case .array, .object:
            return nil
        }
    }

    public var numberValue: Double? {
        switch self {
        case .number(let number):
            return number.isFinite ? number : nil
        case .string(let string):
            return Double(string.trimmingCharacters(in: .whitespacesAndNewlines))
        default:
            return nil
        }
    }

    public var normalizedUInt64: UInt64? {
        switch self {
        case .number(let number):
            guard number.isFinite, number >= 0 else {
                return nil
            }
            let rounded = number.rounded(.towardZero)
            guard rounded == number else {
                return nil
            }
            guard rounded <= Double(UInt64.max) else {
                return nil
            }
            return UInt64(rounded)
        case .string(let string):
            return UInt64(string.trimmingCharacters(in: .whitespacesAndNewlines))
        default:
            return nil
        }
    }

    public var normalizedBytes: Data? {
        switch self {
        case .array(let items):
            var bytes = Data(capacity: items.count)
            for item in items {
                guard case let .number(number) = item,
                      number.isFinite,
                      number.rounded(.towardZero) == number,
                      number >= 0,
                      number <= 255
                else {
                    return nil
                }
                bytes.append(UInt8(number))
            }
            return bytes
        case .string(let string):
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty {
                return nil
            }
            if let base64 = Data(base64Encoded: trimmed) {
                return base64
            }
            let cleaned = trimmed.lowercased().hasPrefix("0x") ? String(trimmed.dropFirst(2)) : trimmed
            guard !cleaned.isEmpty else {
                return nil
            }
            return Data(hexString: cleaned)
        default:
            return nil
        }
    }

    public var normalizedInt64: Int64? {
        switch self {
        case .number(let number):
            guard number.isFinite else {
                return nil
            }
            let rounded = number.rounded(.towardZero)
            guard rounded == number else {
                return nil
            }
            guard rounded >= Double(Int64.min), rounded <= Double(Int64.max) else {
                return nil
            }
            return Int64(rounded)
        case .string(let string):
            return Int64(string.trimmingCharacters(in: .whitespacesAndNewlines))
        default:
            return nil
        }
    }
}

public struct ToriiAttachmentMeta: Decodable, Sendable {
    public let id: String
    public let content_type: String
    public let size: UInt64
    public let created_ms: UInt64
    public let tenant: String?
}

/// Canonical DA manifest bundle returned by Torii.
public struct ToriiDaManifestBundle: Decodable, Sendable, Equatable {
    public let storageTicketHex: String
    public let clientBlobIdHex: String
    public let blobHashHex: String
    public let manifestHashHex: String
    public let chunkRootHex: String
    public let laneId: UInt64
    public let epoch: UInt64
    public let manifestLength: UInt64
    public let manifestBytes: Data
    public let manifestJson: ToriiJSONValue?
    public let chunkPlan: ToriiJSONValue
    public let samplingPlan: ToriiDaSamplingPlan?

    public var manifestIdHex: String { manifestHashHex }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let raw = try container.decode([String: ToriiJSONValue].self)
        try self.init(raw: raw)
    }

    public init(raw: [String: ToriiJSONValue]) throws {
        storageTicketHex = try Self.requireHex(raw, key: "storage_ticket", field: "storage_ticket")
        clientBlobIdHex = try Self.requireHex(raw, key: "client_blob_id", field: "client_blob_id")
        blobHashHex = try Self.requireHex(raw, key: "blob_hash", field: "blob_hash")
        manifestHashHex = try Self.requireHex(raw, key: "manifest_hash", field: "manifest_hash")
        chunkRootHex = try Self.requireHex(raw, key: "chunk_root", field: "chunk_root")
        laneId = try Self.requireUInt64(raw, key: "lane_id", field: "lane_id")
        epoch = try Self.requireUInt64(raw, key: "epoch", field: "epoch")
        manifestLength = Self.optionalUInt64(raw, key: "manifest_len", field: "manifest_len") ?? 0
        manifestBytes = try Self.requireManifestBytes(raw)
        manifestJson = Self.optionalValue(raw, key: "manifest")
        chunkPlan = try Self.requireValue(raw, key: "chunk_plan", field: "chunk_plan")
        samplingPlan = try ToriiDaSamplingPlan.parse(raw["sampling_plan"])
    }

    /// Render the chunk plan as a JSON string (sorted keys).
    public func chunkPlanJSONString(prettyPrinted: Bool = false) throws -> String {
        let data = try chunkPlan.encodedData(prettyPrinted: prettyPrinted)
        guard let json = String(data: data, encoding: .utf8) else {
            throw ToriiClientError.invalidPayload("chunk_plan contained invalid UTF-8 data")
        }
        return json
    }

    /// Attempt to derive the chunker handle from the manifest metadata.
    public func inferChunkerHandle() -> String? {
        guard case .object(let record) = manifestJson else {
            return nil
        }
        if let explicit = ToriiDaManifestBundle.trimmedString(record["chunker_handle"]) {
            return explicit
        }
        let chunkingValue = record["chunking"]
        guard case .object(let chunking) = chunkingValue else {
            return nil
        }
        guard
            let namespace = ToriiDaManifestBundle.trimmedString(chunking["namespace"]),
            let name = ToriiDaManifestBundle.trimmedString(chunking["name"]),
            let version = ToriiDaManifestBundle.trimmedString(chunking["semver"])
        else {
            return nil
        }
        return "\(namespace).\(name)@\(version)"
    }

    private static func requireManifestBytes(_ record: [String: ToriiJSONValue]) throws -> Data {
        let value = try requireString(record, key: "manifest_norito", field: "manifest_norito")
        guard let data = Data(base64Encoded: value) else {
            throw ToriiClientError.invalidPayload("manifest_norito field was not valid base64")
        }
        return data
    }

    private static func requireHex(_ record: [String: ToriiJSONValue],
                                   key: String,
                                   field: String) throws -> String {
        let value = try requireString(record, key: key, field: field)
        var trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix("0x") || trimmed.hasPrefix("0X") {
            trimmed = String(trimmed.dropFirst(2))
        }
        guard Data(hexString: trimmed) != nil else {
            throw ToriiClientError.invalidPayload("\(field) must be a hex string")
        }
        return trimmed.lowercased()
    }

    private static func requireUInt64(_ record: [String: ToriiJSONValue],
                                      key: String,
                                      field: String) throws -> UInt64 {
        if let value = optionalUInt64(record, key: key, field: field) {
            return value
        }
        throw ToriiClientError.invalidPayload("\(field) field was missing or invalid")
    }

    private static func optionalUInt64(_ record: [String: ToriiJSONValue],
                                       key: String,
                                       field _: String) -> UInt64? {
        guard let raw = record[key] else {
            return nil
        }
        return raw.normalizedUInt64
    }

    private static func requireString(_ record: [String: ToriiJSONValue],
                                      key: String,
                                      field: String) throws -> String {
        guard let raw = record[key],
              let string = trimmedString(raw),
              !string.isEmpty
        else {
            throw ToriiClientError.invalidPayload("\(field) field was missing or empty")
        }
        return string
    }

    private static func requireValue(_ record: [String: ToriiJSONValue],
                                     key: String,
                                     field: String) throws -> ToriiJSONValue {
        guard let value = record[key], value != .null else {
            throw ToriiClientError.invalidPayload("\(field) field was missing")
        }
        return value
    }

    private static func optionalValue(_ record: [String: ToriiJSONValue],
                                      key: String) -> ToriiJSONValue? {
        record[key]
    }

    private static func trimmedString(_ value: ToriiJSONValue?) -> String? {
        guard let value else { return nil }
        return value.normalizedString
    }
}

public struct ToriiDaManifestPersistedPaths: Sendable, Equatable {
    public let manifestURL: URL
    public let manifestJsonURL: URL
    public let chunkPlanURL: URL
    public let samplingPlanURL: URL?
    public let label: String
}

public struct ToriiDaSamplingPlan: Sendable, Equatable {
    public struct Sample: Sendable, Equatable {
        public let index: UInt32
        public let role: String
        public let group: UInt32
    }

    public let assignmentHashHex: String
    public let sampleWindow: UInt16
    public let samples: [Sample]

    func jsonPayload() -> ToriiJSONValue {
        let sampleValues = samples.map { sample in
            ToriiJSONValue.object([
                "index": .number(Double(sample.index)),
                "role": .string(sample.role),
                "group": .number(Double(sample.group)),
            ])
        }
        return .object([
            "assignment_hash": .string(assignmentHashHex),
            "sample_window": .number(Double(sampleWindow)),
            "samples": .array(sampleValues),
        ])
    }

    static func parse(_ value: ToriiJSONValue?) throws -> ToriiDaSamplingPlan? {
        guard let value else { return nil }
        guard case .object(let record) = value else {
            throw ToriiClientError.invalidPayload("sampling_plan must be an object")
        }
        let assignment = try requireString(record,
                                           key: "assignment_hash",
                                           field: "sampling_plan.assignment_hash")
        guard let assignmentData = Data(hexString: assignment), assignmentData.count == 32 else {
            throw ToriiClientError.invalidPayload("sampling_plan.assignment_hash must be a 32-byte hex string")
        }
        let sampleWindowRaw = try requireUInt64(record,
                                                key: "sample_window",
                                                field: "sampling_plan.sample_window",
                                                allowZero: true)
        guard sampleWindowRaw <= UInt16.max else {
            throw ToriiClientError.invalidPayload("sampling_plan.sample_window exceeds UInt16 range")
        }
        let sampleWindow = UInt16(sampleWindowRaw)
        let samplesValue = record["samples"] ?? .array([])
        guard case .array(let sampleArray) = samplesValue else {
            throw ToriiClientError.invalidPayload("sampling_plan.samples must be an array")
        }
        var samples = [Sample]()
        for (idx, entry) in sampleArray.enumerated() {
            guard case .object(let obj) = entry else {
                throw ToriiClientError.invalidPayload("sampling_plan.samples[\(idx)] must be an object")
            }
            let indexRaw = try requireUInt64(obj,
                                             key: "index",
                                             field: "sampling_plan.samples[\(idx)].index",
                                             allowZero: true)
            let groupRaw = try requireUInt64(obj,
                                             key: "group",
                                             field: "sampling_plan.samples[\(idx)].group",
                                             allowZero: true)
            guard indexRaw <= UInt32.max else {
                throw ToriiClientError.invalidPayload("sampling_plan.samples[\(idx)].index exceeds UInt32 range")
            }
            guard groupRaw <= UInt32.max else {
                throw ToriiClientError.invalidPayload("sampling_plan.samples[\(idx)].group exceeds UInt32 range")
            }
            let role = try requireString(obj,
                                         key: "role",
                                         field: "sampling_plan.samples[\(idx)].role")
            samples.append(Sample(index: UInt32(indexRaw),
                                  role: role,
                                  group: UInt32(groupRaw)))
        }

        return ToriiDaSamplingPlan(assignmentHashHex: assignment.lowercased(),
                                   sampleWindow: sampleWindow,
                                   samples: samples)
    }

    private static func requireString(_ record: [String: ToriiJSONValue],
                                      key: String,
                                      field: String) throws -> String {
        guard let raw = record[key],
              let string = trimmedString(raw),
              !string.isEmpty
        else {
            throw ToriiClientError.invalidPayload("\(field) field was missing or empty")
        }
        return string
    }

    private static func requireUInt64(_ record: [String: ToriiJSONValue],
                                      key: String,
                                      field: String,
                                      allowZero: Bool = false) throws -> UInt64 {
        if let value = optionalUInt64(record, key: key, allowZero: allowZero) {
            return value
        }
        throw ToriiClientError.invalidPayload("\(field) field was missing or invalid")
    }

    private static func optionalUInt64(_ record: [String: ToriiJSONValue],
                                       key: String,
                                       allowZero: Bool) -> UInt64? {
        guard let raw = record[key] else {
            return nil
        }
        guard let parsed = raw.normalizedUInt64 else {
            return nil
        }
        if !allowZero && parsed == 0 {
            return nil
        }
        return parsed
    }

    private static func trimmedString(_ value: ToriiJSONValue?) -> String? {
        guard let value else { return nil }
        return value.normalizedString
    }

}

public struct ToriiDaIngestPersistedPaths: Sendable, Equatable {
    public let requestJsonURL: URL
    public let receiptJsonURL: URL?
    public let responseHeadersURL: URL?
}

public struct ToriiDaAvailabilityPersistedPaths: Sendable, Equatable {
    public let manifest: ToriiDaManifestPersistedPaths
    public let payloadURL: URL
    public let proofSummaryURL: URL
    public let scoreboardURL: URL?
}

/// Result returned by the DA gateway helper.
public struct ToriiDaGatewayFetchResult: Sendable {
    public let manifest: ToriiDaManifestBundle
    public let manifestIdHex: String
    public let chunkerHandle: String
    public let chunkPlanJSON: String
    public let gatewayResult: SorafsGatewayFetchResult
    public let proofSummary: ToriiDaProofSummary?
}

public struct ToriiDaProofSummary: Decodable, Sendable, Equatable {
    public let blobHashHex: String
    public let chunkRootHex: String
    public let porRootHex: String
    public let leafCount: UInt64
    public let segmentCount: UInt64
    public let chunkCount: UInt64
    public let sampleCount: UInt64
    public let sampleSeed: UInt64
    public let proofCount: UInt64
    public let proofs: [ToriiDaProofRecord]

    public init(
        blobHashHex: String,
        chunkRootHex: String,
        porRootHex: String,
        leafCount: UInt64,
        segmentCount: UInt64,
        chunkCount: UInt64,
        sampleCount: UInt64,
        sampleSeed: UInt64,
        proofCount: UInt64,
        proofs: [ToriiDaProofRecord]
    ) {
        self.blobHashHex = blobHashHex
        self.chunkRootHex = chunkRootHex
        self.porRootHex = porRootHex
        self.leafCount = leafCount
        self.segmentCount = segmentCount
        self.chunkCount = chunkCount
        self.sampleCount = sampleCount
        self.sampleSeed = sampleSeed
        self.proofCount = proofCount
        self.proofs = proofs
    }

    private enum CodingKeys: String, CodingKey {
        case blobHashHex = "blob_hash_hex"
        case chunkRootHex = "chunk_root_hex"
        case porRootHex = "por_root_hex"
        case leafCount = "leaf_count"
        case segmentCount = "segment_count"
        case chunkCount = "chunk_count"
        case sampleCount = "sample_count"
        case sampleSeed = "sample_seed"
        case proofCount = "proof_count"
        case proofs
    }
}

public struct ToriiDaProofRecord: Decodable, Sendable, Equatable {
    public let origin: String
    public let leafIndex: UInt32
    public let chunkIndex: UInt32
    public let segmentIndex: UInt32
    public let leafOffset: UInt64
    public let leafLength: UInt32
    public let segmentOffset: UInt64
    public let segmentLength: UInt32
    public let chunkOffset: UInt64
    public let chunkLength: UInt32
    public let payloadLength: UInt64
    public let chunkDigestHex: String
    public let chunkRootHex: String
    public let segmentDigestHex: String
    public let leafDigestHex: String
    public let leafBytes: Data
    public let segmentLeavesHex: [String]
    public let chunkSegmentsHex: [String]
    public let chunkRootsHex: [String]
    public let verified: Bool

    public init(
        origin: String,
        leafIndex: UInt32,
        chunkIndex: UInt32,
        segmentIndex: UInt32,
        leafOffset: UInt64,
        leafLength: UInt32,
        segmentOffset: UInt64,
        segmentLength: UInt32,
        chunkOffset: UInt64,
        chunkLength: UInt32,
        payloadLength: UInt64,
        chunkDigestHex: String,
        chunkRootHex: String,
        segmentDigestHex: String,
        leafDigestHex: String,
        leafBytes: Data,
        segmentLeavesHex: [String],
        chunkSegmentsHex: [String],
        chunkRootsHex: [String],
        verified: Bool
    ) {
        self.origin = origin
        self.leafIndex = leafIndex
        self.chunkIndex = chunkIndex
        self.segmentIndex = segmentIndex
        self.leafOffset = leafOffset
        self.leafLength = leafLength
        self.segmentOffset = segmentOffset
        self.segmentLength = segmentLength
        self.chunkOffset = chunkOffset
        self.chunkLength = chunkLength
        self.payloadLength = payloadLength
        self.chunkDigestHex = chunkDigestHex
        self.chunkRootHex = chunkRootHex
        self.segmentDigestHex = segmentDigestHex
        self.leafDigestHex = leafDigestHex
        self.leafBytes = leafBytes
        self.segmentLeavesHex = segmentLeavesHex
        self.chunkSegmentsHex = chunkSegmentsHex
        self.chunkRootsHex = chunkRootsHex
        self.verified = verified
    }

    private enum CodingKeys: String, CodingKey {
        case origin
        case leafIndex = "leaf_index"
        case chunkIndex = "chunk_index"
        case segmentIndex = "segment_index"
        case leafOffset = "leaf_offset"
        case leafLength = "leaf_length"
        case segmentOffset = "segment_offset"
        case segmentLength = "segment_length"
        case chunkOffset = "chunk_offset"
        case chunkLength = "chunk_length"
        case payloadLength = "payload_len"
        case chunkDigestHex = "chunk_digest_hex"
        case chunkRootHex = "chunk_root_hex"
        case segmentDigestHex = "segment_digest_hex"
        case leafDigestHex = "leaf_digest_hex"
        case leafBytesB64 = "leaf_bytes_b64"
        case segmentLeavesHex = "segment_leaves_hex"
        case chunkSegmentsHex = "chunk_segments_hex"
        case chunkRootsHex = "chunk_roots_hex"
        case verified
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        origin = try container.decode(String.self, forKey: .origin)
        leafIndex = try container.decode(UInt32.self, forKey: .leafIndex)
        chunkIndex = try container.decode(UInt32.self, forKey: .chunkIndex)
        segmentIndex = try container.decode(UInt32.self, forKey: .segmentIndex)
        leafOffset = try container.decode(UInt64.self, forKey: .leafOffset)
        leafLength = try container.decode(UInt32.self, forKey: .leafLength)
        segmentOffset = try container.decode(UInt64.self, forKey: .segmentOffset)
        segmentLength = try container.decode(UInt32.self, forKey: .segmentLength)
        chunkOffset = try container.decode(UInt64.self, forKey: .chunkOffset)
        chunkLength = try container.decode(UInt32.self, forKey: .chunkLength)
        payloadLength = try container.decode(UInt64.self, forKey: .payloadLength)
        chunkDigestHex = try container.decode(String.self, forKey: .chunkDigestHex)
        chunkRootHex = try container.decode(String.self, forKey: .chunkRootHex)
        segmentDigestHex = try container.decode(String.self, forKey: .segmentDigestHex)
        leafDigestHex = try container.decode(String.self, forKey: .leafDigestHex)
        let leafBytesString = try container.decode(String.self, forKey: .leafBytesB64)
        guard let decodedLeafBytes = Data(base64Encoded: leafBytesString) else {
            throw DecodingError.dataCorruptedError(
                forKey: .leafBytesB64,
                in: container,
                debugDescription: "leaf_bytes_b64 field was not valid base64"
            )
        }
        leafBytes = decodedLeafBytes
        segmentLeavesHex = try container.decode([String].self, forKey: .segmentLeavesHex)
        chunkSegmentsHex = try container.decode([String].self, forKey: .chunkSegmentsHex)
        chunkRootsHex = try container.decode([String].self, forKey: .chunkRootsHex)
        verified = try container.decode(Bool.self, forKey: .verified)
    }
}

/// Options controlling Proof-of-Retrievability summary generation.
public struct ToriiDaProofSummaryOptions: Sendable, Equatable {
    public var sampleCount: Int
    public var sampleSeed: UInt64
    public var leafIndexes: [Int]

    public init(sampleCount: Int = 8, sampleSeed: UInt64 = 0, leafIndexes: [Int] = []) {
        self.sampleCount = sampleCount
        self.sampleSeed = sampleSeed
        self.leafIndexes = leafIndexes
    }
}

/// Typed view over the PoR summary JSON emitted by `iroha da prove`.
public protocol DaProofSummaryGenerating: Sendable {
    func makeProofSummary(manifest: Data,
                          payload: Data,
                          options: ToriiDaProofSummaryOptions) throws -> ToriiDaProofSummary
}

public struct NativeDaProofSummaryGenerator: DaProofSummaryGenerating {
    public static let shared = NativeDaProofSummaryGenerator()

    public init() {}

    public func makeProofSummary(manifest: Data,
                                 payload: Data,
                                 options: ToriiDaProofSummaryOptions) throws -> ToriiDaProofSummary {
        if options.sampleCount < 0 {
            throw ToriiClientError.invalidPayload("proofSummaryOptions.sampleCount must be non-negative")
        }
        for (index, leafIndex) in options.leafIndexes.enumerated() where leafIndex < 0 {
            throw ToriiClientError.invalidPayload(
                "proofSummaryOptions.leafIndexes[\(index)] must be non-negative"
            )
        }
        guard let summaryData = NoritoNativeBridge.shared.daProofSummary(
            manifest: manifest,
            payload: payload,
            options: options
        ) else {
            throw ToriiClientError.invalidPayload("failed to generate DA proof summary")
        }
        let decoder = JSONDecoder()
        return try decoder.decode(ToriiDaProofSummary.self, from: summaryData)
    }
}

public struct ToriiConfidentialKeysetRequest: Encodable, Sendable {
    public var seedHex: String?
    public var seedBase64: String?

    public init(seedHex: String? = nil, seedBase64: String? = nil) {
        self.seedHex = seedHex
        self.seedBase64 = seedBase64
    }

    private enum CodingKeys: String, CodingKey {
        case seedHex = "seed_hex"
        case seedBase64 = "seed_b64"
    }
}

public struct ToriiConfidentialKeysetResponse: Decodable, Sendable {
    public let seedHex: String
    public let seedBase64: String
    public let nullifierKeyHex: String
    public let nullifierKeyBase64: String
    public let incomingViewKeyHex: String
    public let incomingViewKeyBase64: String
    public let outgoingViewKeyHex: String
    public let outgoingViewKeyBase64: String
    public let fullViewKeyHex: String
    public let fullViewKeyBase64: String

    private enum CodingKeys: String, CodingKey {
        case seedHex = "seed_hex"
        case seedBase64 = "seed_b64"
        case nullifierKeyHex = "nullifier_key_hex"
        case nullifierKeyBase64 = "nullifier_key_b64"
        case incomingViewKeyHex = "incoming_view_key_hex"
        case incomingViewKeyBase64 = "incoming_view_key_b64"
        case outgoingViewKeyHex = "outgoing_view_key_hex"
        case outgoingViewKeyBase64 = "outgoing_view_key_b64"
        case fullViewKeyHex = "full_view_key_hex"
        case fullViewKeyBase64 = "full_view_key_b64"
    }

    public func asKeyset() throws -> ConfidentialKeyset {
        guard let seedData = Data(hexString: seedHex) else {
            throw ConfidentialKeyDerivationError.invalidHexEncoding(field: "seed_hex")
        }
        guard let nullifierData = Data(hexString: nullifierKeyHex) else {
            throw ConfidentialKeyDerivationError.invalidHexEncoding(field: "nullifier_key_hex")
        }
        guard let incomingData = Data(hexString: incomingViewKeyHex) else {
            throw ConfidentialKeyDerivationError.invalidHexEncoding(field: "incoming_view_key_hex")
        }
        guard let outgoingData = Data(hexString: outgoingViewKeyHex) else {
            throw ConfidentialKeyDerivationError.invalidHexEncoding(field: "outgoing_view_key_hex")
        }
        guard let fullViewData = Data(hexString: fullViewKeyHex) else {
            throw ConfidentialKeyDerivationError.invalidHexEncoding(field: "full_view_key_hex")
        }
        return try ConfidentialKeyset(
            spendKey: seedData,
            nullifierKey: nullifierData,
            incomingViewKey: incomingData,
            outgoingViewKey: outgoingData,
            fullViewKey: fullViewData
        )
    }
}

public struct ToriiConfidentialPolicyTransition: Decodable, Sendable {
    public let transitionId: String
    public let previousMode: String
    public let newMode: String
    public let effectiveHeight: UInt64
    public let conversionWindow: UInt64?
    public let windowOpenHeight: UInt64?

    private enum CodingKeys: String, CodingKey {
        case transitionId = "transition_id"
        case previousMode = "previous_mode"
        case newMode = "new_mode"
        case effectiveHeight = "effective_height"
        case conversionWindow = "conversion_window"
        case windowOpenHeight = "window_open_height"
    }
}

public struct ToriiConfidentialAssetPolicy: Decodable, Sendable {
    public let assetId: String
    public let blockHeight: UInt64
    public let currentMode: String
    public let effectiveMode: String
    public let vkSetHashHex: String?
    public let poseidonParamsId: UInt32?
    public let pedersenParamsId: UInt32?
    public let pendingTransition: ToriiConfidentialPolicyTransition?

    private enum CodingKeys: String, CodingKey {
        case assetId = "asset_id"
        case blockHeight = "block_height"
        case currentMode = "current_mode"
        case effectiveMode = "effective_mode"
        case vkSetHashHex = "vk_set_hash"
        case poseidonParamsId = "poseidon_params_id"
        case pedersenParamsId = "pedersen_params_id"
        case pendingTransition = "pending_transition"
    }
}

public struct ToriiNodeCapabilities: Decodable, Sendable {
    /// Must match `iroha_data_model::DATA_MODEL_VERSION` on the node.
    public static let expectedDataModelVersion = 1
    public let supportedAbiVersions: [Int]
    public let defaultCompileTarget: Int
    public let dataModelVersion: Int?
    public let crypto: ToriiNodeCryptoCapabilities?

    private enum CodingKeys: String, CodingKey {
        case supportedAbiVersions = "supported_abi_versions"
        case defaultCompileTarget = "default_compile_target"
        case dataModelVersion = "data_model_version"
        case crypto
    }
}

public struct ToriiNodeCryptoCapabilities: Decodable, Sendable {
    public let sm: ToriiNodeSmCapabilities?

    private enum CodingKeys: String, CodingKey {
        case sm
    }
}

public struct ToriiNodeSmCapabilities: Decodable, Sendable {
    public let enabled: Bool
    public let defaultHash: String
    public let allowedSigning: [String]
    public let sm2DistidDefault: String
    public let opensslPreview: Bool
    public let acceleration: ToriiNodeSmAcceleration?

    private enum CodingKeys: String, CodingKey {
        case enabled
        case defaultHash = "default_hash"
        case allowedSigning = "allowed_signing"
        case sm2DistidDefault = "sm2_distid_default"
        case opensslPreview = "openssl_preview"
        case acceleration
    }
}

public struct ToriiNodeSmAcceleration: Decodable, Sendable {
    public let scalar: Bool
    public let neonSm3: Bool
    public let neonSm4: Bool
    public let policy: String

    private enum CodingKeys: String, CodingKey {
        case scalar
        case neonSm3 = "neon_sm3"
        case neonSm4 = "neon_sm4"
        case policy
    }
}

public struct ToriiLoggerConfig: Decodable, Sendable {
    public let level: String
    public let filter: String?
}

public struct ToriiNetworkConfig: Decodable, Sendable {
    public let blockGossipSize: Int
    public let blockGossipPeriodMs: Int
    public let transactionGossipSize: Int
    public let transactionGossipPeriodMs: Int

    private enum CodingKeys: String, CodingKey {
        case blockGossipSize = "block_gossip_size"
        case blockGossipPeriodMs = "block_gossip_period_ms"
        case transactionGossipSize = "transaction_gossip_size"
        case transactionGossipPeriodMs = "transaction_gossip_period_ms"
    }
}

public struct ToriiQueueConfig: Decodable, Sendable {
    public let capacity: Int
}

public struct ToriiConfidentialGasSchedule: Codable, Sendable, Equatable {
    public let proofBase: Int
    public let perPublicInput: Int
    public let perProofByte: Int
    public let perNullifier: Int
    public let perCommitment: Int

    private enum CodingKeys: String, CodingKey {
        case proofBase = "proof_base"
        case perPublicInput = "per_public_input"
        case perProofByte = "per_proof_byte"
        case perNullifier = "per_nullifier"
        case perCommitment = "per_commitment"
    }
}

public struct ToriiNexusAxtConfig: Decodable, Sendable, Equatable {
    public let slotLengthMs: UInt64
    public let maxClockSkewMs: UInt64
    public let proofCacheTtlSlots: UInt64
    public let replayRetentionSlots: UInt64

    private enum CodingKeys: String, CodingKey {
        case slotLengthMs = "slot_length_ms"
        case maxClockSkewMs = "max_clock_skew_ms"
        case proofCacheTtlSlots = "proof_cache_ttl_slots"
        case replayRetentionSlots = "replay_retention_slots"
    }
}

public struct ToriiNexusConfig: Decodable, Sendable {
    public let axt: ToriiNexusAxtConfig?
}

public struct ToriiConfigurationTransport: Decodable, Sendable {
    public let noritoRpc: ToriiConfigurationTransportNoritoRpc?
    public let streaming: ToriiConfigurationStreaming?

    private enum CodingKeys: String, CodingKey {
        case noritoRpc = "norito_rpc"
        case streaming
    }
}

public struct ToriiConfigurationTransportNoritoRpc: Decodable, Sendable {
    public let enabled: Bool
    public let stage: String
    public let requireMtls: Bool
    public let canaryAllowlistSize: Int

    private enum CodingKeys: String, CodingKey {
        case enabled
        case stage
        case requireMtls = "require_mtls"
        case canaryAllowlistSize = "canary_allowlist_size"
    }
}

public struct ToriiConfigurationStreaming: Decodable, Sendable {
    public let soranet: ToriiConfigurationStreamingSoranet?
}

public struct ToriiConfigurationStreamingSoranet: Decodable, Sendable {
    public let enabled: Bool
    public let streamTag: String
    public let exitMultiaddr: String
    public let paddingBudgetMs: UInt16?
    public let accessKind: String
    public let garCategory: String
    public let channelSalt: String
    public let provisionSpoolDir: String
    public let provisionWindowSegments: UInt64
    public let provisionQueueCapacity: UInt64

    private enum CodingKeys: String, CodingKey {
        case enabled
        case streamTag = "stream_tag"
        case exitMultiaddr = "exit_multiaddr"
        case paddingBudgetMs = "padding_budget_ms"
        case accessKind = "access_kind"
        case garCategory = "gar_category"
        case channelSalt = "channel_salt"
        case provisionSpoolDir = "provision_spool_dir"
        case provisionWindowSegments = "provision_window_segments"
        case provisionQueueCapacity = "provision_queue_capacity"
    }
}

public struct ToriiConfigurationSnapshot: Decodable, Sendable {
    public let publicKeyHex: String
    public let logger: ToriiLoggerConfig
    public let network: ToriiNetworkConfig
    public let queue: ToriiQueueConfig?
    public let confidentialGas: ToriiConfidentialGasSchedule?
    public let transport: ToriiConfigurationTransport?
    public let nexus: ToriiNexusConfig?

    private enum CodingKeys: String, CodingKey {
        case publicKeyHex = "public_key"
        case logger
        case network
        case queue
        case confidentialGas = "confidential_gas"
        case transport
        case nexus
    }
}

public struct ToriiRuntimeUpgradeCounters: Decodable, Sendable {
    public let proposed: Int
    public let activated: Int
    public let canceled: Int

    private enum CodingKeys: String, CodingKey {
        case proposed
        case activated
        case canceled
    }
}

public struct ToriiRuntimeMetrics: Decodable, Sendable {
    public let activeAbiVersionsCount: Int
    public let upgradeEventsTotal: ToriiRuntimeUpgradeCounters

    private enum CodingKeys: String, CodingKey {
        case activeAbiVersionsCount = "active_abi_versions_count"
        case upgradeEventsTotal = "upgrade_events_total"
    }
}

public struct ToriiRuntimeAbiActive: Decodable, Sendable {
    public let activeVersions: [Int]
    public let defaultCompileTarget: Int

    private enum CodingKeys: String, CodingKey {
        case activeVersions = "active_versions"
        case defaultCompileTarget = "default_compile_target"
    }
}

public struct ToriiRuntimeAbiHash: Decodable, Sendable {
    public let policy: String
    public let abiHashHex: String

    private enum CodingKeys: String, CodingKey {
        case policy
        case abiHashHex = "abi_hash_hex"
    }
}

public enum ToriiRuntimeUpgradeStatus: Equatable, Sendable {
    case proposed
    case activatedAt(UInt64)
    case canceled
}

public enum ToriiPlatformPolicy: String, Codable, Sendable, CaseIterable {
    case markerKey = "marker_key"
    case playIntegrity = "play_integrity"
    case hmsSafetyDetect = "hms_safety_detect"
    case provisioned = "provisioned"
}

public struct ToriiOfflineListParams: Sendable, Equatable {
    public var filter: String?
    public var limit: UInt64?
    public var offset: UInt64?
    public var sort: String?
    public var addressFormat: String?
    public var controllerId: String?
    public var receiverId: String?
    public var depositAccountId: String?
    public var assetId: String?
    public var certificateExpiresBeforeMs: UInt64?
    public var certificateExpiresAfterMs: UInt64?
    public var policyExpiresBeforeMs: UInt64?
    public var policyExpiresAfterMs: UInt64?
    public var refreshBeforeMs: UInt64?
    public var refreshAfterMs: UInt64?
    public var verdictIdHex: String?
    public var attestationNonceHex: String?
    public var certificateIdHex: String?
    public var platformPolicy: ToriiPlatformPolicy?
    public var requireVerdict: Bool
    public var onlyMissingVerdict: Bool
    public var includeExpired: Bool

    public init(filter: String? = nil,
                limit: UInt64? = nil,
                offset: UInt64? = nil,
                sort: String? = nil,
                addressFormat: String? = nil,
                controllerId: String? = nil,
                receiverId: String? = nil,
                depositAccountId: String? = nil,
                assetId: String? = nil,
                certificateExpiresBeforeMs: UInt64? = nil,
                certificateExpiresAfterMs: UInt64? = nil,
                policyExpiresBeforeMs: UInt64? = nil,
                policyExpiresAfterMs: UInt64? = nil,
                refreshBeforeMs: UInt64? = nil,
                refreshAfterMs: UInt64? = nil,
                verdictIdHex: String? = nil,
                attestationNonceHex: String? = nil,
                certificateIdHex: String? = nil,
                platformPolicy: ToriiPlatformPolicy? = nil,
                requireVerdict: Bool = false,
                onlyMissingVerdict: Bool = false,
                includeExpired: Bool = false) {
        self.filter = filter
        self.limit = limit
        self.offset = offset
        self.sort = sort
        self.addressFormat = addressFormat
        self.controllerId = controllerId
        self.receiverId = receiverId
        self.depositAccountId = depositAccountId
        self.assetId = assetId
        self.certificateExpiresBeforeMs = certificateExpiresBeforeMs
        self.certificateExpiresAfterMs = certificateExpiresAfterMs
        self.policyExpiresBeforeMs = policyExpiresBeforeMs
        self.policyExpiresAfterMs = policyExpiresAfterMs
        self.refreshBeforeMs = refreshBeforeMs
        self.refreshAfterMs = refreshAfterMs
        self.verdictIdHex = verdictIdHex
        self.attestationNonceHex = attestationNonceHex
        self.certificateIdHex = certificateIdHex
        self.platformPolicy = platformPolicy
        self.requireVerdict = requireVerdict
        self.onlyMissingVerdict = onlyMissingVerdict
        self.includeExpired = includeExpired
    }

    public func queryItems() throws -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let filter, !filter.isEmpty {
            items.append(URLQueryItem(name: "filter", value: filter))
        }
        if let limit {
            items.append(URLQueryItem(name: "limit", value: String(limit)))
        }
        if let offset {
            items.append(URLQueryItem(name: "offset", value: String(offset)))
        }
        if let sort, !sort.isEmpty {
            items.append(URLQueryItem(name: "sort", value: sort))
        }
        if let normalized = try ToriiClient.normalizeAddressFormatQueryValue(addressFormat,
                                                                             context: "ToriiOfflineListParams.addressFormat") {
            items.append(URLQueryItem(name: "address_format", value: normalized))
        }
        if let controllerId = controllerId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !controllerId.isEmpty {
            items.append(URLQueryItem(name: "controller_id", value: controllerId))
        }
        if let receiverId = receiverId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !receiverId.isEmpty {
            items.append(URLQueryItem(name: "receiver_id", value: receiverId))
        }
        if let depositAccountId = depositAccountId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !depositAccountId.isEmpty {
            items.append(URLQueryItem(name: "deposit_account_id", value: depositAccountId))
        }
        if let assetId = assetId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !assetId.isEmpty {
            items.append(URLQueryItem(name: "asset_id", value: assetId))
        }
        if let certificateExpiresBeforeMs {
            items.append(URLQueryItem(name: "certificate_expires_before_ms",
                                      value: String(certificateExpiresBeforeMs)))
        }
        if let certificateExpiresAfterMs {
            items.append(URLQueryItem(name: "certificate_expires_after_ms",
                                      value: String(certificateExpiresAfterMs)))
        }
        if let policyExpiresBeforeMs {
            items.append(URLQueryItem(name: "policy_expires_before_ms",
                                      value: String(policyExpiresBeforeMs)))
        }
        if let policyExpiresAfterMs {
            items.append(URLQueryItem(name: "policy_expires_after_ms",
                                      value: String(policyExpiresAfterMs)))
        }
        if let refreshBeforeMs {
            items.append(URLQueryItem(name: "refresh_before_ms",
                                      value: String(refreshBeforeMs)))
        }
        if let refreshAfterMs {
            items.append(URLQueryItem(name: "refresh_after_ms",
                                      value: String(refreshAfterMs)))
        }
        if let verdictIdHex, !verdictIdHex.isEmpty {
            items.append(URLQueryItem(name: "verdict_id_hex", value: verdictIdHex.lowercased()))
        }
        if let attestationNonceHex, !attestationNonceHex.isEmpty {
            items.append(URLQueryItem(name: "attestation_nonce_hex",
                                      value: attestationNonceHex.lowercased()))
        }
        if let certificateIdHex, !certificateIdHex.isEmpty {
            items.append(URLQueryItem(name: "certificate_id_hex",
                                      value: certificateIdHex.lowercased()))
        }
        if let platformPolicy {
            items.append(URLQueryItem(name: "platform_policy", value: platformPolicy.rawValue))
        }
        if requireVerdict {
            items.append(URLQueryItem(name: "require_verdict", value: "true"))
        }
        if onlyMissingVerdict {
            items.append(URLQueryItem(name: "only_missing_verdict", value: "true"))
        }
        if includeExpired {
            items.append(URLQueryItem(name: "include_expired", value: "true"))
        }
        return items.isEmpty ? nil : items
    }
}

public struct ToriiOfflineRevocationListParams: Sendable, Equatable {
    public var filter: String?
    public var limit: UInt64?
    public var offset: UInt64?
    public var sort: String?
    public var addressFormat: String?

    public init(filter: String? = nil,
                limit: UInt64? = nil,
                offset: UInt64? = nil,
                sort: String? = nil,
                addressFormat: String? = nil) {
        self.filter = filter
        self.limit = limit
        self.offset = offset
        self.sort = sort
        self.addressFormat = addressFormat
    }

    public func queryItems() throws -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let filter, !filter.isEmpty {
            items.append(URLQueryItem(name: "filter", value: filter))
        }
        if let limit {
            items.append(URLQueryItem(name: "limit", value: String(limit)))
        }
        if let offset {
            items.append(URLQueryItem(name: "offset", value: String(offset)))
        }
        if let sort, !sort.isEmpty {
            items.append(URLQueryItem(name: "sort", value: sort))
        }
        if let normalized = try ToriiClient.normalizeAddressFormatQueryValue(addressFormat,
                                                                             context: "ToriiOfflineRevocationListParams.addressFormat") {
            items.append(URLQueryItem(name: "address_format", value: normalized))
        }
        return items.isEmpty ? nil : items
    }
}

public struct ToriiOfflineBundleProofStatusParams: Sendable, Equatable {
    public var bundleIdHex: String
    public var addressFormat: String?

    public init(bundleIdHex: String, addressFormat: String? = nil) {
        self.bundleIdHex = bundleIdHex
        self.addressFormat = addressFormat
    }

    public func queryItems() throws -> [URLQueryItem] {
        var items: [URLQueryItem] = []
        var trimmed = bundleIdHex.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix("0x") || trimmed.hasPrefix("0X") {
            trimmed = String(trimmed.dropFirst(2))
        }
        guard !trimmed.isEmpty, Data(hexString: trimmed) != nil else {
            throw ToriiClientError.invalidPayload("bundleIdHex must be a valid hex string")
        }
        items.append(URLQueryItem(name: "bundle_id_hex", value: trimmed.lowercased()))
        if let normalized = try ToriiClient.normalizeAddressFormatQueryValue(addressFormat,
                                                                             context: "ToriiOfflineBundleProofStatusParams.addressFormat") {
            items.append(URLQueryItem(name: "address_format", value: normalized))
        }
        return items
    }
}

public struct ToriiOfflineReceiptListParams: Sendable, Equatable {
    public var filter: String?
    public var limit: UInt64?
    public var offset: UInt64?
    public var sort: String?
    public var addressFormat: String?
    public var controllerId: String?
    public var receiverId: String?
    public var bundleIdHex: String?
    public var certificateIdHex: String?
    public var invoiceId: String?
    public var assetId: String?

    public init(filter: String? = nil,
                limit: UInt64? = nil,
                offset: UInt64? = nil,
                sort: String? = nil,
                addressFormat: String? = nil,
                controllerId: String? = nil,
                receiverId: String? = nil,
                bundleIdHex: String? = nil,
                certificateIdHex: String? = nil,
                invoiceId: String? = nil,
                assetId: String? = nil) {
        self.filter = filter
        self.limit = limit
        self.offset = offset
        self.sort = sort
        self.addressFormat = addressFormat
        self.controllerId = controllerId
        self.receiverId = receiverId
        self.bundleIdHex = bundleIdHex
        self.certificateIdHex = certificateIdHex
        self.invoiceId = invoiceId
        self.assetId = assetId
    }

    public func queryItems() throws -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let filter, !filter.isEmpty {
            items.append(URLQueryItem(name: "filter", value: filter))
        }
        if let limit {
            items.append(URLQueryItem(name: "limit", value: String(limit)))
        }
        if let offset {
            items.append(URLQueryItem(name: "offset", value: String(offset)))
        }
        if let sort, !sort.isEmpty {
            items.append(URLQueryItem(name: "sort", value: sort))
        }
        if let normalized = try ToriiClient.normalizeAddressFormatQueryValue(addressFormat,
                                                                             context: "ToriiOfflineReceiptListParams.addressFormat") {
            items.append(URLQueryItem(name: "address_format", value: normalized))
        }
        if let controllerId = controllerId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !controllerId.isEmpty {
            items.append(URLQueryItem(name: "controller_id", value: controllerId))
        }
        if let receiverId = receiverId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !receiverId.isEmpty {
            items.append(URLQueryItem(name: "receiver_id", value: receiverId))
        }
        if let bundleIdHex = Self.normalizeHex(bundleIdHex) {
            items.append(URLQueryItem(name: "bundle_id_hex", value: bundleIdHex))
        }
        if let certificateIdHex = Self.normalizeHex(certificateIdHex) {
            items.append(URLQueryItem(name: "certificate_id_hex", value: certificateIdHex))
        }
        if let invoiceId = invoiceId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !invoiceId.isEmpty {
            items.append(URLQueryItem(name: "invoice_id", value: invoiceId))
        }
        if let assetId = assetId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !assetId.isEmpty {
            items.append(URLQueryItem(name: "asset_id", value: assetId))
        }
        return items.isEmpty ? nil : items
    }

    private static func normalizeHex(_ raw: String?) -> String? {
        guard var trimmed = raw?.trimmingCharacters(in: .whitespacesAndNewlines),
              !trimmed.isEmpty else {
            return nil
        }
        if trimmed.hasPrefix("0x") || trimmed.hasPrefix("0X") {
            trimmed = String(trimmed.dropFirst(2))
        }
        return trimmed.lowercased()
    }
}

public struct ToriiUaidPortfolioTotals: Decodable, Sendable {
    public let accounts: UInt64
    public let positions: UInt64

    public init(accounts: UInt64 = 0, positions: UInt64 = 0) {
        self.accounts = accounts
        self.positions = positions
    }

    private enum CodingKeys: String, CodingKey {
        case accounts
        case positions
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let accounts = try container.decodeIfPresent(UInt64.self, forKey: .accounts) ?? 0
        let positions = try container.decodeIfPresent(UInt64.self, forKey: .positions) ?? 0
        self.init(accounts: accounts, positions: positions)
    }
}

public struct ToriiUaidPortfolioAsset: Decodable, Sendable {
    public let assetId: String
    public let assetDefinitionId: String
    public let quantity: String

    private enum CodingKeys: String, CodingKey {
        case assetId = "asset_id"
        case assetDefinitionId = "asset_definition_id"
        case quantity
    }
}

public struct ToriiUaidPortfolioAccount: Decodable, Sendable {
    public let accountId: String
    public let label: String?
    public let assets: [ToriiUaidPortfolioAsset]

    private enum CodingKeys: String, CodingKey {
        case accountId = "account_id"
        case label
        case assets
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        accountId = try container.decode(String.self, forKey: .accountId)
        label = try container.decodeIfPresent(String.self, forKey: .label)
        assets = try container.decodeIfPresent([ToriiUaidPortfolioAsset].self, forKey: .assets) ?? []
    }
}

public struct ToriiUaidPortfolioDataspace: Decodable, Sendable {
    public let dataspaceId: UInt64
    public let dataspaceAlias: String?
    public let accounts: [ToriiUaidPortfolioAccount]

    private enum CodingKeys: String, CodingKey {
        case dataspaceId = "dataspace_id"
        case dataspaceAlias = "dataspace_alias"
        case accounts
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        dataspaceId = try container.decode(UInt64.self, forKey: .dataspaceId)
        dataspaceAlias = try container.decodeIfPresent(String.self, forKey: .dataspaceAlias)
        accounts = try container.decodeIfPresent([ToriiUaidPortfolioAccount].self, forKey: .accounts) ?? []
    }
}

public struct ToriiUaidPortfolioResponse: Decodable, Sendable {
    public let uaid: String
    public let totals: ToriiUaidPortfolioTotals
    public let dataspaces: [ToriiUaidPortfolioDataspace]

    private enum CodingKeys: String, CodingKey {
        case uaid
        case totals
        case dataspaces
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        uaid = try container.decode(String.self, forKey: .uaid)
        totals = try container.decodeIfPresent(ToriiUaidPortfolioTotals.self, forKey: .totals) ?? ToriiUaidPortfolioTotals()
        dataspaces = try container.decodeIfPresent([ToriiUaidPortfolioDataspace].self, forKey: .dataspaces) ?? []
    }
}

public struct ToriiUaidBindingsDataspace: Decodable, Sendable {
    public let dataspaceId: UInt64
    public let dataspaceAlias: String?
    public let accounts: [String]

    private enum CodingKeys: String, CodingKey {
        case dataspaceId = "dataspace_id"
        case dataspaceAlias = "dataspace_alias"
        case accounts
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        dataspaceId = try container.decode(UInt64.self, forKey: .dataspaceId)
        dataspaceAlias = try container.decodeIfPresent(String.self, forKey: .dataspaceAlias)
        accounts = try container.decodeIfPresent([String].self, forKey: .accounts) ?? []
    }
}

public struct ToriiUaidBindingsResponse: Decodable, Sendable {
    public let uaid: String
    public let dataspaces: [ToriiUaidBindingsDataspace]

    private enum CodingKeys: String, CodingKey {
        case uaid
        case dataspaces
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        uaid = try container.decode(String.self, forKey: .uaid)
        dataspaces = try container.decodeIfPresent([ToriiUaidBindingsDataspace].self, forKey: .dataspaces) ?? []
    }
}

public struct ToriiUaidBindingsQuery: Sendable, Equatable {
    public var addressFormat: String?

    public init(addressFormat: String? = nil) {
        self.addressFormat = addressFormat
    }

    public func queryItems() throws -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let normalized = try ToriiClient.normalizeAddressFormatQueryValue(addressFormat,
                                                                             context: "ToriiUaidBindingsQuery.addressFormat") {
            items.append(URLQueryItem(name: "address_format", value: normalized))
        }
        return items.isEmpty ? nil : items
    }
}

public struct ToriiUaidPortfolioQuery: Sendable, Equatable {
    public var assetId: String?

    public init(assetId: String? = nil) {
        self.assetId = assetId
    }

    public func queryItems() -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let assetId = assetId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !assetId.isEmpty {
            items.append(URLQueryItem(name: "asset_id", value: assetId))
        }
        return items.isEmpty ? nil : items
    }
}

public enum ToriiUaidManifestStatus: String, Decodable, Sendable {
    case active = "Active"
    case pending = "Pending"
    case expired = "Expired"
    case revoked = "Revoked"
}

public struct ToriiUaidManifestRevocation: Decodable, Sendable {
    public let epoch: UInt64
    public let reason: String?
}

public struct ToriiUaidManifestLifecycle: Decodable, Sendable {
    public let activatedEpoch: UInt64?
    public let expiredEpoch: UInt64?
    public let revocation: ToriiUaidManifestRevocation?

    private enum CodingKeys: String, CodingKey {
        case activatedEpoch = "activated_epoch"
        case expiredEpoch = "expired_epoch"
        case revocation
    }
}

public struct ToriiUaidManifestRecord: Decodable, Sendable {
    public let dataspaceId: UInt64
    public let dataspaceAlias: String?
    public let manifestHash: String
    public let status: ToriiUaidManifestStatus
    public let lifecycle: ToriiUaidManifestLifecycle
    public let accounts: [String]
    public let manifest: ToriiJSONValue

    private enum CodingKeys: String, CodingKey {
        case dataspaceId = "dataspace_id"
        case dataspaceAlias = "dataspace_alias"
        case manifestHash = "manifest_hash"
        case status
        case lifecycle
        case accounts
        case manifest
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        dataspaceId = try container.decode(UInt64.self, forKey: .dataspaceId)
        dataspaceAlias = try container.decodeIfPresent(String.self, forKey: .dataspaceAlias)
        manifestHash = try container.decode(String.self, forKey: .manifestHash)
        status = try container.decode(ToriiUaidManifestStatus.self, forKey: .status)
        lifecycle = try container.decode(ToriiUaidManifestLifecycle.self, forKey: .lifecycle)
        accounts = try container.decodeIfPresent([String].self, forKey: .accounts) ?? []
        manifest = try container.decode(ToriiJSONValue.self, forKey: .manifest)
    }
}

public struct ToriiUaidManifestsResponse: Decodable, Sendable {
    public let uaid: String
    public let total: UInt64
    public let manifests: [ToriiUaidManifestRecord]

    private enum CodingKeys: String, CodingKey {
        case uaid
        case total
        case manifests
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        uaid = try container.decode(String.self, forKey: .uaid)
        total = try container.decodeIfPresent(UInt64.self, forKey: .total) ?? 0
        manifests = try container.decodeIfPresent([ToriiUaidManifestRecord].self, forKey: .manifests) ?? []
    }
}

public enum ToriiSpaceDirectoryManifestStatusFilter: String, Sendable {
    case active = "active"
    case inactive = "inactive"
    case all = "all"
}

public struct ToriiUaidManifestQuery: Sendable, Equatable {
    public var dataspaceId: UInt64?
    public var status: ToriiSpaceDirectoryManifestStatusFilter?
    public var limit: UInt64?
    public var offset: UInt64?
    public var addressFormat: String?

    public init(dataspaceId: UInt64? = nil,
                status: ToriiSpaceDirectoryManifestStatusFilter? = nil,
                limit: UInt64? = nil,
                offset: UInt64? = nil,
                addressFormat: String? = nil) {
        self.dataspaceId = dataspaceId
        self.status = status
        self.limit = limit
        self.offset = offset
        self.addressFormat = addressFormat
    }

    public func queryItems() throws -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let dataspaceId {
            items.append(URLQueryItem(name: "dataspace", value: String(dataspaceId)))
        }
        if let status {
            items.append(URLQueryItem(name: "status", value: status.rawValue))
        }
        if let limit {
            items.append(URLQueryItem(name: "limit", value: String(limit)))
        }
        if let offset {
            items.append(URLQueryItem(name: "offset", value: String(offset)))
        }
        if let normalized = try ToriiClient.normalizeAddressFormatQueryValue(addressFormat,
                                                                             context: "ToriiUaidManifestQuery.addressFormat") {
            items.append(URLQueryItem(name: "address_format", value: normalized))
        }
        return items.isEmpty ? nil : items
    }
}

extension ToriiRuntimeUpgradeStatus: Decodable {
    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let map = try container.decode([String: ToriiJSONValue].self)
        guard map.count == 1, let (key, value) = map.first else {
            throw DecodingError.dataCorruptedError(in: container,
                                                   debugDescription: "RuntimeUpgradeStatus expects single-key object")
        }
        switch key {
        case "Proposed":
            self = .proposed
        case "Canceled":
            self = .canceled
        case "ActivatedAt":
            let height: UInt64
            switch value {
            case .number(let number):
                guard number >= 0, number <= Double(UInt64.max), floor(number) == number else {
                    throw DecodingError.dataCorruptedError(in: container,
                                                           debugDescription: "ActivatedAt height must be a non-negative integer within range")
                }
                height = UInt64(number)
            case .string(let string):
                guard let parsed = UInt64(string) else {
                    throw DecodingError.dataCorruptedError(in: container,
                                                           debugDescription: "ActivatedAt height string is invalid")
                }
                height = parsed
            case .null:
                throw DecodingError.dataCorruptedError(in: container,
                                                       debugDescription: "ActivatedAt height missing")
            default:
                throw DecodingError.dataCorruptedError(in: container,
                                                       debugDescription: "ActivatedAt expects numeric height")
            }
            self = .activatedAt(height)
        default:
            throw DecodingError.dataCorruptedError(in: container,
                                                   debugDescription: "Unknown runtime upgrade status \(key)")
        }
    }
}

public struct ToriiRuntimeUpgradeManifest: Codable, Sendable {
    public let name: String
    public let description: String
    public let abiVersion: UInt16
    public let abiHashHex: String
    public let addedSyscalls: [UInt16]
    public let addedPointerTypes: [UInt16]
    public let startHeight: UInt64
    public let endHeight: UInt64

    public var abiHash: Data {
        Data(hexString: abiHashHex) ?? Data()
    }

    private enum CodingKeys: String, CodingKey {
        case name
        case description
        case abiVersion = "abi_version"
        case abiHashHex = "abi_hash"
        case addedSyscalls = "added_syscalls"
        case addedPointerTypes = "added_pointer_types"
        case startHeight = "start_height"
        case endHeight = "end_height"
    }

    public init(name: String,
                description: String,
                abiVersion: UInt16,
                abiHashHex: String,
                addedSyscalls: [UInt16] = [],
                addedPointerTypes: [UInt16] = [],
                startHeight: UInt64,
                endHeight: UInt64) {
        self.name = name
        self.description = description
        self.abiVersion = abiVersion
        self.abiHashHex = abiHashHex.lowercased()
        self.addedSyscalls = addedSyscalls
        self.addedPointerTypes = addedPointerTypes
        self.startHeight = startHeight
        self.endHeight = endHeight
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        name = try container.decode(String.self, forKey: .name)
        description = try container.decode(String.self, forKey: .description)
        abiVersion = try container.decode(UInt16.self, forKey: .abiVersion)
        let hashHex = try container.decode(String.self, forKey: .abiHashHex)
        abiHashHex = try ToriiValidation.normalized32ByteHex(
            hashHex,
            field: "abi_hash",
            codingPath: container.codingPath + [CodingKeys.abiHashHex]
        )
        addedSyscalls = try container.decodeIfPresent([UInt16].self, forKey: .addedSyscalls) ?? []
        addedPointerTypes = try container.decodeIfPresent([UInt16].self, forKey: .addedPointerTypes) ?? []
        startHeight = try container.decode(UInt64.self, forKey: .startHeight)
        endHeight = try container.decode(UInt64.self, forKey: .endHeight)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(name, forKey: .name)
        try container.encode(description, forKey: .description)
        try container.encode(abiVersion, forKey: .abiVersion)
        let normalizedHash = try ToriiRequestValidation.normalized32ByteHex(abiHashHex, field: "abi_hash")
        try container.encode(normalizedHash, forKey: .abiHashHex)
        if !addedSyscalls.isEmpty {
            try container.encode(addedSyscalls, forKey: .addedSyscalls)
        } else {
            try container.encode([UInt16](), forKey: .addedSyscalls)
        }
        if !addedPointerTypes.isEmpty {
            try container.encode(addedPointerTypes, forKey: .addedPointerTypes)
        } else {
            try container.encode([UInt16](), forKey: .addedPointerTypes)
        }
        try container.encode(startHeight, forKey: .startHeight)
        try container.encode(endHeight, forKey: .endHeight)
    }
}

public struct ToriiRuntimeUpgradeRecord: Decodable, Sendable {
    public let manifest: ToriiRuntimeUpgradeManifest
    public let status: ToriiRuntimeUpgradeStatus
    public let proposer: String
    public let createdHeight: UInt64

    private enum CodingKeys: String, CodingKey {
        case manifest
        case status
        case proposer
        case createdHeight = "created_height"
    }
}

public struct ToriiRuntimeUpgradeListItem: Decodable, Sendable {
    public let idHex: String
    public let record: ToriiRuntimeUpgradeRecord

    private enum CodingKeys: String, CodingKey {
        case idHex = "id_hex"
        case record
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let idHex = try container.decode(String.self, forKey: .idHex)
        self.idHex = try ToriiValidation.normalized32ByteHex(
            idHex,
            field: "id_hex",
            codingPath: container.codingPath + [CodingKeys.idHex]
        )
        record = try container.decode(ToriiRuntimeUpgradeRecord.self, forKey: .record)
    }
}

struct ToriiRuntimeUpgradesListResponse: Decodable {
    let items: [ToriiRuntimeUpgradeListItem]
}

public struct ToriiRuntimeInstruction: Codable, Sendable {
    public let wireId: String
    public let payloadHex: String

    private enum CodingKeys: String, CodingKey {
        case wireId = "wire_id"
        case payloadHex = "payload_hex"
    }
}

public struct ToriiRuntimeUpgradeActionResponse: Decodable, Sendable {
    public let ok: Bool
    public let txInstructions: [ToriiRuntimeInstruction]

    private enum CodingKeys: String, CodingKey {
        case ok
        case txInstructions = "tx_instructions"
    }
}

private struct ToriiRuntimeUpgradeManifestEnvelope: Encodable {
    let manifest: ToriiRuntimeUpgradeManifest
}

public enum ToriiVerifyingKeyStatus: String, Codable, Sendable {
    case proposed = "Proposed"
    case active = "Active"
    case withdrawn = "Withdrawn"
}

public struct ToriiVerifyingKeyInline: Decodable, Sendable {
    public let backend: String
    public let bytes: Data

    private enum CodingKeys: String, CodingKey {
        case backend
        case bytesB64 = "bytes_b64"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let rawBackend = try container.decode(String.self, forKey: .backend)
        backend = try ToriiValidation.normalizedBackend(rawBackend,
                                                        field: "backend",
                                                        codingPath: container.codingPath + [CodingKeys.backend])
        let b64 = try container.decode(String.self, forKey: .bytesB64)
        let trimmed = b64.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let data = Data(base64Encoded: trimmed), !data.isEmpty else {
            throw DecodingError.dataCorruptedError(forKey: .bytesB64,
                                                   in: container,
                                                   debugDescription: "Invalid base64 verifying key bytes")
        }
        bytes = data
    }
}

public struct ToriiVerifyingKeyRecord: Decodable, Sendable {
    public let version: Int
    public let circuitId: String
    public let backend: String
    public let curve: String
    public let publicInputsSchemaHashHex: String
    public let commitmentHex: String
    public let verifyingKeyLength: Int
    public let maxProofBytes: Int
    public let gasScheduleId: String?
    public let metadataUriCid: String?
    public let verifyingKeyBytesCid: String?
    public let activationHeight: UInt64?
    public let withdrawHeight: UInt64?
    public let status: ToriiVerifyingKeyStatus
    public let inlineKey: ToriiVerifyingKeyInline?

    private enum CodingKeys: String, CodingKey {
        case version
        case circuitId = "circuit_id"
        case backend
        case curve
        case publicInputsSchemaHashHex = "public_inputs_schema_hash"
        case commitmentHex = "commitment"
        case verifyingKeyLength = "vk_len"
        case maxProofBytes = "max_proof_bytes"
        case gasScheduleId = "gas_schedule_id"
        case metadataUriCid = "metadata_uri_cid"
        case verifyingKeyBytesCid = "vk_bytes_cid"
        case activationHeight = "activation_height"
        case withdrawHeight = "withdraw_height"
        case status
        case inlineKey = "key"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let versionValue = try container.decode(Int.self, forKey: .version)
        version = try ToriiValidation.validatedUInt32(versionValue,
                                                      field: "version",
                                                      codingPath: container.codingPath + [CodingKeys.version])
        let rawCircuitId = try container.decode(String.self, forKey: .circuitId)
        circuitId = try ToriiValidation.normalizedNonEmpty(rawCircuitId,
                                                           field: "circuit_id",
                                                           codingPath: container.codingPath + [CodingKeys.circuitId])
        let rawBackend = try container.decode(String.self, forKey: .backend)
        backend = try ToriiValidation.normalizedBackend(rawBackend,
                                                        field: "backend",
                                                        codingPath: container.codingPath + [CodingKeys.backend])
        let rawCurve = try container.decode(String.self, forKey: .curve)
        curve = try ToriiValidation.normalizedNonEmpty(rawCurve,
                                                       field: "curve",
                                                       codingPath: container.codingPath + [CodingKeys.curve])
        let rawSchema = try container.decode(String.self, forKey: .publicInputsSchemaHashHex)
        publicInputsSchemaHashHex = try ToriiValidation.normalized32ByteHex(
            rawSchema,
            field: "public_inputs_schema_hash",
            codingPath: container.codingPath + [CodingKeys.publicInputsSchemaHashHex]
        )
        let rawCommitment = try container.decode(String.self, forKey: .commitmentHex)
        commitmentHex = try ToriiValidation.normalized32ByteHex(
            rawCommitment,
            field: "commitment",
            codingPath: container.codingPath + [CodingKeys.commitmentHex]
        )
        let vkLenValue = try container.decode(Int.self, forKey: .verifyingKeyLength)
        verifyingKeyLength = try ToriiValidation.validatedUInt32(vkLenValue,
                                                                 field: "vk_len",
                                                                 codingPath: container.codingPath + [CodingKeys.verifyingKeyLength])
        let maxProofValue = try container.decode(Int.self, forKey: .maxProofBytes)
        maxProofBytes = try ToriiValidation.validatedUInt32(maxProofValue,
                                                            field: "max_proof_bytes",
                                                            codingPath: container.codingPath + [CodingKeys.maxProofBytes])
        gasScheduleId = try container.decodeIfPresent(String.self, forKey: .gasScheduleId)
        metadataUriCid = try container.decodeIfPresent(String.self, forKey: .metadataUriCid)
        verifyingKeyBytesCid = try container.decodeIfPresent(String.self, forKey: .verifyingKeyBytesCid)
        activationHeight = try container.decodeIfPresent(UInt64.self, forKey: .activationHeight)
        withdrawHeight = try container.decodeIfPresent(UInt64.self, forKey: .withdrawHeight)
        status = try container.decode(ToriiVerifyingKeyStatus.self, forKey: .status)
        inlineKey = try container.decodeIfPresent(ToriiVerifyingKeyInline.self, forKey: .inlineKey)
        if let inlineKey {
            if inlineKey.backend != backend {
                throw DecodingError.dataCorrupted(
                    DecodingError.Context(
                        codingPath: container.codingPath + [CodingKeys.inlineKey],
                        debugDescription: "inline key backend must match record backend"
                    )
                )
            }
            if verifyingKeyLength != inlineKey.bytes.count {
                throw DecodingError.dataCorrupted(
                    DecodingError.Context(
                        codingPath: container.codingPath + [CodingKeys.verifyingKeyLength],
                        debugDescription: "vk_len must match inline key byte length"
                    )
                )
            }
        }
    }
}

public struct ToriiVerifyingKeyId: Decodable, Sendable {
    public let backend: String
    public let name: String

    private enum CodingKeys: String, CodingKey {
        case backend
        case name
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let rawBackend = try container.decode(String.self, forKey: .backend)
        backend = try ToriiValidation.normalizedBackend(rawBackend,
                                                        field: "backend",
                                                        codingPath: container.codingPath + [CodingKeys.backend])
        let rawName = try container.decode(String.self, forKey: .name)
        let normalizedName = try ToriiValidation.normalizedNonEmpty(rawName,
                                                                    field: "name",
                                                                    codingPath: container.codingPath + [CodingKeys.name])
        if normalizedName.contains(":") {
            throw DecodingError.dataCorrupted(
                DecodingError.Context(
                    codingPath: container.codingPath + [CodingKeys.name],
                    debugDescription: "name must not contain ':' characters"
                )
            )
        }
        name = normalizedName
    }

    public init(backend: String, name: String) throws {
        let trimmedBackend = backend.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedBackend.isEmpty else {
            throw ToriiClientError.invalidPayload("backend must be a non-empty string.")
        }
        if trimmedBackend.contains(":") {
            throw ToriiClientError.invalidPayload("backend must not contain ':' characters.")
        }
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedName.isEmpty else {
            throw ToriiClientError.invalidPayload("name must be a non-empty string.")
        }
        if trimmedName.contains(":") {
            throw ToriiClientError.invalidPayload("name must not contain ':' characters.")
        }
        self.backend = trimmedBackend
        self.name = trimmedName
    }
}

public struct ToriiVerifyingKeyDetail: Decodable, Sendable {
    public let id: ToriiVerifyingKeyId
    public let record: ToriiVerifyingKeyRecord
}

public struct ToriiVerifyingKeyListItem: Decodable, Sendable {
    public let id: ToriiVerifyingKeyId
    public let record: ToriiVerifyingKeyRecord?

    private enum CodingKeys: String, CodingKey {
        case id
        case record
        case backend
        case name
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        if let idObj = try? container.decode(ToriiVerifyingKeyId.self, forKey: .id) {
            id = idObj
        } else {
            let backend = try container.decode(String.self, forKey: .backend)
            let name = try container.decode(String.self, forKey: .name)
            id = try ToriiVerifyingKeyId(backend: backend, name: name)
        }
        record = try container.decodeIfPresent(ToriiVerifyingKeyRecord.self, forKey: .record)
    }
}

private struct ToriiVerifyingKeyListResponse: Decodable {
    let items: [ToriiVerifyingKeyListItem]
}

public struct ToriiVerifyingKeyListQuery: Sendable {
    public var backend: String?
    public var status: ToriiVerifyingKeyStatus?
    public var nameContains: String?
    public var limit: Int?
    public var offset: Int?
    public enum Order: String, Sendable {
        case ascending = "asc"
        case descending = "desc"
    }
    public var order: Order?
    public var idsOnly: Bool?

    public init(backend: String? = nil,
                status: ToriiVerifyingKeyStatus? = nil,
                nameContains: String? = nil,
                limit: Int? = nil,
                offset: Int? = nil,
                order: Order? = nil,
                idsOnly: Bool? = nil) {
        self.backend = backend
        self.status = status
        self.nameContains = nameContains
        self.limit = limit
        self.offset = offset
        self.order = order
        self.idsOnly = idsOnly
    }

    public func queryItems() -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let backend {
            items.append(URLQueryItem(name: "backend", value: backend))
        }
        if let status {
            items.append(URLQueryItem(name: "status", value: status.rawValue))
        }
        if let nameContains {
            items.append(URLQueryItem(name: "name_contains", value: nameContains))
        }
        if let limit, limit >= 0 {
            items.append(URLQueryItem(name: "limit", value: String(limit)))
        }
        if let offset, offset >= 0 {
            items.append(URLQueryItem(name: "offset", value: String(offset)))
        }
        if let order {
            items.append(URLQueryItem(name: "order", value: order.rawValue))
        }
        if let idsOnly {
            items.append(URLQueryItem(name: "ids_only", value: idsOnly ? "true" : "false"))
        }
        return items.isEmpty ? nil : items
    }
}

public struct ToriiVerifyingKeyRegisterRequest: Encodable, Sendable {
    public var authority: String
    public var privateKey: String
    public var backend: String
    public var name: String
    public var version: UInt32
    public var circuitId: String
    public var publicInputsSchemaHashHex: String
    public var curve: String?
    public var gasScheduleId: String
    public var verifyingKeyLength: UInt32?
    public var maxProofBytes: UInt32?
    public var metadataUriCid: String?
    public var verifyingKeyBytesCid: String?
    public var activationHeight: UInt64?
    public var withdrawHeight: UInt64?
    public var commitmentHex: String?
    public var verifyingKeyBytes: Data?
    public var status: ToriiVerifyingKeyStatus?

    public init(authority: String,
                privateKey: String,
                backend: String,
                name: String,
                version: UInt32,
                circuitId: String,
                publicInputsSchemaHashHex: String,
                gasScheduleId: String,
                verifyingKeyBytes: Data? = nil,
                verifyingKeyLength: UInt32? = nil,
                status: ToriiVerifyingKeyStatus? = nil) {
        self.authority = authority
        self.privateKey = privateKey
        self.backend = backend
        self.name = name
        self.version = version
        self.circuitId = circuitId
        self.publicInputsSchemaHashHex = publicInputsSchemaHashHex
        self.curve = nil
        self.gasScheduleId = gasScheduleId
        self.verifyingKeyLength = verifyingKeyLength
        self.maxProofBytes = nil
        self.metadataUriCid = nil
        self.verifyingKeyBytesCid = nil
        self.activationHeight = nil
        self.withdrawHeight = nil
        self.commitmentHex = nil
        self.verifyingKeyBytes = verifyingKeyBytes
        self.status = status
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case privateKey = "private_key"
        case backend
        case name
        case version
        case circuitId = "circuit_id"
        case publicInputsSchemaHashHex = "public_inputs_schema_hash_hex"
        case curve
        case gasScheduleId = "gas_schedule_id"
        case verifyingKeyLength = "vk_len"
        case maxProofBytes = "max_proof_bytes"
        case metadataUriCid = "metadata_uri_cid"
        case verifyingKeyBytesCid = "vk_bytes_cid"
        case activationHeight = "activation_height"
        case withdrawHeight = "withdraw_height"
        case commitmentHex = "commitment_hex"
        case verifyingKeyBytes = "vk_bytes"
        case status
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedAuthority = try ToriiVerifyingKeyRequestValidation.normalizedNonEmpty(authority,
                                                                                            field: "authority")
        let normalizedPrivateKey = try ToriiVerifyingKeyRequestValidation.normalizedNonEmpty(privateKey,
                                                                                              field: "private_key")
        let normalizedBackend = try ToriiVerifyingKeyRequestValidation.normalizedBackend(backend, field: "backend")
        let normalizedName = try ToriiVerifyingKeyRequestValidation.normalizedName(name, field: "name")
        let normalizedCircuitId = try ToriiVerifyingKeyRequestValidation.normalizedNonEmpty(circuitId,
                                                                                             field: "circuit_id")
        let normalizedSchemaHash = try ToriiVerifyingKeyRequestValidation.normalized32ByteHex(
            publicInputsSchemaHashHex,
            field: "public_inputs_schema_hash_hex"
        )
        let normalizedCurve = try ToriiVerifyingKeyRequestValidation.normalizedOptionalNonEmpty(curve,
                                                                                                field: "curve")
        let normalizedGasSchedule = try ToriiVerifyingKeyRequestValidation.normalizedNonEmpty(gasScheduleId,
                                                                                               field: "gas_schedule_id")
        let normalizedCommitment = try ToriiVerifyingKeyRequestValidation.normalizedOptional32ByteHex(
            commitmentHex,
            field: "commitment_hex"
        )
        let normalizedMetadataCid = try ToriiVerifyingKeyRequestValidation.normalizedOptionalNonEmpty(
            metadataUriCid,
            field: "metadata_uri_cid"
        )
        let normalizedVkBytesCid = try ToriiVerifyingKeyRequestValidation.normalizedOptionalNonEmpty(
            verifyingKeyBytesCid,
            field: "vk_bytes_cid"
        )
        let vkPayload = try ToriiVerifyingKeyRequestValidation.verifyingKeyPayload(
            bytes: verifyingKeyBytes,
            explicitLength: verifyingKeyLength
        )
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedAuthority, forKey: .authority)
        try container.encode(normalizedPrivateKey, forKey: .privateKey)
        try container.encode(normalizedBackend, forKey: .backend)
        try container.encode(normalizedName, forKey: .name)
        try container.encode(version, forKey: .version)
        try container.encode(normalizedCircuitId, forKey: .circuitId)
        try container.encode(normalizedSchemaHash, forKey: .publicInputsSchemaHashHex)
        try container.encodeIfPresent(normalizedCurve, forKey: .curve)
        try container.encode(normalizedGasSchedule, forKey: .gasScheduleId)
        if let vkPayload {
            try container.encode(vkPayload.bytesBase64, forKey: .verifyingKeyBytes)
            try container.encode(vkPayload.length, forKey: .verifyingKeyLength)
        } else if let len = verifyingKeyLength {
            try container.encode(len, forKey: .verifyingKeyLength)
        }
        try container.encodeIfPresent(maxProofBytes, forKey: .maxProofBytes)
        try container.encodeIfPresent(normalizedMetadataCid, forKey: .metadataUriCid)
        try container.encodeIfPresent(normalizedVkBytesCid, forKey: .verifyingKeyBytesCid)
        try container.encodeIfPresent(activationHeight, forKey: .activationHeight)
        try container.encodeIfPresent(withdrawHeight, forKey: .withdrawHeight)
        try container.encodeIfPresent(normalizedCommitment, forKey: .commitmentHex)
        if let status {
            try container.encode(status.rawValue, forKey: .status)
        }
    }
}

public struct ToriiVerifyingKeyUpdateRequest: Encodable, Sendable {
    public var authority: String
    public var privateKey: String
    public var backend: String
    public var name: String
    public var version: UInt32
    public var circuitId: String
    public var publicInputsSchemaHashHex: String
    public var curve: String?
    public var gasScheduleId: String?
    public var commitmentHex: String?
    public var verifyingKeyLength: UInt32?
    public var maxProofBytes: UInt32?
    public var metadataUriCid: String?
    public var verifyingKeyBytesCid: String?
    public var activationHeight: UInt64?
    public var withdrawHeight: UInt64?
    public var verifyingKeyBytes: Data?
    public var status: ToriiVerifyingKeyStatus?

    public init(authority: String,
                privateKey: String,
                backend: String,
                name: String,
                version: UInt32,
                circuitId: String,
                publicInputsSchemaHashHex: String) {
        self.authority = authority
        self.privateKey = privateKey
        self.backend = backend
        self.name = name
        self.version = version
        self.circuitId = circuitId
        self.publicInputsSchemaHashHex = publicInputsSchemaHashHex
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case privateKey = "private_key"
        case backend
        case name
        case version
        case circuitId = "circuit_id"
        case publicInputsSchemaHashHex = "public_inputs_schema_hash_hex"
        case curve
        case gasScheduleId = "gas_schedule_id"
        case commitmentHex = "commitment_hex"
        case verifyingKeyLength = "vk_len"
        case maxProofBytes = "max_proof_bytes"
        case metadataUriCid = "metadata_uri_cid"
        case verifyingKeyBytesCid = "vk_bytes_cid"
        case activationHeight = "activation_height"
        case withdrawHeight = "withdraw_height"
        case verifyingKeyBytes = "vk_bytes"
        case status
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedAuthority = try ToriiVerifyingKeyRequestValidation.normalizedNonEmpty(authority,
                                                                                            field: "authority")
        let normalizedPrivateKey = try ToriiVerifyingKeyRequestValidation.normalizedNonEmpty(privateKey,
                                                                                              field: "private_key")
        let normalizedBackend = try ToriiVerifyingKeyRequestValidation.normalizedBackend(backend, field: "backend")
        let normalizedName = try ToriiVerifyingKeyRequestValidation.normalizedName(name, field: "name")
        let normalizedCircuitId = try ToriiVerifyingKeyRequestValidation.normalizedNonEmpty(circuitId,
                                                                                             field: "circuit_id")
        let normalizedSchemaHash = try ToriiVerifyingKeyRequestValidation.normalized32ByteHex(
            publicInputsSchemaHashHex,
            field: "public_inputs_schema_hash_hex"
        )
        let normalizedCurve = try ToriiVerifyingKeyRequestValidation.normalizedOptionalNonEmpty(curve,
                                                                                                field: "curve")
        let normalizedGasSchedule = try ToriiVerifyingKeyRequestValidation.normalizedOptionalNonEmpty(gasScheduleId,
                                                                                                      field: "gas_schedule_id")
        let normalizedCommitment = try ToriiVerifyingKeyRequestValidation.normalizedOptional32ByteHex(
            commitmentHex,
            field: "commitment_hex"
        )
        let normalizedMetadataCid = try ToriiVerifyingKeyRequestValidation.normalizedOptionalNonEmpty(
            metadataUriCid,
            field: "metadata_uri_cid"
        )
        let normalizedVkBytesCid = try ToriiVerifyingKeyRequestValidation.normalizedOptionalNonEmpty(
            verifyingKeyBytesCid,
            field: "vk_bytes_cid"
        )
        let vkPayload = try ToriiVerifyingKeyRequestValidation.verifyingKeyPayload(
            bytes: verifyingKeyBytes,
            explicitLength: verifyingKeyLength
        )
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedAuthority, forKey: .authority)
        try container.encode(normalizedPrivateKey, forKey: .privateKey)
        try container.encode(normalizedBackend, forKey: .backend)
        try container.encode(normalizedName, forKey: .name)
        try container.encode(version, forKey: .version)
        try container.encode(normalizedCircuitId, forKey: .circuitId)
        try container.encode(normalizedSchemaHash, forKey: .publicInputsSchemaHashHex)
        try container.encodeIfPresent(normalizedCurve, forKey: .curve)
        try container.encodeIfPresent(normalizedGasSchedule, forKey: .gasScheduleId)
        try container.encodeIfPresent(normalizedCommitment, forKey: .commitmentHex)
        if let vkPayload {
            try container.encode(vkPayload.bytesBase64, forKey: .verifyingKeyBytes)
            try container.encode(vkPayload.length, forKey: .verifyingKeyLength)
        } else if let len = verifyingKeyLength {
            try container.encode(len, forKey: .verifyingKeyLength)
        }
        try container.encodeIfPresent(maxProofBytes, forKey: .maxProofBytes)
        try container.encodeIfPresent(normalizedMetadataCid, forKey: .metadataUriCid)
        try container.encodeIfPresent(normalizedVkBytesCid, forKey: .verifyingKeyBytesCid)
        try container.encodeIfPresent(activationHeight, forKey: .activationHeight)
        try container.encodeIfPresent(withdrawHeight, forKey: .withdrawHeight)
        if let status {
            try container.encode(status.rawValue, forKey: .status)
        }
    }
}

fileprivate enum ToriiVerifyingKeyRequestValidation {
    struct VkPayload {
        let bytesBase64: String
        let length: UInt32
    }

    static func normalizedNonEmpty(_ value: String, field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("\(field) must be a non-empty string.")
        }
        return trimmed
    }

    static func normalizedBackend(_ value: String, field: String) throws -> String {
        let trimmed = try normalizedNonEmpty(value, field: field)
        if trimmed.contains(":") {
            throw ToriiClientError.invalidPayload("\(field) must not contain ':' characters.")
        }
        return trimmed
    }

    static func normalizedName(_ value: String, field: String) throws -> String {
        let trimmed = try normalizedNonEmpty(value, field: field)
        if trimmed.contains(":") {
            throw ToriiClientError.invalidPayload("\(field) must not contain ':' characters.")
        }
        return trimmed
    }

    static func normalizedOptionalNonEmpty(_ value: String?, field: String) throws -> String? {
        guard let value else { return nil }
        return try normalizedNonEmpty(value, field: field)
    }

    static func normalized32ByteHex(_ value: String, field: String) throws -> String {
        var trimmed = try normalizedNonEmpty(value, field: field)
        if trimmed.hasPrefix("0x") || trimmed.hasPrefix("0X") {
            trimmed = String(trimmed.dropFirst(2))
        }
        guard trimmed.count == 64, Data(hexString: trimmed) != nil else {
            throw ToriiClientError.invalidPayload("\(field) must be a 32-byte hex string.")
        }
        return trimmed.lowercased()
    }

    static func normalizedOptional32ByteHex(_ value: String?, field: String) throws -> String? {
        guard let value else { return nil }
        return try normalized32ByteHex(value, field: field)
    }

    static func verifyingKeyPayload(bytes: Data?, explicitLength: UInt32?) throws -> VkPayload? {
        guard let bytes else {
            return nil
        }
        guard !bytes.isEmpty else {
            throw ToriiClientError.invalidPayload("vk_bytes must not be empty.")
        }
        guard bytes.count <= Int(UInt32.max) else {
            throw ToriiClientError.invalidPayload("vk_bytes length must fit in a u32.")
        }
        let length = UInt32(bytes.count)
        if let explicitLength, explicitLength != length {
            throw ToriiClientError.invalidPayload("vk_len must match vk_bytes length.")
        }
        return VkPayload(bytesBase64: bytes.base64EncodedString(),
                         length: explicitLength ?? length)
    }
}

fileprivate enum ToriiRequestValidation {
    static func normalizedNonEmpty(_ value: String, field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("\(field) must be a non-empty string.")
        }
        return trimmed
    }

    static func normalizedOptionalNonEmpty(_ value: String?, field: String) throws -> String? {
        guard let value else { return nil }
        return try normalizedNonEmpty(value, field: field)
    }

    static func normalized32ByteHex(_ value: String, field: String) throws -> String {
        var trimmed = try normalizedNonEmpty(value, field: field)
        if trimmed.hasPrefix("0x") || trimmed.hasPrefix("0X") {
            trimmed = String(trimmed.dropFirst(2))
        }
        guard trimmed.count == 64, Data(hexString: trimmed) != nil else {
            throw ToriiClientError.invalidPayload("\(field) must be a 32-byte hex string.")
        }
        return trimmed.lowercased()
    }

    static func normalizedOptional32ByteHex(_ value: String?, field: String) throws -> String? {
        guard let value else { return nil }
        return try normalized32ByteHex(value, field: field)
    }

    static func normalizedBase64(_ value: String, field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("\(field) must be a non-empty base64 string.")
        }
        guard let data = Data(base64Encoded: trimmed), !data.isEmpty else {
            throw ToriiClientError.invalidPayload("\(field) must be valid base64.")
        }
        return trimmed
    }
}

public enum ToriiVerifyingKeyEvent: Sendable {
    case registered(id: ToriiVerifyingKeyId, record: ToriiVerifyingKeyRecord)
    case updated(id: ToriiVerifyingKeyId, record: ToriiVerifyingKeyRecord)
}

public struct ToriiVerifyingKeyEventMessage: Sendable {
    public let event: ToriiVerifyingKeyEvent
    public let eventName: String?
    public let eventId: String?
    public let retryHintMilliseconds: Int?
    public let rawEvent: String
}

public struct ToriiVerifyingKeyEventFilter: Sendable {
    public var backend: String?
    public var name: String?
    public var includeRegistered: Bool
    public var includeUpdated: Bool

    public init(backend: String? = nil,
                name: String? = nil,
                includeRegistered: Bool = true,
                includeUpdated: Bool = true) {
        self.backend = backend
        self.name = name
        self.includeRegistered = includeRegistered
        self.includeUpdated = includeUpdated
    }

    public func queryItems() throws -> [URLQueryItem]? {
        guard includeRegistered || includeUpdated else {
            throw ToriiClientError.invalidPayload("Enable at least one verifying key event type.")
        }
        var body: [String: Any] = [
            "event_set": [
                "Registered": includeRegistered,
                "Updated": includeUpdated,
            ],
        ]

        if backend != nil || name != nil {
            guard let backend, let name else {
                throw ToriiClientError.invalidPayload(
                    "Provide both backend and name when filtering verifying key events by id."
                )
            }
            let normalizedBackend = try Self.normalizedIdComponent(backend, field: "backend")
            let normalizedName = try Self.normalizedIdComponent(name, field: "name")
            body["id_matcher"] = ["backend": normalizedBackend, "name": normalizedName]
        }

        let filterPayload: [String: Any] = ["VerifyingKey": body]
        let data = try JSONSerialization.data(withJSONObject: filterPayload, options: [])
        guard let json = String(data: data, encoding: .utf8) else {
            throw ToriiClientError.invalidPayload("Failed to encode verifying key event filter.")
        }
        return [URLQueryItem(name: "filter", value: json)]
    }

    private static func normalizedIdComponent(_ value: String, field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("\(field) must be a non-empty string.")
        }
        if trimmed.contains(":") {
            throw ToriiClientError.invalidPayload("\(field) must not contain ':' characters.")
        }
        return trimmed
    }
}

public struct ToriiProofId: Decodable, Sendable {
    public let backend: String
    public let proofHashHex: String

    public init(backend: String, proofHashHex: String) {
        self.backend = backend
        self.proofHashHex = proofHashHex.lowercased()
    }

    private enum CodingKeys: String, CodingKey {
        case backend
        case proofHashHex = "proof_hash_hex"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let backend = try container.decode(String.self, forKey: .backend)
        let normalizedBackend = try ToriiValidation.normalizedBackend(
            backend,
            field: "backend",
            codingPath: container.codingPath + [CodingKeys.backend]
        )
        let hashHex = try container.decode(String.self, forKey: .proofHashHex)
        let normalizedHash = try ToriiValidation.normalized32ByteHex(
            hashHex,
            field: "proof_hash_hex",
            codingPath: container.codingPath + [CodingKeys.proofHashHex]
        )
        self.backend = normalizedBackend
        self.proofHashHex = normalizedHash
    }
}

fileprivate enum ToriiValidation {
    static func normalizedNonEmpty(_ value: String,
                                   field: String,
                                   codingPath: [CodingKey]) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw DecodingError.dataCorrupted(
                DecodingError.Context(
                    codingPath: codingPath,
                    debugDescription: "\(field) must be a non-empty string"
                )
            )
        }
        return trimmed
    }

    static func normalizedBackend(_ value: String,
                                  field: String,
                                  codingPath: [CodingKey]) throws -> String {
        let trimmed = try normalizedNonEmpty(value, field: field, codingPath: codingPath)
        if trimmed.contains(":") {
            throw DecodingError.dataCorrupted(
                DecodingError.Context(
                    codingPath: codingPath,
                    debugDescription: "\(field) must not contain ':' characters"
                )
            )
        }
        return trimmed
    }

    static func normalized32ByteHex(_ value: String,
                                    field: String,
                                    codingPath: [CodingKey]) throws -> String {
        var trimmed = try normalizedNonEmpty(value, field: field, codingPath: codingPath)
        if trimmed.hasPrefix("0x") || trimmed.hasPrefix("0X") {
            trimmed = String(trimmed.dropFirst(2))
        }
        guard trimmed.count == 64, Data(hexString: trimmed) != nil else {
            throw DecodingError.dataCorrupted(
                DecodingError.Context(
                    codingPath: codingPath,
                    debugDescription: "\(field) must be a 32-byte hex string"
                )
            )
        }
        return trimmed.lowercased()
    }

    static func normalizedBase64(_ value: String,
                                 field: String,
                                 codingPath: [CodingKey]) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let data = Data(base64Encoded: trimmed), !data.isEmpty else {
            throw DecodingError.dataCorrupted(
                DecodingError.Context(
                    codingPath: codingPath,
                    debugDescription: "\(field) must be valid base64"
                )
            )
        }
        return trimmed
    }

    static func validatedUInt32(_ value: Int,
                                field: String,
                                codingPath: [CodingKey]) throws -> Int {
        guard value >= 0, value <= Int(UInt32.max) else {
            throw DecodingError.dataCorrupted(
                DecodingError.Context(
                    codingPath: codingPath,
                    debugDescription: "\(field) must fit in a u32"
                )
            )
        }
        return value
    }
}

public struct ToriiProofEventBody: Decodable, Sendable {
    public let id: ToriiProofId
    public let verifyingKeyId: ToriiVerifyingKeyId?
    public let verifyingKeyCommitmentHex: String?
    public let callHashHex: String?
    public let envelopeHashHex: String?

    private enum CodingKeys: String, CodingKey {
        case id
        case verifyingKeyId = "vk_ref"
        case verifyingKeyCommitmentHex = "vk_commitment"
        case callHashHex = "call_hash"
        case envelopeHashHex = "envelope_hash"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(ToriiProofId.self, forKey: .id)
        verifyingKeyId = try container.decodeIfPresent(ToriiVerifyingKeyId.self, forKey: .verifyingKeyId)
        if let commitment = try container.decodeIfPresent(String.self, forKey: .verifyingKeyCommitmentHex) {
            verifyingKeyCommitmentHex = try ToriiValidation.normalized32ByteHex(
                commitment,
                field: "vk_commitment",
                codingPath: container.codingPath + [CodingKeys.verifyingKeyCommitmentHex]
            )
        } else {
            verifyingKeyCommitmentHex = nil
        }
        if let callHash = try container.decodeIfPresent(String.self, forKey: .callHashHex) {
            callHashHex = try ToriiValidation.normalized32ByteHex(
                callHash,
                field: "call_hash",
                codingPath: container.codingPath + [CodingKeys.callHashHex]
            )
        } else {
            callHashHex = nil
        }
        if let envelopeHash = try container.decodeIfPresent(String.self, forKey: .envelopeHashHex) {
            envelopeHashHex = try ToriiValidation.normalized32ByteHex(
                envelopeHash,
                field: "envelope_hash",
                codingPath: container.codingPath + [CodingKeys.envelopeHashHex]
            )
        } else {
            envelopeHashHex = nil
        }
    }
}

public enum ToriiProofEvent: Sendable {
    case verified(ToriiProofEventBody)
    case rejected(ToriiProofEventBody)
}

public struct ToriiProofEventMessage: Sendable {
    public let event: ToriiProofEvent
    public let eventName: String?
    public let eventId: String?
    public let retryHintMilliseconds: Int?
    public let rawEvent: String
}

public struct ToriiProofEventFilter: Sendable {
    public var backend: String?
    public var proofHashHex: String?
    public var includeVerified: Bool
    public var includeRejected: Bool

    public init(backend: String? = nil,
                proofHashHex: String? = nil,
                includeVerified: Bool = true,
                includeRejected: Bool = true) {
        self.backend = backend
        self.proofHashHex = proofHashHex
        self.includeVerified = includeVerified
        self.includeRejected = includeRejected
    }

    public func queryItems() throws -> [URLQueryItem]? {
        guard includeVerified || includeRejected else {
            throw ToriiClientError.invalidPayload("Enable at least one proof event type.")
        }

        if backend != nil || proofHashHex != nil {
            guard let backend, let proofHashHex else {
                throw ToriiClientError.invalidPayload(
                    "Provide both backend and proofHashHex when filtering proof events by id."
                )
            }
            let normalizedBackend = try Self.normalizedBackend(backend)
            let normalizedHash = try Self.normalizedHashHex(proofHashHex)
            let filterPayload: [String: Any] = [
                "Proof": [
                    "id_matcher": ["backend": normalizedBackend, "hash_hex": normalizedHash],
                    "event_set": [
                        "Verified": includeVerified,
                        "Rejected": includeRejected,
                    ],
                ],
            ]
            let data = try JSONSerialization.data(withJSONObject: filterPayload, options: [])
            guard let json = String(data: data, encoding: .utf8) else {
                throw ToriiClientError.invalidPayload("Failed to encode proof event filter.")
            }
            return [URLQueryItem(name: "filter", value: json)]
        }

        let filterPayload: [String: Any] = [
            "Proof": [
                "event_set": [
                    "Verified": includeVerified,
                    "Rejected": includeRejected,
                ],
            ],
        ]
        let data = try JSONSerialization.data(withJSONObject: filterPayload, options: [])
        guard let json = String(data: data, encoding: .utf8) else {
            throw ToriiClientError.invalidPayload("Failed to encode proof event filter.")
        }
        return [URLQueryItem(name: "filter", value: json)]
    }

    private static func normalizedBackend(_ value: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("backend must be a non-empty string.")
        }
        if trimmed.contains(":") {
            throw ToriiClientError.invalidPayload("backend must not contain ':' characters.")
        }
        return trimmed
    }

    private static func normalizedHashHex(_ value: String) throws -> String {
        var trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("proofHashHex must be a non-empty string.")
        }
        if trimmed.hasPrefix("0x") || trimmed.hasPrefix("0X") {
            trimmed = String(trimmed.dropFirst(2))
        }
        guard trimmed.count == 64, Data(hexString: trimmed) != nil else {
            throw ToriiClientError.invalidPayload("Expected 32-byte proof hash in hex form.")
        }
        return trimmed.lowercased()
    }
}

public struct ToriiTriggerNumberOfExecutionsChanged: Decodable, Sendable {
    public let triggerId: String
    public let delta: UInt32

    private enum CodingKeys: String, CodingKey {
        case trigger
        case by
    }

    public init(triggerId: String, delta: UInt32) {
        self.triggerId = triggerId
        self.delta = delta
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        triggerId = try container.decode(String.self, forKey: .trigger)
        delta = try container.decode(UInt32.self, forKey: .by)
    }
}

public struct ToriiTriggerMetadataChanged: Decodable, Sendable {
    public let triggerId: String
    public let key: String
    public let value: ToriiJSONValue

    private enum CodingKeys: String, CodingKey {
        case target
        case key
        case value
    }

    public init(triggerId: String, key: String, value: ToriiJSONValue) {
        self.triggerId = triggerId
        self.key = key
        self.value = value
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        triggerId = try container.decode(String.self, forKey: .target)
        key = try container.decode(String.self, forKey: .key)
        value = try container.decode(ToriiJSONValue.self, forKey: .value)
    }
}

public enum ToriiTriggerEvent: Sendable {
    case created(triggerId: String)
    case deleted(triggerId: String)
    case extended(ToriiTriggerNumberOfExecutionsChanged)
    case shortened(ToriiTriggerNumberOfExecutionsChanged)
    case metadataInserted(ToriiTriggerMetadataChanged)
    case metadataRemoved(ToriiTriggerMetadataChanged)
}

public struct ToriiTriggerEventMessage: Sendable {
    public let event: ToriiTriggerEvent
    public let eventName: String?
    public let eventId: String?
    public let retryHintMilliseconds: Int?
    public let rawEvent: String
}

public struct ToriiTriggerEventFilter: Sendable {
    public var triggerId: String?
    public var includeCreated: Bool
    public var includeDeleted: Bool
    public var includeExtended: Bool
    public var includeShortened: Bool
    public var includeMetadataInserted: Bool
    public var includeMetadataRemoved: Bool

    public init(triggerId: String? = nil,
                includeCreated: Bool = true,
                includeDeleted: Bool = true,
                includeExtended: Bool = true,
                includeShortened: Bool = true,
                includeMetadataInserted: Bool = true,
                includeMetadataRemoved: Bool = true) {
        self.triggerId = triggerId
        self.includeCreated = includeCreated
        self.includeDeleted = includeDeleted
        self.includeExtended = includeExtended
        self.includeShortened = includeShortened
        self.includeMetadataInserted = includeMetadataInserted
        self.includeMetadataRemoved = includeMetadataRemoved
    }

    public func queryItems() throws -> [URLQueryItem]? {
        guard includeCreated || includeDeleted || includeExtended || includeShortened || includeMetadataInserted || includeMetadataRemoved else {
            throw ToriiClientError.invalidPayload("Enable at least one trigger event type.")
        }
        var body: [String: Any] = [
            "event_set": [
                "Created": includeCreated,
                "Deleted": includeDeleted,
                "Extended": includeExtended,
                "Shortened": includeShortened,
                "MetadataInserted": includeMetadataInserted,
                "MetadataRemoved": includeMetadataRemoved,
            ],
        ]

        if let triggerId {
            body["id_matcher"] = triggerId
        }

        let filterPayload: [String: Any] = ["Trigger": body]
        let data = try JSONSerialization.data(withJSONObject: filterPayload, options: [])
        guard let json = String(data: data, encoding: .utf8) else {
            throw ToriiClientError.invalidPayload("Failed to encode trigger event filter.")
        }
        return [URLQueryItem(name: "filter", value: json)]
    }
}

public struct ToriiContractAccessSetHints: Codable, Sendable, Equatable {
    public var readKeys: [String]
    public var writeKeys: [String]

    public init(readKeys: [String] = [], writeKeys: [String] = []) {
        self.readKeys = readKeys
        self.writeKeys = writeKeys
    }

    private enum CodingKeys: String, CodingKey {
        case readKeys = "read_keys"
        case writeKeys = "write_keys"
    }
}

public struct ToriiContractManifest: Codable, Sendable, Equatable {
    public var codeHash: String?
    public var abiHash: String?
    public var compilerFingerprint: String?
    public var featuresBitmap: UInt64?
    public var accessSetHints: ToriiContractAccessSetHints?

    public init(codeHash: String? = nil,
                abiHash: String? = nil,
                compilerFingerprint: String? = nil,
                featuresBitmap: UInt64? = nil,
                accessSetHints: ToriiContractAccessSetHints? = nil) {
        self.codeHash = codeHash
        self.abiHash = abiHash
        self.compilerFingerprint = compilerFingerprint
        self.featuresBitmap = featuresBitmap
        self.accessSetHints = accessSetHints
    }

    private enum CodingKeys: String, CodingKey {
        case codeHash = "code_hash"
        case abiHash = "abi_hash"
        case compilerFingerprint = "compiler_fingerprint"
        case featuresBitmap = "features_bitmap"
        case accessSetHints = "access_set_hints"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        if let codeHash = try container.decodeIfPresent(String.self, forKey: .codeHash) {
            self.codeHash = try ToriiValidation.normalized32ByteHex(
                codeHash,
                field: "code_hash",
                codingPath: container.codingPath + [CodingKeys.codeHash]
            )
        } else {
            self.codeHash = nil
        }
        if let abiHash = try container.decodeIfPresent(String.self, forKey: .abiHash) {
            self.abiHash = try ToriiValidation.normalized32ByteHex(
                abiHash,
                field: "abi_hash",
                codingPath: container.codingPath + [CodingKeys.abiHash]
            )
        } else {
            self.abiHash = nil
        }
        compilerFingerprint = try container.decodeIfPresent(String.self, forKey: .compilerFingerprint)
        featuresBitmap = try container.decodeIfPresent(UInt64.self, forKey: .featuresBitmap)
        accessSetHints = try container.decodeIfPresent(ToriiContractAccessSetHints.self,
                                                       forKey: .accessSetHints)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        if let codeHash {
            try container.encode(
                try ToriiRequestValidation.normalized32ByteHex(codeHash, field: "code_hash"),
                forKey: .codeHash
            )
        }
        if let abiHash {
            try container.encode(
                try ToriiRequestValidation.normalized32ByteHex(abiHash, field: "abi_hash"),
                forKey: .abiHash
            )
        }
        try container.encodeIfPresent(compilerFingerprint, forKey: .compilerFingerprint)
        try container.encodeIfPresent(featuresBitmap, forKey: .featuresBitmap)
        try container.encodeIfPresent(accessSetHints, forKey: .accessSetHints)
    }
}

public struct ToriiRegisterContractCodeRequest: Encodable, Sendable {
    public typealias Manifest = ToriiContractManifest

    public var authority: String
    public var privateKey: String
    public var manifest: Manifest
    public var codeBytes: String?

    public init(authority: String,
                privateKey: String,
                manifest: Manifest,
                codeBytes: String? = nil) {
        self.authority = authority
        self.privateKey = privateKey
        self.manifest = manifest
        self.codeBytes = codeBytes
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case privateKey = "private_key"
        case manifest
    }

    public func encode(to encoder: Encoder) throws {
        if codeBytes != nil {
            throw ToriiClientError.invalidPayload("code_bytes is not accepted by /v1/contracts/code")
        }
        let normalizedAuthority = try ToriiRequestValidation.normalizedNonEmpty(authority,
                                                                                field: "authority")
        let normalizedPrivateKey = try ToriiRequestValidation.normalizedNonEmpty(privateKey,
                                                                                  field: "private_key")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedAuthority, forKey: .authority)
        try container.encode(normalizedPrivateKey, forKey: .privateKey)
        try container.encode(manifest, forKey: .manifest)
    }
}

public struct ToriiContractManifestRecord: Decodable, Sendable {
    public let manifest: ToriiContractManifest
    public let codeBytes: String?

    private enum CodingKeys: String, CodingKey {
        case manifest
        case codeBytes = "code_bytes"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        manifest = try container.decode(ToriiContractManifest.self, forKey: .manifest)
        if let codeBytes = try container.decodeIfPresent(String.self, forKey: .codeBytes) {
            self.codeBytes = try ToriiValidation.normalizedBase64(
                codeBytes,
                field: "code_bytes",
                codingPath: container.codingPath + [CodingKeys.codeBytes]
            )
        } else {
            self.codeBytes = nil
        }
    }
}

public struct ToriiDeployContractRequest: Encodable, Sendable {
    public var authority: String
    public var privateKey: String
    public var codeB64: String
    public var codeHash: String?
    public var abiHash: String?
    public var manifest: ToriiContractManifest?

    public init(authority: String,
                privateKey: String,
                codeB64: String,
                codeHash: String? = nil,
                abiHash: String? = nil,
                manifest: ToriiContractManifest? = nil) {
        self.authority = authority
        self.privateKey = privateKey
        self.codeB64 = codeB64
        self.codeHash = codeHash
        self.abiHash = abiHash
        self.manifest = manifest
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case privateKey = "private_key"
        case codeB64 = "code_b64"
        case codeHash = "code_hash"
        case abiHash = "abi_hash"
        case manifest
    }

    public func encode(to encoder: Encoder) throws {
        if codeHash != nil || abiHash != nil || manifest != nil {
            throw ToriiClientError.invalidPayload("code_hash, abi_hash, and manifest are not accepted by /v1/contracts/deploy")
        }
        let normalizedAuthority = try ToriiRequestValidation.normalizedNonEmpty(authority,
                                                                                field: "authority")
        let normalizedPrivateKey = try ToriiRequestValidation.normalizedNonEmpty(privateKey,
                                                                                  field: "private_key")
        let normalizedCodeB64 = try ToriiRequestValidation.normalizedBase64(codeB64, field: "code_b64")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedAuthority, forKey: .authority)
        try container.encode(normalizedPrivateKey, forKey: .privateKey)
        try container.encode(normalizedCodeB64, forKey: .codeB64)
    }
}

public struct ToriiDeployContractResponse: Decodable, Sendable {
    public let ok: Bool
    public let codeHashHex: String?
    public let abiHashHex: String?

    private enum CodingKeys: String, CodingKey {
        case ok
        case codeHashHex = "code_hash_hex"
        case abiHashHex = "abi_hash_hex"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        ok = try container.decode(Bool.self, forKey: .ok)
        if let codeHashHex = try container.decodeIfPresent(String.self, forKey: .codeHashHex) {
            self.codeHashHex = try ToriiValidation.normalized32ByteHex(
                codeHashHex,
                field: "code_hash_hex",
                codingPath: container.codingPath + [CodingKeys.codeHashHex]
            )
        } else {
            self.codeHashHex = nil
        }
        if let abiHashHex = try container.decodeIfPresent(String.self, forKey: .abiHashHex) {
            self.abiHashHex = try ToriiValidation.normalized32ByteHex(
                abiHashHex,
                field: "abi_hash_hex",
                codingPath: container.codingPath + [CodingKeys.abiHashHex]
            )
        } else {
            self.abiHashHex = nil
        }
    }
}

public struct ToriiContractCodeBytes: Decodable, Sendable {
    public let codeB64: String

    private enum CodingKeys: String, CodingKey {
        case codeB64 = "code_b64"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let codeB64 = try container.decode(String.self, forKey: .codeB64)
        self.codeB64 = try ToriiValidation.normalizedBase64(codeB64,
                                                            field: "code_b64",
                                                            codingPath: container.codingPath + [CodingKeys.codeB64])
    }
}

public struct ToriiDeployContractInstanceRequest: Encodable, Sendable {
    public var authority: String
    public var privateKey: String
    public var namespace: String
    public var contractId: String
    public var codeB64: String
    public var manifest: ToriiContractManifest?

    public init(authority: String,
                privateKey: String,
                namespace: String,
                contractId: String,
                codeB64: String,
                manifest: ToriiContractManifest? = nil) {
        self.authority = authority
        self.privateKey = privateKey
        self.namespace = namespace
        self.contractId = contractId
        self.codeB64 = codeB64
        self.manifest = manifest
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case privateKey = "private_key"
        case namespace
        case contractId = "contract_id"
        case codeB64 = "code_b64"
        case manifest
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedAuthority = try ToriiRequestValidation.normalizedNonEmpty(authority,
                                                                                field: "authority")
        let normalizedPrivateKey = try ToriiRequestValidation.normalizedNonEmpty(privateKey,
                                                                                  field: "private_key")
        let normalizedNamespace = try ToriiRequestValidation.normalizedNonEmpty(namespace,
                                                                                field: "namespace")
        let normalizedContractId = try ToriiRequestValidation.normalizedNonEmpty(contractId,
                                                                                 field: "contract_id")
        let normalizedCodeB64 = try ToriiRequestValidation.normalizedBase64(codeB64, field: "code_b64")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedAuthority, forKey: .authority)
        try container.encode(normalizedPrivateKey, forKey: .privateKey)
        try container.encode(normalizedNamespace, forKey: .namespace)
        try container.encode(normalizedContractId, forKey: .contractId)
        try container.encode(normalizedCodeB64, forKey: .codeB64)
        try container.encodeIfPresent(manifest, forKey: .manifest)
    }
}

public struct ToriiDeployContractInstanceResponse: Decodable, Sendable {
    public let ok: Bool
    public let namespace: String
    public let contractId: String
    public let codeHashHex: String
    public let abiHashHex: String

    private enum CodingKeys: String, CodingKey {
        case ok
        case namespace
        case contractId = "contract_id"
        case codeHashHex = "code_hash_hex"
        case abiHashHex = "abi_hash_hex"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        ok = try container.decode(Bool.self, forKey: .ok)
        namespace = try container.decode(String.self, forKey: .namespace)
        contractId = try container.decode(String.self, forKey: .contractId)
        let codeHashHex = try container.decode(String.self, forKey: .codeHashHex)
        self.codeHashHex = try ToriiValidation.normalized32ByteHex(
            codeHashHex,
            field: "code_hash_hex",
            codingPath: container.codingPath + [CodingKeys.codeHashHex]
        )
        let abiHashHex = try container.decode(String.self, forKey: .abiHashHex)
        self.abiHashHex = try ToriiValidation.normalized32ByteHex(
            abiHashHex,
            field: "abi_hash_hex",
            codingPath: container.codingPath + [CodingKeys.abiHashHex]
        )
    }
}

public struct ToriiActivateContractInstanceRequest: Encodable, Sendable {
    public var authority: String
    public var privateKey: String
    public var namespace: String
    public var contractId: String
    public var codeHash: String

    public init(authority: String,
                privateKey: String,
                namespace: String,
                contractId: String,
                codeHash: String) {
        self.authority = authority
        self.privateKey = privateKey
        self.namespace = namespace
        self.contractId = contractId
        self.codeHash = codeHash
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case privateKey = "private_key"
        case namespace
        case contractId = "contract_id"
        case codeHash = "code_hash"
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedAuthority = try ToriiRequestValidation.normalizedNonEmpty(authority,
                                                                                field: "authority")
        let normalizedPrivateKey = try ToriiRequestValidation.normalizedNonEmpty(privateKey,
                                                                                  field: "private_key")
        let normalizedNamespace = try ToriiRequestValidation.normalizedNonEmpty(namespace,
                                                                                field: "namespace")
        let normalizedContractId = try ToriiRequestValidation.normalizedNonEmpty(contractId,
                                                                                 field: "contract_id")
        let normalizedCodeHash = try ToriiRequestValidation.normalized32ByteHex(codeHash, field: "code_hash")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedAuthority, forKey: .authority)
        try container.encode(normalizedPrivateKey, forKey: .privateKey)
        try container.encode(normalizedNamespace, forKey: .namespace)
        try container.encode(normalizedContractId, forKey: .contractId)
        try container.encode(normalizedCodeHash, forKey: .codeHash)
    }
}

public struct ToriiActivateContractInstanceResponse: Decodable, Sendable {
    public let ok: Bool
}

public enum ToriiMetricsResponse: Equatable, Sendable {
    case text(String)
    case json(ToriiJSONValue)

    public var text: String? {
        if case let .text(value) = self {
            return value
        }
        return nil
    }

    public var json: ToriiJSONValue? {
        if case let .json(value) = self {
            return value
        }
        return nil
    }
}

public enum ToriiJSONValue: Codable, Sendable, Equatable {
    case string(String)
    case number(Double)
    case bool(Bool)
    case array([ToriiJSONValue])
    case object([String: ToriiJSONValue])
    case null

    public init(from decoder: Decoder) throws {
        if let container = try? decoder.singleValueContainer() {
            if container.decodeNil() {
                self = .null
            } else if let stringValue = try? container.decode(String.self) {
                self = .string(stringValue)
            } else if let boolValue = try? container.decode(Bool.self) {
                self = .bool(boolValue)
            } else if let doubleValue = try? container.decode(Double.self) {
                self = .number(doubleValue)
            } else if let arrayValue = try? container.decode([ToriiJSONValue].self) {
                self = .array(arrayValue)
            } else if let dictValue = try? container.decode([String: ToriiJSONValue].self) {
                self = .object(dictValue)
            } else {
                throw DecodingError.dataCorruptedError(in: container,
                                                       debugDescription: "Unsupported JSON value")
            }
        } else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath,
                                                    debugDescription: "Unable to decode ToriiJSONValue"))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let string):
            try container.encode(string)
        case .number(let number):
            try container.encode(number)
        case .bool(let bool):
            try container.encode(bool)
        case .array(let array):
            try container.encode(array)
        case .object(let dictionary):
            try container.encode(dictionary)
        case .null:
            try container.encodeNil()
        }
    }
}

public extension ToriiJSONValue {
    subscript(key: String) -> ToriiJSONValue? {
        guard case let .object(dictionary) = self else {
            return nil
        }
        return dictionary[key]
    }

    func encodedData(prettyPrinted: Bool = false) throws -> Data {
        let encoder = JSONEncoder()
        if prettyPrinted {
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        }
        return try encoder.encode(self)
    }

    func decode<T: Decodable>(as type: T.Type = T.self,
                              decoder: JSONDecoder = JSONDecoder()) throws -> T {
        let data = try encodedData()
        return try decoder.decode(T.self, from: data)
    }
}


public struct ToriiStatusMetrics: Sendable, Equatable {
    public let commitLatencyMs: Int
    public let queueSize: Int
    public let queueDelta: Int
    public let txApprovedDelta: Int
    public let txRejectedDelta: Int
    public let viewChangeDelta: Int

    public var hasActivity: Bool {
        queueDelta != 0 || txApprovedDelta != 0 || txRejectedDelta != 0 || viewChangeDelta != 0
    }

    static func fromSamples(previous: ToriiStatusPayload?, current: ToriiStatusPayload) -> ToriiStatusMetrics {
        guard let previous else {
            return ToriiStatusMetrics(
                commitLatencyMs: current.commitTimeMs,
                queueSize: current.queueSize,
                queueDelta: 0,
                txApprovedDelta: 0,
                txRejectedDelta: 0,
                viewChangeDelta: 0
            )
        }
        return ToriiStatusMetrics(
            commitLatencyMs: current.commitTimeMs,
            queueSize: current.queueSize,
            queueDelta: current.queueSize - previous.queueSize,
            txApprovedDelta: max(0, current.txsApproved - previous.txsApproved),
            txRejectedDelta: max(0, current.txsRejected - previous.txsRejected),
            viewChangeDelta: max(0, current.viewChanges - previous.viewChanges)
        )
    }
}

public struct ToriiStatusPayload: Decodable, Sendable, Equatable {
    public let peers: Int
    public let queueSize: Int
    public let commitTimeMs: Int
    public let txsApproved: Int
    public let txsRejected: Int
    public let viewChanges: Int
    public let laneGovernanceSealedTotal: Int
    public let laneGovernanceSealedAliases: [String]
    public let raw: [String: ToriiJSONValue]

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let raw = try container.decode([String: ToriiJSONValue].self)
        try self.init(raw: raw)
    }

    public init(raw: [String: ToriiJSONValue]) throws {
        self.raw = raw
        self.peers = try Self.decodeInt(raw["peers"], field: "peers")
        self.queueSize = try Self.decodeInt(raw["queue_size"], field: "queue_size")
        self.commitTimeMs = try Self.decodeInt(raw["commit_time_ms"], field: "commit_time_ms")
        self.txsApproved = try Self.decodeInt(raw["txs_approved"], field: "txs_approved")
        self.txsRejected = try Self.decodeInt(raw["txs_rejected"], field: "txs_rejected")
        self.viewChanges = try Self.decodeInt(raw["view_changes"], field: "view_changes")
        self.laneGovernanceSealedTotal = try Self.decodeInt(raw["lane_governance_sealed_total"], field: "lane_governance_sealed_total")
        self.laneGovernanceSealedAliases = try Self.decodeStringArray(raw["lane_governance_sealed_aliases"], field: "lane_governance_sealed_aliases")
    }

    public subscript(field name: String) -> ToriiJSONValue? {
        raw[name]
    }

    private static func decodeInt(_ value: ToriiJSONValue?, field: String) throws -> Int {
        guard let value else { return 0 }
        switch value {
        case .bool(let bool):
            return bool ? 1 : 0
        case .number:
            guard let parsed = value.normalizedInt64 else {
                throw ToriiClientError.invalidPayload("status field `\(field)` must be numeric")
            }
            guard parsed >= 0 else {
                throw ToriiClientError.invalidPayload("status field `\(field)` must be non-negative")
            }
            guard parsed <= Int64(Int.max) else {
                throw ToriiClientError.invalidPayload("status field `\(field)` must be numeric")
            }
            return Int(parsed)
        case .string(let string):
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            guard let parsed = Int(trimmed) else {
                throw ToriiClientError.invalidPayload("status field `\(field)` must be numeric")
            }
            guard parsed >= 0 else {
                throw ToriiClientError.invalidPayload("status field `\(field)` must be non-negative")
            }
            return parsed
        default:
            throw ToriiClientError.invalidPayload("status field `\(field)` must be numeric")
        }
    }

    private static func decodeStringArray(_ value: ToriiJSONValue?, field: String) throws -> [String] {
        guard let value else { return [] }
        switch value {
        case .array(let items):
            return try items.map { element in
                if case let .string(string) = element {
                    return string
                } else {
                    throw ToriiClientError.invalidPayload("status field `\(field)` must be an array of strings")
                }
            }
        case .null:
            return []
        default:
            throw ToriiClientError.invalidPayload("status field `\(field)` must be an array of strings")
        }
    }
}

public struct ToriiStatusSnapshot: Sendable, Equatable {
    public let timestamp: Date
    public let status: ToriiStatusPayload
    public let metrics: ToriiStatusMetrics
}

struct ToriiStatusState {
    private var previous: ToriiStatusPayload?
    private var latestSequence: UInt64 = 0
    private var nextSequence: UInt64 = 1

    mutating func reserveSequence() -> UInt64 {
        defer { nextSequence &+= 1 }
        return nextSequence
    }

    mutating func record(_ payload: ToriiStatusPayload, sequence: UInt64) -> ToriiStatusMetrics {
        if sequence > latestSequence {
            let metrics = ToriiStatusMetrics.fromSamples(previous: previous, current: payload)
            previous = payload
            latestSequence = sequence
            return metrics
        } else {
            // Stale sample: ignore for timeline and report zero deltas.
            return ToriiStatusMetrics.fromSamples(previous: nil, current: payload)
        }
    }
}

private enum ToriiDataModelCompatibility {
    case unknown
    case compatible(version: Int)
    case incompatible(expected: Int, actual: Int?)
}

public struct ToriiGovernanceInstruction: Codable, Sendable {
    public let wireId: String
    public let payloadHex: String

    private enum CodingKeys: String, CodingKey {
        case wireId = "wire_id"
        case payloadHex = "payload_hex"
    }
}

public struct ToriiGovernanceWindow: Codable, Sendable {
    public var lower: UInt64
    public var upper: UInt64

    public init(lower: UInt64, upper: UInt64) {
        self.lower = lower
        self.upper = upper
    }
}

public struct ToriiGovernanceDeployContractProposalRequest: Encodable, Sendable {
    public var namespace: String
    public var contractId: String
    public var codeHashHex: String
    public var abiHashHex: String
    public var abiVersion: String
    public var window: ToriiGovernanceWindow?

    private enum CodingKeys: String, CodingKey {
        case namespace
        case contractId = "contract_id"
        case codeHashHex = "code_hash"
        case abiHashHex = "abi_hash"
        case abiVersion = "abi_version"
        case window
    }

    public init(namespace: String,
                contractId: String,
                codeHashHex: String,
                abiHashHex: String,
                abiVersion: String,
                window: ToriiGovernanceWindow? = nil) {
        self.namespace = namespace
        self.contractId = contractId
        self.codeHashHex = codeHashHex
        self.abiHashHex = abiHashHex
        self.abiVersion = abiVersion
        self.window = window
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedNamespace = try ToriiRequestValidation.normalizedNonEmpty(namespace, field: "namespace")
        let normalizedContractId = try ToriiRequestValidation.normalizedNonEmpty(contractId, field: "contract_id")
        let normalizedCodeHash = try ToriiRequestValidation.normalized32ByteHex(codeHashHex, field: "code_hash")
        let normalizedAbiHash = try ToriiRequestValidation.normalized32ByteHex(abiHashHex, field: "abi_hash")
        let normalizedAbiVersion = try ToriiRequestValidation.normalizedNonEmpty(abiVersion, field: "abi_version")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedNamespace, forKey: .namespace)
        try container.encode(normalizedContractId, forKey: .contractId)
        try container.encode(normalizedCodeHash, forKey: .codeHashHex)
        try container.encode(normalizedAbiHash, forKey: .abiHashHex)
        try container.encode(normalizedAbiVersion, forKey: .abiVersion)
        try container.encodeIfPresent(window, forKey: .window)
    }
}

public struct ToriiGovernanceZkBallotRequest: Encodable, Sendable {
    public var authority: String
    public var chainId: String
    public var electionId: String
    public var proofB64: String
    public var publicInputs: [String: ToriiJSONValue]?

    private enum CodingKeys: String, CodingKey {
        case authority
        case chainId = "chain_id"
        case electionId = "election_id"
        case proofB64 = "proof_b64"
        case publicInputs = "public"
    }

    public init(authority: String,
                chainId: String,
                electionId: String,
                proofB64: String,
                publicInputs: [String: ToriiJSONValue]? = nil) {
        self.authority = authority
        self.chainId = chainId
        self.electionId = electionId
        self.proofB64 = proofB64
        self.publicInputs = publicInputs
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedAuthority = try ToriiRequestValidation.normalizedNonEmpty(authority, field: "authority")
        let normalizedChainId = try ToriiRequestValidation.normalizedNonEmpty(chainId, field: "chain_id")
        let normalizedElectionId = try ToriiRequestValidation.normalizedNonEmpty(electionId, field: "election_id")
        let normalizedProof = try ToriiRequestValidation.normalizedBase64(proofB64, field: "proof_b64")
        let normalizedPublicInputs: [String: ToriiJSONValue]? = try {
            guard let publicInputs else { return nil }
            return try normalizeGovernanceZkPublicInputs(publicInputs, field: "public inputs")
        }()
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedAuthority, forKey: .authority)
        try container.encode(normalizedChainId, forKey: .chainId)
        try container.encode(normalizedElectionId, forKey: .electionId)
        try container.encode(normalizedProof, forKey: .proofB64)
        try container.encodeIfPresent(normalizedPublicInputs, forKey: .publicInputs)
    }
}

fileprivate func normalizeGovernanceZkPublicInputs(_ inputs: [String: ToriiJSONValue],
                                                   field: String) throws -> [String: ToriiJSONValue] {
    var normalized = inputs
    try rejectGovernanceZkPublicInputKey(&normalized,
                                         key: "durationBlocks",
                                         canonicalKey: "duration_blocks",
                                         field: field)
    try rejectGovernanceZkPublicInputKey(&normalized,
                                         key: "root_hint_hex",
                                         canonicalKey: "root_hint",
                                         field: field)
    try rejectGovernanceZkPublicInputKey(&normalized,
                                         key: "rootHintHex",
                                         canonicalKey: "root_hint",
                                         field: field)
    try rejectGovernanceZkPublicInputKey(&normalized,
                                         key: "rootHint",
                                         canonicalKey: "root_hint",
                                         field: field)
    try rejectGovernanceZkPublicInputKey(&normalized,
                                         key: "nullifier_hex",
                                         canonicalKey: "nullifier",
                                         field: field)
    try rejectGovernanceZkPublicInputKey(&normalized,
                                         key: "nullifierHex",
                                         canonicalKey: "nullifier",
                                         field: field)
    try normalizeGovernanceZkPublicInputHex(&normalized, key: "root_hint", field: field)
    try normalizeGovernanceZkPublicInputHex(&normalized, key: "nullifier", field: field)
    let hasOwner = governanceZkHintPresent(normalized["owner"])
    let hasAmount = governanceZkHintPresent(normalized["amount"])
    let hasDuration = governanceZkHintPresent(normalized["duration_blocks"])
    let hasAnyHint = hasOwner || hasAmount || hasDuration
    if hasAnyHint && !(hasOwner && hasAmount && hasDuration) {
        throw ToriiClientError.invalidPayload(
            "\(field) must include owner, amount, and duration_blocks when providing lock hints."
        )
    }
    try ensureGovernanceZkOwnerCanonical(normalized, field: field)
    return normalized
}

fileprivate func rejectGovernanceZkPublicInputKey(_ inputs: inout [String: ToriiJSONValue],
                                                 key: String,
                                                 canonicalKey: String,
                                                 field: String) throws {
    guard inputs[key] != nil else { return }
    throw ToriiClientError.invalidPayload(
        "\(field) must use \(canonicalKey) (unsupported key \(key))."
    )
}

fileprivate func normalizeGovernanceZkPublicInputHex(_ inputs: inout [String: ToriiJSONValue],
                                                     key: String,
                                                     field: String) throws {
    guard let value = inputs[key] else { return }
    if case .null = value { return }
    guard case let .string(raw) = value else {
        throw ToriiClientError.invalidPayload(
            "\(field).\(key) must be a 32-byte hex string."
        )
    }
    let canonical = try canonicalizeZkBallotHexHint(raw, field: "\(field).\(key)")
    inputs[key] = .string(canonical)
}

fileprivate func canonicalizeZkBallotHexHint(_ raw: String, field: String) throws -> String {
    var trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
    if let colonIndex = trimmed.firstIndex(of: ":") {
        let scheme = String(trimmed[..<colonIndex])
        let rest = String(trimmed[trimmed.index(after: colonIndex)...])
        if !scheme.isEmpty && scheme.lowercased() != "blake2b32" {
            throw ToriiClientError.invalidPayload("\(field) must be a 32-byte hex string.")
        }
        trimmed = rest.trimmingCharacters(in: .whitespacesAndNewlines)
    }
    if trimmed.hasPrefix("0x") || trimmed.hasPrefix("0X") {
        trimmed = String(trimmed.dropFirst(2))
    }
    guard trimmed.count == 64, Data(hexString: trimmed) != nil else {
        throw ToriiClientError.invalidPayload("\(field) must be a 32-byte hex string.")
    }
    return trimmed.lowercased()
}

fileprivate func ensureGovernanceZkOwnerCanonical(_ inputs: [String: ToriiJSONValue],
                                                  field: String) throws {
    guard let value = inputs["owner"] else { return }
    if case .null = value { return }
    guard case let .string(owner) = value else {
        throw ToriiClientError.invalidPayload("\(field).owner must be a canonical account id.")
    }
    let canonical = try canonicalizeGovernanceZkOwnerLiteral(owner, field: field)
    if canonical != owner {
        throw ToriiClientError.invalidPayload("\(field).owner must use canonical account id form.")
    }
}

fileprivate func canonicalizeGovernanceZkOwnerLiteral(_ raw: String, field: String) throws -> String {
    let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty, trimmed == raw else {
        throw ToriiClientError.invalidPayload("\(field).owner must be a canonical account id.")
    }
    if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
        throw ToriiClientError.invalidPayload("\(field).owner must be a canonical account id.")
    }
    if trimmed.contains("@") {
        throw ToriiClientError.invalidPayload("\(field).owner must be a canonical account id.")
    }
    let (address, format) = try AccountAddress.parseAny(
        trimmed,
        expectedPrefix: 0x02F1
    )
    guard format == .ih58 else {
        throw ToriiClientError.invalidPayload("\(field).owner must be a canonical account id.")
    }
    let ih58 = try address.toIH58(networkPrefix: 0x02F1)
    return ih58
}

fileprivate func governanceZkHintPresent(_ value: ToriiJSONValue?) -> Bool {
    guard let value else { return false }
    if case .null = value {
        return false
    }
    return true
}

public struct ToriiGovernancePlainBallotRequest: Encodable, Sendable {
    public var authority: String
    public var chainId: String
    public var referendumId: String
    public var owner: String
    public var amount: String
    public var durationBlocks: UInt64
    public var direction: String

    private enum CodingKeys: String, CodingKey {
        case authority
        case chainId = "chain_id"
        case referendumId = "referendum_id"
        case owner
        case amount
        case durationBlocks = "duration_blocks"
        case direction
    }

    public init(authority: String,
                chainId: String,
                referendumId: String,
                owner: String,
                amount: String,
                durationBlocks: UInt64,
                direction: String) {
        self.authority = authority
        self.chainId = chainId
        self.referendumId = referendumId
        self.owner = owner
        self.amount = amount
        self.durationBlocks = durationBlocks
        self.direction = direction
    }
}

public struct ToriiGovernanceFinalizeRequest: Encodable, Sendable {
    public var referendumId: String
    public var proposalId: String

    private enum CodingKeys: String, CodingKey {
        case referendumId = "referendum_id"
        case proposalId = "proposal_id"
    }

    public init(referendumId: String, proposalId: String) {
        self.referendumId = referendumId
        self.proposalId = proposalId
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedReferendumId = try ToriiRequestValidation.normalizedNonEmpty(referendumId,
                                                                                   field: "referendum_id")
        let normalizedProposalId = try ToriiRequestValidation.normalized32ByteHex(proposalId,
                                                                                  field: "proposal_id")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedReferendumId, forKey: .referendumId)
        try container.encode(normalizedProposalId, forKey: .proposalId)
    }
}

public struct ToriiGovernanceEnactRequest: Encodable, Sendable {
    public var proposalId: String
    public var preimageHash: String?
    public var window: ToriiGovernanceWindow?

    private enum CodingKeys: String, CodingKey {
        case proposalId = "proposal_id"
        case preimageHash = "preimage_hash"
        case window
    }

    public init(proposalId: String,
                preimageHash: String? = nil,
                window: ToriiGovernanceWindow? = nil) {
        self.proposalId = proposalId
        self.preimageHash = preimageHash
        self.window = window
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedProposalId = try ToriiRequestValidation.normalized32ByteHex(proposalId,
                                                                                  field: "proposal_id")
        let normalizedPreimage = try ToriiRequestValidation.normalizedOptional32ByteHex(preimageHash,
                                                                                        field: "preimage_hash")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedProposalId, forKey: .proposalId)
        try container.encodeIfPresent(normalizedPreimage, forKey: .preimageHash)
        try container.encodeIfPresent(window, forKey: .window)
    }
}

public struct ToriiGovernanceProposalResponse: Decodable, Sendable {
    public let ok: Bool
    public let proposalId: String
    public let txInstructions: [ToriiGovernanceInstruction]

    private enum CodingKeys: String, CodingKey {
        case ok
        case proposalId = "proposal_id"
        case txInstructions = "tx_instructions"
    }

    public init(ok: Bool,
                proposalId: String,
                txInstructions: [ToriiGovernanceInstruction]) {
        self.ok = ok
        self.proposalId = proposalId
        self.txInstructions = txInstructions
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        ok = try container.decode(Bool.self, forKey: .ok)
        let proposalId = try container.decode(String.self, forKey: .proposalId)
        self.proposalId = try ToriiValidation.normalized32ByteHex(
            proposalId,
            field: "proposal_id",
            codingPath: container.codingPath + [CodingKeys.proposalId]
        )
        txInstructions = try container.decode([ToriiGovernanceInstruction].self, forKey: .txInstructions)
    }
}

public struct ToriiGovernanceBallotResponse: Decodable, Sendable {
    public let ok: Bool
    public let accepted: Bool
    public let reason: String?
    public let txInstructions: [ToriiGovernanceInstruction]

    private enum CodingKeys: String, CodingKey {
        case ok
        case accepted
        case reason
        case txInstructions = "tx_instructions"
    }

    public init(ok: Bool,
                accepted: Bool,
                reason: String?,
                txInstructions: [ToriiGovernanceInstruction]) {
        self.ok = ok
        self.accepted = accepted
        self.reason = reason
        self.txInstructions = txInstructions
    }
}

public struct ToriiGovernanceFinalizeResponse: Decodable, Sendable {
    public let ok: Bool
    public let txInstructions: [ToriiGovernanceInstruction]

    private enum CodingKeys: String, CodingKey {
        case ok
        case txInstructions = "tx_instructions"
    }
}

public struct ToriiGovernanceEnactResponse: Decodable, Sendable {
    public let ok: Bool
    public let txInstructions: [ToriiGovernanceInstruction]

    private enum CodingKeys: String, CodingKey {
        case ok
        case txInstructions = "tx_instructions"
    }
}

public enum ToriiGovernanceProposalKind: Decodable, Sendable, Equatable {
    case deployContract(ToriiGovernanceDeployContractKind)
    case unknown(String)

    private enum CodingKeys: String, CodingKey {
        case deployContract = "DeployContract"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let dictionary = try? container.decode([String: ToriiGovernanceDeployContractKind].self) {
            guard dictionary.count == 1 else {
                throw DecodingError.dataCorruptedError(in: container,
                                                       debugDescription: "Governance proposal kind expects single-key object")
            }
            if let deploy = dictionary["DeployContract"] {
                self = .deployContract(deploy)
                return
            }
        }
        if let raw = try? container.decode([String: ToriiJSONValue].self) {
            guard raw.count == 1, let key = raw.keys.first else {
                throw DecodingError.dataCorruptedError(in: container,
                                                       debugDescription: "Governance proposal kind expects single-key object")
            }
            self = .unknown(key)
            return
        }
        throw DecodingError.dataCorruptedError(in: container,
                                               debugDescription: "Unable to decode governance proposal kind")
    }
}

public struct ToriiGovernanceDeployContractKind: Decodable, Sendable, Equatable {
    public let namespace: String
    public let contractId: String
    public let codeHashHex: String
    public let abiHashHex: String
    public let abiVersion: String

    private enum CodingKeys: String, CodingKey {
        case namespace
        case contractId = "contract_id"
        case codeHashHex = "code_hash_hex"
        case abiHashHex = "abi_hash_hex"
        case abiVersion = "abi_version"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        namespace = try container.decode(String.self, forKey: .namespace)
        contractId = try container.decode(String.self, forKey: .contractId)
        let codeHashHex = try container.decode(String.self, forKey: .codeHashHex)
        self.codeHashHex = try ToriiValidation.normalized32ByteHex(
            codeHashHex,
            field: "code_hash_hex",
            codingPath: container.codingPath + [CodingKeys.codeHashHex]
        )
        let abiHashHex = try container.decode(String.self, forKey: .abiHashHex)
        self.abiHashHex = try ToriiValidation.normalized32ByteHex(
            abiHashHex,
            field: "abi_hash_hex",
            codingPath: container.codingPath + [CodingKeys.abiHashHex]
        )
        abiVersion = try container.decode(String.self, forKey: .abiVersion)
    }
}

public enum ToriiGovernanceProposalStatus: String, Decodable, Sendable {
    case proposed = "Proposed"
    case approved = "Approved"
    case rejected = "Rejected"
    case enacted = "Enacted"
}

public struct ToriiGovernanceProposalRecord: Decodable, Sendable {
    public let proposer: String
    public let kind: ToriiGovernanceProposalKind
    public let createdHeight: UInt64
    public let status: ToriiGovernanceProposalStatus

    private enum CodingKeys: String, CodingKey {
        case proposer
        case kind
        case createdHeight = "created_height"
        case status
    }
}

public struct ToriiGovernanceProposalGetResponse: Decodable, Sendable {
    public let found: Bool
    public let proposal: ToriiGovernanceProposalRecord?
}

public struct ToriiGovernanceLockRecord: Decodable, Sendable {
    public let owner: String
    public let amount: String
    public let expiryHeight: UInt64
    public let direction: UInt8
    public let durationBlocks: UInt64

    private enum CodingKeys: String, CodingKey {
        case owner
        case amount
        case expiryHeight = "expiry_height"
        case direction
        case durationBlocks = "duration_blocks"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        owner = try container.decode(String.self, forKey: .owner)
        if let stringValue = try? container.decode(String.self, forKey: .amount) {
            let trimmed = stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                throw DecodingError.dataCorruptedError(forKey: .amount,
                                                       in: container,
                                                       debugDescription: "amount must be a non-empty string")
            }
            amount = trimmed
        } else if let intValue = try? container.decode(UInt64.self, forKey: .amount) {
            amount = String(intValue)
        } else {
            throw DecodingError.dataCorruptedError(forKey: .amount,
                                                   in: container,
                                                   debugDescription: "amount must be a string or unsigned integer")
        }
        expiryHeight = try container.decode(UInt64.self, forKey: .expiryHeight)
        direction = try container.decode(UInt8.self, forKey: .direction)
        durationBlocks = try container.decodeIfPresent(UInt64.self, forKey: .durationBlocks) ?? 0
    }
}

public struct ToriiGovernanceLocksResponse: Decodable, Sendable {
    public let found: Bool
    public let referendumId: String
    public let locks: [String: ToriiGovernanceLockRecord]?

    private enum CodingKeys: String, CodingKey {
        case found
        case referendumId = "referendum_id"
        case locks
    }
}

public enum ToriiGovernanceReferendumStatus: String, Decodable, Sendable {
    case proposed = "Proposed"
    case open = "Open"
    case closed = "Closed"
}

public enum ToriiGovernanceReferendumMode: String, Decodable, Sendable {
    case zk = "Zk"
    case plain = "Plain"
}

public struct ToriiGovernanceReferendumRecord: Decodable, Sendable {
    public let hStart: UInt64
    public let hEnd: UInt64
    public let status: ToriiGovernanceReferendumStatus
    public let mode: ToriiGovernanceReferendumMode

    private enum CodingKeys: String, CodingKey {
        case hStart = "h_start"
        case hEnd = "h_end"
        case status
        case mode
    }
}

public struct ToriiGovernanceReferendumResponse: Decodable, Sendable {
    public let found: Bool
    public let referendum: ToriiGovernanceReferendumRecord?
}

public struct ToriiGovernanceTallyResponse: Decodable, Sendable {
    public let referendumId: String
    public let approve: String
    public let reject: String
    public let abstain: String

    private enum CodingKeys: String, CodingKey {
        case referendumId = "referendum_id"
        case approve
        case reject
        case abstain
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        referendumId = try container.decode(String.self, forKey: .referendumId)
        approve = try ToriiGovernanceTallyResponse.decodeBigInt(from: container, key: .approve)
        reject = try ToriiGovernanceTallyResponse.decodeBigInt(from: container, key: .reject)
        abstain = try ToriiGovernanceTallyResponse.decodeBigInt(from: container, key: .abstain)
    }

    private static func decodeBigInt(from container: KeyedDecodingContainer<CodingKeys>, key: CodingKeys) throws -> String {
        if let stringValue = try? container.decode(String.self, forKey: key) {
            let trimmed = stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                throw DecodingError.dataCorruptedError(forKey: key,
                                                       in: container,
                                                       debugDescription: "\(key.stringValue) must be a non-empty string")
            }
            return trimmed
        }
        if let intValue = try? container.decode(UInt64.self, forKey: key) {
            return String(intValue)
        }
        throw DecodingError.dataCorruptedError(forKey: key,
                                               in: container,
                                               debugDescription: "\(key.stringValue) must be a string or unsigned integer")
    }
}

public struct ToriiGovernanceUnlockStatsResponse: Decodable, Sendable {
    public let heightCurrent: UInt64
    public let expiredLocksNow: UInt64
    public let referendaWithExpired: UInt64
    public let lastSweepHeight: UInt64

    private enum CodingKeys: String, CodingKey {
        case heightCurrent = "height_current"
        case expiredLocksNow = "expired_locks_now"
        case referendaWithExpired = "referenda_with_expired"
        case lastSweepHeight = "last_sweep_height"
    }
}

public struct ToriiSubmitTransactionResponse: Decodable, Sendable {
    public struct Payload: Decodable, Sendable {
        public let txHash: String
        public let submittedAtMs: UInt64
        public let submittedAtHeight: UInt64
        public let signer: String

        private enum CodingKeys: String, CodingKey {
            case txHash = "tx_hash"
            case submittedAtMs = "submitted_at_ms"
            case submittedAtHeight = "submitted_at_height"
            case signer
        }
    }

    public let payload: Payload
    public let signature: String

    /// Convenience accessor for transaction hash.
    public var hash: String { payload.txHash }
}

public struct ToriiPipelineTransactionStatus: Decodable, Sendable {
    public struct Status: Decodable, Sendable {
        public let kind: String
        public let content: String?

        public var state: PipelineTransactionState {
            PipelineTransactionState(kind: kind)
        }
    }

    public struct Content: Decodable, Sendable {
        public let hash: String
        public let status: Status
    }

    public let kind: String
    public let content: Content
}

public struct ToriiPipelineRecovery: Decodable, Sendable {
    public struct Dag: Decodable, Sendable {
        public let fingerprint: String
        public let key_count: UInt64

        private enum CodingKeys: String, CodingKey {
            case fingerprint
            case key_count = "key_count"
        }
    }

    public struct TransactionSnapshot: Decodable, Sendable {
        public let hash: String
        public let reads: [String]
        public let writes: [String]
    }

    public let format: String
    public let height: UInt64
    public let dag: Dag
    public let txs: [TransactionSnapshot]
}

public enum PipelineTransactionState: Hashable, Sendable {
    case queued
    case approved
    case committed
    case applied
    case rejected
    case expired
    case other(String)

    public init(kind: String) {
        switch kind {
        case "Queued": self = .queued
        case "Approved": self = .approved
        case "Committed": self = .committed
        case "Applied": self = .applied
        case "Rejected": self = .rejected
        case "Expired": self = .expired
        default: self = .other(kind)
        }
    }

    public var kind: String {
        switch self {
        case .queued: return "Queued"
        case .approved: return "Approved"
        case .committed: return "Committed"
        case .applied: return "Applied"
        case .rejected: return "Rejected"
        case .expired: return "Expired"
        case .other(let value): return value
        }
    }

    public var isKnownTerminalSuccess: Bool {
        switch self {
        case .approved, .committed, .applied:
            return true
        default:
            return false
        }
    }

    public var isKnownTerminalFailure: Bool {
        switch self {
        case .rejected, .expired:
            return true
        default:
            return false
        }
    }
}

public struct ToriiTimeSnapshot: Decodable, Sendable {
    public let now: UInt64
    public let offset_ms: Int64
    public let confidence_ms: UInt64

    private enum CodingKeys: String, CodingKey {
        case now
        case offset_ms = "offset_ms"
        case confidence_ms = "confidence_ms"
    }
}

public struct ToriiTimeStatusSnapshot: Decodable, Sendable {
    public struct Sample: Decodable, Sendable {
        public let peer: String
        public let last_offset_ms: Int64
        public let last_rtt_ms: UInt64
        public let count: UInt64

        private enum CodingKeys: String, CodingKey {
            case peer
            case last_offset_ms = "last_offset_ms"
            case last_rtt_ms = "last_rtt_ms"
            case count
        }
    }

    public struct RTTBucket: Decodable, Sendable {
        public let le: UInt64
        public let count: UInt64
    }

    public struct RTTSnapshot: Decodable, Sendable {
        public let buckets: [RTTBucket]
        public let sum_ms: UInt64
        public let count: UInt64

        private enum CodingKeys: String, CodingKey {
            case buckets
            case sum_ms = "sum_ms"
            case count
        }
    }

    public let peers: UInt64
    public let samples: [Sample]
    public let rtt: RTTSnapshot?
    public let note: String?
}

/// Deterministic Sumeragi membership hash snapshot mirrored from `/v1/sumeragi/status`.
public struct ToriiSumeragiMembershipSnapshot: Decodable, Sendable {
    /// Block height covered by the membership digest.
    public let height: UInt64
    /// Consensus view associated with the digest.
    public let view: UInt64
    /// Epoch identifier paired with the digest.
    public let epoch: UInt64
    /// Optional canonical digest for the membership (hex encoded).
    public let viewHash: String?

    private enum CodingKeys: String, CodingKey {
        case height
        case view
        case epoch
        case viewHash = "view_hash"
    }
}

public struct ToriiLaneCommitmentSnapshot: Decodable, Sendable, Equatable {
    public let blockHeight: UInt64
    public let laneId: UInt64
    public let txCount: UInt64
    public let totalChunks: UInt64
    public let rbcBytesTotal: UInt64
    public let teuTotal: UInt64
    public let blockHash: String

    private enum CodingKeys: String, CodingKey {
        case blockHeight = "block_height"
        case laneId = "lane_id"
        case txCount = "tx_count"
        case totalChunks = "total_chunks"
        case rbcBytesTotal = "rbc_bytes_total"
        case teuTotal = "teu_total"
        case blockHash = "block_hash"
    }
}

public struct ToriiDataspaceCommitmentSnapshot: Decodable, Sendable, Equatable {
    public let blockHeight: UInt64
    public let laneId: UInt64
    public let dataspaceId: UInt64
    public let txCount: UInt64
    public let totalChunks: UInt64
    public let rbcBytesTotal: UInt64
    public let teuTotal: UInt64
    public let blockHash: String

    private enum CodingKeys: String, CodingKey {
        case blockHeight = "block_height"
        case laneId = "lane_id"
        case dataspaceId = "dataspace_id"
        case txCount = "tx_count"
        case totalChunks = "total_chunks"
        case rbcBytesTotal = "rbc_bytes_total"
        case teuTotal = "teu_total"
        case blockHash = "block_hash"
    }
}

public struct ToriiLaneRuntimeUpgradeHookSnapshot: Decodable, Sendable, Equatable {
    public let allow: Bool
    public let requireMetadata: Bool
    public let metadataKey: String?
    public let allowedIds: [String]

    private enum CodingKeys: String, CodingKey {
        case allow
        case requireMetadata = "require_metadata"
        case metadataKey = "metadata_key"
        case allowedIds = "allowed_ids"
    }
}

public struct ToriiLaneGovernanceSnapshot: Decodable, Sendable, Equatable {
    public let laneId: UInt64
    public let alias: String
    public let dataspaceId: UInt64
    public let visibility: String
    public let storageProfile: String
    public let governance: String?
    public let manifestRequired: Bool
    public let manifestReady: Bool
    public let manifestPath: String?
    public let validatorIds: [String]
    public let quorum: UInt64?
    public let protectedNamespaces: [String]
    public let runtimeUpgrade: ToriiLaneRuntimeUpgradeHookSnapshot?
    public let privacyCommitments: [ToriiLanePrivacyCommitmentSnapshot]

    private enum CodingKeys: String, CodingKey {
        case laneId = "lane_id"
        case alias
        case dataspaceId = "dataspace_id"
        case visibility
        case storageProfile = "storage_profile"
        case governance
        case manifestRequired = "manifest_required"
        case manifestReady = "manifest_ready"
        case manifestPath = "manifest_path"
        case validatorIds = "validator_ids"
        case quorum
        case protectedNamespaces = "protected_namespaces"
        case runtimeUpgrade = "runtime_upgrade"
        case privacyCommitments = "privacy_commitments"
    }
}

public struct ToriiLanePrivacyCommitmentSnapshot: Decodable, Sendable, Equatable {
    public let id: UInt64
    public let scheme: String
    public let merkle: ToriiLaneMerkleCommitmentSnapshot?
    public let snark: ToriiLaneSnarkCommitmentSnapshot?
}

public struct ToriiLaneMerkleCommitmentSnapshot: Decodable, Sendable, Equatable {
    public let root: String
    public let maxDepth: UInt64

    private enum CodingKeys: String, CodingKey {
        case root
        case maxDepth = "max_depth"
    }
}

public struct ToriiLaneSnarkCommitmentSnapshot: Decodable, Sendable, Equatable {
    public let circuitId: UInt64
    public let verifyingKeyDigest: String
    public let statementHash: String
    public let proofHash: String

    private enum CodingKeys: String, CodingKey {
        case circuitId = "circuit_id"
        case verifyingKeyDigest = "verifying_key_digest"
        case statementHash = "statement_hash"
        case proofHash = "proof_hash"
    }
}

/// Snapshot returned by `/v1/sumeragi/status`, preserving unknown fields while decoding known ones.
public struct ToriiSumeragiStatusSnapshot: Decodable, Sendable {
    /// Runtime consensus mode tag (permissioned or npos).
    public let modeTag: String?
    /// Staged consensus mode if activation is pending.
    public let stagedModeTag: String?
    /// Activation height for the staged mode (if any).
    public let stagedModeActivationHeight: UInt64?
    /// Blocks elapsed after the activation height without applying the staged mode.
    public let modeActivationLagBlocks: UInt64?
    /// Consensus handshake caps derived from runtime configuration.
    public let consensusCaps: ToriiConsensusCaps?
    /// Structured membership digest if the node exposes it.
    public let membership: ToriiSumeragiMembershipSnapshot?
    /// Latest Nexus lane commitments accounted in the status.
    public let laneCommitments: [ToriiLaneCommitmentSnapshot]
    /// Latest Nexus dataspace commitments accounted in the status.
    public let dataspaceCommitments: [ToriiDataspaceCommitmentSnapshot]
    /// Governance manifest coverage per lane.
    public let laneGovernance: [ToriiLaneGovernanceSnapshot]
    /// Original payload keyed by field name for forward compatibility.
    public let fields: [String: ToriiJSONValue]

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let raw = try container.decode([String: ToriiJSONValue].self)
        self.fields = raw
        self.modeTag = raw["mode_tag"]?.normalizedString
        self.stagedModeTag = raw["staged_mode_tag"]?.normalizedString
        self.stagedModeActivationHeight = raw["staged_mode_activation_height"]?.normalizedUInt64
        self.modeActivationLagBlocks = raw["mode_activation_lag_blocks"]?.normalizedUInt64
        self.consensusCaps = try Self.decodeValue(raw["consensus_caps"], field: "consensus_caps")
        self.membership = try Self.decodeValue(raw["membership"], field: "membership")
        self.laneCommitments = try Self.decodeArray(raw["lane_commitments"], field: "lane_commitments")
        self.dataspaceCommitments = try Self.decodeArray(raw["dataspace_commitments"], field: "dataspace_commitments")
        self.laneGovernance = try Self.decodeArray(raw["lane_governance"], field: "lane_governance")
    }

    /// Access an arbitrary raw field by name.
    public subscript(field name: String) -> ToriiJSONValue? {
        fields[name]
    }

    private static func decodeValue<T: Decodable>(_ value: ToriiJSONValue?, field: String) throws -> T? {
        guard let value else { return nil }
        if case .null = value {
            return nil
        }
        let encoder = JSONEncoder()
        let data = try encoder.encode(value)
        do {
            return try JSONDecoder().decode(T.self, from: data)
        } catch {
            throw ToriiClientError.invalidPayload("failed to decode \(field): \(error.localizedDescription)")
        }
    }

    private static func decodeArray<T: Decodable>(_ value: ToriiJSONValue?, field: String) throws -> [T] {
        guard let value else { return [] }
        if case .null = value {
            return []
        }
        let encoder = JSONEncoder()
        let data = try encoder.encode(value)
        do {
            return try JSONDecoder().decode([T].self, from: data)
        } catch {
            throw ToriiClientError.invalidPayload("failed to decode \(field): \(error.localizedDescription)")
        }
    }
}

/// Commit QC record returned by `/v1/sumeragi/commit_qc/{hash}`.
public struct ToriiSumeragiCommitQcRecord: Decodable, Sendable, Equatable {
    public let subjectBlockHash: String
    public let commitQc: ToriiSumeragiCommitQc?

    private enum CodingKeys: String, CodingKey {
        case subjectBlockHash = "subject_block_hash"
        case commitQc = "commit_qc"
    }
}

/// Full commit QC details returned by `/v1/sumeragi/commit_qc/{hash}`.
public struct ToriiSumeragiCommitQc: Decodable, Sendable, Equatable {
    public let phase: String
    public let parentStateRoot: String
    public let postStateRoot: String
    public let height: UInt64
    public let view: UInt64
    public let epoch: UInt64
    public let modeTag: String
    public let validatorSetHash: String
    public let validatorSetHashVersion: UInt16
    public let validatorSet: [String]
    public let signersBitmap: String
    public let blsAggregateSignature: String

    private enum CodingKeys: String, CodingKey {
        case phase
        case parentStateRoot = "parent_state_root"
        case postStateRoot = "post_state_root"
        case height
        case view
        case epoch
        case modeTag = "mode_tag"
        case validatorSetHash = "validator_set_hash"
        case validatorSetHashVersion = "validator_set_hash_version"
        case validatorSet = "validator_set"
        case signersBitmap = "signers_bitmap"
        case blsAggregateSignature = "bls_aggregate_signature"
    }
}

/// Consensus handshake caps exposed by `/v1/sumeragi/status`.
public struct ToriiConsensusCaps: Decodable, Sendable {
    public let collectorsK: UInt64
    public let redundantSendR: UInt64
    public let daEnabled: Bool
    public let rbcChunkMaxBytes: UInt64
    public let rbcSessionTtlMs: UInt64
    public let rbcStoreMaxSessions: UInt64
    public let rbcStoreSoftSessions: UInt64
    public let rbcStoreMaxBytes: UInt64
    public let rbcStoreSoftBytes: UInt64

    private enum CodingKeys: String, CodingKey {
        case collectorsK = "collectors_k"
        case redundantSendR = "redundant_send_r"
        case daEnabled = "da_enabled"
        case rbcChunkMaxBytes = "rbc_chunk_max_bytes"
        case rbcSessionTtlMs = "rbc_session_ttl_ms"
        case rbcStoreMaxSessions = "rbc_store_max_sessions"
        case rbcStoreSoftSessions = "rbc_store_soft_sessions"
        case rbcStoreMaxBytes = "rbc_store_max_bytes"
        case rbcStoreSoftBytes = "rbc_store_soft_bytes"
    }
}

public struct ToriiProverReport: Decodable, Sendable {
    public let id: String
    public let ok: Bool
    public let error: String?
    public let content_type: String
    public let size: UInt64
    public let created_ms: UInt64
    public let processed_ms: UInt64
    public let latency_ms: UInt64?
    public let zk1_tags: [String]?
}

public struct ToriiProverReportsFilter: Sendable {
    public var okOnly: Bool?
    public var failedOnly: Bool?
    public var errorsOnly: Bool?
    public var id: String?
    public var contentType: String?
    public var hasTag: String?
    public var limit: UInt32?
    public var sinceMs: UInt64?
    public var beforeMs: UInt64?
    public var idsOnly: Bool?
    public var order: String?
    public var offset: UInt32?
    public var latest: Bool?
    public var messagesOnly: Bool?

    public init(
        okOnly: Bool? = nil,
        failedOnly: Bool? = nil,
        errorsOnly: Bool? = nil,
        id: String? = nil,
        contentType: String? = nil,
        hasTag: String? = nil,
        limit: UInt32? = nil,
        sinceMs: UInt64? = nil,
        beforeMs: UInt64? = nil,
        idsOnly: Bool? = nil,
        order: String? = nil,
        offset: UInt32? = nil,
        latest: Bool? = nil,
        messagesOnly: Bool? = nil
    ) {
        self.okOnly = okOnly
        self.failedOnly = failedOnly
        self.errorsOnly = errorsOnly
        self.id = id
        self.contentType = contentType
        self.hasTag = hasTag
        self.limit = limit
        self.sinceMs = sinceMs
        self.beforeMs = beforeMs
        self.idsOnly = idsOnly
        self.order = order
        self.offset = offset
        self.latest = latest
        self.messagesOnly = messagesOnly
    }

    public func queryItems() -> [URLQueryItem] {
        var items: [URLQueryItem] = []
        if okOnly == true { items.append(URLQueryItem(name: "ok_only", value: "true")) }
        if failedOnly == true { items.append(URLQueryItem(name: "failed_only", value: "true")) }
        if errorsOnly == true { items.append(URLQueryItem(name: "errors_only", value: "true")) }
        if let id { items.append(URLQueryItem(name: "id", value: id)) }
        if let contentType { items.append(URLQueryItem(name: "content_type", value: contentType)) }
        if let hasTag { items.append(URLQueryItem(name: "has_tag", value: hasTag)) }
        if let limit { items.append(URLQueryItem(name: "limit", value: String(limit))) }
        if let sinceMs { items.append(URLQueryItem(name: "since_ms", value: String(sinceMs))) }
        if let beforeMs { items.append(URLQueryItem(name: "before_ms", value: String(beforeMs))) }
        if idsOnly == true { items.append(URLQueryItem(name: "ids_only", value: "true")) }
        if let order { items.append(URLQueryItem(name: "order", value: order)) }
        if let offset { items.append(URLQueryItem(name: "offset", value: String(offset))) }
        if latest == true { items.append(URLQueryItem(name: "latest", value: "true")) }
        if messagesOnly == true { items.append(URLQueryItem(name: "messages_only", value: "true")) }
        return items
    }
}

public enum ToriiClientError: Error, Sendable {
    case invalidURL(String)
    case transport(Swift.Error)
    case invalidResponse
    case emptyBody
    case httpStatus(code: Int, message: String?, rejectCode: String?)
    case decoding(Swift.Error)
    case invalidPayload(String)
    case incompatibleDataModel(expected: Int, actual: Int?)
}

extension ToriiClientError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .invalidURL(let value):
            return "Failed to construct Torii URL from path '\(value)'."
        case .transport(let error):
            return error.localizedDescription
        case .invalidResponse:
            return "Torii response was not an HTTP response."
        case .emptyBody:
            return "Torii response body was unexpectedly empty."
        case let .httpStatus(code, message, rejectCode):
            let suffix = message ?? HTTPURLResponse.localizedString(forStatusCode: code)
            if let rejectCode {
                return "Torii responded with HTTP status \(code) (\(suffix)). Reject code: \(rejectCode)."
            }
            return "Torii responded with HTTP status \(code) (\(suffix))."
        case .decoding(let error):
            return "Failed to decode Torii response: \(error.localizedDescription)"
        case .invalidPayload(let reason):
            return "Torii response payload was invalid: \(reason)"
        case let .incompatibleDataModel(expected, actual):
            if let actual {
                return "Torii data model version mismatch (expected \(expected), got \(actual))."
            }
            return "Torii data model version mismatch (expected \(expected), missing on node)."
        }
    }
}

public protocol ToriiTransactionSubmitting: AnyObject {
    func submitTransaction(data: Data,
                           mode: PipelineEndpointMode,
                           idempotencyKey: String?) async throws -> ToriiSubmitTransactionResponse?
    @discardableResult
    func submitTransaction(data: Data,
                           mode: PipelineEndpointMode,
                           idempotencyKey: String?,
                           completion: @escaping (Swift.Result<ToriiSubmitTransactionResponse?, Swift.Error>) -> Void) -> Task<Void, Never>
}

public extension ToriiTransactionSubmitting where Self: Sendable {
    func submitTransaction(data: Data) async throws -> ToriiSubmitTransactionResponse? {
        try await submitTransaction(data: data, mode: .pipeline, idempotencyKey: nil)
    }

    func submitTransaction(data: Data,
                           mode: PipelineEndpointMode) async throws -> ToriiSubmitTransactionResponse? {
        try await submitTransaction(data: data, mode: mode, idempotencyKey: nil)
    }

    @discardableResult
    func submitTransaction(data: Data,
                           completion: @escaping (Swift.Result<ToriiSubmitTransactionResponse?, Swift.Error>) -> Void) -> Task<Void, Never> {
        submitTransaction(data: data, mode: .pipeline, idempotencyKey: nil, completion: completion)
    }

    @discardableResult
    func submitTransaction(data: Data,
                           mode: PipelineEndpointMode,
                           idempotencyKey: String? = nil,
                           completion: @escaping (Swift.Result<ToriiSubmitTransactionResponse?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runCompletionTask(operation: { [self] in
            try await submitTransaction(data: data, mode: mode, idempotencyKey: idempotencyKey)
        }, completion: completion)
    }
}

public final class ToriiClient: ToriiTransactionSubmitting, @unchecked Sendable {
    /// Base URL for all requests. Normalized to directory URL (ends with "/") to ensure
    /// correct relative URL resolution per RFC 3986.
    public let baseURL: URL
    private let session: URLSession
    private var statusState = ToriiStatusState()
    private var dataModelCompatibility = ToriiDataModelCompatibility.unknown
    private let dataModelQueue = DispatchQueue(label: "org.hyperledger.iroha.torii.data-model")
    private static let defaultListPageSize = 100

    public init(baseURL: URL, session: URLSession = .shared) {
        // Normalize to directory URL for correct relative URL resolution.
        // Without trailing slash, URL(string:relativeTo:) replaces the last path component
        // instead of appending (per RFC 3986).
        self.baseURL = baseURL.hasDirectoryPath ? baseURL : baseURL.appendingPathComponent("")
        self.session = session
    }

    /// Invalidates the underlying URLSession and cancels all outstanding tasks.
    /// Call this method when you are done using the client to release resources.
    public func invalidateAndCancel() {
        session.invalidateAndCancel()
    }

    // MARK: - Public completion-based API

    @discardableResult
    public func getAssets(accountId: String,
                          limit: Int = 100,
                          assetId: String? = nil,
                          completion: @escaping (Result<[ToriiAssetBalance], Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getAssets(accountId: accountId, limit: limit, assetId: assetId) }
    }

    @discardableResult
    public func getTransactions(accountId: String,
                                limit: Int = 50,
                                offset: Int = 0,
                                assetId: String? = nil,
                                completion: @escaping (Result<ToriiTxEnvelope, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getTransactions(accountId: accountId, limit: limit, offset: offset, assetId: assetId) }
    }

    @discardableResult
    public func getUaidPortfolio(uaid: String,
                                 query: ToriiUaidPortfolioQuery? = nil,
                                 completion: @escaping (Result<ToriiUaidPortfolioResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getUaidPortfolio(uaid: uaid, query: query) }
    }

    @discardableResult
    public func getUaidBindings(uaid: String,
                                query: ToriiUaidBindingsQuery? = nil,
                                completion: @escaping (Result<ToriiUaidBindingsResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getUaidBindings(uaid: uaid, query: query) }
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func getExplorerAccountQr(accountId: String,
                                     addressFormat: AccountAddressFormat? = nil,
                                     completion: @escaping (Result<ToriiExplorerAccountQr, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getExplorerAccountQr(accountId: accountId, addressFormat: addressFormat)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func getExplorerInstructions(params: ToriiExplorerInstructionsParams? = nil,
                                         completion: @escaping (Result<ToriiExplorerInstructionsPage, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getExplorerInstructions(params: params) }
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func getExplorerTransactions(params: ToriiExplorerTransactionsParams? = nil,
                                         completion: @escaping (Result<ToriiExplorerTransactionsPage, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getExplorerTransactions(params: params) }
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func getExplorerTransactionDetail(hashHex: String,
                                              addressFormat: AccountAddressFormat? = nil,
                                              completion: @escaping (Result<ToriiExplorerTransactionDetail, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getExplorerTransactionDetail(hashHex: hashHex, addressFormat: addressFormat)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func getExplorerInstructionDetail(hashHex: String,
                                              index: UInt64,
                                              addressFormat: AccountAddressFormat? = nil,
                                              completion: @escaping (Result<ToriiExplorerInstructionItem, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getExplorerInstructionDetail(hashHex: hashHex,
                                                        index: index,
                                                        addressFormat: addressFormat)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func getExplorerTransfers(params: ToriiExplorerInstructionsParams? = nil,
                                     matchingAccount accountId: String? = nil,
                                     assetDefinitionId: String? = nil,
                                     assetId: String? = nil,
                                     completion: @escaping (Result<[ToriiExplorerTransferRecord], Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getExplorerTransfers(params: params,
                                                matchingAccount: accountId,
                                                assetDefinitionId: assetDefinitionId,
                                                assetId: assetId)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func getExplorerTransferSummaries(params: ToriiExplorerInstructionsParams? = nil,
                                             matchingAccount accountId: String? = nil,
                                             assetDefinitionId: String? = nil,
                                             assetId: String? = nil,
                                             relativeTo relativeAccountId: String? = nil,
                                             completion: @escaping (Result<[ToriiExplorerTransferSummary], Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getExplorerTransferSummaries(params: params,
                                                        matchingAccount: accountId,
                                                        assetDefinitionId: assetDefinitionId,
                                                        assetId: assetId,
                                                        relativeTo: relativeAccountId)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func getExplorerTransactionTransfers(hashHex: String,
                                                addressFormat: AccountAddressFormat? = nil,
                                                matchingAccount accountId: String? = nil,
                                                assetDefinitionId: String? = nil,
                                                assetId: String? = nil,
                                                maxItems: UInt64? = nil,
                                                completion: @escaping (Result<[ToriiExplorerTransferRecord], Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getExplorerTransactionTransfers(hashHex: hashHex,
                                                           addressFormat: addressFormat,
                                                           matchingAccount: accountId,
                                                           assetDefinitionId: assetDefinitionId,
                                                           assetId: assetId,
                                                           maxItems: maxItems)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func getExplorerTransactionTransferSummaries(hashHex: String,
                                                        addressFormat: AccountAddressFormat? = nil,
                                                        matchingAccount accountId: String? = nil,
                                                        assetDefinitionId: String? = nil,
                                                        assetId: String? = nil,
                                                        relativeTo relativeAccountId: String? = nil,
                                                        maxItems: UInt64? = nil,
                                                        completion: @escaping (Result<[ToriiExplorerTransferSummary], Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getExplorerTransactionTransferSummaries(hashHex: hashHex,
                                                                   addressFormat: addressFormat,
                                                                   matchingAccount: accountId,
                                                                   assetDefinitionId: assetDefinitionId,
                                                                   assetId: assetId,
                                                                   relativeTo: relativeAccountId,
                                                                   maxItems: maxItems)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func getAccountTransferHistory(accountId: String,
                                          page: UInt64? = nil,
                                          perPage: UInt64? = nil,
                                          addressFormat: AccountAddressFormat? = nil,
                                          assetDefinitionId: String? = nil,
                                          assetId: String? = nil,
                                          completion: @escaping (Result<[ToriiExplorerTransferSummary], Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getAccountTransferHistory(accountId: accountId,
                                                     page: page,
                                                     perPage: perPage,
                                                     addressFormat: addressFormat,
                                                     assetDefinitionId: assetDefinitionId,
                                                     assetId: assetId)
        }
    }

    /// Convenience alias for transfer-rich transaction history derived from explorer instructions.
    @available(iOS 15.0, macOS 12.0, *)
    @discardableResult
    public func getTransactionHistory(accountId: String,
                                      page: UInt64? = nil,
                                      perPage: UInt64? = nil,
                                      addressFormat: AccountAddressFormat? = nil,
                                      assetDefinitionId: String? = nil,
                                      assetId: String? = nil,
                                      completion: @escaping (Result<[ToriiExplorerTransferSummary], Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getTransactionHistory(accountId: accountId,
                                                 page: page,
                                                 perPage: perPage,
                                                 addressFormat: addressFormat,
                                                 assetDefinitionId: assetDefinitionId,
                                                 assetId: assetId)
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func iterateAccountTransferHistory(accountId: String,
                                              page: UInt64? = nil,
                                              perPage: UInt64? = nil,
                                              addressFormat: AccountAddressFormat? = nil,
                                              assetDefinitionId: String? = nil,
                                              assetId: String? = nil,
                                              maxItems: UInt64? = nil) -> AsyncThrowingStream<ToriiExplorerTransferSummary, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let normalizedAccount = try ToriiRequestValidation.normalizedNonEmpty(accountId,
                                                                                           field: "accountId")
                    var currentPage = page ?? 1
                    var currentPerPage = perPage
                    var remaining = maxItems
                    while true {
                        try Task.checkCancellation()
                        let params = ToriiExplorerInstructionsParams(page: currentPage,
                                                                     perPage: currentPerPage,
                                                                     addressFormat: addressFormat,
                                                                     kind: "Transfer",
                                                                     assetId: assetId)
                        let response = try await getExplorerInstructions(params: params)
                        if currentPerPage == nil {
                            currentPerPage = response.pagination.perPage
                        }
                        let summaries = response.transferSummaries(matchingAccount: normalizedAccount,
                                                                   assetDefinitionId: assetDefinitionId,
                                                                   assetId: assetId,
                                                                   relativeTo: normalizedAccount)
                        for summary in summaries {
                            continuation.yield(summary)
                            if let remainingValue = remaining {
                                if remainingValue <= 1 {
                                    remaining = 0
                                    break
                                }
                                remaining = remainingValue - 1
                            }
                        }
                        if remaining == 0 {
                            break
                        }
                        if response.items.isEmpty || response.pagination.totalPages == 0 {
                            break
                        }
                        if currentPage >= response.pagination.totalPages {
                            break
                        }
                        currentPage += 1
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }

    @discardableResult
    public func listDomains(options: ToriiListOptions = ToriiListOptions(),
                            completion: @escaping (Result<ToriiDomainListPage, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listDomains(options: options) }
    }

    @discardableResult
    public func listSubscriptionPlans(params: ToriiSubscriptionPlanListParams? = nil,
                                      completion: @escaping (Result<ToriiSubscriptionPlanListResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listSubscriptionPlans(params: params) }
    }

    @discardableResult
    public func createSubscriptionPlan(_ requestBody: ToriiSubscriptionPlanCreateRequest,
                                       completion: @escaping (Result<ToriiSubscriptionPlanCreateResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.createSubscriptionPlan(requestBody) }
    }

    @discardableResult
    public func listSubscriptions(params: ToriiSubscriptionListParams? = nil,
                                  completion: @escaping (Result<ToriiSubscriptionListResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listSubscriptions(params: params) }
    }

    @discardableResult
    public func createSubscription(_ requestBody: ToriiSubscriptionCreateRequest,
                                   completion: @escaping (Result<ToriiSubscriptionCreateResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.createSubscription(requestBody) }
    }

    @discardableResult
    public func getSubscription(subscriptionId: String,
                                completion: @escaping (Result<ToriiSubscriptionRecord?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getSubscription(subscriptionId: subscriptionId) }
    }

    @discardableResult
    public func pauseSubscription(subscriptionId: String,
                                  requestBody: ToriiSubscriptionActionRequest,
                                  completion: @escaping (Result<ToriiSubscriptionActionResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.pauseSubscription(subscriptionId: subscriptionId, requestBody: requestBody) }
    }

    @discardableResult
    public func resumeSubscription(subscriptionId: String,
                                   requestBody: ToriiSubscriptionActionRequest,
                                   completion: @escaping (Result<ToriiSubscriptionActionResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.resumeSubscription(subscriptionId: subscriptionId, requestBody: requestBody) }
    }

    @discardableResult
    public func cancelSubscription(subscriptionId: String,
                                   requestBody: ToriiSubscriptionActionRequest,
                                   completion: @escaping (Result<ToriiSubscriptionActionResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.cancelSubscription(subscriptionId: subscriptionId, requestBody: requestBody) }
    }

    public func keepSubscription(subscriptionId: String,
                                 requestBody: ToriiSubscriptionActionRequest,
                                 completion: @escaping (Result<ToriiSubscriptionActionResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.keepSubscription(subscriptionId: subscriptionId, requestBody: requestBody) }
    }

    @discardableResult
    public func chargeSubscriptionNow(subscriptionId: String,
                                      requestBody: ToriiSubscriptionActionRequest,
                                      completion: @escaping (Result<ToriiSubscriptionActionResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.chargeSubscriptionNow(subscriptionId: subscriptionId, requestBody: requestBody) }
    }

    @discardableResult
    public func recordSubscriptionUsage(subscriptionId: String,
                                        requestBody: ToriiSubscriptionUsageRequest,
                                        completion: @escaping (Result<ToriiSubscriptionActionResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.recordSubscriptionUsage(subscriptionId: subscriptionId, requestBody: requestBody) }
    }

    @discardableResult
    public func getUaidManifests(uaid: String,
                                 query: ToriiUaidManifestQuery? = nil,
                                 completion: @escaping (Result<ToriiUaidManifestsResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getUaidManifests(uaid: uaid, query: query) }
    }

    @discardableResult
    public func listOfflineAllowances(params: ToriiOfflineListParams? = nil,
                                      completion: @escaping (Result<ToriiOfflineAllowanceList, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listOfflineAllowances(params: params) }
    }

    @discardableResult
    public func listOfflineTransfers(params: ToriiOfflineListParams? = nil,
                                     completion: @escaping (Result<ToriiOfflineTransferList, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listOfflineTransfers(params: params) }
    }

    public func listOfflineSummaries(params: ToriiOfflineListParams? = nil,
                                     completion: @escaping (Result<ToriiOfflineSummaryList, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listOfflineSummaries(params: params) }
    }

    @discardableResult
    public func listOfflineRevocations(params: ToriiOfflineRevocationListParams? = nil,
                                       completion: @escaping (Result<ToriiOfflineRevocationList, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listOfflineRevocations(params: params) }
    }

    @discardableResult
    public func getOfflineBundleProofStatus(params: ToriiOfflineBundleProofStatusParams,
                                            completion: @escaping (Result<ToriiOfflineBundleProofStatus, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getOfflineBundleProofStatus(params: params) }
    }

    @discardableResult
    public func listOfflineReceipts(params: ToriiOfflineReceiptListParams? = nil,
                                    completion: @escaping (Result<ToriiOfflineReceiptList, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listOfflineReceipts(params: params) }
    }

    @discardableResult
    public func queryOfflineReceipts(_ envelope: ToriiQueryEnvelope,
                                     completion: @escaping (Result<ToriiOfflineReceiptList, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.queryOfflineReceipts(envelope) }
    }

    @discardableResult
    public func queryOfflineAllowances(_ envelope: ToriiQueryEnvelope,
                                       completion: @escaping (Result<ToriiOfflineAllowanceList, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.queryOfflineAllowances(envelope) }
    }

    @discardableResult
    public func queryOfflineCertificates(_ envelope: ToriiQueryEnvelope,
                                         completion: @escaping (Result<ToriiOfflineAllowanceList, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.queryOfflineCertificates(envelope) }
    }

    @discardableResult
    public func queryOfflineTransfers(_ envelope: ToriiQueryEnvelope,
                                      completion: @escaping (Result<ToriiOfflineTransferList, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.queryOfflineTransfers(envelope) }
    }

    @discardableResult
    public func queryOfflineSettlements(_ envelope: ToriiQueryEnvelope,
                                        completion: @escaping (Result<ToriiOfflineTransferList, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.queryOfflineSettlements(envelope) }
    }

    @discardableResult
    public func queryOfflineRevocations(_ envelope: ToriiQueryEnvelope,
                                        completion: @escaping (Result<ToriiOfflineRevocationList, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.queryOfflineRevocations(envelope) }
    }

    @discardableResult
    public func getOfflineState(completion: @escaping (Result<ToriiOfflineStateResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getOfflineState() }
    }

    @discardableResult
    public func issueOfflineCertificate(_ requestBody: ToriiOfflineCertificateIssueRequest,
                                        completion: @escaping (Result<ToriiOfflineCertificateIssueResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.issueOfflineCertificate(requestBody) }
    }

    @discardableResult
    public func registerOfflineAllowance(_ requestBody: ToriiOfflineAllowanceRegisterRequest,
                                         completion: @escaping (Result<ToriiOfflineAllowanceRegisterResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.registerOfflineAllowance(requestBody) }
    }

    @discardableResult
    public func renewOfflineAllowance(certificateIdHex: String,
                                      requestBody: ToriiOfflineAllowanceRegisterRequest,
                                      completion: @escaping (Result<ToriiOfflineAllowanceRegisterResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.renewOfflineAllowance(certificateIdHex: certificateIdHex,
                                                                   requestBody: requestBody) }
    }

    @discardableResult
    public func issueOfflineCertificateRenewal(certificateIdHex: String,
                                               requestBody: ToriiOfflineCertificateIssueRequest,
                                               completion: @escaping (Result<ToriiOfflineCertificateIssueResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.issueOfflineCertificateRenewal(certificateIdHex: certificateIdHex,
                                                                            requestBody: requestBody) }
    }

    @discardableResult
    public func topUpOfflineAllowance(draft: OfflineWalletCertificateDraft,
                                      authority: String,
                                      privateKey: String,
                                      completion: @escaping (Result<ToriiOfflineTopUpResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.topUpOfflineAllowance(draft: draft,
                                                                   authority: authority,
                                                                   privateKey: privateKey) }
    }

    @discardableResult
    public func topUpOfflineAllowanceRenewal(certificateIdHex: String,
                                             draft: OfflineWalletCertificateDraft,
                                             authority: String,
                                             privateKey: String,
                                             completion: @escaping (Result<ToriiOfflineTopUpResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.topUpOfflineAllowanceRenewal(certificateIdHex: certificateIdHex,
                                                                          draft: draft,
                                                                          authority: authority,
                                                                          privateKey: privateKey) }
    }

    @discardableResult
    public func submitOfflineSpendReceipts(_ requestBody: ToriiOfflineSpendReceiptsSubmitRequest,
                                           completion: @escaping (Result<ToriiOfflineSpendReceiptsSubmitResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.submitOfflineSpendReceipts(requestBody) }
    }

    @discardableResult
    public func submitOfflineSettlement(_ requestBody: ToriiOfflineSettlementSubmitRequest,
                                        completion: @escaping (Result<ToriiOfflineSettlementSubmitResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.submitOfflineSettlement(requestBody) }
    }

    @discardableResult
    public func requestOfflineTransferProof(_ requestBody: ToriiOfflineTransferProofRequest,
                                            completion: @escaping (Result<ToriiJSONValue, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.requestOfflineTransferProof(requestBody) }
    }

    @discardableResult
    public func getConnectStatus(completion: @escaping (Result<ToriiConnectStatusSnapshot?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getConnectStatus() }
    }

    @discardableResult
    public func createConnectSession(sid: String,
                                     node: String? = nil,
                                     completion: @escaping (Result<ToriiConnectSessionResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.createConnectSession(sid: sid, node: node) }
    }

    @discardableResult
    public func deleteConnectSession(sid: String,
                                     completion: @escaping (Result<Bool, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.deleteConnectSession(sid: sid) }
    }

    @discardableResult
    public func listConnectApps(options: ToriiConnectAppListOptions = ToriiConnectAppListOptions(),
                                completion: @escaping (Result<ToriiConnectAppRegistryPage, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listConnectApps(options: options) }
    }

    @discardableResult
    public func getConnectApp(appId: String,
                              completion: @escaping (Result<ToriiConnectAppRecord, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getConnectApp(appId: appId) }
    }

    @discardableResult
    public func registerConnectApp(_ input: ToriiConnectAppUpsertInput,
                                   completion: @escaping (Result<ToriiConnectAppRecord?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.registerConnectApp(input) }
    }

    @discardableResult
    public func deleteConnectApp(appId: String,
                                 completion: @escaping (Result<Bool, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.deleteConnectApp(appId: appId) }
    }

    @discardableResult
    public func getConnectAppPolicy(completion: @escaping (Result<ToriiConnectAppPolicyControls, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getConnectAppPolicy() }
    }

    @discardableResult
    public func updateConnectAppPolicy(_ updates: ToriiConnectAppPolicyUpdate,
                                       completion: @escaping (Result<ToriiConnectAppPolicyControls, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.updateConnectAppPolicy(updates) }
    }

    @discardableResult
    public func getConnectAdmissionManifest(completion: @escaping (Result<ToriiConnectAdmissionManifest, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getConnectAdmissionManifest() }
    }

    @discardableResult
    public func setConnectAdmissionManifest(_ manifest: ToriiConnectAdmissionManifestInput,
                                            completion: @escaping (Result<ToriiConnectAdmissionManifest, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.setConnectAdmissionManifest(manifest) }
    }

    @discardableResult
    public func uploadAttachment(data: Data, contentType: String, completion: @escaping (Result<ToriiAttachmentMeta, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.uploadAttachment(data: data, contentType: contentType) }
    }

    @discardableResult
    public func listAttachments(completion: @escaping (Result<[ToriiAttachmentMeta], Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listAttachments() }
    }

    @discardableResult
    public func getAttachment(id: String, completion: @escaping (Result<(Data, String?), Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getAttachment(id: id) }
    }

    @discardableResult
    public func deleteAttachment(id: String, completion: @escaping (Result<Void, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.deleteAttachment(id: id) }
    }

    @discardableResult
    public func getDaManifestBundle(storageTicketHex: String,
                                    blockHashHex: String? = nil,
                                    completion: @escaping (Result<ToriiDaManifestBundle, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getDaManifestBundle(storageTicketHex: storageTicketHex, blockHashHex: blockHashHex) }
    }

    @discardableResult
    public func fetchDaPayloadViaGateway(storageTicketHex: String? = nil,
                                         manifestBundle: ToriiDaManifestBundle? = nil,
                                         chunkerHandle: String? = nil,
                                         providers: [SorafsGatewayProvider],
                                         options: SorafsGatewayFetchOptions? = nil,
                                         proofSummaryOptions: ToriiDaProofSummaryOptions? = nil,
                                         orchestrator: SorafsGatewayFetching = SorafsOrchestratorClient(),
                                         proofSummaryGenerator: DaProofSummaryGenerating = NativeDaProofSummaryGenerator.shared,
                                         cancellationHandler: (@Sendable () -> Void)? = nil,
                                         completion: @escaping (Result<ToriiDaGatewayFetchResult, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.fetchDaPayloadViaGateway(storageTicketHex: storageTicketHex,
                                                                      manifestBundle: manifestBundle,
                                                                      chunkerHandle: chunkerHandle,
                                                                      providers: providers,
                                                                      options: options,
                                                                      proofSummaryOptions: proofSummaryOptions,
                                                                      orchestrator: orchestrator,
                                                                      proofSummaryGenerator: proofSummaryGenerator,
                                                                      cancellationHandler: cancellationHandler) }
    }

    @discardableResult
    public func getHealth(completion: @escaping (Result<String, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getHealth() }
    }

    @discardableResult
    public func getStatusSnapshot(completion: @escaping (Result<ToriiStatusSnapshot, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getStatusSnapshot() }
    }

    @discardableResult
    public func getMetrics(asText: Bool = false,
                           completion: @escaping (Result<ToriiMetricsResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getMetrics(asText: asText) }
    }

    @discardableResult
    public func getNodeCapabilities(completion: @escaping (Result<ToriiNodeCapabilities, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getNodeCapabilities() }
    }

    @discardableResult
    public func getRuntimeMetrics(completion: @escaping (Result<ToriiRuntimeMetrics, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getRuntimeMetrics() }
    }

    @discardableResult
    public func getRuntimeAbiActive(completion: @escaping (Result<ToriiRuntimeAbiActive, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getRuntimeAbiActive() }
    }

    @discardableResult
    public func getRuntimeAbiHash(completion: @escaping (Result<ToriiRuntimeAbiHash, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getRuntimeAbiHash() }
    }

    @discardableResult
    public func listRuntimeUpgrades(completion: @escaping (Result<[ToriiRuntimeUpgradeListItem], Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listRuntimeUpgrades() }
    }

    @discardableResult
    public func proposeRuntimeUpgrade(manifest: ToriiRuntimeUpgradeManifest,
                                      completion: @escaping (Result<ToriiRuntimeUpgradeActionResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.proposeRuntimeUpgrade(manifest: manifest) }
    }

    @discardableResult
    public func activateRuntimeUpgrade(idHex: String,
                                       completion: @escaping (Result<ToriiRuntimeUpgradeActionResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.activateRuntimeUpgrade(idHex: idHex) }
    }

    @discardableResult
    public func cancelRuntimeUpgrade(idHex: String,
                                     completion: @escaping (Result<ToriiRuntimeUpgradeActionResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.cancelRuntimeUpgrade(idHex: idHex) }
    }

    @discardableResult
    public func getVerifyingKey(backend: String,
                                name: String,
                                completion: @escaping (Result<ToriiVerifyingKeyDetail, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getVerifyingKey(backend: backend, name: name) }
    }

    @discardableResult
    public func listVerifyingKeys(query: ToriiVerifyingKeyListQuery? = nil,
                                  completion: @escaping (Result<[ToriiVerifyingKeyListItem], Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listVerifyingKeys(query: query) }
    }

    @discardableResult
    public func registerVerifyingKey(_ requestBody: ToriiVerifyingKeyRegisterRequest,
                                     completion: @escaping (Result<Void, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.registerVerifyingKey(requestBody)
            return ()
        }
    }

    @discardableResult
    public func updateVerifyingKey(_ requestBody: ToriiVerifyingKeyUpdateRequest,
                                   completion: @escaping (Result<Void, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.updateVerifyingKey(requestBody)
            return ()
        }
    }

    @discardableResult
    public func listProverReports(filter: ToriiProverReportsFilter? = nil, completion: @escaping (Result<[ToriiProverReport], Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.listProverReports(filter: filter) }
    }

    @discardableResult
    public func getProverReport(id: String, completion: @escaping (Result<ToriiProverReport, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getProverReport(id: id) }
    }

    @discardableResult
    public func deleteProverReport(id: String, completion: @escaping (Result<Void, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.deleteProverReport(id: id) }
    }

    @discardableResult
    public func countProverReports(filter: ToriiProverReportsFilter? = nil, completion: @escaping (Result<UInt64, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.countProverReports(filter: filter) }
    }

    @discardableResult
    public func registerContractCode(_ requestBody: ToriiRegisterContractCodeRequest, completion: @escaping (Result<Void, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.registerContractCode(requestBody) }
    }

    @discardableResult
    public func fetchContractManifest(codeHashHex: String, completion: @escaping (Result<ToriiContractManifestRecord, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.fetchContractManifest(codeHashHex: codeHashHex) }
    }

    @discardableResult
    public func deployContract(_ requestBody: ToriiDeployContractRequest, completion: @escaping (Result<ToriiDeployContractResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.deployContract(requestBody) }
    }

    @discardableResult
    public func deployContractInstance(_ requestBody: ToriiDeployContractInstanceRequest,
                                       completion: @escaping (Result<ToriiDeployContractInstanceResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.deployContractInstance(requestBody) }
    }

    @discardableResult
    public func activateContractInstance(_ requestBody: ToriiActivateContractInstanceRequest,
                                         completion: @escaping (Result<ToriiActivateContractInstanceResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.activateContractInstance(requestBody) }
    }

    @discardableResult
    public func fetchContractCodeBytes(codeHashHex: String, completion: @escaping (Result<ToriiContractCodeBytes, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.fetchContractCodeBytes(codeHashHex: codeHashHex) }
    }

    @discardableResult
    public func submitTransaction(data: Data, completion: @escaping (Result<ToriiSubmitTransactionResponse?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.submitTransaction(data: data) }
    }

    @discardableResult
    public func submitTransaction(data: Data,
                                  idempotencyKey: String?,
                                  completion: @escaping (Result<ToriiSubmitTransactionResponse?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.submitTransaction(data: data, mode: .pipeline, idempotencyKey: idempotencyKey) }
    }

    @discardableResult
    public func getTransactionStatus(hashHex: String, completion: @escaping (Result<ToriiPipelineTransactionStatus?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getTransactionStatus(hashHex: hashHex) }
    }

    @discardableResult
    public func registerAccount(_ requestBody: ToriiAccountOnboardingRequest,
                                completion: @escaping (Result<ToriiAccountOnboardingResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.registerAccount(requestBody) }
    }

    @discardableResult
    public func getTransactionStatus(hashHex: String,
                                     mode: PipelineEndpointMode,
                                     completion: @escaping (Result<ToriiPipelineTransactionStatus?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getTransactionStatus(hashHex: hashHex, mode: mode) }
    }

    @discardableResult
    public func getPipelineRecovery(height: UInt64, completion: @escaping (Result<ToriiPipelineRecovery?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getPipelineRecovery(height: height) }
    }

    @discardableResult
    public func getTimeNow(completion: @escaping (Result<ToriiTimeSnapshot, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getTimeNow() }
    }

    @discardableResult
    public func getTimeStatus(completion: @escaping (Result<ToriiTimeStatusSnapshot, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getTimeStatus() }
    }

    @discardableResult
    public func getSumeragiStatus(completion: @escaping (Result<ToriiSumeragiStatusSnapshot, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getSumeragiStatus() }
    }

    @discardableResult
    public func getSumeragiCommitQc(blockHashHex: String,
                                    completion: @escaping (Result<ToriiSumeragiCommitQcRecord, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getSumeragiCommitQc(blockHashHex: blockHashHex) }
    }

    // MARK: - Governance (Completion)

    @discardableResult
    public func submitGovernanceDeployContractProposal(_ requestBody: ToriiGovernanceDeployContractProposalRequest,
                                                       completion: @escaping (Result<ToriiGovernanceProposalResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.submitGovernanceDeployContractProposal(requestBody) }
    }

    @discardableResult
    public func submitGovernancePlainBallot(_ requestBody: ToriiGovernancePlainBallotRequest,
                                            completion: @escaping (Result<ToriiGovernanceBallotResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.submitGovernancePlainBallot(requestBody) }
    }

    @discardableResult
    public func submitGovernanceZkBallot(_ requestBody: ToriiGovernanceZkBallotRequest,
                                         completion: @escaping (Result<ToriiGovernanceBallotResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.submitGovernanceZkBallot(requestBody) }
    }

    @discardableResult
    public func finalizeGovernanceReferendum(_ requestBody: ToriiGovernanceFinalizeRequest,
                                             completion: @escaping (Result<ToriiGovernanceFinalizeResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.finalizeGovernanceReferendum(requestBody) }
    }

    @discardableResult
    public func enactGovernanceProposal(_ requestBody: ToriiGovernanceEnactRequest,
                                        completion: @escaping (Result<ToriiGovernanceEnactResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.enactGovernanceProposal(requestBody) }
    }

    @discardableResult
    public func getGovernanceProposal(idHex: String,
                                      completion: @escaping (Result<ToriiGovernanceProposalGetResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getGovernanceProposal(idHex: idHex) }
    }

    @discardableResult
    public func getGovernanceLocks(referendumId: String,
                                   completion: @escaping (Result<ToriiGovernanceLocksResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getGovernanceLocks(referendumId: referendumId) }
    }

    @discardableResult
    public func getGovernanceReferendum(id: String,
                                        completion: @escaping (Result<ToriiGovernanceReferendumResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getGovernanceReferendum(id: id) }
    }

    @discardableResult
    public func getGovernanceTally(id: String,
                                   completion: @escaping (Result<ToriiGovernanceTallyResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getGovernanceTally(id: id) }
    }

    @discardableResult
    public func getGovernanceUnlockStats(height: UInt64? = nil,
                                         referendumId: String? = nil,
                                         completion: @escaping (Result<ToriiGovernanceUnlockStatsResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) { try await self.getGovernanceUnlockStats(height: height, referendumId: referendumId) }
    }

    // MARK: - Async API

    public func registerAccount(_ requestBody: ToriiAccountOnboardingRequest) async throws -> ToriiAccountOnboardingResponse {
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/accounts/onboard",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let (data, response) = try await send(request)
        try ensureStatus(response, equals: 202)
        return try decodeJSON(ToriiAccountOnboardingResponse.self, from: data)
    }

    public func getAssets(accountId: String, limit: Int = 100, assetId: String? = nil) async throws -> [ToriiAssetBalance] {
        let encodedAccountId = try encodeAccountIdPath(accountId)
        var items = [URLQueryItem(name: "limit", value: String(limit))]
        if let assetId = assetId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !assetId.isEmpty {
            items.append(URLQueryItem(name: "asset_id", value: assetId))
        }
        let request = try makeRequest(path: "/v1/accounts/\(encodedAccountId)/assets",
                                      queryItems: items)
        let data = try await data(for: request)
        return try decodeAssetBalances(from: data)
    }

    /// Returns account transaction metadata from `/v1/accounts/{account_id}/transactions`.
    /// Use `getTransactionHistory`/`getAccountTransferHistory` for transfer details.
    public func getTransactions(accountId: String, limit: Int = 50, offset: Int = 0, assetId: String? = nil) async throws -> ToriiTxEnvelope {
        let encodedAccountId = try encodeAccountIdPath(accountId)
        var items = [
            URLQueryItem(name: "limit", value: String(limit)),
            URLQueryItem(name: "offset", value: String(offset))
        ]
        if let assetId = assetId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !assetId.isEmpty {
            items.append(URLQueryItem(name: "asset_id", value: assetId))
        }
        let request = try makeRequest(path: "/v1/accounts/\(encodedAccountId)/transactions", queryItems: items)
        let data = try await data(for: request)
        return try decodeTransactionEnvelope(from: data)
    }

    public func getExplorerAccountQr(accountId: String,
                                     addressFormat: AccountAddressFormat? = nil) async throws -> ToriiExplorerAccountQr {
        let encodedAccountId = try encodeAccountIdPath(accountId)
        var queryItems: [URLQueryItem]? = nil
        if let addressFormat {
            let value = try ToriiClient.explorerAddressFormatQueryValue(addressFormat,
                                                                        context: "explorer QR payloads")
            queryItems = [URLQueryItem(name: "address_format", value: value)]
        }
        let request = try makeRequest(path: "/v1/explorer/accounts/\(encodedAccountId)/qr", queryItems: queryItems)
        let data = try await data(for: request)
        return try decodeJSON(ToriiExplorerAccountQr.self, from: data)
    }

    public func getExplorerInstructions(params: ToriiExplorerInstructionsParams? = nil) async throws -> ToriiExplorerInstructionsPage {
        let request = try makeRequest(path: "/v1/explorer/instructions",
                                      queryItems: try params?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiExplorerInstructionsPage.self, from: data)
    }

    public func getExplorerTransactions(params: ToriiExplorerTransactionsParams? = nil) async throws -> ToriiExplorerTransactionsPage {
        let request = try makeRequest(path: "/v1/explorer/transactions",
                                      queryItems: try params?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiExplorerTransactionsPage.self, from: data)
    }

    public func getExplorerTransactionDetail(hashHex: String,
                                              addressFormat: AccountAddressFormat? = nil) async throws -> ToriiExplorerTransactionDetail {
        let normalizedHash = try ToriiRequestValidation.normalizedNonEmpty(hashHex, field: "hashHex")
        let encodedHash = encodePathComponent(normalizedHash)
        var queryItems: [URLQueryItem]? = nil
        if let addressFormat {
            let value = try ToriiClient.explorerAddressFormatQueryValue(addressFormat,
                                                                        context: "explorer transactions")
            queryItems = [URLQueryItem(name: "address_format", value: value)]
        }
        let request = try makeRequest(path: "/v1/explorer/transactions/\(encodedHash)",
                                      queryItems: queryItems)
        let data = try await data(for: request)
        return try decodeJSON(ToriiExplorerTransactionDetail.self, from: data)
    }

    public func getExplorerInstructionDetail(hashHex: String,
                                              index: UInt64,
                                              addressFormat: AccountAddressFormat? = nil) async throws -> ToriiExplorerInstructionItem {
        let normalizedHash = try ToriiRequestValidation.normalizedNonEmpty(hashHex, field: "hashHex")
        let encodedHash = encodePathComponent(normalizedHash)
        var queryItems: [URLQueryItem]? = nil
        if let addressFormat {
            let value = try ToriiClient.explorerAddressFormatQueryValue(addressFormat,
                                                                        context: "explorer instructions")
            queryItems = [URLQueryItem(name: "address_format", value: value)]
        }
        let request = try makeRequest(path: "/v1/explorer/instructions/\(encodedHash)/\(index)",
                                      queryItems: queryItems)
        let data = try await data(for: request)
        return try decodeJSON(ToriiExplorerInstructionItem.self, from: data)
    }

    public func getExplorerTransfers(params: ToriiExplorerInstructionsParams? = nil,
                                     matchingAccount accountId: String? = nil,
                                     assetDefinitionId: String? = nil,
                                     assetId: String? = nil) async throws -> [ToriiExplorerTransferRecord] {
        var effectiveParams = params ?? ToriiExplorerInstructionsParams()
        let kindTrimmed = effectiveParams.kind?.trimmingCharacters(in: .whitespacesAndNewlines)
        if kindTrimmed == nil || kindTrimmed?.isEmpty == true {
            effectiveParams.kind = "Transfer"
        }
        if let assetId = assetId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !assetId.isEmpty {
            let existing = effectiveParams.assetId?.trimmingCharacters(in: .whitespacesAndNewlines)
            if existing == nil || existing?.isEmpty == true {
                effectiveParams.assetId = assetId
            }
        }
        let page = try await getExplorerInstructions(params: effectiveParams)
        return page.transferRecords(matchingAccount: accountId,
                                    assetDefinitionId: assetDefinitionId,
                                    assetId: assetId)
    }

    public func getExplorerTransferSummaries(params: ToriiExplorerInstructionsParams? = nil,
                                             matchingAccount accountId: String? = nil,
                                             assetDefinitionId: String? = nil,
                                             assetId: String? = nil,
                                             relativeTo relativeAccountId: String? = nil) async throws -> [ToriiExplorerTransferSummary] {
        var effectiveParams = params ?? ToriiExplorerInstructionsParams()
        let kindTrimmed = effectiveParams.kind?.trimmingCharacters(in: .whitespacesAndNewlines)
        if kindTrimmed == nil || kindTrimmed?.isEmpty == true {
            effectiveParams.kind = "Transfer"
        }
        if let assetId = assetId?.trimmingCharacters(in: .whitespacesAndNewlines),
           !assetId.isEmpty {
            let existing = effectiveParams.assetId?.trimmingCharacters(in: .whitespacesAndNewlines)
            if existing == nil || existing?.isEmpty == true {
                effectiveParams.assetId = assetId
            }
        }
        let page = try await getExplorerInstructions(params: effectiveParams)
        return page.transferSummaries(matchingAccount: accountId,
                                      assetDefinitionId: assetDefinitionId,
                                      assetId: assetId,
                                      relativeTo: relativeAccountId)
    }

    public func getExplorerTransactionTransfers(hashHex: String,
                                                addressFormat: AccountAddressFormat? = nil,
                                                matchingAccount accountId: String? = nil,
                                                assetDefinitionId: String? = nil,
                                                assetId: String? = nil,
                                                maxItems: UInt64? = nil) async throws -> [ToriiExplorerTransferRecord] {
        if let maxItems, maxItems == 0 {
            return []
        }
        let normalizedHash = try ToriiRequestValidation.normalizedNonEmpty(hashHex, field: "hashHex")
        var currentPage: UInt64 = 1
        var currentPerPage: UInt64?
        var remaining = maxItems
        var records: [ToriiExplorerTransferRecord] = []
        while true {
            let params = ToriiExplorerInstructionsParams(page: currentPage,
                                                         perPage: currentPerPage,
                                                         addressFormat: addressFormat,
                                                         transactionHash: normalizedHash,
                                                         kind: "Transfer",
                                                         assetId: assetId)
            let response = try await getExplorerInstructions(params: params)
            if currentPerPage == nil {
                currentPerPage = response.pagination.perPage
            }
            let pageRecords = response.transferRecords(matchingAccount: accountId,
                                                       assetDefinitionId: assetDefinitionId,
                                                       assetId: assetId)
            for record in pageRecords {
                records.append(record)
                if let remainingValue = remaining {
                    if remainingValue <= 1 {
                        remaining = 0
                        break
                    }
                    remaining = remainingValue - 1
                }
            }
            if remaining == 0 {
                break
            }
            if response.items.isEmpty || response.pagination.totalPages == 0 {
                break
            }
            if currentPage >= response.pagination.totalPages {
                break
            }
            currentPage += 1
        }
        return records
    }

    public func getExplorerTransactionTransferSummaries(hashHex: String,
                                                        addressFormat: AccountAddressFormat? = nil,
                                                        matchingAccount accountId: String? = nil,
                                                        assetDefinitionId: String? = nil,
                                                        assetId: String? = nil,
                                                        relativeTo relativeAccountId: String? = nil,
                                                        maxItems: UInt64? = nil) async throws -> [ToriiExplorerTransferSummary] {
        let records = try await getExplorerTransactionTransfers(hashHex: hashHex,
                                                                addressFormat: addressFormat,
                                                                matchingAccount: accountId,
                                                                assetDefinitionId: assetDefinitionId,
                                                                assetId: assetId,
                                                                maxItems: maxItems)
        let relative = relativeAccountId ?? accountId
        return records.flatMap { $0.summaries(relativeTo: relative) }
    }

    /// Stream transfer summaries for a specific transaction, emitting history first then live updates.
    @available(iOS 15.0, macOS 12.0, *)
    public func streamTransactionTransferSummaries(hashHex: String,
                                                   addressFormat: AccountAddressFormat? = nil,
                                                   matchingAccount accountId: String? = nil,
                                                   assetDefinitionId: String? = nil,
                                                   assetId: String? = nil,
                                                   relativeTo relativeAccountId: String? = nil,
                                                   lastEventId: String? = nil,
                                                   maxItems: UInt64? = nil,
                                                   dedupeLimit: Int = 10_000) -> AsyncThrowingStream<ToriiExplorerTransferSummary, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    if let maxItems, maxItems == 0 {
                        continuation.finish()
                        return
                    }
                    let normalizedHash = try ToriiRequestValidation.normalizedNonEmpty(hashHex, field: "hashHex")
                    let relative = relativeAccountId ?? accountId
                    var remaining = maxItems
                    var deduper = TransferSummaryDeduper(limit: dedupeLimit)

                    func emit(_ summary: ToriiExplorerTransferSummary) -> Bool {
                        guard deduper.shouldYield(summary) else {
                            return true
                        }
                        continuation.yield(summary)
                        if let remainingValue = remaining {
                            if remainingValue <= 1 {
                                remaining = 0
                                return false
                            }
                            remaining = remainingValue - 1
                        }
                        return true
                    }

                    let history = try await getExplorerTransactionTransferSummaries(hashHex: normalizedHash,
                                                                                    addressFormat: addressFormat,
                                                                                    matchingAccount: accountId,
                                                                                    assetDefinitionId: assetDefinitionId,
                                                                                    assetId: assetId,
                                                                                    relativeTo: relative,
                                                                                    maxItems: remaining)
                    for summary in history {
                        if Task.isCancelled {
                            break
                        }
                        if !emit(summary) {
                            continuation.finish()
                            return
                        }
                    }

                    if remaining == 0 {
                        continuation.finish()
                        return
                    }

                    for try await summary in streamExplorerTransferSummaries(lastEventId: lastEventId,
                                                                             matchingAccount: accountId,
                                                                             assetDefinitionId: assetDefinitionId,
                                                                             assetId: assetId,
                                                                             relativeTo: relative) {
                        if Task.isCancelled {
                            break
                        }
                        if summary.transactionHash != normalizedHash {
                            continue
                        }
                        if !emit(summary) {
                            continuation.finish()
                            return
                        }
                    }

                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    if Task.isCancelled {
                        continuation.finish()
                    } else {
                        continuation.finish(throwing: error)
                    }
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }

    public func getAccountTransferHistory(accountId: String,
                                          page: UInt64? = nil,
                                          perPage: UInt64? = nil,
                                          addressFormat: AccountAddressFormat? = nil,
                                          assetDefinitionId: String? = nil,
                                          assetId: String? = nil) async throws -> [ToriiExplorerTransferSummary] {
        let normalizedAccount = try ToriiRequestValidation.normalizedNonEmpty(accountId, field: "accountId")
        let params = ToriiExplorerInstructionsParams(page: page,
                                                     perPage: perPage,
                                                     addressFormat: addressFormat,
                                                     kind: "Transfer",
                                                     assetId: assetId)
        return try await getExplorerTransferSummaries(params: params,
                                                      matchingAccount: normalizedAccount,
                                                      assetDefinitionId: assetDefinitionId,
                                                      assetId: assetId,
                                                      relativeTo: normalizedAccount)
    }

    /// Convenience alias for transfer-rich transaction history derived from explorer instructions.
    public func getTransactionHistory(accountId: String,
                                      page: UInt64? = nil,
                                      perPage: UInt64? = nil,
                                      addressFormat: AccountAddressFormat? = nil,
                                      assetDefinitionId: String? = nil,
                                      assetId: String? = nil) async throws -> [ToriiExplorerTransferSummary] {
        try await getAccountTransferHistory(accountId: accountId,
                                            page: page,
                                            perPage: perPage,
                                            addressFormat: addressFormat,
                                            assetDefinitionId: assetDefinitionId,
                                            assetId: assetId)
    }

    public func listDomains(options: ToriiListOptions = ToriiListOptions()) async throws -> ToriiDomainListPage {
        let queryItems = try makeListQueryItems(options: options)
        let request = try makeRequest(path: "/v1/domains", queryItems: queryItems)
        let data = try await data(for: request)
        guard !data.isEmpty else {
            return ToriiDomainListPage(items: [], total: 0)
        }
        return try decodeJSON(ToriiDomainListPage.self, from: data)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func iterateDomains(options: ToriiListOptions = ToriiListOptions(),
                               pageSize: Int? = nil,
                               maxItems: Int? = nil) -> AsyncThrowingStream<ToriiDomainRecord, Swift.Error> {
        iterateList(options: options, pageSize: pageSize, maxItems: maxItems) { opts in
            try await self.listDomains(options: opts)
        }
    }

    public func listSubscriptionPlans(params: ToriiSubscriptionPlanListParams? = nil) async throws -> ToriiSubscriptionPlanListResponse {
        let request = try makeRequest(path: "/v1/subscriptions/plans", queryItems: try params?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiSubscriptionPlanListResponse.self, from: data)
    }

    public func createSubscriptionPlan(_ requestBody: ToriiSubscriptionPlanCreateRequest) async throws -> ToriiSubscriptionPlanCreateResponse {
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/subscriptions/plans",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiSubscriptionPlanCreateResponse.self, from: data)
    }

    public func listSubscriptions(params: ToriiSubscriptionListParams? = nil) async throws -> ToriiSubscriptionListResponse {
        let request = try makeRequest(path: "/v1/subscriptions", queryItems: try params?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiSubscriptionListResponse.self, from: data)
    }

    public func createSubscription(_ requestBody: ToriiSubscriptionCreateRequest) async throws -> ToriiSubscriptionCreateResponse {
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/subscriptions",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiSubscriptionCreateResponse.self, from: data)
    }

    public func getSubscription(subscriptionId: String) async throws -> ToriiSubscriptionRecord? {
        let normalizedId = try ToriiRequestValidation.normalizedNonEmpty(subscriptionId,
                                                                         field: "subscriptionId")
        let encoded = encodePathComponent(normalizedId)
        let request = try makeRequest(path: "/v1/subscriptions/\(encoded)")
        let (data, response) = try await send(request)
        if response.statusCode == 404 {
            return nil
        }
        try ensureStatus(response, equals: 200)
        if data.isEmpty {
            throw ToriiClientError.emptyBody
        }
        return try decodeJSON(ToriiSubscriptionRecord.self, from: data)
    }

    public func pauseSubscription(subscriptionId: String,
                                  requestBody: ToriiSubscriptionActionRequest) async throws -> ToriiSubscriptionActionResponse {
        return try await executeSubscriptionAction(pathSuffix: "pause",
                                                   subscriptionId: subscriptionId,
                                                   requestBody: requestBody)
    }

    public func resumeSubscription(subscriptionId: String,
                                   requestBody: ToriiSubscriptionActionRequest) async throws -> ToriiSubscriptionActionResponse {
        return try await executeSubscriptionAction(pathSuffix: "resume",
                                                   subscriptionId: subscriptionId,
                                                   requestBody: requestBody)
    }

    public func cancelSubscription(subscriptionId: String,
                                   requestBody: ToriiSubscriptionActionRequest) async throws -> ToriiSubscriptionActionResponse {
        return try await executeSubscriptionAction(pathSuffix: "cancel",
                                                   subscriptionId: subscriptionId,
                                                   requestBody: requestBody)
    }

    public func keepSubscription(subscriptionId: String,
                                 requestBody: ToriiSubscriptionActionRequest) async throws -> ToriiSubscriptionActionResponse {
        return try await executeSubscriptionAction(pathSuffix: "keep",
                                                   subscriptionId: subscriptionId,
                                                   requestBody: requestBody)
    }

    public func chargeSubscriptionNow(subscriptionId: String,
                                      requestBody: ToriiSubscriptionActionRequest) async throws -> ToriiSubscriptionActionResponse {
        return try await executeSubscriptionAction(pathSuffix: "charge-now",
                                                   subscriptionId: subscriptionId,
                                                   requestBody: requestBody)
    }

    public func recordSubscriptionUsage(subscriptionId: String,
                                        requestBody: ToriiSubscriptionUsageRequest) async throws -> ToriiSubscriptionActionResponse {
        let normalizedId = try ToriiRequestValidation.normalizedNonEmpty(subscriptionId,
                                                                         field: "subscriptionId")
        let encoded = encodePathComponent(normalizedId)
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/subscriptions/\(encoded)/usage",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiSubscriptionActionResponse.self, from: data)
    }

    public func getUaidPortfolio(uaid: String,
                                 query: ToriiUaidPortfolioQuery? = nil) async throws -> ToriiUaidPortfolioResponse {
        let canonical = try canonicalizeUaidLiteral(uaid)
        let encoded = encodePathComponent(canonical)
        let request = try makeRequest(path: "/v1/accounts/\(encoded)/portfolio",
                                      queryItems: query?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiUaidPortfolioResponse.self, from: data)
    }

    public func getUaidBindings(uaid: String,
                                query: ToriiUaidBindingsQuery? = nil) async throws -> ToriiUaidBindingsResponse {
        let canonical = try canonicalizeUaidLiteral(uaid)
        let encoded = encodePathComponent(canonical)
        let request = try makeRequest(path: "/v1/space-directory/uaids/\(encoded)",
                                      queryItems: try query?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiUaidBindingsResponse.self, from: data)
    }

    public func getUaidManifests(uaid: String,
                                 query: ToriiUaidManifestQuery? = nil) async throws -> ToriiUaidManifestsResponse {
        let canonical = try canonicalizeUaidLiteral(uaid)
        let encoded = encodePathComponent(canonical)
        let request = try makeRequest(path: "/v1/space-directory/uaids/\(encoded)/manifests",
                                      queryItems: try query?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiUaidManifestsResponse.self, from: data)
    }

    public func listOfflineAllowances(params: ToriiOfflineListParams? = nil) async throws -> ToriiOfflineAllowanceList {
        let request = try makeRequest(path: "/v1/offline/allowances", queryItems: try params?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineAllowanceList.self, from: data)
    }

    public func listOfflineTransfers(params: ToriiOfflineListParams? = nil) async throws -> ToriiOfflineTransferList {
        let request = try makeRequest(path: "/v1/offline/transfers", queryItems: try params?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineTransferList.self, from: data)
    }

    public func listOfflineSummaries(params: ToriiOfflineListParams? = nil) async throws -> ToriiOfflineSummaryList {
        let request = try makeRequest(path: "/v1/offline/summaries", queryItems: try params?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineSummaryList.self, from: data)
    }

    public func listOfflineRevocations(params: ToriiOfflineRevocationListParams? = nil) async throws -> ToriiOfflineRevocationList {
        let request = try makeRequest(path: "/v1/offline/revocations", queryItems: try params?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineRevocationList.self, from: data)
    }

    public func getOfflineBundleProofStatus(params: ToriiOfflineBundleProofStatusParams) async throws -> ToriiOfflineBundleProofStatus {
        let request = try makeRequest(path: "/v1/offline/bundle/proof_status", queryItems: try params.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineBundleProofStatus.self, from: data)
    }

    public func listOfflineReceipts(params: ToriiOfflineReceiptListParams? = nil) async throws -> ToriiOfflineReceiptList {
        let request = try makeRequest(path: "/v1/offline/receipts", queryItems: try params?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineReceiptList.self, from: data)
    }

    public func queryOfflineReceipts(_ envelope: ToriiQueryEnvelope) async throws -> ToriiOfflineReceiptList {
        let encoder = JSONEncoder()
        let body = try encoder.encode(envelope)
        let request = try makeRequest(path: "/v1/offline/receipts/query",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineReceiptList.self, from: data)
    }

    public func queryOfflineAllowances(_ envelope: ToriiQueryEnvelope) async throws -> ToriiOfflineAllowanceList {
        let encoder = JSONEncoder()
        let body = try encoder.encode(envelope)
        let request = try makeRequest(path: "/v1/offline/allowances/query",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineAllowanceList.self, from: data)
    }

    public func queryOfflineCertificates(_ envelope: ToriiQueryEnvelope) async throws -> ToriiOfflineAllowanceList {
        let encoder = JSONEncoder()
        let body = try encoder.encode(envelope)
        let request = try makeRequest(path: "/v1/offline/certificates/query",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineAllowanceList.self, from: data)
    }

    public func queryOfflineTransfers(_ envelope: ToriiQueryEnvelope) async throws -> ToriiOfflineTransferList {
        let encoder = JSONEncoder()
        let body = try encoder.encode(envelope)
        let request = try makeRequest(path: "/v1/offline/transfers/query",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineTransferList.self, from: data)
    }

    public func queryOfflineSettlements(_ envelope: ToriiQueryEnvelope) async throws -> ToriiOfflineTransferList {
        let encoder = JSONEncoder()
        let body = try encoder.encode(envelope)
        let request = try makeRequest(path: "/v1/offline/settlements/query",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineTransferList.self, from: data)
    }

    public func queryOfflineRevocations(_ envelope: ToriiQueryEnvelope) async throws -> ToriiOfflineRevocationList {
        let encoder = JSONEncoder()
        let body = try encoder.encode(envelope)
        let request = try makeRequest(path: "/v1/offline/revocations/query",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineRevocationList.self, from: data)
    }

    public func getOfflineState() async throws -> ToriiOfflineStateResponse {
        let request = try makeRequest(path: "/v1/offline/state")
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineStateResponse.self, from: data)
    }

    public func issueOfflineCertificate(_ requestBody: ToriiOfflineCertificateIssueRequest) async throws -> ToriiOfflineCertificateIssueResponse {
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/offline/certificates/issue",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineCertificateIssueResponse.self, from: data)
    }

    public func registerOfflineAllowance(_ requestBody: ToriiOfflineAllowanceRegisterRequest) async throws -> ToriiOfflineAllowanceRegisterResponse {
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/offline/allowances",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineAllowanceRegisterResponse.self, from: data)
    }

    public func renewOfflineAllowance(certificateIdHex: String,
                                      requestBody: ToriiOfflineAllowanceRegisterRequest) async throws -> ToriiOfflineAllowanceRegisterResponse {
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let encodedId = encodePathComponent(certificateIdHex)
        let request = try makeRequest(path: "/v1/offline/allowances/\(encodedId)/renew",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineAllowanceRegisterResponse.self, from: data)
    }

    public func issueOfflineCertificateRenewal(certificateIdHex: String,
                                               requestBody: ToriiOfflineCertificateIssueRequest) async throws -> ToriiOfflineCertificateIssueResponse {
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let encodedId = encodePathComponent(certificateIdHex)
        let request = try makeRequest(path: "/v1/offline/certificates/\(encodedId)/renew/issue",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineCertificateIssueResponse.self, from: data)
    }

    public func topUpOfflineAllowance(draft: OfflineWalletCertificateDraft,
                                      authority: String,
                                      privateKey: String) async throws -> ToriiOfflineTopUpResponse {
        let issueRequest = try ToriiOfflineCertificateIssueRequest(certificate: draft)
        let issued = try await issueOfflineCertificate(issueRequest)
        let registerRequest = ToriiOfflineAllowanceRegisterRequest(authority: authority,
                                                                   privateKey: privateKey,
                                                                   certificate: issued.certificate)
        let registered = try await registerOfflineAllowance(registerRequest)
        try ensureTopUpCertificateIdsMatch(issued: issued, registered: registered)
        return ToriiOfflineTopUpResponse(certificate: issued, registration: registered)
    }

    public func topUpOfflineAllowanceRenewal(certificateIdHex: String,
                                             draft: OfflineWalletCertificateDraft,
                                             authority: String,
                                             privateKey: String) async throws -> ToriiOfflineTopUpResponse {
        let issueRequest = try ToriiOfflineCertificateIssueRequest(certificate: draft)
        let issued = try await issueOfflineCertificateRenewal(certificateIdHex: certificateIdHex,
                                                              requestBody: issueRequest)
        let renewRequest = ToriiOfflineAllowanceRegisterRequest(authority: authority,
                                                                privateKey: privateKey,
                                                                certificate: issued.certificate)
        let registered = try await renewOfflineAllowance(certificateIdHex: certificateIdHex,
                                                         requestBody: renewRequest)
        try ensureTopUpCertificateIdsMatch(issued: issued, registered: registered)
        return ToriiOfflineTopUpResponse(certificate: issued, registration: registered)
    }

    public func submitOfflineSpendReceipts(_ requestBody: ToriiOfflineSpendReceiptsSubmitRequest) async throws -> ToriiOfflineSpendReceiptsSubmitResponse {
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/offline/spend-receipts",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineSpendReceiptsSubmitResponse.self, from: data)
    }

    public func submitOfflineSettlement(_ requestBody: ToriiOfflineSettlementSubmitRequest) async throws -> ToriiOfflineSettlementSubmitResponse {
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/offline/settlements",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiOfflineSettlementSubmitResponse.self, from: data)
    }

    public func requestOfflineTransferProof(_ requestBody: ToriiOfflineTransferProofRequest) async throws -> ToriiJSONValue {
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/offline/transfers/proof",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiJSONValue.self, from: data)
    }

    public func getConnectStatus() async throws -> ToriiConnectStatusSnapshot? {
        let request = try makeRequest(path: "/v1/connect/status",
                                      headers: ["Accept": "application/json"])
        let (data, response) = try await send(request)
        if response.statusCode == 404 {
            return nil
        }
        try ensureStatus(response, equals: 200)
        if data.isEmpty {
            throw ToriiClientError.emptyBody
        }
        return try decodeJSON(ToriiConnectStatusSnapshot.self, from: data)
    }

    public func createConnectSession(sid: String,
                                     node: String? = nil) async throws -> ToriiConnectSessionResponse {
        var payload: [String: ToriiJSONValue] = [
            "sid": .string(try ToriiConnectJSON.trimmedNonEmpty(sid, field: "sid"))
        ]
        if let node = node {
            let trimmed = node.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmed.isEmpty {
                payload["node"] = .string(trimmed)
            }
        }
        let request = try makeRequest(path: "/v1/connect/session",
                                      method: .post,
                                      body: try ToriiConnectJSON.encodePayload(payload),
                                      headers: [
                                        "Content-Type": "application/json",
                                        "Accept": "application/json"
                                      ])
        let data = try await data(for: request)
        return try decodeJSON(ToriiConnectSessionResponse.self, from: data)
    }

    public func deleteConnectSession(sid: String) async throws -> Bool {
        let trimmed = try ToriiConnectJSON.trimmedNonEmpty(sid, field: "sid")
        let encoded = encodePathComponent(trimmed)
        let request = try makeRequest(path: "/v1/connect/session/\(encoded)", method: .delete)
        let (_, response) = try await send(request)
        if response.statusCode == 404 {
            return false
        }
        try ensureStatus(response, equals: 204)
        return true
    }

    public func listConnectApps(options: ToriiConnectAppListOptions = ToriiConnectAppListOptions()) async throws -> ToriiConnectAppRegistryPage {
        let request = try makeRequest(path: "/v1/connect/app/apps",
                                      queryItems: try options.queryItems(),
                                      headers: ["Accept": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiConnectAppRegistryPage.self, from: data)
    }

    public func getConnectApp(appId: String) async throws -> ToriiConnectAppRecord {
        let trimmed = try ToriiConnectJSON.trimmedNonEmpty(appId, field: "appId")
        let encoded = encodePathComponent(trimmed)
        let request = try makeRequest(path: "/v1/connect/app/apps/\(encoded)",
                                      headers: ["Accept": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiConnectAppRecord.self, from: data)
    }

    public func registerConnectApp(_ input: ToriiConnectAppUpsertInput) async throws -> ToriiConnectAppRecord? {
        let request = try makeRequest(path: "/v1/connect/app/apps",
                                      method: .post,
                                      body: try ToriiConnectJSON.encodePayload(input.payload()),
                                      headers: [
                                        "Content-Type": "application/json",
                                        "Accept": "application/json"
                                      ])
        let (data, response) = try await send(request)
        try ensureStatus(response, in: 200..<203)
        if data.isEmpty {
            return nil
        }
        return try decodeJSON(ToriiConnectAppRecord.self, from: data)
    }

    public func deleteConnectApp(appId: String) async throws -> Bool {
        let trimmed = try ToriiConnectJSON.trimmedNonEmpty(appId, field: "appId")
        let encoded = encodePathComponent(trimmed)
        let request = try makeRequest(path: "/v1/connect/app/apps/\(encoded)", method: .delete)
        let (_, response) = try await send(request)
        if response.statusCode == 404 {
            return false
        }
        switch response.statusCode {
        case 200, 202, 204:
            return true
        default:
            throw ToriiClientError.httpStatus(code: response.statusCode,
                                              message: HTTPURLResponse.localizedString(forStatusCode: response.statusCode),
                                              rejectCode: rejectCode(from: response))
        }
    }

    public func getConnectAppPolicy() async throws -> ToriiConnectAppPolicyControls {
        let request = try makeRequest(path: "/v1/connect/app/policy",
                                      headers: ["Accept": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiConnectAppPolicyControls.self, from: data)
    }

    public func updateConnectAppPolicy(_ updates: ToriiConnectAppPolicyUpdate) async throws -> ToriiConnectAppPolicyControls {
        let request = try makeRequest(path: "/v1/connect/app/policy",
                                      method: .post,
                                      body: try ToriiConnectJSON.encodePayload(updates.payload()),
                                      headers: [
                                        "Content-Type": "application/json",
                                        "Accept": "application/json"
                                      ])
        let data = try await data(for: request, acceptedStatus: 200..<203)
        return try decodeJSON(ToriiConnectAppPolicyControls.self, from: data)
    }

    public func getConnectAdmissionManifest() async throws -> ToriiConnectAdmissionManifest {
        let request = try makeRequest(path: "/v1/connect/app/manifest",
                                      headers: ["Accept": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiConnectAdmissionManifest.self, from: data)
    }

    public func setConnectAdmissionManifest(_ manifest: ToriiConnectAdmissionManifestInput) async throws -> ToriiConnectAdmissionManifest {
        let request = try makeRequest(path: "/v1/connect/app/manifest",
                                      method: .put,
                                      body: try ToriiConnectJSON.encodePayload(manifest.payload()),
                                      headers: [
                                        "Content-Type": "application/json",
                                        "Accept": "application/json"
                                      ])
        let data = try await data(for: request, acceptedStatus: 200..<203)
        return try decodeJSON(ToriiConnectAdmissionManifest.self, from: data)
    }

    public func uploadAttachment(data: Data, contentType: String) async throws -> ToriiAttachmentMeta {
        let request = try makeRequest(path: "/v1/zk/attachments", method: .post, body: data, headers: [
            "Content-Type": contentType
        ])
        let responseData = try await self.data(for: request)
        return try decodeJSON(ToriiAttachmentMeta.self, from: responseData)
    }

    public func listAttachments() async throws -> [ToriiAttachmentMeta] {
        let request = try makeRequest(path: "/v1/zk/attachments")
        let data = try await data(for: request)
        return try decodeJSON([ToriiAttachmentMeta].self, from: data)
    }

    public func getAttachment(id: String) async throws -> (Data, String?) {
        let normalizedId = try ToriiRequestValidation.normalizedNonEmpty(id, field: "id")
        let encodedId = encodePathComponent(normalizedId)
        let request = try makeRequest(path: "/v1/zk/attachments/\(encodedId)")
        let (data, response) = try await send(request)
        try ensureStatus(response, in: 200..<300)
        let contentType = response.value(forHTTPHeaderField: "Content-Type")
        return (data, contentType)
    }

    public func deleteAttachment(id: String) async throws {
        let normalizedId = try ToriiRequestValidation.normalizedNonEmpty(id, field: "id")
        let encodedId = encodePathComponent(normalizedId)
        let request = try makeRequest(path: "/v1/zk/attachments/\(encodedId)", method: .delete)
        let (_, response) = try await send(request)
        try ensureStatus(response, equals: 204)
    }

    // MARK: Data Availability (Async)

    public func getDaManifestBundle(storageTicketHex: String,
                                    blockHashHex: String? = nil) async throws -> ToriiDaManifestBundle {
        let normalized = try ToriiClient.normalizeStorageTicketHex(storageTicketHex)
        var queryItems: [URLQueryItem] = []
        if let blockHashHex, !blockHashHex.isEmpty {
            let normalizedBlock = try ToriiClient.normalizeHex32(blockHashHex, field: "block_hash")
            queryItems.append(URLQueryItem(name: "block_hash", value: normalizedBlock))
        }
        let request = try makeRequest(
            path: "/v1/da/manifests/\(normalized)",
            queryItems: queryItems.isEmpty ? nil : queryItems,
            headers: ["Accept": "application/json"]
        )
        let data = try await data(for: request)
        return try decodeJSON(ToriiDaManifestBundle.self, from: data)
    }

    public func getDaManifestBundle(storageTicketHex: String,
                                    blockHashHex: String? = nil,
                                    outputDir: URL,
                                    label: String? = nil,
                                    fileManager: FileManager = .default) async throws -> (ToriiDaManifestBundle, ToriiDaManifestPersistedPaths) {
        let bundle = try await getDaManifestBundle(storageTicketHex: storageTicketHex, blockHashHex: blockHashHex)
        let paths = try ToriiClient.persistDaManifestBundle(bundle,
                                                            outputDir: outputDir,
                                                            label: label ?? bundle.storageTicketHex,
                                                            fileManager: fileManager)
        return (bundle, paths)
    }

    public func fetchDaPayloadViaGateway(
        storageTicketHex: String? = nil,
        manifestBundle: ToriiDaManifestBundle? = nil,
        chunkerHandle: String? = nil,
        providers: [SorafsGatewayProvider],
        options: SorafsGatewayFetchOptions? = nil,
        proofSummaryOptions: ToriiDaProofSummaryOptions? = nil,
        orchestrator: SorafsGatewayFetching = SorafsOrchestratorClient(),
        proofSummaryGenerator: DaProofSummaryGenerating = NativeDaProofSummaryGenerator.shared,
        cancellationHandler: (@Sendable () -> Void)? = nil
    ) async throws -> ToriiDaGatewayFetchResult {
        guard !providers.isEmpty else {
            throw ToriiClientError.invalidPayload("at least one gateway provider must be supplied")
        }
        let bundle = try await resolveDaManifestBundle(storageTicketHex: storageTicketHex, manifestBundle: manifestBundle)
        let handle = try ToriiClient.resolveChunkerHandle(preferred: chunkerHandle, manifest: bundle)
        let chunkPlanJSON = try bundle.chunkPlanJSONString()
        let mergedOptions: SorafsGatewayFetchOptions
        if var existing = options {
            existing.chunkerHandle = handle
            mergedOptions = existing
        } else {
            mergedOptions = SorafsGatewayFetchOptions(chunkerHandle: handle)
        }
        let result = try await orchestrator.fetchGatewayPayload(
            plan: bundle.chunkPlan,
            providers: providers,
            options: mergedOptions,
            cancellationHandler: cancellationHandler
        )
        let proofSummary: ToriiDaProofSummary?
        if let summaryOptions = proofSummaryOptions {
            if summaryOptions.sampleCount < 0 {
                throw ToriiClientError.invalidPayload("proofSummaryOptions.sampleCount must be non-negative")
            }
            let normalized = ToriiDaProofSummaryOptions(
                sampleCount: summaryOptions.sampleCount,
                sampleSeed: summaryOptions.sampleSeed,
                leafIndexes: summaryOptions.leafIndexes
            )
            proofSummary = try proofSummaryGenerator.makeProofSummary(
                manifest: bundle.manifestBytes,
                payload: result.payload,
                options: normalized
            )
        } else {
            proofSummary = nil
        }
        return ToriiDaGatewayFetchResult(
            manifest: bundle,
            manifestIdHex: bundle.blobHashHex,
            chunkerHandle: handle,
            chunkPlanJSON: chunkPlanJSON,
            gatewayResult: result,
            proofSummary: proofSummary
        )
    }

    public func proveDaAvailabilityToDirectory(
        storageTicketHex: String? = nil,
        manifestBundle: ToriiDaManifestBundle? = nil,
        providers: [SorafsGatewayProvider],
        outputDir: URL,
        chunkerHandle: String? = nil,
        options: SorafsGatewayFetchOptions? = nil,
        proofSummaryOptions: ToriiDaProofSummaryOptions? = nil,
        orchestrator: SorafsGatewayFetching = SorafsOrchestratorClient(),
        proofSummaryGenerator: DaProofSummaryGenerating = NativeDaProofSummaryGenerator.shared,
        fileManager: FileManager = .default
    ) async throws -> (ToriiDaGatewayFetchResult, ToriiDaAvailabilityPersistedPaths) {
        guard !providers.isEmpty else {
            throw ToriiClientError.invalidPayload("at least one gateway provider must be supplied")
        }
        let bundle: ToriiDaManifestBundle
        if let manifestBundle {
            bundle = manifestBundle
        } else {
            guard let ticket = storageTicketHex else {
                throw ToriiClientError.invalidPayload("storageTicketHex is required when manifestBundle is not supplied")
            }
            bundle = try await getDaManifestBundle(storageTicketHex: ticket)
        }
        let manifestPaths = try ToriiClient.persistDaManifestBundle(bundle,
                                                                    outputDir: outputDir,
                                                                    label: bundle.storageTicketHex,
                                                                    fileManager: fileManager)
        let fetchOptions = options ?? SorafsGatewayFetchOptions()
        let fetchResult = try await fetchDaPayloadViaGateway(
            storageTicketHex: storageTicketHex,
            manifestBundle: bundle,
            chunkerHandle: chunkerHandle,
            providers: providers,
            options: fetchOptions,
            proofSummaryOptions: proofSummaryOptions ?? ToriiDaProofSummaryOptions(),
            orchestrator: orchestrator,
            proofSummaryGenerator: proofSummaryGenerator
        )

        try fileManager.createDirectory(at: outputDir, withIntermediateDirectories: true, attributes: nil)
        let payloadURL = outputDir.appendingPathComponent("payload_\(manifestPaths.label).car")
        try fetchResult.gatewayResult.payload.write(to: payloadURL, options: .atomic)

        let scoreboardURL = outputDir.appendingPathComponent("scoreboard.json")
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let scoreboard = fetchResult.gatewayResult.report.scoreboard ?? []
        let scoreboardData = try encoder.encode(scoreboard)
        try scoreboardData.write(to: scoreboardURL, options: .atomic)

        let proofSummaryURL = outputDir.appendingPathComponent("proof_summary_\(manifestPaths.label).json")
        let summary = try fetchResult.proofSummary ?? proofSummaryGenerator.makeProofSummary(
            manifest: bundle.manifestBytes,
            payload: fetchResult.gatewayResult.payload,
            options: proofSummaryOptions ?? ToriiDaProofSummaryOptions()
        )
        _ = try DaProofSummaryArtifactEmitter.emit(
            summary: summary,
            manifestBytes: bundle.manifestBytes,
            payloadBytes: fetchResult.gatewayResult.payload,
            proofOptions: proofSummaryOptions,
            manifestPath: manifestPaths.manifestURL.path,
            payloadPath: payloadURL.path,
            outputURL: proofSummaryURL,
            generator: proofSummaryGenerator,
            fileManager: fileManager
        )

        let paths = ToriiDaAvailabilityPersistedPaths(manifest: manifestPaths,
                                                      payloadURL: payloadURL,
                                                      proofSummaryURL: proofSummaryURL,
                                                      scoreboardURL: scoreboardURL)
        return (fetchResult, paths)
    }

    public func submitDaBlob(_ submission: ToriiDaBlobSubmission,
                             artifactDirectory: URL? = nil,
                             noSubmit: Bool = false,
                             fileManager: FileManager = .default) async throws -> ToriiDaIngestSubmitResult {
        let builder = ToriiDaIngestRequestBuilder(submission: submission, allowUnsigned: noSubmit)
        let (body, artifacts) = try builder.makeRequestBody()
        if let artifactDirectory {
            try ToriiClient.persistDaRequestArtifacts(body: body,
                                                      directory: artifactDirectory,
                                                      fileManager: fileManager)
        }
        if noSubmit {
            return ToriiDaIngestSubmitResult(status: "prepared",
                                             duplicate: false,
                                             receipt: nil,
                                             artifacts: artifacts,
                                             pdpCommitmentHeaderBase64: nil)
        }
        let request = try makeRequest(
            path: "/v1/da/ingest",
            method: .post,
            body: body,
            headers: [
                "Content-Type": "application/json",
                "Accept": "application/json"
            ]
        )
        let (data, response) = try await send(request)
        try ensureStatus(response, equals: 202)
        let payload = try decodeJSON(ToriiDaIngestSubmitPayload.self, from: data)
        let headerValue = response.value(forHTTPHeaderField: ToriiPdpCommitmentHeader)
        if let artifactDirectory {
            try ToriiClient.persistDaReceiptArtifacts(responseBody: data,
                                                      pdpHeader: headerValue,
                                                      directory: artifactDirectory,
                                                      fileManager: fileManager)
        }
        return ToriiDaIngestSubmitResult(
            status: payload.status,
            duplicate: payload.duplicate,
            receipt: payload.receipt,
            artifacts: artifacts,
            pdpCommitmentHeaderBase64: headerValue
        )
    }

    // MARK: Governance (Async)

    public func submitGovernanceDeployContractProposal(_ requestBody: ToriiGovernanceDeployContractProposalRequest) async throws -> ToriiGovernanceProposalResponse {
        try await postGovernanceJSON(path: "/v1/gov/proposals/deploy-contract",
                                     body: requestBody,
                                     responseType: ToriiGovernanceProposalResponse.self)
    }

    public func submitGovernancePlainBallot(_ requestBody: ToriiGovernancePlainBallotRequest) async throws -> ToriiGovernanceBallotResponse {
        try await postGovernanceJSON(path: "/v1/gov/ballots/plain",
                                     body: requestBody,
                                     responseType: ToriiGovernanceBallotResponse.self)
    }

    public func submitGovernanceZkBallot(_ requestBody: ToriiGovernanceZkBallotRequest) async throws -> ToriiGovernanceBallotResponse {
        try await postGovernanceJSON(path: "/v1/gov/ballots/zk",
                                     body: requestBody,
                                     responseType: ToriiGovernanceBallotResponse.self)
    }

    public func finalizeGovernanceReferendum(_ requestBody: ToriiGovernanceFinalizeRequest) async throws -> ToriiGovernanceFinalizeResponse {
        try await postGovernanceJSON(path: "/v1/gov/finalize",
                                     body: requestBody,
                                     responseType: ToriiGovernanceFinalizeResponse.self)
    }

    public func enactGovernanceProposal(_ requestBody: ToriiGovernanceEnactRequest) async throws -> ToriiGovernanceEnactResponse {
        try await postGovernanceJSON(path: "/v1/gov/enact",
                                     body: requestBody,
                                     responseType: ToriiGovernanceEnactResponse.self)
    }

    public func getGovernanceProposal(idHex: String) async throws -> ToriiGovernanceProposalGetResponse {
        let normalized = try ToriiRequestValidation.normalized32ByteHex(idHex, field: "idHex")
        let encoded = encodePathComponent(normalized)
        return try await getGovernanceJSON(path: "/v1/gov/proposals/\(encoded)",
                                           responseType: ToriiGovernanceProposalGetResponse.self)
    }

    public func getGovernanceLocks(referendumId: String) async throws -> ToriiGovernanceLocksResponse {
        try await getGovernanceJSON(path: "/v1/gov/locks/\(referendumId)",
                                    responseType: ToriiGovernanceLocksResponse.self)
    }

    public func getGovernanceReferendum(id: String) async throws -> ToriiGovernanceReferendumResponse {
        try await getGovernanceJSON(path: "/v1/gov/referenda/\(id)",
                                    responseType: ToriiGovernanceReferendumResponse.self)
    }

    public func getGovernanceTally(id: String) async throws -> ToriiGovernanceTallyResponse {
        try await getGovernanceJSON(path: "/v1/gov/tally/\(id)",
                                    responseType: ToriiGovernanceTallyResponse.self)
    }

    public func getGovernanceUnlockStats(height: UInt64? = nil,
                                         referendumId: String? = nil) async throws -> ToriiGovernanceUnlockStatsResponse {
        var items: [URLQueryItem] = []
        if let height {
            items.append(URLQueryItem(name: "height", value: String(height)))
        }
        if let referendumId {
            items.append(URLQueryItem(name: "referendum_id", value: referendumId))
        }
        let queryItems = items.isEmpty ? nil : items
        return try await getGovernanceJSON(path: "/v1/gov/unlocks/stats",
                                           queryItems: queryItems,
                                           responseType: ToriiGovernanceUnlockStatsResponse.self)
    }

    public func listProverReports(filter: ToriiProverReportsFilter? = nil) async throws -> [ToriiProverReport] {
        let request = try makeRequest(path: "/v1/zk/prover/reports", queryItems: filter?.queryItems())
        let data = try await data(for: request)
        return try decodeJSON([ToriiProverReport].self, from: data)
    }

    public func getProverReport(id: String) async throws -> ToriiProverReport {
        let normalizedId = try ToriiRequestValidation.normalizedNonEmpty(id, field: "id")
        let encodedId = encodePathComponent(normalizedId)
        let request = try makeRequest(path: "/v1/zk/prover/reports/\(encodedId)")
        let data = try await data(for: request)
        return try decodeJSON(ToriiProverReport.self, from: data)
    }

    public func deleteProverReport(id: String) async throws {
        let normalizedId = try ToriiRequestValidation.normalizedNonEmpty(id, field: "id")
        let encodedId = encodePathComponent(normalizedId)
        let request = try makeRequest(path: "/v1/zk/prover/reports/\(encodedId)", method: .delete)
        let (_, response) = try await send(request)
        try ensureStatus(response, equals: 204)
    }

    public func countProverReports(filter: ToriiProverReportsFilter? = nil) async throws -> UInt64 {
        let request = try makeRequest(path: "/v1/zk/prover/reports/count", queryItems: filter?.queryItems())
        let data = try await data(for: request)
        if let decoded = try? decodeJSON(CountEnvelope.self, from: data) {
            return decoded.count
        }
        if let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
           let count = StrictJSONNumber.uint64(from: object["count"]) {
            return count
        }
        throw ToriiClientError.invalidPayload("Expected \"count\" field in response.")
    }

    @discardableResult
    public func deriveConfidentialKeyset(seedHex: String? = nil,
                                         seedBase64: String? = nil,
                                         completion: @escaping (Result<ToriiConfidentialKeysetResponse, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.deriveConfidentialKeyset(seedHex: seedHex, seedBase64: seedBase64)
        }
    }

    public func deriveConfidentialKeyset(seedHex: String? = nil,
                                         seedBase64: String? = nil) async throws -> ToriiConfidentialKeysetResponse {
        let trimmedHex = seedHex?.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedBase64 = seedBase64?.trimmingCharacters(in: .whitespacesAndNewlines)
        let requestBody = ToriiConfidentialKeysetRequest(
            seedHex: (trimmedHex?.isEmpty ?? true) ? nil : trimmedHex,
            seedBase64: (trimmedBase64?.isEmpty ?? true) ? nil : trimmedBase64
        )
        if requestBody.seedHex == nil && requestBody.seedBase64 == nil {
            throw ToriiClientError.invalidPayload("Provide either seedHex or seedBase64.")
        }

        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/confidential/derive-keyset",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiConfidentialKeysetResponse.self, from: data)
    }

    @discardableResult
    public func getConfidentialAssetPolicy(assetDefinitionId: String,
                                           completion: @escaping (Result<ToriiConfidentialAssetPolicy, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getConfidentialAssetPolicy(assetDefinitionId: assetDefinitionId)
        }
    }

    public func getConfidentialAssetPolicy(assetDefinitionId: String) async throws -> ToriiConfidentialAssetPolicy {
        let trimmed = assetDefinitionId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("assetDefinitionId must be a non-empty string.")
        }
        let encoded = encodePathComponent(trimmed)
        let path = "/v1/confidential/assets/\(encoded)/transitions"
        let request = try makeRequest(path: path)
        let data = try await data(for: request)
        return try decodeJSON(ToriiConfidentialAssetPolicy.self, from: data)
    }

    public func registerContractCode(_ requestBody: ToriiRegisterContractCodeRequest) async throws {
        let request = try makeRequest(path: "/v1/contracts/code",
                                      method: .post,
                                      queryItems: nil,
                                      body: try JSONEncoder().encode(requestBody),
                                      headers: ["Content-Type": "application/json"])
        let (_, response) = try await send(request)
        try ensureStatus(response, in: 200..<300)
    }

    public func fetchContractManifest(codeHashHex: String) async throws -> ToriiContractManifestRecord {
        let normalized = try ToriiRequestValidation.normalized32ByteHex(codeHashHex, field: "codeHashHex")
        let encoded = encodePathComponent(normalized)
        let request = try makeRequest(path: "/v1/contracts/code/\(encoded)")
        let data = try await data(for: request)
        return try decodeJSON(ToriiContractManifestRecord.self, from: data)
    }

    public func deployContract(_ requestBody: ToriiDeployContractRequest) async throws -> ToriiDeployContractResponse {
        let request = try makeRequest(path: "/v1/contracts/deploy",
                                      method: .post,
                                      queryItems: nil,
                                      body: try JSONEncoder().encode(requestBody),
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiDeployContractResponse.self, from: data)
    }

    public func deployContractInstance(_ requestBody: ToriiDeployContractInstanceRequest) async throws -> ToriiDeployContractInstanceResponse {
        let request = try makeRequest(path: "/v1/contracts/instance",
                                      method: .post,
                                      queryItems: nil,
                                      body: try JSONEncoder().encode(requestBody),
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiDeployContractInstanceResponse.self, from: data)
    }

    public func activateContractInstance(_ requestBody: ToriiActivateContractInstanceRequest) async throws -> ToriiActivateContractInstanceResponse {
        let request = try makeRequest(path: "/v1/contracts/instance/activate",
                                      method: .post,
                                      queryItems: nil,
                                      body: try JSONEncoder().encode(requestBody),
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiActivateContractInstanceResponse.self, from: data)
    }

    public func fetchContractCodeBytes(codeHashHex: String) async throws -> ToriiContractCodeBytes {
        let normalized = try ToriiRequestValidation.normalized32ByteHex(codeHashHex, field: "codeHashHex")
        let encoded = encodePathComponent(normalized)
        let request = try makeRequest(path: "/v1/contracts/code-bytes/\(encoded)")
        let data = try await data(for: request)
        return try decodeJSON(ToriiContractCodeBytes.self, from: data)
    }

    public func getHealth() async throws -> String {
        let request = try makeRequest(path: "/v1/health",
                                      headers: ["Accept": "text/plain"])
        let (data, response) = try await send(request)
        try ensureStatus(response, equals: 200)
        return try decodeUTF8String(from: data, context: "health")
    }

    public func getMetrics(asText: Bool = false) async throws -> ToriiMetricsResponse {
        var headers: [String: String] = [:]
        if asText {
            headers["Accept"] = "text/plain"
        }
        let request = try makeRequest(path: "/v1/metrics", headers: headers)
        let (data, response) = try await send(request)
        try ensureStatus(response, equals: 200)
        if asText {
            return .text(try decodeUTF8String(from: data, context: "metrics (text)"))
        }
        if let contentType = response.value(forHTTPHeaderField: "Content-Type")?.lowercased(),
           contentType.contains("application/json") {
            let json = try decodeJSON(ToriiJSONValue.self, from: data)
            return .json(json)
        }
        if let json = try? decodeJSON(ToriiJSONValue.self, from: data) {
            return .json(json)
        }
        return .text(try decodeUTF8String(from: data, context: "metrics"))
    }

    @discardableResult
    public func getConfiguration(completion: @escaping (Result<ToriiConfigurationSnapshot, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getConfiguration()
        }
    }

    public func getConfiguration() async throws -> ToriiConfigurationSnapshot {
        let request = try makeRequest(path: "/v1/configuration")
        let data = try await data(for: request)
        return try decodeJSON(ToriiConfigurationSnapshot.self, from: data)
    }

    @discardableResult
    public func getConfidentialGasSchedule(completion: @escaping (Result<ToriiConfidentialGasSchedule?, Swift.Error>) -> Void) -> Task<Void, Never> {
        runTask(completion) {
            try await self.getConfidentialGasSchedule()
        }
    }

    public func getConfidentialGasSchedule() async throws -> ToriiConfidentialGasSchedule? {
        let snapshot = try await getConfiguration()
        return snapshot.confidentialGas
    }

    public func getNodeCapabilities() async throws -> ToriiNodeCapabilities {
        let request = try makeRequest(path: "/v1/node/capabilities")
        let data = try await data(for: request)
        return try decodeJSON(ToriiNodeCapabilities.self, from: data)
    }

    public func getRuntimeMetrics() async throws -> ToriiRuntimeMetrics {
        let request = try makeRequest(path: "/v1/runtime/metrics")
        let data = try await data(for: request)
        return try decodeJSON(ToriiRuntimeMetrics.self, from: data)
    }

    public func getRuntimeAbiActive() async throws -> ToriiRuntimeAbiActive {
        let request = try makeRequest(path: "/v1/runtime/abi/active")
        let data = try await data(for: request)
        return try decodeJSON(ToriiRuntimeAbiActive.self, from: data)
    }

    public func getRuntimeAbiHash() async throws -> ToriiRuntimeAbiHash {
        let request = try makeRequest(path: "/v1/runtime/abi/hash")
        let data = try await data(for: request)
        return try decodeJSON(ToriiRuntimeAbiHash.self, from: data)
    }

    public func listRuntimeUpgrades() async throws -> [ToriiRuntimeUpgradeListItem] {
        let request = try makeRequest(path: "/v1/runtime/upgrades")
        let data = try await data(for: request)
        let response = try decodeJSON(ToriiRuntimeUpgradesListResponse.self, from: data)
        return response.items
    }

    public func proposeRuntimeUpgrade(manifest: ToriiRuntimeUpgradeManifest) async throws -> ToriiRuntimeUpgradeActionResponse {
        let envelope = ToriiRuntimeUpgradeManifestEnvelope(manifest: manifest)
        let encoder = JSONEncoder()
        let body = try encoder.encode(envelope)
        let request = try makeRequest(path: "/v1/runtime/upgrades/propose",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let (data, response) = try await send(request)
        try ensureStatus(response, equals: 200)
        return try decodeJSON(ToriiRuntimeUpgradeActionResponse.self, from: data)
    }

    public func activateRuntimeUpgrade(idHex: String) async throws -> ToriiRuntimeUpgradeActionResponse {
        let normalized = try ToriiRequestValidation.normalized32ByteHex(idHex, field: "idHex")
        let encoded = encodePathComponent(normalized)
        let request = try makeRequest(path: "/v1/runtime/upgrades/activate/\(encoded)",
                                      method: .post,
                                      body: Data(),
                                      headers: ["Content-Type": "application/json"])
        let (data, response) = try await send(request)
        try ensureStatus(response, equals: 200)
        return try decodeJSON(ToriiRuntimeUpgradeActionResponse.self, from: data)
    }

    public func cancelRuntimeUpgrade(idHex: String) async throws -> ToriiRuntimeUpgradeActionResponse {
        let normalized = try ToriiRequestValidation.normalized32ByteHex(idHex, field: "idHex")
        let encoded = encodePathComponent(normalized)
        let request = try makeRequest(path: "/v1/runtime/upgrades/cancel/\(encoded)",
                                      method: .post,
                                      body: Data(),
                                      headers: ["Content-Type": "application/json"])
        let (data, response) = try await send(request)
        try ensureStatus(response, equals: 200)
        return try decodeJSON(ToriiRuntimeUpgradeActionResponse.self, from: data)
    }

    public func getVerifyingKey(backend: String, name: String) async throws -> ToriiVerifyingKeyDetail {
        let pathBackend = encodePathComponent(backend)
        let pathName = encodePathComponent(name)
        let request = try makeRequest(path: "/v1/zk/vk/\(pathBackend)/\(pathName)")
        let data = try await data(for: request)
        return try decodeJSON(ToriiVerifyingKeyDetail.self, from: data)
    }

    public func listVerifyingKeys(query: ToriiVerifyingKeyListQuery? = nil) async throws -> [ToriiVerifyingKeyListItem] {
        let request = try makeRequest(path: "/v1/zk/vk",
                                      queryItems: query?.queryItems())
        let data = try await data(for: request)
        let decoder = JSONDecoder()
        do {
            return try decoder.decode([ToriiVerifyingKeyListItem].self, from: data)
        } catch let arrayError {
            do {
                let response = try decoder.decode(ToriiVerifyingKeyListResponse.self, from: data)
                return response.items
            } catch {
                throw ToriiClientError.decoding(arrayError)
            }
        }
    }

    public func registerVerifyingKey(_ requestBody: ToriiVerifyingKeyRegisterRequest) async throws {
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/zk/vk/register",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let (_, response) = try await send(request)
        try ensureStatus(response, equals: 202)
    }

    public func updateVerifyingKey(_ requestBody: ToriiVerifyingKeyUpdateRequest) async throws {
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/zk/vk/update",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let (_, response) = try await send(request)
        try ensureStatus(response, equals: 202)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func streamVerifyingKeyEvents(filter: ToriiVerifyingKeyEventFilter = ToriiVerifyingKeyEventFilter(),
                                         lastEventId: String? = nil) -> AsyncThrowingStream<ToriiVerifyingKeyEventMessage, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let queryItems = try filter.queryItems()
                    var headers = ["Accept": "text/event-stream"]
                    if let lastEventId {
                        headers["Last-Event-ID"] = lastEventId
                    }
                    let request = try makeRequest(path: "/v1/events/sse",
                                                  queryItems: queryItems,
                                                  headers: headers)
                    let (bytes, response) = try await session.bytes(for: request)
                    guard let httpResponse = response as? HTTPURLResponse else {
                        throw ToriiClientError.invalidResponse
                    }
                    try ensureStatus(httpResponse, equals: 200)

                    var buffer: [String] = []
                    var lineAccumulator = Data()
                    var byteIterator = bytes.makeAsyncIterator()
                    while let byte = try await byteIterator.next() {
                        if Task.isCancelled {
                            break
                        }
                        if byte == UInt8(ascii: "\n") {
                            let rawLine = String(decoding: lineAccumulator, as: UTF8.self)
                            if rawLine.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                                if let message = try parseVerifyingKeyEvent(from: buffer) {
                                    continuation.yield(message)
                                }
                                buffer.removeAll(keepingCapacity: true)
                            } else {
                                buffer.append(rawLine)
                            }
                            lineAccumulator.removeAll(keepingCapacity: true)
                        } else {
                            lineAccumulator.append(byte)
                        }
                    }
                    if !lineAccumulator.isEmpty {
                        let rawLine = String(decoding: lineAccumulator, as: UTF8.self)
                        buffer.append(rawLine)
                    }

                    if let message = try parseVerifyingKeyEvent(from: buffer) {
                        continuation.yield(message)
                    }

                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    if Task.isCancelled {
                        continuation.finish()
                    } else {
                        continuation.finish(throwing: error)
                    }
                }
            }

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func streamTriggerEvents(filter: ToriiTriggerEventFilter = ToriiTriggerEventFilter(),
                                    lastEventId: String? = nil) -> AsyncThrowingStream<ToriiTriggerEventMessage, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let queryItems = try filter.queryItems()
                    var headers = ["Accept": "text/event-stream"]
                    if let lastEventId {
                        headers["Last-Event-ID"] = lastEventId
                    }
                    let request = try makeRequest(path: "/v1/events/sse",
                                                  queryItems: queryItems,
                                                  headers: headers)
                    let (bytes, response) = try await session.bytes(for: request)
                    guard let httpResponse = response as? HTTPURLResponse else {
                        throw ToriiClientError.invalidResponse
                    }
                    try ensureStatus(httpResponse, equals: 200)

                    var buffer: [String] = []
                    var lineAccumulator = Data()
                    var iterator = bytes.makeAsyncIterator()
                    while let byte = try await iterator.next() {
                        if Task.isCancelled {
                            break
                        }
                        if byte == UInt8(ascii: "\n") {
                            let rawLine = String(decoding: lineAccumulator, as: UTF8.self)
                            if rawLine.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                                if let message = try parseTriggerEvent(from: buffer) {
                                    continuation.yield(message)
                                }
                                buffer.removeAll(keepingCapacity: true)
                            } else {
                                buffer.append(rawLine)
                            }
                            lineAccumulator.removeAll(keepingCapacity: true)
                        } else {
                            lineAccumulator.append(byte)
                        }
                    }
                    if !lineAccumulator.isEmpty {
                        let rawLine = String(decoding: lineAccumulator, as: UTF8.self)
                        buffer.append(rawLine)
                    }

                    if let message = try parseTriggerEvent(from: buffer) {
                        continuation.yield(message)
                    }

                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    if Task.isCancelled {
                        continuation.finish()
                    } else {
                        continuation.finish(throwing: error)
                    }
                }
            }

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func streamProofEvents(filter: ToriiProofEventFilter = ToriiProofEventFilter(),
                                  lastEventId: String? = nil) -> AsyncThrowingStream<ToriiProofEventMessage, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let queryItems = try filter.queryItems()
                    var headers = ["Accept": "text/event-stream"]
                    if let lastEventId {
                        headers["Last-Event-ID"] = lastEventId
                    }
                    let request = try makeRequest(path: "/v1/events/sse",
                                                  queryItems: queryItems,
                                                  headers: headers)
                    let (bytes, response) = try await session.bytes(for: request)
                    guard let httpResponse = response as? HTTPURLResponse else {
                        throw ToriiClientError.invalidResponse
                    }
                    try ensureStatus(httpResponse, equals: 200)

                    var buffer: [String] = []
                    var lineAccumulator = Data()
                    var iterator = bytes.makeAsyncIterator()
                    while let byte = try await iterator.next() {
                        if Task.isCancelled {
                            break
                        }
                        if byte == UInt8(ascii: "\n") {
                            let rawLine = String(decoding: lineAccumulator, as: UTF8.self)
                            if rawLine.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                                if let message = try parseProofEvent(from: buffer) {
                                    continuation.yield(message)
                                }
                                buffer.removeAll(keepingCapacity: true)
                            } else {
                                buffer.append(rawLine)
                            }
                            lineAccumulator.removeAll(keepingCapacity: true)
                        } else {
                            lineAccumulator.append(byte)
                        }
                    }
                    if !lineAccumulator.isEmpty {
                        let rawLine = String(decoding: lineAccumulator, as: UTF8.self)
                        buffer.append(rawLine)
                    }

                    if let message = try parseProofEvent(from: buffer) {
                        continuation.yield(message)
                    }

                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    if Task.isCancelled {
                        continuation.finish()
                    } else {
                        continuation.finish(throwing: error)
                    }
                }
            }

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    /// Stream explorer transaction summaries via SSE as blocks commit.
    @available(iOS 15.0, macOS 12.0, *)
    public func streamExplorerTransactions(lastEventId: String? = nil) -> AsyncThrowingStream<ToriiExplorerTransactionItem, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    var headers = ["Accept": "text/event-stream"]
                    if let lastEventId {
                        headers["Last-Event-ID"] = lastEventId
                    }
                    let request = try makeRequest(path: "/v1/explorer/transactions/stream",
                                                  headers: headers)
                    let (bytes, response) = try await session.bytes(for: request)
                    guard let httpResponse = response as? HTTPURLResponse else {
                        throw ToriiClientError.invalidResponse
                    }
                    try ensureStatus(httpResponse, equals: 200)

                    var buffer: [String] = []
                    var lineAccumulator = Data()
                    var iterator = bytes.makeAsyncIterator()
                    while let byte = try await iterator.next() {
                        if Task.isCancelled {
                            break
                        }
                        if byte == UInt8(ascii: "\n") {
                            let rawLine = String(decoding: lineAccumulator, as: UTF8.self)
                            if rawLine.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                                if let item = try parseExplorerTransactionEvent(from: buffer) {
                                    continuation.yield(item)
                                }
                                buffer.removeAll(keepingCapacity: true)
                            } else {
                                buffer.append(rawLine)
                            }
                            lineAccumulator.removeAll(keepingCapacity: true)
                        } else {
                            lineAccumulator.append(byte)
                        }
                    }
                    if !lineAccumulator.isEmpty {
                        let rawLine = String(decoding: lineAccumulator, as: UTF8.self)
                        buffer.append(rawLine)
                    }

                    if let item = try parseExplorerTransactionEvent(from: buffer) {
                        continuation.yield(item)
                    }

                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    if Task.isCancelled {
                        continuation.finish()
                    } else {
                        continuation.finish(throwing: error)
                    }
                }
            }

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    /// Stream explorer instruction payloads via SSE as blocks commit.
    @available(iOS 15.0, macOS 12.0, *)
    public func streamExplorerInstructions(lastEventId: String? = nil) -> AsyncThrowingStream<ToriiExplorerInstructionItem, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    var headers = ["Accept": "text/event-stream"]
                    if let lastEventId {
                        headers["Last-Event-ID"] = lastEventId
                    }
                    let request = try makeRequest(path: "/v1/explorer/instructions/stream",
                                                  headers: headers)
                    let (bytes, response) = try await session.bytes(for: request)
                    guard let httpResponse = response as? HTTPURLResponse else {
                        throw ToriiClientError.invalidResponse
                    }
                    try ensureStatus(httpResponse, equals: 200)

                    var buffer: [String] = []
                    var lineAccumulator = Data()
                    var iterator = bytes.makeAsyncIterator()
                    while let byte = try await iterator.next() {
                        if Task.isCancelled {
                            break
                        }
                        if byte == UInt8(ascii: "\n") {
                            let rawLine = String(decoding: lineAccumulator, as: UTF8.self)
                            if rawLine.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                                if let item = try parseExplorerInstructionEvent(from: buffer) {
                                    continuation.yield(item)
                                }
                                buffer.removeAll(keepingCapacity: true)
                            } else {
                                buffer.append(rawLine)
                            }
                            lineAccumulator.removeAll(keepingCapacity: true)
                        } else {
                            lineAccumulator.append(byte)
                        }
                    }
                    if !lineAccumulator.isEmpty {
                        let rawLine = String(decoding: lineAccumulator, as: UTF8.self)
                        buffer.append(rawLine)
                    }

                    if let item = try parseExplorerInstructionEvent(from: buffer) {
                        continuation.yield(item)
                    }

                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    if Task.isCancelled {
                        continuation.finish()
                    } else {
                        continuation.finish(throwing: error)
                    }
                }
            }

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    /// Stream transfer records derived from the explorer instruction SSE feed.
    @available(iOS 15.0, macOS 12.0, *)
    public func streamExplorerTransfers(lastEventId: String? = nil,
                                        matchingAccount accountId: String? = nil,
                                        assetDefinitionId: String? = nil,
                                        assetId: String? = nil) -> AsyncThrowingStream<ToriiExplorerTransferRecord, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let accountFilter = accountId?.trimmingCharacters(in: .whitespacesAndNewlines)
                    let assetFilter = assetDefinitionId?.trimmingCharacters(in: .whitespacesAndNewlines)
                    let assetIdFilter = assetId?.trimmingCharacters(in: .whitespacesAndNewlines)
                    for try await item in streamExplorerInstructions(lastEventId: lastEventId) {
                        if Task.isCancelled {
                            break
                        }
                        guard let details = item.transferDetails() else {
                            continue
                        }
                        if let accountFilter, !accountFilter.isEmpty, !details.involvesAccount(accountFilter) {
                            continue
                        }
                        if let assetFilter, !assetFilter.isEmpty, !details.involvesAssetDefinition(assetFilter) {
                            continue
                        }
                        if let assetIdFilter, !assetIdFilter.isEmpty, !details.involvesAssetId(assetIdFilter) {
                            continue
                        }
                        continuation.yield(ToriiExplorerTransferRecord(instruction: item, details: details))
                    }
                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    if Task.isCancelled {
                        continuation.finish()
                    } else {
                        continuation.finish(throwing: error)
                    }
                }
            }
            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    /// Stream transfer summaries derived from the explorer instruction SSE feed.
    @available(iOS 15.0, macOS 12.0, *)
    public func streamExplorerTransferSummaries(lastEventId: String? = nil,
                                                matchingAccount accountId: String? = nil,
                                                assetDefinitionId: String? = nil,
                                                assetId: String? = nil,
                                                relativeTo relativeAccountId: String? = nil) -> AsyncThrowingStream<ToriiExplorerTransferSummary, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let accountFilter = accountId?.trimmingCharacters(in: .whitespacesAndNewlines)
                    let assetFilter = assetDefinitionId?.trimmingCharacters(in: .whitespacesAndNewlines)
                    let assetIdFilter = assetId?.trimmingCharacters(in: .whitespacesAndNewlines)
                    let relativeCandidate = relativeAccountId ?? accountId
                    let relativeTrimmed = relativeCandidate?.trimmingCharacters(in: .whitespacesAndNewlines)
                    let relativeAccount = (relativeTrimmed?.isEmpty ?? true) ? nil : relativeTrimmed
                    for try await item in streamExplorerInstructions(lastEventId: lastEventId) {
                        if Task.isCancelled {
                            break
                        }
                        guard let details = item.transferDetails() else {
                            continue
                        }
                        if let accountFilter, !accountFilter.isEmpty, !details.involvesAccount(accountFilter) {
                            continue
                        }
                        if let assetFilter, !assetFilter.isEmpty, !details.involvesAssetDefinition(assetFilter) {
                            continue
                        }
                        if let assetIdFilter, !assetIdFilter.isEmpty, !details.involvesAssetId(assetIdFilter) {
                            continue
                        }
                        let record = ToriiExplorerTransferRecord(instruction: item, details: details)
                        for summary in record.summaries(relativeTo: relativeAccount) {
                            continuation.yield(summary)
                        }
                    }
                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    if Task.isCancelled {
                        continuation.finish()
                    } else {
                        continuation.finish(throwing: error)
                    }
                }
            }
            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    /// Stream existing account transfer history first, then keep streaming live transfer updates.
    @available(iOS 15.0, macOS 12.0, *)
    public func streamAccountTransferHistory(accountId: String,
                                             page: UInt64? = nil,
                                             perPage: UInt64? = nil,
                                             addressFormat: AccountAddressFormat? = nil,
                                             assetDefinitionId: String? = nil,
                                             assetId: String? = nil,
                                             lastEventId: String? = nil,
                                             maxItems: UInt64? = nil,
                                             dedupeLimit: Int = 10_000) -> AsyncThrowingStream<ToriiExplorerTransferSummary, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    if let maxItems, maxItems == 0 {
                        continuation.finish()
                        return
                    }
                    let normalizedAccount = try ToriiRequestValidation.normalizedNonEmpty(accountId,
                                                                                           field: "accountId")
                    var remaining = maxItems
                    var deduper = TransferSummaryDeduper(limit: dedupeLimit)

                    func emit(_ summary: ToriiExplorerTransferSummary) -> Bool {
                        guard deduper.shouldYield(summary) else {
                            return true
                        }
                        continuation.yield(summary)
                        if let remainingValue = remaining {
                            if remainingValue <= 1 {
                                remaining = 0
                                return false
                            }
                            remaining = remainingValue - 1
                        }
                        return true
                    }

                    for try await summary in iterateAccountTransferHistory(accountId: normalizedAccount,
                                                                           page: page,
                                                                           perPage: perPage,
                                                                           addressFormat: addressFormat,
                                                                           assetDefinitionId: assetDefinitionId,
                                                                           assetId: assetId,
                                                                           maxItems: remaining) {
                        if Task.isCancelled {
                            break
                        }
                        if !emit(summary) {
                            continuation.finish()
                            return
                        }
                    }

                    if remaining == 0 {
                        continuation.finish()
                        return
                    }

                    for try await summary in streamExplorerTransferSummaries(lastEventId: lastEventId,
                                                                             matchingAccount: normalizedAccount,
                                                                             assetDefinitionId: assetDefinitionId,
                                                                             assetId: assetId,
                                                                             relativeTo: normalizedAccount) {
                        if Task.isCancelled {
                            break
                        }
                        if !emit(summary) {
                            continuation.finish()
                            return
                        }
                    }

                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    if Task.isCancelled {
                        continuation.finish()
                    } else {
                        continuation.finish(throwing: error)
                    }
                }
            }
            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    private func ensureDataModelCompatibility() async throws {
        let expected = ToriiNodeCapabilities.expectedDataModelVersion
        let cached = dataModelQueue.sync { dataModelCompatibility }
        switch cached {
        case .compatible:
            return
        case let .incompatible(expected, actual):
            throw ToriiClientError.incompatibleDataModel(expected: expected, actual: actual)
        case .unknown:
            break
        }

        let capabilities = try await getNodeCapabilities()
        let actual = capabilities.dataModelVersion
        guard actual == expected else {
            dataModelQueue.sync {
                dataModelCompatibility = .incompatible(expected: expected, actual: actual)
            }
            throw ToriiClientError.incompatibleDataModel(expected: expected, actual: actual)
        }
        dataModelQueue.sync {
            dataModelCompatibility = .compatible(version: expected)
        }
    }

    private func ensureTopUpCertificateIdsMatch(issued: ToriiOfflineCertificateIssueResponse,
                                                registered: ToriiOfflineAllowanceRegisterResponse) throws {
        let issuedId = issued.certificateIdHex.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let registeredId = registered.certificateIdHex.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if issuedId != registeredId {
            throw ToriiClientError.invalidPayload(
                "Offline top-up certificate mismatch (issued \(issued.certificateIdHex), registered \(registered.certificateIdHex))."
            )
        }
    }

    public func submitTransaction(data: Data,
                                  mode: PipelineEndpointMode,
                                  idempotencyKey: String? = nil) async throws -> ToriiSubmitTransactionResponse? {
        try await ensureDataModelCompatibility()
        let paths = pipelineEndpoints(for: mode)
        var headers: [String: String] = [
            "Content-Type": "application/x-norito",
            "Accept": "application/x-norito, application/json"
        ]
        if let key = idempotencyKey, !key.isEmpty {
            headers["Idempotency-Key"] = key
        }
        let request = try makeRequest(path: paths.submit,
                                      method: .post,
                                      body: data,
                                      headers: headers)
        let (responseData, response) = try await send(request)
        try ensureStatus(response, in: 200..<300)
        guard !responseData.isEmpty else { return nil }
        let contentType = response.value(forHTTPHeaderField: "Content-Type")?.lowercased() ?? ""
        if contentType.contains("application/x-norito") {
            guard let jsonString = NoritoNativeBridge.shared.decodeTransactionReceipt(responseData),
                  let jsonData = jsonString.data(using: .utf8) else {
                throw ToriiClientError.invalidPayload("failed to decode Norito receipt")
            }
            return try decodeJSON(ToriiSubmitTransactionResponse.self, from: jsonData)
        }
        return try decodeJSON(ToriiSubmitTransactionResponse.self, from: responseData)
    }

    public func getTransactionStatus(hashHex: String,
                                     mode: PipelineEndpointMode = .pipeline) async throws -> ToriiPipelineTransactionStatus? {
        let paths = pipelineEndpoints(for: mode)
        let request = try makeRequest(
            path: paths.status,
            queryItems: [URLQueryItem(name: "hash", value: hashHex)],
            headers: ["Accept": "application/json"]
        )
        let (data, response) = try await send(request)
        if response.statusCode == 404 { return nil }
        try ensureStatus(response, in: 200..<300)
        guard !data.isEmpty else { return nil }
        return try decodeJSON(ToriiPipelineTransactionStatus.self, from: data)
    }

    public func getPipelineRecovery(height: UInt64) async throws -> ToriiPipelineRecovery? {
        let request = try makeRequest(path: "/v1/pipeline/recovery/\(height)")
        let (data, response) = try await send(request)
        if response.statusCode == 404 { return nil }
        try ensureStatus(response, in: 200..<300)
        guard !data.isEmpty else { return nil }
        return try decodeJSON(ToriiPipelineRecovery.self, from: data)
    }

    public func getTimeNow() async throws -> ToriiTimeSnapshot {
        let request = try makeRequest(path: "/v1/time/now")
        let (data, response) = try await send(request)
        try ensureStatus(response, in: 200..<300)
        return try decodeJSON(ToriiTimeSnapshot.self, from: data)
    }

    public func getTimeStatus() async throws -> ToriiTimeStatusSnapshot {
        let request = try makeRequest(path: "/v1/time/status")
        let (data, response) = try await send(request)
        try ensureStatus(response, in: 200..<300)
        return try decodeJSON(ToriiTimeStatusSnapshot.self, from: data)
    }

    public func getSumeragiStatus() async throws -> ToriiSumeragiStatusSnapshot {
        let request = try makeRequest(path: "/v1/sumeragi/status",
                                      headers: ["Accept": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiSumeragiStatusSnapshot.self, from: data)
    }

    public func getSumeragiCommitQc(blockHashHex: String) async throws -> ToriiSumeragiCommitQcRecord {
        let normalized = try ToriiClient.normalizeHex32(blockHashHex, field: "block_hash")
        let request = try makeRequest(path: "/v1/sumeragi/commit_qc/\(normalized)",
                                      headers: ["Accept": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiSumeragiCommitQcRecord.self, from: data)
    }

    public func getStatusSnapshot() async throws -> ToriiStatusSnapshot {
        let sequence = statusState.reserveSequence()
        let request = try makeRequest(path: "/v1/status",
                                      headers: ["Accept": "application/json"])
        let data = try await data(for: request)
        let payload = try decodeJSON(ToriiStatusPayload.self, from: data)
        let metrics = statusState.record(payload, sequence: sequence)
        return ToriiStatusSnapshot(timestamp: Date(), status: payload, metrics: metrics)
    }

    private func executeSubscriptionAction(pathSuffix: String,
                                           subscriptionId: String,
                                           requestBody: ToriiSubscriptionActionRequest) async throws -> ToriiSubscriptionActionResponse {
        let normalizedId = try ToriiRequestValidation.normalizedNonEmpty(subscriptionId,
                                                                         field: "subscriptionId")
        let encoded = encodePathComponent(normalizedId)
        let encoder = JSONEncoder()
        let body = try encoder.encode(requestBody)
        let request = try makeRequest(path: "/v1/subscriptions/\(encoded)/\(pathSuffix)",
                                      method: .post,
                                      body: body,
                                      headers: ["Content-Type": "application/json"])
        let data = try await data(for: request)
        return try decodeJSON(ToriiSubscriptionActionResponse.self, from: data)
    }

    private func makeListQueryItems(options: ToriiListOptions,
                                    overrideLimit: Int? = nil,
                                    overrideOffset: Int? = nil) throws -> [URLQueryItem]? {
        var items: [URLQueryItem] = []
        if let limitValue = overrideLimit ?? options.limit {
            let normalized = try normalizedPositive(limitValue, context: "limit")
            items.append(URLQueryItem(name: "limit", value: String(normalized)))
        }
        if let offsetValue = overrideOffset ?? options.offset {
            let normalized = try normalizedOffset(offsetValue, context: "offset")
            items.append(URLQueryItem(name: "offset", value: String(normalized)))
        }
        if let filter = options.filter,
           let encodedFilter = try filter.encodedValue(),
           !encodedFilter.isEmpty {
            items.append(URLQueryItem(name: "filter", value: encodedFilter))
        }
        if let sort = options.sort,
           let encodedSort = sort.encodedValue(),
           !encodedSort.isEmpty {
            items.append(URLQueryItem(name: "sort", value: encodedSort))
        }
        return items.isEmpty ? nil : items
    }

    private func normalizedPositive(_ value: Int, context: String) throws -> Int {
        guard value > 0 else {
            throw ToriiClientError.invalidPayload("\(context) must be positive")
        }
        return value
    }

    private func normalizedOffset(_ value: Int, context: String) throws -> Int {
        guard value >= 0 else {
            throw ToriiClientError.invalidPayload("\(context) must be non-negative")
        }
        return value
    }

    private func resolvedPageSize(requested: Int?, remaining: Int?) throws -> Int {
        let candidate = try normalizedPositive(requested ?? ToriiClient.defaultListPageSize, context: "pageSize")
        if let remaining, remaining > 0 {
            return min(candidate, remaining)
        }
        return candidate
    }

    @available(iOS 15.0, macOS 12.0, *)
    private func iterateList<Page: ToriiListPageProtocol>(
        options: ToriiListOptions,
        pageSize: Int?,
        maxItems: Int?,
        fetcher: @Sendable @escaping (ToriiListOptions) async throws -> Page
    ) -> AsyncThrowingStream<Page.Item, Swift.Error> {
        AsyncThrowingStream { continuation in
            if let maxItems, maxItems <= 0 {
                continuation.finish(throwing: ToriiClientError.invalidPayload("maxItems must be positive"))
                return
            }
            let task = Task {
                do {
                    var remaining = maxItems
                    var offset = try self.normalizedOffset(options.offset ?? 0, context: "offset")
                    var baseOptions = options
                    baseOptions.limit = nil
                    baseOptions.offset = nil
                    var produced = 0
                    while true {
                        var requestOptions = baseOptions
                        let preferred = pageSize ?? options.limit
                        let limit = try self.resolvedPageSize(requested: preferred, remaining: remaining)
                        requestOptions.limit = limit
                        requestOptions.offset = offset
                        let page = try await fetcher(requestOptions)
                        guard !page.items.isEmpty else { break }
                        for item in page.items {
                            continuation.yield(item)
                            produced += 1
                            if var outstanding = remaining {
                                outstanding -= 1
                                remaining = outstanding
                                if outstanding <= 0 {
                                    continuation.finish()
                                    return
                                }
                            }
                        }
                        if page.items.count < limit || produced >= page.total {
                            break
                        }
                        offset += page.items.count
                    }
                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }

    // MARK: - Async helpers

    private func pipelineEndpoints(for _: PipelineEndpointMode) -> (submit: String, status: String) {
        // Iroha 3 uses /transaction for Norito binary submission
        // and /v1/pipeline/transactions/status for JSON status queries
        ("/transaction", "/v1/pipeline/transactions/status")
    }

    private enum HTTPMethod: String {
        case get = "GET"
        case post = "POST"
        case put = "PUT"
        case delete = "DELETE"
    }

    private struct CountEnvelope: Decodable {
        let count: UInt64

        private enum CodingKeys: String, CodingKey { case count }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let value = try container.decode(ToriiJSONValue.self, forKey: .count)
            guard let normalized = value.normalizedUInt64 else {
                throw DecodingError.dataCorruptedError(forKey: .count,
                                                       in: container,
                                                       debugDescription: "count must be an unsigned integer")
            }
            count = normalized
        }
    }

    private func resolveDaManifestBundle(storageTicketHex: String?,
                                         manifestBundle: ToriiDaManifestBundle?,
                                         blockHashHex: String? = nil) async throws -> ToriiDaManifestBundle {
        if let manifestBundle {
            return manifestBundle
        }
        guard let ticket = storageTicketHex else {
            throw ToriiClientError.invalidPayload("storageTicketHex is required when manifestBundle is not provided")
        }
        return try await getDaManifestBundle(storageTicketHex: ticket, blockHashHex: blockHashHex)
    }

    private static func resolveChunkerHandle(preferred: String?,
                                             manifest: ToriiDaManifestBundle) throws -> String {
        if let provided = preferred?.trimmingCharacters(in: .whitespacesAndNewlines),
           !provided.isEmpty {
            return provided
        }
        if let inferred = manifest.inferChunkerHandle() {
            return inferred
        }
        throw ToriiClientError.invalidPayload("chunkerHandle is required when the manifest omits chunking metadata")
    }

    private static func normalizeStorageTicketHex(_ ticket: String) throws -> String {
        var body = ticket.trimmingCharacters(in: .whitespacesAndNewlines)
        if body.hasPrefix("0x") || body.hasPrefix("0X") {
            body = String(body.dropFirst(2))
        }
        guard body.count == 64, Data(hexString: body) != nil else {
            throw ToriiClientError.invalidPayload("storageTicketHex must be a 32-byte hex string")
        }
        return body.lowercased()
    }

    private static func normalizeHex32(_ hex: String, field: String) throws -> String {
        var body = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if body.hasPrefix("0x") || body.hasPrefix("0X") {
            body = String(body.dropFirst(2))
        }
        guard body.count == 64, Data(hexString: body) != nil else {
            throw ToriiClientError.invalidPayload("\(field) must be a 32-byte hex string")
        }
        return body.lowercased()
    }

    private func makeRequest(path: String,
                             method: HTTPMethod = .get,
                             queryItems: [URLQueryItem]? = nil,
                             body: Data? = nil,
                             headers: [String: String] = [:]) throws -> URLRequest {
        // Remove leading slash to make path relative (required for URL(string:relativeTo:))
        let relativePath = path.hasPrefix("/") ? String(path.dropFirst()) : path

        // URL(string:relativeTo:) preserves percent-encoding without double-encoding.
        // baseURL is normalized to directory URL in init, so relative resolution works correctly.
        guard let url = URL(string: relativePath, relativeTo: baseURL),
              var urlComponents = URLComponents(url: url, resolvingAgainstBaseURL: true) else {
            throw ToriiClientError.invalidURL(path)
        }

        if let items = queryItems, !items.isEmpty {
            urlComponents.queryItems = items
        }
        guard let finalURL = urlComponents.url else {
            throw ToriiClientError.invalidURL(path)
        }

        var request = URLRequest(url: finalURL)
        request.httpMethod = method.rawValue
        request.httpBody = body
        headers.forEach { key, value in
            request.setValue(value, forHTTPHeaderField: key)
        }
        return request
    }

    private func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        do {
            let (data, response) = try await session.data(for: request, delegate: nil)
            guard let http = response as? HTTPURLResponse else {
                throw ToriiClientError.invalidResponse
            }
            return (data, http)
        } catch is CancellationError {
            throw CancellationError()
        } catch {
            throw ToriiClientError.transport(error)
        }
    }

    private func rejectCode(from response: HTTPURLResponse) -> String? {
        guard let raw = response.value(forHTTPHeaderField: "x-iroha-reject-code")?.trimmingCharacters(in: .whitespacesAndNewlines),
              !raw.isEmpty
        else {
            return nil
        }
        return raw
    }

    private func ensureStatus(_ response: HTTPURLResponse, in range: Range<Int>) throws {
        guard range.contains(response.statusCode) else {
            throw ToriiClientError.httpStatus(code: response.statusCode,
                                              message: HTTPURLResponse.localizedString(forStatusCode: response.statusCode),
                                              rejectCode: rejectCode(from: response))
        }
    }

    private func ensureStatus(_ response: HTTPURLResponse, equals code: Int) throws {
        guard response.statusCode == code else {
            throw ToriiClientError.httpStatus(code: response.statusCode,
                                              message: HTTPURLResponse.localizedString(forStatusCode: response.statusCode),
                                              rejectCode: rejectCode(from: response))
        }
    }

    private func data(for request: URLRequest,
                      acceptedStatus: Range<Int> = 200..<300,
                      allowEmptyBody: Bool = false) async throws -> Data {
        let (data, response) = try await send(request)
        try ensureStatus(response, in: acceptedStatus)
        if data.isEmpty && !allowEmptyBody {
            throw ToriiClientError.emptyBody
        }
        return data
    }

    private func decodeUTF8String(from data: Data, context: String) throws -> String {
        guard let text = String(data: data, encoding: .utf8) else {
            throw ToriiClientError.invalidPayload("\(context) response is not valid UTF-8.")
        }
        return text
    }

    private func decodeJSON<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        do {
            return try JSONDecoder().decode(T.self, from: data)
        } catch {
            throw ToriiClientError.decoding(error)
        }
    }

    private func encodePathComponent(_ value: String) -> String {
        var allowed = CharacterSet.urlPathAllowed
        allowed.remove(charactersIn: "/:@?&=#%")
        return value.addingPercentEncoding(withAllowedCharacters: allowed) ?? value
    }

    private func encodeAccountIdPath(_ accountId: String) throws -> String {
        let trimmed = accountId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("accountId must be a non-empty string.")
        }
        return encodePathComponent(trimmed)
    }

    fileprivate static func explorerAddressFormatQueryValue(_ format: AccountAddressFormat,
                                                            context: String) throws -> String {
        switch format {
        case .ih58:
            return "ih58"
        case .compressed:
            return "compressed"
        case .canonicalHex:
            throw ToriiClientError.invalidPayload(
                "addressFormat=canonicalHex is not supported for \(context); use .ih58 or .compressed."
            )
        }
    }

    private func canonicalizeUaidLiteral(_ literal: String) throws -> String {
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("uaid must be a non-empty string.")
        }
        let prefix = trimmed.prefix(5).lowercased()
        let rawHex: String
        if prefix == "uaid:" {
            rawHex = String(trimmed.dropFirst(5)).trimmingCharacters(in: .whitespacesAndNewlines)
        } else {
            rawHex = trimmed
        }
        guard rawHex.count == 64 else {
            throw ToriiClientError.invalidPayload("uaid must contain exactly 64 hex characters.")
        }
        let hexSet = CharacterSet(charactersIn: "0123456789abcdefABCDEF")
        let allHex = rawHex.unicodeScalars.allSatisfy { hexSet.contains($0) }
        guard allHex else {
            throw ToriiClientError.invalidPayload("uaid must contain only hex characters.")
        }
        let lastChar = rawHex.lowercased().suffix(1)
        guard ["1", "3", "5", "7", "9", "b", "d", "f"].contains(String(lastChar)) else {
            throw ToriiClientError.invalidPayload("uaid must have least significant bit set to 1.")
        }
        return "uaid:\(rawHex.lowercased())"
    }

    fileprivate static func normalizeAddressFormatQueryValue(_ raw: String?,
                                                             context: String) throws -> String? {
        guard let raw else { return nil }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            return nil
        }
        let lowered = trimmed.lowercased()
        switch lowered {
        case "ih58":
            return "ih58"
        case "compressed":
            return "compressed"
        default:
            throw ToriiClientError.invalidPayload(
                "\(context) must be one of ih58 or compressed."
            )
        }
    }

    private func postGovernanceJSON<Request: Encodable, Response: Decodable>(path: String,
                                                                             body: Request,
                                                                             responseType: Response.Type) async throws -> Response {
        let encoder = JSONEncoder()
        let payload = try encoder.encode(body)
        let request = try makeRequest(path: path,
                                      method: .post,
                                      body: payload,
                                      headers: ["Content-Type": "application/json"])
        let (data, response) = try await send(request)
        try ensureStatus(response, in: 200..<300)
        return try decodeJSON(Response.self, from: data)
    }

    private func getGovernanceJSON<Response: Decodable>(path: String,
                                                        queryItems: [URLQueryItem]? = nil,
                                                        responseType: Response.Type) async throws -> Response {
        let request = try makeRequest(path: path, queryItems: queryItems)
        let (data, response) = try await send(request)
        try ensureStatus(response, in: 200..<300)
        return try decodeJSON(Response.self, from: data)
    }

    private func decodeAssetBalances(from data: Data) throws -> [ToriiAssetBalance] {
        let decoder = JSONDecoder()
        if let balances = try? decoder.decode([ToriiAssetBalance].self, from: data) {
            return balances
        }
        if let envelope = try? decoder.decode(ToriiAssetBalanceEnvelope.self, from: data) {
            return envelope.items
        }
        return try decoder.decode([ToriiAssetBalance].self, from: data)
    }

    private struct ToriiAssetBalanceEnvelope: Decodable {
        let items: [ToriiAssetBalance]
        let total: UInt64
    }

    private func parseVerifyingKeyEvent(from lines: [String]) throws -> ToriiVerifyingKeyEventMessage? {
        guard let parsed = try parseServerSentEvent(from: lines) else {
            return nil
        }
        guard let payloadString = parsed.data else {
            return nil
        }
        guard let payloadData = payloadString.data(using: .utf8) else {
            throw ToriiClientError.invalidPayload("SSE payload is not valid UTF-8.")
        }
        let envelope = try decodeJSON(ToriiVerifyingKeyEventEnvelope.self, from: payloadData)
        return ToriiVerifyingKeyEventMessage(event: envelope.event,
                                             eventName: parsed.eventName,
                                             eventId: parsed.id,
                                             retryHintMilliseconds: parsed.retry,
                                             rawEvent: parsed.raw)
    }

    private struct ToriiSseParsedEvent {
        let eventName: String?
        let data: String?
        let id: String?
        let retry: Int?
        let raw: String
    }

    private func parseServerSentEvent(from lines: [String]) throws -> ToriiSseParsedEvent? {
        guard !lines.isEmpty else { return nil }
        var dataChunks: [String] = []
        var eventName: String?
        var identifier: String?
        var retry: Int?

        for entry in lines {
            let trimmedEntry = entry.trimmingCharacters(in: .whitespaces)
            if trimmedEntry.hasPrefix(":") {
                continue
            }
            let components = entry.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
            let rawField = components.first.map(String.init) ?? ""
            let field = rawField.trimmingCharacters(in: .whitespaces)
            let value = components.count > 1 ? components[1].trimmingCharacters(in: .whitespaces) : ""
            switch field {
            case "data":
                dataChunks.append(value)
            case "event":
                eventName = value.isEmpty ? nil : value
            case "id":
                identifier = value.isEmpty ? nil : value
            case "retry":
                if let parsed = Int(value) {
                    retry = parsed
                }
            default:
                continue
            }
        }

        if dataChunks.isEmpty && eventName == nil && identifier == nil && retry == nil {
            return nil
        }

        let dataString = dataChunks.isEmpty ? nil : dataChunks.joined(separator: "\n")
        return ToriiSseParsedEvent(eventName: eventName,
                                   data: dataString,
                                   id: identifier,
                                   retry: retry,
                                   raw: lines.joined(separator: "\n"))
    }

    private func parseExplorerTransactionEvent(from lines: [String]) throws -> ToriiExplorerTransactionItem? {
        guard let parsed = try parseServerSentEvent(from: lines) else {
            return nil
        }
        guard let payloadString = parsed.data else {
            return nil
        }
        guard let payloadData = payloadString.data(using: .utf8) else {
            throw ToriiClientError.invalidPayload("SSE payload is not valid UTF-8.")
        }
        return try decodeJSON(ToriiExplorerTransactionItem.self, from: payloadData)
    }

    private func parseExplorerInstructionEvent(from lines: [String]) throws -> ToriiExplorerInstructionItem? {
        guard let parsed = try parseServerSentEvent(from: lines) else {
            return nil
        }
        guard let payloadString = parsed.data else {
            return nil
        }
        guard let payloadData = payloadString.data(using: .utf8) else {
            throw ToriiClientError.invalidPayload("SSE payload is not valid UTF-8.")
        }
        return try decodeJSON(ToriiExplorerInstructionItem.self, from: payloadData)
    }

    private struct ToriiVerifyingKeyEventEnvelope: Decodable {
        let event: ToriiVerifyingKeyEvent

        enum CodingKeys: String, CodingKey {
            case verifyingKey = "VerifyingKey"
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let wrapper = try container.decode(ToriiVerifyingKeyEventWrapper.self, forKey: .verifyingKey)
            event = wrapper.toEvent()
        }
    }

    private enum ToriiVerifyingKeyEventWrapper: Decodable {
        case registered(ToriiVerifyingKeyEventRecordPayload)
        case updated(ToriiVerifyingKeyEventRecordPayload)

        enum CodingKeys: String, CodingKey {
            case registered = "Registered"
            case updated = "Updated"
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let keys = container.allKeys
            guard keys.count == 1 else {
                throw DecodingError.dataCorrupted(
                    DecodingError.Context(
                        codingPath: decoder.codingPath,
                        debugDescription: "Expected exactly one verifying key event payload."
                    )
                )
            }
            switch keys[0] {
            case .registered:
                self = .registered(try container.decode(ToriiVerifyingKeyEventRecordPayload.self, forKey: .registered))
            case .updated:
                self = .updated(try container.decode(ToriiVerifyingKeyEventRecordPayload.self, forKey: .updated))
            }
        }

        func toEvent() -> ToriiVerifyingKeyEvent {
            switch self {
            case .registered(let payload):
                return .registered(id: payload.id, record: payload.record)
            case .updated(let payload):
                return .updated(id: payload.id, record: payload.record)
            }
        }
    }

    private struct ToriiVerifyingKeyEventRecordPayload: Decodable {
        let id: ToriiVerifyingKeyId
        let record: ToriiVerifyingKeyRecord
    }

    private func parseTriggerEvent(from lines: [String]) throws -> ToriiTriggerEventMessage? {
        guard let parsed = try parseServerSentEvent(from: lines) else {
            return nil
        }
        guard let payloadString = parsed.data else {
            return nil
        }
        guard let payloadData = payloadString.data(using: .utf8) else {
            throw ToriiClientError.invalidPayload("SSE payload is not valid UTF-8.")
        }
        let envelope = try decodeJSON(ToriiTriggerEventEnvelope.self, from: payloadData)
        return ToriiTriggerEventMessage(event: envelope.event,
                                         eventName: parsed.eventName,
                                         eventId: parsed.id,
                                         retryHintMilliseconds: parsed.retry,
                                         rawEvent: parsed.raw)
    }

    private struct ToriiTriggerEventEnvelope: Decodable {
        let event: ToriiTriggerEvent

        enum CodingKeys: String, CodingKey {
            case trigger = "Trigger"
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let wrapper = try container.decode(ToriiTriggerEventWrapper.self, forKey: .trigger)
            event = wrapper.toEvent()
        }
    }

    private enum ToriiTriggerEventWrapper: Decodable {
        case created(String)
        case deleted(String)
        case extended(ToriiTriggerNumberOfExecutionsChanged)
        case shortened(ToriiTriggerNumberOfExecutionsChanged)
        case metadataInserted(ToriiTriggerMetadataChanged)
        case metadataRemoved(ToriiTriggerMetadataChanged)

        enum CodingKeys: String, CodingKey {
            case created = "Created"
            case deleted = "Deleted"
            case extended = "Extended"
            case shortened = "Shortened"
            case metadataInserted = "MetadataInserted"
            case metadataRemoved = "MetadataRemoved"
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let keys = container.allKeys
            guard keys.count == 1 else {
                throw DecodingError.dataCorrupted(
                    DecodingError.Context(
                        codingPath: decoder.codingPath,
                        debugDescription: "Expected exactly one trigger event payload."
                    )
                )
            }
            switch keys[0] {
            case .created:
                self = .created(try container.decode(String.self, forKey: .created))
            case .deleted:
                self = .deleted(try container.decode(String.self, forKey: .deleted))
            case .extended:
                self = .extended(try container.decode(ToriiTriggerNumberOfExecutionsChanged.self, forKey: .extended))
            case .shortened:
                self = .shortened(try container.decode(ToriiTriggerNumberOfExecutionsChanged.self, forKey: .shortened))
            case .metadataInserted:
                self = .metadataInserted(try container.decode(ToriiTriggerMetadataChanged.self, forKey: .metadataInserted))
            case .metadataRemoved:
                self = .metadataRemoved(try container.decode(ToriiTriggerMetadataChanged.self, forKey: .metadataRemoved))
            }
        }

        func toEvent() -> ToriiTriggerEvent {
            switch self {
            case .created(let id):
                return .created(triggerId: id)
            case .deleted(let id):
                return .deleted(triggerId: id)
            case .extended(let payload):
                return .extended(payload)
            case .shortened(let payload):
                return .shortened(payload)
            case .metadataInserted(let payload):
                return .metadataInserted(payload)
            case .metadataRemoved(let payload):
                return .metadataRemoved(payload)
            }
        }
    }

    private func parseProofEvent(from lines: [String]) throws -> ToriiProofEventMessage? {
        guard let parsed = try parseServerSentEvent(from: lines) else {
            return nil
        }
        guard let payloadString = parsed.data else {
            return nil
        }
        guard let payloadData = payloadString.data(using: .utf8) else {
            throw ToriiClientError.invalidPayload("SSE payload is not valid UTF-8.")
        }
        let envelope = try decodeJSON(ToriiProofEventEnvelope.self, from: payloadData)
        return ToriiProofEventMessage(event: envelope.event,
                                      eventName: parsed.eventName,
                                      eventId: parsed.id,
                                      retryHintMilliseconds: parsed.retry,
                                      rawEvent: parsed.raw)
    }

    private struct ToriiProofEventEnvelope: Decodable {
        let event: ToriiProofEvent

        enum CodingKeys: String, CodingKey {
            case proof = "Proof"
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let wrapper = try container.decode(ToriiProofEventWrapper.self, forKey: .proof)
            event = wrapper.toEvent()
        }
    }

    private enum ToriiProofEventWrapper: Decodable {
        case verified(ToriiProofEventBody)
        case rejected(ToriiProofEventBody)

        enum CodingKeys: String, CodingKey {
            case verified = "Verified"
            case rejected = "Rejected"
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let keys = container.allKeys
            guard keys.count == 1 else {
                throw DecodingError.dataCorrupted(
                    DecodingError.Context(
                        codingPath: decoder.codingPath,
                        debugDescription: "Expected exactly one proof event payload."
                    )
                )
            }
            switch keys[0] {
            case .verified:
                self = .verified(try container.decode(ToriiProofEventBody.self, forKey: .verified))
            case .rejected:
                self = .rejected(try container.decode(ToriiProofEventBody.self, forKey: .rejected))
            }
        }

        func toEvent() -> ToriiProofEvent {
            switch self {
            case .verified(let payload):
                return .verified(payload)
            case .rejected(let payload):
                return .rejected(payload)
            }
        }
    }


    private func decodeTransactionEnvelope(from data: Data) throws -> ToriiTxEnvelope {
        let decoder = JSONDecoder()
        if let envelope = try? decoder.decode(ToriiTxEnvelope.self, from: data) {
            return envelope
        }
        let items = try decoder.decode([ToriiTxItem].self, from: data)
        return ToriiTxEnvelope(items: items, total: UInt64(items.count))
    }

    @discardableResult
    private func runTask<T>(_ completion: @escaping (Result<T, Swift.Error>) -> Void,
                            operation: @Sendable @escaping () async throws -> T) -> Task<Void, Never> {
        runCompletionTask(operation: operation, completion: completion)
    }
}

private struct TransferSummaryDeduper {
    let limit: Int
    private var keys: Set<String> = []
    private var order: [String] = []

    init(limit: Int) {
        self.limit = limit
    }

    mutating func shouldYield(_ summary: ToriiExplorerTransferSummary) -> Bool {
        guard limit > 0 else {
            return true
        }
        let key = Self.makeKey(summary)
        if keys.contains(key) {
            return false
        }
        keys.insert(key)
        order.append(key)
        if keys.count > limit {
            let overflow = keys.count - limit
            for _ in 0..<overflow {
                guard let oldest = order.first else {
                    break
                }
                order.removeFirst()
                keys.remove(oldest)
            }
        }
        return true
    }

    private static func makeKey(_ summary: ToriiExplorerTransferSummary) -> String {
        "\(summary.transactionHash)|\(summary.instructionIndex)|\(summary.transferIndex)|\(summary.senderAccountId)|\(summary.receiverAccountId)|\(summary.assetDefinitionId)|\(summary.amount)"
    }
}

private struct CompletionBox<Value>: @unchecked Sendable {
    let completion: (Result<Value, Swift.Error>) -> Void

    func call(_ result: Result<Value, Swift.Error>) {
        completion(result)
    }
}

private struct SendableResult<Value>: @unchecked Sendable {
    let result: Result<Value, Swift.Error>
}

@discardableResult
private func runCompletionTask<Value>(operation: @Sendable @escaping () async throws -> Value,
                                      completion: @escaping (Result<Value, Swift.Error>) -> Void) -> Task<Void, Never> {
    let completionBox = CompletionBox(completion: completion)

    return Task {
        do {
            let value = try await operation()
            guard !Task.isCancelled else { return }
            let result = SendableResult(result: .success(value))
            await MainActor.run {
                completionBox.call(result.result)
            }
        } catch {
            guard !Task.isCancelled else { return }
            let result = SendableResult<Value>(result: .failure(error))
            await MainActor.run {
                completionBox.call(result.result)
            }
        }
    }
}
