import os
import subprocess
import sys


BASE_DIR = os.path.dirname(__file__)


def pick_out_dir() -> str:
    out_dir = os.getenv("SS_OUT_DIR", "").strip()
    if out_dir:
        return out_dir
    if os.path.isdir("/tmp/sakura_storm_viz"):
        return "/tmp/sakura_storm_viz"
    return BASE_DIR


def run(cmd: list[str], env: dict[str, str]) -> None:
    print("+", " ".join(cmd))
    subprocess.run(cmd, check=True, env=env)


def run_for_preset(out_dir: str, env_base: dict[str, str], preset: str) -> None:
    env = dict(env_base)
    env["SS_PALETTE_PRESET"] = preset
    preset_tag = preset or "v27"

    base_gif = os.path.join(out_dir, f"sakura_storm_v27_base_for_overlay_{preset_tag}.gif")
    env_basegen = dict(env)
    env_basegen["SS_OUT_GIF"] = base_gif
    env_basegen["SS_KATAKANA_OVERLAY"] = "0"
    env_basegen["SS_KATAKANA_BOXES"] = "0"
    env_basegen["SS_KATAKANA_DIFF"] = "0"
    env_basegen["SS_LOGO_OVERLAY"] = "0"
    run([sys.executable, os.path.join(BASE_DIR, "gen_sakura_v27_procedural.py")], env_basegen)

    symbols_npz = os.path.join(out_dir, f"v27_symbols_katabox_{preset_tag}.npz")
    env_extract = dict(env)
    env_extract["SS_REF_GIF"] = base_gif
    env_extract["SS_OUT_NPZ"] = symbols_npz
    run([sys.executable, os.path.join(BASE_DIR, "extract_v27_symbols.py")], env_extract)

    overlay_gif = os.path.join(out_dir, f"sakura_storm_v27_katabox_{preset_tag}.gif")
    env_symbols = dict(env)
    env_symbols["SS_SYMBOLS_NPZ"] = symbols_npz
    env_symbols["SS_OUT_GIF"] = overlay_gif
    run([sys.executable, os.path.join(BASE_DIR, "gen_sakura_v27_symbols.py")], env_symbols)


def main() -> None:
    out_dir = pick_out_dir()
    os.makedirs(out_dir, exist_ok=True)

    env_base = dict(os.environ)
    env_base.setdefault("SS_KATAKANA_MODE", "iroha")
    env_base.setdefault("SS_PALETTE_PRESET", "v26_preview")
    env_base.setdefault("SS_GRID_N", "50")
    env_base.setdefault("SS_CELL_SIZE", "10")
    env_base.setdefault("SS_GRID_OFFSET", "6")
    env_base.setdefault("SS_GLYPH_SIZE", "9")
    env_base.setdefault("SS_GLYPH_THRESH", "70")
    v26_ref = "/tmp/sakura_storm_viz/sakura_storm_encoded_preview_logo_3rings_v26_fullwidth_katakana.gif"
    if os.path.exists(v26_ref):
        env_base.setdefault("SS_LOGO_MASK_REF", v26_ref)
        env_base.setdefault("SS_LOGO_MASK_COLORS", "29,16,24;36,18,28;43,20,32")
    env_base.setdefault("SS_ACTIVE_BANDS", "0-243")
    env_base.setdefault("SS_KATAKANA_OVERLAY", "1")
    env_base.setdefault("SS_KATAKANA_BOXES", "1")
    env_base.setdefault("SS_KATAKANA_DIFF", "1")
    env_base.setdefault("SS_KATAKANA_TEXT_ON", "0,0,0")
    env_base.setdefault("SS_KATAKANA_TEXT_OFF", "255,244,250")
    env_base.setdefault("SS_KATAKANA_BOX_ON", "255,225,240")
    env_base.setdefault("SS_KATAKANA_BOX_OFF", "92,56,72")
    env_base.setdefault("SS_KATAKANA_ALPHA", "255")
    env_base.setdefault("SS_KATAKANA_BOX_ALPHA", "255")
    env_base.setdefault("SS_KATAKANA_SCALE", "1.06")
    env_base.setdefault("SS_KATAKANA_BOX_SCALE", "1.08")
    env_base.setdefault("SS_RING_DIM_ANGLES", "0,120,240")
    env_base.setdefault("SS_LOGO_SCALE", "0.12")
    env_base.setdefault("SS_LOGO_ALPHA", "0")
    env_base.setdefault("SS_LOGO_THRESH_1", "120")
    env_base.setdefault("SS_LOGO_THRESH_2", "170")
    env_base.setdefault("SS_LOGO_THRESH_3", "210")
    env_base.setdefault("SS_LOGO_MASK_THRESH", "200")
    env_base.setdefault("SS_LOGO_OVERLAY", "0")
    env_base.setdefault("SS_LOGO_OVERLAY_ALPHA", "255")
    env_base.setdefault("SS_LOGO_OVERLAY_COLOR", "255,255,255")
    env_base.setdefault("SS_LOGO_OVERLAY_POST", "1")
    env_base.setdefault("SS_KATAKANA_OUTSIDE_LOGO", "1")
    env_base.setdefault("SS_MASK_LOGO", "1")

    presets_raw = env_base.get("SS_PALETTE_PRESETS", "").strip()
    if presets_raw:
        presets = [p.strip() for p in presets_raw.split(",") if p.strip()]
    else:
        presets = [env_base.get("SS_PALETTE_PRESET", "v27")]

    for preset in presets:
        run_for_preset(out_dir, env_base, preset)


if __name__ == "__main__":
    main()
