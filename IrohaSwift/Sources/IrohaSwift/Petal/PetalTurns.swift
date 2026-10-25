import Foundation

/// The turn layer of Petal Stream: data in the way the lit petals point (port
/// of `crates/iroha_petal/src/turns.rs`).
///
/// A field lane tells the decoder exactly which petals of its sector are lit.
/// The first ``carriersPerSector`` lit petals of the sector, in the fixed
/// pseudo-random ``carrierOrder(_:)``, carry two more bits each: the petal is
/// turned about its own centre by 0°, 45°, 90° or 135° from its outward
/// direction (``turnAngle(_:)``). A turned petal stays inside its own cell, so
/// it moves no light toward its neighbours and the field lanes read as before;
/// a camera that resolves the petals' shapes also reads the sector's turn lane.
///
/// The turn lane of a sector is one Reed–Solomon codeword, RS(90, 68),
/// XOR-whitened: four fountain atoms from the repair range (ids from
/// ``idBase``, see ``turnFirstID(frame:sector:)``) and a CRC-32C check that
/// binds them to the stream tag, frame and sector of the field lane they ride
/// on (``turnCheck(_:sector:atoms:)``). Carrier `j` shows bits `2j` and
/// `2j + 1` of the transmitted codeword (most significant bit first) as a
/// Gray-coded position (``turnPosition(_:)``). A sector with fewer than
/// ``carriersPerSector`` lit petals turns none (about one sector in 60,000).
///
/// ``readTurns(_:frame:skip:)`` reads the turn lanes of a decoded capture:
/// every carrier's neighbourhood is sampled in the petal's own outward frame,
/// following a smooth correction of the pose fitted to the carriers'
/// centroids; the blur is the one under which the carriers and their lit
/// neighbours explain the image best; and each carrier takes the turn whose
/// (slightly wider) template matches best once its lit neighbours, at their
/// own decided turns, are subtracted.
public enum PetalTurns {
    /// Turn positions a carrier petal can take.
    public static let positions = 4
    /// Bits one carrier shows.
    public static let bitsPerCarrier = 2
    /// Carrier petals of a sector.
    public static let carriersPerSector = 360
    /// Codeword length of a turn lane in bytes.
    public static let wordLength = carriersPerSector * bitsPerCarrier / 8
    /// Parity bytes of a turn lane.
    public static let parityLength = 22
    /// Data bytes of a turn lane: the atoms, then their check.
    public static let dataLength = wordLength - parityLength
    /// Atoms carried by each turn lane.
    public static let atomCount = 4
    /// Bytes of the check that closes a turn lane's data.
    public static let checkLength = 4
    /// Fountain id of the first turn atom of frame 0. The field lanes and lane
    /// `D` number their atoms from 0 and never come near it.
    public static let idBase: UInt32 = 1 << 31
    /// Gaussian blurs (σ in canvas units) the turn reader fits.
    public static let blurs: [Double] = [1.0, 1.4, 1.9, 2.5, 3.2, 4.0]

    /// Cells of a sector that carry its field codeword (and may carry turns).
    static let codewordCells = PetalLane.field(0).wordLength * 8

    /// The angle (radians, clockwise on screen) by which a petal at turn
    /// position `position` is turned from its outward direction: `π · position / 4`.
    public static func turnAngle(_ position: UInt8) -> Double {
        Double.pi * Double(position) / Double(positions)
    }

    /// The two bits a turn position shows: its Gray code `position ^ (position >> 1)`.
    /// Neighbouring positions (135° and 0° neighbour too, a petal turned by
    /// 180° differing only by its notch) differ in one bit.
    public static func turnLabel(_ position: UInt8) -> UInt8 {
        position ^ (position >> 1)
    }

    /// The turn position that shows the two bits `label` (the inverse of
    /// ``turnLabel(_:)``).
    public static func turnPosition(_ label: UInt8) -> UInt8 {
        label ^ (label >> 1)
    }

    /// The order in which the lit petals of sector `sector` (`0..<4`) become
    /// carriers: its codeword cells (the first `8 × 105` of
    /// ``PetalLayout/sectorCells(_:)``) shuffled by Fisher–Yates with a
    /// ``PetalXorshift32`` seeded `0x4F52_4430 + sector` ("ORD0" … "ORD3"): for
    /// `i` from 839 down to 1, position `i` swaps with position
    /// `nextUInt32() mod (i + 1)`.
    public static func carrierOrder(_ sector: Int) -> [Int] {
        carrierOrders[sector]
    }

    /// ``carrierOrder(_:)`` of the four sectors.
    static let carrierOrders: [[Int]] = (0..<PetalLayout.sectors).map { sector in
        var order = Array(PetalLayout.sectorTable[sector].prefix(codewordCells))
        var rng = PetalXorshift32(seed: 0x4F52_4430 &+ UInt32(sector))
        for i in stride(from: order.count - 1, through: 1, by: -1) {
            let j = Int(rng.nextUInt32() % UInt32(i + 1))
            order.swapAt(i, j)
        }
        return order
    }

    /// The carriers of sector `sector` under the lit map `petals` (one flag per
    /// field cell): the first ``carriersPerSector`` lit cells in
    /// ``carrierOrder(_:)``, or `nil` when fewer are lit (or the sector or the
    /// map is out of shape).
    public static func carriers(_ petals: [Bool], sector: Int) -> [Int]? {
        guard (0..<PetalLayout.sectors).contains(sector), petals.count >= PetalLayout.fieldCells else {
            return nil
        }
        var chosen: [Int] = []
        chosen.reserveCapacity(carriersPerSector)
        for cell in carrierOrders[sector] where petals[cell] {
            chosen.append(cell)
            if chosen.count == carriersPerSector { return chosen }
        }
        return nil
    }

    /// Fountain id of the first of the ``atomCount`` atoms the turn lane of
    /// `sector` carries in `frame`: `2³¹ + 16 · frame + 4 · sector`.
    public static func turnFirstID(frame: UInt16, sector: Int) -> UInt32 {
        idBase
            &+ UInt32(frame) &* UInt32(PetalLayout.sectors * atomCount)
            &+ UInt32(truncatingIfNeeded: sector) &* UInt32(atomCount)
    }

    /// The check of a turn lane: CRC-32C over the stream tag, the frame
    /// (big-endian), the sector and the atoms, so a turn lane read under the
    /// wrong field lane, frame or stream is refused.
    public static func turnCheck(_ header: PetalLaneHeader, sector: Int, atoms: [UInt8]) -> UInt32 {
        var bytes: [UInt8] = [
            header.tag,
            UInt8(truncatingIfNeeded: header.frame >> 8),
            UInt8(truncatingIfNeeded: header.frame),
            UInt8(truncatingIfNeeded: sector),
        ]
        bytes.append(contentsOf: atoms)
        return IrohaPeerCRC32CV1.checksum(bytes)
    }

