import math
import os
import random
from typing import List, Optional, Tuple

import numpy as np
from PIL import Image, ImageDraw, ImageFont

# TODO: refine glyph placement/selection to closer match v27; current parameters are best-fit but not exact.

BASE_DIR = os.path.dirname(__file__)
OUT_GIF = os.getenv(
    "SS_OUT_GIF",
    "/tmp/sakura_storm_viz/sakura_storm_v27_procedural.gif"
    if os.path.isdir("/tmp/sakura_storm_viz")
    else os.path.join(BASE_DIR, "sakura_storm_v27_procedural.gif"),
)

W = int(os.getenv("SS_W", "512"))
H = int(os.getenv("SS_H", "512"))
FRAMES = int(os.getenv("SS_FRAMES", "96"))
FPS = int(os.getenv("SS_FPS", "25"))

# Layout (reverse-engineered defaults)
GRID_N = int(os.getenv("SS_GRID_N", "32"))
CELL_SIZE = int(os.getenv("SS_CELL_SIZE", "16"))
GRID_OFFSET = int(os.getenv("SS_GRID_OFFSET", "0"))
DATA_RADIUS = float(os.getenv("SS_DATA_RADIUS", "247.0"))
OUTER_RADIUS = float(os.getenv("SS_OUTER_RADIUS", "245.0"))
DATA_DENSITY = float(os.getenv("SS_DATA_DENSITY", "1.0"))
ACTIVE_BANDS = os.getenv("SS_ACTIVE_BANDS", "20-169,184-196,208-220,232-244").strip()
MASK_LOGO = os.getenv("SS_MASK_LOGO", "1").strip() == "1"
LOGO_MASK_THRESH = int(os.getenv("SS_LOGO_MASK_THRESH", "10"))

# Palette (v27)
BG = (11, 7, 12)
DATA_BRIGHT = (255, 233, 246)
DATA_BRIGHT2 = (255, 234, 246)
DATA_DIM = (69, 40, 54)
DATA_DIM2 = (68, 40, 54)
RING_BRIGHT = (255, 246, 252)
RING_DIM = (62, 36, 50)
LOGO_SHADES = [(30, 16, 25), (36, 18, 28), (43, 20, 32)]

PALETTE = np.array(
    [
        BG,
        LOGO_SHADES[0],
        LOGO_SHADES[1],
        LOGO_SHADES[2],
        RING_DIM,
        DATA_DIM2,
        DATA_DIM,
        DATA_BRIGHT,
        DATA_BRIGHT2,
        RING_BRIGHT,
    ],
    dtype=np.uint8,
)

# Glyphs
KATAKANA = list("アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン")
FONT_PATH = os.getenv("SS_FONT_PATH", "/System/Library/Fonts/Hiragino Sans GB.ttc")
GLYPH_SIZE = int(os.getenv("SS_GLYPH_SIZE", "15"))
GLYPH_THRESH = int(os.getenv("SS_GLYPH_THRESH", "96"))
GLYPH_POOL_MODE = os.getenv("SS_GLYPH_POOL_MODE", "single_low_extreme").strip().lower()
GLYPH_SEED = int(os.getenv("SS_GLYPH_SEED", "1337"))

# Rings
RING_RADII = [float(x) for x in os.getenv("SS_RING_RADII", "190,214,238").split(",") if x.strip()]
RING_SPACING = float(os.getenv("SS_RING_SPACING", "37.0"))
RING_DOT_R = int(os.getenv("SS_RING_DOT_R", "7"))
RING_DIM_ANGLES = [0.0, 90.0, 180.0, 270.0]

# Logo
LOGO_PATH = os.getenv(
    "SS_LOGO_PATH",
    "/Users/mtakemiya/Library/CloudStorage/Dropbox/soramitsu/tarmo/sora_logo.png",
)
LOGO_SCALE = float(os.getenv("SS_LOGO_SCALE", "0.318"))
LOGO_ALPHA = int(os.getenv("SS_LOGO_ALPHA", "255"))

# Color variant selection (to match dual bright/dim palette usage)
USE_COLOR_VARIANTS = os.getenv("SS_COLOR_VARIANTS", "1").strip() == "1"
BRIGHT_ALT_P = float(os.getenv("SS_BRIGHT_ALT_P", "0.135"))
DIM_ALT_P = float(os.getenv("SS_DIM_ALT_P", "0.58"))

# Coding
SYMBOL_PERIOD = int(os.getenv("SS_SYMBOL_PERIOD", "1"))
LDPC_RATE = float(os.getenv("SS_LDPC_RATE", "0.25"))
LDPC_ROW_W = int(os.getenv("SS_LDPC_ROW_W", "4"))


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


def render_glyph_mask(font: ImageFont.FreeTypeFont, glyph: str) -> np.ndarray:
    img = Image.new("L", (CELL_SIZE, CELL_SIZE), 0)
    draw = ImageDraw.Draw(img)
    bbox = draw.textbbox((0, 0), glyph, font=font)
    gw = bbox[2] - bbox[0]
    gh = bbox[3] - bbox[1]
    tx = CELL_SIZE / 2 - gw / 2
    ty = CELL_SIZE / 2 - gh / 2
    draw.text((tx, ty), glyph, font=font, fill=255)
    arr = np.array(img)
    return arr > GLYPH_THRESH


def build_glyph_pools(font: ImageFont.FreeTypeFont) -> Tuple[List[str], List[str]]:
    densities = []
    for g in KATAKANA:
        m = render_glyph_mask(font, g)
        densities.append((g, float(np.mean(m))))
    densities.sort(key=lambda x: x[1])
    low = [g for g, _ in densities[: max(4, len(densities) // 3)]]
    low_extreme = [g for g, _ in densities[:4]]
    high = [g for g, _ in densities[-max(4, len(densities) // 3) :]]
    if GLYPH_POOL_MODE in ("all", "single_all"):
        return KATAKANA[:], KATAKANA[:]
    if GLYPH_POOL_MODE in ("single_low", "low"):
        return low, low
    if GLYPH_POOL_MODE in ("single_low_extreme", "low_extreme"):
        return low_extreme, low_extreme
    if GLYPH_POOL_MODE in ("lowhigh", "low_high"):
        return low, high
    raise ValueError(f"unknown GLYPH_POOL_MODE {GLYPH_POOL_MODE!r}")


def build_logo_layer() -> Image.Image:
    if not os.path.exists(LOGO_PATH):
        return Image.new("RGBA", (W, H), (0, 0, 0, 0))
    logo = Image.open(LOGO_PATH).convert("RGBA")
    target = int(min(W, H) * LOGO_SCALE)
    target = max(8, min(min(W, H), target))
    logo = logo.resize((target, target), resample=Image.LANCZOS)
    arr = np.array(logo, dtype=np.uint8)
    alpha = arr[..., 3].astype(np.float32) / 255.0
    tinted = np.zeros_like(arr)
    tinted[..., 0] = LOGO_SHADES[-1][0]
    tinted[..., 1] = LOGO_SHADES[-1][1]
    tinted[..., 2] = LOGO_SHADES[-1][2]
    tinted[..., 3] = np.clip(alpha * LOGO_ALPHA, 0, 255).astype(np.uint8)
    logo_tinted = Image.fromarray(tinted, mode="RGBA")
    full = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    x0 = int(W / 2 - target / 2)
    y0 = int(H / 2 - target / 2)
    full.paste(logo_tinted, (x0, y0), logo_tinted)
    return full


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
    mask[y0 : y0 + target, x0 : x0 + target] = alpha > LOGO_MASK_THRESH
    return mask


def build_ring_layer() -> Image.Image:
    layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
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
                    if abs((deg - a + 180.0) % 360.0 - 180.0) < 4.0:
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
        if USE_COLOR_VARIANTS and rng.random() < BRIGHT_ALT_P:
            return DATA_BRIGHT2
        return DATA_BRIGHT
    if USE_COLOR_VARIANTS and rng.random() < DIM_ALT_P:
        return DATA_DIM2
    return DATA_DIM


def main() -> None:
    cells = build_cells()
    bands = parse_bands(ACTIVE_BANDS)
    logo_mask = build_logo_mask()
    active_cells = []
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
    symbols_per_step = len(active_cells)
    steps = FRAMES // max(1, SYMBOL_PERIOD)
    total_symbols = steps * symbols_per_step
    k_source = max(8, int(total_symbols * LDPC_RATE))
    if k_source >= total_symbols:
        k_source = max(8, total_symbols - 1)
    source_rng = np.random.default_rng(1234)
    source_bits = source_rng.integers(0, 2, size=k_source, dtype=np.uint8)
    ra_rows = build_ra_code(k_source, total_symbols, row_w=LDPC_ROW_W, seed=2027)
    codeword_bits = ra_encode(source_bits, ra_rows)

    font = ImageFont.truetype(FONT_PATH, max(8, GLYPH_SIZE))
    low_pool, high_pool = build_glyph_pools(font)
    glyph_masks = {g: render_glyph_mask(font, g) for g in set(low_pool + high_pool)}

    logo_layer = build_logo_layer()
    ring_layer = build_ring_layer()
    if OUTER_RADIUS > 0:
        yy, xx = np.indices((H, W))
        rr_mask = np.sqrt((xx - W / 2.0) ** 2 + (yy - H / 2.0) ** 2)
        outer_mask = rr_mask <= OUTER_RADIUS
    else:
        outer_mask = None

    rng = random.Random(GLYPH_SEED)
    color_rng = random.Random(GLYPH_SEED + 101)
    frames: List[Image.Image] = []
    for f in range(FRAMES):
        step = f // max(1, SYMBOL_PERIOD)
        base = np.zeros((H, W, 3), dtype=np.uint8)
        base[:] = BG
        frame = Image.fromarray(base, mode="RGB").convert("RGBA")
        draw_arr = np.array(frame, dtype=np.uint8)
        # Draw data glyphs
        for idx, (x0, y0, _) in enumerate(active_cells):
            if DATA_DENSITY < 0.999 and rng.random() > DATA_DENSITY:
                continue
            bit = int(codeword_bits[step * symbols_per_step + idx])
            pool = high_pool if bit == 1 else low_pool
            glyph = pool[rng.randrange(len(pool))]
            mask = glyph_masks[glyph]
            color = choose_data_color(bit, color_rng)
            region = draw_arr[y0 : y0 + CELL_SIZE, x0 : x0 + CELL_SIZE]
            region[mask] = color + (255,)
        frame = Image.fromarray(draw_arr, mode="RGBA")
        frame = Image.alpha_composite(frame, ring_layer)
        frame = Image.alpha_composite(frame, logo_layer)
        draw_arr = np.array(frame, dtype=np.uint8)
        if outer_mask is not None:
            draw_arr[~outer_mask] = (BG[0], BG[1], BG[2], 255)
        quant = quantize_to_palette(draw_arr)
        frames.append(Image.fromarray(quant, mode="RGB"))

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
