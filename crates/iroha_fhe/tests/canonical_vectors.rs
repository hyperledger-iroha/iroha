//! Canonical known-answer vectors for every `iroha_fhe` primitive.
//!
//! The expected values below were derived with plain Python integers and
//! rational arithmetic, never with the kernels under test. Two things are then
//! checked for every vector:
//!
//! 1. the kernel reproduces the pinned answer, through the dispatching entry
//!    point and through the scalar reference, so an accelerated path that
//!    drifts from the canonical words fails here; and
//! 2. the independent arbitrary-precision oracle in [`oracle`] re-derives the
//!    pinned answer from the inputs, so the vectors cannot drift from the
//!    mathematics either. The oracle uses `num-bigint` and the textbook
//!    definitions only (quadratic DFT, schoolbook negacyclic product,
//!    constructive CRT, exact rational rounding) and shares no code with the
//!    kernels.
use iroha_fhe::{accel, automorphism, key_switch, modular, ntt, polynomial, rns, rounding};

struct NttVector {
    modulus: u64,
    root: u64,
    input: &'static [u64],
    output: &'static [u64],
}

struct NegacyclicVector {
    modulus: u64,
    psi: u64,
    lhs: &'static [u64],
    rhs: &'static [u64],
    product: &'static [u64],
}

struct CrtVector {
    value: u128,
    residues: [u64; 8],
}

/// The registered RAM-LFE BFV RNS chain, the first production consumer of these kernels.
const REGISTERED_CHAIN: [u64; 8] = [
    30_593, 30_977, 31_489, 31_873, 32_257, 33_409, 35_201, 35_969,
];

const NTT_VECTORS: &[NttVector] = &[
    NttVector {
        modulus: 30_593,
        root: 269,
        input: &[22_905, 4_089, 3_010, 16_237, 22_458, 7_288, 6_482, 15_862],
        output: &[6_552, 6_698, 25_938, 3_356, 11_379, 9_437, 15_211, 12_890],
    },
    NttVector {
        modulus: 4_293_918_721,
        root: 3_453_551_898,
        input: &[
            282_659_782,
            2_271_384_886,
            833_818_634,
            1_596_571_504,
            2_766_174_514,
            8_171_611,
            2_303_839_625,
            3_603_932_887,
            2_078_965_166,
            219_516_719,
            1_528_592_728,
            1_173_586_094,
            701_204_293,
            97_068_224,
            1_077_983_704,
            458_525_577,
        ],
        output: &[
            3_826_321_064,
            1_523_645_375,
            549_575_079,
            2_057_555_780,
            1_314_884_565,
            177_398_743,
            1_645_274_209,
            1_218_523_020,
            2_144_480_944,
            2_123_137_210,
            818_348_408,
            4_063_639_381,
            3_148_572_284,
            2_295_130_448,
            1_151_624_310,
            2_227_958_018,
        ],
    },
    NttVector {
        modulus: 70_368_744_067_073,
        root: 70_287_346_274_229,
        input: &[
            37_175_764_207_069,
            50_913_876_395_641,
            21_086_529_338_571,
            26_908_103_875_081,
            49_433_980_077_771,
            37_576_402_940_725,
            11_468_140_093_949,
            15_839_836_328_127,
        ],
        output: &[
            39_296_401_055_715,
            10_556_076_474_330,
            18_443_541_366_706,
            30_286_364_972_539,
            58_294_938_244_859,
            52_653_596_645_664,
            19_297_864_270_861,
            68_577_330_625_878,
        ],
    },
    NttVector {
        modulus: 30_593,
        root: 10_567,
        input: &[
            26_783, 21_424, 23_280, 28_385, 29_565, 22_242, 20_343, 26_592, 13_062, 25_566, 8_370,
            6_357, 1_475, 25_451, 499, 28_014, 13_073, 1_962, 7_032, 1_403, 17_646, 24_431, 6_518,
            29_328, 27_614, 2_668, 28_877, 27_551, 28_019, 19_735, 9_049, 28_689, 24_747, 261,
            21_684, 1_261, 16_556, 17_921, 25_220, 11_320, 19_329, 14_011, 27_665, 25_346, 15_744,
            20_039, 8_023, 3_505, 13_769, 19_314, 22_556, 16_543, 30_069, 27_865, 17_939, 24_227,
            11_059, 29_814, 793, 24_897, 18_659, 8_559, 5_433, 4_812, 24_975, 29_970, 14_581,
            18_418, 29_275, 23_735, 28_441, 20_542, 1_944, 309, 25_401, 16_590, 20_617, 30_487,
            171, 11_934, 20_810, 17_978, 4_607, 23_878, 29_847, 23_753, 27_924, 5_536, 18_556,
            21_405, 26_629, 12_585, 17_356, 9_767, 12_728, 20_581, 1_889, 8_424, 28_222, 12_848,
            15_742, 25_996, 12_529, 5_634, 22_482, 29_322, 20_084, 17_838, 6_235, 4_441, 10_581,
            15_014, 24_449, 6_014, 9_504, 30_394, 12_579, 5_837, 1_305, 9_193, 7_671, 10_505,
            26_464, 17_508, 19_740, 17_966, 30_518, 13_142,
        ],
        output: &[
            9_240, 14_271, 21_031, 19_685, 15_397, 30_474, 15_474, 3_200, 29_823, 4_065, 5_823,
            20_355, 19_048, 21_947, 21_434, 23_024, 23_042, 8_380, 25_757, 7_340, 8_514, 24_445,
            27_778, 16_183, 13_222, 7_466, 22_130, 3_161, 11_317, 14_223, 12_580, 679, 10_188,
            22_227, 4_346, 15_061, 16_269, 24_060, 29_073, 5_779, 14_114, 14_102, 28_658, 8_659,
            13_091, 28_504, 21_140, 18_651, 3_786, 30_107, 8_638, 13_699, 29_996, 27_815, 24_199,
            29_662, 25_774, 13_434, 23_908, 15_879, 14_696, 9_228, 30_097, 10_937, 7_269, 27_855,
            24_531, 28_997, 14_486, 9_525, 10_980, 9_917, 16_088, 12_721, 26_953, 29_932, 28_519,
            24_040, 3_196, 4_422, 5_896, 20_597, 25_837, 13_266, 29_493, 27_845, 28_125, 18_906,
            30_278, 11_632, 4_468, 3_508, 16_632, 29_097, 2_393, 20_650, 4_172, 12_269, 13_592,
            17_600, 19_439, 26_779, 3_884, 3_212, 6_412, 29_215, 24_670, 27_778, 12_804, 4_272,
            23_210, 13_629, 3_186, 4_625, 6_815, 28_379, 25_582, 11_347, 27_882, 18_244, 2_734,
            28_844, 18_671, 19_378, 8_963, 2_028, 22_480, 16_884,
        ],
    },
];

