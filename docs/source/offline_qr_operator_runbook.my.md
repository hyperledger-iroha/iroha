<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## အော့ဖ်လိုင်း QR အော်ပရေတာ Runbook

ဤ runbook သည် ကင်မရာ-ဆူညံမှုအတွက် လက်တွေ့ကျသော `ecc`/dimension/fps ကြိုတင်သတ်မှတ်မှုများကို သတ်မှတ်သည်
အော့ဖ်လိုင်း QR သယ်ယူပို့ဆောင်ရေးကို အသုံးပြုသည့်အခါ ပတ်ဝန်းကျင်။

### ကြိုတင်သတ်မှတ်မှုများကို အကြံပြုထားသည်။

| ပတ်ဝန်းကျင် | ပုံစံ | ECC | Dimension | FPS | အတုံးအရွယ်အစား | Parity အဖွဲ့ | မှတ်စုများ |
| ---| ---| ---| ---| ---| ---| ---| ---|
| ထိန်းချုပ်ထားသောအလင်းရောင်၊ တိုတောင်းသောအကွာအဝေး | `sakura` | `M` | `360` | `12` | `360` | `0` | အမြင့်ဆုံးသော သွင်းအားစု၊ အနည်းအကျဉ်းသာ ထပ်နေပါသည်။ |
| ပုံမှန်မိုဘိုင်းကင်မရာဆူညံသံ | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | ရောနှောထားသော စက်များအတွက် ဦးစားပေး မျှတသော ကြိုတင်သတ်မှတ်မှု (`~3 KB/s`)။ |
| အလင်းပြန်မှု မြင့်မားခြင်း၊ လှုပ်ရှားမှု မှုန်ဝါးခြင်း၊ အနိမ့်ဆုံး ကင်မရာများ | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | အနိမ့်ဆုံး ဖြတ်သန်းမှု ၊ အပြင်းထန်ဆုံး ကုဒ် ခံနိုင်ရည်ရှိမှု ။ |

### ကုဒ်/ကုဒ် စစ်ဆေးမှုစာရင်း

1. တိကျသောသယ်ယူပို့ဆောင်ရေးခလုတ်များဖြင့် ကုဒ်လုပ်ပါ။
2. စတင်မဖြန့်ချိမီ စကင်နာ-ကွင်းပတ်ဖမ်းယူမှုဖြင့် အတည်ပြုစစ်ဆေးပါ။
3. တူညီသောပုံစံပရိုဖိုင်ကို အစမ်းကြည့်ရှုရန် တူညီမှုကို ဆက်လက်ထားရှိရန် SDK ဖွင့်ရန် အကူအညီပေးသူများတွင် ပင်ထိုးပါ။

ဥပမာ-

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

### စကန်နာ-ကွင်းဆက်စစ်ဆေးခြင်း (sakura-မုန်တိုင်း 3 KB/s ပရိုဖိုင်)

ရိုက်ကူးရေးလမ်းကြောင်းများအားလုံးတွင် တူညီသောသယ်ယူပို့ဆောင်ရေးပရိုဖိုင်ကို အသုံးပြုပါ-

- `chunk_size=336`
- `parity_group=4`
- `fps=12`
- `style=sakura-storm`

အတည်ပြုခြင်းပစ်မှတ်များ-- iOS : `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
- Android : `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- ဘရောက်ဆာ/JS- `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

လက်ခံမှု-

- ပါရီအုပ်စုတစ်ခုစီတွင် ကျဆင်းသွားသော ဒေတာဘောင်တစ်ခုဖြင့် payload ပြန်လည်တည်ဆောက်မှု အပြည့်အဝအောင်မြင်သည်။
- ပုံမှန်ဖမ်းယူသည့်ကွင်းတွင် checksum/payload-hash တူညီမှုမရှိပါ။