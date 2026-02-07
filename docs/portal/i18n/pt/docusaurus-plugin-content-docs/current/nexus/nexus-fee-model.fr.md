---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo de taxa de nexo
título: Mises a jour du modele de frais Nexus
descrição: Espelho de `docs/source/nexus_fee_model.md`, documenta as regras de regulamentação das pistas e as superfícies de reconciliação.
---

:::nota Fonte canônica
Esta página reflete `docs/source/nexus_fee_model.md`. Gardez les duas cópias alinhadas durante a migração das traduções japonesas, hebraicas, espanholas, portuguesas, francesas, russas, árabes e douradas.
:::

# Mises a jour du modele de frais Nexus

O roteador de regulamento unificado captura a manutenção do recus determinado por via para que os operadores possam reconciliar os débitos de gás com o modelo de frais Nexus.

- Para a arquitetura completa do roteador, a política de buffer, a matriz de telemetria e a sequência de implantação, veja `docs/settlement-router.md`. Este guia explica explícitamente os parâmetros documentados aqui e é colocado à disposição do roteiro NX-3 e comenta o SRE que monitora o roteador em produção.
- A configuração do ativo de gás (`pipeline.gas.units_per_gas`) inclui um decimal `twap_local_per_xor`, um `liquidity_profile` (`tier1`, `tier2`, ou `tier3`), e um `volatility_class` (`stable`, `elevated`, `dislocated`). Esses indicadores fornecem o roteador de regra para que a cotação XOR corresponda ao TWAP canônico e ao painel de corte de cabelo para a pista.
- Cada transação que o pagamento do gás registrou em `LaneSettlementReceipt`. Cada vez que você armazena a fonte de identificação fornecida pelo requerente, o micromontante local, o XOR é regulado imediatamente, o XOR atende após o corte de cabelo, a variação realizada (`xor_variance_micro`) e a data do bloco em milissegundos.
- A execução de blocos agrega as regras para lane/dataspace e publica via `lane_settlement_commitments` em `/v1/sumeragi/status`. Os totais expostos `total_local_micro`, `total_xor_due_micro` e `total_xor_after_haircut_micro` são adicionados ao bloco para as exportações noturnas de reconciliação.
- Um novo computador `total_xor_variance_micro` atende à margem de segurança consommee (diferença entre o XOR du e o atendimento pós-haircut), e o `swap_metadata` documenta os parâmetros de conversão determinados (TWAP, épsilon, perfil de liquidez e volatilidade_class) para que os auditores possam verificar os entradas da cotação independentemente da configuração de execução.

Os consumidores podem seguir `lane_settlement_commitments` nas costas dos snapshots existentes de compromissos de pista e de espaço de dados para verificar se os buffers de dinheiro, os paliers de corte de cabelo e a execução de swap correspondente ao modelo de dinheiro Nexus estão configurados.