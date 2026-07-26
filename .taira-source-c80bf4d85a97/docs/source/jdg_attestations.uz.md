---
lang: uz
direction: ltr
source: docs/source/jdg_attestations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 459e8ed4612da7cfa68053e4e299b2f68e7620d4f3b98a8a721ebf8327829ea1
source_last_modified: "2026-01-08T21:57:18.412403+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JDG attestatsiyalari: qo'riqlash, aylanish va saqlash

Ushbu eslatma hozirda `iroha_core` da jo'natiladigan v1 JDG attestatsiya qo'riqchisini hujjatlashtiradi.

- **Qo'mitaning ma'lumotlari:** Norito kodli `JdgCommitteeManifest` to'plamlari har bir ma'lumot maydoni aylanishini ta'minlaydi
  jadvallar (`committee_id`, buyurtma qilingan a'zolar, pol, `activation_height`, `retire_height`).
  Manifestlar `JdgCommitteeSchedule::from_path` bilan yuklanadi va ularni qat'iy ravishda oshiradi
  faollashtirish balandliklari ixtiyoriy mos kelishi (`grace_blocks`) bilan bekor qilish/faollashtirish o'rtasida
  qo'mitalar.
- **Attestatsiya qo'riqchisi:** `JdgAttestationGuard` ma'lumotlar maydonini bog'lash, amal qilish muddati, eskirgan chegaralarni ta'minlaydi,
  qoʻmita identifikatori/eshik chegarasi, imzolovchi aʼzoligi, qoʻllab-quvvatlanadigan imzo sxemalari va ixtiyoriy
  `JdgSdnEnforcer` orqali SDN tekshiruvi. Hajmi kattaligi, maksimal kechikish va ruxsat etilgan imzo sxemalari
  konstruktor parametrlari; `validate(attestation, dataspace, current_height)` faolni qaytaradi
  qo'mita yoki tuzilgan xato.
  - `scheme_id = 1` (`simple_threshold`): har bir imzolovchi imzosi, ixtiyoriy imzo bitmap.
  - `scheme_id = 2` (`bls_normal_aggregate`): bitta oldindan yig'ilgan BLS-normal imzo
    attestatsiya xeshi; imzolovchi bitmap ixtiyoriy, attestatsiyadagi barcha imzolovchilar uchun birlamchi. BLS
    agregat tekshirish manifestda har bir qo'mita a'zosi uchun haqiqiy PoPni talab qiladi; yo'qolgan yoki
    yaroqsiz POPlar attestatsiyani rad etadi.
  `governance.jdg_signature_schemes` orqali ruxsat etilgan ro'yxatni sozlang.
- **Saqlash do'koni:** `JdgAttestationStore` sozlanishi mumkin bo'lgan ma'lumotlar maydoni uchun attestatsiyalarni kuzatib boradi.
  har bir ma'lumot maydoni qopqog'i, qo'shimchadagi eng qadimgi yozuvlarni kesish. `for_dataspace` yoki
  Audit/takrorlash paketlarini olish uchun `for_dataspace_and_epoch`.
- **Testlar:** Birlik qamrovi endi qo'mitaning haqiqiy tanlovini, noma'lum imzolovchini rad etishni, eskirganligini amalga oshiradi
  attestatsiyani rad etish, qo'llab-quvvatlanmaydigan sxema identifikatorlari va ushlab turishni kesish. Qarang
  `crates/iroha_core/src/jurisdiction.rs`.

Qo'riqchi tuzilgan ruxsatnomalar ro'yxatidan tashqari sxemalarni rad etadi.