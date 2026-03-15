---
id: multi-source-rollout
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Multi-Source Client Rollout & Blacklisting Runbook
sidebar_label: Multi-Source Rollout Runbook
description: Operational checklist for staged multi-source rollouts and emergency provider blacklisting.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

## Maqsad

Ushbu runbook SRE va qo'ng'iroq bo'yicha muhandislarga ikkita muhim ish oqimi orqali yo'l ko'rsatadi:

1. Ko'p manbali orkestrni boshqariladigan to'lqinlarda yoyish.
2. Mavjud seanslarni beqarorlashtirmasdan, noto'g'ri ishlaydigan provayderlarni qora ro'yxatga kiritish yoki ustuvorlikdan chiqarish.

Bu SF-6 ostida yetkazib berilgan orkestratsiya stegi allaqachon o'rnatilgan deb taxmin qiladi (`sorafs_orchestrator`, shlyuz chunk-diapazoni API, telemetriya eksportchilari).

> **Shuningdek qarang:** [Orchestrator Operations Runbook](./orchestrator-ops.md) har bir ishga tushirish tartib-qoidalariga kiradi (ko‘rsatkichlar jadvalini suratga olish, bosqichma-bosqich o‘chirish, orqaga qaytarish). Jonli o'zgarishlar paytida ikkala havoladan birgalikda foydalaning.

## 1. Parvoz oldidan tekshirish

1. **Boshqaruv maʼlumotlarini tasdiqlang.**
   - Barcha nomzod provayderlar `ProviderAdvertV1` konvertlarini diapazonning foydali yuklari va oqim byudjetlari bilan nashr etishlari kerak. `/v1/sorafs/providers` orqali tasdiqlang va kutilgan imkoniyatlar maydonlari bilan solishtiring.
   - Kechikish/muvaffaqiyatsizlik tezligini ta'minlovchi telemetriya suratlari har bir kanareyka yugurishidan oldin < 15 daqiqa eski bo'lishi kerak.
