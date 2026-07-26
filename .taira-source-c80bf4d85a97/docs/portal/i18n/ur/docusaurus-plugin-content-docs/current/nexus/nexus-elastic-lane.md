---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-elastic-lane
title: لچکدار lane پروویژنگ (NX-7)
sidebar_label: لچکدار lane پروویژنگ
description: Nexus lane manifests، catalog entries، اور rollout evidence بنانے کے لئے bootstrap ورک فلو۔
---

:::note Canonical Source
یہ صفحہ `docs/source/nexus_elastic_lane.md` کی عکاسی کرتا ہے۔ جب تک ترجمہ پورٹل تک نہیں پہنچتا، دونوں کاپیوں کو aligned رکھیں۔
:::

# لچکدار lane پروویژنگ ٹول کٹ (NX-7)

> **Roadmap item:** NX-7 - لچکدار lane پروویژنگ tooling  
> **Status:** tooling مکمل - manifests، catalog snippets، Norito payloads، smoke tests بناتا ہے،
> اور load-test bundle helper اب slot latency gating + evidence manifests کو جوڑتا ہے تاکہ validators کے load runs
> بغیر مخصوص scripting کے شائع کیے جا سکیں۔

یہ گائیڈ operators کو نئے `scripts/nexus_lane_bootstrap.sh` helper کے ذریعے لے جاتی ہے جو lane manifest generation، lane/dataspace catalog snippets، اور rollout evidence کو خودکار بناتا ہے۔ مقصد یہ ہے کہ نئی Nexus lanes (public یا private) متعدد فائلیں دستی طور پر edit کیے بغیر اور catalog geometry دوبارہ ہاتھ سے derive کیے بغیر آسانی سے بنائی جا سکیں۔

## 1. Prerequisites

1. lane alias، dataspace، validator set، fault tolerance (`f`)، اور settlement policy کے لئے governance approval۔
2. validators کی حتمی فہرست (account IDs) اور protected namespaces کی فہرست۔
3. node configuration repository تک رسائی تاکہ generated snippets شامل کیے جا سکیں۔
4. lane manifest registry کے لئے paths (دیکھیں `nexus.registry.manifest_directory` اور `cache_directory`).
5. lane کے لئے telemetry contacts/PagerDuty handles تاکہ alerts lane کے online ہوتے ہی wired ہو سکیں۔

## 2. lane artefacts بنائیں

repository root سے helper چلائیں:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Key flags:

- `--lane-id` کو `nexus.lane_catalog` میں نئے entry کے index سے match ہونا چاہیے۔
- `--dataspace-alias` اور `--dataspace-id/hash` dataspace catalog entry کو control کرتے ہیں (omit ہونے پر default lane id استعمال ہوتا ہے).
- `--validator` کو repeat کیا جا سکتا ہے یا `--validators-file` سے پڑھا جا سکتا ہے۔
- `--route-instruction` / `--route-account` ready-to-paste routing rules emit کرتے ہیں۔
- `--metadata key=value` (یا `--telemetry-contact/channel/runbook`) runbook contacts capture کرتے ہیں تاکہ dashboards درست owners دکھائیں۔
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` runtime-upgrade hook کو manifest میں شامل کرتے ہیں جب lane کو extended operator controls درکار ہوں۔
- `--encode-space-directory` خودکار طور پر `cargo xtask space-directory encode` چلاتا ہے۔ `--space-directory-out` کے ساتھ استعمال کریں جب `.to` فائل کو default کے علاوہ کسی اور جگہ رکھنا ہو۔

اس اسکرپٹ سے `--output-dir` کے اندر تین artefacts بنتے ہیں (default موجودہ directory ہے)، اور encoding فعال ہونے پر ایک چوتھا بھی بنتا ہے:

1. `<slug>.manifest.json` - lane manifest جس میں validator quorum، protected namespaces، اور runtime-upgrade hook metadata شامل ہو سکتا ہے۔
2. `<slug>.catalog.toml` - ایک TOML snippet جس میں `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`, اور مطلوبہ routing rules ہوتے ہیں۔ dataspace entry میں `fault_tolerance` لازمی طور پر set کریں تاکہ lane-relay committee (`3f+1`) درست سائز ہو۔
3. `<slug>.summary.json` - audit summary جو geometry (slug, segments, metadata) کے ساتھ required rollout steps اور `cargo xtask space-directory encode` کا exact command ( `space_directory_encode.command` کے تحت) بیان کرتا ہے۔ اسے onboarding ticket کے ساتھ evidence کے طور پر attach کریں۔
4. `<slug>.manifest.to` - `--encode-space-directory` فعال ہونے پر بنتا ہے؛ Torii کے `iroha app space-directory manifest publish` flow کے لئے ready ہے۔

`--dry-run` سے JSON/snippets کو بغیر فائل لکھے preview کریں، اور `--force` سے موجودہ artefacts overwrite کریں۔

## 3. تبدیلیاں لاگو کریں

1. manifest JSON کو configured `nexus.registry.manifest_directory` میں کاپی کریں (اور cache directory میں بھی اگر registry remote bundles mirror کرتا ہے). اگر manifests configuration repo میں versioned ہوں تو فائل commit کریں۔
2. catalog snippet کو `config/config.toml` (یا مناسب `config.d/*.toml`) میں append کریں۔ `nexus.lane_count` کم از کم `lane_id + 1` ہونا چاہیے اور نئے lane کے لئے `nexus.routing_policy.rules` اپ ڈیٹ کریں۔
3. Encode کریں (اگر `--encode-space-directory` چھوڑا تھا) اور Space Directory میں manifest publish کریں۔ summary میں موجود command (`space_directory_encode.command`) استعمال کریں۔ اس سے `.manifest.to` payload بنتا ہے اور auditors کے لئے evidence ریکارڈ ہوتا ہے؛ `iroha app space-directory manifest publish` کے ذریعے submit کریں۔
4. `irohad --sora --config path/to/config.toml --trace-config` چلائیں اور trace output کو rollout ticket میں archive کریں۔ یہ ثابت کرتا ہے کہ نئی geometry generated slug/Kura segments کے مطابق ہے۔
5. manifest/catalog تبدیلیاں deploy ہونے کے بعد lane کے لئے assigned validators کو restart کریں۔ future audits کے لئے summary JSON کو ticket میں رکھیں۔

## 4. registry distribution bundle بنائیں

generated manifest اور overlay کو package کریں تاکہ operators ہر host پر configs edit کیے بغیر lane governance data distribute کر سکیں۔ bundler helper manifests کو canonical layout میں کاپی کرتا ہے، `nexus.registry.cache_directory` کے لئے governance catalog overlay بناتا ہے، اور offline transfers کے لئے tarball نکال سکتا ہے:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Outputs:

1. `manifests/<slug>.manifest.json` - انہیں configured `nexus.registry.manifest_directory` میں کاپی کریں۔
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` میں رکھیں۔ ہر `--module` entry ایک pluggable module definition بن جاتی ہے، جس سے governance-module swap-outs (NX-2) ممکن ہوتے ہیں اور صرف cache overlay اپ ڈیٹ کرنا پڑتا ہے، `config.toml` میں ترمیم نہیں۔
3. `summary.json` - hashes، overlay metadata، اور operator instructions شامل ہیں۔
4. Optional `registry_bundle.tar.*` - SCP، S3، یا artifact trackers کے لئے ready۔

مکمل directory (یا archive) کو ہر validator تک sync کریں، air-gapped hosts پر extract کریں، اور Torii restart سے پہلے manifests + cache overlay کو ان کی registry paths میں کاپی کریں۔

## 5. Validator smoke tests

Torii restart کے بعد نیا smoke helper چلائیں تاکہ lane `manifest_ready=true` رپورٹ کرے، metrics میں expected lane count نظر آئے، اور sealed gauge صاف ہو۔ جن lanes کو manifests درکار ہوں انہیں non-empty `manifest_path` ظاہر کرنا چاہیے؛ helper اب path غائب ہونے پر فوراً fail ہوتا ہے تاکہ ہر NX-7 deployment میں signed manifest evidence شامل ہو:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Self-signed environments میں ٹیسٹ کرتے وقت `--insecure` شامل کریں۔ اگر lane غائب ہو، sealed ہو، یا metrics/telemetry expected values سے drift کریں تو script non-zero exit کرتا ہے۔ `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog`, اور `--max-headroom-events` استعمال کریں تاکہ lane-level block height/finality/backlog/headroom telemetery آپ کے operational envelope میں رہے، اور `--max-slot-p95` / `--max-slot-p99` (ساتھ `--min-slot-samples`) کے ساتھ ملا کر NX-18 slot-duration targets helper کے اندر ہی enforce کریں۔

Air-gapped validations (یا CI) کے لئے آپ live endpoint پر جانے کے بجائے captured Torii response replay کر سکتے ہیں:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/` کے recorded fixtures bootstrap helper کے تیار کردہ artefacts کی عکاسی کرتے ہیں تاکہ نئے manifests کو بغیر مخصوص scripting کے lint کیا جا سکے۔ CI اسی flow کو `ci/check_nexus_lane_smoke.sh` اور `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) کے ذریعے چلاتا ہے تاکہ ثابت ہو کہ NX-7 smoke helper شائع شدہ payload format کے مطابق رہتا ہے اور bundle digests/overlays reproducible رہتے ہیں۔
