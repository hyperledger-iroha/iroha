---
lang: ur
direction: rtl
source: docs/genesis.md
status: complete
translator: manual
source_hash: 975c403019da0b8700489610d86a75f26886f40f2b4c10963ee8269a68b4fe9b
source_last_modified: "2025-11-12T00:33:55.324259+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- docs/genesis.md (Genesis configuration) کا اردو ترجمہ -->

# جینیسس کنفیگریشن

`genesis.json` فائل وہ ابتدائی ٹرانزیکشنز بیان کرتی ہے جو Iroha نیٹ ورک کے
اسٹارٹ ہوتے ہی چلتی ہیں۔ یہ ایک JSON آبجیکٹ ہوتا ہے جس میں درج ذیل
فیلڈز شامل ہیں:

- `chain`: چین کا منفرد (unique) identifier۔
- `executor` (اختیاری): executor کے bytecode (`.to`) کا path۔ اگر یہ
  فیلڈ موجود ہو تو genesis، پہلی ٹرانزیکشن کے طور پر `Upgrade` instruction
  شامل کرتا ہے۔ اگر یہ موجود نہ ہو تو کوئی upgrade نہیں ہوتا اور built‑in
  executor استعمال ہوتا ہے۔
- `ivm_dir`: وہ ڈائریکٹری جس میں IVM bytecode لائبریریز موجود ہوں۔ اگر یہ
  فیلڈ نہ ہو تو ڈیفالٹ `"."` استعمال ہوتا ہے۔
- `consensus_mode`: manifest میں اعلان کردہ consensus mode۔ لازم ہے؛ Iroha3 کے لیے `"Npos"` (default) اور Iroha2 کے لیے `"Permissioned"` استعمال کریں۔
- `transactions`: genesis ٹرانزیکشنز کی فہرست، جو sequentially execute ہوتی
  ہیں۔ ہر entry میں یہ sub‑فیلڈز ہو سکتے ہیں:
  - `parameters`: نیٹ ورک کے ابتدائی parameters۔
  - `instructions`: Norito‑encoded instructions۔
  - `ivm_triggers`: triggers، جن کے ساتھ IVM bytecode executables ہوتے ہیں۔
  - `topology`: peers کی ابتدائی topology۔ ہر entry میں `peer` (PeerId بطور
    string، یعنی public key) اور `pop_hex` ہوتے ہیں؛ `pop_hex` تیار کرتے
    وقت چھوڑا جا سکتا ہے مگر سائن سے پہلے ضروری ہے۔
- `crypto`: کرپٹو کنفیگریشن کا snapshot، جو `iroha_config.crypto` سے mirror
  ہوتا ہے (`default_hash`, `allowed_signing`, `allowed_curve_ids`,
  `sm2_distid_default`, `sm_openssl_preview`)۔ فیلڈ `allowed_curve_ids`,
  `crypto.curves.allowed_curve_ids` کو reflect کرتی ہے، تاکہ manifests
  بتا سکیں کہ cluster کون سی controller curves accept کرتا ہے۔ tooling
  SM combinations پر hard rules لگاتا ہے: کوئی manifest اگر `sm2` advertise
  کرے تو اسے hash کو `sm3-256` پر بھی سوئچ کرنا ہو گا، اور جو builds بغیر
  `sm` feature کے compile ہوں گی، وہ `sm2` کو مکمل طور پر reject کریں گی۔

مثال (کمانڈ `kagami genesis generate default --consensus-mode npos` کی آؤٹ پٹ، instructions
مختصر کیے گئے ہیں):

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [78, 82, 84, 48, 0, 0, 19, 123, ...],
      "ivm_triggers": [],
      "topology": []
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### SM2/SM3 کے لیے `crypto` بلاک سیڈ کرنا

`xtask` helper استعمال کر کے ایک ہی قدم میں key inventory اور ready‑to‑paste
config snippet تیار کیا جا سکتا ہے:

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

اس کے بعد `client-sm2.toml` کے اندر درج ذیل مواد ہو گا:

```toml
# Account key material
public_key = "sm2:86264104..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]  # صرف verify موڈ میں رہنے کے لیے "sm2" کو ہٹا دیں
allowed_curve_ids = [1]               # جب controllers allow ہوں تو نئے curve ids (مثلاً SM2 کے لیے 15) شامل کریں
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # اختیاری: صرف OpenSSL/Tongsuo راستہ استعمال کرتے وقت
```

`public_key` اور `private_key` کی ویلیوز کو account/client کنفیگریشن میں
کاپی کریں، اور `genesis.json` کے `crypto` بلاک کو اس snippet کے مطابق اپڈیٹ
کریں (مثلاً `default_hash` کو `sm3-256` پر سیٹ کریں، `allowed_signing` میں
`"sm2"` شامل کریں اور درست `allowed_curve_ids` استعمال کریں)۔ Kagami ایسے
manifests کو reject کر دے گا جہاں hash/curve سیٹنگز اور allowed signing
list آپس میں متضاد ہوں۔

