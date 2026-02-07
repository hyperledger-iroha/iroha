---
lang: kk
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-12-29T18:16:35.095373+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Anchor Lineage пакет үлгісі (SN13-C)

Жол картасы элементі **SN13-C — Манифесттер және SoraNS анкерлері** әрбір бүркеншік атты қажет етеді
детерминирленген дәлелдер бумасын жіберу үшін айналдыру. Осы үлгіні өзіңіздің файлыңызға көшіріңіз
шығару артефакті каталогы (мысалы
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) және ауыстырыңыз
пакетті басқаруға жібермес бұрын толтырғыштар.

## 1. Метадеректер

| Өріс | Мән |
|-------|-------|
| Оқиға идентификаторы | `<taikai.event.launch-2026-07-10>` |
| Ағын / көрсету | `<main-stage>` |
| Бүркеншік ат аттар кеңістігі / аты | `<sora / docs>` |
| Дәлелдер анықтамалығы | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| Оператор байланысы | `<name + email>` |
| GAR / RPT билеті | `<governance ticket or GAR digest>` |

## Бума көмекшісі (міндетті емес)

Спуль артефактілерін көшіріп, бұрын JSON (қосымша қол қойылған) қорытындысын шығарыңыз
қалған бөлімдерді толтыру:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

Көмекші `taikai-anchor-request-*`, `taikai-trm-state-*`,
`taikai-lineage-*`, конверттер мен күзетшілер Taikai катушкасының каталогындағы
(`config.da_ingest.manifest_store_dir/taikai`) сондықтан дәлелдер қалтасы әлдеқашан
төменде сілтеме жасалған нақты файлдарды қамтиды.

## 2. Әулеттік кітап және кеңес

Дискідегі желілік кітапты және бұл үшін жазған JSON Torii анықтамасын тіркеңіз
терезе. Бұлар тікелей келеді
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` және
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| Артефакт | Файл | SHA-256 | Ескертпелер |
|----------|------|---------|-------|
| Тегіс кітапшасы | `taikai-trm-state-docs.json` | `<sha256>` | Алдыңғы манифест дайджест/терезесін дәлелдейді. |
| Тегі туралы анықтама | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | SoraNS анкеріне жүктеп салу алдында түсірілген. |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. Анкерлік пайдалы жүкті түсіру

Torii якорь қызметіне жеткізілген POST пайдалы жүктемесін жазыңыз. Пайдалы жүк
құрамында `envelope_base64`, `ssm_base64`, `trm_base64` және кірістірілген
`lineage_hint` нысаны; Аудиттер бұл түсінікті дәлелдеу үшін осы түсіруге сүйенеді
SoraNS-ке жіберілді. Torii енді бұл JSON автоматты түрде келесідей жазады
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
Taikai спул каталогында (`config.da_ingest.manifest_store_dir/taikai/`), сондықтан
операторлар HTTP журналдарын сызып алудың орнына оны тікелей көшіре алады.

| Артефакт | Файл | SHA-256 | Ескертпелер |
|----------|------|---------|-------|
| Anchor POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | `taikai-anchor-request-*.json` (Taikai спулы) ішінен өңделмеген сұрау көшірілді. |

## 4. Манифест дайджестті мойындау

| Өріс | Мән |
|-------|-------|
| Жаңа манифест дайджест | `<hex digest>` |
| Алдыңғы манифест дайджесті (анықтамадан) | `<hex digest>` |
| Терезенің басталуы/соңы | `<start seq> / <end seq>` |
| Қабылдау уақыт белгісі | `<ISO8601>` |

Жоғарыда жазылған кітапқа/кеңес хэштеріне сілтеме жасаңыз, осылайша шолушылар тексере алады
ауыстырылған терезе.

## 5. Көрсеткіштер / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` суреті: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` демп (бүркеншік ат бойынша): `<file path + hash>`

Есептегішті көрсететін Prometheus/Grafana экспортын немесе `curl` шығысын қамтамасыз етіңіз
арттыру және осы бүркеншік ат үшін `/status` массиві.

## 6. Дәлелдер каталогына арналған манифест

Дәлелдер каталогының детерминирленген манифестін жасаңыз (спул файлдары,
пайдалы жүктемені түсіру, метриканың суреті) сондықтан басқару әрбір хэшті онсыз тексере алады
мұрағатты ашу.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Артефакт | Файл | SHA-256 | Ескертпелер |
|----------|------|---------|-------|
| Дәлелдер манифесті | `manifest.json` | `<sha256>` | Мұны басқару пакетіне / GAR тіркеңіз. |

## 7. Бақылау парағы

- [ ] Тірек кітабы көшірілген + хэштелген.
- [ ] Тегі туралы анықтама көшірілді + хэштелген.
- [ ] Anchor POST пайдалы жүктемесі түсірілді және хэштелген.
- [ ] Манифест дайджест кестесі толтырылды.
- [ ] Көрсеткіштердің суреті экспортталды (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] `scripts/repo_evidence_manifest.py` көмегімен жасалған манифест.
- [ ] Хэштері + байланыс ақпараты бар басқаруға жүктеп салынған пакет.

Әрбір бүркеншік ат айналымы үшін осы үлгіні сақтау SoraNS басқаруын сақтайды
репродуктивті топтаманы біріктіріңіз және тектік кеңестерді GAR/RPT дәлелдеріне тікелей байланыстырады.