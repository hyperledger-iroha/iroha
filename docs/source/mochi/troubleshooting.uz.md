---
lang: uz
direction: ltr
source: docs/source/mochi/troubleshooting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb67b304bae01fa4a50d25dc9f086811dabfbcb24239b3ec9679338248e18be6
source_last_modified: "2025-12-29T18:16:35.985892+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# MOCHI nosozliklarini bartaraf etish bo'yicha qo'llanma

Mahalliy MOCHI klasterlari ishga tushirishdan bosh tortganda, bu ish kitobidan foydalaning
tsiklni qayta ishga tushiring yoki blok/hodisa/holat yangilanishlarini oqimlashni to'xtating. U uzaytiradi
yo'l xaritasi elementi "Hujjatlar va tarqatish" nazoratchi xatti-harakatlarini o'zgartirish orqali
`mochi-core` aniq tiklash bosqichlariga.

## 1. Birinchi javob beruvchining nazorat ro'yxati

1. MOCHI ishlatayotgan maʼlumotlar ildizini yozib oling. Standart quyidagicha
   `$TMPDIR/mochi/<profile-slug>`; maxsus yo'llar UI sarlavha satrida paydo bo'ladi va
   `cargo run -p mochi-ui-egui -- --data-root ...` orqali.
2. Ishchi maydon ildizidan `./ci/check_mochi.sh` ni ishga tushiring. Bu yadroni tasdiqlaydi,
   Konfiguratsiyalarni o'zgartirishni boshlashdan oldin foydalanuvchi interfeysi va integratsiya qutilari.
3. Oldindan o'rnatilganga e'tibor bering (`single-peer` yoki `four-peer-bft`). Yaratilgan topologiya
   ma'lumotlar ildizi ostida qancha tengdosh papkalarni/jurnallarni kutish kerakligini aniqlaydi.

## 2. Jurnallar va telemetriya dalillarini to'plang

`NetworkPaths::ensure` (qarang `mochi/mochi-core/src/config.rs`) barqarorlikni yaratadi
tartib:

```
<data_root>/<profile>/
  peers/<alias>/...
  logs/<alias>.log
  genesis/
  snapshots/
```

O'zgartirishlar kiritishdan oldin quyidagi amallarni bajaring:

- Oxirgi suratga olish uchun **Jurnallar** yorlig'idan foydalaning yoki to'g'ridan-to'g'ri `logs/<alias>.log` ni oching
  Har bir tengdosh uchun 200 qator. Nazoratchi stdout/stderr/tizim kanallarini kuzatib boradi
  `PeerLogStream` orqali, shuning uchun bu fayllar UI chiqishiga mos keladi.
- Suratni **Xizmat ko‘rsatish → Suratni eksport qilish** (yoki qo‘ng‘iroq) orqali eksport qiling
  `Supervisor::export_snapshot`). Snapshot saqlash, konfiguratsiyalar va
  `snapshots/<timestamp>-<label>/` tizimiga kiradi.
- Agar muammo oqim vidjetlari bilan bog'liq bo'lsa, `ManagedBlockStream` nusxasini oling,
  `ManagedEventStream` va `ManagedStatusStream` sog'liqni saqlash ko'rsatkichlari
  Boshqaruv paneli. UI oxirgi qayta ulanishga urinish va xato sababini ko'rsatadi; tutmoq
  voqea rekordi uchun skrinshot.

## 3. Tengdoshlarni ishga tushirish muammolarini hal qilish

Ko'pgina tengdoshlarni ishga tushirishda muvaffaqiyatsizliklar uchta chelakka bo'linadi:

### Ikkilik fayl yoki noto'g'ri bekor qilish

`SupervisorBuilder`, `irohad`, `kagami` va (kelajakda) `iroha_cli` ga chiqadi.
Agar UI "jarayonni ishlab chiqara olmadi" yoki "ruxsat rad etildi" haqida xabar bersa, MOCHIga ishora qiling
yaxshi ma'lum ikkiliklarda:

```bash
cargo run -p mochi-ui-egui -- \
  --irohad /path/to/irohad \
  --kagami /path/to/kagami \
  --iroha-cli /path/to/iroha_cli
```

Buning oldini olish uchun `MOCHI_IROHAD`, `MOCHI_KAGAMI` va `MOCHI_IROHA_CLI` ni oʻrnatishingiz mumkin.
bayroqlarni qayta-qayta terish. To'plam tuzilmalarini disk raskadrovka qilishda ularni solishtiring
`BundleConfig`, `mochi/mochi-ui-egui/src/config/` ichidagi yo'llarga qarshi
`target/mochi-bundle`.

### Port to'qnashuvi

`PortAllocator` konfiguratsiyalarni yozishdan oldin orqaga qaytish interfeysini tekshiradi. Agar ko'rsangiz
`failed to allocate Torii port` yoki `failed to allocate P2P port`, boshqasi
jarayon allaqachon standart diapazonda (8080/1337) tinglanmoqda. MOCHI-ni qayta ishga tushiring
aniq asoslar bilan:

```bash
cargo run -p mochi-ui-egui -- --torii-start 12000 --p2p-start 19000
```

Quruvchi ushbu bazalardan ketma-ket portlarni chiqaradi, shuning uchun diapazonni zaxiralang
oldindan o'rnatilgan o'lchamingiz (`peer_count` tengdoshlari → `peer_count` portlari har bir transport uchun).

### Ibtido va saqlash buzilishiAgar Kagami manifest chiqarishdan oldin chiqsa, tengdoshlar darhol ishdan chiqadi. Tekshirish
Ma'lumotlar ildizi ichida `genesis/*.json`/`.toml`. bilan qayta ishga tushirish
`--kagami /path/to/kagami` yoki **Sozlamalar** dialog oynasini o'ng ikkilik tomon yo'naltiring.
Saqlash buzilishi uchun Xizmat bo'limining **O'chirish va qayta yaratish** dan foydalaning.
papkalarni qo'lda o'chirish o'rniga tugma (quyida qoplangan); ni qayta yaratadi
jarayonlarni qayta boshlashdan oldin peer kataloglari va oniy rasm ildizlari.

### Avtomatik qayta ishga tushirishni sozlash

`[supervisor.restart]`, `config/local.toml` (yoki CLI bayroqlari)
`--restart-mode`, `--restart-max`, `--restart-backoff-ms`) qanchalik tez-tez tekshiriladi
supervayzer muvaffaqiyatsiz tengdoshlarini qaytadan sinab ko'radi. UI kerak bo'lganda `mode = "never"` sozlang
birinchi nosozlikni darhol yuzaga keltiring yoki `max_restarts`/`backoff_ms` ni qisqartiring
tez muvaffaqiyatsiz bo'lishi kerak bo'lgan CI ishlari uchun qayta urinish oynasini torting.

## 4. Tengdoshlarni xavfsiz tiklash

1. Boshqaruv panelidagi zararlangan tengdoshlarni to'xtating yoki UIdan chiqing. Nazoratchi
   tengdosh ishlayotgan vaqtda xotirani tozalashni rad etadi (`PeerHandle::wipe_storage`
   qaytaradi `PeerStillRunning`).
2. **Xizmat ko‘rsatish → O‘chirish va qayta yaratish** ga o‘ting. MOCHI:
   - `peers/<alias>/storage`ni o'chirish;
   - `genesis/` ostida konfiguratsiya/genezisni qayta tiklash uchun Kagami ni qayta ishga tushiring; va
   - saqlangan CLI/muhitni bekor qilish bilan tengdoshlarni qayta ishga tushiring.
3. Agar buni qo'lda qilishingiz kerak bo'lsa:
   ```bash
   cargo run -p mochi-ui-egui -- --data-root /tmp/mochi --profile four-peer-bft --help
   # Note the actual root printed above, then:
   rm -rf /tmp/mochi/four-peer-bft
   ```
   Keyin MOCHI-ni qayta ishga tushiring, shunda `NetworkPaths::ensure` daraxtni qayta yaratadi.

