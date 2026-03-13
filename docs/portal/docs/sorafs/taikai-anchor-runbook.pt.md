---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 50261b1f3173cd3916b29c81e85cc92ed8c14c38a0e0296be38397fe9b5c0596
source_last_modified: "2025-11-21T18:08:23.480735+00:00"
translation_last_reviewed: 2026-01-30
---

# Runbook de observabilidade do ancoramento Taikai

Esta copia do portal espelha o runbook canonico em
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md).
Use-a ao ensaiar ancoras de routing-manifest (TRM) do SN13-C para que operadores de
SoraFS/SoraNet possam correlacionar artefatos de spool, telemetria Prometheus e
provas de governanca sem sair do preview do portal.

## Escopo e owners

- **Programa:** SN13-C — manifests Taikai e ancoras SoraNS.
- **Owners:** Media Platform WG, DA Program, Networking TL, Docs/DevRel.
- **Objetivo:** Fornecer um playbook deterministico para alertas Sev 1/Sev 2,
  validacao de telemetria e captura de evidencias enquanto os routing manifests
  Taikai avancam pelos aliases.

## Quickstart (Sev 1/Sev 2)

1. **Capture artefatos de spool** — copie os arquivos mais recentes
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json` e
   `taikai-lineage-*.json` de
   `config.da_ingest.manifest_store_dir/taikai/` antes de reiniciar workers.
2. **Dump de telemetria `/status`** — registre o array
   `telemetry.taikai_alias_rotations` para provar qual janela de manifest esta ativa:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **Verifique dashboards e alertas** — carregue
   `dashboards/grafana/taikai_viewer.json` (filtros de cluster + stream) e observe
   se alguma regra em
   `dashboards/alerts/taikai_viewer_rules.yml` disparou (`TaikaiLiveEdgeDrift`,
   `TaikaiIngestFailure`, `TaikaiCekRotationLag`, eventos de saude PoR da SoraFS).
4. **Inspecione Prometheus** — rode as queries da secao "Referencia de metricas"
   para confirmar que a latencia/deriva de ingest e os contadores de rotacao de alias
   se comportam como esperado. Escale se
   `taikai_trm_alias_rotations_total` estagnar por varias janelas ou se os contadores
   de erro aumentarem.

## Referencia de metricas

| Metrica | Proposito |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | Histograma de latencia de ingest CMAF por cluster/stream (alvo: p95 < 750 ms, p99 < 900 ms). |
| `taikai_ingest_live_edge_drift_ms` | Drift de live-edge entre encoder e workers de ancoragem (pagina se p99 > 1.5 s por 10 min). |
| `taikai_ingest_segment_errors_total{reason}` | Contadores de erro por razao (`decode`, `manifest_mismatch`, `lineage_replay`, ...). Qualquer aumento dispara `TaikaiIngestFailure`. |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | Incrementa sempre que `/v2/da/ingest` aceita um novo TRM para um alias; use `rate()` para validar a cadencia de rotacao. |
| `/status → telemetry.taikai_alias_rotations[]` | Snapshot JSON com `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total` e timestamps para bundles de evidencia. |
| `taikai_viewer_*` (rebuffer, idade de rotacao CEK, saude PQ, alertas) | KPIs do viewer para garantir que rotacao CEK + circuitos PQ permanecem saudaveis durante os anchors. |

### Snippets PromQL

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## Dashboards e alertas

- **Grafana viewer board:** `dashboards/grafana/taikai_viewer.json` — latencia p95/p99,
  drift de live-edge, erros de segmento, idade de rotacao CEK, alertas do viewer.
- **Grafana cache board:** `dashboards/grafana/taikai_cache.json` — promocoes hot/warm/cold
  e negacoes de QoS quando as janelas de alias rotacionam.
- **Alertmanager rules:** `dashboards/alerts/taikai_viewer_rules.yml` — paging por drift,
  avisos de falha de ingest, lag de rotacao CEK e penalidades/cooldowns de saude PoR
  da SoraFS. Garanta receivers para cada cluster de producao.

## Checklist de bundle de evidencias

- Artefatos de spool (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- Rode `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>`
  para emitir um inventario JSON assinado de envelopes pendentes/entregues e copiar
  arquivos request/SSM/TRM/lineage para o bundle do drill. O caminho de spool
  padrao e `storage/da_manifests/taikai` vindo de `torii.toml`.
- Snapshot `/status` cobrindo `telemetry.taikai_alias_rotations`.
- Exportacoes Prometheus (JSON/CSV) para as metricas acima durante a janela do incidente.
- Capturas Grafana com filtros visiveis.
- IDs do Alertmanager referenciando os disparos relevantes.
- Link para `docs/examples/taikai_anchor_lineage_packet.md` descrevendo o pacote
  canonico de evidencia.

## Espelhamento de dashboards e cadencia de drills

Atender ao requisito SN13-C significa provar que os dashboards Taikai
viewer/cache sao refletidos dentro do portal **e** que o drill de evidencia de
ancoragem roda em uma cadencia previsivel.

1. **Mirroring do portal.** Quando `dashboards/grafana/taikai_viewer.json` ou
   `dashboards/grafana/taikai_cache.json` mudarem, resuma os deltas em
   `sorafs/taikai-monitoring-dashboards` (este portal) e anote os checksums JSON
   na descricao do PR do portal. Destaque novos paineis/limiares para que revisores
   possam correlacionar com a pasta Grafana gerenciada.
2. **Drill mensal.**
   - Execute o drill na primeira terca-feira de cada mes as 15:00 UTC para que a
     evidencia chegue antes do sync de governanca SN13.
   - Capture artefatos de spool, telemetria `/status` e capturas Grafana em
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`.
   - Registre a execucao com
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
3. **Revisao e publicacao.** Em ate 48 horas, revise alertas/falsos positivos com
   DA Program + NetOps, registre follow-ups no drill log e linke o upload do bucket
   de governanca a partir de `docs/source/sorafs/runbooks-index.md`.

Se dashboards ou drills atrasarem, SN13-C nao pode sair de 🈺; mantenha esta secao
atualizada quando a cadencia ou as expectativas de evidencia mudarem.

## Comandos uteis

```bash
# Snapshot de telemetria de rotacao de alias em um diretorio de artefatos
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# Liste entradas do spool para um alias/evento especifico
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# Inspecione motivos de mismatch TRM no spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

Mantenha esta copia do portal sincronizada com o runbook canonico quando a
telemetria de ancoragem Taikai, os dashboards ou os requisitos de evidencia de
governanca mudarem.