2. **Bosqich konfiguratsiyasi.**
   - Qatlamli `iroha_config` daraxtida orkestr JSON konfiguratsiyasini davom ettiring:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     JSON-ni tarqatish uchun maxsus cheklovlar bilan yangilang (`max_providers`, qayta urinib ko'ring byudjetlar). Xuddi shu faylni sahnalashtirish/ishlab chiqarishga yuboring, shunda farqlar kichik bo'lib qoladi.
3. **Kononik moslamalarni mashq qiling.**
   - Manifest/token muhiti o'zgaruvchilarini to'ldiring va deterministik olishni ishga tushiring:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     Atrof-muhit o'zgaruvchilari kanareykada ishtirok etuvchi har bir provayder uchun manifest foydali yuk dayjestini (hex) va base64-kodlangan oqim tokenlarini o'z ichiga olishi kerak.
   - Oldingi versiyaga nisbatan `artifacts/canary.scoreboard.json` farqi. Har qanday yangi nomaqbul provayder yoki vazn o'zgarishi >10% ko'rib chiqishni talab qiladi.
4. **Telemetriya simli ekanligini tekshiring.**
   - `docs/examples/sorafs_fetch_dashboard.json` da Grafana eksportini oching. Davom etishdan oldin `sorafs_orchestrator_*` koʻrsatkichlari bosqichma-bosqich toʻldirilganligiga ishonch hosil qiling.

## 2. Favqulodda provayderning qora ro'yxati

Provayder buzilgan bo'laklarga xizmat ko'rsatsa, vaqtni doimiy ravishda tugatsa yoki muvofiqlikni tekshirishda muvaffaqiyatsizlikka uchrasa, ushbu tartibni bajaring.

1. **Dalillarni qo‘lga olish.**
   - Oxirgi qabul qilish xulosasini eksport qiling (`--json-out` chiqishi). Muvaffaqiyatsiz bo'lak indekslarini, provayder taxalluslarini va hazm qilish mos kelmasligini yozib oling.
   - `telemetry::sorafs.fetch.*` maqsadlaridan tegishli jurnal parchalarini saqlang.
2. **Darhol bekor qilishni qo'llang.**
   - Orkestrga tarqatilgan telemetriya snapshotida provayderni jazolangan deb belgilang (`penalty=true` yoki qisqich `token_health` ni `0` ga o'rnating). Keyingi reyting jadvali provayderni avtomatik ravishda chiqarib tashlaydi.
   - Ad-hoc tutun sinovlari uchun `--deny-provider gw-alpha` ni `sorafs_cli fetch` ga o'tkazing, shunda muvaffaqiyatsizlik yo'li telemetriya tarqalishini kutmasdan amalga oshiriladi.
   - Yangilangan telemetriya/konfiguratsiya to'plamini ta'sirlangan muhitga qayta joylashtiring (staging → kanareyka → ishlab chiqarish). Hodisalar jurnalida o'zgarishlarni hujjatlashtiring.
3. **Bekor qilishni tasdiqlang.**
   - Kanonik moslamani qayta ishga tushiring. Tabloda provayderni `policy_denied` sabab bilan nomaqbul deb belgilashini tasdiqlang.
   - Hisoblagich rad etilgan provayder uchun o'sishni to'xtatganiga ishonch hosil qilish uchun `sorafs_orchestrator_provider_failures_total` ni tekshiring.
4. **Uzoq muddatli taqiqlarni kuchaytiring.**
   - Agar provayder >24 soat davomida bloklangan bo'lsa, reklamani aylantirish yoki to'xtatib turish uchun boshqaruv chiptasini ko'taring. Ovoz berish tugaguniga qadar rad etishlar ro‘yxatini joyida saqlang va provayder reyting jadvaliga qayta kirmasligi uchun telemetriya suratlarini yangilang.
5. **Orqaga qaytarish protokoli.**
   - Provayderni qayta tiklash uchun uni rad etish roʻyxatidan olib tashlang, qayta joylashtiring va yangi jadval suratini oling. O'zgarishlarni o'limdan keyingi voqeaga qo'shing.

## 3. Bosqichli ishlab chiqarish rejasi

| Bosqich | Qo'llash doirasi | Kerakli signallar | Go/No-Go mezonlari |
|-------|-------|------------------|-------------------|
| **Laboratoriya** | Maxsus integratsiya klasteri | Armatura yuklamalariga qarshi qo'lda CLI olish | Barcha qismlar muvaffaqiyatli bo'ldi, provayderning xatolik hisoblagichlari 0 da qoladi, qayta urinish darajasi < 5%. |
| **Sahnalashtirish** | To'liq nazorat-samolyot staging | Grafana asboblar paneli ulangan; faqat ogohlantirish rejimida ogohlantirish qoidalari | `sorafs_orchestrator_active_fetches` har bir sinovdan keyin nolga qaytadi; `warn/critical` ogohlantirish otishmalari yo'q. |
| **Kanarya** | Ishlab chiqarish trafigining ≤10% | Peyjer ovozi o'chirilgan, ammo telemetriya real vaqtda kuzatilmoqda | Qayta urinish nisbati < 10%, provayderning nosozliklari maʼlum shovqinli tengdoshlar bilan ajratilgan, kechikish gistogrammasi bosqichma-bosqich ±20% bilan mos keladi. |
| **Umumiy mavjudlik** | 100% ishlab chiqarish | Peyjer qoidalari faol | 24 soat davomida nol `NoHealthyProviders` xatolar, qayta urinish nisbati barqaror, asboblar panelidagi SLA panellari yashil. |

Har bir bosqich uchun:

1. Orkestr JSON-ni mo'ljallangan `max_providers` bilan yangilang va qayta urinib ko'ring.
2. `sorafs_cli fetch` yoki SDK integratsiya test to'plamini kanonik moslamaga va atrof-muhitning vakili manifestiga qarshi ishga tushiring.
3. Tablo + sarhisob artefaktlarini yozib oling va ularni reliz yozuviga biriktiring.
4. Keyingi bosqichga o'tishdan oldin qo'ng'iroq bo'yicha muhandis bilan telemetriya asboblar panelini ko'rib chiqing.

## 4. Kuzatish va hodisa ilgaklari

- **Ko'rsatkichlar:** Alertmanager monitorlari `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` va `sorafs_orchestrator_retries_total` mavjudligiga ishonch hosil qiling. To'satdan ko'tarilish odatda provayder yuk ostida yomonlashayotganini anglatadi.
- **Jurnallar:** `telemetry::sorafs.fetch.*` maqsadlarini umumiy jurnal agregatoriga yo'naltiring. Triajni tezlashtirish uchun `event=complete status=failed` uchun saqlangan qidiruvlarni yarating.
- **Ko'rsatkichlar jadvali:** Har bir tablo artefaktini uzoq muddatli saqlash uchun saqlang. JSON muvofiqlikni tekshirish va bosqichma-bosqich orqaga qaytarish uchun dalil izi sifatida ikki baravar ishlaydi.
- **Boshqaruv panellari:** Grafana kanonik taxtasini (`docs/examples/sorafs_fetch_dashboard.json`) `docs/examples/sorafs_fetch_alerts.yaml` ogohlantirish qoidalari bilan ishlab chiqarish papkasiga klonlang.

## 5. Aloqa va hujjatlar

- Vaqt tamg'asi, operator, sabab va tegishli hodisa bilan operatsiyalarni o'zgartirish jurnalidagi har bir rad etish/ko'paytirish o'zgarishlarini qayd qiling.
- Mijoz kutganlarini moslashtirish uchun provayder vazni yoki qayta urinib ko'ring.
- GA tugallangandan so'ng, `status.md` ni ishlab chiqarish xulosasi bilan yangilang va ushbu runbook ma'lumotnomasini nashr yozuvlarida arxivlang.