---
lang: az
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito-RPC Overview
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC İcmal

Norito-RPC Torii API-ləri üçün ikili nəqliyyatdır. Eyni HTTP yollarını təkrar istifadə edir
`/v2/pipeline` kimi, lakin sxem daxil olan Norito çərçivəli faydalı yükləri mübadilə edir
hash və yoxlama məbləğləri. Deterministik, təsdiqlənmiş cavablara ehtiyacınız olduqda istifadə edin və ya
boru kəmərinin JSON cavabları darboğaza çevrildikdə.

## Niyə keçid?
- CRC64 və şema hashləri ilə deterministik çərçivələmə dekodlaşdırma xətalarını azaldır.
- SDK-larda paylaşılan Norito köməkçiləri mövcud məlumat modeli növlərindən təkrar istifadə etməyə imkan verir.
- Torii telemetriyada artıq Norito seanslarını qeyd edir, beləliklə operatorlar nəzarət edə bilər
təmin edilmiş tablosuna uyğun qəbul edilməsi.

## Müraciət etmək

```bash
curl \
  -H 'Content-Type: application/x-norito' \
  -H 'Accept: application/x-norito' \
  -H "Authorization: Bearer ${TOKEN}" \
  --data-binary @signed_transaction.norito \
  https://torii.devnet.sora.example/v2/transactions/submit
```

1. Yükünüzü Norito kodek (`iroha_client`, SDK köməkçiləri və ya) ilə seriyalaşdırın
   `norito::to_bytes`).
2. Sorğunu `Content-Type: application/x-norito` ilə göndərin.
3. `Accept: application/x-norito` istifadə edərək Norito cavabını tələb edin.
4. Uyğun SDK köməkçisindən istifadə edərək cavabı deşifrə edin.

SDK üçün xüsusi təlimat:
- **Rust**: Siz təyin etdiyiniz zaman `iroha_client::Client` avtomatik olaraq Norito ilə danışıqlar aparır
  `Accept` başlığı.
- **Python**: `iroha_python.norito_rpc`-dən `NoritoRpcClient` istifadə edin.
- **Android**: `NoritoRpcClient` və `NoritoRpcRequestOptions` istifadə edin
  Android SDK.
- **JavaScript/Swift**: köməkçilər `docs/source/torii/norito_rpc_tracker.md`-də izlənilir
  və NRPC-3-ün bir hissəsi kimi yerə enəcək.

## Sınaq Konsol nümunəsi

Tərtibatçı portalı "Try It" proksisini göndərir ki, rəyçilər Norito-i təkrar oxuya bilsinlər.
sifarişli skriptlər yazmadan faydalı yüklər.

1. [Proksi işə salın](./try-it.md#start-the-proxy-locally) və təyin edin
   `TRYIT_PROXY_PUBLIC_URL` beləliklə, vidjetlər trafikin hara göndəriləcəyini bilir.
2. Bu səhifədə **Sınaq** kartını və ya `/reference/torii-swagger` açın
   paneli seçin və `POST /v2/pipeline/submit` kimi son nöqtəni seçin.
3. **Məzmun növü**-ni `application/x-norito`-ə dəyişin, **İkili** seçin
   redaktoru və `fixtures/norito_rpc/transfer_asset.norito` yükləyin
   (və ya siyahıda göstərilən hər hansı faydalı yük
   `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. OAuth cihaz kodu vidceti və ya əl nişanı vasitəsilə daşıyıcı nişanı təmin edin
   sahəsi (proxy ilə konfiqurasiya edildikdə `X-TryIt-Auth` ləğvetmələrini qəbul edir
   `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Sorğunu göndərin və Torii-in siyahıda sadalanan `schema_hash` ilə səsləşdiyini yoxlayın.
   `fixtures/norito_rpc/schema_hashes.json`. Uyğun hashlər bunu təsdiqləyir
   Norito başlığı brauzerdən/proksi hopdan xilas oldu.

Yol xəritəsinin sübutu üçün Sınaq skrinşotunu bir qaçışla birləşdirin
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Ssenari sarılır
`cargo xtask norito-rpc-verify`, JSON xülasəsini yazır
`artifacts/norito_rpc/<timestamp>/` və eyni qurğuları çəkir
portal istehlak edilmişdir.

## Problemlərin aradan qaldırılması

| Simptom | Göründüyü yer | Ehtimal olunan səbəb | Düzəlt |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii cavab | Çatışmayan və ya səhv `Content-Type` başlığı | Yükü göndərməzdən əvvəl `Content-Type: application/x-norito` təyin edin. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii cavab gövdəsi/başlıqları | Quraşdırma sxemi hash Torii quruluşundan fərqlənir | `cargo xtask norito-rpc-fixtures` ilə qurğuları bərpa edin və `fixtures/norito_rpc/schema_hashes.json`-də hashı təsdiqləyin; son nöqtə hələ Norito-i aktivləşdirməyibsə, JSON-a qayıdın. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Proksi cavabını sınayın | Sorğu `TRYIT_PROXY_ALLOWED_ORIGINS` | siyahısında olmayan mənbədən gəldi Portal mənşəyini (məsələn, `https://docs.devnet.sora.example`) env var-a əlavə edin və proxy-ni yenidən başladın. |
| `{"error":"rate_limited"}` (HTTP 429) | Proksi cavabını sınayın | IP başına kvota `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` büdcəsini keçdi | Daxili yük testi üçün limiti artırın və ya pəncərə sıfırlanana qədər gözləyin (JSON cavabında `retryAfterMs`-ə baxın). |
| `{"error":"upstream_timeout"}` (HTTP 504) və ya `{"error":"upstream_error"}` (HTTP 502) | Proksi cavabını sınayın | Torii vaxtı bitdi və ya proksi konfiqurasiya edilmiş arxa hissəyə çata bilmədi | `TRYIT_PROXY_TARGET`-in əlçatan olduğunu yoxlayın, Torii sağlamlığını yoxlayın və ya daha böyük `TRYIT_PROXY_TIMEOUT_MS` ilə yenidən cəhd edin. |

Daha çox Sınaq diaqnostikası və OAuth məsləhətləri mövcuddur
[`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Əlavə resurslar
- Nəqliyyat RFC: `docs/source/torii/norito_rpc.md`
- İcra xülasəsi: `docs/source/torii/norito_rpc_brief.md`
- Fəaliyyət izləyicisi: `docs/source/torii/norito_rpc_tracker.md`
- Sınaq proksi təlimatları: `docs/portal/docs/devportal/try-it.md`