const NEGACYCLIC_VECTORS: &[NegacyclicVector] = &[
    NegacyclicVector {
        modulus: 30_593,
        psi: 10_567,
        lhs: &[
            2_278, 18_121, 29_786, 5_742, 12_759, 16_344, 4_493, 29_753, 5_027, 20_168, 1_525,
            15_976, 6_495, 10_213, 21_537, 3_835, 5_416, 1_491, 6_307, 19_495, 10_805, 30_240,
            13_476, 4_964, 23_685, 14_791, 15_124, 25_047, 29_392, 22_496, 9_409, 4_040, 16_000,
            26_286, 5_109, 21_345, 28_066, 3_728, 7_240, 26_395, 18_100, 15_927, 10_141, 26_391,
            14_856, 15_146, 6_802, 20_215, 14_331, 13_892, 24_077, 8_738, 28_252, 6_516, 2_552,
            9_792, 3_206, 17_063, 371, 5_472, 8_124, 27_378, 5_713, 4_295,
        ],
        rhs: &[
            6_212, 1_035, 4_445, 1_453, 16_924, 11_178, 28_519, 11_257, 10_216, 12_488, 20_894,
            23_264, 14_415, 11_753, 853, 8_760, 21_151, 3_926, 30_420, 19_322, 5_842, 11_226,
            23_212, 19_018, 14_080, 10_974, 14_758, 28_891, 30_044, 3_220, 10_252, 2_039, 15_671,
            19_747, 2_987, 14_171, 25_642, 2_151, 9_296, 11_106, 14_923, 26_310, 1_581, 22_358,
            19_158, 10_379, 19_600, 5_021, 18_342, 9_675, 6_471, 225, 110, 7_178, 20_081, 5_604,
            22_486, 28_614, 27_332, 12_402, 86, 13_270, 27_971, 8_329,
        ],
        product: &[
            17_520, 13_684, 21_117, 28_931, 12_381, 26_098, 20_103, 19_664, 21_103, 8_819, 4_438,
            6_297, 9_581, 19_883, 12_345, 26_329, 6_719, 19_163, 1_127, 28_421, 2_615, 27_576,
            11_266, 9_164, 26_480, 11_010, 15_935, 27_092, 13_760, 17_704, 3_631, 11_462, 20_922,
            688, 28_071, 2_209, 18_604, 13_396, 29_892, 13_719, 15_063, 29_792, 6_508, 1_675,
            20_665, 2_466, 9_297, 10_987, 8_070, 2_349, 1_978, 6_959, 21_217, 9_367, 13_586,
            28_623, 15_459, 4_194, 22_494, 25_095, 9_220, 2_748, 26_967, 5_784,
        ],
    },
    NegacyclicVector {
        modulus: 35_969,
        psi: 22_798,
        lhs: &[
            14_267, 19_767, 24_799, 12_397, 35_273, 32_403, 32_065, 8_719, 2_365, 3_997, 26_988,
            30_825, 2_398, 33_216, 30_042, 2_923, 4_038, 11_471, 35_205, 26_006, 34_757, 22_295,
            14_642, 26_954, 13_032, 9_538, 3_122, 31_983, 20_490, 1_862, 16_715, 12_730, 2_972,
            17_051, 14_012, 1_535, 21_622, 16_230, 21_861, 12_659, 26_207, 15_230, 11_319, 1_274,
            18_701, 5_232, 23_208, 29_637, 22_141, 5_845, 30_850, 25_505, 29_195, 22_691, 30_404,
            18_106, 5_330, 14_534, 20_895, 96, 22_716, 513, 17_174, 27_654,
        ],
        rhs: &[
            12_326, 32_189, 4_070, 1_034, 19_783, 8_391, 23_241, 3_417, 18_545, 17_225, 20_869,
            9_344, 806, 10_298, 35_100, 23_046, 26_937, 4_426, 33_831, 23_255, 3_985, 1_632,
            34_978, 31_467, 921, 18_398, 28_210, 1_063, 18_051, 23_030, 30_718, 23_314, 2_171,
            21_987, 34_516, 9_706, 19_929, 13_006, 29_036, 7_701, 14_191, 22_040, 13_906, 444,
            34_422, 6_611, 16_541, 1_971, 21_710, 22_391, 29_532, 28_788, 6_752, 20_155, 7_581,
            27_106, 3_225, 16_058, 17_075, 24_306, 2_785, 14_721, 31_298, 34_385,
        ],
        product: &[
            10_320, 7_589, 26_246, 15_368, 34_586, 22_817, 17_924, 3_106, 15_478, 23_903, 3_160,
            19_038, 10_171, 34_296, 31_390, 33_919, 33_096, 21_544, 27_764, 13_021, 20_559, 27_604,
            28_165, 1_622, 6_745, 20_776, 320, 17_883, 35_512, 25_911, 27_972, 30_297, 5_217,
            11_985, 24_577, 33_324, 9_709, 23_437, 15_573, 24_504, 6_668, 16_127, 31_834, 6_807,
            21_247, 4_779, 33_662, 29_779, 9_858, 35_948, 4_007, 11_482, 24_737, 3_951, 12_447,
            11_793, 29_317, 26_716, 4_021, 6_730, 25_360, 19_674, 8_723, 7_864,
        ],
    },
    NegacyclicVector {
        modulus: 2_013_265_921,
        psi: 1_400_279_418,
        lhs: &[
            1_369_280_874,
            1_527_395_962,
            1_388_365_015,
            12_017_037,
            286_547_190,
            1_107_238_749,
            786_402_016,
            1_657_347_038,
        ],
        rhs: &[
            720_041_758,
            1_280_120_307,
            1_802_856_950,
            1_974_888_210,
            1_798_446_958,
            1_985_036_742,
            470_848_405,
            987_484_042,
        ],
        product: &[
            240_924_975,
            1_479_232_564,
            1_355_342_906,
            1_141_200_518,
            878_984_420,
            210_374_496,
            1_283_020_747,
            1_423_527_246,
        ],
    },
    NegacyclicVector {
        modulus: 4_293_918_721,
        psi: 3_325_913_544,
        lhs: &[
            2_141_278_760,
            2_828_356_851,
            1_569_965_600,
            1_254_058_718,
            1_087_738_113,
            520_479_933,
            1_317_517_633,
            477_106_692,
            532_163_910,
            817_875_228,
            855_521_552,
            3_236_695_127,
            2_694_456_035,
            1_756_454_454,
            3_882_634_538,
            3_032_566_268,
        ],
        rhs: &[
            276_536_660,
            3_353_429_871,
            2_345_252_733,
            2_171_864_770,
            160_082_967,
            3_525_131_763,
            3_916_325_529,
            2_680_392_496,
            3_730_802_840,
            78_971_175,
            2_481_068_991,
            1_733_509_928,
            964_244_143,
            2_070_624_781,
            1_579_991_845,
            2_469_799_011,
        ],
        product: &[
            3_700_115_480,
            3_334_133_918,
            1_490_988_526,
            3_705_308_837,
            4_000_864_717,
            25_085_410,
            3_979_755_221,
            2_974_739_127,
            1_526_292_887,
            2_810_611_736,
            3_384_812_228,
            2_371_165_244,
            150_410_045,
            2_046_262_043,
            2_174_727_209,
            3_364_927_084,
        ],
    },
];

/// Product of the registered chain.
const REGISTERED_PRODUCT: u128 = 1_297_818_766_851_231_300_719_063_877_926_878_849;
const CRT_VECTORS: &[CrtVector] = &[
    CrtVector {
        value: 0,
        residues: [0, 0, 0, 0, 0, 0, 0, 0],
    },
    CrtVector {
        value: 1,
        residues: [1, 1, 1, 1, 1, 1, 1, 1],
    },
    CrtVector {
        value: 1_297_818_766_851_231_300_719_063_877_926_878_848,
        residues: [
            30_592, 30_976, 31_488, 31_872, 32_256, 33_408, 35_200, 35_968,
        ],
    },
    CrtVector {
        value: 648_909_383_425_615_650_359_531_938_963_439_424,
        residues: [
            15_296, 15_488, 15_744, 15_936, 16_128, 16_704, 17_600, 17_984,
        ],
    },
    CrtVector {
        value: 648_909_383_425_615_650_359_531_938_963_439_425,
        residues: [
            15_297, 15_489, 15_745, 15_937, 16_129, 16_705, 17_601, 17_985,
        ],
    },
    CrtVector {
        value: 955_323_399_664_798_892_118_432_108_576_942_751,
        residues: [3_399, 25_403, 637, 14_733, 15_486, 17_116, 2_158, 22_929],
    },
    CrtVector {
        value: 269_484_031,
        residues: [20_887, 15_108, 1_169, 29_689, 9_053, 7_037, 20_376, 4_283],
    },
    CrtVector {
        value: 18_446_744_073_709_551_615,
        residues: [4_440, 12_472, 22_672, 4_584, 22_236, 766, 27_379, 24_080],
    },
];

