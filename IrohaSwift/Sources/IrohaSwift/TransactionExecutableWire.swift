import Foundation

/// How the native bridge frames one signed-transaction archive
/// (`connect_norito_decode_signed_transaction_json_v1`).
enum NativeTransactionArchive: UInt8, Sendable {
    /// The governed version-1 submission wire; current layout only.
    case versioned = 0
    /// A headerless archive whose owner names its layout.
    case bare = 1
    /// A Norito frame whose header declares the selected layout.
    case framed = 2
}

/// A refusal of the native instruction layout transcoder.
enum NativeInstructionLayoutError: Error, Equatable, CustomStringConvertible {
    /// `CONNECT_NORITO_ERR_INSTRUCTION_UNREGISTERED`.
    static let unregisteredStatus: Int32 = -516
    /// `CONNECT_NORITO_ERR_INSTRUCTION_PAYLOAD`.
    static let payloadStatus: Int32 = -517

    /// The ABI-33 bridge or its transcoder export is not loaded.
    case bridgeUnavailable
    /// The fixed layout frames no instruction envelope.
    case invalidLayout
    /// The payload is not the registered instruction's canonical payload in `layout`.
    case noncanonicalPayload(wireName: String, layout: NoritoWireLayout)
    /// Any other native status.
    case bridgeStatus(Int32)

    var description: String {
        switch self {
        case .bridgeUnavailable:
            return "dense instruction envelopes need the ABI-33 native instruction transcoder"
        case .invalidLayout:
            return "instruction envelopes are framed in the compact or dense layout"
        case let .noncanonicalPayload(wireName, layout):
            return "instruction \(wireName) payload is not its canonical \(layout) payload"
        case let .bridgeStatus(status):
            return "native instruction transcoder failed with status \(status)"
        }
    }
}

/// One `InstructionBox` pair as a transaction payload carries it.
///
/// The pair is the instruction's registry wire ID and its payload:
/// - in the current layouts the payload is one nested Norito frame (header and
///   payload);
/// - in the dense layout it is the instruction's bare dense payload, because a
///   frame embedded in a dense payload carries no nested header (`norito.md`,
///   "Dense layout", rule 6).
///
/// A registered instruction keeps one typed form, its Norito frame, in every
/// layout: the native bridge (`connect_norito_transcode_instruction_v1`)
/// decodes a dense payload into the frame of exactly the instruction its wire
/// ID registers and transcodes the frame back when a dense payload is written.
/// Only a wire ID the bridge does not register stays an uninterpreted dense
/// payload, which encodes only in the dense layout it was decoded from.
struct TransactionInstructionWire: Equatable, Sendable {
    /// Longest wire ID an interface definition can declare.
    static let maximumWireNameBytes = 192

    let wireName: String
    /// The nested frame, or the bare dense payload of an unregistered wire ID.
    let payload: Data
    /// Whether `payload` is an opaque bare dense payload rather than a frame.
    let isDense: Bool

    /// - Parameters:
    ///   - wireName: registry wire ID: 1 to 192 bytes of ASCII letters, digits
    ///     and `. _ : - @ /`.
    ///   - payload: the nested frame (current layouts) or bare payload (dense).
    ///   - layout: the layout of the transaction payload that carries the pair.
    init(wireName: String, payload: Data, layout: NoritoWireLayout) throws {
        guard Self.isWellFormedWireName(wireName) else {
            throw ExecutableBatchInputError.invalidInstructionWireName
        }
        if layout == .dense {
            if let frame = try NoritoNativeBridge.shared.transcodeInstruction(
                wireName: wireName,
                payload: payload,
                from: .dense,
                to: .compact
            ) {
                self.payload = frame
                isDense = false
            } else {
                self.payload = Data(payload)
                isDense = true
            }
        } else {
            guard noritoDecodeFrame(payload) != nil else {
                throw ExecutableBatchInputError.invalidInstructionFrame
            }
            self.payload = Data(payload)
            isDense = false
        }
        self.wireName = wireName
    }

    static func isWellFormedWireName(_ wireName: String) -> Bool {
        let bytes = wireName.utf8
        return !bytes.isEmpty
            && bytes.count <= maximumWireNameBytes
            && bytes.allSatisfy { byte in
                (0x30...0x39).contains(byte)
                    || (0x41...0x5A).contains(byte)
                    || (0x61...0x7A).contains(byte)
                    || [0x2E, 0x5F, 0x3A, 0x2D, 0x40, 0x2F].contains(byte)
            }
    }

