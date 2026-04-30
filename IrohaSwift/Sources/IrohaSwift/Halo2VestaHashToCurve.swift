import Foundation

enum VestaHashToCurve {
    private static let curveID = "vesta"
    private static let dstSuffix = "_XMD:BLAKE2b_SSWU_RO_"
    private static let xmdChunkLength = 64
    private static let xmdBlockLength = 128

    private static let isoA = PastaFq.fromRawLimbs([
        0xc515ad7242eaa6b1,
        0x9673928c7d01b212,
        0x81639c4d96f78773,
        0x267f9b2ee592271a,
    ])
    private static let isoB = PastaFq(1265)
    private static let z = PastaFq.fromRawLimbs([
        0x8c46eb20fffffff4,
        0x224698fc0994a8dd,
        0x0000000000000000,
        0x4000000000000000,
    ])
    private static let theta = PastaFq.fromRawLimbs([
        0x632cae9872df1b5d,
        0x38578ccadf03ac27,
        0x53c3808d9e2f2357,
        0x2b3483a1ee9a382f,
    ])
    private static let isogenyConstants = [
        PastaFq.fromRawLimbs([
            0x43cd42c800000001,
            0x0205dd51cfa0961a,
            0x8e38e38e38e38e39,
            0x38e38e38e38e38e3,
        ]),
        PastaFq.fromRawLimbs([
            0x8b95c6aaf703bcc5,
            0x216b8861ec72bd5d,
            0xacecf10f5f7c09a2,
            0x1d935247b4473d17,
        ]),
        PastaFq.fromRawLimbs([
            0xaeac67bbeb586a3d,
            0xd59d03d23b39cb11,
            0xed7ee4a9cdf78f8f,
            0x18760c7f7a9ad20d,
        ]),
        PastaFq.fromRawLimbs([
            0xfb539a6f0000002b,
            0xe1c521a795ac8356,
            0x1c71c71c71c71c71,
            0x31c71c71c71c71c7,
        ]),
        PastaFq.fromRawLimbs([
            0xb7284f7eaf21a2e9,
            0xa3ad678129b604d3,
            0x1454798a5b5c56b2,
            0x0a2de485568125d5,
        ]),
        PastaFq.fromRawLimbs([
            0xf169c187d2533465,
            0x30cd6d53df49d235,
            0x0c621de8b91c242a,
            0x14735171ee542778,
        ]),
        PastaFq.fromRawLimbs([
            0x6bef1642aaaaaaab,
            0x5601f4709a8adcb3,
            0x0da12f684bda12f68,
            0x12f684bda12f684b,
        ]),
        PastaFq.fromRawLimbs([
            0x8bee58e5fb81de63,
            0x21d910aefb03b31d,
            0xd6767887afbe04d1,
            0x2ec9a923da239e8b,
        ]),
        PastaFq.fromRawLimbs([
            0x4986913ab4443034,
            0x97a3ca5c24e9ea63,
            0x66d1466e9de10e64,
            0x19b0d87e16e25788,
        ]),
        PastaFq.fromRawLimbs([
            0x8f64842c55555533,
            0x8bc32d36fb21a6a3,
            0x425ed097b425ed09,
            0x1ed097b425ed097b,
        ]),
        PastaFq.fromRawLimbs([
            0x58dfecce86b2745e,
            0x06a767bfc35b5bac,
            0x9e7eb64f890a820c,
            0x2f44d6c801c1b8bf,
        ]),
        PastaFq.fromRawLimbs([
            0xd43d449776f99d2f,
            0x926847fb9ddd76a1,
            0x252659ba2b546c7e,
            0x3d59f455cafc7668,
        ]),
        PastaFq.fromRawLimbs([
            0x8c46eb20fffffde5,
            0x224698fc0994a8dd,
            0x0000000000000000,
            0x4000000000000000,
        ]),
    ]

    static func hash(domainPrefix: String, message: Data) -> VestaProjective {
        let fields = hashToField(domainPrefix: domainPrefix, message: message)
        let q0 = mapToIsoCurve(fields[0]).toAffine()
        let q1 = mapToIsoCurve(fields[1]).toAffine()
        return isoMap(q0.added(q1))
    }