const BASIS_SOURCE_PRODUCT: u128 = 29_841_475_398_529;
const BASIS_TARGET: [u64; 3] = [35_201, 35_969, 2_013_265_921];
const BASIS_VALUES: [u128; 8] = [
    0,
    1,
    29_841_475_398_528,
    14_920_737_699_264,
    14_920_737_699_265,
    8_154_955_338_749,
    16_865_747_575_654,
    5_507_152_747_874,
];
const BASIS_SOURCE_RESIDUES: [[u64; 8]; 3] = [
    [0, 1, 30_592, 15_296, 15_297, 26_651, 15_592, 17_711],
    [0, 1, 30_976, 15_488, 15_489, 5_857, 26_175, 12_667],
    [0, 1, 31_488, 15_744, 15_745, 25_226, 16_699, 3_883],
];
const BASIS_TARGET_RESIDUES: [[u64; 8]; 3] = [
    [0, 1, 27_825, 31_513, 31_514, 3_263, 19_910, 17_516],
    [0, 1, 3_456, 1_728, 1_729, 29_588, 15_214, 17_422],
    [
        0,
        1,
        847_917_466,
        423_958_733,
        423_958_734,
        1_228_358_699,
        618_955_437,
        870_453_939,
    ],
];
const BASIS_CENTERED_TARGET_RESIDUES: [[u64; 8]; 3] = [
    [0, 1, 35_200, 31_513, 3_688, 3_263, 27_285, 17_516],
    [0, 1, 35_968, 1_728, 34_241, 29_588, 11_757, 17_422],
    [
        0,
        1,
        2_013_265_920,
        423_958_733,
        1_589_307_188,
        1_228_358_699,
        1_784_303_891,
        870_453_939,
    ],
];

/// The registered RAM-LFE BFV ciphertext modulus `q = 257 * 2^48`
/// (`RAM_LFE_BFV_CIPHERTEXT_MODULUS` in `iroha_crypto`). Scaling by `t / q` with `t = 257`
/// divides by `2^48`, so the exact half point of the rounding is `2^47`.
const REGISTERED_CIPHERTEXT_MODULUS: u64 = 72_339_069_014_638_592;
/// The registered RAM-LFE BFV plaintext modulus `t`.
const REGISTERED_PLAINTEXT_MODULUS: u64 = 257;
/// `2^47`: the coefficient whose scaled value is exactly one half at the registered moduli.
const REGISTERED_HALF_POINT: i128 = 140_737_488_355_328;

/// `(coefficient, numerator, denominator, round(coefficient * numerator / denominator))`.
///
/// The first block uses the registered moduli. The second uses the small pair
/// `t = 257, q = 257 * 2^20` (half point `2^19`), which is not a registered modulus.
const SCALE_ROUND_VECTORS: &[(i128, u64, u64, i128)] = &[
    (0, 257, 72_339_069_014_638_592, 0),
    (1, 257, 72_339_069_014_638_592, 0),
    (-1, 257, 72_339_069_014_638_592, 0),
    (140_737_488_355_327, 257, 72_339_069_014_638_592, 0),
    (140_737_488_355_328, 257, 72_339_069_014_638_592, 1),
    (140_737_488_355_329, 257, 72_339_069_014_638_592, 1),
    (-140_737_488_355_327, 257, 72_339_069_014_638_592, 0),
    (-140_737_488_355_328, 257, 72_339_069_014_638_592, -1),
    (-140_737_488_355_329, 257, 72_339_069_014_638_592, -1),
    (422_212_465_065_984, 257, 72_339_069_014_638_592, 2),
    (-422_212_465_065_984, 257, 72_339_069_014_638_592, -2),
    (36_169_534_507_319_296, 257, 72_339_069_014_638_592, 129),
    (-36_169_534_507_319_295, 257, 72_339_069_014_638_592, -128),
    (217_157_944_532_271_104, 257, 72_339_069_014_638_592, 772),
    (-217_157_944_532_271_104, 257, 72_339_069_014_638_592, -772),
    (
        1_298_074_214_633_706_907_273_361_570_660_352,
        257,
        72_339_069_014_638_592,
        4_611_686_018_427_387_905,
    ),
    (0, 257, 269_484_032, 0),
    (1, 257, 269_484_032, 0),
    (-1, 257, 269_484_032, 0),
    (524_287, 257, 269_484_032, 0),
    (524_288, 257, 269_484_032, 1),
    (524_289, 257, 269_484_032, 1),
    (-524_287, 257, 269_484_032, 0),
    (-524_288, 257, 269_484_032, -1),
    (-524_289, 257, 269_484_032, -1),
    (1_572_864, 257, 269_484_032, 2),
    (-1_572_864, 257, 269_484_032, -2),
    (18_155_410_875_744_256, 257, 269_484_032, 17_314_349_056),
    (-18_155_410_876_268_544, 257, 269_484_032, -17_314_349_057),
    (
        1_267_650_600_228_229_401_496_703_729_664,
        257,
        269_484_032,
        1_208_925_819_614_629_174_706_177,
    ),
    (1, 4, 8, 1),
    (-1, 4, 8, -1),
    (3, 4, 8, 2),
    (-3, 4, 8, -2),
    (3, 1, 7, 0),
    (4, 1, 7, 1),
    (-3, 1, 7, 0),
    (-4, 1, 7, -1),
    (7, 30_593, 35_969, 6),
    (-17_984, 30_593, 35_969, -15_296),
    (17_984, 30_593, 35_969, 15_296),
];

