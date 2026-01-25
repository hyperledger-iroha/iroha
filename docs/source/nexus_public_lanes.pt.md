---
lang: pt
direction: ltr
source: docs/source/nexus_public_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9bb3a13cec7d80bfd1729709eb0744a5a062954002ada5d48608f62f8907668
source_last_modified: "2025-12-08T18:48:53.874766+00:00"
translation_last_reviewed: 2026-01-01
---

# Staking de lanes publicas Nexus (NX-9)

Status: Em progresso -> **runtime + docs de operadores alinhados** (Abr 2026)
Responsaveis: Economics WG / Governance WG / Core Runtime
Referencia do roadmap: NX-9 - staking de lane publico e modulo de recompensas

Esta nota captura o modelo de dados canonico, a superficie de instrucoes, os controles de governanca
 e os hooks operacionais para o programa de staking de lanes publicas do Nexus. O objetivo e permitir
que validadores permissionless entrem nas lanes publicas, bond de stake, prestem servico de blocos e
recebam recompensas enquanto a governanca mantem alavancas deterministicas de slashing/runbook.

O scaffolding de codigo agora vive em:

- Tipos do modelo de dados: `crates/iroha_data_model/src/nexus/staking.rs`
- Definicoes ISI: `crates/iroha_data_model/src/isi/staking.rs`
- Stub do executor core (retorna um erro deterministico de guarda ate a logica NX-9 chegar):
  `crates/iroha_core/src/smartcontracts/isi/staking.rs`

Torii/SDKs podem comecar a fiacao dos payloads Norito antes da implementacao completa do runtime;
as instrucoes de stake agora travam o asset de staking configurado retirando de `stake_account`/`staker`
para uma conta escrow bonded (`nexus.staking.stake_escrow_account_id`). Slashes debitam o escrow e
creditam o sink configurado (`nexus.staking.slash_sink_account_id`), e os unbonds devolvem fundos
para a conta de origem quando o timer expira.

## 1. Estado do ledger e tipos

### 1.1 Registros de validadores

`PublicLaneValidatorRecord` rastreia o estado canonico de cada validador:

| Campo | Descricao |
|-------|-----------|
| `lane_id: LaneId` | Lane que o validador atende. |
| `validator: AccountId` | Conta que assina mensagens de consenso. |
| `stake_account: AccountId` | Conta que fornece o self-bond (pode diferir da identidade do validador). |
| `total_stake: Numeric` | Self stake + delegacoes aprovadas. |
| `self_stake: Numeric` | Stake fornecido pelo validador. |
| `metadata: Metadata` | Comissao %, ids de telemetria, flags jurisdicionais, info de contato. |
| `status: PublicLaneValidatorStatus` | Ciclo de vida (pending/active/jailed/exiting/etc.). O payload `PendingActivation` codifica a epoca alvo. |
| `activation_epoch: Option<u64>` | Epoca em que o validador ficou ativo (definida na ativacao). |
| `activation_height: Option<u64>` | Altura do bloco registrada na ativacao. |
| `last_reward_epoch: Option<u64>` | Epoca que gerou o ultimo pagamento. |

`PublicLaneValidatorStatus` enumera fases do ciclo de vida:

- `PendingActivation(epoch)` - aguardando a epoca de ativacao especificada pela governanca; o payload de tupla
  guarda a epoca de ativacao mais cedo derivada de `epoch_length_blocks`.
- `Active` - participa do consenso e pode receber recompensas.
- `Jailed { reason }` - suspenso temporariamente (downtime, breach de telemetria, etc.).
- `Exiting { releases_at_ms }` - unbonding; recompensas deixam de acumular.
- `Exited` - removido do conjunto.
- `Slashed { slash_id }` - evento de slashing registrado para auditoria.

Metadados de ativacao sao monotonic: `activation_epoch`/`activation_height` sao definidos na primeira
vez que um validador pending se torna ativo e qualquer tentativa de reativar em uma epoca/altura anterior
 e rejeitada. Validadores pending sao promovidos automaticamente no inicio do primeiro bloco cuja epoca
atenda o limite programado, e o contador de metricas de ativacao
(`nexus_public_lane_validator_activation_total`) registra a promocao junto com a mudanca de status.

