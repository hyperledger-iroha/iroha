---
lang: kk
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 565d4e8bf0a043b2c83a03ec87a8c71a30da34f56d94a28cad03677963b3e69a
source_last_modified: "2025-12-29T18:16:35.206864+00:00"
translation_last_reviewed: 2026-02-07
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: Checklist to activate SNNet-9 compliance policies with attestation digests and evidence capture.
translator: machine-google-reviewed
---

Бұл қысқаша SNNet-9 сәйкестік конфигурациясын қайталанатын параметрмен шығару үшін пайдаланыңыз,
аудитке қолайлы процесс. Әрбір оператор үшін оны юрисдикциялық шолумен жұптаңыз
бірдей дайджесттер мен дәлелдемелердің орналасуын пайдаланады.

## қадамдар

1. **Конфигурацияны құрастыру**
   - `governance/compliance/soranet_opt_outs.json` импорттау.
   - `operator_jurisdictions` нұсқасын жарияланған аттестаттау дайджесттерімен біріктіріңіз
     [юрисдикциялық шолуда](gar-jurisdictional-review).
2. **Тексеру**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - Қосымша: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **Дәлелдерді басып алу**
   - `artifacts/soranet/compliance/<YYYYMMDD>/` астында сақтау:
     - `config.json` (соңғы сәйкестік блогы)
     - `attestations.json` (URI + дайджесттер)
     - тексеру журналдары
     - қол қойылған PDF файлдарына/Norito конверттеріне сілтемелер
4. **Іске қосу**
   - Шығарманы белгілеңіз (`gar-opt-out-<date>`), оркестр/SDK конфигурацияларын қайта орналастырыңыз,
     және күтілетін жерде журналдарда шығарылатын `compliance_*` оқиғаларын растаңыз.
5. **Жабу**
   - Басқару кеңесіне дәлелдемелерді тапсырыңыз.
   - GAR журналында белсендіру терезесін + бекітушілерді тіркеңіз.
   - Юрисдикциялық тексеру кестесінен келесі тексеру күндерін жоспарлаңыз.

## Жылдам бақылау тізімі

- [ ] `jurisdiction_opt_outs` канондық каталогқа сәйкес келеді.
- [ ] Аттестаттау дайджесттері дәл көшірілген.
- [ ] Тексеру пәрмендері іске қосылады және мұрағатталды.
- [ ] `artifacts/soranet/compliance/<date>/` ішінде сақталған дәлелдер жинағы.
- [ ] Шығарылым тегі + GAR журналы жаңартылды.
- [ ] Келесі шолу туралы еске салғыштар орнатылды.

## Сондай-ақ қараңыз

- [GAR юрисдикциялық шолуы](gar-jurisdictional-review)
- [GAR Compliance Playbook (көзі)](../../../source/soranet/gar_compliance_playbook.md)