/// `(value, from_modulus, to_modulus, switched)`.
///
/// The first block switches from the registered ciphertext modulus: to the first registered
/// limb, to the plaintext modulus (half point `2^47`) and to the small `257 * 2^20` modulus.
const MODULUS_SWITCH_VECTORS: &[(u64, u64, u64, u64)] = &[
    (0, 72_339_069_014_638_592, 30_593, 0),
    (1, 72_339_069_014_638_592, 30_593, 0),
    (140_737_488_355_327, 72_339_069_014_638_592, 30_593, 60),
    (140_737_488_355_328, 72_339_069_014_638_592, 30_593, 60),
    (140_737_488_355_329, 72_339_069_014_638_592, 30_593, 60),
    (
        36_169_534_507_319_295,
        72_339_069_014_638_592,
        30_593,
        15_296,
    ),
    (
        36_169_534_507_319_296,
        72_339_069_014_638_592,
        30_593,
        15_297,
    ),
    (
        36_169_534_507_319_297,
        72_339_069_014_638_592,
        30_593,
        15_297,
    ),
    (
        72_198_331_526_283_264,
        72_339_069_014_638_592,
        30_593,
        30_533,
    ),
    (
        72_198_331_526_283_265,
        72_339_069_014_638_592,
        30_593,
        30_533,
    ),
    (72_339_069_014_638_591, 72_339_069_014_638_592, 30_593, 0),
    (
        24_113_023_004_879_530,
        72_339_069_014_638_592,
        30_593,
        10_198,
    ),
    (0, 72_339_069_014_638_592, 257, 0),
    (1, 72_339_069_014_638_592, 257, 0),
    (140_737_488_355_327, 72_339_069_014_638_592, 257, 0),
    (140_737_488_355_328, 72_339_069_014_638_592, 257, 1),
    (140_737_488_355_329, 72_339_069_014_638_592, 257, 1),
    (36_169_534_507_319_295, 72_339_069_014_638_592, 257, 128),
    (36_169_534_507_319_296, 72_339_069_014_638_592, 257, 129),
    (36_169_534_507_319_297, 72_339_069_014_638_592, 257, 129),
    (72_198_331_526_283_264, 72_339_069_014_638_592, 257, 256),
    (72_198_331_526_283_265, 72_339_069_014_638_592, 257, 0),
    (72_339_069_014_638_591, 72_339_069_014_638_592, 257, 0),
    (24_113_023_004_879_530, 72_339_069_014_638_592, 257, 86),
    (0, 72_339_069_014_638_592, 269_484_032, 0),
    (1, 72_339_069_014_638_592, 269_484_032, 0),
    (
        140_737_488_355_327,
        72_339_069_014_638_592,
        269_484_032,
        524_288,
    ),
    (
        140_737_488_355_328,
        72_339_069_014_638_592,
        269_484_032,
        524_288,
    ),
    (
        140_737_488_355_329,
        72_339_069_014_638_592,
        269_484_032,
        524_288,
    ),
    (
        36_169_534_507_319_295,
        72_339_069_014_638_592,
        269_484_032,
        134_742_016,
    ),
    (
        36_169_534_507_319_296,
        72_339_069_014_638_592,
        269_484_032,
        134_742_016,
    ),
    (
        36_169_534_507_319_297,
        72_339_069_014_638_592,
        269_484_032,
        134_742_016,
    ),
    (
        72_198_331_526_283_264,
        72_339_069_014_638_592,
        269_484_032,
        268_959_744,
    ),
    (
        72_198_331_526_283_265,
        72_339_069_014_638_592,
        269_484_032,
        268_959_744,
    ),
    (
        72_339_069_014_638_591,
        72_339_069_014_638_592,
        269_484_032,
        0,
    ),
    (
        24_113_023_004_879_530,
        72_339_069_014_638_592,
        269_484_032,
        89_828_011,
    ),
    (0, 16, 4, 0),
    (1, 16, 4, 0),
    (2, 16, 4, 1),
    (5, 16, 4, 1),
    (6, 16, 4, 2),
    (7, 16, 4, 2),
    (8, 16, 4, 2),
    (9, 16, 4, 2),
    (10, 16, 4, 2),
    (15, 16, 4, 0),
    (0, 269_484_032, 30_593, 0),
    (1, 269_484_032, 30_593, 0),
    (2, 269_484_032, 30_593, 0),
    (6, 269_484_032, 30_593, 0),
    (10, 269_484_032, 30_593, 0),
    (89_828_010, 269_484_032, 30_593, 10_198),
    (134_742_015, 269_484_032, 30_593, 15_296),
    (134_742_016, 269_484_032, 30_593, 15_297),
    (134_742_017, 269_484_032, 30_593, 15_297),
    (269_484_031, 269_484_032, 30_593, 0),
    (0, 35_969, 30_593, 0),
    (1, 35_969, 30_593, 1),
    (2, 35_969, 30_593, 2),
    (6, 35_969, 30_593, 5),
    (10, 35_969, 30_593, 9),
    (11_989, 35_969, 30_593, 10_197),
    (17_983, 35_969, 30_593, 15_295),
    (17_984, 35_969, 30_593, 15_296),
    (17_985, 35_969, 30_593, 15_297),
    (35_968, 35_969, 30_593, 30_592),
    (0, 30_593, 35_969, 0),
    (1, 30_593, 35_969, 1),
    (2, 30_593, 35_969, 2),
    (6, 30_593, 35_969, 7),
    (10, 30_593, 35_969, 12),
    (10_197, 30_593, 35_969, 11_989),
    (15_295, 30_593, 35_969, 17_983),
    (15_296, 30_593, 35_969, 17_984),
    (15_297, 30_593, 35_969, 17_985),
    (30_592, 30_593, 35_969, 35_968),
    (0, 17, 4_293_918_721, 0),
    (1, 17, 4_293_918_721, 252_583_454),
    (2, 17, 4_293_918_721, 505_166_908),
    (5, 17, 4_293_918_721, 1_262_917_271),
    (6, 17, 4_293_918_721, 1_515_500_725),
    (7, 17, 4_293_918_721, 1_768_084_179),
    (8, 17, 4_293_918_721, 2_020_667_633),
    (9, 17, 4_293_918_721, 2_273_251_088),
    (10, 17, 4_293_918_721, 2_525_834_542),
    (16, 17, 4_293_918_721, 4_041_335_267),
];

/// `(value, divisor, round(value / divisor))`.
const DIV_ROUND_VECTORS: &[(i128, i128, i128)] = &[
    (0, 4, 0),
    (1, 4, 0),
    (2, 4, 1),
    (3, 4, 1),
    (5, 4, 1),
    (6, 4, 2),
    (-1, 4, 0),
    (-2, 4, -1),
    (-5, 4, -1),
    (-6, 4, -2),
    (7, 5, 1),
    (8, 5, 2),
    (-7, 5, -1),
    (-8, 5, -2),
    (
        1_267_650_600_228_229_401_496_703_729_664,
        1_048_576,
        1_208_925_819_614_629_174_706_177,
    ),
    (
        -1_267_650_600_228_229_401_496_703_729_664,
        1_048_576,
        -1_208_925_819_614_629_174_706_177,
    ),
    (
        1_267_650_600_228_229_401_496_703_729_663,
        1_048_576,
        1_208_925_819_614_629_174_706_176,
    ),
];

const CONVOLUTION_LHS: [u64; 4] = [
    4_611_686_018_427_387_903,
    2_305_843_009_213_693_952,
    1_234_567_890_123_456_789,
    1,
];
const CONVOLUTION_RHS: [u64; 4] = [
    2_305_843_009_213_693_959,
    4_611_686_018_427_387_847,
    3,
    1_152_921_504_606_846_976,
];
const CONVOLUTION_LINEAR: [u128; 8] = [
    10_633_823_966_279_327_013_206_415_602_020_777_977,
    26_584_559_915_698_317_206_739_253_201_314_250_809,
    13_480_543_705_120_199_549_556_459_593_351_202_704,
    11_010_351_460_821_408_779_879_351_736_617_550_426,
    2_658_455_991_569_831_754_123_003_809_358_447_366,
    1_423_359_869_420_436_337_641_010_675_071_320_067,
    1_152_921_504_606_846_976,
    0,
];
const CONVOLUTION_FOLDED: [i128; 4] = [
    7_975_367_974_709_495_259_083_411_792_662_330_611,
    25_161_200_046_277_880_869_098_242_526_242_930_742,
    13_480_543_705_120_199_548_403_538_088_744_355_728,
    11_010_351_460_821_408_779_879_351_736_617_550_426,
];

/// A small `257 * 2^20` modulus; the registered-modulus vectors follow below.
const AUTOMORPHISM_MODULUS: u64 = 269_484_032;
const AUTOMORPHISM_INPUT: [u64; 8] = [
    118_478_662,
    45_748_893,
    114_928_104,
    6_059_799,
    234_871_130,
    67_525_665,
    158_274_076,
    146_117_691,
];
/// `(power, image of the input under X -> X^power)`.
const AUTOMORPHISM_VECTORS: &[(u32, [u64; 8])] = &[
    (
        3,
        [
            118_478_662,
            263_424_233,
            158_274_076,
            45_748_893,
            34_612_902,
            146_117_691,
            114_928_104,
            201_958_367,
        ],
    ),
    (
        5,
        [
            118_478_662,
            201_958_367,
            154_555_928,
            146_117_691,
            234_871_130,
            45_748_893,
            111_209_956,
            263_424_233,
        ],
    ),
    (
        15,
        [
            118_478_662,
            123_366_341,
            111_209_956,
            201_958_367,
            34_612_902,
            263_424_233,
            154_555_928,
            223_735_139,
        ],
    ),
];