    /// The data of a turn lane: the atoms followed by their
    /// ``turnCheck(_:sector:atoms:)`` (big-endian).
    ///
    /// - Throws: ``PetalCodecError/invalidShape`` unless `atoms` holds exactly
    ///   ``atomCount`` atoms.
    public static func turnData(_ header: PetalLaneHeader, sector: Int, atoms: [UInt8]) throws -> [UInt8] {
        guard atoms.count == atomCount * PetalLane.atomLength else { throw PetalCodecError.invalidShape }
        return turnDataValidated(header, sector: sector, atoms: atoms)
    }

    static func turnDataValidated(_ header: PetalLaneHeader, sector: Int, atoms: [UInt8]) -> [UInt8] {
        let check = turnCheck(header, sector: sector, atoms: atoms)
        var data = atoms
        data.reserveCapacity(dataLength)
        data.append(UInt8(truncatingIfNeeded: check >> 24))
        data.append(UInt8(truncatingIfNeeded: check >> 16))
        data.append(UInt8(truncatingIfNeeded: check >> 8))
        data.append(UInt8(truncatingIfNeeded: check))
        return data
    }

    /// The atoms of turn lane data whose check matches `header` and `sector`;
    /// `nil` for a length mismatch, a sector out of range or a wrong check.
    public static func turnPacket(_ header: PetalLaneHeader, sector: Int, data: [UInt8]) -> PetalAtomPacket? {
        guard data.count == dataLength, (0..<PetalLayout.sectors).contains(sector) else { return nil }
        let split = atomCount * PetalLane.atomLength
        let atoms = Array(data[..<split])
        let check = turnCheck(header, sector: sector, atoms: atoms)
        guard data[split] == UInt8(truncatingIfNeeded: check >> 24),
              data[split + 1] == UInt8(truncatingIfNeeded: check >> 16),
              data[split + 2] == UInt8(truncatingIfNeeded: check >> 8),
              data[split + 3] == UInt8(truncatingIfNeeded: check) else { return nil }
        return PetalAtomPacket(
            header: header,
            firstID: turnFirstID(frame: header.frame, sector: sector),
            atoms: (0..<atomCount).map { index in
                Array(atoms[(index * PetalLane.atomLength)..<((index + 1) * PetalLane.atomLength)])
            }
        )
    }

    /// The whitening sequence of the turn lane of `sector`: one byte per
    /// codeword byte from a ``PetalXorshift32`` seeded `0x5452_4E30 + sector`
    /// ("TRN0" … "TRN3").
    public static func turnWhitening(_ sector: Int) -> [UInt8] {
        (0..<whitenings.count).contains(sector) ? whitenings[sector] : makeWhitening(sector)
    }

    private static let whitenings = (0..<PetalLayout.sectors).map(makeWhitening)

    private static func makeWhitening(_ sector: Int) -> [UInt8] {
        var rng = PetalXorshift32(seed: 0x5452_4E30 &+ UInt32(truncatingIfNeeded: sector))
        return (0..<wordLength).map { _ in rng.nextByte() }
    }

    private static let code = PetalReedSolomon(validatedParityLength: parityLength)

    /// Encodes turn lane data into the transmitted (whitened) codeword.
    ///
    /// - Throws: ``PetalCodecError/invalidShape`` unless `data` is exactly
    ///   ``dataLength`` bytes.
    public static func encodeTurnLane(sector: Int, data: [UInt8]) throws -> [UInt8] {
        guard data.count == dataLength else { throw PetalCodecError.invalidShape }
        return encodeValidated(sector: sector, data: data)
    }

    static func encodeValidated(sector: Int, data: [UInt8]) -> [UInt8] {
        var word = code.encodeValidated(data)
        let mask = turnWhitening(sector)
        for index in word.indices { word[index] ^= mask[index] }
        return word
    }

    /// Decodes a transmitted turn codeword, returning its data and how many
    /// byte positions the Reed–Solomon decoder rewrote. The check is not
    /// verified here (see ``turnPacket(_:sector:data:)``).
    ///
    /// - Throws: ``PetalCodecError`` when the word has the wrong length or is
    ///   uncorrectable.
    public static func decodeTurnLaneCounted(
        sector: Int,
        _ transmitted: [UInt8],
        erasures: [Int] = []
    ) throws -> (data: [UInt8], corrected: Int) {
        guard transmitted.count == wordLength else { throw PetalCodecError.invalidShape }
        let mask = turnWhitening(sector)
        var word = transmitted
        for index in word.indices { word[index] ^= mask[index] }
        let corrected = try code.decode(&word, erasures: erasures)
        return (Array(word.prefix(dataLength)), corrected)
    }

    /// The turn position carrier `index` of a transmitted turn codeword shows.
    public static func carrierPosition(_ word: [UInt8], index: Int) -> UInt8 {
        turnPosition((word[index / 4] >> (6 - 2 * (index % 4))) & 3)
    }

    /// Sine of every petal's axis (its outward direction plus turn position
    /// `p`) at `p * fieldCells + cell`, computed as the reference renderer
    /// computes it.
    static let axisSin: [Double] = axes.sin
    /// Cosine of every petal's axis, laid out like ``axisSin``.
    static let axisCos: [Double] = axes.cos

    private static let axes: (sin: [Double], cos: [Double]) = {
        var sines = [Double](repeating: 0, count: positions * PetalLayout.fieldCells)
        var cosines = sines
        for position in 0..<positions {
            for cell in 0..<PetalLayout.fieldCells {
                let angle = PetalLayout.cellAngles[cell] + turnAngle(UInt8(position))
                let (s, c) = PetalNumeric.sinCos(angle)
                sines[position * PetalLayout.fieldCells + cell] = s
                cosines[position * PetalLayout.fieldCells + cell] = c
            }
        }
        return (sines, cosines)
    }()

    // MARK: - Reading

