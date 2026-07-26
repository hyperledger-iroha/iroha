---
lang: kk
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2025-12-29T18:16:35.926476+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 ЕО заңды қол қою туралы жазбасы — 2026.1 GA (Android SDK)

## Түйіндеме

- **Шығарылым / Пойыз:** 2026.1 GA (Android SDK)
- **Шолу күні:** 2026-04-15
- **Кеңесші / Шолушы:** София Мартинс — Сәйкестік және заң
- **Қолдану аясы:** ETSI EN 319 401 қауіпсіздік мақсаты, GDPR DPIA қысқаша мазмұны, SBOM аттестаттауы, AND6 құрылғы-зертханасының күтпеген жағдайының дәлелі
- **Байланысты билеттер:** `_android-device-lab` / AND6-DR-202602, AND6 басқару трекері (`GOV-AND6-2026Q1`)

## Артефакттарды тексеру тізімі

| Артефакт | SHA-256 | Орналасқан жері / Сілтеме | Ескертпелер |
|----------|---------|-----------------|-------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | 2026.1 GA шығарылым идентификаторлары мен қауіп үлгісінің дельталарына сәйкес келеді (Torii NRPC толықтырулары). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Анықтамалар AND7 телеметрия саясаты (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore жинағы (`android-sdk-release#4821`). | CycloneDX + шығу тегі қаралды; Buildkite тапсырмасына `android-sdk-release#4821` сәйкес келеді. |
| Дәлелдер журналы | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (`android-device-lab-failover-20260220` жолы) | Журналдың түсірілген бума хэштері + сыйымдылық суреті + жады жазбасын растайды. |
| Құрылғы-зертхананың төтенше жағдайлар жинағы | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | `bundle-manifest.json`-тен алынған хэш; AND6-DR-202602 билеті Заңға/Сәйкестікке тапсырылған. |

## Қорытындылар және ерекшеліктер

- Ешқандай блоктау мәселелері анықталмаған. Артефактілер ETSI/GDPR талаптарына сәйкес келеді; AND7 телеметрия паритеті DPIA қорытындысында көрсетілген және қосымша жұмсартулар қажет емес.
- Ұсыныс: жоспарланған DR-2026-05-Q2 жаттығуын бақылаңыз (билет AND6-DR-202605) және нәтижелер топтамасын келесі басқару тексеру нүктесіне дейін дәлелдер журналына қосыңыз.

## Бекіту

- **Шешім:** Бекітілді
- **Қолы / Уақыт белгісі:** _София Мартинс (басқару порталы арқылы цифрлық қолтаңба, 2026-04-15 14:32 UTC)_
- **Кейінгі иелері:** Device Lab Ops (DR-2026-05-Q2 дәлелдемелер жинағын 2026-05-31 дейін жеткізу)