/// Input at the registered ciphertext modulus: zero, one, `q - 1`, the even midpoint, the first
/// negative residue and three other residues.
const REGISTERED_AUTOMORPHISM_INPUT: [u64; 8] = [
    0,
    1,
    72_339_069_014_638_591,
    36_169_534_507_319_296,
    36_169_534_507_319_297,
    50_776_993_813_698_068,
    63_023_082_341_783_969,
    63_903_215_954_102_178,
];
/// `(power, image of the registered input under X -> X^power)`.
const REGISTERED_AUTOMORPHISM_VECTORS: &[(u32, [u64; 8])] = &[
    (
        3,
        [
            0,
            36_169_534_507_319_296,
            63_023_082_341_783_969,
            1,
            36_169_534_507_319_295,
            63_903_215_954_102_178,
            72_339_069_014_638_591,
            21_562_075_200_940_524,
        ],
    ),
    (
        5,
        [
            0,
            21_562_075_200_940_524,
            1,
            63_903_215_954_102_178,
            36_169_534_507_319_297,
            1,
            9_315_986_672_854_623,
            36_169_534_507_319_296,
        ],
    ),
    (
        15,
        [
            0,
            8_435_853_060_536_414,
            9_315_986_672_854_623,
            21_562_075_200_940_524,
            36_169_534_507_319_295,
            36_169_534_507_319_296,
            1,
            72_339_069_014_638_591,
        ],
    ),
];

/// Residues of the small `257 * 2^20` modulus in base `2^12`, three digits.
const DIGIT_INPUT: [u64; 6] = [0, 1, 4_095, 4_096, 269_484_031, 134_742_016];
const DIGIT_VECTORS: [[u64; 6]; 3] = [
    [0, 1, 4_095, 0, 4_095, 0],
    [0, 0, 0, 1, 255, 128],
    [0, 0, 0, 0, 16, 8],
];
/// Residues of the registered ciphertext modulus in base `2^12`: five digits cover its 57 bits.
const REGISTERED_DIGIT_INPUT: [u64; 8] = [
    0,
    1,
    4_095,
    4_096,
    72_339_069_014_638_591,
    36_169_534_507_319_296,
    281_474_976_710_656,
    72_057_662_757_408_769,
];
const REGISTERED_DIGIT_VECTORS: [[u64; 8]; 5] = [
    [0, 1, 4_095, 0, 4_095, 0, 0, 1],
    [0, 0, 0, 1, 4_095, 0, 0, 1],
    [0, 0, 0, 0, 4_095, 0, 0, 0],
    [0, 0, 0, 0, 4_095, 2_048, 0, 1],
    [0, 0, 0, 0, 256, 128, 1, 256],
];

/// `(lhs, rhs, modulus, sum, difference, product)`.
const SCALAR_VECTORS: &[(u64, u64, u64, u64, u64, u64)] = &[
    (30_592, 30_592, 30_593, 30_591, 0, 1),
    (0, 1, 30_593, 1, 30_592, 0),
    (
        18_446_744_073_709_551_615,
        18_446_744_073_709_551_615,
        18_446_744_073_709_551_557,
        116,
        0,
        3_364,
    ),
    (
        18_446_744_073_709_551_556,
        18_446_744_073_709_551_556,
        18_446_744_073_709_551_557,
        18_446_744_073_709_551_555,
        0,
        1,
    ),
    (
        4_293_918_720,
        4_293_918_720,
        4_293_918_721,
        4_293_918_719,
        0,
        1,
    ),
    (
        9_223_372_036_854_775_808,
        9_223_372_036_854_775_813,
        9_223_372_036_854_775_817,
        9_223_372_036_854_775_804,
        9_223_372_036_854_775_812,
        36,
    ),
    (
        12_345_678_901_234_567_890,
        9_876_543_210_987_654_321,
        70_368_744_067_073,
        54_210_816_837_103,
        37_198_421_456_145,
        7_113_368_018_104,
    ),
    (5, 7, 1, 0, 0, 0),
];

/// `(base, exponent, modulus, power)`.
const POWER_VECTORS: &[(u64, u64, u64, u64)] = &[
    (3, 30_592, 30_593, 1),
    (
        2,
        18_446_744_073_709_551_615,
        18_446_744_073_709_551_557,
        576_460_752_303_423_488,
    ),
    (19, 2_146_959_360, 4_293_918_721, 4_293_918_720),
    (7, 0, 30_593, 1),
    (12_345, 65_537, 2_013_265_921, 1_092_586_541),
];

/// `(value, prime modulus, inverse)`.
const INVERSE_VECTORS: &[(u64, u64, u64)] = &[
    (2, 30_593, 15_297),
    (64, 30_593, 30_115),
    (128, 30_593, 30_354),
    (3, 18_446_744_073_709_551_557, 6_148_914_691_236_517_186),
    (1_024, 70_368_744_067_073, 70_300_024_590_445),
];

/// First primitive 128th root found from generator candidates 2, 3, ... for each registered limb.
const REGISTERED_ORDER_128_ROOTS: [u64; 8] =
    [10_567, 4_247, 2_375, 14_967, 28_384, 4_557, 13_827, 22_798];

/// Independent arbitrary-precision reference. Nothing here calls `iroha_fhe`.
mod oracle {
    use num_bigint::{BigInt, BigUint, Sign};

    pub fn big(value: u64) -> BigInt {
        BigInt::from(value)
    }

    pub fn to_u64(value: &BigInt) -> u64 {
        let (sign, digits) = value.to_u64_digits();
        assert_ne!(sign, Sign::Minus, "oracle residue must be non-negative");
        match digits.as_slice() {
            [] => 0,
            [digit] => *digit,
            _ => panic!("oracle residue exceeds one word"),
        }
    }

    pub fn to_u128(value: &BigInt) -> u128 {
        let (sign, digits) = value.to_u64_digits();
        assert_ne!(sign, Sign::Minus, "oracle value must be non-negative");
        match digits.as_slice() {
            [] => 0,
            [low] => u128::from(*low),
            [low, high] => (u128::from(*high) << 64) | u128::from(*low),
            _ => panic!("oracle value exceeds two words"),
        }
    }

    /// Least non-negative residue.
    pub fn rem(value: &BigInt, modulus: u64) -> u64 {
        let modulus = big(modulus);
        to_u64(&(((value % &modulus) + &modulus) % &modulus))
    }

    pub fn pow(base: u64, exponent: u64, modulus: u64) -> u64 {
        let result = BigUint::from(base).modpow(&BigUint::from(exponent), &BigUint::from(modulus));
        to_u64(&BigInt::from(result))
    }

    /// Transform by its definition: `out[k] = sum_i in[i] * root^(i * k)`.
    pub fn dft(input: &[u64], root: u64, modulus: u64) -> Vec<u64> {
        (0..input.len())
            .map(|k| {
                let mut sum = BigInt::from(0);
                for (i, &value) in input.iter().enumerate() {
                    let exponent = u64::try_from(i * k).expect("exponent fits");
                    sum += big(value) * big(pow(root, exponent, modulus));
                }
                rem(&sum, modulus)
            })
            .collect()
    }