    /// Half the side, in canvas units, of the square a petal template is
    /// tabulated over.
    static let templateHalf = 9.0
    /// Template table step in canvas units.
    static let templateStep = 0.25
    /// Template table side in steps.
    static let templateSide = 73
    /// From this fitted blur on (3.2 units), the camera cannot resolve the
    /// turns and the reader stops.
    static let blurLimit = 4
    /// The window of a carrier is sampled every canvas unit over
    /// ±``gridHalf`` units in its own frame.
    static let gridHalf = 6
    /// Window side in samples.
    static let gridSide = 13
    /// Samples of a window.
    static let gridPoints = gridSide * gridSide
    /// σ in canvas units of the Gaussian that weights the window.
    static let windowSigma = 3.6
    /// σ in canvas units of the Gaussian that follows a carrier's centroid.
    static let centroidSigma = 2.6
    /// Lit petals within this distance of a carrier are modelled as its
    /// neighbours.
    static let neighbourDistance = 19.0
    /// A neighbour's template is subtracted within this distance of its centre.
    /// Radially adjacent cell centres are 11 units apart and the window grid is
    /// integral, so a whole number here would put grid points exactly on the
    /// boundary, where the last bit of a platform's `sin` and `cos` decides.
    static let neighbourReach = 9.5
    /// The warp and the blur are fitted on every this-many-th carrier.
    static let sampleStep = 17

    /// The turn reader's blurred petal template for blur `blur` (an index into
    /// ``blurs``) at a point of the unturned petal's own frame (`along` its
    /// axis, `across` it): what conformance tests compare an SDK's tables with.
    /// Zero for an index outside ``blurs``.
    public static func turnTemplate(blur: Int, along: Double, across: Double) -> Double {
        guard blurs.indices.contains(blur) else { return 0 }
        return Tables.at(Tables.shared.templates + blur * Tables.templateCells, along, across)
    }

    /// The window grid, along the outward direction fastest; per blur the
    /// template and the weighted kernel of every turn position on the grid;
    /// and every field cell's neighbours. Built once, on first use, and kept
    /// for the life of the process (about 0.5 MB).
    final class Tables: @unchecked Sendable {
        static let shared = Tables()
        /// Cells of one template table.
        static let templateCells = templateSide * templateSide

        /// `a` (along) of every grid point.
        let gridA: UnsafeMutablePointer<Double>
        /// `b` (across) of every grid point.
        let gridB: UnsafeMutablePointer<Double>
        /// The window Gaussian at every grid point.
        let gridWeight: UnsafeMutablePointer<Double>
        /// The template of every blur, ``templateCells`` values each, row `j`
        /// (across) by column `i` (along).
        let templates: UnsafeMutablePointer<Double>
        /// Per blur, the kernels of the four turn positions on the grid, at
        /// `(blur * positions + position) * gridPoints + point`.
        let kernels: UnsafeMutablePointer<Double>
        /// Sine and cosine of every turn angle.
        let turnSin: UnsafeMutablePointer<Double>
        let turnCos: UnsafeMutablePointer<Double>
        /// The cells within ``neighbourDistance`` of every field cell (itself
        /// excluded), in index order: entries `nearStart[c]..<nearStart[c + 1]`
        /// of `nearCell`, with their centres in cell `c`'s outward frame
        /// (`nearAlong`, `nearAcross`) and the angle of their outward direction
        /// from cell `c`'s (`nearAngle`).
        let nearStart: UnsafeMutablePointer<Int>
        let nearCount: Int
        let nearCell: UnsafeMutablePointer<Int32>
        let nearAlong: UnsafeMutablePointer<Double>
        let nearAcross: UnsafeMutablePointer<Double>
        let nearAngle: UnsafeMutablePointer<Double>

        private static func table<T>(_ values: [T]) -> UnsafeMutablePointer<T> {
            let table = UnsafeMutablePointer<T>.allocate(capacity: max(values.count, 1))
            _ = UnsafeMutableBufferPointer(start: table, count: values.count).initialize(from: values)
            return table
        }

        private init() {
            var gridA: [Double] = []
            var gridB: [Double] = []
            var gridWeight: [Double] = []
            for b in -gridHalf...gridHalf {
                for a in -gridHalf...gridHalf {
                    let (a, b) = (Double(a), Double(b))
                    gridA.append(a)
                    gridB.append(b)
                    gridWeight.append(exp(-(a * a + b * b) / (2.0 * windowSigma * windowSigma)))
                }
            }
            let side = templateSide
            // the coverage of every table cell by the unturned petal (4 × 4 samples per cell)
            var cover = [Double](repeating: 0, count: side * side)
            for j in 0..<side {
                for i in 0..<side {
                    let along = -templateHalf + Double(i) * templateStep
                    let across = -templateHalf + Double(j) * templateStep
                    var hits = 0
                    for sj in 0..<4 {
                        for si in 0..<4 {
                            let da = (Double(si) - 1.5) * templateStep / 4.0
                            let dc = (Double(sj) - 1.5) * templateStep / 4.0
                            if PetalLayout.petalLit(dx: along + da, dy: across + dc, angle: 0.0) { hits += 1 }
                        }
                    }
                    cover[j * side + i] = Double(hits) / 16.0
                }
            }
            var templates: [Double] = []
            templates.reserveCapacity(blurs.count * Self.templateCells)
            for sigma in blurs { templates += Self.blurred(cover, sigma: sigma) }
            var turnSin: [Double] = []
            var turnCos: [Double] = []
            for position in 0..<positions {
                let (s, c) = PetalNumeric.sinCos(turnAngle(UInt8(position)))
                turnSin.append(s)
                turnCos.append(c)
            }
            self.templates = Self.table(templates)
            var kernels = [Double](repeating: 0, count: blurs.count * positions * gridPoints)
            for blur in blurs.indices {
                let template = self.templates + blur * Self.templateCells
                for position in 0..<positions {
                    let (s, c) = (turnSin[position], turnCos[position])
                    for point in 0..<gridPoints {
                        let (a, b) = (gridA[point], gridB[point])
                        kernels[(blur * positions + position) * gridPoints + point] =
                            gridWeight[point] * Self.at(template, a * c + b * s, -a * s + b * c)
                    }
                }
            }
            // neighbours: bands are 11 units apart, so only the two bands on either side hold them
            let xs = PetalLayout.cellCenterX
            let ys = PetalLayout.cellCenterY
            var nearStart = [0]
            nearStart.reserveCapacity(PetalLayout.fieldCells + 1)
            var nearCell: [Int32] = []
            var nearAlong: [Double] = []
            var nearAcross: [Double] = []
            var nearAngle: [Double] = []
            let limit = neighbourDistance * neighbourDistance
            for cell in 0..<PetalLayout.fieldCells {
                let (x, y) = (xs[cell], ys[cell])
                let angle = PetalLayout.cellAngles[cell]
                let (s, c) = (PetalLayout.cellSin[cell], PetalLayout.cellCos[cell])
                let band = Int(PetalLayout.cellBands[cell])
                let first = PetalLayout.bandOffsets[max(band - 2, 0)]
                let last = PetalLayout.bandOffsets[min(band + 3, PetalLayout.bandCount)]
                for other in first..<last where other != cell {
                    let dx = xs[other] - x
                    let dy = ys[other] - y
                    if dx * dx + dy * dy <= limit {
                        nearCell.append(Int32(other))
                        nearAlong.append(dx * c + dy * s)
                        nearAcross.append(-dx * s + dy * c)
                        nearAngle.append(PetalLayout.cellAngles[other] - angle)
                    }
                }
                nearStart.append(nearCell.count)
            }
            self.gridA = Self.table(gridA)
            self.gridB = Self.table(gridB)
            self.gridWeight = Self.table(gridWeight)
            self.kernels = Self.table(kernels)
            self.turnSin = Self.table(turnSin)
            self.turnCos = Self.table(turnCos)
            self.nearStart = Self.table(nearStart)
            nearCount = nearCell.count
            self.nearCell = Self.table(nearCell)
            self.nearAlong = Self.table(nearAlong)
            self.nearAcross = Self.table(nearAcross)
            self.nearAngle = Self.table(nearAngle)
        }

