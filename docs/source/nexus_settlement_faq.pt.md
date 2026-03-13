<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: pt
direction: ltr
source: docs/source/nexus_settlement_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9ccbc52b2d34a410c5b724b6421fc91bd403cd40d0a03360315a2ae2e3e504ec
source_last_modified: "2025-11-21T14:07:47.531238+00:00"
translation_last_reviewed: 2026-01-01
---

# FAQ de settlement do Nexus

**Link do roadmap:** NX-14 - documentacao do Nexus e runbooks de operadores  
**Status:** Redigido 2026-03-24 (espelha specs do settlement router e playbook CBDC)  
**Audiencia:** Operadores, autores de SDK e revisores de governanca preparando o lancamento do
Nexus (Iroha 3).

Este FAQ responde as perguntas que surgiram durante a revisao NX-14 sobre roteamento de
settlement, conversao de XOR, telemetria e evidencia de auditoria. Consulte
`docs/source/settlement_router.md` para a especificacao completa e
`docs/source/cbdc_lane_playbook.md` para os knobs de politica especificos de CBDC.

> **TL;DR:** Todos os fluxos de settlement passam pelo Settlement Router, que
> debita buffers XOR em lanes publicas e aplica tarifas por lane. Operadores
> devem manter a config de roteamento (`config/config.toml`), dashboards de telemetria e
> logs de auditoria em sync com os manifests publicados.

## Perguntas frequentes

### Quais lanes lidam com settlement e como saber onde meu DS se encaixa?

- Cada dataspace declara um `settlement_handle` no manifest. Handles padrao se mapeiam assim:
  - `xor_global` para lanes publicas padrao.
  - `xor_lane_weighted` para lanes publicas customizadas que buscam liquidez em outro lugar.
  - `xor_hosted_custody` para lanes privadas/CBDC (buffer XOR em escrow).
  - `xor_dual_fund` para lanes hibridas/confidenciais que misturam fluxos shielded + publicos.
- Consulte `docs/source/nexus_lanes.md` para classes de lane e
  `docs/source/project_tracker/nexus_config_deltas/*.md` para as aprovacoes mais recentes do
  catalogo. `irohad --sora --config ... --trace-config` imprime o catalogo efetivo em runtime para
  auditorias.

### Como o Settlement Router determina taxas de conversao?

- O router aplica um unico caminho com precificacao deterministica:
  - Para lanes publicas usamos o pool de liquidez XOR on-chain (DEX publica). Oraculos de preco
    fazem fallback para o TWAP aprovado pela governanca quando a liquidez e baixa.
  - Lanes privadas pre-financiam buffers XOR. Quando debitam um settlement, o router registra a
    tupla de conversao `{lane_id, source_token, xor_amount, haircut}` e aplica haircuts aprovados
    pela governanca (`haircut.rs`) se os buffers sairem do alinhamento.
- A configuracao vive sob `[settlement]` em `config/config.toml`. Evite edicoes customizadas salvo
  instrucao da governanca. Consulte `docs/source/settlement_router.md` para descricoes dos campos.

### Como taxas e rebates sao aplicados?

- As taxas sao expressas por lane no manifest:
  - `base_fee_bps` - aplica a cada debito de settlement.
  - `liquidity_haircut_bps` - compensa provedores de liquidez compartilhada.
  - `rebate_policy` - opcional (ex., rebates promocionais CBDC).
- O router emite eventos `SettlementApplied` (formato Norito) com detalhamento de taxas para que
  SDKs e auditores conciliem entradas do ledger.

### Que telemetria prova que settlements estao saudaveis?

- Metricas Prometheus (exportadas via `iroha_telemetry` e settlement router):
  - `nexus_settlement_latency_seconds{lane_id}` - P99 deve ficar abaixo de 900 ms para lanes
    publicas / 1200 ms para lanes privadas.
  - `settlement_router_conversion_total{source_token}` - confirma volumes de conversao por token.
  - `settlement_router_haircut_total{lane_id}` - alertar quando nao-zero sem nota de governanca.
  - `iroha_settlement_buffer_xor{lane_id,dataspace_id}` - mostra debitos XOR ao vivo por lane
    (micro unidades). Alertar quando <25 %/10 % de Bmin.
  - `iroha_settlement_pnl_xor{lane_id,dataspace_id}` - variacao de haircut realizada para conciliar
    com P&L de tesouraria.
  - `iroha_settlement_haircut_bp{lane_id,dataspace_id}` - epsilon efetivo aplicado no ultimo bloco;
    auditores comparam com a policy do router.
  - `iroha_settlement_swapline_utilisation{lane_id,dataspace_id,profile}` - uso de linha de credito
    sponsor/MM; alertar acima de 80 %.
- `nexus_lane_block_height{lane,dataspace}` - ultima altura de bloco observada para o par
  lane/dataspace; manter peers vizinhos dentro de poucos slots.