    /// Schoolbook product over the integers folded by `X^n = -1`.
    pub fn negacyclic(lhs: &[u64], rhs: &[u64]) -> Vec<BigInt> {
        let n = lhs.len();
        let mut output = vec![BigInt::from(0); n];
        for (i, &left) in lhs.iter().enumerate() {
            for (j, &right) in rhs.iter().enumerate() {
                let term = big(left) * big(right);
                if i + j < n {
                    output[i + j] += term;
                } else {
                    output[i + j - n] -= term;
                }
            }
        }
        output
    }

    /// Linear product over the integers.
    pub fn linear(lhs: &[u64], rhs: &[u64]) -> Vec<BigInt> {
        let mut output = vec![BigInt::from(0); lhs.len() + rhs.len()];
        for (i, &left) in lhs.iter().enumerate() {
            for (j, &right) in rhs.iter().enumerate() {
                output[i + j] += big(left) * big(right);
            }
        }
        output
    }

    /// Constructive CRT: `sum_i r_i * P_i * (P_i^-1 mod q_i) mod P` with `P_i = P / q_i`.
    pub fn crt(residues: &[u64], moduli: &[u64]) -> BigInt {
        let product: BigInt = moduli.iter().map(|&modulus| big(modulus)).product();
        let mut sum = BigInt::from(0);
        for (&residue, &modulus) in residues.iter().zip(moduli) {
            let partial = &product / big(modulus);
            // Fermat inverse modulo the prime limb.
            let inverse = pow(rem(&partial, modulus), modulus - 2, modulus);
            sum += big(residue) * &partial * big(inverse);
        }
        sum % product
    }

    /// `round(numerator / denominator)` to the nearest integer, ties away from zero, decided by
    /// comparing `2 * remainder` with the denominator.
    pub fn round_half_away(numerator: &BigInt, denominator: &BigInt) -> BigInt {
        assert_eq!(denominator.sign(), Sign::Plus);
        let negative = numerator.sign() == Sign::Minus;
        let magnitude = if negative {
            -numerator
        } else {
            numerator.clone()
        };
        let floor = &magnitude / denominator;
        let remainder = &magnitude - &floor * denominator;
        let rounded = if &remainder * 2 >= *denominator {
            floor + 1
        } else {
            floor
        };
        if negative { -rounded } else { rounded }
    }

    pub fn from_i128(value: i128) -> BigInt {
        BigInt::from(value)
    }

    pub fn to_i128(value: &BigInt) -> i128 {
        let negative = value.sign() == Sign::Minus;
        let magnitude = to_u128(&if negative { -value } else { value.clone() });
        let magnitude = i128::try_from(magnitude).expect("oracle value fits i128");
        if negative { -magnitude } else { magnitude }
    }

    /// Centered representative: above `floor(modulus / 2)` is negative.
    pub fn center(value: &BigInt, modulus: &BigInt) -> BigInt {
        if value > &(modulus / 2) {
            value - modulus
        } else {
            value.clone()
        }
    }
}

#[test]
fn scalar_vectors_match_and_agree_with_the_oracle() {
    for &(lhs, rhs, modulus, sum, difference, product) in SCALAR_VECTORS {
        assert_eq!(modular::add_mod_u64(lhs, rhs, modulus), sum);
        assert_eq!(modular::sub_mod_u64(lhs, rhs, modulus), difference);
        assert_eq!(modular::mul_mod_u64(lhs, rhs, modulus), product);
        assert_eq!(
            oracle::rem(&(oracle::big(lhs) + oracle::big(rhs)), modulus),
            sum
        );
        assert_eq!(
            oracle::rem(&(oracle::big(lhs) - oracle::big(rhs)), modulus),
            difference
        );
        assert_eq!(
            oracle::rem(&(oracle::big(lhs) * oracle::big(rhs)), modulus),
            product
        );
    }
    for &(base, exponent, modulus, power) in POWER_VECTORS {
        assert_eq!(modular::mod_pow_u64(base, exponent, modulus), power);
        assert_eq!(oracle::pow(base, exponent, modulus), power);
    }
    for &(value, modulus, inverse) in INVERSE_VECTORS {
        assert_eq!(modular::mod_inv_prime_u64(value, modulus), Some(inverse));
        assert_eq!(
            oracle::rem(&(oracle::big(value) * oracle::big(inverse)), modulus),
            1
        );
    }
    for (&modulus, &root) in REGISTERED_CHAIN.iter().zip(&REGISTERED_ORDER_128_ROOTS) {
        assert!(modular::is_prime_u64(modulus));
        assert_eq!(
            modular::primitive_root_of_order_with_candidate_limit(modulus, 128, 4_096),
            Some(root)
        );
        assert!(modular::is_primitive_root_of_order(modulus, root, 128));
        assert_eq!(oracle::pow(root, 128, modulus), 1);
        assert_eq!(oracle::pow(root, 64, modulus), modulus - 1);
    }
}

#[test]
fn cyclic_ntt_vectors_match_forward_inverse_and_the_definition() {
    for vector in NTT_VECTORS {
        let NttVector {
            modulus,
            root,
            input,
            output,
        } = *vector;
        assert_eq!(
            oracle::dft(input, root, modulus),
            output,
            "pinned output is the definition"
        );
        for transform in [ntt::cyclic_ntt_in_place, ntt::cyclic_ntt_in_place_scalar] {
            let mut values = input.to_vec();
            transform(&mut values, root, modulus, false).expect("forward");
            assert_eq!(values, output, "modulus {modulus} len {}", input.len());
            transform(&mut values, root, modulus, true).expect("inverse");
            assert_eq!(values, input, "round trip");
        }
        let mut generic = input.to_vec();
        ntt::cyclic_ntt_with(&mut generic, &modular::WordModulus(modulus), root);
        assert_eq!(generic, output);
    }
}

#[test]
fn negacyclic_vectors_match_ntt_schoolbook_and_the_integer_oracle() {
    for vector in NEGACYCLIC_VECTORS {
        let NegacyclicVector {
            modulus,
            psi,
            lhs,
            rhs,
            product,
        } = *vector;
        let degree = lhs.len();
        let reference: Vec<u64> = oracle::negacyclic(lhs, rhs)
            .iter()
            .map(|coefficient| oracle::rem(coefficient, modulus))
            .collect();
        assert_eq!(
            reference, product,
            "pinned product is the schoolbook product"
        );
        assert_eq!(
            ntt::negacyclic_multiply_ntt(lhs, rhs, psi, modulus).expect("transform product"),
            product
        );
        assert_eq!(
            polynomial::negacyclic_mul_mod_schoolbook(lhs, rhs, degree, modulus),
            product
        );
        assert_eq!(
            polynomial::reduce_raw_mod(
                &polynomial::negacyclic_mul_raw_schoolbook(lhs, rhs, degree),
                modulus
            ),
            product
        );
        // The transform path is commutative and inverts back through the transform pair.
        assert_eq!(
            ntt::negacyclic_multiply_ntt(rhs, lhs, psi, modulus).expect("commuted"),
            product
        );
        let arithmetic = modular::WordModulus(modulus);
        let mut round_trip = lhs.to_vec();
        ntt::forward_negacyclic_ntt_with(&mut round_trip, &arithmetic, psi);
        ntt::inverse_negacyclic_ntt_with(
            &mut round_trip,
            &arithmetic,
            modular::mod_inv_prime_u64(psi, modulus).expect("psi inverse"),
            modular::mod_inv_prime_u64(degree as u64, modulus).expect("degree inverse"),
        );
        assert_eq!(round_trip, lhs);
    }
}