        /// The coverage blurred by a Gaussian of `sigma` canvas units truncated
        /// at `3σ`: rows, then columns; taps outside the table count as zero,
        /// and the kernel is normalised by its full sum.
        private static func blurred(_ cover: [Double], sigma: Double) -> [Double] {
            let side = templateSide
            let radius = Int((3.0 * sigma / templateStep).rounded(.up))
            var kernel: [Double] = []
            for k in -radius...radius {
                let t = Double(k) * templateStep
                kernel.append(exp(-(t * t) / (2.0 * sigma * sigma)))
            }
            var norm = -0.0
            for weight in kernel { norm += weight }
            func pass(_ source: [Double], rows: Bool) -> [Double] {
                var out = [Double](repeating: 0, count: side * side)
                for j in 0..<side {
                    for i in 0..<side {
                        var sum = 0.0
                        for (t, weight) in kernel.enumerated() {
                            let k = t - radius
                            let (ii, jj) = rows ? (i + k, j) : (i, j + k)
                            if ii >= 0, jj >= 0, ii < side, jj < side {
                                sum += weight * source[jj * side + ii]
                            }
                        }
                        out[j * side + i] = sum / norm
                    }
                }
                return out
            }
            return pass(pass(cover, rows: true), rows: false)
        }

        /// A template at a point of the petal's own frame (bilinear; zero
        /// outside the table).
        @inline(__always)
        static func at(_ table: UnsafePointer<Double>, _ along: Double, _ across: Double) -> Double {
            let x = (along + templateHalf) / templateStep
            let y = (across + templateHalf) / templateStep
            let last = Double(templateSide - 1)
            if !(x >= 0.0 && y >= 0.0 && x < last && y < last) { return 0.0 }
            let i = Int(x)
            let j = Int(y)
            let fx = x - Double(i)
            let fy = y - Double(j)
            let row = j * templateSide + i
            let top = table[row] * (1.0 - fx) + table[row + 1] * fx
            let bottom = table[row + templateSide] * (1.0 - fx) + table[row + templateSide + 1] * fx
            return top * (1.0 - fy) + bottom * fy
        }
    }

    /// Reads the turn lanes of the sectors `frame` decoded from `image` (the
    /// capture `frame` was decoded from), except those flagged in `skip`.
    ///
    /// The carriers of every decoded sector are decided (so that each
    /// carrier's neighbours are modelled at their own turns), but only the
    /// lanes of the sectors not skipped are decoded, and none when the image
    /// is too blurred to show turns. A lane passes when its Reed–Solomon
    /// codeword decodes, with 0, 2, 5, 7 or 11 of its least confident bytes
    /// erased (a byte's confidence is the smallest lead of its four carriers'
    /// turns), into data whose ``turnCheck(_:sector:atoms:)`` matches its
    /// sector's field lane header. Returns the lane data (atoms and check) per
    /// sector, sector 0 first (always four entries). A short `skip` list skips
    /// nothing beyond it.
    public static func readTurns(
        _ image: PetalLuma,
        frame: PetalDecodedFrame,
        skip: [Bool] = [false, false, false, false]
    ) -> [PetalLaneResult?] {
        let skip = (0..<PetalLayout.sectors).map { $0 < skip.count && skip[$0] }
        var results = [PetalLaneResult?](repeating: nil, count: PetalLayout.sectors)
        Workspace.lease { workspace in
            guard let reading = image.withView({ view in
                decideTurns(view, frame: frame, need: skip.map { !$0 }, limit: true, workspace)
            }) else { return }
            for sector in 0..<PetalLayout.sectors where !skip[sector] {
                guard let range = reading.ranges[sector], let header = reading.headers[sector] else { continue }
                var word = [UInt8](repeating: 0, count: wordLength)
                var confidence = [Double](repeating: .greatestFiniteMagnitude, count: wordLength)
                for (j, carrier) in range.enumerated() {
                    word[j / 4] |= turnLabel(workspace.positions[carrier]) << (6 - 2 * (j % 4))
                    confidence[j / 4] = PetalNumeric.minNum(confidence[j / 4], workspace.leads[carrier])
                }
                results[sector] = decodeTurnWord(sector: sector, header: header, word: word, confidence: confidence)
            }
        }
        return results
    }

    /// What ``readTurns(_:frame:skip:)`` decides for every carrier of the
    /// sectors `frame` decoded, before any Reed–Solomon correction and whatever
    /// the blur; `nil` when no decoded sector has carriers.
    public static func observedTurns(_ image: PetalLuma, frame: PetalDecodedFrame) -> PetalObservedTurns? {
        var observed: PetalObservedTurns?
        Workspace.lease { workspace in
            guard let reading = image.withView({ view in
                decideTurns(view, frame: frame, need: [true, true, true, true], limit: false, workspace)
            }) else { return }
            observed = PetalObservedTurns(
                blur: blurs[reading.fitted],
                carriers: (0..<reading.count).map { index in
                    PetalObservedTurns.Carrier(
                        cell: workspace.cells[index],
                        position: workspace.positions[index],
                        lead: workspace.leads[index]
                    )
                }
            )
        }
        return observed
    }

