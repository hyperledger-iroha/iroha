---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-ratchet-runbook
título: Como usar PQ Ratchet SoraNet
sidebar_label: Catraca Runbook PQ
descrição: Шаги ensaio de plantão para повышения или понижения стадийной Política de anonimato PQ с детерминированной telemetria валидацией.
---

:::nota História Canônica
Esta página está configurada para `docs/source/soranet/pq_ratchet_runbook.md`. Selecione uma cópia sincronizada, pois os documentos não serão exibidos na configuração.
:::

## Abençoado

Este runbook описывает последовательность firedrill para a política de anonimato pós-quântica (PQ) do estágio SoraNet. Os operadores realizam a promoção (Estágio A -> Estágio B -> Estágio C), e o rebaixamento controlado ocorre no Estágio B/A antes do fornecimento de PQ. Perfure ganchos de telemetria válidos (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) e colete artefatos para registro de ensaio de incidentes.

## Pré-requisitos

- Use o binário `sorafs_orchestrator` com ponderação de capacidade (commit равен или позже broca de referência из `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Coloque a pilha Prometheus/Grafana, que contém `dashboards/grafana/soranet_pq_ratchet.json`.
- Instantâneo do diretório de guarda nomeado. Получите и проверьте копию до drill:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Se o diretório de origem for publicado como JSON, verifique-o no binário Norito `soranet-directory build` para usar ajudantes de rotação.

- Obter metadados e artefatos de pré-estágio rotacionados pelo emissor com a CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Alterar janela одобрен on-call командами rede e observabilidade.

## Etapas da promoção

1. **Auditoria de etapa**

   Estágio inicial de ativação:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Antes da promoção, verifique `anon-guard-pq`.

2. **Promoção na Fase B (Maioria PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Подождите >=5 minutos para manifestação de manifestação.
   - Em Grafana (painel `SoraNet PQ Ratchet Drill`) убедитесь, este painel "Eventos de Política" coloca `outcome=met` para `stage=anon-majority-pq`.
   - Faça uma captura de tela ou um painel JSON e use o log de incidentes.

3. **Promoção no Estágio C (PQ Estrito)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verifique, este histograma `sorafs_orchestrator_pq_ratio_*` é 1.0.
   - Убедитесь, что brownout counter остается плоским; иначе выполните шаги rebaixamento.

## Exercício de rebaixamento/queda de energia

1. **Induцируйте синтетический дефицит PQ**

   Abra os relés PQ no playground, abra o diretório guard para as entradas clássicas e atualize o cache do orquestrador:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Apagamento de telemetria**

   - Painel: painel "Taxa de queda de energia" tem valor 0.
   -PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` é compatível com `anonymity_outcome="brownout"` com `anonymity_reason="missing_majority_pq"`.

3. **Rebaixamento para Estágio B / Estágio A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Se o fornecimento de PQ for necessário, consulte `anon-guard-pq`. Perfurar завершен, contadores de queda de energia стабилизируются e promoções podem ser mais úteis.

4. **Diretório de proteção de segurança**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetria e artefatos- **Painel:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** убедитесь, что alerta de queda de energia `sorafs_orchestrator_policy_events_total` остается ниже настроенного SLO (&lt;5% em 10 minutos de outubro).
- **Registro de incidentes:** use trechos de telemetria e registre-os em `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Captura assinada:** используйте `cargo xtask soranet-rollout-capture`, чтобы скопировать log de perfuração e placar em `artifacts/soranet_pq_rollout/<timestamp>/`, вычислить BLAKE3 digests e создать подписанный `rollout_capture.json`.

Exemplo:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Use metadados de geração e assinatura de pacote de governança.

## Reversão

Se o drill for realmente novo PQ, você estará no Estágio A, usará Networking TL e usará métricas sob medida para o guard directory diffs e o rastreador de incidentes. Ao usar a função de exportação de diretório de proteção, você mantém o serviço normal.

:::tip Cobertura de regressão
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` предоставляет validação sintética, которая поддерживает этот broca.
:::