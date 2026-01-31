import math
import os
import random
from typing import List, Optional, Tuple

import numpy as np
from PIL import Image, ImageDraw, ImageFont

# Reverse-engineered v27: exact match when defaults are used.

from v27_params import KATAKANA, KATAKANA_V27, PALETTE_PRESETS, V27

BASE_DIR = os.path.dirname(__file__)
OUT_GIF = os.getenv(
    "SS_OUT_GIF",
    "/tmp/sakura_storm_viz/sakura_storm_v27_procedural.gif"
    if os.path.isdir("/tmp/sakura_storm_viz")
    else os.path.join(BASE_DIR, "sakura_storm_v27_procedural.gif"),
)

W = int(os.getenv("SS_W", str(V27.w)))
H = int(os.getenv("SS_H", str(V27.h)))
FRAMES = int(os.getenv("SS_FRAMES", str(V27.frames)))
FPS = int(os.getenv("SS_FPS", str(V27.fps)))

# Layout (reverse-engineered defaults)
GRID_N = int(os.getenv("SS_GRID_N", str(V27.grid_n)))
CELL_SIZE = int(os.getenv("SS_CELL_SIZE", str(V27.cell_size)))
GRID_OFFSET = int(os.getenv("SS_GRID_OFFSET", str(V27.grid_offset)))
DATA_RADIUS = float(os.getenv("SS_DATA_RADIUS", str(V27.data_radius)))
OUTER_RADIUS = float(os.getenv("SS_OUTER_RADIUS", str(V27.outer_radius)))
DATA_DENSITY = float(os.getenv("SS_DATA_DENSITY", "1.0"))
ACTIVE_BANDS = os.getenv("SS_ACTIVE_BANDS", "").strip()
MASK_LOGO = os.getenv("SS_MASK_LOGO", "0").strip() == "1"
LOGO_MASK_THRESH = int(os.getenv("SS_LOGO_MASK_THRESH", str(V27.logo_thresh_1)))
DYNAMIC_RADIUS = float(os.getenv("SS_DYNAMIC_RADIUS", "0.0"))
DYNAMIC_POOL = os.getenv("SS_DYNAMIC_POOL", "low").strip().lower()
STATIC_POOL = os.getenv("SS_STATIC_POOL", "low").strip().lower()
STATIC_GLYPH_SEED = int(os.getenv("SS_STATIC_GLYPH_SEED", "9001"))
BITS_PER_SYMBOL = int(os.getenv("SS_BITS_PER_SYMBOL", "1"))
KATAKANA_MODE = os.getenv("SS_KATAKANA_MODE", "v27").strip().lower()
PALETTE_PRESET = os.getenv("SS_PALETTE_PRESET", "").strip().lower()
KATAKANA_OVERLAY = os.getenv("SS_KATAKANA_OVERLAY", "0").strip() == "1"
KATAKANA_NONDATA = os.getenv("SS_KATAKANA_NONDATA", "0").strip() == "1"
KATAKANA_DIFF = os.getenv("SS_KATAKANA_DIFF", "0").strip() == "1"
KATAKANA_ALPHA = int(os.getenv("SS_KATAKANA_ALPHA", "80"))
KATAKANA_SCALE = float(os.getenv("SS_KATAKANA_SCALE", "0.8"))
KATAKANA_SEED = int(os.getenv("SS_KATAKANA_SEED", "1337"))
KATAKANA_BOXES = os.getenv("SS_KATAKANA_BOXES", "0").strip() == "1"
KATAKANA_BOX_ALPHA = int(os.getenv("SS_KATAKANA_BOX_ALPHA", "80"))
KATAKANA_BOX_SCALE = float(os.getenv("SS_KATAKANA_BOX_SCALE", "0.9"))
KATAKANA_COLOR = os.getenv("SS_KATAKANA_COLOR", "")
KATAKANA_BOX_ON = os.getenv("SS_KATAKANA_BOX_ON", "")
KATAKANA_BOX_OFF = os.getenv("SS_KATAKANA_BOX_OFF", "")
KATAKANA_TEXT_ON = os.getenv("SS_KATAKANA_TEXT_ON", "")
KATAKANA_TEXT_OFF = os.getenv("SS_KATAKANA_TEXT_OFF", "")

