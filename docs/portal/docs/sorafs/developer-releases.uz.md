---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a59639046a496f35c3cf80006a3330a25407b8143213f1b02b5ce766a70b4f0
source_last_modified: "2026-01-05T09:28:11.867590+00:00"
translation_last_reviewed: 2026-02-07
title: Release Process
summary: Run the CLI/SDK release gate, apply the shared versioning policy, and publish canonical release notes.
translator: machine-google-reviewed
---

# Chiqarish jarayoni

SoraFS ikkilik (`sorafs_cli`, `sorafs_fetch`, yordamchilar) va SDK qutilari
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) birgalikda jo'natiladi. Chiqarish
quvur liniyasi CLI va kutubxonalarni bir xilda ushlab turadi, lint/test qoplamasini ta'minlaydi va
quyi oqim iste'molchilari uchun artefaktlarni ushlaydi. Har biri uchun quyidagi nazorat roʻyxatini ishga tushiring
nomzod yorlig'i.

## 0. Xavfsizlik tekshiruvidan chiqishni tasdiqlang

Texnik bo'shatish eshigini ishga tushirishdan oldin, oxirgi xavfsizlik tekshiruvini oling
artefaktlar:

- So'nggi SF-6 xavfsizlik tekshiruvi eslatmasini yuklab oling ([reports/sf6-security-review](./reports/sf6-security-review.md))
  va uning SHA256 xeshini reliz chiptasiga yozib oling.
- Ta'mirlash chiptasi havolasini ilova qiling (masalan, `governance/tickets/SF6-SR-2026.md`) va ro'yxatdan o'tishga e'tibor bering
  Xavfsizlik muhandisligi va asboblar ishchi guruhidan tasdiqlovchilar.
