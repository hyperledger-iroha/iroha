---
lang: pt
direction: ltr
source: docs/source/runbooks/nexus_multilane_rehearsal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0aa4642cc60f384f6c52aaae2f97a6e4e8f741d6365c483514c2016d1ba10e82
source_last_modified: "2025-12-14T09:53:36.243318+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook de rehearsal de lançamento multi‑lane do Nexus

Este runbook guia o rehearsal Phase B4 do Nexus. Ele valida que o bundle de
`iroha_config` aprovado pela governança e o manifesto genesis multi‑lane se
comportem de forma determinística em telemetria, roteamento e drills de rollback.

## Escopo

- Exercitar os três lanes do Nexus (`core`, `governance`, `zk`) com ingress Torii
  misto (transações, deploys de contrato, ações de governança) usando a seed de
  carga assinada `NEXUS-REH-2026Q1`.
- Capturar artefatos de telemetria/trace exigidos pela aceitação B4 (scrape
  Prometheus, export OTLP, logs estruturados, traces de admissão Norito, métricas RBC).
- Executar o drill de rollback `B4-RB-2026Q1` imediatamente após o dry‑run e
  confirmar que o perfil single‑lane reaplica corretamente.

## Pré‑condições

1. `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` reflete a
   aprovação GOV-2026-03-19 (manifestos assinados + iniciais dos revisores).
2. `defaults/nexus/config.toml` (sha256
   `4f57655666bb0c83221cd3b56fd37218822e4c63db07e78a6694db51077f7017`, blake2b
   `65827a4b0348a7837f181529f602dc3315eba55d6ca968aaafb85b4ef8cfb2f6759283de77590ec5ec42d67f5717b54a299a733b617a50eb2990d1259c848017`, com
   `nexus.enabled = true` embutido) e `defaults/nexus/genesis.json` correspondem
   aos hashes aprovados; `kagami genesis bootstrap --profile nexus` reporta o
   mesmo digest registrado no tracker.
3. O catálogo de lanes corresponde ao layout aprovado de três lanes;
   `irohad --sora --config defaults/nexus/config.toml` deve emitir o banner do
   router Nexus.
4. A CI multi‑lane está verde: `ci/check_nexus_multilane_pipeline.sh`
   (executa `integration_tests/tests/nexus/multilane_pipeline.rs` via
   `.github/workflows/integration_tests_multilane.yml`) e
   `ci/check_nexus_multilane.sh` (cobertura de router) passam para que o perfil
   Nexus permaneça pronto para multi‑lane (`nexus.enabled = true`, hashes do
   catálogo Sora intactos, storage por lane em `blocks/lane_{id:03}_{slug}` e
   merge logs provisionados). Capture os digests de artefatos no tracker quando
   o bundle de defaults mudar.
5. Dashboards e alertas de telemetria para métricas Nexus estão importados na
   pasta de Grafana do rehearsal; rotas de alerta apontam para o serviço
   PagerDuty do rehearsal.
6. Os lanes do Torii SDK estão configurados conforme a tabela de política de
   roteamento e conseguem reproduzir a carga do rehearsal localmente.

## Visão geral do cronograma

| Fase | Janela alvo | Responsáveis | Critério de saída |
|-------|-------------|--------------|-------------------|
| Preparação | Apr 1 – 5 2026 | @program-mgmt, @telemetry-ops | Seed publicada, dashboards preparados, nós provisionados. |
| Freeze de staging | Apr 8 2026 18:00 UTC | @release-eng | Hashes de config/genesis re‑verificados; aviso de freeze enviado. |
| Execução | Apr 9 2026 15:00 UTC | @qa-veracity, @nexus-core, @torii-sdk | Checklist concluído sem incidentes bloqueantes; pacote de telemetria arquivado. |
| Drill de rollback | Imediatamente pós‑execução | @sre-core | Checklist `B4-RB-2026Q1` concluído; telemetria de rollback capturada. |
| Retrospectiva | Até Apr 15 2026 | @program-mgmt, @telemetry-ops, @governance | Doc de retro/lições aprendidas + tracker de bloqueios publicado. |

## Checklist de execução (Apr 9 2026 15:00 UTC)

1. **Atestado de config** — `iroha_cli config show --actual` em cada nó;
   confirmar hashes iguais à entrada do tracker.
