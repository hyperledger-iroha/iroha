---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elastic-lane
título: Via elástica (NX-7)
sidebar_label: Faixa elástica usada
description: Bootstrap-proцесс для создания манифестов lane Nexus, записей каталога и доказательств rollout.
---

:::nota História Canônica
Esta página contém `docs/source/nexus_elastic_lane.md`. Selecione uma cópia sincronizada, pois a transferência não será exibida no portal.
:::

# Набор инструментов для эластичного выделения lane (NX-7)

> **Roteiro de Пункт:** NX-7 - инструменты эластичного выделения lane  
> **Статус:** инструменты завершены - генерируют манифесты, фрагменты каталога, cargas úteis Norito, testes de fumaça,
> um auxiliar para o pacote de teste de carga теперь связывает гейтинг по latência de slot + манифесты доказательств, чтобы прогоны нагрузки валидаторов
> Você pode publicar scripts sem cast.

Este é o tipo de operação que fornece o novo ajudante `scripts/nexus_lane_bootstrap.sh`, который автоматизирует генерацию манифестов lane, Implementação de faixa de catálogo de fragmentos/espaço de dados e distribuição. Цель - легко поднимать новые Nexus pistas (públicas ou privadas) sem ручного редактирования нескольких файлов и без ручного пересчета геометрии каталога.

## 1. Treino de segurança

1. Governança - одобрение для alias lane, espaço de dados, набора валидаторов, tolerância a falhas (`f`) e liquidação política.
2. Финальный список валидаторов (IDs de conta) e список защищенных namespaces.
3. Ao abrir o repositório de configuração, você cria fragmentos de geração.
4. Coloque para restaurar a pista do Manipheстов (como `nexus.registry.manifest_directory` e `cache_directory`).
5. Entre em contato com os identificadores de telemetria/PagerDuty para a pista, esses alertas podem ser encontrados on-line.

## 2. Pista de Artefatos de Сгенерируйте

Запустите helper из корня репозитория:

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

Bandeiras de sucesso:

- `--lane-id` é atualizado com o índice de novas descrições em `nexus.lane_catalog`.
- `--dataspace-alias` e `--dataspace-id/hash` управляют записью dataspace no catálogo (por умолчанию используется id lane).
- `--validator` pode ser instalado ou removido de `--validators-file`.
- `--route-instruction` / `--route-account` é um produto que está pronto para ser comercializado.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) фиксируют контакты runbook, чтобы dashboards показывали правильных владельцев.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` добавляют hook runtime-upgrade em манифест, когда lane требует расширенных операторских контролей.
- `--encode-space-directory` é usado automaticamente para `cargo xtask space-directory encode`. Use o `--space-directory-out`, exceto o código de código `.to` no mesmo local.

Script criado por um artefacto em `--output-dir` (por meio do catálogo de tecnologia) e opcionalmente fornecido por codificação включенном:1. `<slug>.manifest.json` - манифест lane с quorum валидаторов, zaщищенными namespaces e опциональными метаданными hook runtime-upgrade.
2. `<slug>.catalog.toml` - TOML-фрагмент с `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` e запрошенными правилами маршрутизации. Verifique se o `fault_tolerance` está localizado no espaço de dados, ele configurou o comutador de relé de pista (`3f+1`).
3. `<slug>.summary.json` - audit-сводка с геометрией (slug, сегменты, метаданные), требуемыми шагами rollout и точной командой `cargo xtask space-directory encode` (por `space_directory_encode.command`). Use este JSON como ticket de integração para doação.
4. `<slug>.manifest.to` - instalado por `--encode-space-directory`; готов для потока Torii `iroha app space-directory manifest publish`.

Use `--dry-run`, use JSON/фрагменты para criar arquivos e `--force` para configuração artefactos originais.

## 3. Configuração de configuração

1. Limpe o arquivo JSON no local `nexus.registry.manifest_directory` (e no diretório de cache, exceto o registro para armazenar pacotes configuráveis). Certifique-se de que não há versão manifestada nos repositórios de configuração.
2. Insira o catálogo de fragmentos em `config/config.toml` (ou em `config.d/*.toml`). Verifique se o `nexus.lane_count` não é o `lane_id + 1`, e atualize o `nexus.routing_policy.rules`, pois este é o caso. pista nova.
3. Abra (proposta `--encode-space-directory`) e abra o arquivo no Space Directory, usando o comando de resumo (`space_directory_encode.command`). Isso é `.manifest.to`, который ожидает Torii, e фиксирует доказательства para аудиторов; Execute o `iroha app space-directory manifest publish`.
4. Abra `irohad --sora --config path/to/config.toml --trace-config` e atualize seu rastreamento na implementação do ticket. Isto é possível, esta nova geometria é baseada no segmento slug/Kura do gerador.
5. Selecione os validadores, coloque-os na pista e altere a configuração da manifestação/caso. Solicite um resumo JSON no tíquete para usuários auditados.

## 4. Соберите bundle распределения реестра

Упакуйте сгенерированный манифест и overlay, чтобы операторы могли распространять данные governança по lanes без правок конфигов na каждом хосте. Helper упаковки копирует манифесты в канонический layout, создает опциональный sobreposição de catálogo de governança para `nexus.registry.cache_directory` e может собрать tarball para off-line:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

O que você precisa:

1. `manifests/<slug>.manifest.json` - copie-o para o `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - classificado em `nexus.registry.cache_directory`. Каждая запись `--module` становится подключаемым определением модуля, позволяя замену governança-módulo (NX-2) через sobreposição de cache обновление вместо редактирования `config.toml`.
3. `summary.json` - содержит хеши, метаданные overlay e инструкции para operações.
4. Opcional `registry_bundle.tar.*` - dispositivo para SCP, S3 ou artefactos de rastreamento.

Синхронизируйте весь каталог (или архив) на каждый валидатор, распакуйте на air-gapped хостах и скопируйте манифесты + sobreposição de cache em sua configuração inicial Torii.

## 5. Testes de fumaça validadosПосле перезапуска Torii запустите новый smoke helper, чтобы проверить, что lane сообщает `manifest_ready=true`, métrica показывают ожидаемое количество pistas e medidor selado. Lanes, которым нужны манифесты, обязаны иметь непустой `manifest_path`; helper теперь мгновенно падает при отсутствии пути, чтобы каждый деплой NX-7 включал доказательство подписанного manifestação:

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

Adicione `--insecure` através de uma assinatura autoassinada. O script não contém nenhum código, exceto faixa aberta, selado, ou métrica/telemetria de faixa com precisão значениями. Use `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` e `--max-headroom-events`, quais são os telefones instalados na pista (você precisa de um bloco/financiamento/backlog/headroom) na área de armazenamento, e é baseado em `--max-slot-p95` / `--max-slot-p99` (e `--min-slot-samples`) para instalar o NX-18 em um slot específico, você não precisa de um ajudante.

Para um dispositivo air-gapped (ou CI) você pode usar o Torii para obter o endpoint atual:

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

Acessar fixtures em `fixtures/nexus/lanes/` отражают артефакты, создаваемые bootstrap helper, чтобы новые манифесты можно было lint-ить Não há scripts personalizados. CI гоняет тот же поток через `ci/check_nexus_lane_smoke.sh` e `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`), чтобы убедиться, что smoke helper NX-7 остается совместимым опубликованным форматом payload e что digests/overlays bundle остаются воспроизводимыми.