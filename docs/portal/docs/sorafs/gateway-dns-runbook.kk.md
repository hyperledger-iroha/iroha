---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 672a5e3a6f0c3e8999400bc6fa8c66cc3be1ba2119431c5fd26f6d9a436f767f
source_last_modified: "2025-12-29T18:16:35.187152+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS шлюз және DNS Kickoff Runbook

Бұл портал көшірмесі канондық жұмыс кітабын көрсетеді
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Ол орталықтандырылмаған DNS және шлюз үшін операциялық қоршауларды басып алады
жұмыс ағыны, сондықтан желі, операциялар және құжаттама жетекшілері қайталай алады
2025-03 басталуының алдында автоматтандыру стек.

## Қолдану аясы және жеткізілімдері

- Детерминистикалық репетиция арқылы DNS (SF‑4) және шлюз (SF‑5) кезеңдерін байланыстырыңыз
  хост туындысы, шешуші каталогының шығарылымдары, TLS/GAR автоматтандыруы және дәлелдер
  басып алу.
- Бастама кірістерін сақтаңыз (күн тәртібі, шақыру, қатысуды бақылаушы, GAR телеметриясы
  сурет) соңғы иесі тағайындауларымен синхрондалған.
- Басқаруды тексерушілер үшін тексерілетін артефакт жинағын жасаңыз: шешуші
  каталогтың шығарылым жазбалары, шлюзді тексеру журналдары, сәйкестік белдеуінің шығысы және
  Docs/DevRel қорытындысы.

## Рөлдер мен жауапкершіліктер

| Жұмыс ағыны | Міндеттері | Қажетті артефактілер |
|------------|------------------|--------------------|
| Желілік TL (DNS стегі) | Детерминирленген хост жоспарын сақтаңыз, RAD каталогының шығарылымдарын іске қосыңыз, шешуші телеметрия кірістерін жариялаңыз. | `artifacts/soradns_directory/<ts>/`, `docs/source/soradns/deterministic_hosts.md` үшін айырмашылықтар, RAD метадеректері. |
| Ops Automation Lead (шлюз) | TLS/ECH/GAR автоматтандыру жаттығуларын орындаңыз, `sorafs-gateway-probe` іске қосыңыз, PagerDuty ілмектерін жаңартыңыз. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON зонды, `ops/drill-log.md` жазбалары. |
| QA Guild & Tooling WG | `ci/check_sorafs_gateway_conformance.sh` іске қосыңыз, құрылғыларды өңдеңіз, Norito өзін-өзі растау жинақтарын мұрағатлаңыз. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Құжаттар / DevRel | Хаттамаларды жазып алыңыз, дизайнды алдын ала оқылған + қосымшаларды жаңартыңыз және осы порталда дәлелдер жиынтығын жариялаңыз. | `docs/source/sorafs_gateway_dns_design_*.md` файлдары мен шығару жазбалары жаңартылды. |

## Енгізулер және алғышарттар

- Детерминистік хост спецификациясы (`docs/source/soradns/deterministic_hosts.md`) және
  шешуші аттестаттау тірегі (`docs/source/soradns/resolver_attestation_directory.md`).
- Шлюз артефактілері: оператор анықтамалығы, TLS/ECH автоматтандыру көмекшілері,
  тікелей режим нұсқаулығы және `docs/source/sorafs_gateway_*` астында өзін-өзі сертификаттау жұмыс процесі.
- Құралдар: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh` және CI көмекшілері
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Құпиялар: GAR шығару кілті, DNS/TLS ACME тіркелгі деректері, PagerDuty бағыттау кілті,
  Шешушілерді алу үшін Torii аутентификация белгісі.

## Ұшу алдындағы бақылау тізімі

1. Қатысушылар мен күн тәртібін жаңарту арқылы растаңыз
   `docs/source/sorafs_gateway_dns_design_attendance.md` және айналымда
   ағымдағы күн тәртібі (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Сахналық артефакті тамырлар сияқты
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` және
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Жаңарту құрылғылары (GAR манифесттері, RAD дәлелдері, шлюз сәйкестік бумалары) және
   `git submodule` күйінің соңғы репетиция тегіне сәйкес келетініне көз жеткізіңіз.
4. Құпияларды тексеріңіз (Ed25519 шығару кілті, ACME тіркелгі файлы, PagerDuty белгісі)
   қойманың бақылау сомасын көрсету және сәйкестендіру.
