import math
import os
import random
from typing import List, Optional, Tuple

import numpy as np
from PIL import Image, ImageDraw, ImageFont

from v27_params import KATAKANA, KATAKANA_V27, PALETTE_PRESETS, V27

BASE_DIR = os.path.dirname(__file__)
SYMBOLS_NPZ = os.getenv(
    "SS_SYMBOLS_NPZ",
    "/tmp/sakura_storm_viz/v27_symbols.npz"
    if os.path.isdir("/tmp/sakura_storm_viz")
    else os.path.join(BASE_DIR, "v27_symbols.npz"),
)
OUT_GIF = os.getenv(
    "SS_OUT_GIF",
    "/tmp/sakura_storm_viz/sakura_storm_v27_from_symbols.gif"
    if os.path.isdir("/tmp/sakura_storm_viz")
    else os.path.join(BASE_DIR, "sakura_storm_v27_from_symbols.gif"),
)

W = int(os.getenv("SS_W", str(V27.w)))
H = int(os.getenv("SS_H", str(V27.h)))
FPS = int(os.getenv("SS_FPS", str(V27.fps)))

# Rings
RING_RADII = [float(x) for x in os.getenv("SS_RING_RADII", ",".join(str(x) for x in V27.ring_radii)).split(",") if x.strip()]
RING_SPACING = float(os.getenv("SS_RING_SPACING", str(V27.ring_spacing)))
RING_DOT_R = int(os.getenv("SS_RING_DOT_R", str(V27.ring_dot_r)))
RING_DIM_ANGLES = [float(x) for x in os.getenv("SS_RING_DIM_ANGLES", ",".join(str(x) for x in V27.ring_dim_angles)).split(",") if x.strip()]
RING_DIM_TOL = float(os.getenv("SS_RING_DIM_TOL", "4.0"))
RING_OVERRIDE = os.getenv("SS_RING_OVERRIDE", "0").strip() == "1"

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
LOGO_OVERLAY = os.getenv("SS_LOGO_OVERLAY", "0").strip() == "1"
LOGO_OVERLAY_ALPHA = int(os.getenv("SS_LOGO_OVERLAY_ALPHA", "255"))
LOGO_OVERLAY_COLOR = os.getenv("SS_LOGO_OVERLAY_COLOR", "")
LOGO_OVERLAY_POST = os.getenv("SS_LOGO_OVERLAY_POST", "0").strip() == "1"
LOGO_MASK_REF = os.getenv("SS_LOGO_MASK_REF", "").strip()
LOGO_MASK_COLORS = os.getenv("SS_LOGO_MASK_COLORS", "").strip()

# Outer mask
OUTER_RADIUS = float(os.getenv("SS_OUTER_RADIUS", str(V27.outer_radius)))

# Glyphs
FONT_PATH = os.getenv("SS_FONT_PATH", "/System/Library/Fonts/Hiragino Sans GB.ttc")
FONT_INDEX = int(os.getenv("SS_FONT_INDEX", str(V27.font_index)))
GLYPH_SIZE = int(os.getenv("SS_GLYPH_SIZE", str(V27.glyph_size)))
GLYPH_THRESH = int(os.getenv("SS_GLYPH_THRESH", str(V27.glyph_thresh)))
PALETTE_PRESET = os.getenv("SS_PALETTE_PRESET", "").strip().lower()
KATAKANA_MODE = os.getenv("SS_KATAKANA_MODE", "v27").strip().lower()
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
KATAKANA_FONT_PATH = os.getenv("SS_KATAKANA_FONT_PATH", FONT_PATH)
KATAKANA_FONT_INDEX = int(os.getenv("SS_KATAKANA_FONT_INDEX", str(FONT_INDEX)))
KATAKANA_QUANTIZE = os.getenv("SS_KATAKANA_QUANTIZE", "1").strip() == "1"
FORCE_PALETTE = os.getenv("SS_FORCE_PALETTE", "0").strip() == "1"
KATAKANA_OUTSIDE_LOGO = os.getenv("SS_KATAKANA_OUTSIDE_LOGO", "0").strip() == "1"