    /// Tries Reed–Solomon with 0, 2, 5, 7 and 11 of the least confident bytes
    /// erased (ties keep byte order) and accepts the first result whose check
    /// matches.
    static func decodeTurnWord(
        sector: Int,
        header: PetalLaneHeader,
        word: [UInt8],
        confidence: [Double]
    ) -> PetalLaneResult? {
        let keys = confidence.map(PetalNumeric.totalOrderKey)
        // Stable ascending sort under total order (Rust `sort_by`).
        let order = word.indices.sorted { a, b in
            keys[a] != keys[b] ? keys[a] < keys[b] : a < b
        }
        let nsym = parityLength
        // Rust `Vec::dedup`: drop consecutive repeats.
        var schedule: [Int] = []
        for erasures in [0, nsym / 8, nsym / 4, nsym / 3, nsym / 2] where schedule.last != erasures {
            schedule.append(erasures)
        }
        for erasures in schedule {
            let positions = Array(order.prefix(erasures))
            var trial = word
            // zero the erased bytes so stale values cannot leak through
            for position in positions { trial[position] = 0 }
            guard let (data, corrected) = try? decodeTurnLaneCounted(sector: sector, trial, erasures: positions) else {
                continue
            }
            if turnPacket(header, sector: sector, data: data) != nil {
                return PetalLaneResult(data: data, corrected: corrected, erasures: positions.count)
            }
        }
        return nil
    }

    // MARK: - The reader

    /// What the reader decided for one capture; the carriers, their turn
    /// positions and leads stay in the workspace.
    struct Reading {
        let headers: [PetalLaneHeader?]
        /// The carriers of each sector, as indices into the workspace's carriers.
        let ranges: [Range<Int>?]
        /// Carriers of every decoded sector.
        let count: Int
        /// Index into ``blurs`` of the blur that fits best.
        let fitted: Int
    }

    /// Working memory of the reader, reused across captures so that a read
    /// allocates next to nothing: the windows of up to 1,440 carriers (about
    /// 2 MB) and the per-cell maps.
    final class Workspace {
        /// Carriers of four sectors at most.
        static let capacity = PetalLayout.sectors * carriersPerSector
        /// Carriers the warp is fitted on, at most.
        static let sampleCapacity = (capacity + sampleStep - 1) / sampleStep

        /// Whether each field cell is lit, in the sectors that decoded.
        let lit = UnsafeMutablePointer<Bool>.allocate(capacity: PetalLayout.fieldCells)
        /// The carrier index of every field cell, -1 for cells that carry nothing.
        let carrierOf = UnsafeMutablePointer<Int32>.allocate(capacity: PetalLayout.fieldCells)
        /// The cell of every carrier, sector by sector in carrier order.
        let cells = UnsafeMutablePointer<Int>.allocate(capacity: capacity)
        /// The normalised window of every carrier, ``gridPoints`` values each.
        let windows = UnsafeMutablePointer<Double>.allocate(capacity: capacity * gridPoints)
        /// The turn position and lead decided for every carrier.
        let positions = UnsafeMutablePointer<UInt8>.allocate(capacity: capacity)
        let leads = UnsafeMutablePointer<Double>.allocate(capacity: capacity)
        /// Per carrier, its best turn position under the blur being fitted
        /// (-1 until decided).
        let turnOf = UnsafeMutablePointer<Int8>.allocate(capacity: capacity)
        /// One window.
        let scratch = UnsafeMutablePointer<Double>.allocate(capacity: gridPoints)
        /// The modelled window of one carrier.
        let model = UnsafeMutablePointer<Double>.allocate(capacity: gridPoints)
        /// The carriers the warp is fitted on: centre `x`, `y` and offsets
        /// `x`, `y`, ``sampleCapacity`` values each.
        let samples = UnsafeMutablePointer<Double>.allocate(capacity: 4 * sampleCapacity)

        init() {
            lit.initialize(repeating: false, count: PetalLayout.fieldCells)
            carrierOf.initialize(repeating: -1, count: PetalLayout.fieldCells)
            cells.initialize(repeating: 0, count: Self.capacity)
            windows.initialize(repeating: 0, count: Self.capacity * gridPoints)
            positions.initialize(repeating: 0, count: Self.capacity)
            leads.initialize(repeating: 0, count: Self.capacity)
            turnOf.initialize(repeating: -1, count: Self.capacity)
            scratch.initialize(repeating: 0, count: gridPoints)
            model.initialize(repeating: 0, count: gridPoints)
            samples.initialize(repeating: 0, count: 4 * Self.sampleCapacity)
        }

        deinit {
            lit.deallocate()
            carrierOf.deallocate()
            cells.deallocate()
            windows.deallocate()
            positions.deallocate()
            leads.deallocate()
            turnOf.deallocate()
            scratch.deallocate()
            model.deallocate()
            samples.deallocate()
        }

        private static let lock = NSLock()
        private static var idle: [Workspace] = []

        /// Runs `body` with a workspace no other thread uses: an idle one, or a
        /// new one when every workspace is busy. At most two are kept.
        static func lease(_ body: (Workspace) -> Void) {
            lock.lock()
            let workspace = idle.popLast() ?? Workspace()
            lock.unlock()
            body(workspace)
            lock.lock()
            if idle.count < 2 { idle.append(workspace) }
            lock.unlock()
        }
    }

