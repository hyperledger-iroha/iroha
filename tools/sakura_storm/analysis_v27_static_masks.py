import os
from dataclasses import dataclass
from typing import Dict, List, Tuple

import numpy as np
from PIL import Image


BASE_DIR = os.path.dirname(__file__)
REF_GIF = os.getenv(
    "SS_REF_GIF",
    os.path.join(
        BASE_DIR,
        "sakura_storm_encoded_preview_logo_3rings_v27_fullwidth_katakana_lowdensity_bright.gif",
    ),
)

DATA_COLS = {(255, 233, 246), (255, 234, 246), (68, 40, 54), (69, 40, 54)}
RING_BRIGHT = (255, 246, 252)
RING_DIM = (62, 36, 50)
LOGO_COLS = {(30, 16, 25), (36, 18, 28), (43, 20, 32)}


@dataclass
class MaskStats:
    area: int
    bbox_area: int
    bbox_w: int
    bbox_h: int
    fill_ratio: float
    best_pad: int
    best_pad_diff: float


def load_frames(path: str) -> np.ndarray:
    im = Image.open(path)
    frames = []
    for i in range(im.n_frames):
        im.seek(i)
        frames.append(np.array(im.convert("RGB"), dtype=np.uint8))
    return np.stack(frames, axis=0)


def build_exclusion_masks(frames: np.ndarray) -> Tuple[np.ndarray, np.ndarray]:
    ring_mask = (frames == np.array(RING_BRIGHT, dtype=np.uint8)).all(axis=3) | (
        frames == np.array(RING_DIM, dtype=np.uint8)
    ).all(axis=3)
    logo_mask = np.zeros(frames.shape[:3], dtype=bool)
    for col in LOGO_COLS:
        logo_mask |= (frames == np.array(col, dtype=np.uint8)).all(axis=3)
    return ring_mask, logo_mask


def data_mask_for_tile(tile: np.ndarray) -> np.ndarray:
    m = np.zeros(tile.shape[:2], dtype=bool)
    for col in DATA_COLS:
        m |= (tile == np.array(col, dtype=np.uint8)).all(axis=2)
    return m


def box_mask(cell: int, pad: int) -> np.ndarray:
    m = np.zeros((cell, cell), dtype=bool)
    if pad >= cell // 2:
        return m
    m[pad : cell - pad, pad : cell - pad] = True
    return m


def bbox_of(mask: np.ndarray) -> Tuple[int, int, int, int]:
    ys, xs = np.where(mask)
    if len(ys) == 0:
        return 0, 0, 0, 0
    y0 = int(ys.min())
    y1 = int(ys.max())
    x0 = int(xs.min())
    x1 = int(xs.max())
    return x0, y0, x1 + 1, y1 + 1


def analyze_static_masks(frames: np.ndarray, grid_n: int, cell: int) -> List[MaskStats]:
    ring_mask, logo_mask = build_exclusion_masks(frames)
    stats: List[MaskStats] = []
    for r in range(grid_n):
        for c in range(grid_n):
            y0 = r * cell
            x0 = c * cell
            if ring_mask[:, y0 : y0 + cell, x0 : x0 + cell].any() or logo_mask[
                :, y0 : y0 + cell, x0 : x0 + cell
            ].any():
                continue
            cell_masks = []
            for i in range(frames.shape[0]):
                tile = frames[i, y0 : y0 + cell, x0 : x0 + cell]
                m = data_mask_for_tile(tile)
                cell_masks.append(m)
            if not any(m.any() for m in cell_masks):
                continue
            uniq = {np.packbits(m.reshape(-1).astype(np.uint8)).tobytes() for m in cell_masks}
            if len(uniq) != 1:
                continue
            static_mask = cell_masks[0]
            if not static_mask.any():
                continue
            area = int(static_mask.sum())
            x0b, y0b, x1b, y1b = bbox_of(static_mask)
            bbox_w = x1b - x0b
            bbox_h = y1b - y0b
            bbox_area = bbox_w * bbox_h if bbox_w > 0 and bbox_h > 0 else 0
            fill_ratio = float(area / bbox_area) if bbox_area else 0.0
            best_pad = -1
            best_diff = 1.0
            for pad in range(0, cell // 2 + 1):
                bm = box_mask(cell, pad)
                diff = float(np.mean(static_mask != bm))
                if diff < best_diff:
                    best_diff = diff
                    best_pad = pad
            stats.append(
                MaskStats(
                    area=area,
                    bbox_area=bbox_area,
                    bbox_w=bbox_w,
                    bbox_h=bbox_h,
                    fill_ratio=fill_ratio,
                    best_pad=best_pad,
                    best_pad_diff=best_diff,
                )
            )
    return stats


def summarize(stats: List[MaskStats]) -> None:
    if not stats:
        print("no static masks found")
        return
    areas = np.array([s.area for s in stats])
    fill = np.array([s.fill_ratio for s in stats])
    pad = np.array([s.best_pad for s in stats])
    diff = np.array([s.best_pad_diff for s in stats])
    print("static masks", len(stats))
    print("area mean", float(areas.mean()), "std", float(areas.std()))
    print("fill mean", float(fill.mean()), "std", float(fill.std()))
    print("pad mode", int(np.bincount(pad).argmax()))
    print("pad diff mean", float(diff.mean()), "std", float(diff.std()))
    for p in range(0, max(1, int(pad.max()) + 1)):
        count = int((pad == p).sum())
        if count:
            avg_diff = float(diff[pad == p].mean())
            print("pad", p, "count", count, "avg_diff", f"{avg_diff:.3f}")


def main() -> None:
    frames = load_frames(REF_GIF)
    grid_n = frames.shape[1] // 16
    cell = frames.shape[1] // grid_n
    stats = analyze_static_masks(frames, grid_n, cell)
    summarize(stats)


if __name__ == "__main__":
    main()