> **ٹِپ:** جب صرف output inspect کرنا ہو تو `--snippet-out -` استعمال کر کے
> snippet کو stdout پر بھیجیں۔ اسی طرح key inventory کو stdout پر دیکھنے
> کے لیے `--json-out -` استعمال کیا جا سکتا ہے۔

اگر آپ low‑level CLI کمانڈز manually چلانا چاہتے ہیں، تو equivalent flow یہ
ہے:

```bash
# 1. deterministic key material تیار کریں (JSON ڈِسک پر لکھتا ہے)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. وہ snippet ری ہائیڈریٹ کریں جسے client/config فائلوں میں چپکایا جا سکتا ہے
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **نوٹ:** اوپر `jq` اس لیے استعمال کیا گیا ہے کہ copy/paste کی manual
> محنت سے بچا جا سکے۔ اگر `jq` دستیاب نہ ہو، تو `sm2-key.json` کھول کر
> `private_key_hex` کی ویلیو کو کاپی کریں اور اسے `crypto sm2 export` کو
> براہِ راست دیں۔

> **Migration guide:** اگر آپ موجودہ نیٹ ورک کو SM2/SM3/SM4 میں migrate
> کر رہے ہیں تو layered `iroha_config` overrides، manifest regeneration اور
> rollback planning کے لیے
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md)
> کو فالو کریں۔

## Generate اور validate کرنا

1. Template generate کریں:

   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     [--genesis-public-key <public-key>] \
     > genesis.json
   ```

   - `--executor` (اختیاری) کسی IVM executor `.to` فائل کی طرف پوائنٹ کرتا
     ہے؛ اگر موجود ہو تو Kagami genesis میں پہلی ٹرانزیکشن کے طور پر
     `Upgrade` instruction شامل کرتا ہے۔
   - `--genesis-public-key` وہ public key سیٹ کرتا ہے جس سے genesis block
     پر دستخط کیے جائیں گے؛ اسے `iroha_crypto::Algorithm` میں سپورٹڈ
     multihash ہونا چاہیے (بشمول GOST TC26 variants جب متعلقہ feature
     فعال ہو)۔
   - Iroha3 میں `--consensus-mode npos` لازمی ہے اور staged cutovers سپورٹ نہیں ہوتے؛ Iroha2 میں default موڈ `permissioned` ہے۔

2. edit کے دوران validate کریں:

   ```bash
   cargo run -p iroha_kagami -- genesis validate genesis.json
   ```

   اس سے یقین دہانی ہوتی ہے کہ `genesis.json` schema کے مطابق ہے،
   parameters درست ہیں، `Name` values معتبر ہیں، اور Norito instructions
   کامیابی سے decode ہو سکتی ہیں۔

3. deployment کے لیے sign کریں:

   ```bash
   cargo run -p iroha_kagami -- genesis sign \
     genesis.json \
     --public-key <PK> \
     --private-key <SK> \
     --out-file genesis.signed.nrt
   ```

   جب `irohad` کو صرف `--genesis-manifest-json` (بغیر signed genesis block) کے
   ساتھ اسٹارٹ کیا جائے، تو node اپنے runtime crypto config کو manifest
   سے derive کرتا ہے؛ اگر genesis block بھی فراہم کیا گیا ہو، تو manifest
   اور config دونوں کو بالکل match کرنا ضروری ہے۔

`kagami genesis sign` JSON کی صحت (validity) کو چیک کرتا ہے اور Norito‑encoded
block تیار کرتا ہے جو node config میں `genesis.file` کے ذریعے استعمال کے
لیے تیار ہوتا ہے۔ حاصل شدہ فائل `genesis.signed.nrt` پہلے ہی canonical
wire فارم میں ہوتی ہے: ایک version byte، جس کے بعد Norito header آتا ہے
جو payload layout کو بیان کرتا ہے۔ ہمیشہ اسی framed output کو
distribute کریں۔ signed payloads کے لیے `.nrt` suffix کو ترجیح دیں؛ اگر
آپ کو genesis میں executor upgrade کی ضرورت نہ ہو تو `executor` فیلڈ کو
چھوڑ دیں اور `.to` فائل فراہم نہ کریں۔

NPoS منشورات (`--consensus-mode npos` یا مرحلہ وار کٹ اوور صرف Iroha2 میں) پر دستخط کرتے وقت `kagami genesis sign` کو `sumeragi_npos_parameters` payload درکار ہوتا ہے؛ اسے `kagami genesis generate --consensus-mode npos` سے بنائیں یا یہ parameter دستی طور پر شامل کریں۔
ڈیفالٹ طور پر `kagami genesis sign` manifest کے `consensus_mode` کو استعمال کرتا ہے؛ `--consensus-mode` سے override کریں۔

## Genesis کیا کر سکتا ہے؟

Genesis درج ذیل آپریشنز کو سپورٹ کرتا ہے۔ Kagami انہیں ایک well‑defined
آرڈر میں ٹرانزیکشنز کی شکل میں assemble کرتا ہے تاکہ peers ایک ہی
sequence deterministically execute کریں:

- **Parameters**: Sumeragi کے ابتدائی values سیٹ کرنا (block/commit
  times، drift وغیرہ)، Block (زیادہ سے زیادہ ٹرانزیکشنز)، Transaction
  (زیادہ سے زیادہ instructions اور bytecode size)، executor اور smart
  contracts کے limits (fuel، memory، depth)، اور custom parameters۔ Kagami
  `Sumeragi::NextMode` اور `sumeragi_npos_parameters` payload کو
  `parameters` block میں seed کرتا ہے، اور signed block میں generated
  `SetParameter` instructions شامل ہوتی ہیں تاکہ startup پر consensus
  knobs کو on‑chain state سے apply کیا جا سکے۔
- **Native instructions**: Domain، Account، Asset Definition کو register/
  unregister کرنا؛ assets کو mint/burn/transfer کرنا؛ domain اور asset
  definition کی ملکیت transfer کرنا؛ metadata میں تبدیلی کرنا؛ permissions
  اور roles دینا۔
- **IVM triggers**: ایسے triggers register کرنا جو IVM bytecode execute
  کرتے ہیں (تفصیل `ivm_triggers` میں)۔ triggers کے executables، `ivm_dir`
  کے نسبتاً paths کے طور پر resolve ہوتے ہیں۔
- **Topology**: `topology` array کے ذریعے ابتدائی peers فراہم کرنا، جو کسی بھی
  ٹرانزیکشن (اکثر پہلی یا آخری) میں شامل ہو سکتی ہے۔ ہر entry
  `{ "peer": "<public_key>", "pop_hex": "<hex>" }` ہوتی ہے؛ `pop_hex` کو
  تیاری کے وقت چھوڑا جا سکتا ہے، مگر دستخط سے پہلے لازم ہے۔
- **Executor upgrade (اختیاری)**: اگر `executor` موجود ہو، تو genesis پہلی
  ٹرانزیکشن کی جگہ ایک `Upgrade` instruction insert کرتا ہے؛ بصورتِ دیگر
  genesis براہِ راست parameters/instructions سے شروع ہو جاتا ہے۔

### ٹرانزیکشن آرڈرنگ

تصوری طور پر، genesis ٹرانزیکشنز اس ترتیب سے پروسیس ہوتی ہیں:

1. (اختیاری) executor upgrade  
2. `transactions` میں موجود ہر ٹرانزیکشن کے لیے:
   - parameter updates
   - native instructions
   - IVM trigger registrations
   - topology entries

Kagami اور node کا کوڈ اس آرڈر کو enforce کرتے ہیں، تاکہ مثلاً parameters،
اسی ٹرانزیکشن کی subsequent instructions سے پہلے apply ہو جائیں۔

## تجویز کردہ ورک فلو

- Kagami template سے شروع کریں:
  - صرف built‑in ISI:  
    `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json` (Iroha3 default؛ Iroha2 کے لیے `--consensus-mode permissioned`)
  - optional executor upgrade کے ساتھ: `--executor <path/to/executor.to>` شامل کریں۔
- `<PK>` کوئی بھی multihash ہو سکتا ہے جسے `iroha_crypto::Algorithm`
  پہچانتا ہو، بشمول TC26 GOST variants (مثلاً
  `gost3410-2012-256-paramset-a:...`)، جب Kagami کو `--features gost` کے
  ساتھ build کیا جائے۔
- edit کے دوران validate کریں: `kagami genesis validate genesis.json`
- deployment کے لیے sign کریں:
  `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- peers configure کریں: `genesis.file` میں signed Norito فائل (مثلاً
  `genesis.signed.nrt`) کا path ڈالیں اور `genesis.public_key` میں وہی
  `<PK>` استعمال کریں جس سے دستخط کیے گئے تھے۔

نوٹس:
- Kagami کا “default” template ایک sample domain اور accounts register کرتا
  ہے، چند assets mint کرتا ہے اور minimal permissions دیتا ہے، صرف
  built‑in ISIs استعمال کرتے ہوئے، یعنی `.to` درکار نہیں۔
- اگر آپ executor upgrade شامل کریں تو اسے لازمی طور پر پہلی ٹرانزیکشن
  ہونا چاہیے؛ Kagami اس constraint کو generation/signing کے وقت enforce
  کرتا ہے۔
- `kagami genesis validate` استعمال کریں تاکہ invalid `Name` values
  (مثلاً whitespace) اور malformed instructions کو sign کرنے سے پہلے
  detect کیا جا سکے۔

## Docker/Swarm کے ساتھ چلانا

فراہم کردہ Docker Compose اور Swarm tooling دونوں صورتوں کو handle کرتے
ہیں:

- بغیر executor: compose کمانڈ کسی missing/empty `executor` فیلڈ کو strip
  کر کے فائل کو sign کرتی ہے۔
- executor کے ساتھ: executor کا relative path container کے اندر absolute
  path میں resolve کر کے فائل کو sign کیا جاتا ہے۔

اس سے development، ایسی مشینوں پر بھی آسان رہتا ہے جہاں prebuilt IVM
samples موجود نہ ہوں، اور ساتھ ہی ساتھ، ضرورت پڑنے پر executor upgrades
کی سہولت بھی برقرار رہتی ہے۔

</div>