2. **Warm‑up dos lanes** — reproduzir a seed por 2 slots e verificar
   `nexus_lane_state_total` com atividade nos três lanes.
3. **Captura de telemetria** — registrar snapshots de Prometheus `/metrics`,
   amostras OTLP, logs estruturados do Torii (por lane/dataspace) e métricas RBC.
4. **Hooks de governança** — executar o subconjunto de transações de governança
   e verificar roteamento por lane + tags de telemetria.
5. **Drill de incidente** — simular saturação de lane conforme plano; garantir
   alertas disparados e resposta registrada.
6. **Drill de rollback `B4-RB-2026Q1`** — aplicar perfil single‑lane, reproduzir o
   checklist de rollback, coletar evidências de telemetria e re‑aplicar o bundle Nexus.
7. **Upload de artefatos** — enviar pacote de telemetria, traces do Torii e drill log
   para o bucket de evidência Nexus; vincular em `docs/source/nexus_transition_notes.md`.
8. **Manifesto/validação** — executar `scripts/telemetry/validate_nexus_telemetry_pack.py \
   --pack-dir <path> --slot-range <start-end> --workload-seed <value> \
   --require-slot-range --require-workload-seed` para gerar `telemetry_manifest.json`
   + `.sha256`, e anexar o manifesto à entrada do tracker do rehearsal. O helper
   normaliza limites de slot (registrados como inteiros no manifesto) e falha
   rápido quando alguma dica falta para manter artefatos determinísticos.

## Saídas

- Checklist do rehearsal assinado + log do drill de incidente.
- Pacote de telemetria (`prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`).
- Manifesto de telemetria + digest gerado pelo script de validação.
- Documento de retrospectiva com bloqueios, mitigações e responsáveis.

## Resumo de execução — Apr 9 2026

- Rehearsal executado 15:00 UTC–16:12 UTC com seed `NEXUS-REH-2026Q1`; os três
  lanes sustentaram ~2,4k TEU por slot e `nexus_lane_state_total` reportou
  envelopes equilibrados.
- Pacote de telemetria arquivado em `artifacts/nexus/rehearsals/2026q1/` (inclui
  `prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`, log de incidente
  e evidência de rollback). Checksums registrados em
  `docs/source/project_tracker/nexus_rehearsal_2026q1.md`.
- Drill de rollback `B4-RB-2026Q1` concluído às 16:18 UTC; perfil single‑lane
  reaplicado em 6m42s sem lanes travados, e bundle Nexus reabilitado após
  confirmação de telemetria.
- Incidente de saturação de lane injetado no slot 842 (clamp forçado de headroom)
  disparou os alertas esperados; o playbook de mitigação encerrou o page em 11m
  com timeline do PagerDuty documentada.
- Nenhum bloqueio impediu a conclusão; itens de follow‑up (automação de log de
  headroom TEU, script validador do pacote de telemetria) estão na retro de Apr 15.

## Escalonamento

- Incidentes bloqueantes ou regressões de telemetria interrompem o rehearsal e
  exigem escalonamento para governança em até 4 horas úteis.
- Qualquer desvio do bundle de config/genesis aprovado deve reiniciar o rehearsal
  após re‑aprovação.

## Validação do pacote de telemetria (Concluído)

Execute `scripts/telemetry/validate_nexus_telemetry_pack.py` após cada rehearsal
para provar que o bundle de telemetria contém os artefatos canônicos (export de
Prometheus, OTLP NDJSON, logs estruturados do Torii, log de rollback) e capturar
seus digests SHA-256. O helper escreve `telemetry_manifest.json` e o arquivo
`.sha256` correspondente para que a governança cite os hashes de evidência no
pacote de retro.

Para o rehearsal de Apr 9 2026, o manifesto validado fica junto aos artefatos em
`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` com seu digest em
`telemetry_manifest.json.sha256`. Anexe ambos ao tracker ao publicar a retro.

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus_rehearsal_2026q1 \
  --slot-range 820-860 \
  --workload-seed NEXUS-REH-2026Q1 \
  --metadata rehearsal_id=B4-2026Q1 team=telemetry-ops
```

Passe `--require-slot-range` / `--require-workload-seed` na CI para bloquear
uploads que esquecem essas anotações. Use `--expected <name>` para adicionar
artefatos extras (ex.: recibos DA) se o plano do rehearsal exigir.
