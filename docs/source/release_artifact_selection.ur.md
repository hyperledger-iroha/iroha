---
lang: ur
direction: rtl
source: docs/source/release_artifact_selection.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3ea92fbfd7a44cd789ecf187e0edc0dcb33969d45836dd55af706424c66656b
source_last_modified: "2025-11-02T04:40:39.806222+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/release_artifact_selection.md -->

# Iroha ریلیز آرٹیفیکٹ انتخاب

یہ نوٹ واضح کرتا ہے کہ ہر ریلیز پروفائل کیلئے کون سے آرٹیفیکٹس (bundles اور کنٹینر امیجز) آپریٹرز کو ڈپلائے کرنے چاہئیں۔

## پروفائلز

- **iroha2 (Self-hosted networks)** — single-lane کنفیگریشن جو `defaults/genesis.json` اور `defaults/client.toml` کے مطابق ہے۔
- **iroha3 (SORA Nexus)** — Nexus multi-lane کنفیگریشن جو `defaults/nexus/*` templates استعمال کرتی ہے۔

## Bundles (بائنریز)

Bundles کو `scripts/build_release_bundle.sh` کے ذریعے `--profile` کو `iroha2` یا `iroha3` پر سیٹ کرکے تیار کیا جاتا ہے۔

ہر tarball میں شامل ہے:

- `bin/` — `irohad`, `iroha`, اور `kagami` جو deploy پروفائل کے ساتھ built ہیں۔
- `config/` — پروفائل مخصوص genesis/client کنفیگریشن (single vs. nexus)۔ Nexus bundles میں lanes اور DA پیرامیٹرز کے ساتھ `config.toml` شامل ہوتا ہے۔
- `PROFILE.toml` — پروفائل، کنفیگ، ورژن، commit، OS/arch، اور enabled feature set کی میٹاڈیٹا۔
- tarball کے ساتھ لکھے جانے والے میٹاڈیٹا آرٹیفیکٹس:
  - `<profile>-<version>-<os>.tar.zst`
  - `<profile>-<version>-<os>.tar.zst.sha256`
  - `<profile>-<version>-<os>.tar.zst.sig` اور `.pub` (جب `--signing-key` فراہم ہو)
  - `<profile>-<version>-manifest.json` جو tarball path، hash، اور signature کی تفصیلات محفوظ کرتا ہے

## کنٹینر امیجز

کنٹینر امیجز `scripts/build_release_image.sh` کے ذریعے انہی profile/config arguments کے ساتھ تیار ہوتی ہیں۔

Outputs:

- `<profile>-<version>-<os>-image.tar`
- `<profile>-<version>-<os>-image.tar.sha256`
- اختیاری signature/public key (`*.sig`/`*.pub`)
- `<profile>-<version>-image.json` جو tag، image ID، hash، اور signature metadata ریکارڈ کرتا ہے

## درست آرٹیفیکٹ کا انتخاب

1. deployment surface طے کریں:
   - **SORA Nexus / multi-lane** -> `iroha3` کا bundle اور image استعمال کریں۔
   - **Self-hosted single-lane** -> `iroha2` کے artefacts استعمال کریں۔
   - اگر شک ہو تو `scripts/select_release_profile.py --network <alias>` یا `--chain-id <id>` چلائیں؛ یہ helper نیٹ ورکس کو درست پروفائل سے `release/network_profiles.toml` کے مطابق میپ کرتا ہے۔
2. مطلوبہ tarball اور متعلقہ manifest فائلیں ڈاؤن لوڈ کریں۔ unpack کرنے سے پہلے SHA256 hash اور signature کی تصدیق کریں:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub        -signature iroha3-<version>-linux.tar.zst.sig        iroha3-<version>-linux.tar.zst
   ```
3. bundle نکالیں (`tar --use-compress-program=zstd -xf <tar>`) اور `bin/` کو deployment PATH میں رکھیں۔ ضرورت کے مطابق لوکل کنفیگریشن overrides لاگو کریں۔
4. اگر کنٹینرائزڈ deployments استعمال کر رہے ہوں تو `docker load -i <profile>-<version>-<os>-image.tar` سے امیج لوڈ کریں۔ لوڈ کرنے سے پہلے hash/Signature اوپر کی طرح verify کریں۔

## Nexus کنفیگریشن چیک لسٹ

- `config/config.toml` میں `[nexus]`, `[nexus.lane_catalog]`, `[nexus.dataspace_catalog]`, اور `[nexus.da]` سیکشنز شامل ہونے چاہئیں۔
- تصدیق کریں کہ lane routing rules گورننس توقعات (`nexus.routing_policy`) سے میل کھاتے ہیں۔
- DA thresholds (`nexus.da`) اور fusion parameters (`nexus.fusion`) کونسل سے منظور شدہ سیٹنگز کے مطابق ہوں۔

## Single-lane کنفیگریشن چیک لسٹ

- `config/config.d` (اگر موجود ہو) میں صرف single-lane overrides ہوں، `[nexus]` سیکشنز نہ ہوں۔
- یقینی بنائیں کہ `config/client.toml` مطلوبہ Torii endpoint اور peer list کو ریفرنس کرتا ہے۔
- Genesis کو self-hosted نیٹ ورک کیلئے canonical domains/assets برقرار رکھنے چاہئیں۔

## Tooling کا فوری حوالہ

- `scripts/build_release_bundle.sh --help`
- `scripts/build_release_image.sh --help`
- `scripts/select_release_profile.py --list`
- `docs/source/sora_nexus_operator_onboarding.md` — artefacts منتخب ہونے کے بعد Sora Nexus data-space آپریٹرز کیلئے end-to-end onboarding فلو۔

</div>
