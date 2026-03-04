import os
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
CAND_GIF = os.getenv(
    "SS_CAND_GIF",
    "/tmp/sakura_storm_viz/sakura_storm_v27_procedural.gif"
    if os.path.isdir("/tmp/sakura_storm_viz")
    else os.path.join(BASE_DIR, "sakura_storm_v27_procedural.gif"),
)


def load_frames(path: str) -> np.ndarray:
    im = Image.open(path)
    frames = []
    for i in range(im.n_frames):
        im.seek(i)
        frames.append(np.array(im.convert("RGB"), dtype=np.uint8))
    return np.stack(frames, axis=0)


def main() -> None:
    ref = load_frames(REF_GIF)
    cand = load_frames(CAND_GIF)
    if ref.shape != cand.shape:
        print("shape mismatch", ref.shape, cand.shape)
        return
    diff = np.abs(ref.astype(np.int16) - cand.astype(np.int16))
    mad = float(np.mean(diff))
    pix_diff = float(np.mean(np.any(diff > 0, axis=3)))
    print("MAD", f"{mad:.3f}")
    print("pixel_diff_frac", f"{pix_diff:.3f}")


if __name__ == "__main__":
    main()