Deployments permissioned mantem o roster de peers genesis ativo mesmo antes de existir stake de
validadores public-lane: enquanto os peers tiverem chaves de consenso ativas, o runtime recorre aos
peers genesis para o conjunto de validadores. Isso evita um deadlock de bootstrap enquanto a admissao
 de staking esta desabilitada ou ainda em rollout.

### 1.2 Shares de stake e unbonding

Delegadores (e validadores aumentando o proprio bond) sao modelados via
`PublicLaneStakeShare`:

- `bonded: Numeric` - montante bonded ativo.
- `pending_unbonds: BTreeMap<Hash, PublicLaneUnbonding>` - retiradas pendentes com chave `request_id`
  fornecida pelo cliente.
- `metadata` armazena pistas UX/back-office (ex., numeros de referencia de desk de custodia).

`PublicLaneUnbonding` guarda o cronograma deterministico de retirada (`amount`, `release_at_ms`).
Torii agora expoe shares ativas e retiradas pendentes via `GET /v1/nexus/public_lanes/{lane}/stake`
para que wallets mostrem timers sem RPCs bespoke.

Hooks de ciclo de vida (forcados pelo runtime):

- Entradas `PendingActivation(epoch)` mudam automaticamente para `Active` quando a epoca atual alcanca `epoch`.
  A ativacao registra `activation_epoch` e `activation_height`, e regressoes sao rejeitadas tanto para
  auto-ativacao quanto para chamadas explicitas `ActivatePublicLaneValidator`.
- Entradas `Exiting(releases_at_ms)` transicionam para `Exited` quando o timestamp do bloco passa de
  `releases_at_ms`, limpando linhas de stake-share para que a capacidade do validador seja recuperada
  sem limpeza manual.
- O registro de recompensas rejeita shares de validadores a menos que o validador esteja `Active`,
  mantendo validadores pending/exiting/jailed fora do acroamento de payouts.

### 1.3 Registros de recompensas

Distribuicoes de recompensas usam `PublicLaneRewardRecord` e `PublicLaneRewardShare`:

```norito
{
  "lane_id": 1,
  "epoch": 4242,
  "asset": "xor#wonderland",
  "total_reward": "250.0000",
  "shares": [
    { "account": "RnuaJGGDL8HNkN8bwHwBTU32fTWQmbRoM3QZBJintx5RqTU7GgPJmNiA", "role": "Validator", "amount": "150" },
    { "account": "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r", "role": "Nominator", "amount": "100" }
  ],
  "metadata": {
    "telemetry_epoch_root": "0x4afe...",
    "distribution_tx": "0xaabbccdd"
  }
}
```

Os registros fornecem evidencia deterministica para auditores e dashboards em cada payout.
A estrutura de recompensa flui para o ISI `RecordPublicLaneRewards`.

Guardas de runtime:

- Builds Nexus devem estar habilitadas; builds offline/stub rejeitam o registro de recompensas.
- Epocas de recompensa avancam de forma monotona por lane; epocas stale ou duplicadas sao rejeitadas.
- Assets de recompensa devem coincidir com o fee sink configurado (`nexus.fees.fee_sink_account_id` /
  `nexus.fees.fee_asset_id`) e o saldo do sink deve cobrir totalmente `total_reward`.
- Cada share deve ser positiva e respeitar a especificacao numerica do asset; totais de shares devem
  igualar `total_reward`.

## 2. Catalogo de instrucoes

Todas as instrucoes vivem em `iroha_data_model::isi::staking`. Elas derivam encoders/decoders Norito
para que SDKs submetam os payloads sem codecs bespoke.

### 2.1 `RegisterPublicLaneValidator`

Registra um validador e bond um stake inicial:

```norito
{
  "lane_id": 1,
  "validator": "RnuaJGGDL8HNkN8bwHwBTU32fTWQmbRoM3QZBJintx5RqTU7GgPJmNiA",
  "stake_account": "RnuaJGGDL8HNkN8bwHwBTU32fTWQmbRoM3QZBJintx5RqTU7GgPJmNiA",
  "initial_stake": "150000",
  "metadata": {
    "commission_bps": 750,
    "jurisdiction": "JP",
    "telemetry_id": "val-01"
  }
}
```