5. Түтінге қарсы телеметрия мақсаттары (Pushgateway соңғы нүктесі, GAR Grafana тақтасы) бұрын
   бұрғыға.

## Автоматтандыруды қайталау қадамдары

### Детерминистік хост картасы және RAD каталогының шығарылымы

1. Ұсынылған манифестке қарсы детерминирленген хост туынды көмекшісін іске қосыңыз
   орнатыңыз және ешбір ауытқудың жоқтығын растаңыз
   `docs/source/soradns/deterministic_hosts.md`.
2. Шешуші каталогтар бумасын жасаңыз:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Басып шығарылған каталог идентификаторын, SHA-256 және ішіндегі шығыс жолдарын жазыңыз
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` және старт
   минут.

### DNS телеметриясын түсіру

- Қолдану арқылы ≥10 минутқа арналған құйрықты шешуші мөлдірлік журналдары
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Pushgateway метрикасын экспорттаңыз және NDJSON суреттерін іске қосумен бірге мұрағаттаңыз
  ID каталогы.

### Шлюзді автоматтандыру жаттығулары

1. TLS/ECH зондын орындаңыз:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Сәйкестік сымын (`ci/check_sorafs_gateway_conformance.sh`) және іске қосыңыз
   жаңарту үшін өзін-өзі растау көмекшісі (`scripts/sorafs_gateway_self_cert.sh`).
   Norito аттестаттау жинағы.
3. Автоматтандыру жолының соңына дейін жұмыс істейтінін дәлелдеу үшін PagerDuty/Webhook оқиғаларын түсіріңіз
   соңы.

### Дәлелдемелерді орау

- `ops/drill-log.md` уақыт белгілерімен, қатысушылармен және зерттеу хэштерімен жаңартыңыз.
- Артефактілерді іске қосу идентификаторы каталогтары астында сақтаңыз және атқарушы қорытындыны жариялаңыз
  Docs/DevRel жиналысының хаттамаларында.
- Бастауды қарау алдында басқару билетіндегі дәлелдер жинағын байланыстырыңыз.

## Сеансты жеңілдету және дәлелдемелерді тапсыру

- **Модератор хронологиясы:**  
  - T‑24h — Бағдарламаны басқару `#nexus-steering` ішінде еске салғыш + күн тәртібі/қатысу суретін орналастырады.  
  - T‑2h — Networking TL GAR телеметрия суретін жаңартады және дельталарды `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` ішінде жазады.  
  - T‑15m — Ops Automation зондтың дайындығын тексереді және `artifacts/sorafs_gateway_dns/current` ішіне белсенді іске қосу идентификаторын жазады.  
  - Қоңырау кезінде — Модератор осы runbook-пен бөліседі және тірі жазушыны тағайындайды; Docs/DevRel түсіру әрекет элементтерін кірістірілген.
- **Минут үлгісі:** Қаңқаны мына жерден көшіріңіз
  `docs/source/sorafs_gateway_dns_design_minutes.md` (сонымен қатар порталда көрсетілген
  бума) және сеансқа бір толтырылған дананы орындаңыз. Қатысушылар тізімін қосыңыз,
  шешімдер, әрекет элементтері, дәлелдемелердің хэштері және өтелмеген тәуекелдер.
- **Дәлелдерді жүктеп салу:** Жаттығудан `runbook_bundle/` каталогын көшіріп алыңыз,
  көрсетілген PDF хаттамаларын тіркеңіз, SHA-256 хэштерін хаттамаға жазыңыз + күн тәртібі,
  содан кейін жүктеп салғаннан кейін басқару шолушысы бүркеншік атына пинг жасаңыз
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Дәлелдердің суреті (2025 жылғы наурыздың басталуы)

Жол картасында және басқаруда сілтеме жасалған соңғы репетиция/тірі артефактілер
минут `s3://sora-governance/sorafs/gateway_dns/` шелегі астында тұрады. Хэштер
төменде канондық манифестті көрсетеді (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Құрғақ жүгіру — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Топтама тарбол: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF минуттары: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Тікелей семинар — 03.03.2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Жүктеп салуды күтуде: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel SHA-256 файлын көрсетілген PDF жинаққа кірген кезде қосады.)_

## Қатысты материал

- [Шлюз операцияларының оқу кітабы](./operations-playbook.md)
- [SoraFS бақылау жоспары](./observability-plan.md)
- [Орталықтандырылмаған DNS және шлюз трекері](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)