def render_glyph_mask(font: ImageFont.FreeTypeFont, glyph: str, cell: int) -> np.ndarray:
    img = Image.new("L", (cell, cell), 0)
    draw = ImageDraw.Draw(img)
    bbox = draw.textbbox((0, 0), glyph, font=font)
    gw = bbox[2] - bbox[0]
    gh = bbox[3] - bbox[1]
    tx = cell / 2 - gw / 2
    ty = cell / 2 - gh / 2
    draw.text((tx, ty), glyph, font=font, fill=255)
    arr = np.array(img)
    return arr > GLYPH_THRESH


def resolve_katakana() -> List[str]:
    if KATAKANA_MODE in ("v27", "legacy"):
        return KATAKANA_V27[:]
    if KATAKANA_MODE in ("iroha", "iroha_extra"):
        return KATAKANA[:]
    raise ValueError(f"unknown KATAKANA_MODE {KATAKANA_MODE!r}")


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


def parse_color_list(text: str) -> List[Tuple[int, int, int]]:
    if not text:
        return []
    parts = [p.strip() for p in text.split(";") if p.strip()]
    colors: List[Tuple[int, int, int]] = []
    for item in parts:
        cols = parse_color(item, None)
        if cols is None:
            continue
        colors.append(cols)
    return colors


def build_logo_mask_from_ref(logo_shades: np.ndarray, bg_color: np.ndarray) -> Tuple[Optional[np.ndarray], Optional[np.ndarray]]:
    if not LOGO_MASK_REF or not os.path.exists(LOGO_MASK_REF):
        return None, None
    ref = Image.open(LOGO_MASK_REF)
    if ref.n_frames > 1:
        ref.seek(0)
    ref = ref.convert("RGB")
    if ref.size != (W, H):
        ref = ref.resize((W, H), resample=Image.NEAREST)
    arr = np.array(ref, dtype=np.uint8)
    colors = parse_color_list(LOGO_MASK_COLORS)
    if not colors and PALETTE_PRESET == "v26_preview":
        colors = [tuple(c) for c in logo_shades]
    if not colors:
        return None, None
    mask = np.zeros((H, W), dtype=bool)
    for col in colors:
        mask |= (arr == np.array(col, dtype=np.uint8)).all(axis=2)
    logo_arr = np.zeros((H, W, 3), dtype=np.uint8)
    logo_arr[:] = bg_color
    logo_arr[mask] = np.array(colors[0], dtype=np.uint8)
    return logo_arr, mask


def apply_logo_overlay(arr: np.ndarray, logo_arr: np.ndarray, logo_mask_arr: np.ndarray, fallback: Tuple[int, int, int]) -> np.ndarray:
    if not LOGO_OVERLAY or not logo_mask_arr.any():
        return arr
    overlay_color = None
    if LOGO_OVERLAY_COLOR:
        overlay_color = parse_color(LOGO_OVERLAY_COLOR, fallback)
    out = arr.copy()
    if LOGO_OVERLAY_ALPHA >= 255:
        if overlay_color is None:
            out[logo_mask_arr] = logo_arr[logo_mask_arr]
        else:
            out[logo_mask_arr] = np.array(overlay_color, dtype=np.uint8)
        return out
    alpha = max(0, min(255, LOGO_OVERLAY_ALPHA)) / 255.0
    blended = out.astype(np.float32)
    if overlay_color is None:
        src = logo_arr.astype(np.float32)
    else:
        src = np.zeros_like(out, dtype=np.float32)
        src[logo_mask_arr] = np.array(overlay_color, dtype=np.float32)
    blended[logo_mask_arr] = blended[logo_mask_arr] * (1.0 - alpha) + src[logo_mask_arr] * alpha
    return blended.astype(np.uint8)


def quantize_to_palette(arr: np.ndarray, palette: np.ndarray) -> np.ndarray:
    rgb = arr[..., :3].astype(np.int32)
    pal = palette.astype(np.int32)
    diff = rgb[:, :, None, :] - pal[None, None, :, :]
    dist = np.sum(diff * diff, axis=3)
    idx = np.argmin(dist, axis=2)
    return pal[idx].astype(np.uint8)


