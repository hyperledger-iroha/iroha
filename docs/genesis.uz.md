---
lang: uz
direction: ltr
source: docs/genesis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c2eab4379aa346ab7d111e1c51c0230238f260647187f1a33c1819640b9bf2c
source_last_modified: "2026-01-28T14:25:37.056140+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Genesis konfiguratsiyasi

`genesis.json` fayli Iroha tarmog'i ishga tushganda bajariladigan birinchi tranzaktsiyalarni belgilaydi. Fayl quyidagi maydonlarga ega JSON obyektidir:

- `chain` – noyob zanjir identifikatori.
- `executor` (ixtiyoriy) – ijrochi baytekodiga yo'l (`.to`). Agar mavjud bo'lsa,
  genesis birinchi tranzaksiya sifatida Yangilash yo'riqnomasini o'z ichiga oladi. Agar o'tkazib yuborilsa,
  hech qanday yangilash amalga oshirilmaydi va o'rnatilgan ijrochi ishlatiladi.
- `ivm_dir` - IVM bayt-kod kutubxonalarini o'z ichiga olgan katalog. Agar o'tkazib yuborilsa, birlamchi `"."`.
- `consensus_mode` - manifestda e'lon qilingan konsensus rejimi. Majburiy; umumiy Sora Nexus maʼlumotlar maydoni uchun `"Npos"` yoki boshqa Iroha3 maʼlumotlar maydonlari uchun `"Permissioned"`/`"Npos"` dan foydalaning. Iroha2 standarti `"Permissioned"`.
- `transactions` - ketma-ket bajariladigan genezis operatsiyalari ro'yxati. Har bir yozuv quyidagilarni o'z ichiga olishi mumkin:
  - `parameters` – boshlang'ich tarmoq parametrlari.
  - `instructions` - tuzilgan Norito ko'rsatmalari (masalan, `{ "Register": { "Domain": { "id": "wonderland" }}}`). Xom bayt massivlari qabul qilinmaydi va `SetParameter` ko'rsatmalari bu erda rad etiladi - `parameters` bloki orqali urug' parametrlari va ko'rsatmalarni normallashtirish/imzolash imkonini beradi.
  - `ivm_triggers` - IVM bayt-kod bajariladigan fayllar bilan ishga tushirgichlar.
  - `topology` – dastlabki tengdosh topologiyasi. Har bir yozuv peer identifikatori va PoP ni birga saqlaydi: `{ "peer": "<public_key>", "pop_hex": "<hex>" }`. `pop_hex` yozish paytida oʻtkazib yuborilishi mumkin, lekin imzolashdan oldin boʻlishi kerak.
- `crypto` – `iroha_config.crypto` (`default_hash`, `allowed_signing`, `allowed_curve_ids`, `sm2_distid_default`, `allowed_signing`0, I01) dan aks ettirilgan kriptografiya oniy tasviri. `allowed_curve_ids` `crypto.curves.allowed_curve_ids`ni aks ettiradi, shuning uchun manifestlar klaster qaysi kontroller egri chiziqlarini qabul qilishini reklama qilishi mumkin. Asboblar SM kombinatsiyalarini qo'llaydi: `sm2` ro'yxatidagi manifestlar xeshni `sm3-256` ga almashtirishi kerak, `sm` xususiyatisiz tuzilgan tuzilmalar esa `sm2` ni butunlay rad etadi. Normalizatsiya imzolangan genezisga `crypto_manifest_meta` maxsus parametrini kiritadi; Agar kiritilgan foydali yuk reklama qilingan suratga mos kelmasa, tugunlar ishga tushirishni rad etadi.

