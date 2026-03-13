---
lang: uz
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0581539af03125aca3ed8009387b4552361d982d8cdc80f6a8ae5bbe3e2f271f
source_last_modified: "2026-01-05T09:28:11.915872+00:00"
translation_last_reviewed: 2026-02-07
id: puzzle-service-operations
title: Puzzle Service Operations Guide
sidebar_label: Puzzle Service Ops
description: Operating the `soranet-puzzle-service` daemon for Argon2/ML-DSA admission tickets.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

# Puzzle xizmati operatsiyalari bo'yicha qo'llanma

`soranet-puzzle-service` demoni (`tools/soranet-puzzle-service/`) muammolari
Releyning `pow.puzzle.*` siyosatini aks ettiruvchi Argon2 tomonidan qo'llab-quvvatlanadigan kirish chiptalari
va konfiguratsiya qilinganida, brokerlar chekka o'rni nomidan ML-DSA qabul tokenlarini.
U beshta HTTP so'nggi nuqtalarini ko'rsatadi:

- `GET /healthz` - jonli zond.
- `GET /v2/puzzle/config` - olingan samarali PoW/jumboq parametrlarini qaytaradi
  JSON relesidan (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v2/puzzle/mint` - Argon2 chiptasini zarb qiladi; ixtiyoriy JSON tanasi
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  qisqaroq TTL so'raydi (qoida oynasiga mahkamlangan), chiptani a ga bog'laydi
  transkript xesh va rele imzolangan chipta + imzo barmoq izini qaytaradi
  imzolash kalitlari sozlanganda.
- `GET /v2/token/config` - qachon `pow.token.enabled = true`, faolni qaytaradi
  Qabul qilish tokeni siyosati (emitentning barmoq izi, TTL/soatning egilish chegaralari, reley identifikatori,
  va birlashtirilgan bekor qilish to'plami).
- `POST /v2/token/mint` - etkazib berilganga bog'langan ML-DSA qabul tokenini chiqaradi
  xeshni davom ettirish; so'rov organi `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }` ni qabul qiladi.

Xizmat tomonidan ishlab chiqarilgan chiptalar tasdiqlangan
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`
integratsiya testi, u shuningdek volumetrik DoS paytida o'rni drossellarini mashq qiladi
stsenariylar.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Token chiqarishni sozlash

`pow.token.*` ostida o'rni JSON maydonlarini o'rnating (qarang
Misol uchun `tools/soranet-relay/deploy/config/relay.entry.json`) yoqish uchun
ML-DSA tokenlari. Kamida emitentga ochiq kalitni taqdim eting va ixtiyoriy
bekor qilish ro'yxati:

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

Puzzle xizmati ushbu qiymatlarni qayta ishlatadi va Norito ni avtomatik ravishda qayta yuklaydi
Ish vaqtida JSON bekor qilish fayli. `soranet-admission-token` CLI dan foydalaning
(`cargo run -p soranet-relay --bin soranet_admission_token`) zarb qilish va tekshirish
tokenlarni oflayn rejimda o'tkazing, `token_id_hex` yozuvlarini bekor qilish fayliga qo'shing va audit qiling
ishlab chiqarishga yangilanishlarni kiritishdan oldin mavjud hisob ma'lumotlari.

Emitentning maxfiy kalitini CLI bayroqlari orqali jumboq xizmatiga o'tkazing:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` maxfiy tarmoqdan tashqarida boshqarilsa ham mavjud
asboblar quvur liniyasi. Bekor qilish faylini kuzatuvchisi `/v2/token/config` oqimini saqlaydi;
orqada qolmaslik uchun yangilanishlarni `soranet-admission-token revoke` buyrug'i bilan muvofiqlashtiring
bekor qilish holati.

ML-DSA-44 ommabopini reklama qilish uchun JSON releyida `pow.signed_ticket_public_key_hex` ni o'rnating
imzolangan PoW chiptalarini tekshirish uchun ishlatiladigan kalit; `/v2/puzzle/config` kalit va uning BLAKE3 ni aks ettiradi
barmoq izi (`signed_ticket_public_key_fingerprint_hex`), shuning uchun mijozlar tekshirgichni mahkamlashi mumkin.
Imzolangan chiptalar reley identifikatori va transkript bog'lanishlari bilan tasdiqlanadi va bir xil bo'ladi
bekor qilish do'koni; Xom 74 bayt PoW chiptalari imzolangan chipta tekshiruvchisi bo'lganda haqiqiy bo'lib qoladi
tuzilgan. Imzolovchi sirini `--signed-ticket-secret-hex` yoki orqali o'tkazing
Puzzle xizmatini ishga tushirishda `--signed-ticket-secret-path`; ishga tushirish mos kelmasligini rad etadi
agar sir `pow.signed_ticket_public_key_hex` ga qarshi tasdiqlanmasa, kalit juftlari.
`POST /v2/puzzle/mint` `"signed": true` (va ixtiyoriy `"transcript_hash_hex"`) ni qabul qiladi
Xom chipta baytlari bilan birga Norito kodli imzolangan chiptani qaytaring; javoblar kiradi
`signed_ticket_b64` va `signed_ticket_fingerprint_hex` barmoq izlarini takrorlashni kuzatishga yordam beradi.
Imzolovchi siri sozlanmagan boʻlsa, `signed = true` soʻrovlari rad etiladi.

## Kalitlarni aylantirish o'yin kitobi

1. **Yangi deskriptor majburiyatini to'plang.** Boshqaruv estafetani e'lon qiladi
   katalog to'plamida tavsiflovchi majburiyat. Olti burchakli satrdan nusxa ko'chiring
   `handshake.descriptor_commit_hex` o'rni ichida JSON konfiguratsiyasi ulashildi
   jumboq xizmati bilan.
2. **Pazzle siyosati chegaralarini ko‘rib chiqing.** Yangilanganni tasdiqlang
   `pow.puzzle.{memory_kib,time_cost,lanes}` qiymatlari nashrga mos keladi
   reja. Operatorlar Argon2 konfiguratsiyasini deterministik bo'ylab saqlashlari kerak
   o'rni (minimal 4MiB xotira, 1≤lanes≤16).
3. **Qayta ishga tushirishni bosqichma-bosqich o'tkazing.** Boshqaruvdan so'ng tizim blokini yoki konteynerni qayta yuklang
   aylanishni kesish haqida e'lon qiladi. Xizmat issiq qayta yuklashni qo'llab-quvvatlamaydi; a
   yangi deskriptor majburiyatini olish uchun qayta ishga tushirish talab qilinadi.
4. **Tasdiqlash.** `POST /v2/puzzle/mint` orqali chipta chiqaring va tasdiqlang
   qaytarilgan `difficulty` va `expires_at` yangi siyosatga mos keladi. Sovutish hisoboti
   (`docs/source/soranet/reports/pow_resilience.md`) kutilgan kechikishni ushlaydi
   ma'lumot uchun chegaralar. Tokenlar yoqilganda, `/v2/token/config` ni oling
   e'lon qilingan emitentning barmoq izlari va bekor qilish soni mos kelishiga ishonch hosil qiling
   kutilgan qiymatlar.

## Favqulodda o'chirish tartibi

1. Umumiy o'rni konfiguratsiyasida `pow.puzzle.enabled = false` ni o'rnating. Saqlash
   `pow.required = true`, agar xeshkash chiptalari majburiy qolishi kerak.
2. `pow.emergency` yozuvlarini ixtiyoriy ravishda eskirgan deskriptorlarni rad etish uchun kiriting.
   Argon2 darvozasi oflayn.
3. O'zgartirishni qo'llash uchun releyni ham, jumboq xizmatini ham qayta ishga tushiring.
4. Qiyinchilikni kamaytirish uchun `soranet_handshake_pow_difficulty` ni monitor qiling
   kutilgan hashcash qiymati va `/v2/puzzle/config` hisobotlarini tekshiring
   `puzzle = null`.

## Monitoring va ogohlantirish

- **Kutilish muddati:** `soranet_handshake_latency_seconds`-ni kuzatib boring va P95-ni saqlang
  300 ms dan past. Namlash testi ofsetlari qo'riqchi uchun kalibrlash ma'lumotlarini beradi
  throttles.【docs/source/soranet/reports/pow_resilience.md:1】
- **Kvota bosimi:** `soranet_guard_capacity_report.py` o'rni ko'rsatkichlari bilan foydalaning
  `pow.quotas` sovutish vaqtini sozlash uchun (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Bulmacani tekislash:** `soranet_handshake_pow_difficulty` mos kelishi kerak
  qiyinchilik `/v2/puzzle/config` tomonidan qaytarildi. Divergentsiya eskirgan releyni ko'rsatadi
  konfiguratsiya yoki muvaffaqiyatsiz qayta ishga tushirish.
- **Token tayyorligi:** `/v2/token/config` `enabled = false` ga tushsa, ogohlantirish
  kutilmaganda yoki `revocation_source` eskirgan vaqt belgilari haqida xabar bersa. Operatorlar
  Agar token bo'lsa, Norito bekor qilish faylini CLI orqali aylantirishi kerak
  bu yakuniy nuqtani aniq saqlash uchun nafaqaga chiqqan.
- **Xizmat salomatligi:** `/healthz` probini odatdagi jonli kadans va ogohlantirishda
  agar `/v2/puzzle/mint` HTTP 500 javoblarini qaytarsa (Argon2 parametrini bildiradi)
  mos kelmasligi yoki RNG xatosi). Token zarb qilish xatolari HTTP 4xx/5xx orqali yuzaga keladi
  `/v2/token/mint` bo'yicha javoblar; takroriy nosozliklarni peyjing holati sifatida ko'rib chiqing.

## Muvofiqlik va audit jurnali

O'rni gaz kelebeği sabablarini o'z ichiga olgan tuzilgan `handshake` hodisalarini chiqaradi.
sovutish muddatlari. Quvur liniyasida tavsiflangan muvofiqligini ta'minlang
`docs/source/soranet/relay_audit_pipeline.md` bu jurnallarni yutadi, shuning uchun jumboq
siyosatdagi o'zgarishlar tekshirilishi mumkin. Jumboq darvozasi yoqilganda, arxivlang
zarb qilingan chipta namunalari va Norito konfiguratsiya snapshoti
kelajakdagi audit uchun chipta. Qabul tokenlari texnik xizmat ko'rsatish oynalari oldidan zarb qilingan
ularning `token_id_hex` qiymatlari bilan kuzatilishi va ichiga kiritilishi kerak
bekor qilish fayli muddati tugashi yoki bekor qilinganidan keyin.