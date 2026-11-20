import XCTest
@testable import IrohaSwift

/// The dense Norito layout (`norito.md`, "Dense layout (header minor 1)") in the
/// pure-Swift codec, pinned to the shared `fixtures/norito_dense/vectors.json`.
/// Nothing here calls the native bridge.
final class NoritoDenseLayoutTests: XCTestCase {
    // MARK: - Shared vectors

    func testSharedBareVectorsDecodeAndReencodeByteIdenticallyInBothLayouts() throws {
        let document = try DenseVectorDocument.load()
        XCTAssertEqual(document.layout, "norito.md, Dense layout (header minor 1)")
        XCTAssertEqual(
            Set(document.bare.map(\.name)),
            Set(Self.expectedBareValues.keys),
            "every shared vector needs an expected value here, and no vector may disappear"
        )
        XCTAssertEqual(document.bare.count, Self.expectedBareValues.count)

        for vector in document.bare {
            let codec = try XCTUnwrap(
                Self.bareCodecs[vector.type],
                "\(vector.name): no Swift codec for vector type \(vector.type)"
            )
            let expected = try XCTUnwrap(Self.expectedBareValues[vector.name], vector.name)
            let dense = try XCTUnwrap(Data(hexString: vector.dense), vector.name)
            let compact = try XCTUnwrap(Data(hexString: vector.compact), vector.name)

            let fromDense = try codec.decode(dense, .dense)
            XCTAssertEqual(fromDense, expected, "\(vector.name): dense value")
            XCTAssertEqual(try codec.encode(fromDense, .dense), dense, "\(vector.name): dense bytes")

            // The current layout is unchanged: the compact twin decodes to the
            // same value and re-encodes to the same bytes.
            let fromCompact = try codec.decode(compact, .compact)
            XCTAssertEqual(fromCompact, expected, "\(vector.name): compact value")
            XCTAssertEqual(
                try codec.encode(fromCompact, .compact),
                compact,
                "\(vector.name): compact bytes"
            )

            // A payload of one layout never decodes as the other, unless the two
            // encodings are the same bytes (`Option::None`, `Result`).
            if dense != compact {
                XCTAssertThrowsError(
                    try codec.decode(dense, .compact),
                    "\(vector.name): dense bytes read as the compact layout"
                )
                XCTAssertThrowsError(
                    try codec.decode(compact, .dense),
                    "\(vector.name): compact bytes read as the dense layout"
                )
            }

            // Truncated input and trailing bytes are refused in both layouts.
            for (layout, bytes) in [(NoritoWireLayout.dense, dense), (.compact, compact)] {
                XCTAssertThrowsError(
                    try codec.decode(Data(bytes.dropLast()), layout),
                    "\(vector.name): truncated \(layout)"
                )
                XCTAssertThrowsError(
                    try codec.decode(bytes + Data([0]), layout),
                    "\(vector.name): trailing byte \(layout)"
                )
            }
        }
    }

    func testSharedFrameVectorsDeclareTheirLayoutInTheHeader() throws {
        let document = try DenseVectorDocument.load()
        XCTAssertEqual(document.frames.map(\.name), ["wide"])
        let expected = WideRecord(
            value: (high: 0x0102_0304_0506_0708, low: 0x090A_0B0C_0D0E_0F10),
            tag: Data([0xCA, 0xFE])
        )
        for vector in document.frames {
            XCTAssertEqual(vector.type, "Wide { value: u128, tag: [u8; 2] }")
            let schema = noritoSchemaHash(forTypeName: vector.schemaName)
            let dense = try XCTUnwrap(Data(hexString: vector.dense), vector.name)
            let compact = try XCTUnwrap(Data(hexString: vector.compact), vector.name)

            // Dense: minor 1, no flags, no padding, payload at byte 40.
            let denseFrame = try XCTUnwrap(noritoDecodeDeclaredFrame(dense), vector.name)
            XCTAssertEqual(denseFrame.layout, .dense)
            XCTAssertEqual(denseFrame.header.minor, NoritoHeader.versionMinorDense)
            XCTAssertEqual(denseFrame.header.flags, 0)
            XCTAssertEqual(denseFrame.header.layout, .dense)
            XCTAssertEqual(denseFrame.header.schema, schema)
            XCTAssertEqual(denseFrame.paddingLength, 0)
            XCTAssertEqual(denseFrame.payload, Data(dense.dropFirst(NoritoHeader.encodedLength)))
            XCTAssertEqual(try WideRecord.decode(denseFrame.payload, layout: .dense), expected)
            XCTAssertEqual(try expected.encode(layout: .dense), denseFrame.payload)
            // `Wide` is 16-aligned, yet a dense frame carries no padding.
            XCTAssertEqual(
                noritoEncode(
                    typeName: vector.schemaName,
                    payload: try expected.encode(layout: .dense),
                    layout: .dense,
                    payloadAlignment: 16
                ),
                dense
            )

            // Current layout: minor 0, COMPACT_LEN, 8 bytes of alignment padding.
            let compactFrame = try XCTUnwrap(noritoDecodeDeclaredFrame(compact), vector.name)
            XCTAssertEqual(compactFrame.layout, .compact)
            XCTAssertEqual(compactFrame.header.minor, NoritoHeader.versionMinor)
            XCTAssertEqual(compactFrame.header.flags, NoritoHeader.compactLen)
            XCTAssertEqual(compactFrame.header.schema, schema)
            XCTAssertEqual(compactFrame.paddingLength, 8)
            XCTAssertEqual(try WideRecord.decode(compactFrame.payload, layout: .compact), expected)
            XCTAssertEqual(
                noritoEncode(
                    typeName: vector.schemaName,
                    payload: try expected.encode(layout: .compact),
                    layout: .compact,
                    payloadAlignment: 16
                ),
                compact
            )
            XCTAssertEqual(
                noritoEncode(
                    typeName: vector.schemaName,
                    payload: try expected.encode(layout: .compact),
                    flags: NoritoHeader.compactLen,
                    payloadAlignment: 16
                ),
                compact,
                "the flag-based encoder is the layout-based encoder for minor 0"
            )

            // The header is the only layout selector.
            XCTAssertNil(noritoDecodeFrame(dense), "a current-layout decoder refuses a dense frame")
            XCTAssertNotNil(noritoDecodeFrame(dense, layout: .dense))
            XCTAssertNil(noritoDecodeFrame(dense, layout: .compact))
            XCTAssertNil(noritoDecodeFrame(dense, layout: .fixed))
            XCTAssertNotNil(noritoDecodeFrame(compact))
            XCTAssertNotNil(noritoDecodeFrame(compact, layout: .compact))
            XCTAssertNil(noritoDecodeFrame(compact, layout: .dense))
            XCTAssertNil(noritoDecodeFrame(compact, layout: .fixed))
        }
    }