Regras de validacao:

- `initial_stake` >= `min_self_stake` (parametro de governanca).
- Metadata DEVE incluir hooks de contato/telemetria antes da ativacao.
- A governanca aprova/nega a entrada; ate la o status e `PendingActivation` e o runtime
  promove o validador para `Active` no proximo limite de epoca assim que a epoca alvo for alcancada.

### 2.2 `BondPublicLaneStake`

Bonda stake adicional (self-bond do validador ou contribuicao do delegador).

Campos chave: `staker`, `amount`, metadata opcional para statements. O runtime deve aplicar limites
especificos por lane (`max_delegators`, `min_bond`, `commission caps`).

### 2.3 `SchedulePublicLaneUnbond`

Inicia o timer de unbonding. Os submitters fornecem um `request_id` deterministico
(recomendacao: `blake2b(invoice)`), `amount`, e `release_at_ms`. O runtime deve verificar
`amount` <= stake bonded e ajustar `release_at_ms` ao periodo de unbonding configurado.

### 2.4 `FinalizePublicLaneUnbond`

Depois que o timer expira, este ISI desbloqueia o stake pendente e o devolve a `staker`.
O executor valida o request id, garante que o timestamp de unlock esta no passado, emite
uma atualizacao `PublicLaneStakeShare` e registra telemetria.

### 2.5 `SlashPublicLaneValidator`

A governanca usa esta instrucao para debitar stake e jail/eject validadores.

- `slash_id` vincula o evento a telemetria + docs de incidentes.
- `reason_code` e uma string enum estavel (ex., `double_sign`, `downtime`, `safety_violation`).
- `metadata` armazena hashes de bundles de evidencia, pointers de runbook ou IDs de reguladores.

Slashes se propagam para delegadores com base na politica de governanca (perda proporcional ou
validator-first). A logica de runtime emitira anotacoes `PublicLaneRewardRecord` quando NX-9 chegar.

### 2.6 `RecordPublicLaneRewards`

Registra o payout de uma epoca. Campos:

- `reward_asset`: asset distribuido (default `xor#nexus`).
- `total_reward`: total mint/transfer.
- `shares`: vetor de entradas `PublicLaneRewardShare`.
- `metadata`: referencias a transacoes de payout, hashes raiz ou dashboards.

Este ISI e idempotente por `(lane_id, epoch)` e fundamenta a contabilidade noturna.

## 3. Operacoes, ciclo de vida e tooling

- **Ciclo de vida + modos:** lanes stake-elected sao habilitadas via
  `nexus.staking.public_validator_mode = stake_elected` enquanto lanes restritas
  permanecem admin-managed (`nexus.staking.restricted_validator_mode = admin_managed`).
  Deployments permissioned mantem peers genesis ativos ate haver stake; para lanes stake-elected
  ainda exigimos um peer registrado com chave de consenso ativa presente na topologia de commit
  antes de `RegisterPublicLaneValidator` ter sucesso. Fingerprints genesis e
  `use_stake_snapshot_roster` decidem se o runtime deriva o roster a partir de snapshots de stake
  ou recorre a peers genesis.
- **Operacoes de ativacao/saida:** registros entram em `PendingActivation` e sao auto-promovidos no
  primeiro bloco cuja epoca atinge o limite programado (`epoch_length_blocks`). Operadores tambem
  podem chamar `ActivatePublicLaneValidator` apos o limite para forcar a promocao. Saidas movem
  validadores para `Exiting(release_at_ms)` e liberam capacidade apenas quando o timestamp do bloco
  alcanca `release_at_ms`; re-registracao apos slash ainda requer saida para que o registro fique
  `Exited` e a capacidade seja recuperada. Checagens de capacidade usam `nexus.staking.max_validators`
  e rodam apos o finalizador de saida, entao saidas com data futura bloqueiam novas registracoes
  ate o timer expirar.
