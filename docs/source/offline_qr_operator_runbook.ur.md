<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## آف لائن QR آپریٹر رن بک

یہ رن بک کیمرے کے شور کے لیے عملی `ecc`/dimension/fps presets کی وضاحت کرتی ہے۔
آف لائن QR ٹرانسپورٹ استعمال کرتے وقت ماحول۔

### تجویز کردہ پیش سیٹ

| ماحولیات | انداز | ای سی سی | طول و عرض | FPS | ٹکڑا سائز | برابری گروپ | نوٹس |
| --- | --- | --- | --- | --- | --- | --- | --- |
| کنٹرول لائٹنگ، مختصر رینج | `sakura` | `M` | `360` | `12` | `360` | `0` | سب سے زیادہ تھرو پٹ، کم سے کم فالتو پن۔ |
| عام موبائل کیمرے کا شور | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | مخلوط آلات کے لیے ترجیحی متوازن پیش سیٹ (`~3 KB/s`)۔ |
| زیادہ چکاچوند، حرکت دھندلا، کم کے آخر والے کیمرے | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | کم تھرو پٹ، مضبوط ترین ڈی کوڈ لچک۔ |

### انکوڈ/ڈی کوڈ چیک لسٹ

1. واضح ٹرانسپورٹ نوبس کے ساتھ انکوڈ کریں۔
2. رول آؤٹ سے پہلے اسکینر لوپ کیپچر کے ساتھ تصدیق کریں۔
3. پیش نظارہ برابری برقرار رکھنے کے لیے SDK پلے بیک مددگاروں میں اسی طرز کے پروفائل کو پن کریں۔

مثال:

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

### سکینر لوپ کی توثیق (ساکورا طوفان 3 KB/s پروفائل)

تمام کیپچر راستوں پر ایک ہی ٹرانسپورٹ پروفائل کا استعمال کریں:

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

توثیق کے اہداف:- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- Android: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- براؤزر/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

قبولیت:

- مکمل پے لوڈ کی تعمیر نو فی برابری گروپ کے ایک گرائے گئے ڈیٹا فریم کے ساتھ کامیاب ہوتی ہے۔
- عام کیپچر لوپ میں کوئی چیکسم/پے لوڈ-ہیش مماثل نہیں ہے۔