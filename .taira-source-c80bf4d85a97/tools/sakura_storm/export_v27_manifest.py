import json
import os
from dataclasses import asdict

from v27_params import KATAKANA, KATAKANA_EXTRA, KATAKANA_IROHA_BASE, KATAKANA_V27, V27


BASE_DIR = os.path.dirname(__file__)
OUT_JSON = os.getenv("SS_OUT_JSON", os.path.join(BASE_DIR, "v27_manifest.json"))


def main() -> None:
    payload = asdict(V27)
    payload["katakana_iroha_base"] = KATAKANA_IROHA_BASE
    payload["katakana_extra"] = KATAKANA_EXTRA
    payload["katakana"] = KATAKANA
    payload["katakana_v27"] = KATAKANA_V27
    with open(OUT_JSON, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2, sort_keys=True)
        f.write("\n")
    print("manifest", OUT_JSON)


if __name__ == "__main__":
    main()
