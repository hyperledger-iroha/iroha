import XCTest
@testable import IrohaSwift

/// Conformance against the shared golden vectors in
/// `fixtures/petal/petal_stream_v2.json` (every section of the file).
final class PetalFixtureTests: XCTestCase {
    private typealias Support = PetalTestSupport

    private func fixture() throws -> [String: Any] {
        try XCTUnwrap(Support.streamFixture, "fixtures/petal/petal_stream_v2.json is missing")
    }

    /// A fixture number that the reference wrote from a single-precision constant.
    private func single(_ value: [String: Any], _ key: String) throws -> Float {
        Float(try XCTUnwrap(value[key] as? NSNumber, key).doubleValue)
    }

    func testConstantsAndLayoutMatch() throws {
        let doc = try fixture()
        XCTAssertEqual(try Support.integer(doc, "fixture_version"), 2)
        XCTAssertEqual(try Support.string(doc, "format"), "petal-stream-v2")
        let constants = try Support.object(doc, "constants")
        XCTAssertEqual(Double(try Support.integer(constants, "canvas")), PetalLayout.canvas)
        XCTAssertEqual(try Support.integer(constants, "atom_len"), PetalLane.atomLength)
        XCTAssertEqual(try Support.integer(constants, "d_word"), PetalLane.d.wordLength)
        XCTAssertEqual(try Support.integer(constants, "d_parity"), PetalLane.d.parityLength)
        XCTAssertEqual(try Support.integer(constants, "field_word"), PetalLane.field(0).wordLength)
        XCTAssertEqual(try Support.integer(constants, "field_parity"), PetalLane.field(0).parityLength)
        XCTAssertEqual(try Support.integer(constants, "field_atoms"), PetalLane.field(0).atomCount)
        XCTAssertEqual(try Support.integer(constants, "sectors"), PetalLayout.sectors)
        XCTAssertEqual(try Support.integer(constants, "atoms_per_frame"), PetalLane.atomsPerFrame)
        XCTAssertEqual(try Support.integer(constants, "field_cells"), PetalLayout.fieldCells)
        XCTAssertEqual(try Support.integer(constants, "beacon_interval"), Int(PetalStream.beaconInterval))
        XCTAssertEqual(try Support.integer(constants, "format_version"), Int(PetalStream.formatVersion))

        let layout = try Support.object(doc, "layout")
        XCTAssertEqual(
            try Support.integers(layout, "finder_centers").map(Double.init),
            PetalLayout.finderCenters.flatMap { [$0.x, $0.y] }
        )
        XCTAssertEqual(Double(try Support.integer(layout, "band_inner")), PetalLayout.bandInner)
        XCTAssertEqual(Double(try Support.integer(layout, "band_spacing")), PetalLayout.bandSpacing)
        XCTAssertEqual(try Support.integers(layout, "band_slots"), PetalLayout.bandSlots)
        XCTAssertEqual(try single(layout, "petal_radius"), Float(PetalLayout.petalRadius))
        XCTAssertEqual(try single(layout, "petal_half_length"), Float(PetalLayout.petalHalfLength))
        XCTAssertEqual(try single(layout, "petal_half_width"), Float(PetalLayout.petalHalfWidth))
        XCTAssertEqual(try single(layout, "petal_notch_radius"), Float(PetalLayout.petalNotchRadius))
        XCTAssertEqual(try single(layout, "petal_notch_distance"), Float(PetalLayout.petalNotchDistance))
        let sectors = try XCTUnwrap(layout["sector_cells"] as? [[NSNumber]])
        XCTAssertEqual(sectors.count, PetalLayout.sectors)
        for (sector, cells) in sectors.enumerated() {
            XCTAssertEqual(cells.map(\.intValue), PetalLayout.sectorCells(sector), "sector \(sector)")
        }
        XCTAssertEqual(try Support.array(layout, "mask") as? [String], PetalLayout.mask)
        XCTAssertEqual(Double(try Support.integer(layout, "mask_origin")), PetalLayout.maskOrigin)
        XCTAssertEqual(Double(try Support.integer(layout, "mask_pitch")), PetalLayout.maskPitch)
        XCTAssertEqual(Double(try Support.integer(layout, "backdrop_radius")), PetalLayout.backdropRadius)
        XCTAssertEqual(
            Support.flags(try Support.bytes(layout, "light_cells_hex")),
            (0..<PetalLayout.fieldCells).map(PetalLayout.cellLight)
        )
        XCTAssertEqual(
            try Support.integers(layout, "ring_radii").map(Double.init),
            PetalLayout.ringRadii
        )
        XCTAssertEqual(try Support.integers(layout, "ring_slots"), PetalLayout.ringSlots)
        XCTAssertEqual(Double(try Support.integer(layout, "dot_radius")), PetalLayout.dotRadius)
        XCTAssertEqual(try Support.integers(layout, "data_slots"), PetalLayout.dataSlots)
        XCTAssertEqual(try Support.integers(layout, "gate_slots"), PetalLayout.gateSlots)
        XCTAssertEqual(try Support.integers(layout, "guard_slots"), PetalLayout.guardSlots)

        let palette = try Support.object(doc, "palette")
        func color(_ key: String) throws -> PetalColor {
            let channels = try Support.integers(palette, key).map(UInt8.init)
            return PetalColor(red: channels[0], green: channels[1], blue: channels[2])
        }
        XCTAssertEqual(try color("background"), PetalPalette.standard.background)
        XCTAssertEqual(try color("light"), PetalPalette.standard.light)
        XCTAssertEqual(try color("pink"), PetalPalette.standard.pink)
        XCTAssertEqual(try color("backdrop"), PetalPalette.standard.backdrop)
    }

