<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
lang: kk
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## Офлайн QR операторының Runbook

Бұл runbook шулы камера үшін практикалық `ecc`/өлшем/fps алдын ала орнатуларын анықтайды.
офлайн QR тасымалдауды пайдалану кезіндегі орталар.

### Ұсынылатын алдын ала орнатулар

| Қоршаған орта | Стиль | ECC | Өлшемі | FPS | Бөлшек өлшемі | Паритеттік топ | Ескертпелер |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Басқарылатын жарықтандыру, қысқа диапазондағы | `sakura` | `M` | `360` | `12` | `360` | `0` | Ең жоғары өткізу қабілеті, ең аз резервтеу. |
| Әдеттегі мобильді камера шуы | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | Аралас құрылғылар үшін теңдестірілген алдын ала орнату (`~3 KB/s`). |
| Жоғары жарқырау, қозғалыс бұлдыры, төмен деңгейлі камералар | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | Төмен өткізу қабілеті, ең күшті декодтау тұрақтылығы. |

### Тексеру тізімін кодтау/декодтау

1. Ашық тасымалдау тұтқаларымен кодтау.
2. Шығармас бұрын сканер циклін түсіру арқылы растаңыз.
3. Алдын ала қарау паритетін сақтау үшін SDK ойнату көмекшілерінде бірдей стиль профилін бекітіңіз.

Мысалы:

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

### Сканер циклінің валидациясы (сакура-дауыл 3 КБ/с профилі)

Барлық түсіру жолдарында бірдей тасымалдау профилін пайдаланыңыз:

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

Тексеру мақсаттары:- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- Android: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- Браузер/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

Қабылдау:

- Толық пайдалы жүктемені қайта құру паритет тобына бір түсірілген деректер кадрымен сәтті болады.
- Қалыпты түсіру циклінде бақылау сомасы/пайдалы жүктеме-хэш сәйкессіздіктері жоқ.