    // MARK: - Header negatives

    func testWireLayoutFromHeaderAdmitsExactlyThreeMinorAndFlagPairs() {
        for minor in UInt8.min...UInt8.max {
            for flags in UInt8.min...UInt8.max {
                let layout = NoritoWireLayout(headerMinor: minor, flags: flags)
                switch (minor, flags) {
                case (0, 0x00):
                    XCTAssertEqual(layout, .fixed)
                case (0, 0x02):
                    XCTAssertEqual(layout, .compact)
                case (1, 0x00):
                    XCTAssertEqual(layout, .dense)
                default:
                    XCTAssertNil(layout, "minor=\(minor) flags=\(flags)")
                }
            }
        }
        for layout in NoritoWireLayout.allCases {
            XCTAssertEqual(
                NoritoWireLayout(headerMinor: layout.minor, flags: layout.headerFlags),
                layout
            )
        }
        XCTAssertEqual(NoritoWireLayout.dense.minor, 1)
        XCTAssertEqual(NoritoWireLayout.dense.headerFlags, 0)
    }

    func testFrameDecoderRefusesUnknownMinorsDenseFlagsAndDensePadding() throws {
        let payload = Data([0x03, 0x01, 0x02, 0x03])
        let dense = noritoEncode(typeName: "test.DenseBytes", payload: payload, layout: .dense)
        XCTAssertEqual(dense.count, NoritoHeader.encodedLength + payload.count)
        XCTAssertEqual(dense[5], 1)
        XCTAssertEqual(dense[NoritoHeader.encodedLength - 1], 0)
        XCTAssertEqual(try XCTUnwrap(noritoDecodeDeclaredFrame(dense)).layout, .dense)

        // Unknown header minor.
        for minor in UInt8(2)...UInt8.max {
            var unknown = dense
            unknown[5] = minor
            XCTAssertNil(noritoDecodeDeclaredFrame(unknown), "minor=\(minor)")
            XCTAssertNil(noritoDecodeFrame(unknown), "minor=\(minor)")
            XCTAssertNil(noritoDecodeFrame(unknown, layout: .dense), "minor=\(minor)")
        }
        // The dense layout has no flags.
        for flags in UInt8(1)...UInt8.max {
            var flagged = dense
            flagged[NoritoHeader.encodedLength - 1] = flags
            XCTAssertNil(noritoDecodeDeclaredFrame(flagged), "flags=\(flags)")
        }
        // The dense layout has no padding, even zero-filled.
        var padded = dense
        padded.insert(contentsOf: [UInt8](repeating: 0, count: 8), at: NoritoHeader.encodedLength)
        XCTAssertNil(noritoDecodeDeclaredFrame(padded))
        // Truncated frame and trailing byte.
        XCTAssertNil(noritoDecodeDeclaredFrame(Data(dense.dropLast())))
        XCTAssertNil(noritoDecodeDeclaredFrame(Data(dense.prefix(NoritoHeader.encodedLength - 1))))
        XCTAssertNil(noritoDecodeDeclaredFrame(dense + Data([0])))
        // A corrupted payload fails the checksum.
        var corrupted = dense
        corrupted[corrupted.count - 1] ^= 0x01
        XCTAssertNil(noritoDecodeDeclaredFrame(corrupted))

        // Relabelling a current-layout frame as dense (or the reverse) changes
        // nothing but the declared layout: the typed decoder of that layout then
        // refuses the payload instead of misreading it.
        let compactPayload = NoritoLayoutCodec.encodeBytesVec(Data([1, 2, 3]), layout: .compact)
        var relabelled = noritoEncode(
            typeName: "test.DenseBytes",
            payload: compactPayload,
            layout: .fixed
        )
        relabelled[5] = NoritoHeader.versionMinorDense
        let relabelledFrame = try XCTUnwrap(noritoDecodeDeclaredFrame(relabelled))
        XCTAssertEqual(relabelledFrame.layout, .dense)
        XCTAssertThrowsError(
            try NoritoLayoutCodec.decodeBytesVec(relabelledFrame.payload, layout: relabelledFrame.layout)
        )
    }

    func testCurrentLayoutFramesAreUnchangedByTheLayoutParameter() throws {
        let payload = Data([0xAA, 0xBB])
        for (flags, layout) in [(UInt8(0), NoritoWireLayout.fixed), (NoritoHeader.compactLen, .compact)] {
            for alignment in [1, 8, 16, 32] {
                let byFlags = noritoEncode(
                    typeName: "test.Current",
                    payload: payload,
                    flags: flags,
                    payloadAlignment: alignment
                )
                let byLayout = noritoEncode(
                    typeName: "test.Current",
                    payload: payload,
                    layout: layout,
                    payloadAlignment: alignment
                )
                XCTAssertEqual(byFlags, byLayout)
                XCTAssertEqual(byFlags[5], 0)
                XCTAssertEqual(byFlags[NoritoHeader.encodedLength - 1], flags)
                let frame = try XCTUnwrap(noritoDecodeFrame(byFlags))
                XCTAssertEqual(frame.layout, layout)
                XCTAssertEqual(frame.header.minor, 0)
                XCTAssertEqual(frame.payload, payload)
            }
        }
    }

