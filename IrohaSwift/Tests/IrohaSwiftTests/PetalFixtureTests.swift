import XCTest
@testable import IrohaSwift

/// Conformance against the shared golden vectors in
/// `fixtures/petal/petal_stream_v1.json` (every section of the file).
final class PetalFixtureTests: XCTestCase {
    private typealias Support = PetalTestSupport

    private func fixture() throws -> [String: Any] {
        try XCTUnwrap(Support.streamFixture, "fixtures/petal/petal_stream_v1.json is missing")
    }

    func testConstantsAndLayoutMatch() throws {
        let doc = try fixture()
        XCTAssertEqual(try Support.integer(doc, "fixture_version"), 1)
        XCTAssertEqual(try Support.string(doc, "format"), "petal-stream-v1")
        let constants = try Support.object(doc, "constants")
        XCTAssertEqual(Double(try Support.integer(constants, "canvas")), PetalLayout.canvas)
        XCTAssertEqual(Double(try Support.integer(constants, "tile_origin")), PetalLayout.tileOrigin)
        XCTAssertEqual(Double(try Support.integer(constants, "tile_pitch")), PetalLayout.tilePitch)
        XCTAssertEqual(Double(try Support.integer(constants, "tile_size")), PetalLayout.tileSize)
        XCTAssertEqual(Double(try Support.integer(constants, "dot_radius")), PetalLayout.dotRadius)
        XCTAssertEqual(try Support.integer(constants, "atom_len"), PetalLane.atomLength)
        XCTAssertEqual(try Support.integer(constants, "p_word"), PetalLane.p.wordLength)
        XCTAssertEqual(try Support.integer(constants, "k_word"), PetalLane.k.wordLength)
        XCTAssertEqual(try Support.integer(constants, "d_word"), PetalLane.d.wordLength)
        XCTAssertEqual(try Support.integer(constants, "p_parity"), PetalLane.p.parityLength)
        XCTAssertEqual(try Support.integer(constants, "k_parity"), PetalLane.k.parityLength)
        XCTAssertEqual(try Support.integer(constants, "d_parity"), PetalLane.d.parityLength)
        XCTAssertEqual(try Support.integer(constants, "beacon_interval"), Int(PetalStream.beaconInterval))
        XCTAssertEqual(try Support.integer(constants, "format_version"), Int(PetalStream.formatVersion))

        let layout = try Support.object(doc, "layout")
        XCTAssertEqual(try Support.array(layout, "mask") as? [String], PetalLayout.mask)
        XCTAssertEqual(
            try Support.integers(layout, "tiles_col_row"),
            PetalLayout.tiles.flatMap { [$0.column, $0.row] }
        )
        XCTAssertEqual(
            try Support.integers(layout, "ring_radii").map(Double.init),
            PetalLayout.ringRadii
        )
        XCTAssertEqual(try Support.integers(layout, "ring_slots"), PetalLayout.ringSlots)
        XCTAssertEqual(
            try Support.integers(layout, "finder_centers").map(Double.init),
            PetalLayout.finderCenters.flatMap { [$0.x, $0.y] }
        )
        XCTAssertEqual(try Support.integers(layout, "data_slots"), PetalLayout.dataSlots)
        XCTAssertEqual(try Support.integers(layout, "gate_slots"), PetalLayout.gateSlots)
        XCTAssertEqual(try Support.integers(layout, "guard_slots"), PetalLayout.guardSlots)
    }

    func testGlyphAlphabetStrokesAndTemplatesMatch() throws {
        let glyphs = try Support.object(try fixture(), "glyphs")
        XCTAssertEqual(try Support.string(glyphs, "chars"), String(PetalGlyphs.characters))
        let strokeWidth = try XCTUnwrap(glyphs["stroke_width"] as? NSNumber).doubleValue
        XCTAssertEqual(strokeWidth, PetalGlyphs.strokeWidth, accuracy: 1e-9)
        let strokes = try XCTUnwrap(glyphs["strokes"] as? [[[NSNumber]]])
        XCTAssertEqual(strokes.count, PetalGlyphs.count)
        for (glyph, polylines) in strokes.enumerated() {
            XCTAssertEqual(
                polylines.map { $0.map(\.doubleValue) },
                PetalGlyphs.strokes[glyph].map { $0.flatMap { [$0.x, $0.y] } },
                "glyph \(glyph)"
            )
        }
        let templates = try XCTUnwrap(glyphs["templates"] as? [[NSNumber]])
        XCTAssertEqual(templates.count, PetalGlyphs.templates.count)
        for (glyph, row) in templates.enumerated() {
            XCTAssertEqual(row.map(\.uint8Value), PetalGlyphs.templates[glyph], "glyph \(glyph)")
        }
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
        XCTAssertEqual(try Support.bytes(whitening, "P"), PetalLane.p.whitening)
        XCTAssertEqual(try Support.bytes(whitening, "K"), PetalLane.k.whitening)
        XCTAssertEqual(try Support.bytes(whitening, "D"), PetalLane.d.whitening)
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
                XCTAssertEqual(data.p, try Support.bytes(frame, "p_data"), "\(name) frame \(number)")
                XCTAssertEqual(data.k, try Support.bytes(frame, "k_data"), "\(name) frame \(number)")
                XCTAssertEqual(data.d, try Support.bytes(frame, "d_data"), "\(name) frame \(number)")
                let pWord = try Support.bytes(frame, "p_word")
                let kWord = try Support.bytes(frame, "k_word")
                let dWord = try Support.bytes(frame, "d_word")
                XCTAssertEqual(try PetalLane.p.encode(data.p), pWord)
                XCTAssertEqual(try PetalLane.k.encode(data.k), kWord)
                XCTAssertEqual(try PetalLane.d.encode(data.d), dWord)
                let cells = try PetalFrameCells(p: pWord, k: kWord, d: dWord)
                XCTAssertEqual(
                    cells.glyph.map { String($0, radix: 16) }.joined(),
                    try Support.string(frame, "glyphs")
                )
                XCTAssertEqual(
                    cells.dots.indices.filter { cells.dots[$0] },
                    try Support.integers(frame, "lit_dots")
                )
                XCTAssertEqual(cells, encoder.cells(frame: number))
            }
            // push every fixture frame through lane decoding and the assembler
            if name == "one-pass" {
                var assembler = PetalStreamAssembler()
                for frame in try Support.objects(stream, "frames") {
                    let d = try PetalLane.d.decode(try Support.bytes(frame, "d_word"))
                    assembler.push(dLane: try XCTUnwrap(PetalStream.parseDLane(d)))
                    for (lane, key) in [(PetalLane.p, "p_word"), (PetalLane.k, "k_word")] {
                        let data = try lane.decode(try Support.bytes(frame, key))
                        assembler.push(atoms: try XCTUnwrap(PetalStream.parseAtomLane(lane, data: data)))
                    }
                }
                XCTAssertEqual(try XCTUnwrap(assembler.takeCompleted()).payload, Data(payload))
            }
        }
    }
}
