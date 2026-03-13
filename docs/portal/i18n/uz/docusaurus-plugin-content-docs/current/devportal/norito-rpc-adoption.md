---
id: norito-rpc-adoption
lang: uz
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito-RPC Adoption Schedule
sidebar_label: Norito-RPC adoption
description: Cross-SDK rollout plan, evidence checklist, and automation hooks for roadmap item NRPC-4.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Kanonik rejalashtirish qaydlari `docs/source/torii/norito_rpc_adoption_schedule.md` da yashaydi.  
> Ushbu portal nusxasi SDK mualliflari, operatorlari va ko'rib chiquvchilari uchun taqdimotni kutadi.

## Maqsadlar

- Ikkilik Norito-RPC transportida har bir SDK (Rust CLI, Python, JavaScript, Swift, Android) ni AND4 ishlab chiqarish o'tish tugmasidan oldin tekislang.
- Fazali eshiklar, dalillar to'plamlari va telemetriya ilgaklarini deterministik saqlang, shunda boshqaruv provayderni tekshirishi mumkin.
- NRPC-4 yo'l xaritasi chaqiradigan umumiy yordamchilar yordamida armatura va kanareyka dalillarini qo'lga kiritishni ahamiyatsiz qiling.

## Faza xronologiyasi

| Bosqich | Oyna | Qo'llash doirasi | Chiqish mezonlari |
|-------|--------|-------|---------------|
| **P0 – Laboratoriya pariteti** | Q22025 | Rust CLI + Python tutun to'plamlari CIda `/v2/norito-rpc` ishlaydi, JS yordamchisi birlik testlaridan o'tadi, Android soxta jabduqlar ikki tomonlama transportni mashq qiladi. | CIda `python/iroha_python/scripts/run_norito_rpc_smoke.sh` va `javascript/iroha_js/test/noritoRpcClient.test.js` yashil; Android jabduqlari `./gradlew test` ga ulangan. |
| **P1 – SDK oldindan ko‘rish** | Q32025 | Umumiy jihozlar toʻplami tekshirildi, `scripts/run_norito_rpc_fixtures.sh --sdk <label>` jurnallar + JSON `artifacts/norito_rpc/` da, SDK namunalarida koʻrsatilgan ixtiyoriy Norito transport bayroqlari. | Fikstura manifesti imzolandi, README yangilanishlari obunadan foydalanishni koʻrsatadi, Swift oldindan koʻrish APIsi IOS2 bayrogʻi ortida mavjud. |
| **P2 – Sahnalashtirish / AND4 oldindan ko‘rish** | Q12026 | Staging Torii hovuzlari Norito, Android AND4 oldindan koʻrish mijozlari va Swift IOS2 paritet toʻplamlarini ikkilik transport uchun sukut boʻyicha afzal koʻradi, `dashboards/grafana/torii_norito_rpc_observability.json` toʻldirilgan telemetriya asboblar paneli. | `docs/source/torii/norito_rpc_stage_reports.md` kanareykani suratga oladi, `scripts/telemetry/test_torii_norito_rpc_alerts.sh` o'tadi, Android soxta jabduqlar takrorlash muvaffaqiyat/xato holatlarini suratga oladi. |
| **P3 – Ishlab chiqarish GA** | Q42026 | Norito barcha SDKlar uchun standart transportga aylanadi; JSON qo'ng'irchoq bo'lib qolmoqda. Har bir teg bilan ish arxivi paritet artefaktlarini chiqaring. | Rust/JS/Python/Swift/Android uchun Norito tutun chiqishi nazorat roʻyxati paketlarini chiqaring; Norito va JSON xatolik darajasidagi SLOlar uchun ogohlantirish chegaralari; `status.md` va nashr yozuvlari GA dalillarini keltiradi. |

## SDK yetkazib berish va CI ilgaklari