- `nexus_lane_finality_lag_slots{lane,dataspace}` - slots entre o head global e o bloco mais recente
  para aquela lane; alertar quando >12 fora de drills.
- `nexus_lane_settlement_backlog_xor{lane,dataspace}` - backlog aguardando settlement em XOR; gatear
  cargas CBDC/privadas antes de exceder limites reguladores.

`settlement_router_conversion_total` carrega os labels `lane_id`, `dataspace_id` e `source_token`
para provar qual ativo de gas impulsionou cada conversao. `settlement_router_haircut_total`
acumula unidades XOR (nao micro valores brutos), permitindo que a Tesouraria concilie o ledger de
haircut diretamente do Prometheus.
- `lane_settlement_commitments[*].swap_metadata.volatility_class` mostra se o router aplicou o
  bucket de margem `stable`, `elevated` ou `dislocated`. Entradas elevated/dislocated devem ligar
  ao log de incidente ou nota de governanca.
- Dashboards: `dashboards/grafana/nexus_settlement.json` mais o overview
  `nexus_lanes.json`. Amarre alertas a `dashboards/alerts/nexus_audit_rules.yml`.
- Quando a telemetria de settlement degrada, registre o incidente conforme o runbook em
  `docs/source/nexus_operations.md`.

### Como exportar telemetria de lane para reguladores?

Execute o helper abaixo quando um regulador solicitar a tabela de lanes:

```
cargo xtask nexus-lane-audit \
  --status artifacts/status.json \
  --json-out artifacts/nexus_lane_audit.json \
  --parquet-out artifacts/nexus_lane_audit.parquet \
  --markdown-out artifacts/nexus_lane_audit.md \
  --captured-at 2026-02-12T09:00:00Z \
  --lane-compliance artifacts/lane_compliance_evidence.json
```

* `--status` aceita o blob JSON retornado por `iroha status --format json`.
* `--json-out` captura um array JSON canonico por lane (aliases, dataspace, altura de bloco,
  finality lag, capacidade/utilizacao TEU, contadores de scheduler trigger + utilization, RBC
  throughput, backlog, metadados de governanca, etc.).
* `--parquet-out` escreve o mesmo payload como arquivo Parquet (schema Arrow), pronto para
  reguladores que exigem evidencia columnar.
* `--markdown-out` emite um resumo legivel que sinaliza lanes com lag, backlog nao-zero, evidencia
  de compliance ausente e manifests pendentes; o default e `artifacts/nexus_lane_audit.md`.
* `--lane-compliance` e opcional; quando fornecido deve apontar para o manifest JSON descrito no doc
  de compliance para que as linhas exportadas incorporem a policy de lane correspondente, assinaturas
  de revisores, snapshot de metricas e trechos de audit log.

Arquive ambos os outputs em `artifacts/` com a evidencia routed-trace (capturas de
`nexus_lanes.json`, estado do Alertmanager e `nexus_lane_rules.yml`).

### Que evidencia os auditores esperam?

1. **Snapshot de config** - capture `config/config.toml` com a secao `[settlement]` e o catalogo de
   lanes referenciado pelo manifest atual.
2. **Logs do router** - arquive `settlement_router.log` diariamente; inclui IDs de settlement com
   hash, debitos XOR e onde haircuts foram aplicados.
3. **Exports de telemetria** - snapshot semanal das metricas citadas acima.
4. **Relatorio de conciliacao** - opcional, mas recomendado: exporte entradas `SettlementRecordV1`
   (ver `docs/source/cbdc_lane_playbook.md`) e compare com o ledger de tesouraria.

### Os SDKs precisam de tratamento especial para settlement?

- SDKs devem:
  - Fornecer helpers para consultar eventos de settlement (`/v2/settlement/records`) e interpretar
    logs `SettlementApplied`.
  - Expor lane IDs + settlement handles na configuracao do cliente para que operadores roteiem
    transacoes corretamente.
  - Espelhar payloads Norito definidos em `docs/source/settlement_router.md` (ex.,
    `SettlementInstructionV1`) com testes end-to-end.
- O quickstart do SDK Nexus (proxima secao) detalha snippets por linguagem para onboarding na rede
  publica.

### Como settlements interagem com governanca ou freios de emergencia?

- A governanca pode pausar settlement handles especificos via updates de manifest. O router respeita
  o flag `paused` e rejeita novos settlements com erro deterministico (`ERR_SETTLEMENT_PAUSED`).
- "Haircut clamps" de emergencia limitam debitos maximos de XOR por bloco para evitar drenar
  buffers compartilhados.
- Operadores devem monitorar `governance.settlement_pause_total` e seguir o template de incidente em
  `docs/source/nexus_operations.md`.

### Onde reportar bugs ou solicitar mudancas?