Misol (`kagami genesis generate default --consensus-mode npos` chiqishi, ko'rsatmalar kesilgan):

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [
        { "Register": { "Domain": { "id": "wonderland" } } }
      ],
      "ivm_triggers": [],
      "topology": [
        {
          "peer": "ed25519:...",
          "pop_hex": "ab12cd..."
        }
      ]
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519", "secp256k1"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### SM2/SM3 uchun `crypto` blokini ekish

Bitta bosqichda asosiy inventar va joylashtirishga tayyor konfiguratsiya snippetini yaratish uchun xtask yordamchisidan foydalaning:

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

`client-sm2.toml` hozir quyidagilarni o'z ichiga oladi:

```toml
# Account key material
public_key = "sm2:8626530010..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "secp256k1", "sm2"]  # remove "sm2" to stay in verify-only mode
allowed_curve_ids = [1]               # add new curve ids (e.g., 15 for SM2) when controllers are allowed
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```

`public_key`/`private_key` qiymatlarini hisob/mijoz konfiguratsiyasiga nusxa ko'chiring va `genesis.json` `crypto` blokini parchaga mos kelishi uchun yangilang (masalan, I18NI00000000NI ga I18NI0000000070807 qo'shing. `"sm2"` - `allowed_signing` va o'ng `allowed_curve_ids` ni o'z ichiga oladi). Kagami xesh/egri chiziq sozlamalari va imzolash roʻyxati mos kelmaydigan manifestlarni rad etadi.

> **Maslahat:** Chiqishni tekshirmoqchi bo'lganingizda, parchani `--snippet-out -` bilan stdout-ga uzating. Stdout-da asosiy inventarni chiqarish uchun `--json-out -` dan foydalaning.

Agar siz quyi darajadagi CLI buyruqlarini qo'lda boshqarishni afzal ko'rsangiz, ekvivalent oqim:

```bash
# 1. Produce deterministic key material (writes JSON to disk)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. Re-hydrate the snippet that can be pasted into client/config files
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **Maslahat:** `jq` qo'lda nusxa ko'chirish/joylashtirish bosqichini saqlash uchun yuqorida qo'llaniladi. Agar u mavjud bo'lmasa, `sm2-key.json` ni oching, `private_key_hex` maydonidan nusxa oling va uni to'g'ridan-to'g'ri `crypto sm2 export` ga o'tkazing.

> **Migratsiya qo‘llanmasi:** Mavjud tarmoqni SM2/SM3/SM4 ga o‘tkazishda quyidagi amallarni bajaring.
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md)
> qatlamli `iroha_config` bekor qilish, manifest regeneratsiyasi va orqaga qaytarish uchun
> rejalashtirish.

## Yarating va tasdiqlang

1. Shablonni yarating:
   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     --ivm-dir <ivm/dir> \
     --genesis-public-key <PUBLIC_KEY> > genesis.json
   ```