def resolve_palette() -> Tuple[
    Tuple[int, int, int],
    Tuple[int, int, int],
    Tuple[int, int, int],
    Tuple[int, int, int],
    Tuple[int, int, int],
    List[Tuple[int, int, int]],
]:
    bg = V27.bg
    data_bright = V27.data_bright
    data_dim = V27.data_dim
    ring_bright = V27.ring_bright
    ring_dim = V27.ring_dim
    logo_shades = list(V27.logo_shades)
    if PALETTE_PRESET:
        preset = PALETTE_PRESETS.get(PALETTE_PRESET)
        if preset is None:
            raise ValueError(f"unknown PALETTE_PRESET {PALETTE_PRESET!r}")
        bg = preset["bg"]
        data_bright = preset["data_bright"]
        data_dim = preset["data_dim"]
        ring_bright = preset["ring_bright"]
        ring_dim = preset["ring_dim"]
        logo_shades = list(preset["logo_shades"])
    return bg, data_bright, data_dim, ring_bright, ring_dim, logo_shades


# Simple "R,G,B" parser for overlay colors.
def parse_color(text: str, default: Tuple[int, int, int]) -> Tuple[int, int, int]:
    if not text:
        return default
    parts = [p.strip() for p in text.split(",") if p.strip()]
    if len(parts) != 3:
        return default
    try:
        vals = tuple(int(max(0, min(255, int(p)))) for p in parts)
    except ValueError:
        return default
    return vals


# Palette (default v27 unless overridden)
BG, DATA_BRIGHT, DATA_DIM, RING_BRIGHT, RING_DIM, LOGO_SHADES = resolve_palette()

PALETTE = np.array(
    [
        BG,
        LOGO_SHADES[0],
        LOGO_SHADES[1],
        LOGO_SHADES[2],
        RING_DIM,
        DATA_DIM,
        DATA_BRIGHT,
        RING_BRIGHT,
    ],
    dtype=np.uint8,
)

# Glyphs
FONT_PATH = os.getenv("SS_FONT_PATH", "/System/Library/Fonts/Hiragino Sans GB.ttc")
FONT_INDEX = int(os.getenv("SS_FONT_INDEX", str(V27.font_index)))
GLYPH_SIZE = int(os.getenv("SS_GLYPH_SIZE", str(V27.glyph_size)))
GLYPH_THRESH = int(os.getenv("SS_GLYPH_THRESH", str(V27.glyph_thresh)))
GLYPH_POOL_MODE = os.getenv("SS_GLYPH_POOL_MODE", "auto").strip().lower()
GLYPH_SEED = int(os.getenv("SS_GLYPH_SEED", str(V27.glyph_seed)))
KATAKANA_FONT_PATH = os.getenv("SS_KATAKANA_FONT_PATH", FONT_PATH)
KATAKANA_FONT_INDEX = int(os.getenv("SS_KATAKANA_FONT_INDEX", str(FONT_INDEX)))

# Rings
RING_RADII = [float(x) for x in os.getenv("SS_RING_RADII", ",".join(str(x) for x in V27.ring_radii)).split(",") if x.strip()]
RING_SPACING = float(os.getenv("SS_RING_SPACING", str(V27.ring_spacing)))
RING_DOT_R = int(os.getenv("SS_RING_DOT_R", str(V27.ring_dot_r)))
RING_DIM_ANGLES = [float(x) for x in os.getenv("SS_RING_DIM_ANGLES", "0,90,180,270").split(",") if x.strip()]
RING_DIM_TOL = float(os.getenv("SS_RING_DIM_TOL", "4.0"))

