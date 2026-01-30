---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: signing-ceremony
title: دستخطی تقریب کی جگہ نیا عمل
description: Sora پارلیمنٹ کس طرح SoraFS chunker fixtures کی منظوری اور تقسیم کرتی ہے (SF-1b).
sidebar_label: دستخطی تقریب
---

> Roadmap: **SF-1b — Sora Parliament fixture approvals.**
> پارلیمنٹ کا ورک فلو پرانی آف لائن "کونسل دستخطی تقریب" کی جگہ لیتا ہے۔

SoraFS chunker fixtures کے لیے دستی دستخطی رسم ریٹائر کر دی گئی ہے۔ اب تمام منظوری
**Sora Parliament** کے ذریعے ہوتی ہے، جو Nexus کو گورن کرنے والی sortition-based DAO ہے۔
پارلیمنٹ کے اراکین شہری بننے کے لیے XOR bond کرتے ہیں، پینلز کے درمیان گردش کرتے ہیں،
اور on-chain ووٹس کے ذریعے fixtures ریلیزز کو منظور، مسترد یا رول بیک کرتے ہیں۔
یہ گائیڈ عمل اور developer tooling کی وضاحت کرتی ہے۔

## پارلیمنٹ کا جائزہ

- **شہریت** — آپریٹرز مطلوبہ XOR bond کر کے شہری بنتے ہیں اور sortition کے اہل ہوتے ہیں۔
- **پینلز** — ذمہ داریاں گردش کرنے والے پینلز میں تقسیم ہیں (Infrastructure,
  Moderation, Treasury, ...). Infrastructure Panel SoraFS fixture approvals کا ذمہ دار ہے۔
- **Sortition اور rotation** — پینل سیٹس پارلیمنٹ دستور میں متعین cadence پر دوبارہ
  قرعہ اندازی سے منتخب ہوتے ہیں تاکہ کوئی ایک گروہ منظوریوں پر اجارہ داری نہ رکھ سکے۔

## Fixture approval flow

1. **Proposal submission**
   - Tooling WG امیدوار `manifest_blake3.json` bundle اور fixture diff کو `sorafs.fixtureProposal`
     کے ذریعے on-chain registry میں اپلوڈ کرتا ہے۔
   - پروپوزل BLAKE3 digest، semantic version اور تبدیلی نوٹس ریکارڈ کرتا ہے۔
2. **Review & voting**
   - Infrastructure Panel پارلیمنٹ task queue کے ذریعے اسائنمنٹ وصول کرتا ہے۔
   - پینل ممبرز CI artefacts دیکھتے ہیں، parity tests چلاتے ہیں، اور on-chain weighted votes ڈالتے ہیں۔
3. **Finalisation**
   - جب quorum پورا ہو جائے تو runtime ایک approval event جاری کرتا ہے جس میں canonical manifest digest
     اور fixture payload کے لیے Merkle commitment شامل ہوتا ہے۔
   - یہ event SoraFS registry میں mirror کیا جاتا ہے تاکہ کلائنٹس تازہ ترین Parliament-approved manifest حاصل کر سکیں۔
4. **Distribution**
   - CLI helpers (`cargo xtask sorafs-fetch-fixture`) Nexus RPC سے منظور شدہ manifest کھینچتے ہیں۔
     ریپو کے JSON/TS/Go constants `export_vectors` دوبارہ چلا کر اور digest کو on-chain ریکارڈ کے
     مقابل validate کر کے sync رہتے ہیں۔

## Developer workflow

- Fixtures دوبارہ بنائیں:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Parliament fetch helper استعمال کریں تاکہ منظور شدہ envelope ڈاؤن لوڈ ہو، signatures verify ہوں،
  اور مقامی fixtures refresh ہوں۔ `--signatures` کو Parliament کے شائع کردہ envelope پر پوائنٹ کریں؛
  helper متعلقہ manifest resolve کرتا ہے، BLAKE3 digest دوبارہ حساب کرتا ہے، اور canonical
  `sorafs.sf1@1.0.0` profile نافذ کرتا ہے۔

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

اگر manifest کسی اور URL پر ہو تو `--manifest` پاس کریں۔ غیر دستخط شدہ envelopes رد کر دیے جاتے ہیں
جب تک مقامی smoke runs کے لیے `--allow-unsigned` سیٹ نہ ہو۔

- Staging gateway کے ذریعے manifest validate کرنے کے لیے مقامی payloads کے بجائے Torii کو ہدف بنائیں:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- مقامی CI اب `signer.json` roster کا تقاضا نہیں کرتا۔
  `ci/check_sorafs_fixtures.sh` repo کی حالت کو تازہ ترین on-chain commitment سے موازنہ کرتا ہے اور
  فرق ہونے پر fail کر دیتا ہے۔

## Governance notes

- پارلیمنٹ دستور quorum، rotation اور escalation کو govern کرتا ہے — crate-level configuration درکار نہیں۔
- ہنگامی rollbacks پارلیمنٹ moderation panel کے ذریعے سنبھالے جاتے ہیں۔ Infrastructure Panel ایک revert
  proposal فائل کرتا ہے جو پچھلے manifest digest کو حوالہ دیتا ہے، اور منظوری کے بعد release بدل دی جاتی ہے۔
- تاریخی approvals SoraFS registry میں forensics replay کے لیے دستیاب رہتے ہیں۔

## FAQ

- **`signer.json` کہاں گیا؟**  
  اسے ہٹا دیا گیا ہے۔ تمام signer attribution on-chain موجود ہے؛ ریپو میں `manifest_signatures.json`
  صرف developer fixture ہے جو آخری approval event سے میچ ہونا چاہیے۔

- **کیا اب بھی مقامی Ed25519 signatures درکار ہیں؟**  
  نہیں۔ Parliament approvals on-chain artefacts کے طور پر محفوظ ہوتے ہیں۔ مقامی fixtures
  reproducibility کے لیے ہوتے ہیں مگر Parliament digest کے خلاف validate کیے جاتے ہیں۔

- **ٹیمیں approvals کیسے مانیٹر کرتی ہیں؟**  
  `ParliamentFixtureApproved` event کو subscribe کریں یا Nexus RPC کے ذریعے registry کو query کریں
  تاکہ موجودہ manifest digest اور panel roll call حاصل کیا جا سکے۔