    // MARK: - Writer and reader rules

    func testLayoutWriterSelectsLengthCountTagAndByteElementEncodings() {
        func bytes(_ layout: NoritoWireLayout, _ body: (inout NoritoLayoutWriter) -> Void) -> [UInt8] {
            var writer = NoritoLayoutWriter(layout: layout)
            body(&writer)
            return [UInt8](writer.data)
        }
        let u64ThreeHundred: [UInt8] = [0x2C, 0x01, 0, 0, 0, 0, 0, 0]

        XCTAssertEqual(bytes(.fixed) { $0.writeLength(300) }, u64ThreeHundred)
        XCTAssertEqual(bytes(.compact) { $0.writeLength(300) }, [0xAC, 0x02])
        XCTAssertEqual(bytes(.dense) { $0.writeLength(300) }, [0xAC, 0x02])

        XCTAssertEqual(bytes(.fixed) { $0.writeCount(300) }, u64ThreeHundred)
        XCTAssertEqual(bytes(.compact) { $0.writeCount(300) }, u64ThreeHundred)
        XCTAssertEqual(bytes(.dense) { $0.writeCount(300) }, [0xAC, 0x02])

        XCTAssertEqual(bytes(.fixed) { $0.writeEnumTag(300) }, [0x2C, 0x01, 0, 0])
        XCTAssertEqual(bytes(.compact) { $0.writeEnumTag(300) }, [0x2C, 0x01, 0, 0])
        XCTAssertEqual(bytes(.dense) { $0.writeEnumTag(300) }, [0xAC, 0x02])
        XCTAssertEqual(bytes(.dense) { $0.writeEnumTag(127) }, [0x7F])
        XCTAssertEqual(bytes(.dense) { $0.writeEnumTag(128) }, [0x80, 0x01])
        XCTAssertEqual(
            bytes(.dense) { $0.writeEnumTag(UInt32.max) },
            [0xFF, 0xFF, 0xFF, 0xFF, 0x0F]
        )

        XCTAssertEqual(
            bytes(.fixed) { $0.writeByteFields([7, 8]) },
            [1, 0, 0, 0, 0, 0, 0, 0, 7, 1, 0, 0, 0, 0, 0, 0, 0, 8]
        )
        XCTAssertEqual(bytes(.compact) { $0.writeByteFields([7, 8]) }, [1, 7, 1, 8])
        XCTAssertEqual(bytes(.dense) { $0.writeByteFields([7, 8]) }, [7, 8])

        XCTAssertEqual(bytes(.dense) { $0.writeVarint(0) }, [0])
        XCTAssertEqual(
            bytes(.dense) { $0.writeVarint(UInt64.max) },
            [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01]
        )
        XCTAssertEqual(
            bytes(.dense) { $0.writeField(Data([0xAA])) },
            bytes(.compact) { $0.writeField(Data([0xAA])) }
        )

        var wiped = NoritoLayoutWriter(layout: .dense)
        wiped.writeBytes(Data([1, 2, 3]))
        wiped.wipe()
        XCTAssertTrue(wiped.data.isEmpty)
    }