    /// The payload this pair carries in `layout`: the frame in the current
    /// layouts, the bare dense payload in the dense layout.
    func envelopePayload(layout: NoritoWireLayout) throws -> Data {
        if isDense {
            guard layout == .dense else {
                throw ToriiClientError.invalidPayload(
                    "an opaque dense instruction payload cannot be written in another Norito layout."
                )
            }
            return payload
        }
        guard layout == .dense else { return payload }
        guard let dense = try NoritoNativeBridge.shared.transcodeInstruction(
            wireName: wireName,
            payload: payload,
            from: .compact,
            to: .dense
        ) else {
            throw ToriiClientError.invalidPayload(
                "instruction \(wireName) has no registered dense encoding."
            )
        }
        return dense
    }

    /// `[field: wire ID][field: Vec<u8> payload]` in `layout`.
    func encode(layout: NoritoWireLayout) throws -> Data {
        let envelope = try envelopePayload(layout: layout)
        var instruction = NoritoLayoutWriter(layout: layout)
        instruction.writeField(NoritoLayoutCodec.encodeString(wireName, layout: layout))
        instruction.writeField(NoritoLayoutCodec.encodeBytesVec(envelope, layout: layout))
        return instruction.data
    }

    static func decode(
        _ bytes: Data,
        layout: NoritoWireLayout,
        context: String
    ) throws -> TransactionInstructionWire {
        var pair = ToriiVerifyingKeyCompactReader(bytes, layout: layout)
        let wireName = try ToriiCanonicalTransactionDraft.decodeString(
            try pair.takeField("\(context).wire_id"),
            field: "\(context).wire_id",
            layout: layout
        )
        var encodedPayload = ToriiVerifyingKeyCompactReader(
            try pair.takeField("\(context).payload"),
            layout: layout
        )
        let count = try encodedPayload.takeCount("\(context).payload.count")
        guard count <= UInt64(Int.max) else {
            throw ToriiClientError.invalidPayload("\(context).payload exceeds the runtime bound.")
        }
        let payload = try encodedPayload.takeBytes(Int(count), field: "\(context).payload")
        guard pair.isFinished, encodedPayload.isFinished else {
            throw ToriiClientError.invalidPayload("\(context) contains trailing bytes.")
        }
        do {
            return try TransactionInstructionWire(
                wireName: wireName,
                payload: payload,
                layout: layout
            )
        } catch NativeInstructionLayoutError.bridgeUnavailable {
            throw NativeInstructionLayoutError.bridgeUnavailable
        } catch {
            throw ToriiClientError.invalidPayload(
                "\(context) is not one well-formed instruction pair."
            )
        }
    }
}

/// One ordered item of an `Executable::Batch`.
enum TransactionBatchItemWire: Equatable, Sendable {
    case instruction(TransactionInstructionWire)
    case contractCall(TransactionContractInvocation)

    func encode(layout: NoritoWireLayout) throws -> Data {
        switch self {
        case let .instruction(instruction):
            return Self.encodeInstruction(
                instructionBoxPayload: try instruction.encode(layout: layout),
                layout: layout
            )
        case let .contractCall(invocation):
            return NoritoLayoutCodec.encodeEnum(
                tag: 1,
                fields: [TransactionExecutableWire.encodeContractInvocation(invocation, layout: layout)],
                layout: layout
            )
        }
    }

    /// `ExecutableBatchItem::Instruction` over an `InstructionBox` pair that is
    /// already encoded in `layout`.
    static func encodeInstruction(
        instructionBoxPayload: Data,
        layout: NoritoWireLayout
    ) -> Data {
        NoritoLayoutCodec.encodeEnum(tag: 0, fields: [instructionBoxPayload], layout: layout)
    }

    static func decode(
        _ bytes: Data,
        layout: NoritoWireLayout,
        context: String
    ) throws -> TransactionBatchItemWire {
        var item = ToriiVerifyingKeyCompactReader(bytes, layout: layout)
        let tag = try item.takeEnumTag("\(context).kind")
        let body = try item.takeField("\(context).value")
        guard item.isFinished else {
            throw ToriiClientError.invalidPayload("\(context) contains trailing bytes.")
        }
        switch tag {
        case 0:
            return .instruction(
                try TransactionInstructionWire.decode(body, layout: layout, context: context)
            )
        case 1:
            return .contractCall(
                try TransactionExecutableWire.decodeContractInvocation(
                    body,
                    layout: layout,
                    context: context
                )
            )
        default:
            throw ToriiClientError.invalidPayload("\(context) has an unknown batch item variant.")
        }
    }
}

/// A transaction `Executable` decoded down to its instruction pairs.
///
/// The same typed value encodes in any layout whose class matches its
/// instruction payloads; the caller names the layout and it is never inferred
/// from the bytes.
enum TransactionExecutableWire: Equatable, Sendable {
    /// `Executable::Instructions` (discriminant 0).
    case instructions([TransactionInstructionWire])
    /// `Executable::ContractCall` (discriminant 1).
    case contractCall(TransactionContractInvocation)
    /// `Executable::Ivm` (discriminant 2): raw IVM bytecode.
    case ivm(Data)
    /// `Executable::Batch` (discriminant 4).
    case batch([TransactionBatchItemWire])

