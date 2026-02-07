---
lang: ur
direction: rtl
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T10:01:27.059307+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

٪ فاسٹ پی کیو ٹرانسفر گیجٹ ڈیزائن

# جائزہ

موجودہ فاسٹ پی کیو پلانر `TransferAsset` ہدایت میں شامل ہر قدیم آپریشن کو ریکارڈ کرتا ہے ، جس کا مطلب ہے کہ ہر منتقلی بیلنس ریاضی ، ہیش راؤنڈ ، اور ایس ایم ٹی کی تازہ کاریوں کے لئے الگ الگ ادائیگی کرتی ہے۔ ہر منتقلی کا ٹریس قطاروں کو کم کرنے کے ل we ہم ایک سرشار گیجٹ متعارف کراتے ہیں جو صرف کم سے کم ریاضی/عزم چیکوں کی تصدیق کرتا ہے جبکہ میزبان کیننیکل ریاست کی منتقلی پر عملدرآمد جاری رکھے ہوئے ہے۔

۔
۔

# فن تعمیر

```
Kotodama builder → Norito syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## ٹرانسکرپٹ فارمیٹ

میزبان ایک `TransferTranscript` فی syscall درخواست خارج کرتا ہے:

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- `batch_hash` ٹرانسکرپٹ کو ٹرانزیکشن انٹری پوائنٹ ہیش سے جوڑتا ہے جو ری پلے تحفظ کے لئے ہے۔
- `authority_digest` میزبان کا ہیش ہے جو ترتیب شدہ دستخط کرنے والوں/کورم ڈیٹا سے زیادہ ہے۔ گیجٹ مساوات کی جانچ پڑتال کرتا ہے لیکن دستخط کی توثیق کو دوبارہ نہیں کرتا ہے۔ ٹھوس طور پر میزبان Norito-`AccountId` (جو پہلے ہی کیننیکل ملٹی سیگ کنٹرولر کو سرایت کرتا ہے) اور ہیش `b"iroha:fastpq:v1:authority|" || encoded_account` کو BLAKE2B-256 کے ساتھ جوڑتا ہے ، جس کے نتیجے میں `Hash` کو اسٹور کیا جاتا ہے۔
- `poseidon_preimage_digest` = poseidon (اکاؤنٹ_فرم || اکاؤنٹ_ٹو || اثاثہ || رقم || بیچ_ھاش) ؛ یقینی بناتا ہے کہ گیجٹ میزبان کی طرح ڈائجسٹ کو دوبارہ گنتی کرتا ہے۔ مشترکہ پوسیڈون 2 مددگار کے ذریعے گزرنے سے پہلے ننگی Norito انکوڈنگ کا استعمال کرتے ہوئے `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` کے طور پر پرییمج بائٹس تعمیر کیے جاتے ہیں۔ یہ ہضم سنگل ڈیلٹا ٹرانسکرپٹس کے لئے موجود ہے اور اسے ملٹی ڈیلٹا بیچوں کے لئے چھوڑ دیا گیا ہے۔

تمام شعبوں کو Norito کے ذریعے سیریلائز کیا جاتا ہے لہذا موجودہ عزم کی ضمانت دی جاتی ہے۔
`from_path` اور `to_path` دونوں کو استعمال کرتے ہوئے Norito BLOBs کے طور پر خارج کیا گیا ہے
`TransferMerkleProofV1` اسکیما: `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`۔
مستقبل کے ورژن اسکیما کو بڑھا سکتے ہیں جبکہ پروور ورژن ٹیگ کو نافذ کرتا ہے
ضابطہ کشائی سے پہلے `TransitionBatch` میٹا ڈیٹا Norito-ENCODED ٹرانسکرپٹ کو سرایت کرتا ہے
`transfer_transcripts` کلید کے تحت ویکٹر تاکہ پروور گواہ کو ڈی کوڈ کرسکے
آؤٹ آف بینڈ سوالات انجام دیئے بغیر۔ عوامی آدانوں (`dsid` ، `slot` ، جڑیں ،
`perm_root` ، `tx_set_hash`) `FastpqTransitionBatch.public_inputs` میں لے جایا جاتا ہے ،
انٹری ہیش/ٹرانسکرپٹ گنتی کی کتاب کی کیپنگ کے لئے میٹا ڈیٹا چھوڑنا۔ میزبان پلمبنگ تک
زمینیں ، حامی مصنوعی طور پر کلیدی/توازن کے جوڑے سے ثبوتوں سے اخذ کرتی ہے
جب بھی ٹرانسکرپٹ اختیاری شعبوں کو چھوڑ دیتا ہے تب بھی ہمیشہ ایک ڈٹرمینسٹک ایس ایم ٹی راستہ شامل کریں۔

## گیجٹ لے آؤٹ

1. ** بیلنس ریاضی کا بلاک **
   - ان پٹ: `from_balance_before` ، `amount` ، `to_balance_before`۔
   - چیک:
     - `from_balance_before >= amount` (مشترکہ RNS سڑن کے ساتھ رینج گیجٹ)۔
     - `from_balance_after = from_balance_before - amount`۔
     - `to_balance_after = to_balance_before + amount`۔
   - ایک کسٹم گیٹ میں بھری ہوئی ہے لہذا تینوں مساوات ایک قطار گروپ کا استعمال کرتی ہیں۔2. ** پوسیڈن عزم بلاک **
   - مشترکہ پوسیڈن لوک اپ ٹیبل کا استعمال کرتے ہوئے `poseidon_preimage_digest` کو دوبارہ شامل کریں جو پہلے سے دوسرے گیجٹ میں استعمال ہوتا ہے۔ ٹریس میں فی ٹرانسفر پوسیڈن راؤنڈ نہیں ہے۔

3. ** مرکل پاتھ بلاک **
   - موجودہ کیگی ایس ایم ٹی گیجٹ کو "جوڑ بنانے والی اپ ڈیٹ" وضع کے ساتھ بڑھاتا ہے۔ دو پتے (مرسل ، وصول کنندہ) بہن بھائی ہیشوں کے لئے ایک ہی کالم کا اشتراک کرتے ہیں ، جس سے نقل کی قطاریں کم ہوتی ہیں۔

4. ** اتھارٹی ڈائجسٹ چیک **
   - میزبان فراہم کردہ ڈائجسٹ اور گواہ کی قیمت کے مابین مساوات کی آسان رکاوٹ۔ دستخط اپنے سرشار گیجٹ میں باقی ہیں۔

5. ** بیچ لوپ **
   - پروگرام `transfer_asset` بلڈرز اور `transfer_v1_batch_end()` کے لوپ سے پہلے `transfer_v1_batch_begin()` پر کال کریں۔ اگرچہ دائرہ کار فعال ہے میزبان ہر منتقلی کو بفر کرتا ہے اور انہیں ایک واحد `TransferAssetBatch` کے طور پر دوبارہ بھیج دیتا ہے ، پوسیڈن/ایس ایم ٹی سیاق و سباق کو ایک بار ایک بیچ میں دوبارہ استعمال کرتے ہوئے۔ ہر اضافی ڈیلٹا میں صرف ریاضی اور دو پتیوں کی جانچ پڑتال ہوتی ہے۔ ٹرانسکرپٹ ڈیکوڈر اب ملٹی ڈیلٹا بیچوں کو قبول کرتا ہے اور ان کو `TransferGadgetInput::deltas` کے طور پر سطح پر رکھتا ہے تاکہ منصوبہ ساز Norito کو دوبارہ پڑھنے کے بغیر گواہوں کو جوڑ سکے۔ معاہدے جن میں پہلے ہی Norito پے لوڈ ہانڈی ہے (جیسے ، CLI/SDKs) `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)` کو کال کرکے مکمل طور پر دائرہ کار کو چھوڑ سکتا ہے ، جس میں میزبان کو ایک سیسکل میں مکمل طور پر انکوڈڈ بیچ کے حوالے کیا جاتا ہے۔

# میزبان اور پروور تبدیلیاں| پرت | تبدیلیاں |
| ------- | --------- |
| `ivm::syscalls` | `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) شامل کریں تاکہ پروگرام متعدد `transfer_v1` syscals کو انٹرمیڈیٹ ISIS ، پلس `transfer_v1_batch_apply` (`transfer_v1_batch_apply` (`transfer_v1_batch_apply` کے علاوہ |
| `ivm::host` & ٹیسٹ | کور/ڈیفالٹ میزبان `transfer_v1` کو بیچ کے ساتھ جوڑتے ہیں جبکہ دائرہ کار فعال ہوتا ہے ، سطح `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}` ، اور MOCK WSV میزبان بفرز اندراجات سے پہلے رجعت ٹیسٹوں کا ارتکاب کرنے سے پہلے ڈٹرمینسٹک توازن کا دعوی کرسکتا ہے۔ تازہ ترین معلومات .
| `iroha_core` | ریاستی منتقلی کے بعد `TransferTranscript` emit ، Kotodama کے ساتھ Kotodama کے ساتھ Kotodama کے ساتھ `StateBlock::capture_exec_witness` بنائیں ، اور فاسٹ پی کیو پروور لین کو چلائیں لہذا دونوں Torii/CLI ٹولنگ حاصل کریں اور اسٹیج 6 بیکینڈ کو کینونیکل حاصل کریں۔ `TransferAssetBatch` گروپس ایک ہی ٹرانسکرپٹ میں ترتیب وار منتقلی کرتے ہیں ، ملٹی ڈیلٹا بیچوں کے لئے پوسیڈن ڈائجسٹ کو چھوڑ دیتے ہیں تاکہ گیجٹ انٹریوں کے عین مطابق داخل ہوسکے۔ |
| `fastpq_prover` | `gadgets::transfer` اب کثیر ڈیلٹا ٹرانسکرپٹس (بیلنس ریاضی + پوسیڈن ڈائجسٹ) اور سطحوں کے ساختہ گواہوں (جس میں پلیس ہولڈر کے جوڑے والے ایس ایم ٹی بلبس بھی شامل ہے) کے پلانر (`crates/fastpq_prover/src/gadgets/transfer.rs`) کی توثیق کرتا ہے۔ `trace::build_trace` بیچ میٹا ڈیٹا سے باہر ان نقلوں کو ڈیکوڈ کرتا ہے ، `transfer_transcripts` پے لوڈ سے محروم منتقلی بیچوں کو مسترد کرتا ہے ، توثیق شدہ گواہوں کو `Trace::transfer_witnesses` سے منسلک کرتا ہے ، اور `TracePolynomialData::transfer_plan()` منصوبے کو زندہ رکھتا ہے۔ قطار گنتی کے رجعت کا استعمال اب `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) کے ذریعے جہاز کرتا ہے ، جس میں 65536 بولڈ قطاروں تک کے منظرناموں کا احاطہ کیا جاتا ہے ، جبکہ جوڑ بنانے والی ایس ایم ٹی وائرنگ TF-3 بیچ-ہیلپر سنگ میل کے پیچھے رہ جاتی ہے (جگہ کے حامل اس سوئپ لینڈس کو تبدیل کرتے ہیں جب تک کہ اس میں سوئپ زمین کو مستحکم رکھا جاتا ہے۔ |
| Kotodama | `transfer_batch((from,to,asset,amount), …)` مددگار کو `transfer_v1_batch_begin` میں کم کرتا ہے ، ترتیب `transfer_asset` کالز ، اور `transfer_v1_batch_end`۔ ہر ٹیوپل دلیل کو `(AccountId, AccountId, AssetDefinitionId, int)` شکل پر عمل کرنا چاہئے۔ سنگل ٹرانسفر موجودہ بلڈر کو برقرار رکھتا ہے۔ |

مثال Kotodama استعمال:

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

`TransferAssetBatch` انفرادی `Transfer::asset_numeric` کالز کی طرح ایک ہی اجازت اور ریاضی کے چیکوں کو انجام دیتا ہے ، لیکن ایک واحد `TransferTranscript` کے اندر تمام ڈیلٹا کو ریکارڈ کرتا ہے۔ ملٹی ڈیلٹا ٹرانسکرپٹس پوسیڈن ڈائجسٹ کو شامل کرتے ہیں جب تک کہ فی ڈیلٹا کے وعدے فالو اپ میں نہ آجائیں۔ Kotodama بلڈر اب شروع/اختتام سیسکلس کو خود بخود خارج کرتا ہے ، لہذا معاہدے بغیر ہینڈ انکوڈنگ Norito پے لوڈ کے بغیر بیچڈ ٹرانسفر تعینات کرسکتے ہیں۔

## قطار گنتی رجعت پسندی

`fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) فاسٹ پی کیو ٹرانزیشن بیچوں کو ترتیب دینے والا سلیکٹر گنتی کے ساتھ سنتھائز کرتا ہے اور اس کے نتیجے میں `row_usage` خلاصہ (`total_rows` ، فی سلیکٹر گنتی ، تناسب) کے ساتھ ساتھ پیڈڈ لمبائی/لاگ ان کی اطلاع دیتا ہے۔ 65536-صف کی چھت کے لئے بینچ مارک پر قبضہ کریں:

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```خارج شدہ JSON فاسٹ پی کیو بیچ نمونے کی آئینہ دار ہے جو `iroha_cli audit witness` اب ڈیفالٹ کے ذریعہ خارج ہوتا ہے (ان کو دبانے کے لئے `--no-fastpq-batches` کو پاس کریں) ، لہذا `scripts/fastpq/check_row_usage.py` اور CI گیٹ مصنوعی رنز کو اسنیپ شاٹس کے خلاف مختلف ہوسکتا ہے جب پلانر کی تبدیلیوں کی توثیق کرتے ہو۔

# رول آؤٹ پلان

1. ** TF-1 (ٹرانسکرپٹ پلمبنگ) **: ✅ `StateTransaction::record_transfer_transcripts` اب ہر Kotodama/بیچ کے لئے Norito ٹرانسکرپٹس کا اخراج کرتا ہے ، Kotodama کے آپریٹرز اور پروور لین (اگر آپ کو کسی پتلی کی ضرورت ہو تو `--no-fastpq-batches` استعمال کریں آؤٹ پٹ). 【کریٹس/آئروہ_کور/ایس آر سی/اسٹیٹ۔ آر ایس: 8801 】【 کریٹس/اروہ_کور/ایس آر سی/سمرگی/گواہ۔
2. ** TF-2 (گیجٹ پر عمل درآمد) **: ✅ `gadgets::transfer` اب ملٹی ڈیلٹا ٹرانسکرپٹس (بیلنس ریاضی + پوسیڈن ڈائجسٹ) کی توثیق کرتا ہے ، جب میزبان ان کو چھوڑ دیتے ہیں تو ان کو الگ الگ کرنے والے SMT ثبوتوں کی ترکیبیں ، `TransferGadgetPlan` ، اور I18NIC کے ذریعہ ساختی گواہوں کو بے نقاب کرتی ہیں۔ `Trace::transfer_witnesses` ثبوتوں سے ایس ایم ٹی کالموں کو مقبول کرتے ہوئے۔ `fastpq_row_bench` 65536-قطار رجعت کنٹرول کو اپنی گرفت میں لے لیتا ہے لہذا منصوبہ سازوں کو دوبارہ استعمال کیے بغیر Norito کو دوبارہ کھیلے بغیر ٹریک کرتا ہے پے لوڈز۔ 【کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/گیجٹ/ٹرانسفر۔
3.
4.

# سوالات کھولیں

- ** ڈومین کی حدود **: 2⁴ قطار سے زیادہ نشانات کے لئے موجودہ FFT پلانر گھبراہٹ۔ TF-2 کو یا تو ڈومین سائز میں اضافہ کرنا چاہئے یا کم بینچ مارک ہدف کی دستاویز کرنا چاہئے۔
- ** ملٹی ایسٹ بیچز **: ابتدائی گیجٹ فی ڈیلٹا میں ایک ہی اثاثہ شناخت سنبھالتا ہے۔ اگر ہمیں متضاد بیچوں کی ضرورت ہو تو ، ہمیں یہ یقینی بنانا ہوگا کہ پوسیڈن گواہ کو کراس اثاثہ ری پلے کو روکنے کے لئے ہر بار اثاثہ شامل ہوتا ہے۔
۔


یہ دستاویز ڈیزائن کے فیصلوں کا سراغ لگاتی ہے۔ جب سنگ میل اتریں تو اسے روڈ میپ اندراجات کے ساتھ ہم آہنگ رکھیں۔