    func testPinnedWritersMatchTheLayoutWriterByteForByte() {
        func script(
            uint8: (UInt8) -> Void,
            uint16: (UInt16) -> Void,
            uint32: (UInt32) -> Void,
            uint64: (UInt64) -> Void,
            length: (UInt64) -> Void,
            bytes: (Data) -> Void,
            field: (Data) -> Void,
            byteFields: ([UInt8]) -> Void
        ) {
            uint8(0xA5)
            uint16(0xBEEF)
            uint32(0xDEAD_BEEF)
            uint64(0x0102_0304_0506_0708)
            length(0)
            length(127)
            length(128)
            length(UInt64.max)
            bytes(Data([9, 8, 7]))
            field(Data())
            field(Data(repeating: 0x33, count: 200))
            byteFields([])
            byteFields(Array(0...255))
        }

        var canonical = CanonicalNoritoWriter()
        var fixed = NoritoLayoutWriter(layout: .fixed)
        script(
            uint8: { canonical.writeUInt8($0); fixed.writeUInt8($0) },
            uint16: { canonical.writeUInt16LE($0); fixed.writeUInt16LE($0) },
            uint32: { canonical.writeUInt32LE($0); fixed.writeUInt32LE($0) },
            uint64: { canonical.writeUInt64LE($0); fixed.writeUInt64LE($0) },
            length: { canonical.writeLength($0); fixed.writeLength($0) },
            bytes: { canonical.writeBytes($0); fixed.writeBytes($0) },
            field: { canonical.writeField($0); fixed.writeField($0) },
            byteFields: { canonical.writeByteFields($0); fixed.writeByteFields($0) }
        )
        XCTAssertEqual(canonical.data, fixed.data)
        // 1 + 2 + 4 + 8 scalars, four u64 lengths, 3 raw bytes, two u64-prefixed
        // fields (0 and 200 bytes) and 256 nine-byte elements.
        XCTAssertEqual(canonical.data.count, 15 + 32 + 3 + 8 + 208 + 256 * 9)

        var compact = CompactNoritoWriter()
        var layoutCompact = NoritoLayoutWriter(layout: .compact)
        script(
            uint8: { compact.writeUInt8($0); layoutCompact.writeUInt8($0) },
            uint16: { compact.writeUInt16LE($0); layoutCompact.writeUInt16LE($0) },
            uint32: { compact.writeUInt32LE($0); layoutCompact.writeUInt32LE($0) },
            uint64: { compact.writeUInt64LE($0); layoutCompact.writeUInt64LE($0) },
            length: { compact.writeLength($0); layoutCompact.writeLength($0) },
            bytes: { compact.writeBytes($0); layoutCompact.writeBytes($0) },
            field: { compact.writeField($0); layoutCompact.writeField($0) },
            byteFields: { compact.writeByteFields($0); layoutCompact.writeByteFields($0) }
        )
        XCTAssertEqual(compact.data, layoutCompact.data)
        // Varint lengths 0, 127, 128 and UInt64.max take 1 + 1 + 2 + 10 bytes;
        // the fields take 1 and 2 + 200; every byte element takes two bytes.
        XCTAssertEqual(compact.data.count, 15 + 14 + 3 + 1 + 202 + 256 * 2)
        compact.wipe()
        XCTAssertTrue(compact.data.isEmpty)

        // The typed helpers of the pinned namespaces are the layout codec.
        let blob = Data([1, 2, 3])
        XCTAssertEqual(CompactNorito.encodeString("ab"), NoritoLayoutCodec.encodeString("ab", layout: .compact))
        XCTAssertEqual(CanonicalNorito.encodeString("ab"), NoritoLayoutCodec.encodeString("ab", layout: .fixed))
        XCTAssertEqual(CompactNorito.encodeBytesVec(blob), NoritoLayoutCodec.encodeBytesVec(blob, layout: .compact))
        XCTAssertEqual(CanonicalNorito.encodeBytesVec(blob), NoritoLayoutCodec.encodeBytesVec(blob, layout: .fixed))
        XCTAssertEqual(CompactNorito.encodeConstVec(blob), NoritoLayoutCodec.encodeConstVec(blob, layout: .compact))
        XCTAssertEqual(CanonicalNorito.encodeConstVec(blob), NoritoLayoutCodec.encodeConstVec(blob, layout: .fixed))
        XCTAssertEqual(
            try CompactNorito.encodeOption(UInt8(7), encode: CompactNorito.encodeUInt8),
            NoritoLayoutCodec.encodeOption(UInt8(7), layout: .compact, encode: CompactNorito.encodeUInt8)
        )
        XCTAssertEqual(
            try CanonicalNorito.encodeOption(UInt8(7), encode: CanonicalNorito.encodeUInt8),
            NoritoLayoutCodec.encodeOption(UInt8(7), layout: .fixed, encode: CanonicalNorito.encodeUInt8)
        )
        XCTAssertEqual(
            try CompactNorito.encodeVec(["a", "bc"], encode: CompactNorito.encodeString),
            NoritoLayoutCodec.encodeVec(["a", "bc"], layout: .compact, encode: CompactNorito.encodeString)
        )
        XCTAssertEqual(
            try CanonicalNorito.encodeVec(["a", "bc"], encode: CanonicalNorito.encodeString),
            NoritoLayoutCodec.encodeVec(["a", "bc"], layout: .fixed, encode: CanonicalNorito.encodeString)
        )
        XCTAssertEqual(try CanonicalNorito.decodeString(CanonicalNorito.encodeString("héllo")), "héllo")
    }