    func testChecksumsPRNGAndWhiteningMatch() throws {
        let doc = try fixture()
        for entry in try Support.objects(doc, "crc32c") {
            XCTAssertEqual(
                Int(IrohaPeerCRC32CV1.checksum(try Support.bytes(entry, "input_hex"))),
                try Support.integer(entry, "crc32c")
            )
        }
        let prng = try Support.object(doc, "prng")
        var rng = PetalXorshift32(seed: 1)
        XCTAssertEqual(try Support.integers(prng, "xorshift32_seed1"), (0..<6).map { _ in Int(rng.nextUInt32()) })
        for entry in try Support.objects(prng, "mix32") {
            let input = UInt32(try Support.integer(entry, "in"))
            XCTAssertEqual(Int(PetalFountain.mix32(input)), try Support.integer(entry, "out"))
        }
        let whitening = try Support.object(doc, "whitening")
        XCTAssertEqual(try Support.bytes(whitening, "D"), PetalLane.d.whitening)
        for sector in 0..<PetalLayout.sectors {
            XCTAssertEqual(try Support.bytes(whitening, "F\(sector)"), PetalLane.field(sector).whitening, "F\(sector)")
        }
    }

    func testReedSolomonVectorsEncodeAndCorrect() throws {
        for entry in try Support.objects(try fixture(), "reed_solomon") {
            let nsym = try Support.integer(entry, "nsym")
            let data = try Support.bytes(entry, "data_hex")
            let word = try Support.bytes(entry, "codeword_hex")
            let rs = try PetalReedSolomon(parityLength: nsym)
            XCTAssertEqual(try rs.encode(data), word)
            // damage up to the correction capacity and recover
            var damaged = word
            for i in 0..<(nsym / 2) { damaged[i * 3 % word.count] ^= 0x5A }
            try rs.decode(&damaged)
            XCTAssertEqual(damaged, word, "nsym \(nsym)")
        }
    }

