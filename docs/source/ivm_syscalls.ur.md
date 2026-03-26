---
lang: ur
direction: rtl
source: docs/source/ivm_syscalls.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bcf280df1e00065199d386e07b9fd67d8f94c4046d73cfa3b63d1eec18228cd8
source_last_modified: "2026-01-22T16:01:14.866000+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM syscall abi

اس دستاویز میں IVM سیسکل نمبرز ، پوائنٹر-ABI کالنگ کنونشنز ، محفوظ نمبر کی حدود ، اور Kotodama ایکس کم کے ذریعہ استعمال ہونے والے معاہدے کا سامنا کرنے والے سیسکلز کی کیننیکل ٹیبل کی وضاحت کی گئی ہے۔ یہ `ivm.md` (فن تعمیر) اور `kotodama_grammar.md` (زبان) کی تکمیل کرتا ہے۔

ورژننگ
- تسلیم شدہ سیسکلز کا سیٹ بائیک کوڈ ہیڈر `abi_version` فیلڈ پر منحصر ہے۔ پہلی ریلیز صرف `abi_version = 1` کو قبول کرتی ہے۔ داخلے کے وقت دیگر اقدار کو مسترد کردیا جاتا ہے۔ `E_SCALL_UNKNOWN` کے ساتھ فعال `abi_version` کے لئے نامعلوم نمبر
- رن ٹائم اپ گریڈ `abi_version = 1` رکھیں اور سیسکل یا پوائنٹر - ABI سطحوں میں توسیع نہ کریں۔
- سیسکل گیس کے اخراجات بائیک کوڈ ہیڈر ورژن کے پابند ورژن والے گیس شیڈول کا حصہ ہیں۔ `ivm.md` (گیس پالیسی) دیکھیں۔

نمبر کی حدود
- `0x00..=0x1F`: VM کور/یوٹیلیٹی (ڈیبگ/ایگزٹ مددگار `CoreHost` کے تحت دستیاب ہیں the باقی DEV مددگار صرف مذاق کی میزبان ہیں)۔
- `0x20..=0x5F`: Iroha کور ISI برج (ABI V1 میں مستحکم)۔
- `0x60..=0x7F`: توسیع ISIS پروٹوکول کی خصوصیات کے ذریعہ گیٹڈ (جب بھی فعال ہونے پر ABI V1 کا حصہ ہے)۔
- `0x80..=0xFF`: میزبان/کریپٹو مددگار اور محفوظ سلاٹ ؛ صرف ABI V1 اجازت نامے میں موجود نمبر قبول کیے جاتے ہیں۔

پائیدار مددگار (ABI V1)
- پائیدار اسٹیٹ ہیلپر سیسکلز (0x50–0x5a: state_ {get ، سیٹ ، سیٹ ، ڈیل} ، انکوڈ/ڈیکوڈ_نٹ ، بلڈ_پاتھ_*، JSON/اسکیما انکوڈ/ڈیکوڈ) V1 ABI کا حصہ ہیں اور `abi_hash` کمپیوٹیشن میں شامل ہیں۔
-کور ہوسٹ تاروں ریاست_ {حاصل کریں ، سیٹ کریں ، ڈیل} کو WSV کی حمایت یافتہ پائیدار سمارٹ معاہدہ ریاست ؛ دیو/ٹیسٹ میزبان مقامی طور پر برقرار رہ سکتے ہیں لیکن انہیں ایک جیسے سیسکل سیمنٹکس کو محفوظ رکھنا چاہئے۔

پوائنٹر - اے بی آئی کالنگ کنونشن (اسمارٹ - کنٹریکٹ سیسکلز)
- دلائل `r10+` میں RAW `u64` اقدار کے طور پر یا ان پٹ خطے میں اشارے کے طور پر Norito TLV لفافے (جیسے ، `AccountId` ، `AssetDefinitionId` ، I18000028X ، I1800000028X ، `AssetDefinitionId` ، `AccountId` ، `AccountId` `NftId`)۔
- اسکیلر ریٹرن ویلیوز `u64` میزبان سے واپس آئے ہیں۔ پوائنٹر کے نتائج میزبان کے ذریعہ `r10` میں لکھے گئے ہیں۔