def build_logo_layer(w: int, h: int, logo_shades: np.ndarray, bg: np.ndarray) -> Image.Image:
    layer = np.zeros((h, w, 3), dtype=np.uint8)
    layer[:] = bg
    if not os.path.exists(LOGO_PATH):
        return Image.fromarray(layer, mode="RGB")
    logo = Image.open(LOGO_PATH).convert("RGBA")
    target = int(min(w, h) * LOGO_SCALE)
    target = max(8, min(min(w, h), target))
    logo = logo.resize((target, target), resample=Image.LANCZOS)
    arr = np.array(logo, dtype=np.uint8)
    alpha = arr[..., 3].astype(np.float32)
    alpha = np.clip(alpha * (LOGO_ALPHA / 255.0), 0.0, 255.0).astype(np.uint8)
    x0 = int(w / 2 - target / 2)
    y0 = int(h / 2 - target / 2)
    region = layer[y0 : y0 + target, x0 : x0 + target]
    region[alpha >= LOGO_THRESH_1] = logo_shades[0]
    region[alpha >= LOGO_THRESH_2] = logo_shades[1]
    region[alpha >= LOGO_THRESH_3] = logo_shades[2]
    return Image.fromarray(layer, mode="RGB")


def build_ring_layer(w: int, h: int, ring_bright: np.ndarray, ring_dim: np.ndarray) -> Image.Image:
    layer = Image.new("RGB", (w, h), (0, 0, 0))
    draw = ImageDraw.Draw(layer)
    for r in RING_RADII:
        n = max(8, int(round((2.0 * math.pi * r) / max(1.0, RING_SPACING))))
        for k in range(n):
            ang = 2.0 * math.pi * k / n
            x = w / 2 + r * math.cos(ang)
            y = h / 2 + r * math.sin(ang)
            color = ring_bright
            if abs(r - 214.0) < 0.8:
                deg = (math.degrees(ang) + 360.0) % 360.0
                for a in RING_DIM_ANGLES:
                    if abs((deg - a + 180.0) % 360.0 - 180.0) < RING_DIM_TOL:
                        color = ring_dim
                        break
            draw.ellipse(
                (x - RING_DOT_R, y - RING_DOT_R, x + RING_DOT_R, y + RING_DOT_R),
                fill=tuple(int(c) for c in color),
            )
    return layer


def apply_palette_preset(
    bg: np.ndarray,
    data_bright: np.ndarray,
    data_dim: np.ndarray,
    ring_bright: np.ndarray,
    ring_dim: np.ndarray,
    logo_shades: np.ndarray,
) -> Tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    if not PALETTE_PRESET:
        return bg, data_bright, data_dim, ring_bright, ring_dim, logo_shades
    preset = PALETTE_PRESETS.get(PALETTE_PRESET)
    if preset is None:
        raise ValueError(f"unknown PALETTE_PRESET {PALETTE_PRESET!r}")
    bg = np.array(preset["bg"], dtype=np.uint8)
    data_bright = np.array(preset["data_bright"], dtype=np.uint8)
    data_dim = np.array(preset["data_dim"], dtype=np.uint8)
    ring_bright = np.array(preset["ring_bright"], dtype=np.uint8)
    ring_dim = np.array(preset["ring_dim"], dtype=np.uint8)
    logo_shades = np.array(preset["logo_shades"], dtype=np.uint8)
    return bg, data_bright, data_dim, ring_bright, ring_dim, logo_shades