- Gaps de feature -> abra um issue taggeado `NX-14` e relacione ao roadmap.
- Incidentes urgentes de settlement -> page o Nexus primary (ver
  `docs/source/nexus_operations.md`) e anexe logs do router.
- Correcoes de documentacao -> abra PRs contra este arquivo e seus equivalentes no portal
  (`docs/portal/docs/nexus/overview.md`, `docs/portal/docs/nexus/operations.md`).

### Pode mostrar exemplos de fluxos de settlement?

Os snippets abaixo mostram o que os auditores esperam para os tipos de lane mais comuns. Capture o
log do router, hashes do ledger e o export de telemetria correspondente para cada cenario para que
revisores possam reproduzir a evidencia.

#### Lane CBDC privada (`xor_hosted_custody`)

Abaixo ha um log do router resumido para uma lane CBDC privada usando o handle hosted custody. O
log prova debitos XOR deterministas, composicao de taxas e IDs de telemetria:

```text
2026-03-24T11:42:07Z settlement_router lane=3 dataspace=ds::cbdc::jp
    handle=xor_hosted_custody settlement_id=0x9c2f...a413
    source_token=JPYCBDC amount=125000.00
    xor_debited=312.500000 xor_rate=400.000000 haircut_bps=25 base_fee_bps=15
    fee_breakdown={base=0.046875, haircut=0.078125}
    ledger_tx=0x7ab1...ff11 telemetry_trace=nexus-settle-20260324T1142Z-lane3
```

No Prometheus voce deve ver as metricas correspondentes:

```text
nexus_settlement_latency_seconds{lane_id="3"} 0.842
settlement_router_conversion_total{lane_id="3",source_token="JPYCBDC"} += 1
settlement_router_haircut_total{lane_id="3"} += 0.078125
```

Arquive o trecho de log, o hash da transacao do ledger e o export de metricas juntos para que os
 auditores possam reconstruir o fluxo. Os proximos exemplos mostram como registrar evidencia para
lanes publicas e hibridas/confidenciais.

#### Lane publica (`xor_global`)

Dataspaces publicos roteiam via `xor_global`, entao o router debita o buffer DEX compartilhado e
registra o TWAP ao vivo que precificou a transferencia. Anexe o hash TWAP ou nota de governanca
quando o oraculo fizer fallback para um valor cacheado.

```text
2026-03-25T08:11:04Z settlement_router lane=0 dataspace=ds::public::creator
    handle=xor_global settlement_id=0x81cc...991c
    source_token=XOR amount=42.000000
    xor_debited=42.000000 xor_rate=1.000000 haircut_bps=0 base_fee_bps=10
    fee_breakdown={base=0.004200, liquidity=0.000000}
    dex_twap_id=twap-20260325T0810Z ledger_tx=0x319e...dd72 telemetry_trace=nexus-settle-20260325T0811Z-lane0
```

As metricas provam o mesmo fluxo:

```text
nexus_settlement_latency_seconds{lane_id="0"} 0.224
settlement_router_conversion_total{lane_id="0",source_token="XOR"} += 1
settlement_router_haircut_total{lane_id="0"} += 0
```

Salve o registro TWAP, o log do router, o snapshot de telemetria e o hash do ledger no mesmo bundle
 de evidencia. Quando alertas dispararem para latencia da lane 0 ou frescor do TWAP, vincule o
 ticket de incidente a esse bundle.

#### Lane hibrida/confidencial (`xor_dual_fund`)

Lanes hibridas misturam buffers shielded com reservas XOR publicas. Cada settlement deve mostrar
qual bucket forneceu o XOR e como a policy de haircut dividiu as taxas. O log do router expoe esses
detalhes via o bloco de metadata dual-fund:

```text
2026-03-26T19:54:31Z settlement_router lane=9 dataspace=ds::hybrid::art
    handle=xor_dual_fund settlement_id=0x55d2...c0ab
    source_token=ARTCREDIT amount=9800.00
    xor_debited_public=12.450000 xor_debited_shielded=11.300000
    xor_rate_public=780.000000 xor_rate_shielded=820.000000
    haircut_bps=35 base_fee_bps=20 dual_fund_ratio=0.52
    fee_breakdown={base=0.239000, haircut=0.418750}
    ledger_tx=0xa924...1104 telemetry_trace=nexus-settle-20260326T1954Z-lane9
```

```text
nexus_settlement_latency_seconds{lane_id="9"} 0.973
settlement_router_conversion_total{lane_id="9",source_token="ARTCREDIT"} += 1
settlement_router_haircut_total{lane_id="9"} += 0.418750
```

Arquive o log do router junto com a policy dual-fund (extrato do catalogo de governanca), o export
`SettlementRecordV1` da lane e o snippet de telemetria para que os auditores confirmem que o split
shielded/publico respeitou os limites de governanca.

Mantenha este FAQ atualizado quando o comportamento do settlement router mudar ou quando a
 governanca introduzir novas classes de lane/policies de taxas.