- **Rust CLI va integratsiya jabduqlar** – `iroha_cli pipeline` tutun sinovlarini `cargo xtask norito-rpc-verify` erga tushganidan keyin Norito tashishni majburlash uchun kengaytiring. `artifacts/norito_rpc/` ostida artefaktlarni saqlaydigan `cargo test -p integration_tests -- norito_streaming` (laboratoriya) va `cargo xtask norito-rpc-verify` (sahnalash/GA) bilan qo'riqlash.
- **Python SDK** – sukut bo‘yicha chiquvchi tutunni (`python/iroha_python/scripts/release_smoke.sh`) Norito RPC ga o‘rnating, `run_norito_rpc_smoke.sh` ni CI kirish nuqtasi sifatida saqlang va `python/iroha_python/README.md` da hujjat pariteti bilan ishlash. CI maqsadi: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **JavaScript SDK** – `NoritoRpcClient` ni barqarorlashtiring, boshqaruv/so‘rov yordamchilariga `toriiClientConfig.transport.preferred === "norito_rpc"` bo‘lganda Norito ga sukut bo‘lsin va `javascript/iroha_js/recipes/` da oxirigacha namunalarni oling. CI nashr qilishdan oldin `npm test` va dokerlashtirilgan `npm run test:norito-rpc` ishni bajarishi kerak; provenance yuklaydi Norito tutun jurnallari `javascript/iroha_js/artifacts/` ostida.
- **Swift SDK** – Norito ko‘prigini IOS2 bayrog‘i orqasiga o‘tkazing, armatura kadansini aks ettiring va Connect/Norito paritet to‘plami `docs/source/sdk/swift/index.md` da havola qilingan Buildkite yo‘laklarida ishlashiga ishonch hosil qiling.
- **Android SDK** – AND4 oldindan ko‘rish mijozlari va soxta Torii jabduqlari Norito ni qabul qiladi, `docs/source/sdk/android/networking.md` da hujjatlashtirilgan qayta urinish/orqaga qaytish telemetriyasi bilan. Jabduqlar `scripts/run_norito_rpc_fixtures.sh --sdk android` orqali boshqa SDKlar bilan jihozlarni baham ko'radi.

## Dalillar va avtomatlashtirish

- `scripts/run_norito_rpc_fixtures.sh` `cargo xtask norito-rpc-verify` ni oʻrab oladi, stdout/stderr ni oladi va `fixtures.<sdk>.summary.json` chiqaradi, shuning uchun SDK egalari `status.md` ga biriktirish uchun deterministik artefaktga ega. CI paketlarini tartibli saqlash uchun `--sdk <label>` va `--out artifacts/norito_rpc/<stamp>/` dan foydalaning.
- `cargo xtask norito-rpc-verify` sxema xesh paritetini (`fixtures/norito_rpc/schema_hashes.json`) qo'llaydi va agar Torii `X-Iroha-Error-Code: schema_mismatch` qaytarsa, bajarilmaydi. Nosozliklarni tuzatish uchun har bir nosozlikni JSON zaxira nusxasi bilan bog‘lang.
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` va `dashboards/grafana/torii_norito_rpc_observability.json` NRPC-2 uchun ogohlantirish shartnomalarini belgilaydi. Har bir boshqaruv paneli tahriridan so'ng skriptni ishga tushiring va `promtool` chiqishini kanareykalar to'plamida saqlang.
- `docs/source/runbooks/torii_norito_rpc_canary.md` bosqichma-bosqich va ishlab chiqarish mashqlarini tavsiflaydi; armatura xeshlari yoki ogohlantirish eshiklari o'zgarganda uni yangilang.

## Tekshiruvchi nazorat ro'yxati

NRPC-4 bosqichini belgilashdan oldin quyidagilarni tasdiqlang:

1. So'nggi armatura to'plami xeshlari `fixtures/norito_rpc/schema_hashes.json` va `artifacts/norito_rpc/<stamp>/` ostida qayd etilgan mos keladigan CI artefaktiga mos keladi.
2. SDK README / portal hujjatlari JSON-ni qayta tiklashni qanday majburlash kerakligini tasvirlaydi va Norito standart transportini keltiradi.
3. Telemetriya asboblar panelida ogohlantirish havolalari bilan ikki qavatli xatolik darajasi panellari ko'rsatilgan va Alertmanager quruq ishlashi (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) trekerga biriktirilgan.
4. Bu yerda qabul qilish jadvali kuzatuvchi yozuviga (`docs/source/torii/norito_rpc_tracker.md`) mos keladi va yo‘l xaritasi (NRPC-4) bir xil dalillar to‘plamiga havola qiladi.

Jadvalda intizomli bo'lish SDK o'rtasidagi xatti-harakatni oldindan aytish mumkin bo'ladi va boshqaruv auditi Norito-RPCni buyurtmasiz qabul qilish imkonini beradi.