    /// Decides the turn of every carrier of the sectors `frame` decoded, when
    /// a sector that `need` names has carriers; with `limit`, not when the
    /// fitted blur reaches ``blurLimit``.
    ///
    /// 1. The lit petals of the decoded sectors come from their field lanes,
    ///    and the carriers of each from ``carriers(_:sector:)``.
    /// 2. The pose's smooth error: for every carrier in steps of
    ///    ``sampleStep``, the window is sampled at its cell centre and
    ///    normalised, and its centroid turned into a canvas offset; a cubic
    ///    fitted to the offsets over the canvas moves every carrier's window,
    ///    which is then sampled and normalised.
    /// 3. The blur: for every carrier in steps of ``sampleStep``, the model of
    ///    the petal at its best turn plus its lit neighbours (within
    ///    ``neighbourDistance``) at theirs (each decided on its own window,
    ///    unturned when it carries nothing) is correlated with the window,
    ///    weighted by the window Gaussian; the blur with the largest sum fits
    ///    best.
    /// 4. The turns: with the next wider blur's kernels, every carrier takes
    ///    the best turn of its own window, then, carrier by carrier in order,
    ///    the best turn of its window with its lit neighbours' templates at
    ///    their latest turns subtracted.
    static func decideTurns(
        _ image: PetalLumaView,
        frame: PetalDecodedFrame,
        need: [Bool],
        limit: Bool,
        _ workspace: Workspace
    ) -> Reading? {
        let sectors = PetalLayout.sectors
        let fieldLength = PetalLane.field(0).dataLength
        let lit = workspace.lit
        let carrierOf = workspace.carrierOf
        let cells = workspace.cells
        lit.update(repeating: false, count: PetalLayout.fieldCells)
        carrierOf.update(repeating: -1, count: PetalLayout.fieldCells)
        // the lit petals of every sector that decoded
        var headers = [PetalLaneHeader?](repeating: nil, count: sectors)
        for sector in 0..<min(sectors, frame.fields.count) {
            // a caller-built frame may carry lane data of another length: it names no lit petals
            guard let field = frame.fields[sector], field.data.count == fieldLength else { continue }
            headers[sector] = PetalStream.parseHeader(field.data)
            let word = PetalLane.field(sector).encodeValidated(field.data)
            let bits = PetalLayout.sectorTable[sector]
            for bit in 0..<codewordCells {
                lit[bits[bit]] = (word[bit / 8] >> (7 - bit % 8)) & 1 == 1
            }
        }
        // the carriers of every decoded sector, sector by sector in carrier order: the first
        // `carriersPerSector` lit cells, or none
        var ranges = [Range<Int>?](repeating: nil, count: sectors)
        var count = 0
        for sector in 0..<sectors where headers[sector] != nil {
            let start = count
            for cell in carrierOrders[sector] where lit[cell] {
                cells[count] = cell
                count += 1
                if count - start == carriersPerSector { break }
            }
            guard count - start == carriersPerSector else {
                count = start
                continue
            }
            for index in start..<count { carrierOf[cells[index]] = Int32(index) }
            ranges[sector] = start..<count
        }
        if (0..<sectors).allSatisfy({ !need[$0] || ranges[$0] == nil }) { return nil }
        guard let fitted = workspace.decide(image, frame.pose, count: count, limit: limit) else { return nil }
        return Reading(headers: headers, ranges: ranges, count: count, fitted: fitted)
    }

    /// The best turn position for a window and how far its score leads the
    /// runner-up's (ties: the lower position). The four scores are summed in
    /// grid order, each in its own accumulator.
    @inline(__always)
    static func decide(_ window: UnsafePointer<Double>, _ kernels: UnsafePointer<Double>) -> (position: UInt8, lead: Double) {
        let k1 = kernels + gridPoints
        let k2 = kernels + 2 * gridPoints
        let k3 = kernels + 3 * gridPoints
        var s0 = -0.0
        var s1 = -0.0
        var s2 = -0.0
        var s3 = -0.0
        for point in 0..<gridPoints {
            let x = window[point]
            s0 += x * kernels[point]
            s1 += x * k1[point]
            s2 += x * k2[point]
            s3 += x * k3[point]
        }
        var best = 0
        var bestScore = s0
        if s1 > bestScore { best = 1; bestScore = s1 }
        if s2 > bestScore { best = 2; bestScore = s2 }
        if s3 > bestScore { best = 3; bestScore = s3 }
        // the runner-up: the largest other score, folded in position order
        var runnerUp = -Double.infinity
        if best != 0 { runnerUp = PetalNumeric.maxNum(runnerUp, s0) }
        if best != 1 { runnerUp = PetalNumeric.maxNum(runnerUp, s1) }
        if best != 2 { runnerUp = PetalNumeric.maxNum(runnerUp, s2) }
        if best != 3 { runnerUp = PetalNumeric.maxNum(runnerUp, s3) }
        return (UInt8(best), bestScore - runnerUp)
    }

    /// Samples the image on the window grid of the petal at `cell`, centred
    /// `shift` canvas units off the cell centre, in the petal's outward frame
    /// (grid rows across, columns along). Within a window the pose is taken as
    /// affine: with `o` the image point of the centre and `u`, `v` the image
    /// steps of one unit along and across, grid point `(a, b)` is sampled at
    /// `(o + b·v) + a·u`.
    @inline(__always)
    static func sampleWindow(
        _ image: PetalLumaView,
        _ pose: PetalPose,
        cell: Int,
        shiftX: Double,
        shiftY: Double,
        into out: UnsafeMutablePointer<Double>
    ) {
        let x = PetalLayout.cellCenterX[cell] + shiftX
        let y = PetalLayout.cellCenterY[cell] + shiftY
        let s = PetalLayout.cellSin[cell]
        let c = PetalLayout.cellCos[cell]
        let origin = pose.apply(x, y)
        let along = pose.apply(x + c, y + s)
        let across = pose.apply(x - s, y + c)
        let u = (along.0 - origin.0, along.1 - origin.1)
        let v = (across.0 - origin.0, across.1 - origin.1)
        var index = 0
        for b in -gridHalf...gridHalf {
            let b = Double(b)
            let rowX = origin.0 + b * v.0
            let rowY = origin.1 + b * v.1
            for a in -gridHalf...gridHalf {
                let a = Double(a)
                out[index] = image.sample(rowX + a * u.0, rowY + a * u.1)
                index += 1
            }
        }
    }

    /// Rescales a window so that its 10th percentile (of 169 samples, the
    /// 17th smallest) becomes 0 and its 98th (the 166th smallest) 1 (a span
    /// below one grey level counts as one), clamped to −0.2 … 1.2. The two
    /// order statistics are those of IEEE total order, as a selection or a
    /// sort finds them; here two short sorted lists collect the 17 smallest
    /// and the 4 largest samples.
    @inline(__always)
    static func normalise(_ window: UnsafeMutablePointer<Double>) {
        let n = gridPoints
        let lowCount = n / 10 + 1
        let highCount = n - n * 98 / 100
        withUnsafeTemporaryAllocation(of: Int64.self, capacity: lowCount + highCount) { keys in
            let low = keys.baseAddress!
            let high = low + lowCount
            var lows = 0
            var highs = 0
            for index in 0..<n {
                let key = PetalNumeric.totalOrderKey(window[index])
                if lows < lowCount || key < low[lowCount - 1] {
                    var slot = lows < lowCount ? lows : lowCount - 1
                    while slot > 0 && low[slot - 1] > key {
                        low[slot] = low[slot - 1]
                        slot -= 1
                    }
                    low[slot] = key
                    if lows < lowCount { lows += 1 }
                }
                if highs < highCount {
                    var slot = highs
                    while slot > 0 && high[slot - 1] > key {
                        high[slot] = high[slot - 1]
                        slot -= 1
                    }
                    high[slot] = key
                    highs += 1
                } else if key > high[0] {
                    var slot = 0
                    while slot + 1 < highCount && high[slot + 1] < key {
                        high[slot] = high[slot + 1]
                        slot += 1
                    }
                    high[slot] = key
                }
            }
            let bottom = PetalNumeric.totalOrderValue(low[lowCount - 1])
            let top = PetalNumeric.totalOrderValue(high[0])
            let span = PetalNumeric.maxNum(top - bottom, 1.0)
            for index in 0..<n {
                window[index] = PetalNumeric.clamp((window[index] - bottom) / span, -0.2, 1.2)
            }
        }
    }

