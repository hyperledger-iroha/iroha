---
lang: kk
direction: ltr
source: docs/source/ministry/reports/2026-08-mod-red-team-operation-seaglass.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 64cd1112df2f1fc95571ee4ed269e64bde6bf73bd94b19bbf0eaa80a5b43c219
source_last_modified: "2025-12-29T18:16:35.981105+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill — Operation SeaGlass
summary: Evidence and remediation log for the Operation SeaGlass moderation drill (gateway smuggling, governance replay, alert brownout).
translator: machine-google-reviewed
---

# Red-Team Drill — SeaGlass операциясы

- **Бұрғылау идентификаторы:** `20260818-operation-seaglass`
- **Күн және терезе:** `2026-08-18 09:00Z – 11:00Z`
- **Сценарий класы:** `smuggling`
- **Операторлар:** `Miyu Sato, Liam O'Connor`
- **Бақылау тақталары тапсырмадан тоқтатылды:** `364f9573b`
- **Дәлелдер жинағы:** `artifacts/ministry/red-team/2026-08/operation-seaglass/`
- **SoraFS CID (міндетті емес):** `not pinned (local bundle only)`
- ** Қатысты жол картасы элементтері:** `MINFO-9`, сонымен қатар `MINFO-RT-17` / `MINFO-RT-18` байланыстырылған бақылаулар.

## 1. Мақсаттар мен кіру шарттары

- **Негізгі мақсаттар**
  - Жүк түсіру туралы ескертулер кезінде контрабанда әрекеті кезінде бас тарту тізімінің TTL орындалуын және шлюз карантинін растаңыз.
  - Басқаруды қайта ойнатуды анықтауды және модерацияның жұмыс кітабындағы ескертулерді өңдеуді растаңыз.
- **Алғышарттар расталды**
  - `emergency_canon_policy.md` `v2026-08-seaglass` нұсқасы.
  - `dashboards/grafana/ministry_moderation_overview.json` дайджест `sha256:ef5210b5b08d219242119ec4ceb61cb68ee4e42ce2eea8a67991fbff95501cc8`.
  - Қоңырау кезінде қайта анықтау өкілеттігі: `Kenji Ito (GovOps pager)`.

## 2. Орындау хронологиясы

| Уақыт белгісі (UTC) | Актер | Әрекет / Команда | Нәтиже / Ескертулер |
|----------------|-------|------------------|----------------|
| 09:00:12 | Мию Сато | `364f9573b` арқылы `scripts/ministry/export_red_team_evidence.py --freeze-only` арқылы бекітілген бақылау тақталары/ескертулер | Негізгі сызық `dashboards/` | астында түсіріліп, сақталады
| 09:07:44 | Лиам О'Коннор | Жарияланған бас тарту тізімінің суреті + `sorafs_cli ... gateway update-denylist --policy-tier emergency` көмегімен сахналау үшін GAR қайта анықтау | Сурет қабылданды; Alertmanager | ішінде жазылған қайта анықтау терезесі
| 09:17:03 | Мию Сато | Инъекциялық контрабанда пайдалылығы + `moderation_payload_tool.py --scenario seaglass` арқылы басқаруды қайталау | Дабыл 3м12с кейін берілді; басқаруды қайталау жалаушамен белгіленген |
| 09:31:47 | Лиам О'Коннор | Дәлелдерді экспорттау және мөрленген манифест `seaglass_evidence_manifest.json` | `manifests/` | астында сақталған дәлелдер жинағы және хэштер

## 3. Бақылаулар және метрика

| метрикалық | Мақсат | Байқалған | Өтті/өтпеген | Ескертпелер |
|--------|--------|----------|-----------|-------|
| Ескертуге жауап беру кідірісі | = 0,98 | 0,992 | ✅ | Контрабанданы да, пайдалы жүктерді қайталауды да анықтады |
| Шлюз аномалиясын анықтау | Ескерту жіберілді | Ескерту іске қосылды + автоматты карантин | ✅ | Карантин қайталап көру бюджеті таусылғанға дейін қолданылды

- `Grafana export:` `artifacts/ministry/red-team/2026-08/operation-seaglass/dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/ministry/red-team/2026-08/operation-seaglass/alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `artifacts/ministry/red-team/2026-08/operation-seaglass/manifests/seaglass_evidence_manifest.json`

## 4. Нәтижелер және түзетулер

| Ауырлық | табу | Иесі | Мақсатты күн | Күй / Сілтеме |
|----------|---------|-------|-------------|---------------|
| Жоғары | Басқаруды қайталау ескертуі іске қосылды, бірақ күту тізімінің ауыстырылуы іске қосылғанда SoraFS мөрі 2 м кешіктірілді | Governance Ops (Лиам О'Коннор) | 09.05.2026 | `MINFO-RT-17` ашық — қайта ойнату пломбасын автоматтандыруды ауыстыру жолына қосыңыз |
| Орташа | Бақылау тақтасының қатуы SoraFS үшін бекітілмеген; операторлар жергілікті бумаға | Бақылау мүмкіндігі (Мию Сато) | 25.08.2026 | `MINFO-RT-18` ашық — келесі жаттығу алдында қол қойылған CID бар `dashboards/*` - SoraFS істікшелері |
| Төмен | CLI журналы бірінші өтуде Norito манифест хэшін өткізіп жіберді | Министрлік операциясы (Кендзи Ито) | 22.08.2026 | Бұрғылау кезінде бекітіледі; үлгі журналда жаңартылды |Калибрлеудің қалай көрінетінін құжаттау, бас тарту саясаты немесе SDK/құрал өзгерту керек. GitHub/Jira мәселелеріне сілтеме және блокталған/бұғатталмаған күйлерді ескертіңіз.

## 5. Басқару және бекітулер

- **Оқиға командирінің қолтаңбасы:** `Miyu Sato @ 2026-08-18T11:22Z`
- **Басқару кеңесінің қарау күні:** `GovOps-2026-08-22`
- **Кейінгі тексеру парағы:** `[x] status.md updated`, `[x] roadmap row updated`, `[x] transparency packet annotated`

## 6. Қосымшалар

- `[x] CLI logbook (logs/operation_seaglass.log)`
- `[x] Dashboard JSON export`
- `[x] Alertmanager history`
- `[x] SoraFS manifest / CAR`
- `[ ] Override audit log`

Дәлелдер жинағына және SoraFS суретіне жүктеп салынғаннан кейін `[x]` әрбір тіркемені белгілеңіз.

---

_Соңғы жаңартылған күні: 2026-08-18