    func testDenseCountsTagsAndArraysRefuseNonCanonicalAndShortInput() throws {
        // Counts: shortest-form varints only.
        XCTAssertEqual(
            try NoritoLayoutCodec.decodeBytesVec(Data([3, 1, 2, 3]), layout: .dense),
            Data([1, 2, 3])
        )
        XCTAssertThrowsError(try NoritoLayoutCodec.decodeBytesVec(Data([0x83, 0x00, 1, 2, 3]), layout: .dense))
        XCTAssertThrowsError(try NoritoLayoutCodec.decodeBytesVec(Data([3, 1, 2]), layout: .dense))
        XCTAssertThrowsError(try NoritoLayoutCodec.decodeBytesVec(Data([3, 1, 2, 3, 0]), layout: .dense))
        XCTAssertThrowsError(try NoritoLayoutCodec.decodeBytesVec(Data(), layout: .dense))
        // The compact count is a `u64`; its bytes are not a dense byte vector.
        XCTAssertThrowsError(
            try NoritoLayoutCodec.decodeBytesVec(Data([3, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3]), layout: .dense)
        )
        XCTAssertThrowsError(try NoritoLayoutCodec.decodeBytesVec(Data([3, 1, 2, 3]), layout: .compact))

        // Enum tags: the `u32` discriminant as a shortest-form varint.
        func tag(_ bytes: [UInt8], _ layout: NoritoWireLayout = .dense) throws -> UInt32 {
            var reader = CanonicalNoritoReader(data: Data(bytes))
            let tag = try reader.readEnumTag(layout: layout)
            try reader.requireFinished()
            return tag
        }
        XCTAssertEqual(try tag([0]), 0)
        XCTAssertEqual(try tag([0xAC, 0x02]), 300)
        XCTAssertEqual(try tag([0xFF, 0xFF, 0xFF, 0xFF, 0x0F]), UInt32.max)
        XCTAssertThrowsError(try tag([0x80, 0x00]), "overlong")
        XCTAssertThrowsError(try tag([0x80, 0x80, 0x80, 0x80, 0x10]), "2^32 does not fit u32")
        XCTAssertThrowsError(try tag([0x80]), "truncated")
        XCTAssertThrowsError(try tag([0, 0, 0, 0]), "a compact u32 tag read as dense")
        XCTAssertEqual(try tag([0x2C, 0x01, 0, 0], .compact), 300)
        XCTAssertEqual(try tag([0x2C, 0x01, 0, 0], .fixed), 300)
        XCTAssertThrowsError(try tag([0xAC, 0x02], .compact), "a dense tag read as compact")

        // Fixed byte arrays: exactly N raw bytes.
        XCTAssertEqual(
            try NoritoLayoutCodec.decodeByteArray(Data([1, 2, 3, 4]), count: 4, layout: .dense),
            Data([1, 2, 3, 4])
        )
        XCTAssertThrowsError(try NoritoLayoutCodec.decodeByteArray(Data([1, 2, 3]), count: 4, layout: .dense))
        XCTAssertThrowsError(
            try NoritoLayoutCodec.decodeByteArray(Data([1, 1, 1, 2, 1, 3, 1, 4]), count: 4, layout: .dense),
            "per-byte framing read as dense"
        )
        XCTAssertEqual(
            try NoritoLayoutCodec.decodeByteArray(Data([1, 1, 1, 2, 1, 3, 1, 4]), count: 4, layout: .compact),
            Data([1, 2, 3, 4])
        )
        XCTAssertThrowsError(
            try NoritoLayoutCodec.decodeByteArray(Data([1, 2, 3, 4]), count: 4, layout: .compact),
            "raw bytes read as per-byte framing"
        )
        XCTAssertThrowsError(
            try NoritoLayoutCodec.decodeByteArray(Data([2, 1, 1, 1]), count: 1, layout: .compact),
            "a byte element must have length one"
        )

        // Byte element sequences (`ConstVec<u8>`).
        let blob = Data([0xDE, 0xAD])
        XCTAssertEqual(NoritoLayoutCodec.encodeConstVec(blob, layout: .dense), Data([2, 0xDE, 0xAD]))
        for layout in NoritoWireLayout.allCases {
            XCTAssertEqual(
                try NoritoLayoutCodec.decodeConstVec(
                    NoritoLayoutCodec.encodeConstVec(blob, layout: layout),
                    layout: layout
                ),
                blob,
                "\(layout)"
            )
            XCTAssertEqual(
                try NoritoLayoutCodec.decodeString(
                    NoritoLayoutCodec.encodeString("dense", layout: layout),
                    layout: layout
                ),
                "dense",
                "\(layout)"
            )
        }
        XCTAssertThrowsError(
            try NoritoLayoutCodec.decodeConstVec(
                NoritoLayoutCodec.encodeConstVec(blob, layout: .compact),
                layout: .dense
            )
        )
        XCTAssertThrowsError(
            try NoritoLayoutCodec.decodeConstVec(
                NoritoLayoutCodec.encodeConstVec(blob, layout: .dense),
                layout: .compact
            )
        )
        // Strings: invalid UTF-8 and overlong lengths are refused.
        XCTAssertThrowsError(try NoritoLayoutCodec.decodeString(Data([1, 0xFF]), layout: .dense))
        XCTAssertThrowsError(try NoritoLayoutCodec.decodeString(Data([0x81, 0x00, 0x61]), layout: .dense))
        // Options keep their one-byte tag in every layout.
        XCTAssertEqual(
            NoritoLayoutCodec.encodeOption(true, layout: .dense) { Data([$0 ? 1 : 0]) },
            Data([1, 1, 1])
        )
        XCTAssertThrowsError(
            try NoritoLayoutCodec.decodeOption(Data([2]), layout: .dense) { $0 }
        )
        XCTAssertThrowsError(
            try NoritoLayoutCodec.decodeOption(Data([0, 0]), layout: .dense) { $0 }
        )
    }

    func testStrictPayloadReaderFollowsItsLayout() throws {
        // Count 2, enum tag 300, one length-prefixed field, two byte elements.
        func encoded(_ layout: NoritoWireLayout) -> Data {
            var writer = NoritoLayoutWriter(layout: layout)
            writer.writeCount(2)
            writer.writeEnumTag(300)
            writer.writeField(Data([0xAB, 0xCD]))
            writer.writeByteFields([0x10, 0x20])
            return writer.data
        }
        for layout in NoritoWireLayout.allCases {
            var reader = ToriiVerifyingKeyCompactReader(encoded(layout), layout: layout)
            XCTAssertEqual(reader.layout, layout)
            XCTAssertEqual(try reader.takeCount("count"), 2, "\(layout)")
            XCTAssertEqual(try reader.takeEnumTag("tag"), 300, "\(layout)")
            XCTAssertEqual(try reader.takeField("field"), Data([0xAB, 0xCD]), "\(layout)")
            XCTAssertEqual(try reader.takeByteElements(2, field: "bytes"), Data([0x10, 0x20]), "\(layout)")
            XCTAssertTrue(reader.isFinished, "\(layout)")
        }
        // The default layout is the compact layout of the current wire.
        XCTAssertEqual(ToriiVerifyingKeyCompactReader(Data()).layout, .compact)

        var overlong = ToriiVerifyingKeyCompactReader(Data([0x80, 0x00]), layout: .dense)
        XCTAssertThrowsError(try overlong.takeCount("count"))
        var overflow = ToriiVerifyingKeyCompactReader(Data([0x80, 0x80, 0x80, 0x80, 0x10]), layout: .dense)
        XCTAssertThrowsError(try overflow.takeEnumTag("tag"))
        var truncated = ToriiVerifyingKeyCompactReader(Data([0x10]), layout: .dense)
        XCTAssertThrowsError(try truncated.takeByteElements(2, field: "bytes"))
        var framedAsDense = ToriiVerifyingKeyCompactReader(Data([1, 0x10, 1, 0x20]), layout: .dense)
        XCTAssertEqual(try framedAsDense.takeByteElements(2, field: "bytes"), Data([1, 0x10]))
        XCTAssertFalse(framedAsDense.isFinished, "the caller sees the unread framing as trailing bytes")
        var rawAsCompact = ToriiVerifyingKeyCompactReader(Data([0x10, 0x20, 0x30, 0x40]), layout: .compact)
        XCTAssertThrowsError(try rawAsCompact.takeByteElements(2, field: "bytes"))

        var canonical = CanonicalNoritoReader(data: encoded(.dense))
        XCTAssertEqual(try canonical.readCount(layout: .dense), 2)
        XCTAssertEqual(try canonical.readEnumTag(layout: .dense), 300)
        XCTAssertEqual(try canonical.readField(layout: .dense), Data([0xAB, 0xCD]))
        XCTAssertEqual(try canonical.readByteFields(count: 2, layout: .dense), Data([0x10, 0x20]))
        XCTAssertNoThrow(try canonical.requireFinished())
        var fixedLength = CanonicalNoritoReader(data: Data([5, 0, 0, 0, 0, 0, 0, 0]))
        XCTAssertEqual(try fixedLength.readLength(layout: .fixed), 5)
        var byteVec = CanonicalNoritoReader(data: Data([2, 0xAA, 0xBB, 0xCC]))
        XCTAssertEqual(try byteVec.readBytesVec(layout: .dense), Data([0xAA, 0xBB]))
        XCTAssertThrowsError(try byteVec.requireFinished())
        var constVec = CanonicalNoritoReader(data: Data([2, 0, 0, 0, 0, 0, 0, 0, 1, 0xAA, 1, 0xBB]))
        XCTAssertEqual(try constVec.readConstVec(layout: .compact), Data([0xAA, 0xBB]))
    }

