---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 672a5e3a6f0c3e8999400bc6fa8c66cc3be1ba2119431c5fd26f6d9a436f767f
source_last_modified: "2025-12-29T18:16:35.187152+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Gateway & DNS Kickoff Runbook

Ushbu portal nusxasi kanonik ish kitobini aks ettiradi
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
U markazlashtirilmagan DNS va shlyuz uchun operatsion to'siqlarni ushlaydi
ish oqimi, shuning uchun tarmoq, operatsiyalar va hujjatlar yetakchilari mashq qilishlari mumkin
2025-03 boshlanishidan oldin avtomatlashtirish to'plami.

## Qamrov va yetkazib berish

- DNS (SF‑4) va shlyuz (SF‑5) bosqichlarini deterministikni takrorlash orqali bog'lang
  xostni chiqarish, rezolyutsiya katalogini chiqarish, TLS/GAR avtomatlashtirish va dalillar
  qo'lga olish.
- Kickoff ma'lumotlarini saqlang (kun tartibi, taklif, davomat kuzatuvchisi, GAR telemetriyasi
  oniy rasm) egasining so'nggi topshiriqlari bilan sinxronlangan.
- Boshqaruvni ko'rib chiquvchilar uchun tekshiriladigan artefaktlar to'plamini yarating: hal qiluvchi
  katalog reliz yozuvlari, shlyuz tekshiruvi jurnallari, muvofiqlik jabduqlar chiqishi va
  Docs/DevRel xulosasi.

## Rol va mas'uliyat

| Ish oqimi | Mas'uliyat | Kerakli artefaktlar |
|------------|------------------|--------------------|
| Tarmoqli TL (DNS stek) | Deterministik xost rejasini saqlang, RAD katalog relizlarini ishga tushiring, hal qiluvchi telemetriya kiritishlarini nashr eting. | `artifacts/soradns_directory/<ts>/`, `docs/source/soradns/deterministic_hosts.md` uchun farqlar, RAD metamaʼlumotlari. |
| Operatsiyalarni avtomatlashtirish yetakchisi (shlyuz) | TLS/ECH/GAR avtomatlashtirish mashqlarini bajaring, `sorafs-gateway-probe` ni ishga tushiring, PagerDuty ilgaklarini yangilang. | `artifacts/sorafs_gateway_probe/<ts>/`, prob JSON, `ops/drill-log.md` yozuvlari. |
| QA gildiyasi va asboblar WG | `ci/check_sorafs_gateway_conformance.sh` ishga tushiring, moslamalarni tuzating, Norito o'z-o'zidan sertifikat to'plamlarini arxivlang. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Hujjatlar / DevRel | Daqiqalarni yozib oling, dizaynni oldindan o'qilgan + ilovalarni yangilang va dalillar xulosasini ushbu portalda nashr eting. | Yangilangan `docs/source/sorafs_gateway_dns_design_*.md` fayllari va tarqatish qaydlari. |

## Kirishlar va shartlar

- Deterministik xost spetsifikatsiyasi (`docs/source/soradns/deterministic_hosts.md`) va
  hal qiluvchi attestatsiya iskala (`docs/source/soradns/resolver_attestation_directory.md`).
- Gateway artefaktlari: operator qo'llanmasi, TLS/ECH avtomatlashtirish yordamchilari,
  to'g'ridan-to'g'ri rejim ko'rsatmalari va `docs/source/sorafs_gateway_*` ostida o'z-o'zini sertifikatlash ish jarayoni.