- **Config knobs:** `nexus.staking.min_validator_stake`, `nexus.staking.stake_asset_id`,
  `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`,
  `nexus.staking.unbonding_delay`, `nexus.staking.withdraw_grace`, `nexus.staking.max_validators`,
  `nexus.staking.max_slash_bps`, `nexus.staking.reward_dust_threshold`, e os switches de modo acima.
  Passe-os via `iroha_config::parameters::actual::Nexus` e exponha em `status.md` quando valores GA
  forem ratificados.
- **Torii/CLI quickstart:**
  - `iroha app nexus lane-report --summary` mostra entradas do catalogo de lanes, readiness de manifests,
    e modos de validadores (stake-elected vs admin-managed) para que operadores confirmem se a admissao
    de staking esta habilitada para uma lane.
  - `iroha_cli app nexus public-lane validators --lane <id> [--summary] [--address-format {ih58,compressed}]`
    apresenta marcadores de ciclo de vida/ativacao (epoca alvo pending, `activation_epoch` /
    `activation_height`, release de saida, slash id) junto com stake bonded/self.
    `iroha_cli app nexus public-lane stake --lane <id> [--validator ih58...] [--summary]`
    espelha o endpoint `/stake` com hints de pending-unbond por par `(validator, staker)`.
  - Snapshots Torii para dashboards e SDKs:
    - `GET /v1/nexus/public_lanes/{lane}/validators` - metadata, status
      (`PendingActivation`/`Active`/`Exiting`/`Exited`/`Slashed`), epoca/altura de ativacao,
      timers de release, stake bonded, ultima epoca de recompensa.
      `address_format=ih58|compressed` controla o rendering literal (IH58 preferido; compressed (`snx1`) e segunda melhor opcao Sora-only).
    - `GET /v1/nexus/public_lanes/{lane}/stake` - shares de stake (`validator`,
      `staker`, bonded amount) mais timers de pending unbond. `?validator=ih58...`
      filtra a resposta para dashboards focados em um validador; `address_format` aplica-se
      a todos os literais.
  - ISIs de ciclo de vida usam o caminho de transacao padrao (Torii
    `/v1/transactions` ou o pipeline de instrucoes do CLI). Exemplos de payloads Norito JSON:

    ```jsonc
    [
      { "ActivatePublicLaneValidator": { "lane_id": 1, "validator": "RnuaJGGDL8HNkN8bwHwBTU32fTWQmbRoM3QZBJintx5RqTU7GgPJmNiA" } },
      {
        "ExitPublicLaneValidator": {
          "lane_id": 1,
          "validator": "RnuaJGGDL8HNkN8bwHwBTU32fTWQmbRoM3QZBJintx5RqTU7GgPJmNiA",
          "release_at_ms": 1730000000000
        }
      }
    ]
    ```
- **Telemetria + runbooks:** metricas expoem contagens de validadores, stake bonded e pendente,
  totais de recompensa e contadores de slash sob a familia `nexus_public_lane_*`. Conecte dashboards
  ao mesmo conjunto de dados usado pelos testes de aceitacao NX-9 para que deltas de validadores e
  evidencia de recompensa/slash permanecam auditaveis. Instrucoes de slashing continuam somente
  governanca; o registro de recompensas deve provar totais de payout (hash do batch de payout).

## 4. Alinhamento com o roadmap

- OK Runtime e storages WSV implementam o ciclo de vida de validadores NX-9; as regressoes cobrem
  timing de ativacao, prerequisitos de peers, saidas atrasadas e re-registracao apos slashes.
- OK Torii expoe `/v1/nexus/public_lanes/{lane}/{validators,stake,rewards/pending}` com Norito JSON
  para que SDKs e dashboards monitorem o estado da lane sem RPCs custom.
- OK Config knobs e telemetria estao documentados; deployments mistos mantem lanes stake-elected e
  admin-managed isoladas para que rosters de validadores permanecam deterministas.

### 2.7 `CancelConsensusEvidencePenalty`

Cancels consensus slashing before the delayed penalty applies.

- `evidence`: the Norito-encoded `Evidence` payload that was recorded in `consensus_evidence`.
- The record is marked `penalty_cancelled` and `penalty_cancelled_at_height`, preventing slashing when `slashing_delay_blocks` elapses.