    // MARK: - Vector codecs

    /// A decoded shared-vector value. The cases mirror the Rust types the
    /// vectors name; `Choice` and `Record` are the vector-local enum and record.
    fileprivate indirect enum VectorValue: Equatable {
        case bytes(Data)
        case byteArray(Data)
        case optionalByteArray(Data?)
        case byteArrays([Data])
        case strings([String])
        case stringToU16([String: UInt16])
        case choiceUnit
        case choiceOne(String)
        case choiceFar(UInt64)
        case result(isError: Bool, value: UInt8)
        case record(id: UInt32, key: Data, digests: [Data], label: String?, choice: VectorValue)
    }

    fileprivate struct VectorCodec {
        let decode: (Data, NoritoWireLayout) throws -> VectorValue
        let encode: (VectorValue, NoritoWireLayout) throws -> Data
    }

    fileprivate static let expectedBareValues: [String: VectorValue] = [
        "u8_vec": .bytes(Data([1, 2, 3])),
        "u8_vec_128": .bytes(Data(repeating: 0x11, count: 128)),
        "u8_array": .byteArray(Data([1, 2, 3, 4])),
        "option_u8_array": .optionalByteArray(Data([9, 9, 9, 9])),
        "option_none": .optionalByteArray(nil),
        "vec_u8_array": .byteArrays([Data([7, 7]), Data([8, 8])]),
        "vec_string": .strings(["ab"]),
        "vec_string_empty": .strings([]),
        "map_string_u16": .stringToU16(["k": 5]),
        "enum_unit": .choiceUnit,
        "enum_tuple": .choiceOne("hi"),
        "enum_far": .choiceFar(1),
        "result_err": .result(isError: true, value: 7),
        "record": .record(
            id: 7,
            key: Data([1, 2, 3, 4]),
            digests: [Data(repeating: 0xAA, count: 4), Data(repeating: 0xBB, count: 4)],
            label: "hi",
            choice: .choiceFar(1)
        ),
    ]

    fileprivate static let bareCodecs: [String: VectorCodec] = {
        var codecs: [String: VectorCodec] = [:]
        codecs["Vec<u8>"] = VectorCodec(
            decode: { data, layout in
                VectorValue.bytes(try NoritoLayoutCodec.decodeBytesVec(data, layout: layout))
            },
            encode: { value, layout in
                guard case let .bytes(bytes) = value else { throw VectorError.shape }
                return NoritoLayoutCodec.encodeBytesVec(bytes, layout: layout)
            }
        )
        codecs["[u8; 4]"] = VectorCodec(
            decode: { data, layout in
                VectorValue.byteArray(
                    try NoritoLayoutCodec.decodeByteArray(data, count: 4, layout: layout)
                )
            },
            encode: { value, layout in
                guard case let .byteArray(bytes) = value else { throw VectorError.shape }
                return NoritoLayoutCodec.encodeByteArray(bytes, layout: layout)
            }
        )
        codecs["Option<[u8; 4]>"] = VectorCodec(
            decode: { data, layout in
                let array: Data? = try NoritoLayoutCodec.decodeOption(data, layout: layout) { body in
                    try NoritoLayoutCodec.decodeByteArray(body, count: 4, layout: layout)
                }
                return VectorValue.optionalByteArray(array)
            },
            encode: { value, layout in
                guard case let .optionalByteArray(bytes) = value else { throw VectorError.shape }
                return NoritoLayoutCodec.encodeOption(bytes, layout: layout) { array in
                    NoritoLayoutCodec.encodeByteArray(array, layout: layout)
                }
            }
        )
        codecs["Vec<[u8; 2]>"] = VectorCodec(
            decode: { data, layout in
                let arrays: [Data] = try NoritoLayoutCodec.decodeVec(data, layout: layout) { body in
                    try NoritoLayoutCodec.decodeByteArray(body, count: 2, layout: layout)
                }
                return VectorValue.byteArrays(arrays)
            },
            encode: { value, layout in
                guard case let .byteArrays(arrays) = value else { throw VectorError.shape }
                return NoritoLayoutCodec.encodeVec(arrays, layout: layout) { array in
                    NoritoLayoutCodec.encodeByteArray(array, layout: layout)
                }
            }
        )
        codecs["Vec<String>"] = VectorCodec(
            decode: { data, layout in
                let strings: [String] = try NoritoLayoutCodec.decodeVec(data, layout: layout) { body in
                    try NoritoLayoutCodec.decodeString(body, layout: layout)
                }
                return VectorValue.strings(strings)
            },
            encode: { value, layout in
                guard case let .strings(strings) = value else { throw VectorError.shape }
                return NoritoLayoutCodec.encodeVec(strings, layout: layout) { text in
                    NoritoLayoutCodec.encodeString(text, layout: layout)
                }
            }
        )
        codecs["BTreeMap<String, u16>"] = VectorCodec(
            decode: NoritoDenseLayoutTests.decodeStringToU16,
            encode: NoritoDenseLayoutTests.encodeStringToU16
        )
        let choice = VectorCodec(
            decode: NoritoDenseLayoutTests.decodeChoice,
            encode: NoritoDenseLayoutTests.encodeChoice
        )
        codecs["Choice (Unit = 0)"] = choice
        codecs["Choice (One(String) = 1)"] = choice
        codecs["Choice (Far { value: u64 } = 300)"] = choice
        codecs["Result<u8, u8>"] = VectorCodec(
            decode: { data, layout in
                var reader = CanonicalNoritoReader(data: data)
                let tag = try reader.readUInt8()
                guard tag <= 1 else { throw VectorError.shape }
                let body = try reader.readField(layout: layout)
                try reader.requireFinished()
                guard body.count == 1 else { throw VectorError.shape }
                return VectorValue.result(isError: tag == 1, value: body[body.startIndex])
            },
            encode: { value, layout in
                guard case let .result(isError, byte) = value else { throw VectorError.shape }
                var writer = NoritoLayoutWriter(layout: layout)
                writer.writeUInt8(isError ? 1 : 0)
                writer.writeField(Data([byte]))
                return writer.data
            }
        )
        codecs[
            "Record { id: u32, key: [u8; 4], digests: Vec<[u8; 4]>, label: Option<String>, choice: Choice }"
        ] = VectorCodec(
            decode: NoritoDenseLayoutTests.decodeRecord,
            encode: NoritoDenseLayoutTests.encodeRecord
        )
        return codecs
    }()