    /// The centroid (along, across) of the light of a normalised window: every
    /// sample weighs its level (clamped to 0 … 1) times a Gaussian of σ
    /// ``centroidSigma`` that starts at the window centre and follows the
    /// centroid three times; the Gaussian is its along factor times its across
    /// factor.
    static func centroid(_ window: UnsafePointer<Double>) -> (along: Double, across: Double) {
        var ea = 0.0
        var eb = 0.0
        let k = 2.0 * centroidSigma * centroidSigma
        return withUnsafeTemporaryAllocation(of: Double.self, capacity: 2 * gridSide) { factors in
            let along = factors.baseAddress!
            let across = along + gridSide
            for _ in 0..<3 {
                for i in 0..<gridSide {
                    let t = Double(i - gridHalf)
                    along[i] = exp(-(t - ea) * (t - ea) / k)
                    across[i] = exp(-(t - eb) * (t - eb) / k)
                }
                var sum = 0.0
                var sa = 0.0
                var sb = 0.0
                for j in 0..<gridSide {
                    let b = Double(j - gridHalf)
                    for i in 0..<gridSide {
                        let a = Double(i - gridHalf)
                        let w = PetalNumeric.clamp(window[j * gridSide + i], 0.0, 1.0) * along[i] * across[j]
                        sum += w
                        sa += w * a
                        sb += w * b
                    }
                }
                if sum <= 0.0 { break }
                ea = sa / sum
                eb = sb / sum
            }
            return (ea, eb)
        }
    }

    /// The ten cubic monomials of a canvas point, centred on the canvas and
    /// scaled by 384.
    @inline(__always)
    static func monomials(_ x: Double, _ y: Double, into f: UnsafeMutablePointer<Double>) {
        let x = (x - 512.0) / 384.0
        let y = (y - 512.0) / 384.0
        f[0] = 1.0
        f[1] = x
        f[2] = y
        f[3] = x * x
        f[4] = x * y
        f[5] = y * y
        f[6] = x * x * x
        f[7] = x * x * y
        f[8] = x * y * y
        f[9] = y * y * y
    }

    /// The least-squares cubic through `values` at the `count` points
    /// `(xs, ys)`: normal equations (summed point by point) with 10⁻⁶ added to
    /// the diagonal, solved by Gauss–Jordan elimination with partial pivoting
    /// (a pivot below 10⁻¹² skips its column). Writes the ten coefficients to
    /// `coefficients`.
    static func fitCubic(
        _ xs: UnsafePointer<Double>,
        _ ys: UnsafePointer<Double>,
        _ values: UnsafePointer<Double>,
        count: Int,
        into coefficients: UnsafeMutablePointer<Double>
    ) {
        let width = 11
        withUnsafeTemporaryAllocation(of: Double.self, capacity: 10 * width + 2 * width) { buffer in
            let a = buffer.baseAddress!
            let f = a + 10 * width
            let pivotRow = f + width
            a.update(repeating: 0.0, count: 10 * width)
            for point in 0..<count {
                monomials(xs[point], ys[point], into: f)
                for r in 0..<10 {
                    for c in 0..<10 { a[r * width + c] += f[r] * f[c] }
                    a[r * width + 10] += f[r] * values[point]
                }
            }
            for r in 0..<10 { a[r * width + r] += 1e-6 }
            for col in 0..<10 {
                var pivot = col
                for r in (col + 1)..<10 where abs(a[r * width + col]) > abs(a[pivot * width + col]) {
                    pivot = r
                }
                if pivot != col {
                    for c in 0..<width {
                        let value = a[col * width + c]
                        a[col * width + c] = a[pivot * width + c]
                        a[pivot * width + c] = value
                    }
                }
                let p = a[col * width + col]
                if abs(p) < 1e-12 { continue }
                for c in col..<width { a[col * width + c] /= p }
                for c in col..<width { pivotRow[c] = a[col * width + c] }
                for r in 0..<10 where r != col {
                    let factor = a[r * width + col]
                    for c in col..<width { a[r * width + c] -= factor * pivotRow[c] }
                }
            }
            for r in 0..<10 { coefficients[r] = a[r * width + 10] }
        }
    }

    /// The cubic with `coefficients` at a canvas point.
    @inline(__always)
    static func evalCubic(_ coefficients: UnsafePointer<Double>, _ x: Double, _ y: Double) -> Double {
        withUnsafeTemporaryAllocation(of: Double.self, capacity: 10) { f in
            monomials(x, y, into: f.baseAddress!)
            var sum = -0.0
            for index in 0..<10 { sum += f[index] * coefficients[index] }
            return sum
        }
    }

    /// Adds `sign ×` a neighbour's template, turned by `turn`, to a window, at
    /// the grid points within ``neighbourReach`` of the neighbour's centre:
    /// `along`, `across` and its outward angle `angle` in the window's frame.
    @inline(__always)
    static func addNeighbour(
        _ window: UnsafeMutablePointer<Double>,
        _ template: UnsafePointer<Double>,
        along: Double,
        across: Double,
        angle: Double,
        turn: Double,
        sign: Double
    ) {
        let (s, c) = PetalNumeric.sinCos(angle + turn)
        let reach2 = neighbourReach * neighbourReach
        // only the grid rows and columns within reach of the neighbour's centre
        func span(_ centre: Double) -> (Int, Int) {
            let low = Double.maximum((centre - neighbourReach).rounded(.up), -Double(gridHalf))
            let high = Double.minimum((centre + neighbourReach).rounded(.down), Double(gridHalf))
            return (Int(low), Int(high))
        }
        let (a0, a1) = span(along)
        let (b0, b1) = span(across)
        guard a0 <= a1, b0 <= b1 else { return }
        for b in b0...b1 {
            let ub = Double(b) - across
            let row = (b + gridHalf) * gridSide
            for a in a0...a1 {
                let ua = Double(a) - along
                if ua * ua + ub * ub > reach2 { continue }
                window[row + a + gridHalf] += sign * Tables.at(template, ua * c + ub * s, -ua * s + ub * c)
            }
        }
    }
}

