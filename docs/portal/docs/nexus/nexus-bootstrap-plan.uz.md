---
lang: uz
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa25c267f36e3245866776d5149039e1b9833407a84126d66a21cf5296e51414
source_last_modified: "2025-12-29T18:16:35.135788+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-bootstrap-plan
title: Sora Nexus bootstrap & observability
description: Operational plan for bringing the core Nexus validator cluster online before layering SoraFS and SoraNet services.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
Bu sahifa `docs/source/soranexus_bootstrap_plan.md`ni aks ettiradi. Mahalliylashtirilgan versiyalar portalga tushmaguncha ikkala nusxani ham bir xilda saqlang.
:::

# Sora Nexus Bootstrap & Observability Plan

## Maqsadlar
- Boshqaruv kalitlari, Torii API va konsensus monitoringi bilan Sora Nexus validator/kuzatuvchi tarmog'ini tiklang.
- SoraFS/SoraNet o'rnatishni yoqishdan oldin asosiy xizmatlarni (Torii, konsensus, qat'iylik) tasdiqlang.
- Tarmoq sog'lig'ini ta'minlash uchun CI/CD ish oqimlari va kuzatuv panellari/ogohlantirishlarini o'rnating.

## Old shartlar
- Boshqaruv uchun asosiy materiallar (kengash multisig, qo'mita kalitlari) HSM yoki Vault-da mavjud.
- Birlamchi/ikkinchi darajali hududlarda bazaviy infratuzilma (Kubernetes klasterlari yoki yalang'och metall tugunlari).
- So'nggi konsensus parametrlarini aks ettiruvchi yangilangan yuklash konfiguratsiyasi (`configs/nexus/bootstrap/*.toml`).

## Tarmoq muhitlari
- Ikkita Nexus muhitini alohida tarmoq prefikslari bilan boshqaring:
- **Sora Nexus (mainnet)** – kanonik boshqaruv va SoraFS/SoraNet piggyback xizmatlarini o'z ichiga olgan `nexus` ishlab chiqarish tarmog'i prefiksi (zanjir ID `0x02F1` / UUU028X / UUU02).
- **Sora Testus (testnet)** – bosqichli tarmoq prefiksi `testus`, integratsiya sinovi va relizdan oldin tekshirish uchun asosiy tarmoq konfiguratsiyasini aks ettiradi (zanjir UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Har bir muhit uchun alohida genezis fayllari, boshqaruv kalitlari va infratuzilma izlarini saqlang. Testus Nexus ga ko'tarilishdan oldin barcha SoraFS/SoraNet prokatlari uchun sinov asosi bo'lib xizmat qiladi.
- CI/CD quvurlari avval Testus-ga joylashtirilishi, avtomatlashtirilgan tutun sinovlarini o'tkazishi va tekshiruvdan o'tgandan keyin Nexus ga qo'lda ko'tarilishi kerak.
- Yo'naltiruvchi konfiguratsiya to'plamlari `configs/soranexus/nexus/` (asosiy tarmoq) va `configs/soranexus/testus/` (testnet) ostida ishlaydi, ularning har birida `config.toml`, `genesis.json` va Torii namunalari mavjud.

## 1-qadam - Konfiguratsiyani ko'rib chiqish
1. Mavjud hujjatlarni tekshirish:
   - `docs/source/nexus/architecture.md` (konsensus, Torii tartibi).
   - `docs/source/nexus/deployment_checklist.md` (infra talablar).
   - `docs/source/nexus/governance_keys.md` (asosiy saqlash tartib-qoidalari).
2. Tasdiqlash genesis fayllari (`configs/nexus/genesis/*.json`) joriy validator ro'yxati va staking og'irliklari bilan mos keladi.
3. Tarmoq parametrlarini tasdiqlang:
   - Konsensus qo'mitasining soni va kvorum.
   - Blok oralig'i / yakuniylik chegaralari.
   - Torii xizmat ko'rsatish portlari va TLS sertifikatlari.

## 2-qadam – Bootstrap Cluster Deployment
1. Ta'minlash validator tugunlari:
   - Doimiy hajmli `irohad` nusxalarini (validatorlar) joylashtiring.
   - Tarmoq xavfsizlik devori qoidalari konsensusga va tugunlar o'rtasida Torii trafikiga ruxsat berishiga ishonch hosil qiling.
2. TLS bilan har bir validatorda Torii xizmatlarini (REST/WebSocket) ishga tushiring.
3. Qo'shimcha chidamlilik uchun kuzatuvchi tugunlarini (faqat o'qish uchun) joylashtiring.
4. Genezisni tarqatish, konsensusni boshlash va tugunlarni ro'yxatga olish uchun yuklash skriptlarini (`scripts/nexus_bootstrap.sh`) ishga tushiring.
5. Tutun sinovlarini o'tkazing:
   - Torii (`iroha_cli tx submit`) orqali test tranzaktsiyalarini yuboring.
   - Telemetriya orqali blok ishlab chiqarish/yakuniyligini tekshiring.
   - Validatorlar/kuzatuvchilar bo'yicha daftar nusxasini tekshiring.

## 3-qadam – Boshqaruv va kalitlarni boshqarish
1. Kengash multisig konfiguratsiyasini yuklang; boshqaruv takliflari kiritilishi va ratifikatsiya qilinishi mumkinligini tasdiqlaydi.
2. Konsensus/qo‘mita kalitlarini xavfsiz saqlash; kirish jurnali bilan avtomatik zaxira nusxalarini sozlash.
3. Favqulodda kalitlarni aylantirish tartiblarini o'rnating (`docs/source/nexus/key_rotation.md`) va runbookni tekshiring.

## 4-qadam – CI/CD integratsiyasi
1. Quvurlarni sozlash:
   - Validator/Torii tasvirlarini yaratish va nashr etish (GitHub Actions yoki GitLab CI).
   - Avtomatlashtirilgan konfiguratsiyani tekshirish (lint genesis, imzolarni tekshirish).
   - O'rnatish va ishlab chiqarish klasterlari uchun joylashtirish quvurlari (Helm/Kustomize).
2. CIda tutun sinovlarini amalga oshirish (efemer klasterni aylantirish, kanonik tranzaksiya to'plamini ishga tushirish).
3. Muvaffaqiyatsiz o'rnatish va hujjatni ishga tushirish kitoblari uchun orqaga qaytarish skriptlarini qo'shing.

## 5-qadam – Kuzatish va ogohlantirishlar
1. Har bir mintaqada monitoring stekini (Prometheus + Grafana + Alertmanager) joylashtiring.
2. Asosiy ko'rsatkichlarni to'plang:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Torii va konsensus xizmatlari uchun Loki/ELK orqali jurnallar.
3. Boshqaruv panellari:
   - Konsensus salomatligi (blok balandligi, yakuniyligi, tengdoshlar maqomi).
   - Torii API kechikish/xato stavkalari.
   - Boshqaruv operatsiyalari va taklif holati.
4. Ogohlantirishlar:
   - Blok ishlab chiqarish stend (>2 blok oralig'i).
   - Tengdoshlar soni kvorumdan past.
   - Torii xatolik darajasi keskin oshadi.
   - Boshqaruv bo'yicha takliflar navbatining orqada qolishi.

## 6-qadam – Tasdiqlash va topshirish
1. Oxir-oqibat tekshirishni bajaring:
   - Boshqaruv taklifini taqdim etish (masalan, parametrlarni o'zgartirish).
   - Boshqaruv quvurlari ishlarini ta'minlash uchun uni kengash ma'qullashi orqali qayta ishlash.
   - Muvofiqlikni ta'minlash uchun ledger state diff-ni ishga tushiring.
2. Qo'ng'iroq bo'yicha ishlash kitobi (hodisalar javobi, o'zgarish, masshtablash).
3. SoraFS/SoraNet jamoalariga tayyorligi haqida xabar bering; piggyback joylashtirishlarini tasdiqlash Nexus tugunlariga ishora qilishi mumkin.

## Amalga oshirishni tekshirish ro'yxati
- [ ] Ibtido/konfiguratsiya tekshiruvi tugallandi.
- [ ] Tasdiqlovchi va kuzatuvchi tugunlari sog'lom konsensus bilan joylashtirilgan.
- [ ] Boshqaruv kalitlari yuklandi, taklif sinovdan oʻtkazildi.
- [ ] CI/CD quvurlari ishlayapti (qurilish + joylashtirish + tutun sinovlari).
- [ ] Kuzatuv panellari ogohlantirish bilan ishlaydi.
- [ ] Pastki oqim guruhlariga topshirilgan hujjatlar.