    fileprivate enum VectorError: Error {
        case shape
    }

    /// A map is its entry count, then each key and value length-prefixed in key order.
    private static func decodeStringToU16(_ data: Data, _ layout: NoritoWireLayout) throws -> VectorValue {
        var reader = CanonicalNoritoReader(data: data)
        let count = try reader.readCount(layout: layout)
        guard count <= UInt64(reader.remaining()) else { throw VectorError.shape }
        var map: [String: UInt16] = [:]
        var previousKey: Data?
        for _ in 0..<count {
            let keyBytes = try reader.readField(layout: layout)
            let key = try NoritoLayoutCodec.decodeString(keyBytes, layout: layout)
            var value = CanonicalNoritoReader(data: try reader.readField(layout: layout))
            let number = try value.readUInt16LE()
            try value.requireFinished()
            let sortKey = Data(key.utf8)
            guard previousKey.map({ $0.lexicographicallyPrecedes(sortKey) }) ?? true else {
                throw VectorError.shape
            }
            previousKey = sortKey
            map[key] = number
        }
        try reader.requireFinished()
        return .stringToU16(map)
    }

    private static func encodeStringToU16(_ value: VectorValue, _ layout: NoritoWireLayout) throws -> Data {
        guard case let .stringToU16(map) = value else { throw VectorError.shape }
        var writer = NoritoLayoutWriter(layout: layout)
        writer.writeCount(UInt64(map.count))
        for key in map.keys.sorted(by: { Data($0.utf8).lexicographicallyPrecedes(Data($1.utf8)) }) {
            writer.writeField(NoritoLayoutCodec.encodeString(key, layout: layout))
            var number = NoritoLayoutWriter(layout: layout)
            number.writeUInt16LE(map[key] ?? 0)
            writer.writeField(number.data)
        }
        return writer.data
    }

    /// `Choice { Unit = 0, One(String) = 1, Far { value: u64 } = 300 }`.
    private static func decodeChoice(_ data: Data, _ layout: NoritoWireLayout) throws -> VectorValue {
        var reader = CanonicalNoritoReader(data: data)
        let value: VectorValue
        switch try reader.readEnumTag(layout: layout) {
        case 0:
            value = .choiceUnit
        case 1:
            value = .choiceOne(
                try NoritoLayoutCodec.decodeString(try reader.readField(layout: layout), layout: layout)
            )
        case 300:
            var far = CanonicalNoritoReader(data: try reader.readField(layout: layout))
            value = .choiceFar(try far.readUInt64LE())
            try far.requireFinished()
        default:
            throw VectorError.shape
        }
        try reader.requireFinished()
        return value
    }

    private static func encodeChoice(_ value: VectorValue, _ layout: NoritoWireLayout) throws -> Data {
        switch value {
        case .choiceUnit:
            return NoritoLayoutCodec.encodeEnum(tag: 0, fields: [], layout: layout)
        case let .choiceOne(text):
            return NoritoLayoutCodec.encodeEnum(
                tag: 1,
                fields: [NoritoLayoutCodec.encodeString(text, layout: layout)],
                layout: layout
            )
        case let .choiceFar(number):
            var far = NoritoLayoutWriter(layout: layout)
            far.writeUInt64LE(number)
            return NoritoLayoutCodec.encodeEnum(tag: 300, fields: [far.data], layout: layout)
        default:
            throw VectorError.shape
        }
    }

