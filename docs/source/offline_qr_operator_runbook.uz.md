<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## Oflayn QR Operator Runbook

Ushbu runbook kamera shovqinli uchun amaliy `ecc`/dimension/fps sozlamalarini belgilaydi.
oflayn QR transportidan foydalanganda muhitlar.

### Tavsiya etilgan oldindan sozlashlar

| Atrof-muhit | Uslub | ECC | Hajmi | FPS | Bo'lak hajmi | Paritet guruhi | Eslatmalar |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Boshqariladigan yoritish, qisqa masofa | `sakura` | `M` | `360` | `12` | `360` | `0` | Eng yuqori o'tkazuvchanlik, minimal ortiqcha. |
| Odatiy mobil kamera shovqini | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | Aralash qurilmalar uchun afzal qilingan muvozanatli oldindan o'rnatish (`~3 KB/s`). |
| Yuqori porlash, harakatni xiralashtirish, past darajali kameralar | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | Pastroq o'tkazuvchanlik, eng kuchli dekodlash chidamliligi. |

### Tekshirish ro'yxatini kodlash/dekodlash

1. Aniq transport tugmalari bilan kodlash.
2. Chiqarishdan oldin skaner-loop tasviri bilan tasdiqlang.
3. Oldindan ko'rish paritetini saqlash uchun SDK ijro etish yordamchilarida bir xil uslub profilini mahkamlang.

Misol:

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

### Skaner-loop tekshiruvi (sakura-bo'ron 3 KB/s profil)

Barcha suratga olish yo'llari bo'ylab bir xil transport profilidan foydalaning:

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

Tasdiqlash maqsadlari:- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- Android: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- Brauzer/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

Qabul qilish:

- Har bir paritet guruhiga bitta tushirilgan ma'lumot ramkasi bilan to'liq foydali yukni qayta tiklash muvaffaqiyatli bo'ladi.
- Oddiy suratga olish siklida nazorat summasi/payload-xesh mos kelmasligi.