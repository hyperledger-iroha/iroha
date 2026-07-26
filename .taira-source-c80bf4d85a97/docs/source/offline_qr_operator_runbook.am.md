<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## ከመስመር ውጭ QR ኦፕሬተር Runbook

ይህ Runbook ለካሜራ-ጫጫታ ተግባራዊ `ecc`/dimension/fps ቅድመ-ቅምጦችን ይገልጻል።
ከመስመር ውጭ QR መጓጓዣን በሚጠቀሙበት ጊዜ አከባቢዎች።

### የተመከሩ ቅድመ-ቅምጦች

| አካባቢ | ቅጥ | ECC | ልኬት | FPS | ቁራጭ መጠን | የተመጣጣኝ ቡድን | ማስታወሻ |
| --- | --- | --- | --- | --- | --- | --- | --- |
| ቁጥጥር የሚደረግበት ብርሃን፣ አጭር ክልል | `sakura` | `M` | `360` | `12` | `360` | `0` | ከፍተኛው ልቀት፣ አነስተኛ ድግግሞሽ። |
| የተለመደ የሞባይል ካሜራ ጫጫታ | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | ተመራጭ የተመጣጠነ ቅድመ ዝግጅት (`~3 KB/s`) ለተቀላቀሉ መሳሪያዎች። |
| ከፍተኛ አንጸባራቂ፣ የእንቅስቃሴ ብዥታ፣ ዝቅተኛ-መጨረሻ ካሜራዎች | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | ዝቅተኛ የመተላለፊያ ይዘት፣ በጣም ጠንካራው የመቋቋም አቅም መፍታት። |

### ማመሳከሪያ/መግለጫ ዝርዝር

1. ግልጽ በሆነ የማጓጓዣ ቁልፎች መመስጠር።
2. ከመልቀቅዎ በፊት በስካነር-loop ቀረጻ ያረጋግጡ።
3. የቅድመ እይታን እኩልነት ለመጠበቅ በኤስዲኬ መልሶ ማጫወት አጋዥ ውስጥ ተመሳሳይ የቅጥ መገለጫን ይሰኩ።

ምሳሌ፡-

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

### ስካነር-ሉፕ ማረጋገጫ (sakura-storm 3KB/s profile)

በሁሉም የመያዣ መንገዶች ላይ ተመሳሳይ የትራንስፖርት መገለጫ ይጠቀሙ፡-

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

የማረጋገጫ ዒላማዎች፡-- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- አንድሮይድ፡ `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- አሳሽ/JS፡ `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

ተቀባይነት፡-

- ሙሉ ክፍያ መልሶ መገንባት በአንድ የወደቀ የውሂብ ፍሬም በአንድ ተመሳሳይ ቡድን ተሳክቷል።
- ምንም የቼክ ክፍያ/የክፍያ-ሃሽ አለመዛመድ በተለመደው የቀረጻ ዑደት ውስጥ የለም።