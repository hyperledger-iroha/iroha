from pathlib import Path

path = Path("/Users/administrator/dev/iroha-build-taira-latest/crates/iroha_core/src/block.rs")
text = path.read_text()

new_hash = """        [
            127, 253, 243, 16, 56, 84, 148, 21, 121, 38, 145, 202, 29, 204, 49, 113, 127, 74, 95,
            145, 75, 228, 201, 193, 47, 33, 181, 167, 92, 108, 248, 61,
        ],
"""

if "127, 253, 243, 16, 56, 84, 148" in text:
    print("legacy hash already present")
else:
    text = text.replace(
        "const LEGACY_TAIRA_ZK_POLICY_HASHES: [[u8; 32]; 2] = [",
        "const LEGACY_TAIRA_ZK_POLICY_HASHES: [[u8; 32]; 3] = [",
        1,
    )
    marker = """        [
            40, 173, 221, 159, 39, 238, 176, 56, 202, 219, 191, 211, 103, 68, 251, 108, 152,
            88, 38, 166, 13, 99, 153, 170, 152, 200, 97, 80, 160, 147, 6, 254,
        ],
"""
    if marker not in text:
        raise SystemExit("legacy hash insertion marker not found")
    text = text.replace(marker, marker + new_hash, 1)
    path.write_text(text)
    print("legacy hash inserted")
