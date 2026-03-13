---
id: puzzle-service-operations
lang: az
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Puzzle Service Operations Guide
sidebar_label: Puzzle Service Ops
description: Operating the `soranet-puzzle-service` daemon for Argon2/ML-DSA admission tickets.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

# Puzzle Xidməti Əməliyyatları Bələdçisi

`soranet-puzzle-service` demonu (`tools/soranet-puzzle-service/`) problemləri
Rölenin `pow.puzzle.*` siyasətini əks etdirən Argon2 tərəfindən dəstəklənən qəbul biletləri
və konfiqurasiya edildikdə, kənar relelər adından brokerlər ML-DSA qəbul nişanları.
O, beş HTTP son nöqtəsini ifşa edir:

- `GET /healthz` – canlılıq zondu.
- `GET /v2/puzzle/config` – çəkilmiş effektiv PoW/pazl parametrlərini qaytarır
  JSON relesindən (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v2/puzzle/mint` – Arqon2 bileti zərb edir; isteğe bağlı JSON orqanı
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  daha qısa TTL tələb edir (siyasət pəncərəsinə bərkidilir), bileti a ilə bağlayır
  transkript hash və relay imzalı bilet + imza barmaq izini qaytarır
  imzalama açarları konfiqurasiya edildikdə.
- `GET /v2/token/config` – `pow.token.enabled = true`, aktivi qaytardıqda
  Qəbul nişanı siyasəti (emitentin barmaq izi, TTL/saat əyri sərhədləri, relay ID,
  və birləşdirilmiş ləğv dəsti).
- `POST /v2/token/mint` - təchiz edilmiş ML-DSA qəbul nişanını çıxarır
  resume hash; sorğu orqanı `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }` qəbul edir.

Xidmət tərəfindən istehsal edilən biletlər təsdiqlənir
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`
inteqrasiya testi, bu da həcmli DoS zamanı reley tənzimləmələrini həyata keçirir
ssenarilər.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Token buraxılışının konfiqurasiyası

`pow.token.*` altında relay JSON sahələrini təyin edin (bax
Məsələn, `tools/soranet-relay/deploy/config/relay.entry.json`) aktivləşdirmək üçün
ML-DSA tokenləri. Ən azı emitentə açıq açarı təqdim edin və isteğe bağlı
ləğv siyahısı:

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

Puzzle xidməti bu dəyərlərdən təkrar istifadə edir və avtomatik olaraq Norito faylını yenidən yükləyir.
İcra zamanı JSON ləğv faylı. `soranet-admission-token` CLI istifadə edin
(`cargo run -p soranet-relay --bin soranet_admission_token`) zərb etmək və yoxlamaq
tokenləri oflayn edin, ləğvetmə faylına `token_id_hex` qeydlərini əlavə edin və yoxlayın
istehsala yeniləmələri təkan verməzdən əvvəl mövcud etimadnamələri.

