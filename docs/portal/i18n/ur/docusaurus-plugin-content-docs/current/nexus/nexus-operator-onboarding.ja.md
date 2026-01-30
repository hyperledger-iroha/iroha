---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/nexus/nexus-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a7cd08993cc0c8705e2bfdec4d5b8ef4918464f9a6133b57f7e6c4eb9e603ab0
source_last_modified: "2026-01-03T18:08:01+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-operator-onboarding.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-operator-onboarding
title: Sora Nexus data-space آپریٹر آن بورڈنگ
description: `docs/source/sora_nexus_operator_onboarding.md` کا آئینہ، جو Nexus آپریٹرز کے لئے end-to-end ریلیز چیک لسٹ کو ٹریک کرتا ہے۔
---

:::note کینونیکل ماخذ
یہ صفحہ `docs/source/sora_nexus_operator_onboarding.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ایڈیشنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Sora Nexus Data-Space Operator Onboarding

یہ گائیڈ end-to-end فلو کو محفوظ کرتی ہے جس پر Sora Nexus data-space آپریٹرز کو ریلیز کے اعلان کے بعد عمل کرنا ہوتا ہے۔ یہ dual-track runbook (`docs/source/release_dual_track_runbook.md`) اور artefact selection note (`docs/source/release_artifact_selection.md`) کی تکمیل کرتی ہے، اور بتاتی ہے کہ نوڈ کو آن لائن لانے سے پہلے ڈاؤن لوڈ شدہ bundles/images، manifests اور configuration templates کو عالمی lane expectations کے ساتھ کیسے ہم آہنگ کرنا ہے۔

## سامعین اور پیشگی شرائط
- آپ کو Nexus Program نے منظور کیا ہے اور آپ کو data-space assignment مل چکی ہے (lane index، data-space ID/alias، اور routing policy requirements).
- آپ Release Engineering کی شائع کردہ signed release artefacts تک رسائی رکھتے ہیں (tarballs، images، manifests، signatures، public keys).
- آپ نے اپنے validator/observer رول کے لئے پروڈکشن key material تیار یا حاصل کیا ہے (Ed25519 node identity؛ validators کے لئے BLS consensus key + PoP؛ اور کوئی بھی confidential feature toggles).
- آپ ان موجودہ Sora Nexus peers تک رسائی کر سکتے ہیں جو آپ کے نوڈ کا bootstrap کریں گے۔

## مرحلہ 1 - ریلیز پروفائل کی تصدیق
1. وہ network alias یا chain ID شناخت کریں جو آپ کو دیا گیا ہے۔
2. اس ریپوزٹری کے checkout پر `scripts/select_release_profile.py --network <alias>` (یا `--chain-id <id>`) چلائیں۔ helper `release/network_profiles.toml` دیکھ کر deploy ہونے والا پروفائل پرنٹ کرتا ہے۔ Sora Nexus کے لئے جواب `iroha3` ہونا چاہئے۔ کسی بھی دوسرے ویلیو پر رک جائیں اور Release Engineering سے رابطہ کریں۔
3. ریلیز اعلان میں دیا گیا version tag نوٹ کریں (مثلاً `iroha3-v3.2.0`); اسی سے آپ artefacts اور manifests حاصل کریں گے۔

## مرحلہ 2 - artefacts حاصل کریں اور ویریفائی کریں
1. `iroha3` bundle (`<profile>-<version>-<os>.tar.zst`) اور اس کے companion files ڈاؤن لوڈ کریں (`.sha256`, اختیاری `.sig/.pub`, `<profile>-<version>-manifest.json`, اور `<profile>-<version>-image.json` اگر آپ containers ڈپلائے کر رہے ہیں)۔
2. ان پیک کرنے سے پہلے integrity چیک کریں:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   اگر آپ hardware-backed KMS استعمال کرتے ہیں تو `openssl` کو ادارہ منظور شدہ verifier سے بدل دیں۔
3. tarball کے اندر `PROFILE.toml` اور JSON manifests دیکھ کر تصدیق کریں:
   - `profile = "iroha3"`
   - `version`, `commit`, اور `built_at` فیلڈز ریلیز اعلان سے ملتے ہیں۔
   - OS/architecture آپ کے deployment target سے match کرتی ہے۔
4. اگر آپ container image استعمال کرتے ہیں تو `<profile>-<version>-<os>-image.tar` کے لئے hash/signature دوبارہ verify کریں اور `<profile>-<version>-image.json` میں درج image ID کنفرم کریں۔

## مرحلہ 3 - templates سے configuration تیار کریں
1. bundle extract کریں اور `config/` کو اس جگہ کاپی کریں جہاں نوڈ اپنی configuration پڑھے گا۔
2. `config/` کے تحت فائلوں کو templates سمجھیں:
   - `public_key`/`private_key` کو اپنے پروڈکشن Ed25519 keys سے بدلیں۔ اگر نوڈ keys HSM سے لے گا تو private keys کو disk سے ہٹا دیں؛ config کو HSM connector کی طرف پوائنٹ کریں۔
   - `trusted_peers`, `network.address` اور `torii.address` کو آپ کے قابل رسائی interfaces اور مقررہ bootstrap peers کے مطابق ایڈجسٹ کریں۔
   - `client.toml` کو operator-facing Torii endpoint (TLS configuration سمیت، اگر لاگو ہو) اور آپ کی provisioning کردہ credentials کے ساتھ اپ ڈیٹ کریں۔
3. bundle میں فراہم کردہ chain ID برقرار رکھیں، الا یہ کہ Governance واضح طور پر ہدایت دے - global lane ایک واحد canonical chain identifier چاہتا ہے۔
4. نوڈ کو Sora پروفائل فلیگ کے ساتھ اسٹارٹ کرنے کا ارادہ رکھیں: `irohad --sora --config <path>`. اگر فلیگ نہ ہو تو configuration loader SoraFS یا multi-lane سیٹنگز کو reject کر دے گا۔

## مرحلہ 4 - data-space metadata اور routing ہم آہنگ کریں
1. `config/config.toml` ایڈٹ کریں تاکہ `[nexus]` سیکشن Nexus Council کے فراہم کردہ data-space catalog سے match کرے:
   - `lane_count` موجودہ epoch میں فعال lanes کی مجموعی تعداد کے برابر ہونا چاہئے۔
   - `[[nexus.lane_catalog]]` اور `[[nexus.dataspace_catalog]]` کی ہر انٹری میں منفرد `index`/`id` اور متفقہ aliases ہونے چاہئیں۔ موجودہ global entries نہ ہٹائیں؛ اگر council نے اضافی data-spaces دیئے ہیں تو اپنے delegated aliases شامل کریں۔
   - ہر dataspace انٹری میں `fault_tolerance (f)` شامل ہونا یقینی بنائیں؛ lane-relay committees کا سائز `3f+1` ہوتا ہے۔
2. `[[nexus.routing_policy.rules]]` کو اپنی دی گئی پالیسی کے مطابق اپ ڈیٹ کریں۔ default template governance instructions کو lane `1` اور contract deployments کو lane `2` پر route کرتا ہے؛ قواعد شامل یا تبدیل کریں تاکہ آپ کے data-space کی ٹریفک درست lane اور alias پر جائے۔ قواعد کی ترتیب بدلنے سے پہلے Release Engineering کے ساتھ ہم آہنگی کریں۔
3. `[nexus.da]`, `[nexus.da.audit]`, اور `[nexus.da.recovery]` thresholds ریویو کریں۔ آپریٹرز سے توقع ہے کہ وہ council-approved ویلیوز رکھیں؛ صرف اسی وقت بدلیں جب نئی پالیسی منظور ہو۔
4. حتمی configuration کو اپنے operations tracker میں ریکارڈ کریں۔ dual-track release runbook onboarding ticket کے ساتھ موثر `config.toml` (secrets redacted) منسلک کرنے کا تقاضا کرتا ہے۔

## مرحلہ 5 - پری فلائٹ ویلیڈیشن
1. نیٹ ورک میں شامل ہونے سے پہلے built-in configuration validator چلائیں:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   یہ resolved configuration پرنٹ کرتا ہے اور اگر catalog/routing entries میں تضاد ہو یا genesis اور config نہ ملیں تو جلدی fail ہو جاتا ہے۔
2. اگر آپ containers deploy کرتے ہیں تو `docker load -i <profile>-<version>-<os>-image.tar` کے بعد وہی کمانڈ image کے اندر چلائیں ( `--sora` شامل کرنا نہ بھولیں)۔
3. logs میں placeholder lane/data-space identifiers کے warnings دیکھیں۔ اگر ملیں تو مرحلہ 4 پر واپس جائیں - پروڈکشن deployments کو templates کے placeholder IDs پر انحصار نہیں کرنا چاہئے۔
4. اپنا local smoke procedure چلائیں (مثلاً `iroha_cli` سے `FindNetworkStatus` query بھیجیں، تصدیق کریں کہ telemetry endpoints `nexus_lane_state_total` expose کرتے ہیں، اور streaming keys کی rotation/import کی تصدیق کریں)۔

## مرحلہ 6 - Cutover اور hand-off
1. تصدیق شدہ `manifest.json` اور signature artefacts کو release ticket میں محفوظ کریں تاکہ auditors آپ کی checks دوبارہ کر سکیں۔
2. Nexus Operations کو اطلاع دیں کہ نوڈ متعارف کرنے کے لئے تیار ہے؛ شامل کریں:
   - Node identity (peer ID, hostnames, Torii endpoint).
   - مؤثر lane/data-space catalog اور routing policy ویلیوز۔
   - Verified binaries/images کے hashes۔
3. حتمی peer admission (gossip seeds اور lane assignment) کو `@nexus-core` کے ساتھ کوآرڈینیٹ کریں۔ منظوری ملنے سے پہلے نیٹ ورک join نہ کریں؛ Sora Nexus deterministic lane occupancy نافذ کرتا ہے اور updated admissions manifest چاہتا ہے۔
4. نوڈ live ہونے کے بعد اپنے runbooks میں کی گئی overrides اپ ڈیٹ کریں اور release tag نوٹ کریں تاکہ اگلی iteration اسی baseline سے شروع ہو۔

## ریفرنس چیک لسٹ
- [ ] Release profile `iroha3` کے طور پر validate ہو چکا ہے۔
- [ ] Bundle/image کے hashes اور signatures verify ہو چکے ہیں۔
- [ ] Keys، peer addresses اور Torii endpoints پروڈکشن ویلیوز پر اپ ڈیٹ ہیں۔
- [ ] Nexus lane/dataspace catalog اور routing policy council assignment سے match کرتی ہے۔
- [ ] Configuration validator (`irohad --sora --config ... --trace-config`) بغیر warnings کے پاس کرتا ہے۔
- [ ] Manifests/signatures onboarding ticket میں آرکائیو اور Ops کو اطلاع دے دی گئی ہے۔

Nexus migration phases اور telemetry expectations کے وسیع تر سیاق کے لئے [Nexus transition notes](./nexus-transition-notes) دیکھیں۔
