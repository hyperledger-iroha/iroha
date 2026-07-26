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


def main() -> None:
    out_dir = pick_out_dir()
    os.makedirs(out_dir, exist_ok=True)

    env_base = dict(os.environ)
    env_base["SS_KATAKANA_MODE"] = "iroha"

    proc_gif = os.path.join(out_dir, "sakura_storm_v27_procedural_iroha.gif")
    env_proc = dict(env_base)
    env_proc["SS_OUT_GIF"] = proc_gif
    run([sys.executable, os.path.join(BASE_DIR, "gen_sakura_v27_procedural.py")], env_proc)

    symbols_npz = os.path.join(out_dir, "v27_symbols_iroha.npz")
    env_extract = dict(env_base)
    env_extract["SS_REF_GIF"] = proc_gif
    env_extract["SS_OUT_NPZ"] = symbols_npz
    run([sys.executable, os.path.join(BASE_DIR, "extract_v27_symbols.py")], env_extract)

    symbols_gif = os.path.join(out_dir, "sakura_storm_v27_from_symbols_iroha.gif")
    env_symbols = dict(env_base)
    env_symbols["SS_SYMBOLS_NPZ"] = symbols_npz
    env_symbols["SS_OUT_GIF"] = symbols_gif
    run([sys.executable, os.path.join(BASE_DIR, "gen_sakura_v27_symbols.py")], env_symbols)

    env_compare = dict(env_base)
    env_compare["SS_REF_GIF"] = proc_gif
    env_compare["SS_CAND_GIF"] = symbols_gif
    run([sys.executable, os.path.join(BASE_DIR, "compare_v27.py")], env_compare)


if __name__ == "__main__":
    main()
