import Foundation

@testable import IrohaSwift

enum CanonicalUnsignedTransactionTestSupport {
  static func transactionPayload(
    networkId: NetworkId,
    authority: String,
    creationTimeMs: UInt64,
    executable: Data,
    timeToLiveMs: UInt64?,
    nonce: UInt32? = nil,
    feePayment: FeePaymentIntent,
    admissionIntent: TransactionAdmissionIntentV1 = .ordinary,
    metadata: [String: ToriiJSONValue] = [:]
  ) throws -> Data {
    var domain = CompactNoritoWriter()
    domain.writeUInt32LE(0)
    domain.writeField(networkId.bytes)

    var payload = CompactNoritoWriter()
    payload.writeField(domain.data)
    payload.writeField(
      try AccountAddress.parseEncoded(authority)
        .compactNoritoAccountControllerPayload()
    )
    payload.writeField(CompactNorito.encodeUInt64(creationTimeMs))
    payload.writeField(executable)
    payload.writeField(
      try CompactNorito.encodeOption(timeToLiveMs, encode: CompactNorito.encodeUInt64)
    )
    payload.writeField(
      try CompactNorito.encodeOption(nonce, encode: CompactNorito.encodeUInt32)
    )
    payload.writeField(try feePayment.compactNorito())
    payload.writeField(admissionIntent.norito)
    payload.writeField(try encodeMetadata(metadata))
    payload.writeField(Data([0]))
    return payload.data
  }

  static func genericPayload(
    authority: String,
    networkId: NetworkId = TestNetworkIds.canonical,
    creationTimeMs: UInt64 = 123,
    feePayment: FeePaymentIntent = .authority(chargeLimits: [], gasLimit: nil)
  ) throws -> Data {
    var instructions = CompactNoritoWriter()
    instructions.writeUInt64LE(0)
    var executable = CompactNoritoWriter()
    executable.writeUInt32LE(0)
    executable.writeField(instructions.data)
    return try transactionPayload(
      networkId: networkId,
      authority: authority,
      creationTimeMs: creationTimeMs,
      executable: executable.data,
      timeToLiveMs: nil,
      feePayment: feePayment
    )
  }

  static func contractPayload(
    request: ToriiContractCallRequest,
    contractAddress: String,
    codeHashHex: String,
    networkId: NetworkId,
    feePayment: FeePaymentIntent? = nil,
    admissionIntent: TransactionAdmissionIntentV1 = .ordinary,
    additionalMetadata: [String: ToriiJSONValue] = [:]
  ) throws -> Data {
    var invocation = CompactNoritoWriter()
    invocation.writeField(CompactNorito.encodeString(contractAddress))
    guard let codeHash = Data(hexString: codeHashHex), codeHash.count == 32 else {
      throw ToriiClientError.invalidPayload("test contract code hash is invalid")
    }
    invocation.writeField(codeHash)
    invocation.writeField(CompactNorito.encodeString(request.entrypoint))

    var arguments = CompactNoritoWriter()
    if request.payload != nil {
      guard let bytes = request.draftIntent?.invocation.arguments else {
        throw ToriiClientError.invalidPayload(
          "test parameterized contract request is missing draft-intent arguments"
        )
      }
      var record = CompactNoritoWriter()
      record.writeUInt64LE(UInt64(bytes.count))
      record.writeBytes(bytes)
      arguments.writeUInt8(1)
      arguments.writeField(record.data)
    } else {
      arguments.writeUInt8(0)
    }
    invocation.writeField(arguments.data)

    var executable = CompactNoritoWriter()
    executable.writeUInt32LE(1)
    executable.writeField(invocation.data)

    var metadata = request.metadata
    metadata["contract_address"] = .string(contractAddress)
    metadata["contract_code_hash"] = .string(codeHashHex)
    metadata["contract_entrypoint"] = .string(request.entrypoint)
    if let alias = request.contractAlias {
      metadata["contract_alias"] = .string(alias)
    }
    if let payload = request.payload {
      metadata["contract_payload"] = payload
    }
    metadata.merge(additionalMetadata) { _, replacement in replacement }
    return try transactionPayload(
      networkId: networkId,
      authority: request.authority,
      creationTimeMs: request.creationTimeMs!,
      executable: executable.data,
      timeToLiveMs: request.transactionTtlMs,
      feePayment: feePayment ?? request.feePayment,
      admissionIntent: admissionIntent,
      metadata: metadata
    )
  }

