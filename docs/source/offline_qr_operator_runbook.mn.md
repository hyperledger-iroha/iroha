<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## Офлайн QR Operator Runbook

Энэхүү runbook нь дуу чимээ ихтэй камерт зориулсан практик `ecc`/dimension/fps урьдчилсан тохиргоог тодорхойлдог.
офлайн QR тээвэрлэлтийг ашиглах үед орчин.

### Санал болгож буй урьдчилан тохируулгууд

| Байгаль орчин | Style | ECC | Хэмжээ | FPS | Хэмжээний хэмжээ | Паритын бүлэг | Тэмдэглэл |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Хяналттай гэрэлтүүлэг, богино зайн | `sakura` | `M` | `360` | `12` | `360` | `0` | Хамгийн их дамжуулах чадвар, хамгийн бага нөөц. |
| Ердийн хөдөлгөөнт камерын дуу чимээ | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | Холимог төхөөрөмжүүдийн хувьд тэнцвэртэй урьдчилан тохируулсан (`~3 KB/s`). |
| Өндөр хурц гэрэл, хөдөлгөөнийг бүдгэрүүлдэг, бага зэрэглэлийн камер | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | Дамжуулах чадвар бага, код тайлах хамгийн хүчтэй. |

### Шалгах хуудсыг кодлох/тайлах

1. Илэрхий зөөвөрлөх товчлууруудаар кодчил.
2. Дамжуулахын өмнө сканнерийн давталтаар баталгаажуулах.
3. Урьдчилан үзэх тэгш байдлыг хадгалахын тулд SDK тоглуулах туслахуудад ижил загварын профайлыг зүү.

Жишээ:

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

### Сканнерын давталтын баталгаажуулалт (сакура шуурга 3 КБ/с профайл)

Бүх зураг авалтын замд ижил тээвэрлэлтийн профайлыг ашиглана уу:

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

Баталгаажуулах зорилтууд:- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- Android: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- Хөтөч/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

Хүлээн авах:

- Паритын бүлэгт нэг өгөгдлийн хүрээ хасагдсанаар бүрэн ачааллыг сэргээн босгох нь амжилттай болно.
- Хэвийн зураг авалтын гогцоонд шалгах нийлбэр/ачааны ачаалал-хэш таарахгүй байна.