`snapshots/<timestamp>` jildini oʻchirishdan oldin har doim arxivlang, hatto mahalliy boʻlsa ham
ishlab chiqish - bu to'plamlar aniq `irohad` jurnallari va kerakli konfiguratsiyalarni oladi
xatolarni ko'paytirish uchun.

### 4.1 Snapshotlardan tiklash

Tajriba xotirani buzsa yoki yaxshi ma'lum bo'lgan holatni qayta o'ynashingiz kerak bo'lsa, Ta'minotdan foydalaning
Nusxa olish o‘rniga dialog oynasining **Snapshotni tiklash** tugmasi (yoki `Supervisor::restore_snapshot` ga qo‘ng‘iroq qiling)
kataloglarni qo'lda. Toʻplamga mutlaq yoʻlni yoki tozalangan jild nomini koʻrsating
`snapshots/` ostida. Nazoratchi:

1. har qanday yuguruvchi tengdoshlarni to'xtatish;
2. suratning `metadata.json` joriy `chain_id` va tengdoshlar soniga mos kelishini tekshiring;
3. `peers/<alias>/{storage,snapshot,config.toml,latest.log}` ni yana faol profilga nusxalash; va
4. Tengdoshlarni qayta ishga tushirishdan oldin `genesis/genesis.json` ni tiklang, agar ular oldindan ishlayotgan bo'lsa.

Agar surat boshqa oldindan o'rnatilgan yoki zanjir identifikatori uchun yaratilgan bo'lsa, tiklash qo'ng'irog'i a qaytaradi
`SupervisorError::Config`, shuning uchun siz artefaktlarni jimgina aralashtirish o'rniga mos keladigan to'plamni olishingiz mumkin.
Qayta tiklash mashqlarini tezlashtirish uchun har bir oldindan o'rnatilgan kamida bitta yangi suratni saqlang.

## 5. Blok/hodisa/holat oqimlarini tuzatish- **Oqim to‘xtab qoldi, lekin tengdoshlar sog‘lom.** **Voqealar**/**Bloklar** panellarini tekshiring
  qizil holat qatorlari uchun. Boshqariladigan oqimni majburlash uchun "To'xtatish" va "Boshlash" tugmasini bosing
  qayta obuna bo'lish; nazoratchi har bir qayta ulanish urinishini qayd qiladi (tengdosh taxalluslari va
  xato) shuning uchun orqaga qaytish bosqichlarini tasdiqlashingiz mumkin.
- **Holat qoplamasi eskirgan.** `ManagedStatusStream` har bir `/status` so‘rovi
  ikki soniya va `STATUS_POLL_INTERVAL * dan keyin ma'lumotlar eskirganligini ko'rsatadi
  STATUS_STALE_MULTIPLIER` (standart olti soniya). Agar nishon qizil bo'lib qolsa, tasdiqlang
  Peer konfiguratsiyasida `torii_status_url` va shlyuz yoki VPN yo'qligiga ishonch hosil qiling
  orqaga qaytish ulanishlarini blokirovka qilish.
- **Hodisani dekodlashda xatolik yuz berdi.** UI dekodlash bosqichini chop etadi (xom baytlar,
  `BlockSummary` yoki Norito dekodlash) va qoidabuzar tranzaksiya xeshi. Eksport
  hodisani clipboard tugmasi orqali o'tkazing, shuning uchun siz testlarda dekodlashni takrorlashingiz mumkin
  (`mochi-core` ostida yordamchi konstruktorlarni ko'rsatadi
  `mochi/mochi-core/src/torii.rs`).

Oqimlar qayta-qayta ishdan chiqqanda, muammoni aynan tengdosh taxalluslari bilan yangilang va
xato qatori (`ToriiErrorKind`), shuning uchun yo'l xaritasi telemetriya bosqichlari bog'langan holda qoladi
aniq dalillarga.