    /// Whether the executable needs a signature-bound gas limit.
    var requiresGasLimit: Bool {
        switch self {
        case .instructions:
            return false
        case .contractCall, .ivm:
            return true
        case let .batch(items):
            return items.contains { item in
                if case .contractCall = item { return true }
                return false
            }
        }
    }

    /// Wire IDs of every instruction pair the executable carries.
    var instructionWireNames: [String] {
        switch self {
        case let .instructions(instructions):
            return instructions.map(\.wireName)
        case .contractCall, .ivm:
            return []
        case let .batch(items):
            return items.compactMap { item in
                if case let .instruction(instruction) = item { return instruction.wireName }
                return nil
            }
        }
    }

    var isEmpty: Bool {
        switch self {
        case let .instructions(instructions):
            return instructions.isEmpty
        case .contractCall:
            return false
        case let .ivm(bytecode):
            return bytecode.isEmpty
        case let .batch(items):
            return items.isEmpty
        }
    }

    func encode(layout: NoritoWireLayout) throws -> Data {
        switch self {
        case let .instructions(instructions):
            return Self.encodeInstructions(
                instructionBoxPayloads: try instructions.map { try $0.encode(layout: layout) },
                layout: layout
            )
        case let .contractCall(invocation):
            return NoritoLayoutCodec.encodeEnum(
                tag: 1,
                fields: [Self.encodeContractInvocation(invocation, layout: layout)],
                layout: layout
            )
        case let .ivm(bytecode):
            // `IvmBytecode` is a tuple struct over `Vec<u8>`: its one field
            // keeps its own length prefix.
            var wrapper = NoritoLayoutWriter(layout: layout)
            wrapper.writeField(NoritoLayoutCodec.encodeBytesVec(bytecode, layout: layout))
            return NoritoLayoutCodec.encodeEnum(tag: 2, fields: [wrapper.data], layout: layout)
        case let .batch(items):
            return Self.encodeBatch(
                itemPayloads: try items.map { try $0.encode(layout: layout) },
                layout: layout
            )
        }
    }

    /// `Executable::Instructions` over `InstructionBox` pairs that are already
    /// encoded in `layout`.
    static func encodeInstructions(
        instructionBoxPayloads: [Data],
        layout: NoritoWireLayout
    ) -> Data {
        NoritoLayoutCodec.encodeEnum(
            tag: 0,
            fields: [NoritoLayoutCodec.encodeVec(instructionBoxPayloads, layout: layout) { $0 }],
            layout: layout
        )
    }

    /// `Executable::Batch` over `ExecutableBatchItem` values that are already
    /// encoded in `layout`.
    static func encodeBatch(itemPayloads: [Data], layout: NoritoWireLayout) -> Data {
        NoritoLayoutCodec.encodeEnum(
            tag: 4,
            fields: [NoritoLayoutCodec.encodeVec(itemPayloads, layout: layout) { $0 }],
            layout: layout
        )
    }

    /// `ContractInvocation`: address, expected code hash, entrypoint and the
    /// optional argument record. The 32-byte hash is raw in every layout.
    static func encodeContractInvocation(
        _ invocation: TransactionContractInvocation,
        layout: NoritoWireLayout
    ) -> Data {
        var call = NoritoLayoutWriter(layout: layout)
        call.writeField(NoritoLayoutCodec.encodeString(invocation.contractAddress, layout: layout))
        call.writeField(invocation.expectedCodeHash)
        call.writeField(NoritoLayoutCodec.encodeString(invocation.entrypoint, layout: layout))
        call.writeField(
            NoritoLayoutCodec.encodeOption(invocation.arguments, layout: layout) {
                NoritoLayoutCodec.encodeBytesVec($0, layout: layout)
            }
        )
        return call.data
    }

