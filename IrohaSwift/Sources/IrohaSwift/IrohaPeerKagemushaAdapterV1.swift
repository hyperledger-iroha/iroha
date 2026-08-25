import Foundation

/// Narrow adapter between mainline native-canonical Kagemusha archives and the
/// bounded IPM1 small-handoff rail. It never reconstructs Norito bytes and
/// does not alter mobile bridge ABI22. Full Kagemusha V4 archives up to 32 MiB
/// stay on the existing typed Kagemusha rails.
package enum IrohaPeerKagemushaAdapterV1 {
    /// The only IPM1 schema accepted for mainline native Kagemusha archives.
    public static let nativeArchiveSchemaVersion: UInt16 = 0x0102

    public static func wrap(
        _ payload: KagemushaPeerPayload,
        compressionPolicy: IrohaPeerWireCompressionPolicyV1 = .disabled,
        limits: IrohaPeerWireLimitsV1 = .peerV1
    ) throws -> IrohaPeerWireMessageV1 {
        try IrohaPeerWireMessageV1(
            profile: .kagemusha,
            kind: payload.kind.irohaPeerKind,
            schemaVersion: nativeArchiveSchemaVersion,
            canonicalPayload: payload.archive,
            compressionPolicy: compressionPolicy,
            limits: limits
        )
    }

    public static func decode(
        _ message: IrohaPeerWireMessageV1,
        chainDiscriminant: UInt16
    ) throws -> KagemushaPeerPayload {
        guard message.profile == .kagemusha else {
            throw IrohaPeerWireMessageErrorV1.unexpectedProfile(
                expected: .kagemusha,
                actual: message.profile
            )
        }
        guard message.schemaVersion == nativeArchiveSchemaVersion else {
            throw IrohaPeerWireMessageErrorV1.invalidSchemaVersion
        }
        return try KagemushaPeerPayload.decode(
            archive: message.canonicalPayload,
            kind: message.kind.kagemushaKind,
            chainDiscriminant: chainDiscriminant
        )
    }
}

package extension KagemushaPeerPayload {
    func irohaPeerWireMessage(
        compressionPolicy: IrohaPeerWireCompressionPolicyV1 = .disabled,
        limits: IrohaPeerWireLimitsV1 = .peerV1
    ) throws -> IrohaPeerWireMessageV1 {
        try IrohaPeerKagemushaAdapterV1.wrap(
            self,
            compressionPolicy: compressionPolicy,
            limits: limits
        )
    }
}

private extension KagemushaPeerPayloadKind {
    var irohaPeerKind: IrohaPeerWireKindV1 {
        switch self {
        case .receiveRequest: return .receiveRequest
        case .payment: return .payment
        case .acknowledgement: return .acknowledgement
        }
    }
}

private extension IrohaPeerWireKindV1 {
    var kagemushaKind: KagemushaPeerPayloadKind {
        switch self {
        case .receiveRequest: return .receiveRequest
        case .payment: return .payment
        case .acknowledgement: return .acknowledgement
        }
    }
}
