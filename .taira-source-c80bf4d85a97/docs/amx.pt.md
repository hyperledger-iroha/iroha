---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: pt
direction: ltr
source: docs/amx.md
status: complete
translator: manual
source_hash: 563ec4d7d4d96fa04a7b210a962b9927046985eb01d1de0954575cf817f9f226
source_last_modified: "2025-11-09T19:43:51.262828+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução em português de docs/amx.md (AMX Execution & Operations Guide) -->

# Guia de Execução e Operações AMX

**Status:** Rascunho (NX‑17)  
**Público:** Protocolo core, engenheiros de AMX/consenso, SRE/Telemetria,
equipes de SDK e Torii  
**Contexto:** Completa o item de roadmap “Documentação (owner: Docs) — atualizar
`docs/amx.md` com diagramas de tempo, catálogo de erros, expectativas de
operadores e orientação para desenvolvedores sobre geração/uso de PVOs.”

## Resumo

Transações atômicas entre data spaces (AMX) permitem que um único envio toque
vários data spaces (DS) preservando slot de 1 s de finalidade, códigos de
falha determinísticos e confidencialidade para fragmentos em DS privados. Este
guia descreve o modelo de timing, o tratamento canônico de erros, os requisitos
de evidência para operadores e as expectativas de desenvolvedores para Proof
Verification Objects (PVOs), para que o entregável de roadmap seja
auto‑contido sem depender apenas do documento de design de Nexus
(`docs/source/nexus.md`).

Garantias principais:

- Cada envio AMX recebe orçamentos deterministas de prepare/commit; estouros
  causam abortos com códigos documentados em vez de travar lanes.
- Amostras de DA que estouram o orçamento empurram a transação para o próximo
  slot e emitem telemetria explícita (`missing-availability warning`) em vez de
  degradar o throughput de forma silenciosa.
- PVOs desacoplam provas pesadas da janela de 1 s, permitindo que
  clientes/batchers preregistrem artefatos que o host Nexus valida rapidamente
  dentro do slot.

## Modelo de tempo de slot

### Linha do tempo

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- Os orçamentos seguem o plano global do ledger: mempool 70 ms, commit de DA
  ≤300 ms, consenso 300 ms, IVM/AMX 250 ms, settle 40 ms e guard 40 ms.
- Transações que extrapolam a janela de DA são reagendadas de forma
  determinista; os demais estouros levantam códigos como `AMX_TIMEOUT` ou
  `SETTLEMENT_ROUTER_UNAVAILABLE`.
- A fatia de guard absorve exportação de telemetria e auditoria final de forma
  que o slot ainda encerre em 1 s mesmo quando exporters sofrem pequenos
  atrasos.

### Swim lane cross‑DS

```text
Client        DS A (public)        DS B (private)        Nexus Lane        Settlement
  │ submit tx │                     │                     │                 │
  │──────────▶│ prepare fragment    │                     │                 │
  │           │ proof + DA part     │ prepare fragment    │                 │
  │           │───────────────┬────▶│ proof + DA part     │                 │
  │           │               │     │─────────────┬──────▶│ Merge proofs    │
  │           │               │     │             │       │ verify PVO/DA   │
  │           │               │     │             │       │────────┬────────▶ apply
  │◀──────────│ result + code │◀────│ result + code │◀────│ outcome│          receipt
```

Cada fragmento de DS deve concluir sua janela de prepare de 30 ms antes que a
lane monte o slot. Provas faltantes permanecem no mempool para o próximo slot
em vez de bloquear peers.

### Checklist de instrumentação

| Métrica / Traço | Origem | SLO / Alerta | Notas |
|-----------------|--------|--------------|-------|
| `iroha_slot_duration_ms` (histograma) / `iroha_slot_duration_ms_latest` (gauge) | `iroha_telemetry` | p95 ≤ 1000 ms | Gate de CI descrito em `ans3.md`. |
| `iroha_da_quorum_ratio` | `iroha_telemetry` (hook de commit) | ≥0.95 por janela de 30 min | Derivada da telemetria de re‑agendamento de DA; cada bloco atualiza o gauge. |
| `iroha_amx_prepare_ms` | host IVM | p95 ≤ 30 ms por DS scope | Alimenta abortos `AMX_TIMEOUT`. |
| `iroha_amx_commit_ms` | host IVM | p95 ≤ 40 ms por DS scope | Cobre merge de deltas + execução de triggers. |
| `iroha_ivm_exec_ms` | host IVM | alerta se >250 ms por lane | Reflete a janela de execução de overlays de IVM. |
| `iroha_amx_abort_total{stage}` | executor | alerta se >0.05 abortos/slot ou picos sustentados em um único estágio | `stage`: `prepare`, `exec`, `commit`. |
| `iroha_amx_lock_conflicts_total` | escalonador AMX | alerta se >0.1 conflitos/slot | Indica conjuntos de leitura/escrita imprecisos. |

## Catálogo de erros AMX e expectativas de operação

Erros típicos:

- `AMX_TIMEOUT` – um fragmento ultrapassou seu orçamento (prepare/exec/commit).
- `AMX_LOCK_CONFLICT` – conflito de lock detectado pelo escalonador.
- `SETTLEMENT_ROUTER_UNAVAILABLE` – o settlement router não conseguiu rotear a
  operação dentro do slot.
- `PVO_MISSING` – esperava‑se um PVO preregistrado, mas ele não foi encontrado.

Operadores devem:

- Monitorar as métricas acima e manter painéis AMX dedicados em Grafana.
- Manter runbooks para reagir a picos de `AMX_TIMEOUT` ou
  `AMX_LOCK_CONFLICT` (por exemplo, revisar hints de acesso ou ajustar
  orçamentos).
- Arquivar evidências (logs, traces, PVOs) para incidentes AMX de forma que
  seja possível reconstruí‑los offline.

## Orientação para desenvolvedores: PVOs

- Proof Verification Objects encapsulam provas pesadas (por exemplo, provas ZK
  de fragmentos privados) em um artefato que o host Nexus consegue verificar
  rapidamente dentro do slot.
- Clientes devem preregistrar PVOs fora do caminho crítico do slot e referenciá‑los
  em transações AMX por meio de identificadores estáveis.
- O design de PVO deve garantir:
  - Encapsulamento Norito canônico (campos versionados, header Norito,
    checksum).
  - Separação clara entre dados públicos e privados para preservar
    confidencialidade.

Veja `docs/source/nexus.md` para detalhes adicionais de design e os formatos
exatos de PVO.

