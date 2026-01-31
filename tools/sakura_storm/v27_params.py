from dataclasses import dataclass
from typing import List, Tuple


@dataclass(frozen=True)
class V27Params:
    # Canvas / timing
    w: int = 512
    h: int = 512
    frames: int = 96
    fps: int = 25

    # Grid / masks
    grid_n: int = 32
    cell_size: int = 16
    grid_offset: int = 0
    data_radius: float = 243.0
    outer_radius: float = 245.0

    # Colors (v27 palette)
    bg: Tuple[int, int, int] = (11, 7, 12)
    data_bright: Tuple[int, int, int] = (255, 233, 246)
    data_dim: Tuple[int, int, int] = (69, 40, 54)
    ring_bright: Tuple[int, int, int] = (255, 246, 252)
    ring_dim: Tuple[int, int, int] = (62, 36, 50)
    logo_shades: Tuple[Tuple[int, int, int], Tuple[int, int, int], Tuple[int, int, int]] = (
        (30, 16, 25),
        (36, 18, 28),
        (43, 20, 32),
    )

    # Logo thresholds (alpha -> shade)
    logo_scale: float = 0.318
    logo_thresh_1: int = 185
    logo_thresh_2: int = 229
    logo_thresh_3: int = 247

    # Glyphs
    glyph_size: int = 15
    glyph_thresh: int = 96
    font_index: int = 0
    glyph_seed: int = 1337

    # Ring geometry
    ring_radii: Tuple[float, float, float] = (190.0, 214.0, 238.0)
    ring_spacing: float = 37.0
    ring_dot_r: int = 7
    ring_dim_angles: Tuple[float, float, float, float] = (0.0, 90.0, 180.0, 270.0)

    # Coding (RA)
    symbol_period: int = 1
    bits_per_symbol: int = 1
    ldpc_rate: float = 0.25
    ldpc_row_w: int = 4
    source_seed: int = 1234
    ra_seed: int = 2027


V27 = V27Params()

# Iroha ordering (full-width), including archaic kana ヰ/ヱ.
KATAKANA_IROHA_BASE: List[str] = [
    "イ",
    "ロ",
    "ハ",
    "ニ",
    "ホ",
    "ヘ",
    "ト",
    "チ",
    "リ",
    "ヌ",
    "ル",
    "ヲ",
    "ワ",
    "カ",
    "ヨ",
    "タ",
    "レ",
    "ソ",
    "ツ",
    "ネ",
    "ナ",
    "ラ",
    "ム",
    "ウ",
    "ヰ",
    "ノ",
    "オ",
    "ク",
    "ヤ",
    "マ",
    "ケ",
    "フ",
    "コ",
    "エ",
    "テ",
    "ア",
    "サ",
    "キ",
    "ユ",
    "メ",
    "ミ",
    "シ",
    "ヱ",
    "ヒ",
    "モ",
    "セ",
    "ス",
]

# Extra/archaic katakana used in Iroha-era and historical orthography.
# Includes yi/ye digraphs ("イィ", "イェ") and small/voiced variants.
KATAKANA_EXTRA: List[str] = [
    "ヮ",
    "ヵ",
    "ヶ",
    "ヷ",
    "ヸ",
    "ヹ",
    "ヺ",
    "ヿ",
    "イィ",
    "イェ",
]

KATAKANA: List[str] = KATAKANA_IROHA_BASE + KATAKANA_EXTRA

# V27 exact reproduction list (modern gojūon order).
KATAKANA_V27: List[str] = list(
    "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン"
)

# Palette presets for quick recolor (v27 defaults + v26-like crimson).
PALETTE_PRESETS = {
    "v27": {
        "bg": V27.bg,
        "data_bright": V27.data_bright,
        "data_dim": V27.data_dim,
        "ring_bright": V27.ring_bright,
        "ring_dim": V27.ring_dim,
        "logo_shades": V27.logo_shades,
    },
    "v27_pink": {
        "bg": V27.bg,
        "data_bright": V27.data_bright,
        "data_dim": V27.data_dim,
        "ring_bright": V27.ring_bright,
        "ring_dim": V27.ring_dim,
        "logo_shades": V27.logo_shades,
    },
    # Extracted from the v26 fullwidth katakana preview palette.
    "v26_preview": {
        "bg": (12, 9, 14),
        "data_bright": (255, 225, 240),
        "data_dim": (118, 70, 90),
        "ring_bright": (255, 244, 250),
        "ring_dim": (92, 56, 72),
        "logo_shades": ((29, 16, 24), (36, 18, 28), (43, 20, 32)),
    },
    # Derived from the v26 "defi_crimson" scheme (BG/CELL_BG/RING/GLYPH).
    "defi_crimson": {
        "bg": (6, 7, 10),
        "data_bright": (255, 60, 60),
        "data_dim": (102, 24, 24),
        "ring_bright": (255, 80, 80),
        "ring_dim": (24, 10, 14),
        "logo_shades": ((14, 14, 18), (20, 18, 22), (26, 22, 26)),
    },
    "v26_crimson": {
        "bg": (6, 7, 10),
        "data_bright": (255, 60, 60),
        "data_dim": (102, 24, 24),
        "ring_bright": (255, 80, 80),
        "ring_dim": (24, 10, 14),
        "logo_shades": ((14, 14, 18), (20, 18, 22), (26, 22, 26)),
    },
    "defi_crimson_bold": {
        "bg": (5, 5, 8),
        "data_bright": (255, 90, 90),
        "data_dim": (120, 28, 28),
        "ring_bright": (255, 110, 110),
        "ring_dim": (32, 12, 16),
        "logo_shades": ((20, 18, 22), (32, 24, 30), (48, 30, 38)),
    },
    "v26_ink": {
        "bg": (6, 6, 9),
        "data_bright": (255, 100, 130),
        "data_dim": (96, 24, 34),
        "ring_bright": (255, 130, 160),
        "ring_dim": (30, 12, 18),
        "logo_shades": ((18, 16, 20), (28, 20, 26), (38, 24, 32)),
    },
    "v26_teal": {
        "bg": (6, 8, 12),
        "data_bright": (255, 80, 80),
        "data_dim": (100, 24, 24),
        "ring_bright": (160, 250, 240),
        "ring_dim": (10, 20, 22),
        "logo_shades": ((16, 18, 24), (24, 24, 32), (34, 30, 40)),
    },
    "v26_mono": {
        "bg": (6, 6, 8),
        "data_bright": (255, 70, 70),
        "data_dim": (90, 20, 20),
        "ring_bright": (255, 90, 90),
        "ring_dim": (24, 10, 14),
        "logo_shades": ((18, 12, 16), (28, 16, 20), (40, 20, 26)),
    },
    "logo_forward": {
        "bg": V27.bg,
        "data_bright": V27.data_bright,
        "data_dim": V27.data_dim,
        "ring_bright": (245, 225, 238),
        "ring_dim": (70, 40, 55),
        "logo_shades": ((80, 40, 60), (110, 60, 85), (150, 90, 120)),
    },
}
