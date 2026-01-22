<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
id: reserve-ledger-digest
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


A politica Reserve+Rent (item do roadmap **SFM-6**) agora inclui os helpers de CLI `sorafs reserve`
mais o tradutor `scripts/telemetry/reserve_ledger_digest.py` para que execucoes de tesouraria emitam
transferencias deterministicas de rent/reserva. Esta pagina espelha o fluxo definido em
`docs/source/sorafs_reserve_rent_plan.md` e explica como ligar o novo feed de transferencias ao
Grafana + Alertmanager para que revisores de economia e governanca possam auditar cada ciclo de cobranca.

## Fluxo de ponta a ponta

1. **Cotacao + projecao do ledger**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

   sorafs reserve ledger \
     --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
     --provider-account ih58... \
     --treasury-account ih58... \
     --reserve-account ih58... \
     --asset-definition xor#sora \
     --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   O helper do ledger anexa um bloco `ledger_projection` (rent due, reserve shortfall,
   top-up delta, booleanos de underwriting) mais os ISIs Norito `Transfer` necessarios para
   mover XOR entre as contas de tesouraria e reserva.

2. **Gerar o digest + saidas Prometheus/NDJSON**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   O helper de digest normaliza os totais de micro-XOR em XOR, registra se a projecao atende
   ao underwriting e emite as metricas do feed de transferencias `sorafs_reserve_ledger_transfer_xor`
   e `sorafs_reserve_ledger_instruction_total`. Quando varios ledgers precisam ser processados
   (por exemplo, um lote de provedores), repita pares `--ledger`/`--label` e o helper grava um
   unico arquivo NDJSON/Prometheus contendo cada digest para que os dashboards ingiram o ciclo
   inteiro sem glue sob medida. O arquivo `--out-prom` mira um textfile collector do node-exporter -
   coloque o arquivo `.prom` no diretorio monitorado pelo exporter ou envie para o bucket de
   telemetria consumido pelo job do dashboard Reserve - enquanto `--ndjson-out` alimenta os mesmos
   payloads nos pipelines de dados.

3. **Publicar artefatos + evidencias**
   - Armazene digests em `artifacts/sorafs_reserve/ledger/<provider>/` e conecte o resumo Markdown
     ao seu relatorio semanal de economia.
   - Anexe o digest JSON ao burn-down de rent (para que auditores possam refazer a conta) e inclua
     o checksum dentro do pacote de evidencia de governanca.
   - Se o digest sinalizar top-up ou quebra de underwriting, referencie os IDs de alerta
     (`SoraFSReserveLedgerTopUpRequired`, `SoraFSReserveLedgerUnderwritingBreach`) e registre
     quais ISIs de transferencia foram aplicados.

## Metricas -> dashboards -> alertas

| Metrica fonte | Painel do Grafana | Alerta / gancho de politica | Notas |
|--------------|-------------------|-----------------------------|-------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | \"DA Rent Distribution (XOR/hour)\" em `dashboards/grafana/sorafs_capacity_health.json` | Alimente o digest semanal de tesouraria; picos no fluxo de reserva propagam para `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`). |
| `torii_da_rent_gib_months_total` | \"Capacity Usage (GiB-months)\" (mesmo dashboard) | Combine com o digest do ledger para provar que o storage cobrado coincide com transferencias XOR. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | \"Reserve Snapshot (XOR)\" + cards de status em `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerTopUpRequired` dispara quando `requires_top_up=1`; `SoraFSReserveLedgerUnderwritingBreach` dispara quando `meets_underwriting=0`. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | \"Transfers by Kind\", \"Latest Transfer Breakdown\" e os cards de cobertura em `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing` e `SoraFSReserveLedgerTopUpTransferMissing` avisam quando o feed de transferencias esta ausente ou zerado mesmo que rent/top-up seja exigido; os cards de cobertura caem para 0% nos mesmos casos. |

Quando um ciclo de rent termina, atualize os snapshots Prometheus/NDJSON, confirme que os
paineis Grafana capturam o novo `label` e anexe capturas + IDs do Alertmanager ao pacote de
governanca do rent. Isso prova que a projecao CLI, a telemetria e os artefatos de governanca
partem do **mesmo** feed de transferencias e mantem os dashboards de economia do roadmap
alinhados com a automacao Reserve+Rent. Os cards de cobertura devem mostrar 100% (ou 1.0),
e os novos alertas devem limpar assim que as transferencias de rent e top-up de reserva
estiverem presentes no digest.
