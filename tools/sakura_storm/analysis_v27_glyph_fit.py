import os
import random
from typing import List, Tuple

import numpy as np
from PIL import Image, ImageDraw, ImageFont


BASE_DIR = os.path.dirname(__file__)
REF_GIF = os.getenv(
    "SS_REF_GIF",
    os.path.join(
        BASE_DIR,
        "sakura_storm_encoded_preview_logo_3rings_v27_fullwidth_katakana_lowdensity_bright.gif",
    ),
)

FONT_PATH = os.getenv("SS_FONT_PATH", "/System/Library/Fonts/Hiragino Sans GB.ttc")
FONT_INDICES = [int(x) for x in os.getenv("SS_FONT_INDICES", "0,1,2,3").split(",") if x.strip()]
SIZES = [int(x) for x in os.getenv("SS_SIZES", "13,14,15,16,17").split(",") if x.strip()]
STROKES = [int(x) for x in os.getenv("SS_STROKES", "0,1").split(",") if x.strip()]
THRESHOLDS = [int(x) for x in os.getenv("SS_THRESHOLDS", "64,96,128,160,192").split(",") if x.strip()]
SAMPLES = int(os.getenv("SS_SAMPLES", "200"))
SEED = int(os.getenv("SS_SEED", "1337"))


KATAKANA = list("アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン")
DATA_COLS = {(255, 233, 246), (255, 234, 246), (68, 40, 54), (69, 40, 54)}
RING_BRIGHT = (255, 246, 252)
RING_DIM = (62, 36, 50)
LOGO_COLS = {(30, 16, 25), (36, 18, 28), (43, 20, 32)}


def load_frames(path: str) -> np.ndarray:
    im = Image.open(path)
    frames = []
    for i in range(im.n_frames):
        im.seek(i)
        frames.append(np.array(im.convert("RGB"), dtype=np.uint8))
    return np.stack(frames, axis=0)


def collect_dynamic_masks(frames: np.ndarray, grid_n: int, cell: int) -> List[np.ndarray]:
    f, h, w = frames.shape[:3]
    ring_mask = (frames == np.array(RING_BRIGHT, dtype=np.uint8)).all(axis=3) | (
        frames == np.array(RING_DIM, dtype=np.uint8)
    ).all(axis=3)
    logo_mask = np.zeros(frames.shape[:3], dtype=bool)
    for col in LOGO_COLS:
        logo_mask |= (frames == np.array(col, dtype=np.uint8)).all(axis=3)
    masks: List[np.ndarray] = []
    for r in range(grid_n):
        for c in range(grid_n):
            y0 = r * cell
            x0 = c * cell
            if ring_mask[:, y0 : y0 + cell, x0 : x0 + cell].any() or logo_mask[
                :, y0 : y0 + cell, x0 : x0 + cell
            ].any():
                continue
            cell_masks = []
            for i in range(f):
                tile = frames[i, y0 : y0 + cell, x0 : x0 + cell]
                m = np.zeros((cell, cell), dtype=bool)
                for col in DATA_COLS:
                    m |= (tile == np.array(col, dtype=np.uint8)).all(axis=2)
                cell_masks.append(m)
            if not any(m.any() for m in cell_masks):
                continue
            uniq = {np.packbits(m.reshape(-1).astype(np.uint8)).tobytes() for m in cell_masks if m.any()}
            if len(uniq) <= 1:
                continue
            masks.extend([m for m in cell_masks if m.any()])
    return masks


def render_glyph_mask(
    font: ImageFont.FreeTypeFont,
    glyph: str,
    canvas: int,
    stroke: int,
    thresh: int,
) -> np.ndarray:
    img = Image.new("L", (canvas, canvas), 0)
    draw = ImageDraw.Draw(img)
    bbox = draw.textbbox((0, 0), glyph, font=font)
    gw = bbox[2] - bbox[0]
    gh = bbox[3] - bbox[1]
    tx = canvas / 2 - gw / 2
    ty = canvas / 2 - gh / 2
    draw.text((tx, ty), glyph, font=font, fill=255, stroke_width=stroke, stroke_fill=255)
    arr = np.array(img)
    return arr > thresh


def score_config(masks: List[np.ndarray], font_index: int, size: int, stroke: int, thresh: int) -> float:
    font = ImageFont.truetype(FONT_PATH, size, index=font_index)
    canvas = masks[0].shape[0]
    glyph_masks = [render_glyph_mask(font, g, canvas, stroke, thresh) for g in KATAKANA]
    diffs = []
    for m in masks:
        best = 1.0
        for gm in glyph_masks:
            diff = float(np.mean(m != gm))
            if diff < best:
                best = diff
        diffs.append(best)
    return float(np.mean(diffs))


def main() -> None:
    frames = load_frames(REF_GIF)
    grid_n = frames.shape[1] // 16
    cell = frames.shape[1] // grid_n
    masks = collect_dynamic_masks(frames, grid_n, cell)
    rng = random.Random(SEED)
    rng.shuffle(masks)
    sample = masks[: max(20, min(SAMPLES, len(masks)))]
    print("dynamic masks sampled", len(sample))

    results: List[Tuple[float, int, int, int, int]] = []
    for idx in FONT_INDICES:
        for size in SIZES:
            for stroke in STROKES:
                for thresh in THRESHOLDS:
                    score = score_config(sample, idx, size, stroke, thresh)
                    results.append((score, idx, size, stroke, thresh))
                    print("score", f"{score:.4f}", "idx", idx, "size", size, "stroke", stroke, "th", thresh)

    results.sort(key=lambda x: x[0])
    print("\nTop configs:")
    for r in results[:10]:
        print(r)


if __name__ == "__main__":
    main()