#[test]
fn crt_vectors_reconstruct_and_agree_with_constructive_crt() {
    assert_eq!(
        rns::checked_modulus_product(&REGISTERED_CHAIN),
        Ok(REGISTERED_PRODUCT)
    );
    assert_eq!(
        oracle::to_u128(
            &REGISTERED_CHAIN
                .iter()
                .map(|&modulus| oracle::big(modulus))
                .product()
        ),
        REGISTERED_PRODUCT
    );
    for vector in CRT_VECTORS {
        assert_eq!(
            rns::reconstruct_coefficient(&vector.residues, &REGISTERED_CHAIN),
            Ok(vector.value)
        );
        assert_eq!(
            oracle::to_u128(&oracle::crt(&vector.residues, &REGISTERED_CHAIN)),
            vector.value
        );
        for (&residue, &modulus) in vector.residues.iter().zip(&REGISTERED_CHAIN) {
            assert_eq!(u128::from(residue), vector.value % u128::from(modulus));
        }
    }
    // Limb-major polynomial reconstruction over the same vectors.
    let limbs: Vec<Vec<u64>> = (0..REGISTERED_CHAIN.len())
        .map(|limb| {
            CRT_VECTORS
                .iter()
                .map(|vector| vector.residues[limb])
                .collect()
        })
        .collect();
    let values: Vec<u128> = CRT_VECTORS.iter().map(|vector| vector.value).collect();
    assert_eq!(
        rns::reconstruct_polynomial(&limbs, &REGISTERED_CHAIN, CRT_VECTORS.len()),
        Ok(values)
    );
}

#[test]
fn basis_extension_vectors_match_exact_and_centered_conversion() {
    let source = &REGISTERED_CHAIN[..3];
    assert_eq!(
        rns::checked_modulus_product(source),
        Ok(BASIS_SOURCE_PRODUCT)
    );
    let degree = BASIS_VALUES.len();
    let limbs: Vec<Vec<u64>> = BASIS_SOURCE_RESIDUES
        .iter()
        .map(|limb| limb.to_vec())
        .collect();
    let expected: Vec<Vec<u64>> = BASIS_TARGET_RESIDUES
        .iter()
        .map(|limb| limb.to_vec())
        .collect();
    let expected_centered: Vec<Vec<u64>> = BASIS_CENTERED_TARGET_RESIDUES
        .iter()
        .map(|limb| limb.to_vec())
        .collect();

    // Quotient-corrected target-limb extension.
    assert_eq!(
        rns::basis_extend_target_limbs(&limbs, source, &BASIS_TARGET, degree),
        Ok(expected.clone())
    );
    // Reconstructing extension, canonical and centered.
    let coefficients = rns::reconstruct_polynomial(&limbs, source, degree).expect("reconstruct");
    assert_eq!(coefficients, BASIS_VALUES);
    assert_eq!(
        rns::reduce_into_limbs(&coefficients, &BASIS_TARGET),
        Ok(expected)
    );
    assert_eq!(
        rns::reduce_centered_into_limbs(&coefficients, BASIS_SOURCE_PRODUCT, &BASIS_TARGET),
        Ok(expected_centered)
    );

    // Oracle: constructive CRT, then residues of the canonical and the centered integer.
    let source_product = oracle::from_i128(i128::try_from(BASIS_SOURCE_PRODUCT).expect("fits"));
    for index in 0..degree {
        let residues: Vec<u64> = BASIS_SOURCE_RESIDUES
            .iter()
            .map(|limb| limb[index])
            .collect();
        let value = oracle::crt(&residues, source);
        assert_eq!(oracle::to_u128(&value), BASIS_VALUES[index]);
        let centered = oracle::center(&value, &source_product);
        for (limb, &modulus) in BASIS_TARGET.iter().enumerate() {
            assert_eq!(
                oracle::rem(&value, modulus),
                BASIS_TARGET_RESIDUES[limb][index]
            );
            assert_eq!(
                oracle::rem(&centered, modulus),
                BASIS_CENTERED_TARGET_RESIDUES[limb][index]
            );
        }
    }
    // The exact half boundary: floor(P / 2) stays positive and floor(P / 2) + 1 is negative.
    assert_eq!(BASIS_VALUES[3], BASIS_SOURCE_PRODUCT / 2);
    assert_eq!(BASIS_VALUES[4], BASIS_SOURCE_PRODUCT / 2 + 1);
    assert_eq!(
        BASIS_CENTERED_TARGET_RESIDUES[0][3],
        BASIS_TARGET_RESIDUES[0][3]
    );
    assert_ne!(
        BASIS_CENTERED_TARGET_RESIDUES[0][4],
        BASIS_TARGET_RESIDUES[0][4]
    );
}

#[test]
fn rounding_vectors_match_at_exact_halves_and_boundaries() {
    for &(coefficient, numerator, denominator, rounded) in SCALE_ROUND_VECTORS {
        assert_eq!(
            rounding::scale_round_centered(coefficient, numerator, denominator),
            Ok(rounded),
            "{coefficient} * {numerator} / {denominator}"
        );
        let exact = oracle::from_i128(coefficient) * oracle::big(numerator);
        assert_eq!(
            oracle::to_i128(&oracle::round_half_away(&exact, &oracle::big(denominator))),
            rounded
        );
    }
    for &(value, from, to, switched) in MODULUS_SWITCH_VECTORS {
        assert_eq!(
            rounding::modulus_switch_round(value, from, to),
            Ok(switched),
            "{value} from {from} to {to}"
        );
        let lifted = oracle::center(&oracle::big(value), &oracle::big(from));
        assert_eq!(oracle::to_i128(&lifted), rounding::center_lift(value, from));
        let rounded = oracle::round_half_away(&(lifted * oracle::big(to)), &oracle::big(from));
        assert_eq!(oracle::rem(&rounded, to), switched);
    }
    // The registered moduli: the constants above are what the vectors use, and the half point
    // is where the rounding rule decides.
    assert_eq!(
        u128::from(REGISTERED_CIPHERTEXT_MODULUS),
        u128::from(REGISTERED_PLAINTEXT_MODULUS) << 48
    );
    assert_eq!(REGISTERED_HALF_POINT, 1 << 47);
    let registered = SCALE_ROUND_VECTORS
        .iter()
        .filter(|vector| vector.2 == REGISTERED_CIPHERTEXT_MODULUS)
        .count();
    assert_eq!(registered, 16, "registered scale-and-round vectors");
    for (coefficient, rounded) in [
        (REGISTERED_HALF_POINT - 1, 0),
        (REGISTERED_HALF_POINT, 1),
        (-REGISTERED_HALF_POINT, -1),
        (-REGISTERED_HALF_POINT + 1, 0),
    ] {
        assert_eq!(
            rounding::scale_round_centered(
                coefficient,
                REGISTERED_PLAINTEXT_MODULUS,
                REGISTERED_CIPHERTEXT_MODULUS
            ),
            Ok(rounded),
            "registered half point, coefficient {coefficient}"
        );
    }
    let registered = MODULUS_SWITCH_VECTORS
        .iter()
        .filter(|vector| vector.1 == REGISTERED_CIPHERTEXT_MODULUS)
        .count();
    assert_eq!(registered, 36, "registered modulus-switch vectors");
    for &(value, divisor, rounded) in DIV_ROUND_VECTORS {
        assert_eq!(
            rounding::div_round_nearest_i128(value, divisor),
            Ok(rounded)
        );
        assert_eq!(
            oracle::to_i128(&oracle::round_half_away(
                &oracle::from_i128(value),
                &oracle::from_i128(divisor)
            )),
            rounded
        );
    }
    // Ceiling division at exact multiples and one above.
    for (numerator, denominator, expected) in
        [(0_u128, 7_u128, 0_u128), (7, 7, 1), (8, 7, 2), (14, 7, 2)]
    {
        assert_eq!(
            rounding::ceil_div_u128(numerator, denominator),
            Ok(expected)
        );
    }
}