# Logo
LOGO_PATH = os.getenv(
    "SS_LOGO_PATH",
    "/Users/mtakemiya/Library/CloudStorage/Dropbox/soramitsu/tarmo/sora_logo.png",
)
LOGO_SCALE = float(os.getenv("SS_LOGO_SCALE", str(V27.logo_scale)))
LOGO_ALPHA = int(os.getenv("SS_LOGO_ALPHA", "255"))
LOGO_THRESH_1 = int(os.getenv("SS_LOGO_THRESH_1", str(V27.logo_thresh_1)))
LOGO_THRESH_2 = int(os.getenv("SS_LOGO_THRESH_2", str(V27.logo_thresh_2)))
LOGO_THRESH_3 = int(os.getenv("SS_LOGO_THRESH_3", str(V27.logo_thresh_3)))


# Coding
SYMBOL_PERIOD = int(os.getenv("SS_SYMBOL_PERIOD", str(V27.symbol_period)))
LDPC_RATE = float(os.getenv("SS_LDPC_RATE", str(V27.ldpc_rate)))
LDPC_ROW_W = int(os.getenv("SS_LDPC_ROW_W", str(V27.ldpc_row_w)))
SOURCE_SEED = int(os.getenv("SS_SOURCE_SEED", str(V27.source_seed)))
RA_SEED = int(os.getenv("SS_RA_SEED", str(V27.ra_seed)))


def build_cells() -> List[Tuple[int, int, float]]:
    cells = []
    cx = W / 2.0
    cy = H / 2.0
    for r in range(GRID_N):
        y0 = GRID_OFFSET + r * CELL_SIZE
        for c in range(GRID_N):
            x0 = GRID_OFFSET + c * CELL_SIZE
            if x0 < 0 or y0 < 0 or x0 + CELL_SIZE > W or y0 + CELL_SIZE > H:
                continue
            cx_cell = x0 + CELL_SIZE / 2.0
            cy_cell = y0 + CELL_SIZE / 2.0
            rr = math.hypot(cx_cell - cx, cy_cell - cy)
            cells.append((x0, y0, rr))
    return cells


def parse_bands(spec: str) -> List[Tuple[float, float]]:
    bands: List[Tuple[float, float]] = []
    if not spec:
        return bands
    for part in spec.split(","):
        part = part.strip()
        if not part:
            continue
        if "-" not in part:
            raise ValueError(f"bad band spec {part!r}")
        lo_s, hi_s = part.split("-", 1)
        bands.append((float(lo_s), float(hi_s)))
    return bands


def build_ra_code(k: int, n: int, row_w: int, seed: int = 2026):
    if n <= k:
        raise ValueError("n must be > k for parity checks")
    m = n - k
    rng = random.Random(seed)
    rows = []
    for _ in range(m):
        w = min(row_w, k)
        rows.append(rng.sample(range(k), w))
    return rows


def ra_encode(message: np.ndarray, rows: List[List[int]]) -> np.ndarray:
    m = len(rows)
    parity = np.zeros(m, dtype=np.uint8)
    prev = 0
    for i, idxs in enumerate(rows):
        s = 0
        for j in idxs:
            s ^= int(message[j])
        parity[i] = s ^ prev
        prev = parity[i]
    return np.concatenate([message, parity])


def render_glyph_alpha(font: ImageFont.FreeTypeFont, glyph: str) -> np.ndarray:
    img = Image.new("L", (CELL_SIZE, CELL_SIZE), 0)
    draw = ImageDraw.Draw(img)
    bbox = draw.textbbox((0, 0), glyph, font=font)
    gw = bbox[2] - bbox[0]
    gh = bbox[3] - bbox[1]
    tx = CELL_SIZE / 2 - gw / 2
    ty = CELL_SIZE / 2 - gh / 2
    draw.text((tx, ty), glyph, font=font, fill=255)
    return np.array(img)


