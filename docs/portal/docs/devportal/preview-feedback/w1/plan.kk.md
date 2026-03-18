---
lang: kk
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c3eddff3a7b9b5dc4eac39251c9df72d0533a6e1b5865c716d54dc3b1c5de164
source_last_modified: "2025-12-29T18:16:35.108030+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w1-plan
title: W1 partner preflight plan
sidebar_label: W1 plan
description: Tasks, owners, and evidence checklist for the partner preview cohort.
translator: machine-google-reviewed
---

| Элемент | Мәліметтер |
| --- | --- |
| Толқын | W1 — Серіктестер және Torii интеграторлары |
| Мақсатты терезе | 2025 2 тоқсан  3 апта |
| Артефакт тегі (жоспарланған) | `preview-2025-04-12` |
| Трекер мәселесі | `DOCS-SORA-Preview-W1` |

## Мақсаттар

1. Серіктестің алдын ала қарау шарттары үшін қауіпсіз заңды + басқару мақұлдаулары.
2. Шақыру жинағында пайдаланылатын проксиді және телеметриялық суретті көріңіз.
3. Бақылау сомасы тексерілген алдын ала қарау артефакті мен зерттеу нәтижелерін жаңартыңыз.
4. Шақырулар жіберілмес бұрын серіктестер тізімін + сұрау үлгілерін аяқтаңыз.

## Тапсырмаларды бөлу

| ID | Тапсырма | Иесі | Мерзімі | Күй | Ескертпелер |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Алдын ала қарау шарттарының қосымшасы үшін заңды мақұлдауды алыңыз | Docs/DevRel жетекші → Құқықтық | 2025-04-05 | ✅ Аяқталды | `DOCS-SORA-Preview-W1-Legal` заңды билеті 2025‑04‑05 қол қойылған; PDF трекерге тіркелген. |
| W1-P2 | Түсіру Проксиді орнату терезесін (2025‑04‑10) көріңіз және прокси күйін тексеріңіз | Docs/DevRel + Ops | 2025-04-06 | ✅ Аяқталды | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` орындалды 2025‑04‑06; CLI транскрипті + `.env.tryit-proxy.bak` мұрағатталған. |
| W1-P3 | Алдын ала қарау артефакті (`preview-2025-04-12`), `scripts/preview_verify.sh` + `npm run probe:portal` іске қосыңыз, мұрағат дескрипторы/бақылау қосындылары | Портал TL | 2025-04-08 | ✅ Аяқталды | Artefact + `artifacts/docs_preview/W1/preview-2025-04-12/` астында сақталған тексеру журналдары; зонд шығысы трекерге бекітілген. |
| W1-P4 | Серіктес қабылдау үлгілерін (`DOCS-SORA-Preview-REQ-P01…P08`) қарап шығыңыз, контактілерді растаңыз + NDA | Басқару байланысы | 2025-04-07 | ✅ Аяқталды | Барлық сегіз сұрау мақұлданды (соңғы екеуі 2025‑04‑11 тазартылды); трекерде байланыстырылған мақұлдаулар. |
| W1-P5 | Шақыру көшірмесінің жобасы (`docs/examples/docs_preview_invite_template.md` негізінде), әрбір серіктес үшін `<preview_tag>` және `<request_ticket>` жиынтығы | Docs/DevRel жетекші | 2025-04-08 | ✅ Аяқталды | Шақыру жобасы 2025‑04‑12 15:00UTC артефакт сілтемелерімен бірге жіберілді. |

## Ұшу алдындағы бақылау тізімі

> Кеңес: 1-5-қадамдарды автоматты түрде орындау үшін `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` іске қосыңыз (құрастыру, бақылау сомасын тексеру, порталды тексеру, сілтеме тексерушісі және прокси жаңартуын көру). Сценарий трекер мәселесіне тіркеуге болатын JSON журналын жазады.

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` бар) `build/checksums.sha256` және `build/release.json` қалпына келтіру үшін.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` және дескриптордың жанындағы мұрағат `build/link-report.json`.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (немесе `--tryit-target` арқылы сәйкес мақсатты қамтамасыз етіңіз); жаңартылған `.env.tryit-proxy` нұсқасын орындаңыз және `.bak` қайтару үшін сақтаңыз.
6. W1 трекер мәселесін журнал жолдарымен жаңартыңыз (дескриптордың бақылау сомасы, зерттеу шығысы, проксиді өзгертуге тырысыңыз, Grafana суреті).

## Дәлелдерді тексеру парағы

- [x] `DOCS-SORA-Preview-W1` құжатына тіркелген қол қойылған заңды мақұлдау (PDF немесе билет сілтемесі).
- [x] `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` үшін Grafana скриншоттары.
- [x] `preview-2025-04-12` дескриптор + `artifacts/docs_preview/W1/` астында сақталған бақылау сомасы журналы.
- [x] `invite_sent_at` уақыт белгілері толтырылған шақыру тізімі кестесі (W1 трекер журналын қараңыз).
- [x] Кері байланыс артефактілері [`preview-feedback/w1/log.md`](./log.md) әр серіктеске бір жолмен бейнеленген (тізім/телеметрия/шығарылым деректерімен 2025-04-26 жаңартылды).

Тапсырмалар орындалған сайын осы жоспарды жаңартыңыз; трекер жол картасын сақтау үшін оған сілтеме жасайды
тексерілетін.

## Кері байланыс жұмыс процесі

1. Әрбір шолушы үшін үлгінің көшірмесін жасаңыз
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   метадеректерді толтырыңыз және аяқталған көшірмені астында сақтаңыз
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Шақыруларды, телеметриялық бақылау нүктелерін және тікелей журналдағы ашық мәселелерді қорытындылаңыз
   [`preview-feedback/w1/log.md`](./log.md) сондықтан басқару шолушылары бүкіл толқынды қайталай алады
   репозиторийден шықпай-ақ.
3. Білімді тексеру немесе зерттеу экспорты келгенде, оларды журналда көрсетілген артефакт жолына тіркеңіз.
   және трекер мәселесін байланыстырыңыз.