<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## ოფლაინ QR ოპერატორის Runbook

ეს სახელმძღვანელო განსაზღვრავს პრაქტიკულ `ecc`/განზომილებას/fps წინასწარ დაყენებებს ხმაურიანი კამერისთვის
გარემო ოფლაინ QR ტრანსპორტის გამოყენებისას.

### რეკომენდებული წინასწარ დაყენებები

| გარემო | სტილი | ECC | განზომილება | FPS | ნაჭრის ზომა | პარიტეტული ჯგუფი | შენიშვნები |
| --- | --- | --- | --- | --- | --- | --- | --- |
| კონტროლირებადი განათება, მოკლე დიაპაზონი | `sakura` | `M` | `360` | `12` | `360` | `0` | უმაღლესი გამტარუნარიანობა, მინიმალური სიჭარბე. |
| ტიპიური მობილური კამერის ხმაური | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | სასურველი დაბალანსებული წინასწარ დაყენება (`~3 KB/s`) შერეული მოწყობილობებისთვის. |
| მაღალი სიკაშკაშე, მოძრაობის დაბინდვა, დაბალი დონის კამერები | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | დაბალი გამტარუნარიანობა, ყველაზე ძლიერი დეკოდირების ელასტიურობა. |

### დაშიფვრა/გაშიფვრა საკონტროლო სია

1. კოდირება მკაფიო სატრანსპორტო ღილაკებით.
2. ვალიდაცია სკანერის მარყუჟის დაჭერით გაშვებამდე.
3. ჩაამაგრეთ იგივე სტილის პროფილი SDK დაკვრის დამხმარეებში, რათა შეინარჩუნოთ გადახედვის პარიტეტი.

მაგალითი:

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

### სკანერის მარყუჟის ვალიდაცია (sakura-storm 3 KB/s პროფილი)

გამოიყენეთ ერთი და იგივე სატრანსპორტო პროფილი გადაღების ყველა გზაზე:

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

ვალიდაციის მიზნები:- iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- Android: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- ბრაუზერი/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

მიღება:

- სრული დატვირთვის რეკონსტრუქცია წარმატებულია ერთი ამოვარდნილი მონაცემთა ჩარჩოთი თითო პარიტეტული ჯგუფისთვის.
- არ არის საკონტროლო ჯამი/გამტარი დატვირთვა-ჰეშის შეუსაბამობები ნორმალურ დაჭერის ციკლში.