    /// Every record field keeps its own length prefix. A `[u8; N]` record field is
    /// `[len = N][raw bytes]` in both layouts; the per-byte framing of the current
    /// layouts applies to arrays in other positions (here the `digests` elements).
    private static func decodeRecord(_ data: Data, _ layout: NoritoWireLayout) throws -> VectorValue {
        var reader = CanonicalNoritoReader(data: data)
        var id = CanonicalNoritoReader(data: try reader.readField(layout: layout))
        let idValue = try id.readUInt32LE()
        try id.requireFinished()
        let key = try reader.readField(layout: layout)
        guard key.count == 4 else { throw VectorError.shape }
        let digests = try NoritoLayoutCodec.decodeVec(try reader.readField(layout: layout), layout: layout) {
            try NoritoLayoutCodec.decodeByteArray($0, count: 4, layout: layout)
        }
        let label = try NoritoLayoutCodec.decodeOption(try reader.readField(layout: layout), layout: layout) {
            try NoritoLayoutCodec.decodeString($0, layout: layout)
        }
        let choice = try decodeChoice(try reader.readField(layout: layout), layout)
        try reader.requireFinished()
        return .record(id: idValue, key: key, digests: digests, label: label, choice: choice)
    }

    private static func encodeRecord(_ value: VectorValue, _ layout: NoritoWireLayout) throws -> Data {
        guard case let .record(id, key, digests, label, choice) = value else { throw VectorError.shape }
        var writer = NoritoLayoutWriter(layout: layout)
        var idWriter = NoritoLayoutWriter(layout: layout)
        idWriter.writeUInt32LE(id)
        writer.writeField(idWriter.data)
        writer.writeField(key)
        writer.writeField(
            NoritoLayoutCodec.encodeVec(digests, layout: layout) {
                NoritoLayoutCodec.encodeByteArray($0, layout: layout)
            }
        )
        writer.writeField(
            NoritoLayoutCodec.encodeOption(label, layout: layout) {
                NoritoLayoutCodec.encodeString($0, layout: layout)
            }
        )
        writer.writeField(try encodeChoice(choice, layout))
        return writer.data
    }
}

/// `Wide { value: u128, tag: [u8; 2] }`, the 16-aligned frame vector.
private struct WideRecord: Equatable {
    let value: (high: UInt64, low: UInt64)
    let tag: Data

    static func == (lhs: WideRecord, rhs: WideRecord) -> Bool {
        lhs.value.high == rhs.value.high && lhs.value.low == rhs.value.low && lhs.tag == rhs.tag
    }

    static func decode(_ data: Data, layout: NoritoWireLayout) throws -> WideRecord {
        var reader = CanonicalNoritoReader(data: data)
        var value = CanonicalNoritoReader(data: try reader.readField(layout: layout))
        let low = try value.readUInt64LE()
        let high = try value.readUInt64LE()
        try value.requireFinished()
        let tag = try reader.readField(layout: layout)
        try reader.requireFinished()
        guard tag.count == 2 else {
            throw CanonicalNoritoDecodingError.invalidField("Wide.tag must be two bytes")
        }
        return WideRecord(value: (high: high, low: low), tag: tag)
    }

    func encode(layout: NoritoWireLayout) throws -> Data {
        var number = NoritoLayoutWriter(layout: layout)
        number.writeUInt64LE(value.low)
        number.writeUInt64LE(value.high)
        var writer = NoritoLayoutWriter(layout: layout)
        writer.writeField(number.data)
        writer.writeField(tag)
        return writer.data
    }
}

private struct DenseVectorDocument: Decodable {
    struct Bare: Decodable {
        let name: String
        let type: String
        let value: String
        let compact: String
        let dense: String

        private enum CodingKeys: String, CodingKey {
            case name, type, value, compact, dense
        }

        init(from decoder: Decoder) throws {
            try requireExactFixtureKeys(
                decoder,
                ["name", "type", "value", "compact", "dense"],
                context: "dense bare vector"
            )
            let container = try decoder.container(keyedBy: CodingKeys.self)
            name = try container.decode(String.self, forKey: .name)
            type = try container.decode(String.self, forKey: .type)
            value = try container.decode(String.self, forKey: .value)
            compact = try container.decode(String.self, forKey: .compact)
            dense = try container.decode(String.self, forKey: .dense)
        }
    }

    struct Frame: Decodable {
        let name: String
        let schemaName: String
        let type: String
        let value: String
        let compact: String
        let dense: String

        private enum CodingKeys: String, CodingKey {
            case name
            case schemaName = "schema_name"
            case type, value, compact, dense
        }

        init(from decoder: Decoder) throws {
            try requireExactFixtureKeys(
                decoder,
                ["name", "schema_name", "type", "value", "compact", "dense"],
                context: "dense frame vector"
            )
            let container = try decoder.container(keyedBy: CodingKeys.self)
            name = try container.decode(String.self, forKey: .name)
            schemaName = try container.decode(String.self, forKey: .schemaName)
            type = try container.decode(String.self, forKey: .type)
            value = try container.decode(String.self, forKey: .value)
            compact = try container.decode(String.self, forKey: .compact)
            dense = try container.decode(String.self, forKey: .dense)
        }
    }

    let layout: String
    let bare: [Bare]
    let frames: [Frame]

    private enum CodingKeys: String, CodingKey {
        case layout, bare, frames
    }

    init(from decoder: Decoder) throws {
        try requireExactFixtureKeys(decoder, ["layout", "bare", "frames"], context: "dense vectors")
        let container = try decoder.container(keyedBy: CodingKeys.self)
        layout = try container.decode(String.self, forKey: .layout)
        bare = try container.decode([Bare].self, forKey: .bare)
        frames = try container.decode([Frame].self, forKey: .frames)
    }

    static func load() throws -> DenseVectorDocument {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // NoritoDenseLayoutTests.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift package root
        let url = root.appendingPathComponent("fixtures/norito_dense/vectors.json")
        return try StrictFixtureJSON.decode(
            DenseVectorDocument.self,
            from: Data(contentsOf: url),
            using: JSONDecoder(),
            context: url.path
        )
    }
}
