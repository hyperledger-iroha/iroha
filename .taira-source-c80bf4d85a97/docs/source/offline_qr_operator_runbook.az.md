<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## Offline QR Operator Runbook

Bu runbook kamera səs-küylü üçün praktik `ecc`/dimension/fps əvvəlcədən təyinatlarını müəyyən edir.
oflayn QR nəqliyyatından istifadə edərkən mühitlər.

### Tövsiyə olunan ilkin parametrlər

| Ətraf mühit | Stil | ECC | Ölçü | FPS | Parça ölçüsü | Paritet qrupu | Qeydlər |
| --- | --- | --- | --- | --- | --- | --- | --- |
| İdarə olunan işıqlandırma, qısa məsafəli | `sakura` | `M` | `360` | `12` | `360` | `0` | Ən yüksək ötürmə qabiliyyəti, minimum ehtiyat. |
| Tipik mobil kamera səs-küyü | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | Qarışıq cihazlar üçün üstünlük verilən balanslaşdırılmış əvvəlcədən təyin (`~3 KB/s`). |
| Yüksək parıltı, hərəkət bulanıqlığı, aşağı səviyyəli kameralar | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | Aşağı ötürmə qabiliyyəti, ən güclü deşifrə dayanıqlığı. |

### Yoxlama siyahısını şifrələyin/şifrəni açın

1. Aydın nəqliyyat düymələri ilə kodlayın.
2. Yayımlanmazdan əvvəl skaner-döngü çəkmə ilə təsdiqləyin.
3. Önizləmə paritetini saxlamaq üçün eyni stil profilini SDK oxutma köməkçilərində bərkidin.

Misal:

```bash
iroha offline qr encode \
  --style sakura-storm \
  --ecc Q \
  --dimension 512 \
  --fps 12 \
  --chunk-size 336 \
  --parity-group 4 \
  --in payload.bin \
  --out out_dir
```

### Skaner-dövrə doğrulaması (sakura-fırtına 3 KB/s profil)

Bütün tutma yollarında eyni nəqliyyat profilindən istifadə edin:

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

Doğrulama hədəfləri:- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- Android: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- Brauzer/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

Qəbul:

- Tam faydalı yükün yenidən qurulması paritet qrupu başına bir məlumat çərçivəsi düşməklə uğur qazanır.
- Normal tutma dövrəsində yoxlama məbləği/faydalı yük-hesh uyğunsuzluğu yoxdur.