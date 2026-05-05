---
lang: he
direction: rtl
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce0772b1c8b387704d6b07a53c552a8b1dedc913ead40275616c052d0ea473052
source_last_modified: "2025-12-26T11:12:17.796371+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/kagami_profiles.md -->

# פרופילי Kagami עבור Iroha3

Kagami מספקת presets לרשתות Iroha 3 כדי שמפעילים יוכלו להחתים מניפסטים של
genesis דטרמיניסטיים בלי לנהל knobs לכל רשת בנפרד.

- פרופילים: `iroha3-dev` (שרשרת `iroha3-dev.local`, אוספים k=1 r=1, seed VRF נגזר
  מ‑chain id כאשר NPoS נבחר), `iroha3-taira` (שרשרת `iroha3-taira`, אוספים k=3 r=3,
  דורש `--vrf-seed-hex` כאשר NPoS נבחר), `iroha3-nexus` (שרשרת `iroha3-nexus`,
  אוספים k=5 r=3, דורש `--vrf-seed-hex` כאשר NPoS נבחר).
- קונצנזוס: רשתות פרופיל Sora (Nexus + dataspaces) דורשות NPoS ואוסרות staged
  cutovers; פריסות Iroha3 permissioned חייבות לרוץ ללא פרופיל Sora.
- יצירה: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`.
  השתמשו ב‑`--consensus-mode npos` עבור Nexus; `--vrf-seed-hex` תקף רק ל‑NPoS
  (נדרש עבור taira/nexus). Kagami מקבע DA/RBC בקו Iroha3 ומפיקה סיכום (שרשרת,
  אוספים, DA/RBC, seed VRF, fingerprint).
- אימות: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]`
  מריץ מחדש ציפיות פרופיל (chain id, DA/RBC, אוספים, כיסוי PoP, fingerprint קונצנזוס).
  ספקו `--vrf-seed-hex` רק בעת אימות מניפסט NPoS עבור taira/nexus.
- חבילות לדוגמה: חבילות מוכנות נמצאות תחת `defaults/kagami/iroha3-{dev,taira,nexus}/`
  (genesis.json, config.toml, docker-compose.yml, verify.txt, README).
  חזרו וצרו עם `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]`.
- Mochi: `mochi`/`mochi-genesis` מקבלים `--genesis-profile <profile>` ו‑`--vrf-seed-hex <hex>`
  (NPoS בלבד), מעבירים אותם ל‑Kagami, ומדפיסים את אותו סיכום Kagami ל‑stdout/stderr
  כאשר משתמשים בפרופיל.

החבילות משבצות BLS PoPs לצד רשומות הטופולוגיה כך ש‑`kagami verify` מצליח
מהקופסה; התאימו את peers/ports המהימנים בקונפיגים לפי הצורך להרצות smoke מקומיות.

</div>
