#!/usr/bin/env bash
set -euo pipefail

# Generate a simple syscall table from crates/ivm/src/syscalls.rs
# Output: Markdown on stdout

SRC="crates/ivm/src/syscalls.rs"
if [[ ! -f "$SRC" ]]; then
  echo "Error: $SRC not found" >&2
  exit 1
fi

echo "# Generated IVM Syscall Table"
echo
echo "This file is generated from \`$SRC\`. Edit the source to change syscall numbers; then re-run this script."
echo
echo "| Name | Value (hex) | Note |"
echo "|------|-------------|------|"

# Extract const definitions
# Formats handled:
#  - pub const NAME: u32 = 0xNN;
#  - pub const NAME: u32 = DECIMAL;
#  - pub const NAME: u32 = OTHER_CONST;   (alias)

awk '
  BEGIN { FS="[ =;]+" }
  /^pub const SYSCALL_/ {
    name=$3;
    # value token may be at $6 depending on splitting; safer to gsub then split
    line=$0;
    # strip trailing comments
    sub(/\/\/.*$/, "", line)
    # extract rhs after = and before ;
    if (match(line, /=\s*([^;]+);/, m)) {
      rhs=m[1];
      gsub(/^\s+|\s+$/, "", rhs);
      note="";
      if (rhs ~ /^0x[0-9A-Fa-f]+$/) {
        val=rhs;
      } else if (rhs ~ /^[0-9]+$/) {
        # decimal → hex
        dec=rhs+0;
        # convert to hex
        hex=sprintf("0x%X", dec);
        val=hex;
      } else if (rhs ~ /^SYSCALL_/) {
        val="";
        note="alias of " rhs;
      } else {
        val="";
        note=rhs;
      }
      printf("| %s | %s | %s |\n", name, val, note);
    }
  }
' "$SRC"

echo
echo "Note: Aliases resolve to the value of their target constant at compile time."

