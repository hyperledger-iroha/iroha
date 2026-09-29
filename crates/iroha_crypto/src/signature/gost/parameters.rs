//! Canonical parameter literals shared by fixed-width arithmetic and signing.
//!
//! 256-bit sets retain the `CryptoPro` A/B/C parameters from RFC 4357 section 11.4;
//! 512-bit sets retain RFC 7836 appendix A.1. Internal Algorithm names and public
//! encodings use this same owner for both signing and verification.

/// One curve's exact canonical integers and existing diagnostic identity.
pub(super) struct CurveConstants {
    /// Existing algorithm name used by key diagnostics.
    pub(super) name: &'static str,
    /// Width of one scalar or coordinate in bytes.
    pub(super) scalar_len: usize,
    /// Prime field modulus.
    pub(super) p: &'static str,
    /// Prime subgroup order.
    pub(super) q: &'static str,
    /// Short-Weierstrass coefficient a.
    pub(super) a: &'static str,
    /// Short-Weierstrass coefficient b.
    pub(super) b: &'static str,
    /// Generator x coordinate.
    pub(super) gx: &'static str,
    /// Generator y coordinate.
    pub(super) gy: &'static str,
}

/// Canonical integers for `Algorithm::Gost3410_2012_256ParamSetA`.
pub(super) const PARAM_256_A: CurveConstants = CurveConstants {
    name: "Algorithm::Gost3410_2012_256ParamSetA",
    scalar_len: 32,
    p: "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd97",
    q: "ffffffffffffffffffffffffffffffff6c611070995ad10045841b09b761b893",
    a: "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd94",
    b: "00000000000000000000000000000000000000000000000000000000000000a6",
    gx: "0000000000000000000000000000000000000000000000000000000000000001",
    gy: "8d91e471e0989cda27df505a453f2b7635294f2ddf23e3b122acc99c9e9f1e14",
};

/// Canonical integers for `Algorithm::Gost3410_2012_256ParamSetB`.
pub(super) const PARAM_256_B: CurveConstants = CurveConstants {
    name: "Algorithm::Gost3410_2012_256ParamSetB",
    scalar_len: 32,
    p: "8000000000000000000000000000000000000000000000000000000000000c99",
    q: "800000000000000000000000000000015f700cfff1a624e5e497161bcc8a198f",
    a: "8000000000000000000000000000000000000000000000000000000000000c96",
    b: "3e1af419a269a5f866a7d3c25c3df80ae979259373ff2b182f49d4ce7e1bbc8b",
    gx: "0000000000000000000000000000000000000000000000000000000000000001",
    gy: "3fa8124359f96680b83d1c3eb2c070e5c545c9858d03ecfb744bf8d717717efc",
};

/// Canonical integers for `Algorithm::Gost3410_2012_256ParamSetC`.
pub(super) const PARAM_256_C: CurveConstants = CurveConstants {
    name: "Algorithm::Gost3410_2012_256ParamSetC",
    scalar_len: 32,
    p: "9b9f605f5a858107ab1ec85e6b41c8aacf846e86789051d37998f7b9022d759b",
    q: "9b9f605f5a858107ab1ec85e6b41c8aa582ca3511eddfb74f02f3a6598980bb9",
    a: "9b9f605f5a858107ab1ec85e6b41c8aacf846e86789051d37998f7b9022d7598",
    b: "000000000000000000000000000000000000000000000000000000000000805a",
    gx: "0000000000000000000000000000000000000000000000000000000000000000",
    gy: "41ece55743711a8c3cbf3783cd08c0ee4d4dc440d4641a8f366e550dfdb3bb67",
};

/// Canonical integers for `Algorithm::Gost3410_2012_512ParamSetA`.
pub(super) const PARAM_512_A: CurveConstants = CurveConstants {
    name: "Algorithm::Gost3410_2012_512ParamSetA",
    scalar_len: 64,
    p: "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffdc7",
    q: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff27e69532f48d89116ff22b8d4e0560609b4b38abfad2b85dcacdb1411f10b275",
    a: "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffdc4",
    b: "e8c2505dedfc86ddc1bd0b2b6667f1da34b82574761cb0e879bd081cfd0b6265ee3cb090f30d27614cb4574010da90dd862ef9d4ebee4761503190785a71c760",
    gx: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003",
    gy: "7503cfe87a836ae3a61b8816e25450e6ce5e1c93acf1abc1778064fdcbefa921df1626be4fd036e93d75e6a50e3a41e98028fe5fc235f5b889a589cb5215f2a4",
};

/// Canonical integers for `Algorithm::Gost3410_2012_512ParamSetB`.
pub(super) const PARAM_512_B: CurveConstants = CurveConstants {
    name: "Algorithm::Gost3410_2012_512ParamSetB",
    scalar_len: 64,
    p: "8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006f",
    q: "800000000000000000000000000000000000000000000000000000000000000149a1ec142565a545acfdb77bd9d40cfa8b996712101bea0ec6346c54374f25bd",
    a: "8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006c",
    b: "687d1b459dc841457e3e06cf6f5e2517b97c7d614af138bcbf85dc806c4b289f3e965d2db1416d217f8b276fad1ab69c50f78bee1fa3106efb8ccbc7c5140116",
    gx: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002",
    gy: "1a8f7eda389b094c2c071e3647a8940f3c123b697578c213be6dd9e6c8ec7335dcb228fd1edf4a39152cbcaaf8c0398828041055f94ceeec7e21340780fe41bd",
};