کیننیکل سیسکل ٹیبل (سبسیٹ)| ہیکس | نام | دلائل (`r10+` میں) | واپسی | گیس (بیس + متغیر) | نوٹ |
| -------- | -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
| 0x1a | set_account_detail | `&AccountId` ، `&Name` ، `&Json` | `u64=0` | `G_set_detail + bytes(val)` | اکاؤنٹ کے لئے ایک تفصیل لکھتی ہے |
| 0x22 | ٹکسال_اسیٹ | `&AccountId` ، `&AssetDefinitionId` ، `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | اکاؤنٹ میں اثاثہ کے ٹکسال `amount` |
| 0x23 | برن_اسیٹ | `&AccountId` ، `&AssetDefinitionId` ، `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | اکاؤنٹ سے `amount` برنز |
| 0x24 | ٹرانسفر_اسیٹ | `&AccountId(from)` ، `&AccountId(to)` ، `&AssetDefinitionId` ، `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | اکاؤنٹس کے مابین `amount` کی منتقلی |
| 0x29 | منتقلی_V1_BATCH_BEGIN | - | `u64=0` | `G_transfer` | فاسپ کیو ٹرانسفر بیچ اسکوپ شروع کریں
| 0x2a | منتقلی_V1_Batch_end | - | `u64=0` | `G_transfer` | فلش جمع فاسٹ پی کیو ٹرانسفر بیچ |
| 0x2b | ٹرانسفر_V1_BATCH_APPLE | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | ایک ہی سیسکل میں Norito-انکوڈڈ بیچ کا اطلاق کریں
| 0x25 | nft_mint_asset | `&NftId` ، `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | رجسٹر ایک نیا NFT |
| 0x26 | nft_transfer_asset | `&AccountId(from)` ، `&NftId` ، `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | NFT کی ملکیت کی منتقلی |
| 0x27 | nft_set_metadata | `&NftId` ، `&Json` | `u64=0` | `G_nft_set_metadata` | تازہ ترین معلومات NFT میٹا ڈیٹا |
| 0x28 | nft_burn_asset | `&NftId` | `u64=0` | `G_nft_burn_asset` | برنز (تباہ) ایک این ایف ٹی |
| 0xa1 | اسمارٹ کنٹریکٹ_یکسیکیٹ_کوری | `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | ایٹ ایبل سوالات اففیرلی طور پر چلتے ہیں۔ `QueryRequest::Continue` مسترد |
| 0xa2 | create_nfts_for_all_users | - | `u64=count` | `G_create_nfts_for_all` | مددگار ؛ خصوصیت - گیٹڈ || 0xA3 | set_smartcontract_execution_depth | `depth:u64` | `u64=prev` | `G_set_depth` | ایڈمن ؛ خصوصیت - گیٹڈ |
| 0xa4 | get_authority | - (ہوسٹ لکھتے ہیں نتیجہ) | `&AccountId` | `G_get_auth` | میزبان موجودہ اتھارٹی کو `r10` | میں پوائنٹر لکھتا ہے
| 0xf7 | get_merkle_path | `addr:u64` ، `out_ptr:u64` ، اختیاری `root_out:u64` | `u64=len` | `G_mpath + len` | راہ (پتی → جڑ) اور اختیاری جڑ بائٹس لکھتا ہے
| 0xfa | get_merkle_compact | `addr:u64` ، `out_ptr:u64` ، اختیاری `depth_cap:u64` ، اختیاری `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xff | get_register_merkle_compact | `reg_index:u64` ، `out_ptr:u64` ، اختیاری `depth_cap:u64` ، اختیاری `root_out:u64` | `u64=depth` | `G_mpath + depth` | رجسٹر کے عزم کے لئے ایک ہی کمپیکٹ لے آؤٹ |

گیس کا نفاذ
- کور ہوسٹ نے مقامی آئی ایس آئی شیڈول کا استعمال کرتے ہوئے آئی ایس آئی سیسکلز کے لئے اضافی گیس وصول کی۔ فاسٹ پی کیو بیچ کی منتقلی فی انٹری سے وصول کی جاتی ہے۔
- ZK_VERIFY SYSCALLS خفیہ توثیق گیس کے نظام الاوقات (بیس + پروف سائز) کو دوبارہ استعمال کریں۔
-اسمارٹ کنٹریکٹ_کسیکیٹ_کیری چارجز بیس + فی آئٹم + فی بائٹ ؛ ہر آئٹم لاگت اور غیر ترتیب شدہ آفسیٹس کو ضرب لگانے سے فی آئٹم جرمانہ شامل ہوتا ہے۔

نوٹ
- تمام پوائنٹر دلائل حوالہ Norito TLV لفافے ان پٹ خطے میں لفافے اور پہلے ڈیرینس (`E_NORITO_INVALID` غلطی پر) پر توثیق کیا جاتا ہے۔
- تمام تغیرات کو Iroha کے معیاری ایگزیکٹر (`CoreHost` کے ذریعے) کے ذریعے لاگو کیا جاتا ہے ، براہ راست VM کے ذریعہ نہیں۔
- گیس کے عین مطابق مستقل (`G_*`) کی وضاحت گیس کے فعال شیڈول کے ذریعہ کی گئی ہے۔ `ivm.md` دیکھیں۔

غلطیاں
- `E_SCALL_UNKNOWN`: Syscall نمبر فعال `abi_version` کے لئے تسلیم نہیں کیا گیا ہے۔
- ان پٹ توثیق کی غلطیاں VM ٹریپس (جیسے ، خراب شدہ TLVs کے لئے `E_NORITO_INVALID`) کے طور پر پھیلتی ہیں۔

کراس - ریفرنسز
- فن تعمیر اور VM Semantics: `ivm.md`
- زبان اور بلٹین میپنگ: `docs/source/kotodama_grammar.md`

جنریشن نوٹ
- سیسکل مستقل کی ایک مکمل فہرست ماخذ سے تیار کی جاسکتی ہے:
  - `make docs-syscalls` → `docs/source/ivm_syscalls_generated.md` لکھتا ہے
  - `make check-docs` → تصدیق کرتا ہے کہ تیار کردہ جدول تازہ ترین ہے (CI میں مفید ہے)
- مذکورہ بالا سب سیٹ معاہدہ کا سامنا کرنے والے سیسکلز کے لئے ایک کیوریٹڈ ، مستحکم ٹیبل رہتا ہے۔

## ایڈمن/رول TLV مثالوں (موک میزبان)

اس حصے میں TLV کی شکلیں اور کم سے کم JSON پے لوڈ کی دستاویز کی گئی ہے جو موک WSV میزبان کے ذریعہ ایڈمن - اسٹائل سیسکلز کے لئے ٹیسٹوں میں استعمال ہونے والے ایڈمن - اسٹائل سیسکلز کے لئے قبول کیا گیا ہے۔ تمام پوائنٹر دلائل پوائنٹر - ABI (Norito TLV لفافے ان پٹ میں رکھے گئے) کی پیروی کرتے ہیں۔ پروڈکشن میزبان زیادہ سے زیادہ اسکیموں کا استعمال کرسکتے ہیں۔ ان مثالوں کا مقصد اقسام اور بنیادی شکلوں کو واضح کرنا ہے۔- رجسٹر_پیئر / غیر منظم_پیر
  - آرگس: `r10=&Json`
  - مثال JSON: `{ "peer": "peer-id-or-info" }`
  - کور ہوسٹ نوٹ: `REGISTER_PEER` `RegisterPeerWithPop` JSON آبجیکٹ کی توقع کرتا ہے جس کے ساتھ `peer` + `pop` بائٹس (اختیاری `activation_at` ، Kotodama) ؛ `UNREGISTER_PEER` ایک پیر-آئی ڈی سٹرنگ یا `{ "peer": "..." }` کو قبول کرتا ہے۔

- create_trigger / hemt_trigger / set_trigger_enabled
  - create_trigger:
    - آرگس: `r10=&Json`
    - کم سے کم JSON: `{ "name": "t1" }` (مذاق کے ذریعہ اضافی شعبوں کو نظرانداز کیا گیا)
  - ہٹائیں_ٹرگر:
    - آرگس: `r10=&Name` (ٹرگر نام)
  - SET_TRIGGER_ENABLED:
    - آرگس: `r10=&Name` ، `r11=enabled:u64` (0 = غیر فعال ، غیر - صفر = فعال)
  - کور ہوسٹ نوٹ: `CREATE_TRIGGER` ایک مکمل ٹرگر اسپیک (BASE64 Norito `Trigger` سٹرنگ یا کی توقع کرتا ہے
    `{ "id": "<trigger_id>", "action": ... }` `action` کے ساتھ BASE64 Norito `Action` سٹرنگ یا
    ایک JSON آبجیکٹ) ، اور `SET_TRIGGER_ENABLED` ٹرگر میٹا ڈیٹا کی کلید `__enabled` (غائب ہے
    فعال کرنے کے لئے پہلے سے طے شدہ)۔

- کردار: create_role / ڈیلیٹ_رول / گرانٹ_رول / ریووک_رول
  - create_role:
    - آرگس: `r10=&Name` (کردار کا نام) ، `r11=&Json` (اجازت سیٹ)
    - JSON یا تو کلیدی `"perms"` یا `"permissions"` کو قبول کرتا ہے ، ہر ایک کو اجازت ناموں کی ایک تار صف۔
    - مثالیں:
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:<i105-account-id>", "transfer_asset:rose#wonder" ] }`
    - مذاق میں اجازت نامہ نام کے سابقے کے سابقے:
      - `register_domain` ، `register_account` ، `register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - ڈیلیٹ_رول:
    - آرگس: `r10=&Name`
    - اگر ابھی بھی کسی اکاؤنٹ کو یہ کردار تفویض کیا گیا ہے تو ناکام ہوجاتا ہے۔
  - گرانٹ_رول / ریووک_رول:
    - آرگس: `r10=&AccountId` (مضمون) ، `r11=&Name` (کردار کا نام)
  - کور ہوسٹ نوٹ: اجازت JSON ایک مکمل `Permission` آبجیکٹ (`{ "name": "...", "payload": ... }`) یا سٹرنگ (`null` میں پے لوڈ ڈیفالٹس) ہوسکتی ہے۔ `GRANT_PERMISSION`/`REVOKE_PERMISSION` `&Name` یا `&Json(Permission)` کو قبول کریں۔

- غیر منظم اوپس (ڈومین/اکاؤنٹ/اثاثہ): حملہ آور (مذاق)
  - غیر منظم_ڈومین (`r10=&DomainId`) اگر ڈومین میں اکاؤنٹس یا اثاثوں کی تعریفیں موجود ہیں تو ناکام ہوجاتی ہے۔
  - اگر اکاؤنٹ میں غیر صفر بیلنس ہے یا NFTs کا مالک ہے تو غیر منظم_اکاونٹ (`r10=&AccountId`) ناکام ہوجاتا ہے۔
  - اگر اثاثہ کے لئے کوئی بیلنس موجود ہے تو غیر منظم_اسیٹ (`r10=&AssetDefinitionId`) ناکام ہوجاتا ہے۔

نوٹ
- یہ مثالیں ٹیسٹوں میں استعمال ہونے والے موک WSV میزبان کی عکاسی کرتی ہیں۔ اصلی نوڈ میزبان زیادہ تر ایڈمن اسکیموں کو بے نقاب کرسکتے ہیں یا اضافی توثیق کی ضرورت ہوتی ہے۔ پوائنٹر - ABI قواعد ابھی بھی لاگو ہوتے ہیں: TLVS ان پٹ میں ہونا چاہئے ، ورژن = 1 ، قسم کی شناختیں مماثل ہوں گی ، اور پے لوڈ ہیشوں کو توثیق کرنا ضروری ہے۔