#[test]
fn exact_convolution_vectors_match_the_integer_products() {
    let linear = oracle::linear(&CONVOLUTION_LHS, &CONVOLUTION_RHS);
    let linear: Vec<u128> = linear.iter().map(oracle::to_u128).collect();
    assert_eq!(linear, CONVOLUTION_LINEAR);
    let folded: Vec<i128> = oracle::negacyclic(&CONVOLUTION_LHS, &CONVOLUTION_RHS)
        .iter()
        .map(oracle::to_i128)
        .collect();
    assert_eq!(folded, CONVOLUTION_FOLDED);
    assert_eq!(
        ntt::convolve_linear_crt_ntt(&CONVOLUTION_LHS, &CONVOLUTION_RHS),
        Some(CONVOLUTION_LINEAR.to_vec())
    );
    assert_eq!(
        ntt::negacyclic_product_raw_crt_ntt(&CONVOLUTION_LHS, &CONVOLUTION_RHS),
        Some(CONVOLUTION_FOLDED.to_vec())
    );
    assert_eq!(
        polynomial::negacyclic_mul_raw_schoolbook(&CONVOLUTION_LHS, &CONVOLUTION_RHS, 4),
        CONVOLUTION_FOLDED
    );
}

#[test]
fn automorphism_and_digit_vectors_match() {
    let automorphism_sets = [
        (
            AUTOMORPHISM_MODULUS,
            &AUTOMORPHISM_INPUT,
            AUTOMORPHISM_VECTORS,
        ),
        (
            REGISTERED_CIPHERTEXT_MODULUS,
            &REGISTERED_AUTOMORPHISM_INPUT,
            REGISTERED_AUTOMORPHISM_VECTORS,
        ),
    ];
    for (modulus, input, vectors) in automorphism_sets {
        assert!(input.iter().all(|&coefficient| coefficient < modulus));
        for &(power, image) in vectors {
            let power = automorphism::validate_power(8, power).expect("unit power");
            let mut output = [0_u64; 8];
            automorphism::apply_into(input, power, modulus, &mut output).expect("apply");
            assert_eq!(output, image, "modulus {modulus} power {power}");
            // Oracle: substitute X -> X^power and fold by X^8 = -1.
            let mut signed = vec![oracle::from_i128(0); 8];
            for (index, &coefficient) in input.iter().enumerate() {
                let exponent = index * power;
                if (exponent / 8).is_multiple_of(2) {
                    signed[exponent % 8] += oracle::big(coefficient);
                } else {
                    signed[exponent % 8] -= oracle::big(coefficient);
                }
            }
            let reference: Vec<u64> = signed
                .iter()
                .map(|coefficient| oracle::rem(coefficient, modulus))
                .collect();
            assert_eq!(reference, image, "modulus {modulus} power {power}");
        }
    }
    let small_digits: Vec<Vec<u64>> = DIGIT_VECTORS.iter().map(|digit| digit.to_vec()).collect();
    let registered_digits: Vec<Vec<u64>> = REGISTERED_DIGIT_VECTORS
        .iter()
        .map(|digit| digit.to_vec())
        .collect();
    let small_input: &[u64] = &DIGIT_INPUT;
    let registered_input: &[u64] = &REGISTERED_DIGIT_INPUT;
    for (input, expected, modulus) in [
        (small_input, small_digits, AUTOMORPHISM_MODULUS),
        (
            registered_input,
            registered_digits,
            REGISTERED_CIPHERTEXT_MODULUS,
        ),
    ] {
        assert!(input.iter().all(|&coefficient| coefficient < modulus));
        let digits = key_switch::decompose_digits(input, input.len(), 1 << 12, expected.len())
            .expect("digits");
        assert_eq!(digits, expected, "modulus {modulus}");
        for (index, &coefficient) in input.iter().enumerate() {
            let recomposed = expected
                .iter()
                .rev()
                .fold(oracle::from_i128(0), |value, digit| {
                    value * oracle::big(1 << 12) + oracle::big(digit[index])
                });
            assert_eq!(oracle::to_u128(&recomposed), u128::from(coefficient));
        }
    }
}

/// Every dispatching slice kernel returns the words of its scalar reference on the canonical
/// vectors, whatever backend this build and host select.
#[test]
fn accelerated_paths_match_the_scalar_reference_on_canonical_vectors() {
    type Kernel = fn(&mut [u64], &[u64], u64);
    type Reference = fn(&oracle_ops::Pair) -> u64;
    let kernels: [(Kernel, Kernel, Reference); 3] = [
        (
            accel::add_mod_assign,
            accel::add_mod_assign_scalar,
            oracle_ops::add,
        ),
        (
            accel::sub_mod_assign,
            accel::sub_mod_assign_scalar,
            oracle_ops::sub,
        ),
        (
            accel::mul_mod_assign,
            accel::mul_mod_assign_scalar,
            oracle_ops::mul,
        ),
    ];
    for vector in NEGACYCLIC_VECTORS {
        for (dispatching, scalar, reference) in kernels {
            let mut actual = vector.lhs.to_vec();
            let mut expected = vector.lhs.to_vec();
            dispatching(&mut actual, vector.rhs, vector.modulus);
            scalar(&mut expected, vector.rhs, vector.modulus);
            assert_eq!(actual, expected, "modulus {}", vector.modulus);
            for (index, &word) in actual.iter().enumerate() {
                let pair = oracle_ops::Pair {
                    lhs: vector.lhs[index],
                    rhs: vector.rhs[index],
                    modulus: vector.modulus,
                };
                assert_eq!(word, reference(&pair));
            }
        }
        let mut actual = vector.lhs.to_vec();
        let mut expected = vector.lhs.to_vec();
        accel::mul_scalar_mod(&mut actual, vector.psi, vector.modulus);
        accel::mul_scalar_mod_scalar(&mut expected, vector.psi, vector.modulus);
        assert_eq!(actual, expected);
    }
    for vector in NTT_VECTORS {
        let mut dispatched = vector.input.to_vec();
        let mut scalar = vector.input.to_vec();
        ntt::cyclic_ntt_in_place(&mut dispatched, vector.root, vector.modulus, false)
            .expect("forward");
        ntt::cyclic_ntt_in_place_scalar(&mut scalar, vector.root, vector.modulus, false)
            .expect("scalar");
        assert_eq!(dispatched, scalar);
        assert_eq!(dispatched, vector.output);
    }
    // The report names a backend this build can actually select.
    let selectable = match accel::active_backend() {
        accel::Backend::Scalar => true,
        accel::Backend::Neon => cfg!(all(feature = "simd", target_arch = "aarch64")),
        accel::Backend::Avx2 => cfg!(all(feature = "simd", target_arch = "x86_64")),
    };
    assert!(
        selectable,
        "reported backend is not compiled into this build"
    );
}

/// Word operations of the oracle, used to check accelerated slices element by element.
mod oracle_ops {
    use super::oracle;

    pub struct Pair {
        pub lhs: u64,
        pub rhs: u64,
        pub modulus: u64,
    }

    pub fn add(pair: &Pair) -> u64 {
        oracle::rem(
            &(oracle::big(pair.lhs) + oracle::big(pair.rhs)),
            pair.modulus,
        )
    }

    pub fn sub(pair: &Pair) -> u64 {
        oracle::rem(
            &(oracle::big(pair.lhs) - oracle::big(pair.rhs)),
            pair.modulus,
        )
    }

    pub fn mul(pair: &Pair) -> u64 {
        oracle::rem(
            &(oracle::big(pair.lhs) * oracle::big(pair.rhs)),
            pair.modulus,
        )
    }
}
