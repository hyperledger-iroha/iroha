<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
lang: hy
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## Անցանց QR օպերատորի Runbook

Այս գրքույկը սահմանում է `ecc`/չափ/fps գործնական նախադրյալներ ֆոտոխցիկի աղմուկի համար
միջավայրեր, երբ օգտագործում եք անցանց QR տրանսպորտ:

### Առաջարկվող նախադրյալներ

| Շրջակա միջավայր | Ոճ | ECC | Չափը | FPS | Կտորի չափը | Պարիտետային խումբ | Ծանոթագրություններ |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Կարգավորվող լուսավորություն, կարճ հեռահար | `sakura` | `M` | `360` | `12` | `360` | `0` | Ամենաբարձր թողունակությունը, նվազագույն ավելորդությունը: |
| Բջջային տեսախցիկի բնորոշ աղմուկ | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | Նախընտրելի հավասարակշռված նախադրյալ (`~3 KB/s`) խառը սարքերի համար: |
| Բարձր փայլ, շարժման պղտորում, ցածրորակ տեսախցիկներ | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | Ավելի ցածր թողունակություն, ամենաուժեղ վերծանման ճկունություն: |

### Կոդավորել/վերծանել ստուգաթերթը

1. Կոդավորել բացահայտ տրանսպորտային կոճակներով:
2. Վավերացնել սկաներ-ցիկլի նկարահանմամբ նախքան թողարկումը:
3. Ամրացրեք նույն ոճի պրոֆիլը SDK նվագարկման օգնականներում՝ նախադիտման հավասարությունը պահպանելու համար:

Օրինակ՝

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

### Սկաների հանգույցի վավերացում (sakura-storm 3 ԿԲ/վ պրոֆիլ)

Օգտագործեք նույն տրանսպորտային պրոֆիլը նկարահանման բոլոր ուղիներում.

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

Վավերացման թիրախներ.- iOS՝ `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- Android՝ `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- Զննարկիչ/JS՝ `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

Ընդունում:

- Ամբողջական բեռնվածքի վերակառուցումը հաջողվում է մեկ անկված տվյալների շրջանակով յուրաքանչյուր հավասարության խմբի համար:
- Չկան ստուգման գումար/վճարելի բեռ-հեշ անհամապատասխանություններ նորմալ գրավման հանգույցում: