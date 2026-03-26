---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elastic-lane
título: لچکدار lane پروویژنگ (NX-7)
sidebar_label: لچکدار lane پروویژنگ
description: Manifestos de pista Nexus, entradas de catálogo, e evidências de implementação
---

:::nota Fonte Canônica
یہ صفحہ `docs/source/nexus_elastic_lane.md` کی عکاسی کرتا ہے۔ جب تک ترجمہ پورٹل تک نہیں پہنچتا, دونوں کاپیوں کو alinhamento رکھیں۔
:::

# لچکدار lane پروویژنگ ٹول کٹ (NX-7)

> **Item de roteiro:** NX-7 - لچکدار lane پروویژنگ ferramentas  
> **Status:** ferramentas مکمل - manifestos, trechos de catálogo, cargas úteis Norito, testes de fumaça بناتا ہے،
> اور load-test bundle helper اب slot latency gating + manifestos de evidências کو جوڑتا ہے تاکہ validadores کے execuções de carga
> بغیر مخصوص scripts کے شائع کیے جا سکیں۔

یہ گائیڈ operadores کو نئے `scripts/nexus_lane_bootstrap.sh` helper کے ذریعے لے جاتی ہے جو geração de manifesto de pista, trechos de catálogo de pista/espaço de dados, e evidência de implementação کو خودکار بناتا ہے۔ مقصد یہ ہے کہ نئی Nexus pistas (pública ou privada) متعدد فائلیں دستی طور پر editar کیے بغیر اور geometria do catálogo دوبارہ ہاتھ سے derivar کیے بغیر آسانی سے بنائی جا سکیں۔

## 1. Pré-requisitos

1. alias de pista, espaço de dados, conjunto de validador, tolerância a falhas (`f`), e política de liquidação کے لئے aprovação de governança۔
2. validadores کی حتمی فہرست (IDs de conta) e namespaces protegidos کی فہرست۔
3. repositório de configuração de nó تک رسائی تاکہ trechos gerados شامل کیے جا سکیں۔
4. registro de manifesto de pista para caminhos (دیکھیں `nexus.registry.manifest_directory` e `cache_directory`).
5. pista کے لئے contatos de telemetria / identificadores PagerDuty تاکہ pista de alertas کے online ہوتے ہی com fio ہو سکیں۔

## 2. artefatos de pista

raiz do repositório ou ajudante چلائیں:

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

Principais sinalizadores:

- `--lane-id` کو `nexus.lane_catalog` میں نئے entrada کے índice سے correspondência ہونا چاہیے۔
- `--dataspace-alias` e `--dataspace-id/hash` entrada de catálogo de espaço de dados کو control کرتے ہیں (omitir ہونے پر default lane id استعمال ہوتا ہے).
- `--validator` کو repetir کیا جا سکتا ہے یا `--validators-file` سے پڑھا جا سکتا ہے۔
- Regras de roteamento prontas para colar `--route-instruction` / `--route-account` emitem کرتے ہیں۔
- `--metadata key=value` (یا `--telemetry-contact/channel/runbook`) captura de contatos do runbook کرتے ہیں تاکہ painéis درست proprietários دکھائیں۔
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` gancho de atualização de tempo de execução کو manifesto میں شامل کرتے ہیں جب lane کو controles de operador estendidos درکار ہوں۔
- `--encode-space-directory` خودکار طور پر `cargo xtask space-directory encode` چلاتا ہے۔ `--space-directory-out` کے ساتھ استعمال کریں جب `.to` فائل کو padrão کے علاوہ کسی اور جگہ رکھنا ہو۔

اس اسکرپٹ سے `--output-dir` کے اندر تین artefatos بنتے ہیں (diretório موجودہ ہے) ، اور codificação فعال ہونے پر O que você precisa saber:

1. `<slug>.manifest.json` - manifesto de pista جس میں quorum de validador, namespaces protegidos, e metadados de gancho de atualização de tempo de execução شامل ہو سکتا ہے۔
2. `<slug>.catalog.toml` - ایک Snippet TOML جس میں `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`, اور مطلوبہ regras de roteamento ہوتے ہیں۔ entrada de espaço de dados میں `fault_tolerance` لازمی طور پر set کریں تاکہ comitê de retransmissão de pista (`3f+1`) درست سائز ہو۔
3. `<slug>.summary.json` - resumo de auditoria e geometria (slug, segmentos, metadados) کے ساتھ etapas de implementação necessárias اور `cargo xtask space-directory encode` کا comando exato ( `space_directory_encode.command` کے تحت) بیان کرتا ہے۔ اسے bilhete de embarque کے ساتھ evidência کے طور پر anexar کریں۔
4. `<slug>.manifest.to` - `--encode-space-directory` فعال ہونے پر بنتا ہے؛ Torii کے `iroha app space-directory manifest publish` fluxo کے لئے pronto ہے۔

`--dry-run` سے JSON/snippets کو بغیر فائل لکھے preview کریں, اور `--force` سے موجودہ artefatos sobrescrever کریں۔

## 3. تبدیلیاں لاگو کریں

1. manifesto JSON کو configurado `nexus.registry.manifest_directory` میں کاپی کریں (diretório de cache میں بھی اگر espelho de pacotes remotos do registro کرتا ہے). اگر manifesta o repositório de configuração میں versionado ہوں ou فائل commit کریں۔
2. snippet de catálogo کو `config/config.toml` (یا مناسب `config.d/*.toml`) میں anexar کریں۔ `nexus.lane_count` کم از کم `lane_id + 1` ہونا چاہیے اور نئے کے لئے `nexus.routing_policy.rules` اپ ڈیٹ کریں۔
3. Codificar کریں (اگر `--encode-space-directory` چھوڑا تھا) e Space Directory میں manifesto publicar کریں۔ resumo میں موجود comando (`space_directory_encode.command`) استعمال کریں۔ A carga útil `.manifest.to` é fornecida por auditores que possuem evidências ریکارڈ ہوتا ہے؛ `iroha app space-directory manifest publish` کے ذریعے enviar کریں۔
4. `irohad --sora --config path/to/config.toml --trace-config` چلائیں اور saída de rastreamento کو rollout ticket میں arquivo کریں۔ یہ ثابت کرتا ہے کہ نئی segmentos de slug/Kura gerados por geometria کے مطابق ہے۔
5. manifesto/catálogo تبدیلیاں implantar ہونے کے بعد lane کے لئے validadores atribuídos کو reiniciar کریں۔ auditorias futuras کے لئے resumo JSON کو ticket میں رکھیں۔

## 4. pacote de distribuição de registromanifesto gerado اور overlay کو pacote کریں تاکہ operadores ہر host پر configurações editar کیے بغیر dados de governança de pista distribuir کر سکیں۔ manifestos auxiliares do bundler کو layout canônico میں کاپی کرتا ہے، `nexus.registry.cache_directory` کے لئے sobreposição de catálogo de governança بناتا ہے, اور transferências offline کے لئے tarball نکال O que é:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Saídas:

1. `manifests/<slug>.manifest.json` - انہیں configurado `nexus.registry.manifest_directory` میں کاپی کریں۔
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` میں رکھیں۔ ہر Entrada `--module` ایک definição de módulo conectável بن جاتی ہے، جس سے trocas de módulo de governança (NX-2) ممکن ہوتے ہیں اور صرف sobreposição de cache اپ ڈیٹ کرنا پڑتا ہے, `config.toml` میں ترمیم نہیں۔
3. `summary.json` - hashes, metadados de sobreposição, instruções do operador e instruções do operador
4. `registry_bundle.tar.*` opcional - SCP, S3, rastreadores de artefatos کے لئے prontos۔

Diretório مکمل (arquivo یا) کو ہر validador تک sincronização کریں, hosts air-gapped پر extrair کریں, اور Torii reiniciar سے پہلے manifestos + sobreposição de cache کو ان کی caminhos de registro میں کاپی کریں۔

## 5. Testes de fumaça do validador

Torii reiniciar کے بعد نیا smoke helper چلائیں تاکہ pista `manifest_ready=true` رپورٹ کرے, métricas میں contagem de faixa esperada نظر آئے, اور medidor selado صاف ہو۔ جن pistas کو manifestos درکار ہوں انہیں `manifest_path` não vazio ظاہر کرنا چاہیے؛ helper اب path غائب ہونے پر فوراً fail ہوتا ہے تاکہ ہر Implantação NX-7 میں evidência de manifesto assinada شامل ہو:

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

Ambientes autoassinados میں ٹیسٹ کرتے وقت `--insecure` شامل کریں۔ اگر lane غائب ہو، selado ہو، یا valores esperados de métricas/telemetria سے desvio کریں تو script de saída diferente de zero کرتا ہے۔ `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog`, اور `--max-headroom-events` استعمال کریں تاکہ altura do bloco no nível da pista/finalidade/backlog/headroom telemetria آپ کے envelope operacional میں رہے، اور `--max-slot-p95` / `--max-slot-p99` (ساتھ `--min-slot-samples`) کے ساتھ ملا کر Auxiliar de metas de duração de slot NX-18 کے اندر ہی impor کریں۔

Validações air-gapped (یا CI) کے لئے آپ endpoint ao vivo پر جانے کے بجائے capturada repetição de resposta Torii کر سکتے ہیں:

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

`fixtures/nexus/lanes/` کے auxiliar de bootstrap de luminárias gravadas کے تیار کردہ artefatos کی عکاسی کرتے ہیں تاکہ نئے manifestos کو بغیر مخصوص scripts کے lint کیا جا سکے۔ CI اسی flow کو `ci/check_nexus_lane_smoke.sh` اور `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) کے ذریعے چلاتا ہے تاکہ ثابت ہو کہ NX-7 smoke helper شائع شدہ formato de carga útil کے مطابق رہتا ہے اور resumos/sobreposições de pacote reproduzíveis رہتے ہیں۔