    func testFountainMasksAndAtomIDsMatch() throws {
        let doc = try fixture()
        for entry in try Support.objects(doc, "fountain_masks") {
            let mask = PetalFountain.maskWords(
                sourceAtoms: try Support.integer(entry, "k"),
                crc: UInt32(try Support.integer(entry, "crc")),
                id: UInt32(try Support.integer(entry, "id"))
            )
            XCTAssertEqual(mask.map(Int.init), try Support.integers(entry, "mask"))
        }
        let ids = try Support.object(doc, "first_atom_ids")
        for (frame, id) in zip(try Support.integers(ids, "frames"), try Support.integers(ids, "ids")) {
            XCTAssertEqual(Int(PetalStream.firstAtomID(frame: UInt16(frame))), id, "frame \(frame)")
        }
    }

    func testStreamsEncodeIdenticallyAndReassemble() throws {
        let streams = try Support.objects(try fixture(), "streams")
        XCTAssertEqual(streams.count, 3)
        for stream in streams {
            let name = try Support.string(stream, "name")
            let payload = try Support.bytes(stream, "payload_hex")
            let encoder = try PetalStreamEncoder(payload: payload, kind: UInt8(try Support.integer(stream, "kind")))
            let meta = encoder.meta
            XCTAssertEqual(Int(meta.length), try Support.integer(stream, "len"), name)
            XCTAssertEqual(Int(meta.crc), try Support.integer(stream, "crc32c"), name)
            XCTAssertEqual(Int(meta.tag), try Support.integer(stream, "tag"), name)
            XCTAssertEqual(meta.sourceAtoms, try Support.integer(stream, "source_atoms"), name)
            XCTAssertEqual(encoder.systematicFrames, try Support.integer(stream, "systematic_frames"), name)
            for frame in try Support.objects(stream, "frames") {
                let number = UInt16(try Support.integer(frame, "frame"))
                let data = encoder.laneData(frame: number)
                XCTAssertEqual(data.d, try Support.bytes(frame, "d_data"), "\(name) frame \(number)")
                let fieldData = try XCTUnwrap(frame["field_data"] as? [String]).map(Support.hex)
                XCTAssertEqual(data.fields, fieldData, "\(name) frame \(number)")
                let dWord = try Support.bytes(frame, "d_word")
                let fieldWords = try XCTUnwrap(frame["field_words"] as? [String]).map(Support.hex)
                XCTAssertEqual(try PetalLane.d.encode(data.d), dWord)
                for (sector, word) in fieldWords.enumerated() {
                    XCTAssertEqual(
                        try PetalLane.field(sector).encode(data.fields[sector]),
                        word,
                        "\(name) frame \(number) sector \(sector)"
                    )
                }
                let cells = try PetalFrameCells(d: dWord, fields: fieldWords)
                XCTAssertEqual(
                    cells.dots.indices.filter { cells.dots[$0] },
                    try Support.integers(frame, "lit_dots")
                )
                XCTAssertEqual(
                    cells.petals,
                    Support.flags(try Support.bytes(frame, "lit_petals_hex")),
                    "\(name) frame \(number)"
                )
                XCTAssertEqual(cells, encoder.cells(frame: number))
            }
            // push every fixture frame through lane decoding and the assembler
            if name == "one-pass" {
                var assembler = PetalStreamAssembler()
                for frame in try Support.objects(stream, "frames") {
                    let d = try PetalLane.d.decode(try Support.bytes(frame, "d_word"))
                    assembler.push(dLane: try XCTUnwrap(PetalStream.parseDLane(d)))
                    let words = try XCTUnwrap(frame["field_words"] as? [String]).map(Support.hex)
                    for (sector, word) in words.enumerated() {
                        let lane = PetalLane.field(sector)
                        let data = try lane.decode(word)
                        assembler.push(atoms: try XCTUnwrap(PetalStream.parseAtomLane(lane, data: data)))
                    }
                }
                XCTAssertEqual(try XCTUnwrap(assembler.takeCompleted()).payload, Data(payload))
            }
        }
    }
}
