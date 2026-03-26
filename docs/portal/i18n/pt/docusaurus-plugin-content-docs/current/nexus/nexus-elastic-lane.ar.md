---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elastic-lane
título: تجهيز lane المرن (NX-7)
sidebar_label: Lane lane المرن
description: Use o bootstrap para manifestar a pista Nexus e implementá-lo.
---

:::note المصدر الرسمي
Verifique o valor `docs/source/nexus_elastic_lane.md`. Verifique se o produto está funcionando corretamente.
:::

# مجموعة ادوات تجهيز lane المرن (NX-7)

> **عنصر خارطة الطريق:** NX-7 - ادوات تجهيز lane المرن  
> **الحالة:** الادوات مكتملة - Manifestos de تولد, مقتطفات الكتالوج, حمولات Norito, اختبارات fumaça,
> ومساعد pacote لاختبارات الحمل يجمع الان بوابة زمن الاستجابة لكل slot + manifestos الادلة كي تنشر اختبارات حمل المدققين
> دون سكربتات مخصصة.

هذا الدليل يوجه المشغلين عبر المساعد الجديد `scripts/nexus_lane_bootstrap.sh` الذي يقوم بأتمتة توليد manifest لـ lane ومقتطفات Selecione lane/dataspace e rollout. As faixas de rodagem são desviadas para Nexus (de um lado para o outro) Faça isso com cuidado.

## 1. المتطلبات المسبقة

1. موافقة الحوكمة على alias لـ lane e dataspace ومجموعة المدققين وتحمّل الاعطال (`f`) وسياسة liquidação.
2. قائمة نهائية بالمدققين (معرفات الحسابات) e namespaces المحمية.
3. Faça o download do seu telefone com antecedência para obter mais informações.
4. Verifique se os manifestos estão na pista (`nexus.registry.manifest_directory` e `cache_directory`).
5. جهات اتصال للقياس عن بعد/مفاتيح PagerDuty الخاصة بالlane حتى يمكن ربط التنبيهات بمجرد دخول lane الى الخدمة.

## 2. Artefatos توليد na pista

شغّل المساعد من جذر المستودع:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Como fazer:

- `--lane-id` é um índice de referência para `nexus.lane_catalog`.
- `--dataspace-alias` e `--dataspace-id/hash` são inseridos no espaço de dados do espaço de dados (é o ID do local em que está a pista).
- `--validator` é um dispositivo de segurança e um cabo de `--validators-file`.
- `--route-instruction` / `--route-account` تصدر قواعد توجيه جاهزة للصق.
- `--metadata key=value` (e `--telemetry-contact/channel/runbook`) permite que você verifique o runbook do seu computador.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` تضيف hook runtime-upgrade الى manifest عندما يحتاج lane الى ضوابط تشغيل ممتدة.
- `--encode-space-directory` é compatível com `cargo xtask space-directory encode`. Verifique se o `--space-directory-out` está disponível e se o `.to` está no lugar certo.

ينتج السكربت ثلاثة artefatos داخل `--output-dir` (الافتراضي هو مجلد الحالي), مع رابع اختياري عند Codificação simples:

1. `<slug>.manifest.json` - manifesto para lane يحتوي quorum المدققين e namespaces المحمية وبيانات اختيارية para gancho runtime-upgrade.
2. `<slug>.catalog.toml` - مقتطف TOML يحتوي `[[nexus.lane_catalog]]` e `[[nexus.dataspace_catalog]]` واي قواعد توجيه مطلوبة. Use o `fault_tolerance` no espaço de dados para usar o lane-relay (`3f+1`).
3. `<slug>.summary.json` - ملخص تدقيق يصف الهندسة (slug والقطاعات والبيانات) وخطوات rollout المطلوبة والامر Nome `cargo xtask space-directory encode` (nome `space_directory_encode.command`). O JSON é configurado para onboarding.
4. `<slug>.manifest.to` - `--encode-space-directory`; Remova `iroha app space-directory manifest publish` de Torii.

`--dry-run` é definido como JSON/المقتطفات, mas `--force` é definido como `--force`. artefatos الموجودة.

## 3. تطبيق التغييرات

1. Crie o manifesto JSON no `nexus.registry.manifest_directory` do diretório (o diretório de cache e o registro não são remotos). التزم بالملف اذا كانت manifesta تُدار بالنسخ في مستودع التهيئة.
2. Coloque o arquivo em `config/config.toml` (e `config.d/*.toml`). Verifique se o `nexus.lane_count` está no `lane_id + 1` e se o `nexus.routing_policy.rules` está na pista.
3. O arquivo (`--encode-space-directory`) é o manifesto do Space Directory que contém o resumo (`space_directory_encode.command`). A carga útil `.manifest.to` é a carga útil Torii e é usada para carregar o Torii. Use o `iroha app space-directory manifest publish`.
4. Use `irohad --sora --config path/to/config.toml --trace-config` e execute o rastreamento durante a implementação. هذا يثبت ان الهندسة الجديدة تطابق slug/قطاعات Kura المولدة.
5. Abra o arquivo manifest/الكتالوج. Use o JSON de resumo para criar o arquivo.

## 4. Nosso pacote de pacotes

Você pode usar o manifesto e a sobreposição em todas as faixas e nas configurações para definir as configurações. O bundler não é manifesto, mas o overlay é o `nexus.registry.cache_directory`. Baixar tarball offline:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Informações:1. `manifests/<slug>.manifest.json` - Verifique o código `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - é usado em `nexus.registry.cache_directory`. Para usar `--module`, você pode usar o dispositivo de segurança para o computador (NX-2) A sobreposição de sobreposição deve ser feita através do `config.toml`.
3. `summary.json` - Crie hashes e sobreposições e sobreponha-os.
4. Use `registry_bundle.tar.*` - Remova artefatos de SCP e S3 e de armazenamento.

زامن المجلد بالكامل (او الارشيف) لكل مدقق, وفكّه على hosts معزولة, وانسخ manifests + overlay الكاش الى مسارات السجل Verifique o Torii.

## 5. Fumar para fumar

Para obter mais informações sobre Torii, remova a fumaça da máquina de fumaça na faixa de rodagem `manifest_ready=true`, وان As pistas devem ser fechadas e as pistas seladas. يجب ان تعرض lanes التي تتطلب manifestos قيمة `manifest_path` غير فارغة؛ ويفشل المساعد مباشرة عند غياب المسار حتى يتضمن كل نشر NX-7 دليل manifest الموقع:

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

O `--insecure` é autoassinado. Verifique se a pista está fechada e selada e selada e selada. المتوقعة. Use `--min-block-height` e `--max-finality-lag` e `--max-settlement-backlog` e `--max-headroom-events` para passar pela pista (ارتفاع ضمن حدود التشغيل, واربطها مع `--max-slot-p95` / `--max-slot-p99` (مع `--min-slot-samples`) لفرض Este slot é compatível com o NX-18 e é compatível com o slot.

O dispositivo air-gapped (او CI) é usado para configurar o Torii para o endpoint:

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

Os fixtures são usados ​​para `fixtures/nexus/lanes/` e os artefatos são armazenados no bootstrap e nos manifestos de lint. Não. تنفذ CI نفس التدفق عبر `ci/check_nexus_lane_smoke.sh` e `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) لاثبات ان مساعد smoke الخاص بـ NX-7 Não há nenhuma carga útil útil e nenhum resumo/sobreposição para o pacote empacotado no site.