def resolve_katakana() -> List[str]:
    if KATAKANA_MODE in ("v27", "legacy"):
        return KATAKANA_V27[:]
    if KATAKANA_MODE in ("iroha", "iroha_extra"):
        return KATAKANA[:]
    raise ValueError(f"unknown KATAKANA_MODE {KATAKANA_MODE!r}")


def build_glyph_pools(font: ImageFont.FreeTypeFont, glyphs: List[str]) -> Tuple[List[str], List[str], List[str], List[str]]:
    densities = []
    for g in glyphs:
        alpha = render_glyph_alpha(font, g)
        m = alpha > GLYPH_THRESH
        densities.append((g, float(np.mean(m))))
    densities.sort(key=lambda x: x[1])
    low = [g for g, _ in densities[: max(4, len(densities) // 3)]]
    low_extreme = [g for g, _ in densities[:4]]
    high = [g for g, _ in densities[-max(4, len(densities) // 3) :]]
    mid_start = len(densities) // 3
    mid_end = len(densities) - mid_start
    mid = [g for g, _ in densities[mid_start:mid_end]]
    if GLYPH_POOL_MODE in ("auto", ""):
        return low, high, mid, glyphs[:]
    if GLYPH_POOL_MODE in ("all", "single_all"):
        return glyphs[:], glyphs[:], glyphs[:], glyphs[:]
    if GLYPH_POOL_MODE in ("single_low", "low"):
        return low, low, low, low
    if GLYPH_POOL_MODE in ("single_low_extreme", "low_extreme"):
        return low_extreme, low_extreme, low_extreme, low_extreme
    if GLYPH_POOL_MODE in ("lowhigh", "low_high"):
        return low, high, mid, glyphs[:]
    if GLYPH_POOL_MODE in ("mid", "single_mid"):
        return mid, mid, mid, mid
    if GLYPH_POOL_MODE in ("high", "single_high"):
        return high, high, high, high
    raise ValueError(f"unknown GLYPH_POOL_MODE {GLYPH_POOL_MODE!r}")


def build_logo_layer() -> Image.Image:
    layer = np.zeros((H, W, 3), dtype=np.uint8)
    layer[:] = BG
    if not os.path.exists(LOGO_PATH):
        return Image.fromarray(layer, mode="RGB")
    logo = Image.open(LOGO_PATH).convert("RGBA")
    target = int(min(W, H) * LOGO_SCALE)
    target = max(8, min(min(W, H), target))
    logo = logo.resize((target, target), resample=Image.LANCZOS)
    arr = np.array(logo, dtype=np.uint8)
    alpha = arr[..., 3].astype(np.float32)
    alpha = np.clip(alpha * (LOGO_ALPHA / 255.0), 0.0, 255.0).astype(np.uint8)
    x0 = int(W / 2 - target / 2)
    y0 = int(H / 2 - target / 2)
    region = layer[y0 : y0 + target, x0 : x0 + target]
    # Map alpha to discrete shades (no blending) to match v27.
    region[alpha >= LOGO_THRESH_1] = LOGO_SHADES[0]
    region[alpha >= LOGO_THRESH_2] = LOGO_SHADES[1]
    region[alpha >= LOGO_THRESH_3] = LOGO_SHADES[2]
    return Image.fromarray(layer, mode="RGB")


def build_logo_mask() -> Optional[np.ndarray]:
    if not MASK_LOGO or not os.path.exists(LOGO_PATH):
        return None
    logo = Image.open(LOGO_PATH).convert("RGBA")
    target = int(min(W, H) * LOGO_SCALE)
    target = max(8, min(min(W, H), target))
    logo = logo.resize((target, target), resample=Image.LANCZOS)
    alpha = np.array(logo, dtype=np.uint8)[..., 3]
    mask = np.zeros((H, W), dtype=bool)
    x0 = int(W / 2 - target / 2)
    y0 = int(H / 2 - target / 2)
    mask[y0 : y0 + target, x0 : x0 + target] = alpha >= LOGO_MASK_THRESH
    return mask


def build_ring_layer() -> Image.Image:
    layer = Image.new("RGB", (W, H), (0, 0, 0))
    draw = ImageDraw.Draw(layer)
    for r in RING_RADII:
        n = max(8, int(round((2.0 * math.pi * r) / max(1.0, RING_SPACING))))
        for k in range(n):
            ang = 2.0 * math.pi * k / n
            x = W / 2 + r * math.cos(ang)
            y = H / 2 + r * math.sin(ang)
            color = RING_BRIGHT
            if abs(r - 214.0) < 0.8:
                deg = (math.degrees(ang) + 360.0) % 360.0
                for a in RING_DIM_ANGLES:
                    if abs((deg - a + 180.0) % 360.0 - 180.0) < RING_DIM_TOL:
                        color = RING_DIM
                        break
            draw.ellipse(
                (x - RING_DOT_R, y - RING_DOT_R, x + RING_DOT_R, y + RING_DOT_R),
                fill=color,
            )
    return layer


def quantize_to_palette(arr: np.ndarray) -> np.ndarray:
    rgb = arr[..., :3].astype(np.int32)
    pal = PALETTE.astype(np.int32)
    diff = rgb[:, :, None, :] - pal[None, None, :, :]
    dist2 = np.sum(diff * diff, axis=3)
    idx = np.argmin(dist2, axis=2)
    return PALETTE[idx]


def choose_data_color(bit: int, rng: random.Random) -> Tuple[int, int, int]:
    if bit == 1:
        return DATA_BRIGHT
    return DATA_DIM


def main() -> None:
    cells = build_cells()
    bands = parse_bands(ACTIVE_BANDS)
    logo_mask = build_logo_mask()
    active_cells: List[Tuple[int, int, float]] = []
    for x0, y0, rr in cells:
        if rr > DATA_RADIUS:
            continue
        if bands:
            in_band = any(lo <= rr < hi for (lo, hi) in bands)
            if not in_band:
                continue
        if logo_mask is not None:
            if logo_mask[y0 : y0 + CELL_SIZE, x0 : x0 + CELL_SIZE].any():
                continue
        active_cells.append((x0, y0, rr))
    active_set = {(x0, y0) for x0, y0, _ in active_cells}

    dynamic_cells: List[Tuple[int, int, float]] = []
    static_cells: List[Tuple[int, int, float]] = []
    if DYNAMIC_RADIUS <= 0:
        dynamic_cells = list(active_cells)
        static_cells = []
    else:
        for cell in active_cells:
            if cell[2] <= DYNAMIC_RADIUS:
                dynamic_cells.append(cell)
            else:
                static_cells.append(cell)
    symbols_per_step = len(active_cells)
    steps = FRAMES // max(1, SYMBOL_PERIOD)
    total_symbols = steps * symbols_per_step * max(1, BITS_PER_SYMBOL)
    k_source = max(8, int(total_symbols * LDPC_RATE))
    if k_source >= total_symbols:
        k_source = max(8, total_symbols - 1)
    source_rng = np.random.default_rng(SOURCE_SEED)
    source_bits = source_rng.integers(0, 2, size=k_source, dtype=np.uint8)
    ra_rows = build_ra_code(k_source, total_symbols, row_w=LDPC_ROW_W, seed=RA_SEED)
    codeword_bits = ra_encode(source_bits, ra_rows)

    font = ImageFont.truetype(FONT_PATH, max(8, GLYPH_SIZE), index=FONT_INDEX)
    katakana = resolve_katakana()
    low_pool, high_pool, mid_pool, all_pool = build_glyph_pools(font, katakana)
    glyph_masks = {}
    for g in set(low_pool + high_pool + mid_pool + all_pool):
        alpha = render_glyph_alpha(font, g)
        glyph_masks[g] = alpha > GLYPH_THRESH

    katakana_font = None
    katakana_cells: List[Tuple[int, int, float]] = []
    katakana_glyphs: List[str] = []
    katakana_alt: Optional[List[str]] = None
    if KATAKANA_OVERLAY:
        kat_size = max(6, int(round(CELL_SIZE * KATAKANA_SCALE)))
        katakana_font = ImageFont.truetype(KATAKANA_FONT_PATH, kat_size, index=KATAKANA_FONT_INDEX)
        if KATAKANA_NONDATA:
            katakana_cells = [(x0, y0, rr) for (x0, y0, rr) in cells if (x0, y0) not in active_set]
        else:
            katakana_cells = list(active_cells)
        if katakana_cells:
            katakana_cells.sort(key=lambda t: (t[1], t[0]))
        if katakana_cells:
            kat_rng = random.Random(KATAKANA_SEED)
            katakana_glyphs = [kat_rng.choice(katakana) for _ in range(len(katakana_cells))]
            if KATAKANA_DIFF:
                alt_rng = random.Random(KATAKANA_SEED + 17)
                katakana_alt = [alt_rng.choice(katakana) for _ in range(len(katakana_cells))]

    logo_layer = build_logo_layer()
    ring_layer = build_ring_layer()
    if OUTER_RADIUS > 0:
        yy, xx = np.indices((H, W))
        rr_mask = np.sqrt((xx - W / 2.0) ** 2 + (yy - H / 2.0) ** 2)
        outer_mask = rr_mask <= OUTER_RADIUS
    else:
        outer_mask = None
    overlay_symbols_per_step = len(katakana_cells)

    def select_pool(name: str) -> List[str]:
        if name in ("low", "single_low"):
            return low_pool
        if name in ("low_extreme", "single_low_extreme"):
            return low_pool[:4] if len(low_pool) >= 4 else low_pool
        if name in ("high", "single_high"):
            return high_pool
        if name in ("mid", "single_mid"):
            return mid_pool
        if name in ("all", "single_all"):
            return all_pool
        raise ValueError(f"unknown pool {name!r}")

    dynamic_pool = select_pool(DYNAMIC_POOL)
    static_pool = select_pool(STATIC_POOL)

    rng = random.Random(GLYPH_SEED)

    static_glyphs: dict[Tuple[int, int], np.ndarray] = {}
    if static_cells:
        for idx, (x0, y0, _) in enumerate(static_cells):
            grng = random.Random(STATIC_GLYPH_SEED + idx * 1315423911)
            glyph = static_pool[grng.randrange(len(static_pool))]
            static_glyphs[(x0, y0)] = glyph_masks[glyph]
    # Build static base: background + logo + ring (data is drawn on top).
    base = np.zeros((H, W, 3), dtype=np.uint8)
    base[:] = BG
    if logo_layer is not None:
        logo_arr = np.array(logo_layer, dtype=np.uint8)
        logo_mask_arr = (logo_arr != np.array(BG, dtype=np.uint8)).any(axis=2)
        base[logo_mask_arr] = logo_arr[logo_mask_arr]
    if ring_layer is not None:
        ring_arr = np.array(ring_layer, dtype=np.uint8)
        ring_mask_arr = (ring_arr != 0).any(axis=2)
        base[ring_mask_arr] = ring_arr[ring_mask_arr]

    frames: List[Image.Image] = []
    prev_bits_by_cell: dict[Tuple[int, int], int] = {}
    for f in range(FRAMES):
        step = f // max(1, SYMBOL_PERIOD)
        draw_arr = np.array(base, dtype=np.uint8)
        curr_bits_by_cell: dict[Tuple[int, int], int] = {}
        # Draw data glyphs
        for idx, (x0, y0, _) in enumerate(active_cells):
            if DATA_DENSITY < 0.999 and rng.random() > DATA_DENSITY:
                continue
            bit_index = (step * symbols_per_step + idx) * max(1, BITS_PER_SYMBOL)
            b0 = int(codeword_bits[bit_index])
            static_mask = static_glyphs.get((x0, y0))
            region = draw_arr[y0 : y0 + CELL_SIZE, x0 : x0 + CELL_SIZE]
            if static_mask is not None:
                # Static glyphs: uniform color (bright/dim) over fixed mask
                color = choose_data_color(b0, rng)
                region[static_mask] = color
            else:
                # Dynamic glyphs: split bright/dim within glyph using alpha thresholds
                glyph = dynamic_pool[rng.randrange(len(dynamic_pool))]
                mask = glyph_masks[glyph]
                color = choose_data_color(b0, rng)
                region[mask] = color
            curr_bits_by_cell[(x0, y0)] = b0
        if KATAKANA_OVERLAY and katakana_font is not None and katakana_cells:
            frame = Image.fromarray(draw_arr, mode="RGB").convert("RGBA")
            kat_layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
            kdraw = ImageDraw.Draw(kat_layer)
            kat_color = parse_color(KATAKANA_COLOR, DATA_BRIGHT)
            box_on = parse_color(KATAKANA_BOX_ON, DATA_BRIGHT)
            box_off = parse_color(KATAKANA_BOX_OFF, DATA_DIM)
            text_on = parse_color(KATAKANA_TEXT_ON, (0, 0, 0))
            text_off = parse_color(KATAKANA_TEXT_OFF, kat_color)
            for i, (x0, y0, _) in enumerate(katakana_cells):
                if overlay_symbols_per_step <= 0:
                    break
                if not KATAKANA_NONDATA:
                    curr_bit = int(curr_bits_by_cell.get((x0, y0), 0))
                    prev_bit = int(prev_bits_by_cell.get((x0, y0), curr_bit))
                else:
                    base_idx = (step * overlay_symbols_per_step + i) % len(codeword_bits)
                    prev_step = steps - 1 if step == 0 else step - 1
                    prev_idx = (prev_step * overlay_symbols_per_step + i) % len(codeword_bits)
                    curr_bit = int(codeword_bits[base_idx])
                    prev_bit = int(codeword_bits[prev_idx])
                glyph = katakana_glyphs[i]
                if KATAKANA_DIFF and curr_bit != prev_bit and katakana_alt is not None:
                    glyph = katakana_alt[i]
                alpha = KATAKANA_ALPHA
                if KATAKANA_DIFF and curr_bit != prev_bit:
                    alpha = min(255, alpha + 30)
                if KATAKANA_BOXES:
                    bw = int(round(CELL_SIZE * KATAKANA_BOX_SCALE))
                    bh = int(round(CELL_SIZE * KATAKANA_BOX_SCALE))
                    bx0 = int(x0 + (CELL_SIZE - bw) / 2)
                    by0 = int(y0 + (CELL_SIZE - bh) / 2)
                    bcol = box_on if curr_bit == 1 else box_off
                    kdraw.rectangle((bx0, by0, bx0 + bw, by0 + bh), fill=bcol + (KATAKANA_BOX_ALPHA,))
                text_color = text_on if (KATAKANA_BOXES and curr_bit == 1) else text_off
                bbox = kdraw.textbbox((0, 0), glyph, font=katakana_font)
                gw = bbox[2] - bbox[0]
                gh = bbox[3] - bbox[1]
                tx = x0 + CELL_SIZE / 2 - gw / 2
                ty = y0 + CELL_SIZE / 2 - gh / 2
                kdraw.text((tx, ty), glyph, font=katakana_font, fill=text_color + (int(alpha),))
            frame = Image.alpha_composite(frame, kat_layer)
            draw_arr = np.array(frame.convert("RGB"), dtype=np.uint8)
        if outer_mask is not None:
            draw_arr[~outer_mask] = (BG[0], BG[1], BG[2])
        quant = quantize_to_palette(draw_arr)
        frames.append(Image.fromarray(quant, mode="RGB"))
        prev_bits_by_cell = curr_bits_by_cell

    frames[0].save(
        OUT_GIF,
        save_all=True,
        append_images=frames[1:],
        duration=int(1000 / FPS),
        loop=0,
        disposal=2,
    )
    print("gif", OUT_GIF)


if __name__ == "__main__":
    main()
