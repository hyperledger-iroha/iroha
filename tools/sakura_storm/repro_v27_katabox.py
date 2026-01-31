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
    env_base.setdefault("SS_PALETTE_PRESET", "defi_crimson_bold")
    env_base.setdefault("SS_KATAKANA_OVERLAY", "1")
    env_base.setdefault("SS_KATAKANA_BOXES", "1")
    env_base.setdefault("SS_KATAKANA_DIFF", "1")
    env_base.setdefault("SS_KATAKANA_TEXT_ON", "0,0,0")
    env_base.setdefault("SS_KATAKANA_TEXT_OFF", "255,230,230")
    env_base.setdefault("SS_KATAKANA_BOX_ON", "255,70,70")
    env_base.setdefault("SS_KATAKANA_BOX_OFF", "30,12,16")
    env_base.setdefault("SS_KATAKANA_ALPHA", "140")
    env_base.setdefault("SS_KATAKANA_BOX_ALPHA", "140")
    env_base.setdefault("SS_KATAKANA_SCALE", "0.85")
    env_base.setdefault("SS_RING_DIM_ANGLES", "0,120,240")
    env_base.setdefault("SS_LOGO_SCALE", "0.40")
    env_base.setdefault("SS_LOGO_THRESH_1", "170")
    env_base.setdefault("SS_LOGO_THRESH_2", "220")
    env_base.setdefault("SS_LOGO_THRESH_3", "240")

    presets_raw = env_base.get("SS_PALETTE_PRESETS", "").strip()
    if presets_raw:
        presets = [p.strip() for p in presets_raw.split(",") if p.strip()]
    else:
        presets = [env_base.get("SS_PALETTE_PRESET", "v27")]

    for preset in presets:
        run_for_preset(out_dir, env_base, preset)


if __name__ == "__main__":
    main()