  static func contractArgumentRecord(for payload: ToriiJSONValue) throws -> Data {
    Data(try CanonicalNorito.jsonString(from: payload).utf8)
  }

  static func instructionExecutable(_ instructionPairs: [Data]) -> Data {
    var instructions = CompactNoritoWriter()
    instructions.writeUInt64LE(UInt64(instructionPairs.count))
    for instruction in instructionPairs {
      instructions.writeField(instruction)
    }
    var executable = CompactNoritoWriter()
    executable.writeUInt32LE(0)
    executable.writeField(instructions.data)
    return executable.data
  }

  static func assetPayload(
    request: ToriiAssetTransferRequest,
    networkId: NetworkId
  ) throws -> Data {
    var source = CompactNoritoWriter()
    source.writeField(
      try AccountAddress.parseEncoded(request.authority)
        .compactNoritoAccountControllerPayload()
    )
    guard let definition = AssetDefinitionAddress.decode(request.assetDefinitionId) else {
      throw ToriiClientError.invalidPayload("test asset definition is invalid")
    }
    var encodedDefinition = CompactNoritoWriter()
    for byte in definition {
      encodedDefinition.writeField(Data([byte]))
    }
    source.writeField(encodedDefinition.data)
    var scope = CompactNoritoWriter()
    if request.assetBalanceScope == "global" {
      scope.writeUInt32LE(0)
    } else {
      guard request.assetBalanceScope.hasPrefix("dataspace:"),
        let dataspace = UInt64(request.assetBalanceScope.dropFirst("dataspace:".count))
      else {
        throw ToriiClientError.invalidPayload("test asset scope is invalid")
      }
      scope.writeUInt32LE(1)
      var dataspacePayload = CompactNoritoWriter()
      dataspacePayload.writeUInt64LE(dataspace)
      scope.writeField(dataspacePayload.data)
    }
    source.writeField(scope.data)

    var transferBody = CompactNoritoWriter()
    transferBody.writeField(source.data)
    transferBody.writeField(try CanonicalNorito.encodeCompactQuantity(request.amount))
    transferBody.writeField(
      try AccountAddress.parseEncoded(request.destination)
        .compactNoritoAccountControllerPayload()
    )

    var transferBox = CompactNoritoWriter()
    transferBox.writeUInt32LE(2)
    transferBox.writeField(transferBody.data)
    let frame = noritoEncode(
      typeName: "iroha_data_model::isi::transfer::TransferBox",
      payload: transferBox.data,
      flags: NoritoHeader.compactLen
    )
    var instruction = CompactNoritoWriter()
    instruction.writeField(CompactNorito.encodeString("iroha.transfer"))
    instruction.writeField(CanonicalNorito.encodeBytesVec(frame))

    var instructions = CompactNoritoWriter()
    instructions.writeUInt64LE(1)
    instructions.writeField(instruction.data)
    var executable = CompactNoritoWriter()
    executable.writeUInt32LE(0)
    executable.writeField(instructions.data)

    let metadata = request.memo.map { ["memo": ToriiJSONValue.string($0)] } ?? [:]
    return try transactionPayload(
      networkId: networkId,
      authority: request.authority,
      creationTimeMs: request.creationTimeMs,
      executable: executable.data,
      timeToLiveMs: request.transactionTtlMs,
      feePayment: request.feePayment,
      metadata: metadata
    )
  }

  static func transactionHash(for payload: Data) -> Data {
    var entrypoint = CompactNoritoWriter()
    entrypoint.writeUInt32LE(0)
    entrypoint.writeField(payload)
    return IrohaHash.hash(entrypoint.data)
  }

  private static func encodeMetadata(
    _ metadata: [String: ToriiJSONValue]
  ) throws -> Data {
    var writer = CompactNoritoWriter()
    let keys = metadata.keys.sorted { Data($0.utf8).lexicographicallyPrecedes(Data($1.utf8)) }
    writer.writeUInt64LE(UInt64(keys.count))
    for key in keys {
      guard let value = metadata[key] else { continue }
      var entry = CompactNoritoWriter()
      entry.writeField(CompactNorito.encodeString(key))
      var json = CompactNoritoWriter()
      json.writeField(
        CompactNorito.encodeString(try CanonicalNorito.jsonString(from: value))
      )
      entry.writeField(json.data)
      writer.writeField(entry.data)
    }
    return writer.data
  }
}