    static func decodeContractInvocation(
        _ bytes: Data,
        layout: NoritoWireLayout,
        context: String
    ) throws -> TransactionContractInvocation {
        var call = ToriiVerifyingKeyCompactReader(bytes, layout: layout)
        let contractAddress = try ToriiCanonicalTransactionDraft.decodeString(
            try call.takeField("\(context).contract_address"),
            field: "\(context).contract_address",
            layout: layout
        )
        let expectedCodeHash = try call.takeField("\(context).expected_code_hash")
        let entrypoint = try ToriiCanonicalTransactionDraft.decodeString(
            try call.takeField("\(context).entrypoint"),
            field: "\(context).entrypoint",
            layout: layout
        )
        var option = ToriiVerifyingKeyCompactReader(
            try call.takeField("\(context).arguments"),
            layout: layout
        )
        let arguments: Data?
        switch try option.takeUInt8("\(context).arguments.tag") {
        case 0:
            arguments = nil
        case 1:
            var record = ToriiVerifyingKeyCompactReader(
                try option.takeField("\(context).arguments.value"),
                layout: layout
            )
            let count = try record.takeCount("\(context).arguments.count")
            guard count <= UInt64(TransactionContractInvocation.maximumArgumentsBytes) else {
                throw ToriiClientError.invalidPayload(
                    "\(context).arguments exceed the argument-record wire limit."
                )
            }
            arguments = try record.takeBytes(Int(count), field: "\(context).arguments")
            guard record.isFinished else {
                throw ToriiClientError.invalidPayload(
                    "\(context).arguments contain trailing bytes."
                )
            }
        default:
            throw ToriiClientError.invalidPayload("\(context).arguments has an invalid option tag.")
        }
        guard call.isFinished, option.isFinished else {
            throw ToriiClientError.invalidPayload("\(context) contains trailing bytes.")
        }
        do {
            return try TransactionContractInvocation(
                contractAddress: contractAddress,
                expectedCodeHash: expectedCodeHash,
                entrypoint: entrypoint,
                arguments: arguments
            )
        } catch {
            throw ToriiClientError.invalidPayload("\(context) is not one canonical contract call.")
        }
    }

    /// Decode one `Executable` in `layout` and require that it is the exact
    /// canonical encoding of the decoded value in that layout.
    static func decode(
        _ bytes: Data,
        layout: NoritoWireLayout,
        context: String
    ) throws -> TransactionExecutableWire {
        var executable = ToriiVerifyingKeyCompactReader(bytes, layout: layout)
        let tag = try executable.takeEnumTag("\(context).kind")
        let body = try executable.takeField("\(context).value")
        guard executable.isFinished else {
            throw ToriiClientError.invalidPayload("\(context) contains trailing bytes.")
        }
        let decoded: TransactionExecutableWire
        switch tag {
        case 0:
            decoded = .instructions(
                try decodeSequence(body, layout: layout, context: "\(context).instructions") {
                    try TransactionInstructionWire.decode($0, layout: layout, context: $1)
                }
            )
        case 1:
            decoded = .contractCall(
                try decodeContractInvocation(body, layout: layout, context: "\(context).contract_call")
            )
        case 2:
            var wrapper = ToriiVerifyingKeyCompactReader(body, layout: layout)
            var bytecode = ToriiVerifyingKeyCompactReader(
                try wrapper.takeField("\(context).ivm"),
                layout: layout
            )
            let count = try bytecode.takeCount("\(context).ivm.count")
            guard count <= UInt64(Int.max) else {
                throw ToriiClientError.invalidPayload("\(context).ivm exceeds the runtime bound.")
            }
            let code = try bytecode.takeBytes(Int(count), field: "\(context).ivm")
            guard wrapper.isFinished, bytecode.isFinished else {
                throw ToriiClientError.invalidPayload("\(context).ivm contains trailing bytes.")
            }
            decoded = .ivm(code)
        case 4:
            decoded = .batch(
                try decodeSequence(body, layout: layout, context: "\(context).batch") {
                    try TransactionBatchItemWire.decode($0, layout: layout, context: $1)
                }
            )
        case 3:
            // TODO: `Executable::IvmProved` (bytecode, overlay and two
            // commitments) has no builder or shared vector in this SDK yet.
            throw ToriiClientError.invalidPayload(
                "\(context) uses Executable::IvmProved, which this SDK does not decode."
            )
        default:
            throw ToriiClientError.invalidPayload("\(context) has an unknown executable variant.")
        }
        guard try decoded.encode(layout: layout) == bytes else {
            throw ToriiClientError.invalidPayload(
                "\(context) is not the canonical executable encoding of its layout."
            )
        }
        return decoded
    }

    private static func decodeSequence<Element>(
        _ bytes: Data,
        layout: NoritoWireLayout,
        context: String,
        decode: (Data, String) throws -> Element
    ) throws -> [Element] {
        var sequence = ToriiVerifyingKeyCompactReader(bytes, layout: layout)
        let count = try sequence.takeCount("\(context).count")
        // Every element carries at least its own length prefix.
        guard count <= UInt64(bytes.count) else {
            throw ToriiClientError.invalidPayload("\(context) count exceeds its payload.")
        }
        var elements: [Element] = []
        elements.reserveCapacity(Int(count))
        for index in 0..<count {
            let elementContext = "\(context)[\(index)]"
            elements.append(try decode(try sequence.takeField(elementContext), elementContext))
        }
        guard sequence.isFinished else {
            throw ToriiClientError.invalidPayload("\(context) contains trailing bytes.")
        }
        return elements
    }
}