- eslatmadagi tuzatish nazorat ro'yxati yopilganligini tekshiring; hal qilinmagan elementlar chiqarishni bloklaydi.
- Paritet jabduqlar jurnallarini yuklashga tayyorlaning (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  manifest to'plami bilan birga.
- Ishlamoqchi bo'lgan imzolash buyrug'iga `--identity-token-provider` ham, aniq ham kiradi.
  `--identity-token-audience=<aud>` shuning uchun Fulcio qamrovi chiqarilgan dalillarda ushlangan.

Boshqaruvni xabardor qilish va nashrni chop etishda ushbu artefaktlarni qo'shing.

## 1. Chiqarish/sinov eshigini bajaring

`ci/check_sorafs_cli_release.sh` yordamchisi formatlash, Clippy va testlarni bajaradi
CLI va SDK qutilari bo'ylab ish joyining mahalliy maqsadli katalogi (`.target`)
CI konteynerlarida ishlashda ruxsat nizolarni oldini olish uchun.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

Skript quyidagi tasdiqlarni bajaradi:

- `cargo fmt --all -- --check` (ish maydoni)
- `cargo clippy --locked --all-targets` `sorafs_car` uchun (`cli` xususiyati bilan),
  `sorafs_manifest` va `sorafs_chunker`
- Xuddi shu qutilar uchun `cargo test --locked --all-targets`

Har qanday qadam muvaffaqiyatsiz bo'lsa, teglashdan oldin regressiyani tuzating. Reliz tuzilmalari bo'lishi kerak
asosiy bilan uzluksiz; bo'shatish shoxlariga tuzatmalarni tanlamang. Darvoza
kalitsiz imzolash bayroqlarini ham tekshiradi (`--identity-token-issuer`, `--identity-token-audience`)
tegishli hollarda taqdim etiladi; etishmayotgan argumentlar ishda muvaffaqiyatsizlikka uchraydi.

## 2. Versiyalash siyosatini qo'llang

Barcha SoraFS CLI/SDK qutilari SemVerdan foydalanadi:

- `MAJOR`: Birinchi 1.0 versiyasi uchun taqdim etilgan. 1.0 dan oldin `0.y` kichik zarba
  **CLI yuzasi yoki Norito sxemalaridagi buzilish o'zgarishlarini bildiradi**.
  ixtiyoriy siyosat, telemetriya qo'shimchalari orqasida joylashgan maydonlar).
- `PATCH`: xatolar tuzatildi, faqat hujjatlar uchun nashrlar va qaramlik yangilanishlari
  kuzatiladigan xatti-harakatni o'zgartirmang.

Har doim `sorafs_car`, `sorafs_manifest` va `sorafs_chunker` ni bir xilda saqlang
versiya, shuning uchun quyi oqim SDK iste'molchilari bitta moslashtirilgan versiyaga bog'liq bo'lishi mumkin
ip. Versiyalarni o'chirishda:

1. Har bir qutining `Cargo.toml`dagi `version =` maydonlarini yangilang.
2. `Cargo.lock` ni `cargo update -p <crate>@<new-version>` orqali qayta yarating (
   ish maydoni aniq versiyalarni qo'llaydi).
3. Eskirgan artefaktlar qolmasligi uchun chiqarish eshigini qayta ishga tushiring.

## 3. Chiqarish eslatmalarini tayyorlang

Har bir nashrda CLI, SDK va
boshqaruvga ta'sir qiluvchi o'zgarishlar. Shablondan foydalaning
`docs/examples/sorafs_release_notes.md` (uni relizlar artefaktlariga nusxalash
katalog va bo'limlarni aniq tafsilotlar bilan to'ldiring).

Minimal tarkib:

- **E'tiborli jihatlar**: CLI va SDK iste'molchilari uchun xususiyat sarlavhalari.
  talablar.
- **Yangilanish bosqichlari**: yukga bog'liqlikni bartaraf etish va qayta ishga tushirish uchun TL;DR buyruqlari
  deterministik moslamalar.
- **Tasdiqlash**: buyruq chiqish xeshlari yoki konvertlari va aniq
  `ci/check_sorafs_cli_release.sh` reviziyasi bajarildi.

To'ldirilgan reliz yozuvlarini tegga biriktiring (masalan, GitHub nashri tanasi) va saqlang
ularni deterministik tarzda yaratilgan artefaktlar bilan birga.

## 4. Bo'shatish ilgaklarini bajaring

Imzolar to'plamini yaratish uchun `scripts/release_sorafs_cli.sh` ni ishga tushiring va
har bir relizda yuboriladigan tekshirish xulosasi. O'ram CLI ni yaratadi
kerak bo'lganda, `sorafs_cli manifest sign` ga qo'ng'iroq qiladi va darhol qayta o'ynaydi
`manifest verify-signature` shuning uchun teglashdan oldin nosozliklar yuzaga keladi. Misol:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Maslahatlar:

- Chiqarilgan ma'lumotlarni (foydali yuk, rejalar, xulosalar, kutilayotgan token xesh) kuzatib boring
  repo yoki joylashtirish konfiguratsiyasi, shuning uchun skript takrorlanishi mumkin. CI moslamasi
  `fixtures/sorafs_manifest/ci_sample/` ostidagi to'plam kanonik tartibni ko'rsatadi.
- `.github/workflows/sorafs-cli-release.yml` da asosiy CI avtomatizatsiyasi; ni boshqaradi
  chiqarish darvozasi, yuqoridagi skriptni chaqiradi va to'plamlarni/imzolarni sifatida arxivlaydi
  ish jarayonining artefaktlari. Xuddi shu buyruq tartibini aks ettiring (bo'shatish eshigi → belgisi →
  tekshirish) boshqa CI tizimlarida audit jurnallari yaratilgan xeshlarga mos kelishi uchun.
- Yaratilgan `manifest.bundle.json`, `manifest.sig` ni saqlang,
  `manifest.sign.summary.json` va `manifest.verify.summary.json` birgalikda—ular
  boshqaruv bildirishnomasida havola qilingan paketni shakllantiring.
- Chiqarish kanonik moslamalarni yangilaganda, yangilangan manifestni nusxalash,
  bo'lak rejasi va `fixtures/sorafs_manifest/ci_sample/` ga xulosalar (va yangilash
  `docs/examples/sorafs_ci_sample/manifest.template.json`) teglashdan oldin.
  Pastki oqim operatorlari relizni qayta ishlab chiqarish uchun belgilangan qurilmalarga bog'liq
  to'plam.
- `sorafs_cli proof stream` cheklangan kanal tekshiruvi uchun ishga tushirish jurnalini yozib oling va uni tarmoqqa biriktiring.
  oqim himoyasi faolligini isbotlash uchun paketni chiqaring.
- Chiqarish eslatmalarida imzolash paytida foydalanilgan aniq `--identity-token-audience` ni yozib oling; boshqaruv
  nashrni tasdiqlashdan oldin auditoriyani Fulcio siyosatiga qarshi tekshiradi.

Relizda a bo'lsa, `scripts/sorafs_gateway_self_cert.sh` dan foydalaning
shlyuzni ishga tushirish. Attestatsiyani isbotlash uchun uni xuddi shu manifest to'plamiga qarating
nomzod artefaktiga mos keladi:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Belgilang va chop eting

Tekshiruvlar o'tib, ilgaklar tugallangandan so'ng:

1. Ikkilik fayllarni tasdiqlash uchun `sorafs_cli --version` va `sorafs_fetch --version` ni ishga tushiring
   yangi versiya haqida xabar bering.
2. Belgilangan `sorafs_release.toml` da reliz konfiguratsiyasini tayyorlang
   (afzal) yoki joylashtirish repo tomonidan kuzatilgan boshqa konfiguratsiya fayli. Qochish
   ad-hoc muhit o'zgaruvchilariga tayanish; bilan CLI ga o'tish yo'llari
   `--config` (yoki ekvivalenti), shuning uchun chiqarish kirishlari aniq va
   takrorlanishi mumkin.
3. Imzolangan teg (afzal) yoki izohli teg yarating:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Artefaktlarni yuklang (CAR toʻplamlari, manifestlar, isbot xulosalari, relizlar qaydlari,
   attestatsiya natijalari) boshqaruvdan keyin loyihalar reestriga
   [tartibga solish qoʻllanmasi](./developer-deployment.md)dagi nazorat roʻyxati. Chiqarish bo'lsa
   zarb qilingan yangi armatura, ularni umumiy armatura repo yoki ob'ekt do'koniga suring
   auditni avtomatlashtirish nashr etilgan to'plamni manba nazoratidan farq qilishi mumkin.
5. Imzolangan tegga havolalar bilan boshqaruv kanalini xabardor qiling, nashr yozuvlari,
   manifest to'plami/imzo xeshlari, arxivlangan `manifest.sign/verify` xulosalari,
   va har qanday attestatsiya konvertlari. CI ish URL manzilini (yoki jurnal arxivini) qo'shing
   ishlagan `ci/check_sorafs_cli_release.sh` va `scripts/release_sorafs_cli.sh`. Yangilash
   auditorlar artefaktlarni tasdiqlashlarini kuzatishi uchun boshqaruv chiptasi; qachon
   `.github/workflows/sorafs-cli-release.yml` ish xabarlari bildirishnomalari, havola
   maxsus xulosalarni joylashtirishdan ko'ra yozib olingan xesh natijalari.

## 6. Chiqarishdan keyingi kuzatuv

- Hujjatlarning yangi versiyada ko'rsatilishiga ishonch hosil qiling (tezkor ishga tushirishlar, CI shablonlari)
  yangilangan yoki hech qanday o'zgartirish talab qilinmasligini tasdiqlang.
- Agar keyingi ish bo'lsa, yo'l xaritasi yozuvlari (masalan, migratsiya bayroqlari, eskirish
- Auditorlar uchun chiqarish eshigi chiqish jurnallarini arxivlash - ularni imzolanganlar yonida saqlang
  artefaktlar.

Ushbu quvur liniyasidan so'ng CLI, SDK qutilari va boshqaruv garovi saqlanadi
har bir chiqarish sikli uchun qulflash bosqichi.