def main() -> None:
    data = np.load(SYMBOLS_NPZ, allow_pickle=False)
    cells = data["cells"]
    glyph_idx = data["glyph_idx"]
    bits = data["bits"]
    glyphs = [str(g) for g in data["glyphs"].tolist()]
    mask_bits = data.get("mask_bits")
    glyph_masks_ref = data.get("glyph_masks_ref")
    glyph_masks_packed = data.get("glyph_masks")
    allowed_masks = data.get("allowed_masks")
    ring_idx = data.get("ring_idx")
    ring_layer = data.get("ring_layer")
    cell_size = int(data.get("cell_size", 16))
    bg = data.get("bg", np.array([11, 7, 12], dtype=np.uint8))
    data_bright = data.get("data_bright", np.array([255, 233, 246], dtype=np.uint8))
    data_dim = data.get("data_dim", np.array([69, 40, 54], dtype=np.uint8))
    ring_bright = data.get("ring_bright", np.array([255, 246, 252], dtype=np.uint8))
    ring_dim = data.get("ring_dim", np.array([62, 36, 50], dtype=np.uint8))
    logo_shades = data.get("logo_shades", np.array([[30, 16, 25], [36, 18, 28], [43, 20, 32]], dtype=np.uint8))
    static_layer = data.get("static_layer")
    bg, data_bright, data_dim, ring_bright, ring_dim, logo_shades = apply_palette_preset(
        bg,
        data_bright,
        data_dim,
        ring_bright,
        ring_dim,
        logo_shades,
    )
    palette = np.array(
        [
            bg,
            logo_shades[0],
            logo_shades[1],
            logo_shades[2],
            ring_dim,
            data_dim,
            data_bright,
            ring_bright,
        ],
        dtype=np.uint8,
    )

    steps = glyph_idx.shape[0]
    if bits.shape != glyph_idx.shape:
        raise ValueError("bits shape does not match glyph_idx")

    katakana = resolve_katakana()
    katakana_cells: List[Tuple[int, int]] = []
    katakana_glyphs: List[str] = []
    katakana_alt: List[str] | None = None
    katakana_font = None
    mapping_bits = bits.reshape(-1)
    if KATAKANA_OVERLAY and mapping_bits.size > 0:
        kat_size = max(6, int(round(cell_size * KATAKANA_SCALE)))
        katakana_font = ImageFont.truetype(KATAKANA_FONT_PATH, kat_size, index=KATAKANA_FONT_INDEX)
        cell_list = [(int(x), int(y)) for (x, y) in cells.tolist()]
        cell_set = set(cell_list)
        cell_index = {coord: i for i, coord in enumerate(cell_list)}
        if KATAKANA_NONDATA:
            grid_n = max(1, int(W // cell_size))
            for r in range(grid_n):
                y0 = r * cell_size
                for c in range(grid_n):
                    x0 = c * cell_size
                    if (x0, y0) in cell_set:
                        continue
                    if x0 + cell_size > W or y0 + cell_size > H:
                        continue
                    katakana_cells.append((x0, y0))
        else:
            katakana_cells = [(int(x), int(y)) for (x, y) in cells.tolist()]
        if katakana_cells:
            katakana_cells.sort(key=lambda t: (t[1], t[0]))
            rng = random.Random(KATAKANA_SEED)
            katakana_glyphs = [rng.choice(katakana) for _ in range(len(katakana_cells))]
            if KATAKANA_DIFF:
                alt_rng = random.Random(KATAKANA_SEED + 17)
                katakana_alt = [alt_rng.choice(katakana) for _ in range(len(katakana_cells))]
    else:
        cell_index = {}

    glyph_masks = None
    if glyph_masks_packed is not None:
        glyph_masks = []
        for row in glyph_masks_packed:
            flat = np.unpackbits(row, bitorder="big")[: cell_size * cell_size]
            glyph_masks.append(flat.reshape((cell_size, cell_size)).astype(bool))
    elif glyph_masks_ref is not None:
        glyph_masks = []
        for row in glyph_masks_ref:
            flat = np.unpackbits(row, bitorder="big")[: cell_size * cell_size]
            glyph_masks.append(flat.reshape((cell_size, cell_size)).astype(bool))
    elif mask_bits is None:
        font = ImageFont.truetype(FONT_PATH, GLYPH_SIZE, index=FONT_INDEX)
        glyph_masks = [render_glyph_mask(font, g, cell_size) for g in glyphs]

    needs_logo_mask = LOGO_OVERLAY or KATAKANA_OUTSIDE_LOGO
    logo_layer = None if (static_layer is not None and not needs_logo_mask) else build_logo_layer(W, H, logo_shades, bg)
    logo_arr = None
    logo_mask_arr = None
    if logo_layer is not None:
        logo_arr = np.array(logo_layer, dtype=np.uint8)
        logo_mask_arr = (logo_arr != bg).any(axis=2)
    if LOGO_MASK_REF:
        ref_logo_arr, ref_logo_mask = build_logo_mask_from_ref(logo_shades, bg)
        if ref_logo_mask is not None:
            logo_arr = ref_logo_arr
            logo_mask_arr = ref_logo_mask
    ring_layer_img = None
    if ring_layer is None and (static_layer is None or RING_OVERRIDE) and ring_idx is None:
        ring_layer_img = build_ring_layer(W, H, ring_bright, ring_dim)

    frames: List[Image.Image] = []
    for f in range(steps):
        if static_layer is not None:
            base = np.array(static_layer, dtype=np.uint8)
            if RING_OVERRIDE:
                ring_mask = (base == ring_bright).all(axis=2) | (base == ring_dim).all(axis=2)
                base[ring_mask] = bg
            if ring_layer_img is not None:
                ring_arr = np.array(ring_layer_img, dtype=np.uint8)
                ring_mask_arr = (ring_arr != 0).any(axis=2)
                base[ring_mask_arr] = ring_arr[ring_mask_arr]
        else:
            base = np.zeros((H, W, 3), dtype=np.uint8)
            base[:] = bg
            if logo_layer is not None:
                logo_arr = np.array(logo_layer, dtype=np.uint8)
                logo_mask_arr = (logo_arr != bg).any(axis=2)
                base[logo_mask_arr] = logo_arr[logo_mask_arr]
            if ring_layer_img is not None:
                ring_arr = np.array(ring_layer_img, dtype=np.uint8)
                ring_mask_arr = (ring_arr != 0).any(axis=2)
                base[ring_mask_arr] = ring_arr[ring_mask_arr]
        if ring_layer is not None:
            mask = (ring_layer != np.array([0, 0, 0], dtype=np.uint8)).any(axis=2)
            if mask.any():
                base[mask] = ring_layer[mask]
        draw_arr = np.array(base, dtype=np.uint8)
        for i, (x0, y0) in enumerate(cells):
            if mask_bits is not None:
                packed = mask_bits[f, i]
                flat = np.unpackbits(packed, bitorder="big")[: cell_size * cell_size]
                mask = flat.reshape((cell_size, cell_size)).astype(bool)
            else:
                mask = glyph_masks[int(glyph_idx[f, i])]
                if allowed_masks is not None:
                    allowed_flat = np.unpackbits(allowed_masks[i], bitorder="big")[: cell_size * cell_size]
                    allowed = allowed_flat.reshape((cell_size, cell_size)).astype(bool)
                    mask = mask & allowed
            color = data_bright if int(bits[f, i]) == 1 else data_dim
            region = draw_arr[y0 : y0 + cell_size, x0 : x0 + cell_size]
            region[mask] = (int(color[0]), int(color[1]), int(color[2]))
        if KATAKANA_OVERLAY and katakana_font is not None and katakana_cells:
            frame = Image.fromarray(draw_arr, mode="RGB").convert("RGBA")
            kat_layer = Image.new("RGBA", (W, H), (0, 0, 0, 0))
            kdraw = ImageDraw.Draw(kat_layer)
            kat_color = parse_color(KATAKANA_COLOR, (int(data_bright[0]), int(data_bright[1]), int(data_bright[2])))
            box_on = parse_color(KATAKANA_BOX_ON, (int(data_bright[0]), int(data_bright[1]), int(data_bright[2])))
            box_off = parse_color(KATAKANA_BOX_OFF, (int(data_dim[0]), int(data_dim[1]), int(data_dim[2])))
            text_on = parse_color(KATAKANA_TEXT_ON, (0, 0, 0))
            text_off = parse_color(KATAKANA_TEXT_OFF, kat_color)
            overlay_symbols_per_step = len(katakana_cells)
            for i, (x0, y0) in enumerate(katakana_cells):
                if overlay_symbols_per_step <= 0:
                    break
                if KATAKANA_OUTSIDE_LOGO and logo_mask_arr is not None and logo_mask_arr.any():
                    cx = int(x0 + cell_size / 2)
                    cy = int(y0 + cell_size / 2)
                    if 0 <= cx < W and 0 <= cy < H and logo_mask_arr[cy, cx]:
                        continue
                if not KATAKANA_NONDATA:
                    idx = cell_index.get((x0, y0))
                    if idx is None:
                        continue
                    prev_f = steps - 1 if f == 0 else f - 1
                    curr_bit = int(bits[f, idx])
                    prev_bit = int(bits[prev_f, idx])
                else:
                    base_idx = (f * overlay_symbols_per_step + i) % mapping_bits.size
                    prev_step = steps - 1 if f == 0 else f - 1
                    prev_idx = (prev_step * overlay_symbols_per_step + i) % mapping_bits.size
                    curr_bit = int(mapping_bits[base_idx])
                    prev_bit = int(mapping_bits[prev_idx])
                glyph = katakana_glyphs[i]
                if KATAKANA_DIFF and curr_bit != prev_bit and katakana_alt is not None:
                    glyph = katakana_alt[i]
                alpha = KATAKANA_ALPHA
                if KATAKANA_DIFF and curr_bit != prev_bit:
                    alpha = min(255, alpha + 30)
                if KATAKANA_BOXES:
                    bw = int(round(cell_size * KATAKANA_BOX_SCALE))
                    bh = int(round(cell_size * KATAKANA_BOX_SCALE))
                    bx0 = int(x0 + (cell_size - bw) / 2)
                    by0 = int(y0 + (cell_size - bh) / 2)
                    bcol = box_on if curr_bit == 1 else box_off
                    kdraw.rectangle((bx0, by0, bx0 + bw, by0 + bh), fill=bcol + (KATAKANA_BOX_ALPHA,))
                text_color = text_on if (KATAKANA_BOXES and curr_bit == 1) else text_off
                bbox = kdraw.textbbox((0, 0), glyph, font=katakana_font)
                gw = bbox[2] - bbox[0]
                gh = bbox[3] - bbox[1]
                tx = x0 + cell_size / 2 - gw / 2
                ty = y0 + cell_size / 2 - gh / 2
                kdraw.text((tx, ty), glyph, font=katakana_font, fill=text_color + (int(alpha),))
            frame = Image.alpha_composite(frame, kat_layer)
            draw_arr = np.array(frame.convert("RGB"), dtype=np.uint8)
        if LOGO_OVERLAY and (not LOGO_OVERLAY_POST) and logo_arr is not None and logo_mask_arr is not None:
            draw_arr = apply_logo_overlay(
                draw_arr,
                logo_arr,
                logo_mask_arr,
                (int(data_bright[0]), int(data_bright[1]), int(data_bright[2])),
            )
        if ring_idx is not None:
            arr = np.array(draw_arr, dtype=np.uint8)
            mask_b = ring_idx[f] == 1
            mask_d = ring_idx[f] == 2
            if mask_b.any():
                arr[mask_b] = (int(ring_bright[0]), int(ring_bright[1]), int(ring_bright[2]))
            if mask_d.any():
                arr[mask_d] = (int(ring_dim[0]), int(ring_dim[1]), int(ring_dim[2]))
            draw_arr = arr
        if (KATAKANA_OVERLAY and KATAKANA_QUANTIZE) or FORCE_PALETTE:
            draw_arr = quantize_to_palette(draw_arr, palette)
        if LOGO_OVERLAY and LOGO_OVERLAY_POST and logo_arr is not None and logo_mask_arr is not None:
            draw_arr = apply_logo_overlay(
                draw_arr,
                logo_arr,
                logo_mask_arr,
                (int(data_bright[0]), int(data_bright[1]), int(data_bright[2])),
            )
        if OUTER_RADIUS > 0.0 and static_layer is None:
            yy, xx = np.indices((H, W))
            rr = np.sqrt((xx - W / 2.0) ** 2 + (yy - H / 2.0) ** 2)
            draw_arr[rr > OUTER_RADIUS] = (int(bg[0]), int(bg[1]), int(bg[2]))
        frames.append(Image.fromarray(draw_arr, mode="RGB"))

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
