---
lang: kk
direction: ltr
source: docs/source/ministry/red_team_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4a2e50e18749f64212dee5186bfd3a3d034ac906d1a506d420cdcb1b5117517
source_last_modified: "2025-12-29T18:16:35.979926+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team Status (MINFO-9)
summary: Snapshot of the chaos drill program covering upcoming runs, last completed scenario, and remediation items.
translator: machine-google-reviewed
---

# Министрліктің қызыл командасы мәртебесі

Бұл бет [Moderation Red-Team Plan](moderation_red_team_plan.md) толықтырады.
жақын мерзімді бұрғылау күнтізбесін, дәлелдемелерді жинақтауды және түзетуді қадағалау арқылы
күй. Оны түсірілген артефактілермен бірге әрбір жүгіруден кейін жаңартыңыз
`artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`.

## Алдағы жаттығулар

| Күні (UTC) | Сценарий | Ие(лер) | Дәлелдерді дайындау | Ескертпелер |
|------------|---------|----------|---------------|-------|
| 12.11.2026 | **Көзді байлау операциясы** — шлюз деңгейін төмендету әрекеттерімен Taikai аралас режимдегі контрабанда репетициясы | Қауіпсіздік инженериясы (Мию Сато), Министрлік Опс (Лиам О’Коннор) | `scripts/ministry/scaffold_red_team_drill.py` жинағы `docs/source/ministry/reports/red_team/2026-11-operation-blindfold.md` + кезеңдік каталог `artifacts/ministry/red-team/2026-11/operation-blindfold/` | Жаттығулар GAR/Taikai қабаттасуы және DNS ауыстырылуы; іске қосу алдында бас тарту тізімін Merkle суретін және бақылау тақталары түсірілгеннен кейін `export_red_team_evidence.py` іске қосуды қажет етеді. |

## Соңғы бұрғылау суреті

| Күні (UTC) | Сценарий | Дәлелдер жинағы | Түзету және бақылау |
|------------|---------|-----------------|--------------------------|
| 18.08.2026 | **Операция SeaGlass** — Шлюз контрабандасы, басқаруды қайталау және ескерту репетициясы | `artifacts/ministry/red-team/2026-08/operation-seaglass/` (Grafana экспорттары, Alertmanager журналдары, `seaglass_evidence_manifest.json`) | **Ашық:** пломбаны қайта ойнатуды автоматтандыру (`MINFO-RT-17`, иесі: Governance Ops, мерзімі 2026-09-05); бақылау тақтасы SoraFS (`MINFO-RT-18`, Бақылау мүмкіндігі, 2026-08-25 дейін) дейін қатып қалады. **Жабық:** журнал үлгісі Norito манифест хэштерін тасымалдау үшін жаңартылды. |

## Бақылау және құралдар

- Инъекцияға арналған қаптама үшін `scripts/ministry/moderation_payload_tool.py` пайдаланыңыз
  әр сценарий бойынша пайдалы жүктемелер мен жоққа шығару патчтары.
- `scripts/ministry/export_red_team_evidence.py` арқылы бақылау тақтасын/журнал түсірулерін жазыңыз
  Әрбір жаттығудан кейін бірден дәлел манифестінде қол қойылған хэштер болады.
- CI күзетшісі `ci/check_ministry_red_team.sh` жаттығу есептерін орындауды қамтамасыз етеді
  толтырғыш мәтіні жоқ және сілтеме жасалған артефактілер бұрын бар
  біріктіру.

Анықтамаға тікелей сілтеме жасау үшін `status.md` (§ *Министрліктің қызыл командасының мәртебесі*) қараңыз.
апта сайынғы үйлестіру шақыруларында.