    private static func hashToField(domainPrefix: String, message: Data) -> [PastaFq] {
        precondition(domainPrefix.utf8.count < 256)
        let dst = Data(domainPrefix.utf8) + Data("-".utf8) + Data(curveID.utf8) + Data(dstSuffix.utf8)
        precondition(dst.count < 256)

        var b0Input = Data(repeating: 0, count: xmdBlockLength)
        b0Input.append(message)
        b0Input.append(contentsOf: [0, UInt8(xmdChunkLength * 2), 0])
        b0Input.append(dst)
        b0Input.append(UInt8(dst.count))
        let b0 = Blake2b.hash512(b0Input)

        var b1Input = Data()
        b1Input.append(b0)
        b1Input.append(1)
        b1Input.append(dst)
        b1Input.append(UInt8(dst.count))
        let b1 = Blake2b.hash512(b1Input)

        var b2Input = Data(capacity: xmdChunkLength + 1 + dst.count + 1)
        for (lhs, rhs) in zip(b0, b1) {
            b2Input.append(lhs ^ rhs)
        }
        b2Input.append(2)
        b2Input.append(dst)
        b2Input.append(UInt8(dst.count))
        let b2 = Blake2b.hash512(b2Input)

        return [b1, b2].map { digest in
            PastaFq.fromUniformBytes64(Data(digest.reversed()))!
        }
    }

    private static func mapToIsoCurve(_ u: PastaFq) -> IsoVestaProjective {
        let zU2 = z * u.squared()
        let ta = zU2.squared() + zU2
        let numX1 = isoB * (ta + PastaFq.one)
        let div = isoA * (ta == .zero ? z : -ta)
        let numX1Squared = numX1.squared()
        let divSquared = div.squared()
        let divCubed = divSquared * div
        let numGX1 = (numX1Squared + isoA * divSquared) * numX1 + isoB * divCubed
        let numX2 = zU2 * numX1

        let sqrt = PastaFq.sqrtRatio(numerator: numGX1, denominator: divCubed)!
        let y1 = sqrt.root
        let y2 = theta * zU2 * u * y1
        let numX = sqrt.isSquare ? numX1 : numX2
        var y = sqrt.isSquare ? y1 : y2
        if u.isOdd != y.isOdd {
            y = -y
        }
        return IsoVestaProjective(x: numX * div, y: y * divCubed, z: div)
    }

    private static func isoMap(_ point: IsoVestaAffine) -> VestaProjective {
        guard !point.isIdentity else {
            return .identity
        }
        let x = point.x
        let y = point.y
        let iso = isogenyConstants

        let numX = ((iso[0] * x + iso[1]) * x + iso[2]) * x + iso[3]
        let divX = (x + iso[4]) * x + iso[5]
        let numY = (((iso[6] * x + iso[7]) * x + iso[8]) * x + iso[9]) * y
        let divY = ((x + iso[10]) * x + iso[11]) * x + iso[12]

        let zOut = divX * divY
        let xOut = numX * divY * zOut
        let yOut = numY * divX * zOut.squared()
        return VestaProjective(x: xOut, y: yOut, z: zOut)
    }
}

private struct IsoVestaProjective {
    let x: PastaFq
    let y: PastaFq
    let z: PastaFq

    func toAffine() -> IsoVestaAffine {
        guard z != .zero,
              let zInv = z.inverted() else {
            return .identity
        }
        let z2 = zInv.squared()
        let z3 = z2 * zInv
        return IsoVestaAffine(x: x * z2, y: y * z3)
    }
}

private struct IsoVestaAffine: Equatable {
    let x: PastaFq
    let y: PastaFq
    let isIdentity: Bool

    static let identity = IsoVestaAffine(x: .zero, y: .zero, isIdentity: true)

    init(x: PastaFq, y: PastaFq, isIdentity: Bool = false) {
        self.x = x
        self.y = y
        self.isIdentity = isIdentity
    }

    func added(_ rhs: IsoVestaAffine) -> IsoVestaAffine {
        if isIdentity {
            return rhs
        }
        if rhs.isIdentity {
            return self
        }
        if x == rhs.x {
            if y + rhs.y == .zero {
                return .identity
            }
            let numerator = PastaFq(3) * x.squared() + VestaHashToCurveIsoConstants.a
            let denominator = PastaFq(2) * y
            guard let denominatorInv = denominator.inverted() else {
                return .identity
            }
            let slope = numerator * denominatorInv
            let x3 = slope.squared() - x - rhs.x
            let y3 = slope * (x - x3) - y
            return IsoVestaAffine(x: x3, y: y3)
        }
        guard let denominatorInv = (rhs.x - x).inverted() else {
            return .identity
        }
        let slope = (rhs.y - y) * denominatorInv
        let x3 = slope.squared() - x - rhs.x
        let y3 = slope * (x - x3) - y
        return IsoVestaAffine(x: x3, y: y3)
    }
}

private enum VestaHashToCurveIsoConstants {
    static let a = PastaFq.fromRawLimbs([
        0xc515ad7242eaa6b1,
        0x9673928c7d01b212,
        0x81639c4d96f78773,
        0x267f9b2ee592271a,
    ])
}
