---
lang: mn
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-28T04:31:10.012056+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Iroha3 профайл

Kagami нь Iroha 3 сүлжээнд зориулсан урьдчилан тохируулгыг илгээдэг тул операторууд детерминистикийг тамгалах боломжтой
генезис нь сүлжээ тус бүрийн товчлууруудыг жонглёрлохгүйгээр илэрдэг.

- Профайл: `iroha3-dev` (гинж `iroha3-dev.local`, коллекторууд k=1 r=1, NPoS сонгогдсон үед гинжин хэлхээний id-ээс авсан VRF үр), `iroha3-taira` (гинж `iroha3-taira`, коллекторт k=3r шаардлагатай), NPoS сонгосон үед `--vrf-seed-hex`), `iroha3-nexus` (гинж `iroha3-nexus`, коллекторууд k=5 r=3, NPoS-г сонгох үед `--vrf-seed-hex` шаардлагатай).
- Зөвшилцөл: Sora профайл сүлжээ (Nexus + өгөгдлийн орон зай) нь NPoS шаарддаг бөгөөд үе шаттай таслалтыг зөвшөөрөхгүй; зөвшөөрөгдсөн Iroha3 байршуулалт нь Sora профайлгүйгээр ажиллах ёстой.
- Үе: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. Nexus-д `--consensus-mode npos` ашиглах; `--vrf-seed-hex` зөвхөн NPoS-д хүчинтэй (taira/nexus-д шаардлагатай). Kagami DA/RBC-ийг Iroha3 шугам дээр холбож, хураангуй (гинж, коллектор, DA/RBC, VRF үр, хурууны хээ) гаргадаг.
- Баталгаажуулалт: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` профайлын хүлээлтийг (гинжин хэлхээний ID, DA/RBC, цуглуулагч, PoP хамрах хүрээ, зөвшилцөлд хүрсэн хурууны хээ) дахин тоглуулдаг. Зөвхөн taira/nexus-ийн NPoS манифестийг баталгаажуулах үед `--vrf-seed-hex`-г нийлүүлнэ үү.
- Жишээ багцууд: урьдчилан үүсгэсэн багцууд нь `defaults/kagami/iroha3-{dev,taira,nexus}/` (genesis.json, config.toml, docker-compose.yml, verify.txt, README) дагуу амьдардаг. `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]` ашиглан сэргээнэ үү.
- Mochi: `mochi`/`mochi-genesis` нь `--genesis-profile <profile>` болон `--vrf-seed-hex <hex>` (зөвхөн NPoS) -ийг хүлээн авч, тэдгээрийг Kagami руу дамжуулж, ашигласан үед Kagami-ыг нь хэвлэнэ.

Багцууд нь BLS PoP-г топологийн оруулгуудын хажууд суулгасан тул `kagami verify` амжилттай болно
хайрцагнаас гарсан; Орон нутгийн хувьд шаардлагатай бол тохиргоонд итгэмжлэгдсэн үе тэнгийнхэн/портуудыг тохируулна уу
утаа урсдаг.