extension PetalTurns.Workspace {
    /// Samples, warps and normalises the windows of the first `count`
    /// carriers, fits the blur and decides every carrier's turn into
    /// ``positions`` and ``leads``. Returns the fitted blur, or `nil` when
    /// `limit` is set and the blur reaches ``PetalTurns/blurLimit``.
    func decide(_ image: PetalLumaView, _ pose: PetalPose, count: Int, limit: Bool) -> Int? {
        typealias T = PetalTurns
        let points = T.gridPoints
        let tables = T.Tables.shared
        // the pose's smooth error, from the centroids of a sample of the carriers
        let sampledX = samples
        let sampledY = samples + Self.sampleCapacity
        let offsetsX = samples + 2 * Self.sampleCapacity
        let offsetsY = samples + 3 * Self.sampleCapacity
        var sampled = 0
        for index in stride(from: 0, to: count, by: T.sampleStep) {
            let cell = cells[index]
            T.sampleWindow(image, pose, cell: cell, shiftX: 0.0, shiftY: 0.0, into: scratch)
            T.normalise(scratch)
            let (a, b) = T.centroid(scratch)
            let s = PetalLayout.cellSin[cell]
            let c = PetalLayout.cellCos[cell]
            sampledX[sampled] = PetalLayout.cellCenterX[cell]
            sampledY[sampled] = PetalLayout.cellCenterY[cell]
            offsetsX[sampled] = a * c - b * s
            offsetsY[sampled] = a * s + b * c
            sampled += 1
        }
        withUnsafeTemporaryAllocation(of: Double.self, capacity: 20) { fit in
            let fitX = fit.baseAddress!
            let fitY = fitX + 10
            T.fitCubic(sampledX, sampledY, offsetsX, count: sampled, into: fitX)
            T.fitCubic(sampledX, sampledY, offsetsY, count: sampled, into: fitY)
            for index in 0..<count {
                let cell = cells[index]
                let x = PetalLayout.cellCenterX[cell]
                let y = PetalLayout.cellCenterY[cell]
                let window = windows + index * points
                T.sampleWindow(
                    image,
                    pose,
                    cell: cell,
                    shiftX: T.evalCubic(fitX, x, y),
                    shiftY: T.evalCubic(fitY, x, y),
                    into: window
                )
                T.normalise(window)
            }
        }
        // the blur under which a sample of the carriers, each at its best turn with its lit
        // neighbours at theirs, explains the windows best
        let kernelBlock = T.positions * points
        var fitted = 0
        var bestScore = -Double.infinity
        for blur in T.blurs.indices {
            let template = tables.templates + blur * T.Tables.templateCells
            let kernels = tables.kernels + blur * kernelBlock
            turnOf.update(repeating: -1, count: count)
            var score = 0.0
            for index in stride(from: 0, to: count, by: T.sampleStep) {
                let position = turn(of: index, kernels)
                let s = tables.turnSin[position]
                let c = tables.turnCos[position]
                for point in 0..<points {
                    let a = tables.gridA[point]
                    let b = tables.gridB[point]
                    model[point] = T.Tables.at(template, a * c + b * s, -a * s + b * c)
                }
                let cell = cells[index]
                for near in tables.nearStart[cell]..<tables.nearStart[cell + 1] {
                    let other = Int(tables.nearCell[near])
                    if !lit[other] { continue }
                    let carrier = Int(carrierOf[other])
                    let angle = carrier < 0 ? 0.0 : T.turnAngle(UInt8(turn(of: carrier, kernels)))
                    T.addNeighbour(
                        model,
                        template,
                        along: tables.nearAlong[near],
                        across: tables.nearAcross[near],
                        angle: tables.nearAngle[near],
                        turn: angle,
                        sign: 1.0
                    )
                }
                let window = windows + index * points
                var cross = 0.0
                var mm = 0.0
                var xx = 0.0
                for point in 0..<points {
                    let x = window[point]
                    let m = model[point]
                    let w = tables.gridWeight[point]
                    cross += w * x * m
                    mm += w * m * m
                    xx += w * x * x
                }
                score += cross / PetalNumeric.maxNum((mm * xx).squareRoot(), 1e-9)
            }
            if score > bestScore {
                bestScore = score
                fitted = blur
            }
        }
        if limit && fitted >= T.blurLimit { return nil }
        // the turns, decided with the next wider blur
        let blur = min(fitted + 1, T.blurs.count - 1)
        let template = tables.templates + blur * T.Tables.templateCells
        let kernels = tables.kernels + blur * kernelBlock
        for index in 0..<count {
            (positions[index], leads[index]) = T.decide(windows + index * points, kernels)
        }
        for index in 0..<count {
            scratch.update(from: windows + index * points, count: points)
            let cell = cells[index]
            for near in tables.nearStart[cell]..<tables.nearStart[cell + 1] {
                let other = Int(tables.nearCell[near])
                if !lit[other] { continue }
                let carrier = Int(carrierOf[other])
                let angle = carrier < 0 ? 0.0 : T.turnAngle(positions[carrier])
                T.addNeighbour(
                    scratch,
                    template,
                    along: tables.nearAlong[near],
                    across: tables.nearAcross[near],
                    angle: tables.nearAngle[near],
                    turn: angle,
                    sign: -1.0
                )
            }
            (positions[index], leads[index]) = T.decide(scratch, kernels)
        }
        return fitted
    }

    /// The best turn position of carrier `carrier`'s own window under
    /// `kernels`, decided once per blur.
    @inline(__always)
    private func turn(of carrier: Int, _ kernels: UnsafePointer<Double>) -> Int {
        if turnOf[carrier] < 0 {
            turnOf[carrier] = Int8(PetalTurns.decide(windows + carrier * PetalTurns.gridPoints, kernels).position)
        }
        return Int(turnOf[carrier])
    }
}

/// The turns a reader decided for a capture, for diagnostics
/// (``PetalTurns/observedTurns(_:frame:)``).
public struct PetalObservedTurns: Equatable, Sendable {
    /// One carrier petal and the turn decided for it.
    public struct Carrier: Equatable, Sendable {
        /// Field cell index (`0..<3376`).
        public let cell: Int
        /// The turn position decided for it (`0..<4`).
        public let position: UInt8
        /// How far that position's score led the runner-up's.
        public let lead: Double
    }

    /// The blur (Gaussian σ in canvas units) that fits the carriers best.
    public let blur: Double
    /// Every carrier of the decoded sectors, sector by sector in carrier order.
    public let carriers: [Carrier]
}