Emitentin məxfi açarını CLI bayraqları vasitəsilə tapmaca xidmətinə ötürün:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex`, sirri diapazondan kənar bir şəxs tərəfindən idarə edildikdə də mövcuddur.
alət boru kəməri. Ləğv faylı izləyicisi `/v2/token/config` cari saxlayır;
geridə qalmamaq üçün yeniləmələri `soranet-admission-token revoke` əmri ilə əlaqələndirin
ləğv vəziyyəti.

ML-DSA-44 ictimaiyyətini reklam etmək üçün JSON relesində `pow.signed_ticket_public_key_hex` seçin
imzalanmış PoW biletlərini yoxlamaq üçün istifadə edilən açar; `/v2/puzzle/config` açarı və onun BLAKE3-ü əks etdirir
barmaq izi (`signed_ticket_public_key_fingerprint_hex`) beləliklə müştərilər doğrulayıcını pin edə bilsinlər.
İmzalanmış biletlər relay ID-si və transkript bağlamaları ilə təsdiqlənir və eyni şəkildə paylaşılır
geri götürmə mağazası; xam 74 baytlıq PoW biletləri imzalanmış bilet yoxlayıcısı olduqda etibarlı qalır
konfiqurasiya edilmişdir. İmzalayan sirrini `--signed-ticket-secret-hex` və ya vasitəsilə ötürün
Puzzle xidmətini işə salarkən `--signed-ticket-secret-path`; başlanğıc uyğunsuzluğu rədd edir
sirr `pow.signed_ticket_public_key_hex`-ə qarşı doğrulanmazsa, açar cütləri.
`POST /v2/puzzle/mint` `"signed": true` (və isteğe bağlı `"transcript_hash_hex"`) qəbul edir
xam bilet baytları ilə yanaşı Norito kodlu imzalanmış bileti qaytarın; cavablar daxildir
Barmaq izlərini təkrar izləməyə kömək etmək üçün `signed_ticket_b64` və `signed_ticket_fingerprint_hex`.
İmzalayan sirri konfiqurasiya edilmədikdə, `signed = true` ilə sorğular rədd edilir.

## Açar fırlanma oyun kitabı

1. **Yeni deskriptor öhdəliyini toplayın.** İdarəetmə releyi dərc edir
   qovluq paketində deskriptor öhdəliyi. Hex sətrini kopyalayın
   `handshake.descriptor_commit_hex` relay daxilində JSON konfiqurasiyası paylaşıldı
   puzzle xidməti ilə.
2. **Tapmaca siyasətinin sərhədlərini nəzərdən keçirin.** Yenilənmişi təsdiq edin
   `pow.puzzle.{memory_kib,time_cost,lanes}` dəyərlər buraxılışa uyğundur
   plan. Operatorlar Argon2 konfiqurasiyasını deterministik şəkildə saxlamalıdırlar
   relelər (minimum 4MiB yaddaş, 1≤zolaq≤16).
3. **Yenidən işə salın.** İdarəetmədən sonra sistem vahidini və ya konteyneri yenidən yükləyin
   fırlanmanın kəsilməsini elan edir. Xidmətin isti yenidən yükləmə dəstəyi yoxdur; a
   Yeni deskriptor öhdəliyini götürmək üçün yenidən başlama tələb olunur.
4. **Validate edin.** `POST /v2/puzzle/mint` vasitəsilə bilet verin və təsdiq edin
   qaytarılmış `difficulty` və `expires_at` yeni siyasətə uyğun gəlir. Islatma hesabatı
   (`docs/source/soranet/reports/pow_resilience.md`) gözlənilən gecikməni çəkir
   istinad üçün sərhədlər. Tokenlər aktivləşdirildikdə, `/v2/token/config`-i əldə edin
   reklam edilən emitentin barmaq izi və ləğvetmə sayının uyğun olduğundan əmin olun
   gözlənilən dəyərlər.

## Təcili söndürmə proseduru

1. Paylaşılan rele konfiqurasiyasında `pow.puzzle.enabled = false` təyin edin. Saxla
   `pow.required = true`, əgər hashcash ehtiyat biletləri məcburi olaraq qalmalıdır.
2. `pow.emergency` girişlərini istəyə uyğun olaraq köhnə deskriptorları rədd etmək üçün tətbiq edin
   Argon2 qapısı oflayndır.
3. Dəyişikliyi tətbiq etmək üçün həm releyi, həm də tapmaca xidmətini yenidən başladın.
4. Çətinliyin aşağı düşməsini təmin etmək üçün `soranet_handshake_pow_difficulty`-ə nəzarət edin
   gözlənilən hashcash dəyərini və `/v2/puzzle/config` hesabatlarını yoxlayın
   `puzzle = null`.

## Monitorinq və xəbərdarlıq

- **Gecikmə SLO:** `soranet_handshake_latency_seconds`-i izləyin və P95-i saxlayın
  300ms-dən aşağı. Islatma testi ofsetləri qoruyucu üçün kalibrləmə məlumatını təmin edir
  tənzimləyir.【docs/source/soranet/reports/pow_resilience.md:1】
- **Kvota təzyiqi:** Rele ölçüləri ilə `soranet_guard_capacity_report.py` istifadə edin
  `pow.quotas` soyutma müddətini tənzimləmək üçün (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Bulmacanın düzülməsi:** `soranet_handshake_pow_difficulty` uyğun olmalıdır
  çətinlik `/v2/puzzle/config` tərəfindən qaytarıldı. Divergensiya köhnəlmiş röleyi göstərir
  konfiqurasiya və ya uğursuz yenidən başladın.
- **Token hazırlığı:** `/v2/token/config` `enabled = false` səviyyəsinə düşərsə xəbərdarlıq
  gözlənilmədən və ya `revocation_source` köhnə vaxt ştamplarını bildirirsə. Operatorlar
  Token olduqda Norito ləğvetmə faylını CLI vasitəsilə çevirməlidir.
  bu son nöqtəni dəqiq saxlamaq üçün təqaüdə çıxdı.
- **Xidmət sağlamlığı:** `/healthz` zondunu adi canlılıq tempi və siqnalı ilə yoxlayın
  əgər `/v2/puzzle/mint` HTTP 500 cavablarını qaytarırsa (Argon2 parametrini göstərir
  uyğunsuzluq və ya RNG xətaları). Token zərb xətaları HTTP 4xx/5xx vasitəsilə görünür
  `/v2/token/mint`-də cavablar; təkrar uğursuzluqları peyqinq vəziyyəti kimi qəbul edin.

## Uyğunluq və audit qeydi

Röleler, tənzimləmə səbəbləri və daxil olmaqla strukturlaşdırılmış `handshake` hadisələri yayır.
soyutma müddətləri. bəndində təsvir olunan boru kəmərinin uyğunluğunu təmin edin
`docs/source/soranet/relay_audit_pipeline.md` bu qeydləri qəbul edir, o qədər də tapmacadır
siyasət dəyişiklikləri yoxlanıla bilər. Tapmaca qapısı işə salındıqda, arxivləşdirin
buraxılmış bilet nümunələri və Norito konfiqurasiya şəkli
gələcək auditlər üçün bilet. Təmir pəncərələrindən əvvəl zərb edilən qəbul nişanları
onların `token_id_hex` dəyərləri ilə izlənilməli və daxil edilməlidir
onların müddəti bitdikdən və ya ləğv edildikdən sonra ləğvetmə faylı.