- Asboblar: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh` va CI yordamchilari
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Sirlar: GAR chiqarish kaliti, DNS/TLS ACME hisob ma'lumotlari, PagerDuty marshrutlash kaliti,
  Resolverni olish uchun Torii auth tokeni.

## Parvoz oldidan nazorat ro'yxati

1. Ishtirokchilar va kun tartibini yangilash orqali tasdiqlang
   `docs/source/sorafs_gateway_dns_design_attendance.md` va aylanma
   joriy kun tartibi (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Bosqich artefakt ildizlari kabi
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` va
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Yangilash moslamalari (GAR manifestlari, RAD isbotlari, shlyuzga muvofiqlik to'plamlari) va
   `git submodule` holati so'nggi mashq tegiga mos kelishiga ishonch hosil qiling.
4. Sirlarni tekshiring (Ed25519 chiqarish kaliti, ACME hisob fayli, PagerDuty tokeni)
   joriy va mos kassa nazorat summalari.
5. Tutun-test telemetriya maqsadlari (Pushgateway oxirgi nuqtasi, GAR Grafana platasi) oldingi
   matkapga.

## Avtomatlashtirishni mashq qilish bosqichlari

### Deterministik xost xaritasi va RAD katalogining chiqarilishi

1. Taklif etilgan manifestga qarshi deterministik xost hosilasi yordamchisini ishga tushiring
   o'rnating va hech qanday drift yo'qligini tasdiqlang
   `docs/source/soradns/deterministic_hosts.md`.
2. Resolver katalog to'plamini yarating:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Chop etilgan katalog identifikatorini, SHA-256 va chiqish yo'llarini yozib oling
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` va start
   daqiqa.

### DNS telemetriyasini yozib olish

- ≥10 daqiqa davomida quyruq hal qiluvchi shaffoflik jurnallari yordamida
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Pushgateway ko'rsatkichlarini eksport qiling va ishga tushirish bilan birga NDJSON snapshotlarini arxivlang
  ID katalogi.

### Gateway avtomatlashtirish mashqlari

1. TLS/ECH tekshiruvini bajaring:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Muvofiqlik simini ishga tushiring (`ci/check_sorafs_gateway_conformance.sh`) va
   yangilash uchun o'z-o'zini sertifikatlash yordamchisi (`scripts/sorafs_gateway_self_cert.sh`).
   Norito attestatsiya toʻplami.
3. Avtomatlashtirish yo'li oxirigacha ishlashini isbotlash uchun PagerDuty/Webhook hodisalarini yozib oling
   oxiri.

### Dalillarni qadoqlash

- `ops/drill-log.md` ni vaqt belgilari, ishtirokchilar va tekshiruv xeshlari bilan yangilang.
- Artefaktlarni ishga tushirish identifikatori kataloglari ostida saqlang va yakuniy xulosani nashr eting
  Docs/DevRel yig'ilish protokolida.
- Boshlanishni ko'rib chiqishdan oldin boshqaruv biletidagi dalillar to'plamini bog'lang.

## Sessiyani osonlashtirish va dalillarni topshirish

- **Moderator xronologiyasi:**  
  - T‑24h — Dastur boshqaruvi eslatma + kun tartibi/davomat suratini `#nexus-steering` da joylashtiradi.  
  - T‑2h — Networking TL GAR telemetriya suratini yangilaydi va deltalarni `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` da yozib oladi.  
  - T‑15m — Ops Automation zond tayyorligini tekshiradi va faol ishga tushirish identifikatorini `artifacts/sorafs_gateway_dns/current` ga yozadi.  
  - Qo'ng'iroq paytida — Moderator ushbu runbookni baham ko'radi va jonli kotibni tayinlaydi; Docs/DevRel inline harakat elementlarini yozib olish.
- **Daqiqa shabloni:** Skeletdan nusxa oling
  `docs/source/sorafs_gateway_dns_design_minutes.md` (portalda ham aks ettirilgan
  bundle) va har bir seans uchun bitta to'ldirilgan misolni bajaring. Ishtirokchilar ro'yxatini qo'shing,
  qarorlar, harakatlar elementlari, dalillar xeshlari va favqulodda xavflar.
- **Dalillarni yuklash:** Repetitsiyadan `runbook_bundle/` katalogini ziplang,
  ko'rsatilgan daqiqalarni PDF-ga ilova qiling, SHA-256 xeshlarini protokolga + kun tartibiga yozing,
  keyin yuklangandan so'ng boshqaruvni ko'rib chiquvchi taxallusga ping yuboring
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Dalillar surati (2025-yil mart oyi boshlanishi)

Yo'l xaritasi va boshqaruvda havola qilingan so'nggi mashq/jonli artefaktlar
daqiqalar `s3://sora-governance/sorafs/gateway_dns/` paqir ostida yashaydi. Xeshlar
quyida kanonik manifestni aks ettiradi (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Quruq yugurish — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball to'plami: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Daqiqalar PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Jonli seminar — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Yuklash kutilmoqda: `gateway_dns_minutes_20250303.pdf` — Koʻrsatilgan PDF toʻplamga tushgandan soʻng Docs/DevRel SHA-256 qoʻshadi.)_

## Tegishli material

- [Gateway operatsiyalari kitobi](./operations-playbook.md)
- [SoraFS kuzatish rejasi](./observability-plan.md)
- [Markazlashtirilmagan DNS va Gateway kuzatuvchisi](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)