`--consensus-mode` qaysi konsensus parametrlari Kagami urug'larini `parameters` blokiga kiritishni nazorat qiladi. Ommaviy Sora Nexus maʼlumotlar maydoni `npos` ni talab qiladi va bosqichli kesishlarni qoʻllab-quvvatlamaydi; boshqa Iroha3 ma'lumotlar maydonlari ruxsat etilgan yoki NPoS dan foydalanishi mumkin. Iroha2 sukut bo'yicha `permissioned` ga mos keladi va `npos` `--next-consensus-mode`/`--mode-activation-height` orqali amalga oshirishi mumkin. `npos` tanlansa, Kagami NPoS kollektorini yoqish, saylov siyosati va qayta konfiguratsiya oynalarini boshqaradigan `sumeragi_npos_parameters` foydali yukini hosil qiladi; normallashtirish/imzolash ularni imzolangan blokdagi `SetParameter` ko'rsatmalariga aylantiradi.
2. `genesis.json` ni ixtiyoriy ravishda tahrirlang, keyin tasdiqlang va imzolang:
   ```bash
   cargo run -p iroha_kagami -- genesis sign genesis.json \
     --public-key <PUBLIC_KEY> \
     --private-key <PRIVATE_KEY> \
     --out-file genesis.signed.nrt
   ```

   SM2/SM3/SM4-ga tayyor manifestlarni chiqarish uchun `--default-hash sm3-256`-ni o'tkazing va `--allowed-signing sm2`-ni qo'shing (qo'shimcha algoritmlar uchun `--allowed-signing`-ni takrorlang). Standart farqlovchi identifikatorni bekor qilishingiz kerak bo'lsa, `--sm2-distid-default <ID>` dan foydalaning.

   `irohad` ni faqat `--genesis-manifest-json` (imzolangan genezis bloki yo'q) bilan ishga tushirganingizda, tugun endi manifestdan avtomatik ravishda o'zining ish vaqti kripto konfiguratsiyasini ekadi; Agar siz genezis blokini ham taqdim qilsangiz, manifest va konfiguratsiya hali ham to'liq mos kelishi kerak.

- Tasdiqlash eslatmalari:
  - Kagami normallashtirilgan/imzolangan blokda `SetParameter` ko'rsatmalari sifatida `consensus_handshake_meta`, `confidential_registry_root` va `crypto_manifest_meta` ni kiritadi. `irohad` ushbu foydali yuklardan konsensus barmoq izini qayta hisoblab chiqadi va agar qoʻl siqish metamaʼlumotlari yoki kripto oniy tasviri kodlangan parametrlarga mos kelmasa, ishga tushirilmaydi. Bularni manifestda `instructions` dan tashqarida saqlang; ular avtomatik ravishda yaratiladi.
- Normallashtirilgan blokni tekshiring:
  - Yakuniy buyurtma qilingan tranzaktsiyalarni (jumladan, kiritilgan metama'lumotlarni) kalit juftligini taqdim etmasdan ko'rish uchun `kagami genesis normalize genesis.json --format text` ni ishga tushiring.
  - `--format json` dan farqlash yoki ko'rib chiqish uchun mos tuzilgan ko'rinishni o'chirish uchun foydalaning.

`kagami genesis sign` JSON haqiqiyligini tekshiradi va tugun konfiguratsiyasida `genesis.file` orqali foydalanishga tayyor Norito kodlangan blokni ishlab chiqaradi. Olingan `genesis.signed.nrt` allaqachon kanonik sim ko'rinishida: versiya bayti, undan keyin foydali yuk tartibini tavsiflovchi Norito sarlavhasi. Har doim bu ramkali chiqishni tarqating. Imzolangan foydali yuklar uchun `.nrt` qo'shimchasini afzal ko'ring; agar siz genezisda ijrochini yangilashingiz shart bo'lmasa, `executor` maydonini qoldirib, `.to` faylini taqdim etishni o'tkazib yuborishingiz mumkin.

NPoS manifestlarini imzolashda (`--consensus-mode npos` yoki faqat Iroha2 uchun bosqichli kesmalar), `kagami genesis sign` `sumeragi_npos_parameters` foydali yukini talab qiladi; uni `kagami genesis generate --consensus-mode npos` bilan yarating yoki parametrni qo'lda qo'shing.
Odatiy bo'lib, `kagami genesis sign` manifestning `consensus_mode` dan foydalanadi; uni bekor qilish uchun `--consensus-mode` dan o'ting.

## Ibtido nima qila oladi

Genesis quyidagi operatsiyalarni qo'llab-quvvatlaydi. Kagami ularni tranzaktsiyalarga aniq belgilangan tartibda yig'adi, shuning uchun tengdoshlar bir xil ketma-ketlikni aniq bajaradilar.

- Parametrlar: Sumeragi (blok/tasdiqlash vaqtlari, drift), Bloklash (maksimal txs), tranzaksiya (maksimal ko'rsatmalar, bayt-kod hajmi), Ijrochi va aqlli shartnoma chegaralari (yoqilg'i, xotira, chuqurlik) va moslashtirilgan parametrlar uchun boshlang'ich qiymatlarni o'rnating. Kagami urug'lari `Sumeragi::NextMode` va `sumeragi_npos_parameters` foydali yuki (NPoS tanlovi, qayta sozlash) `parameters` bloki orqali ishga tushirilishi zanjir holatidan konsensus tugmalarini qo'llashi mumkin; imzolangan blok yaratilgan `SetParameter` ko'rsatmalarini olib yuradi.
- Mahalliy ko'rsatmalar: domenni ro'yxatdan o'tkazish/ro'yxatdan o'chirish, hisob qaydnomasi, aktiv ta'rifi; Mint/Burn/Transfer aktivlari; Domen va aktiv taʼrifiga egalik huquqini oʻtkazish; Metama'lumotlarni o'zgartirish; Ruxsat va rollarni bering.
- IVM Triggerlari: IVM bayt kodini bajaradigan triggerlarni ro'yxatdan o'tkazing (qarang: `ivm_triggers`). Triggerlarning bajariladigan fayllari `ivm_dir` ga nisbatan hal qilinadi.
- Topologiya: Har qanday tranzaksiya ichidagi `topology` massivi orqali boshlang'ich tengdoshlar to'plamini taqdim eting (odatda birinchi yoki oxirgi). Har bir yozuv `{ "peer": "<public_key>", "pop_hex": "<hex>" }`; `pop_hex` matn yozish vaqtida oʻtkazib yuborilishi mumkin, lekin imzolashdan oldin boʻlishi kerak.
- Ijrochini yangilash (ixtiyoriy): Agar `executor` mavjud bo'lsa, genezis birinchi tranzaksiya sifatida bitta Upgrade yo'riqnomasini kiritadi; aks holda, genezis to'g'ridan-to'g'ri parametrlar/ko'rsatmalar bilan boshlanadi.

### Tranzaksiyaga buyurtma berish

Konseptual ravishda, genezis operatsiyalari quyidagi tartibda qayta ishlanadi:

1) (ixtiyoriy) Ijrochini yangilash
2) `transactions` dagi har bir tranzaksiya uchun:
   - Parametrlarni yangilash
   - Mahalliy ko'rsatmalar
   - IVM ro'yxatga olishni boshlash
   - Topologiya yozuvlari

Kagami va tugun kodi ushbu tartibni ta'minlaydi, masalan, parametrlar xuddi shu tranzaksiyadagi keyingi ko'rsatmalardan oldin qo'llaniladi.

## Tavsiya etilgan ish jarayoni

- Kagami bilan shablondan boshlang:
  - Faqat oʻrnatilgan ISI: `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json` (Sora Nexus umumiy maʼlumotlar maydoni; Iroha2 yoki shaxsiy Iroha3 uchun `--consensus-mode permissioned` dan foydalaning).
  - Maxsus ijrochini yangilash bilan (ixtiyoriy): `--executor <path/to/executor.to>` qo'shing
  - Faqat Iroha2 uchun: kelajakda NPoS ga o'tish uchun `--next-consensus-mode npos --mode-activation-height <HEIGHT>` ni o'tkazing (joriy rejim uchun `--consensus-mode permissioned` saqlang).
- `<PK>` - `iroha_crypto::Algorithm` tomonidan tan olingan har qanday multihash, shu jumladan Kagami `--features gost` bilan qurilganda TC26 GOST variantlari (masalan, `gost3410-2012-256-paramset-a:...`).
- Tahrirlash paytida tasdiqlang: `kagami genesis validate genesis.json`
- Joylashtirish uchun imzo: `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- Tengdoshlarni sozlang: `genesis.file` ni imzolangan Norito fayliga (masalan, `genesis.signed.nrt`) va `genesis.public_key` ni imzolash uchun ishlatiladigan `<PK>` ga sozlang.Eslatmalar:
- Kagami “standart” shabloni namunaviy domen va hisob qaydnomalarini ro‘yxatdan o‘tkazadi, bir nechta aktivlarni zarb qiladi va faqat o‘rnatilgan ISIlar yordamida minimal ruxsatlarni beradi – `.to` shart emas.
- Agar siz ijrochini yangilashni o'z ichiga olgan bo'lsangiz, bu birinchi tranzaksiya bo'lishi kerak. Kagami buni yaratish/imzolashda amalga oshiradi.
- Imzolashdan oldin noto'g'ri `Name` qiymatlarini (masalan, bo'sh joy) va noto'g'ri tuzilgan ko'rsatmalarni qo'lga olish uchun `kagami genesis validate` dan foydalaning.

## Docker/Swarm bilan ishlash

Taqdim etilgan Docker Compose and Swarm asboblari ikkala holatda ham ishlaydi:

- Ijrochisiz: tuzish buyrug'i etishmayotgan/bo'sh `executor` maydonini o'chiradi va faylga imzo qo'yadi.
- Ijrochi bilan: konteyner ichidagi mutlaq yo'lga nisbatan bajaruvchi yo'lini hal qiladi va faylga imzo qo'yadi.

Bu oldindan o'rnatilgan IVM namunalarisiz mashinalarda ishlab chiqishni soddalashtiradi va kerak bo'lganda ijrochini yangilashga imkon beradi.