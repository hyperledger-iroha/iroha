<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: pt
direction: ltr
source: docs/source/nexus_overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bda1352ff13cc866cd02a08f9db6be962798b547e905f2fccf236cd803eb0eda
source_last_modified: "2025-11-08T16:26:32.878050+00:00"
translation_last_reviewed: 2026-01-01
---

# Visao geral do Nexus e contexto operacional

**Link do roadmap:** NX-14 - documentacao do Nexus e runbooks de operacao  
**Status:** Rascunho 2026-03-24 (pareado com `docs/source/nexus_operations.md`)  
**Publico:** gerentes de programa, engenheiros de operacoes e equipes parceiras que
precisam de um resumo de uma pagina da arquitetura do Sora Nexus (Iroha 3) antes
de mergulhar nas especificacoes detalhadas (`docs/source/nexus.md`,
`docs/source/nexus_lanes.md`, `docs/source/nexus_transition_notes.md`).

## 1. Linhas de release e ferramentas compartilhadas

- **Iroha 2** permanece como trilha auto-hospedada para implantacoes de consorcio.
- **Iroha 3 / Sora Nexus** introduz execucao multi-lane, data spaces e governanca compartilhada.
  O mesmo repositorio, toolchain e pipelines de CI constroem ambas as linhas de release, entao
  correcoes no IVM, no compilador Kotodama ou nos SDKs se aplicam automaticamente ao Nexus.
- **Artefatos:** `iroha3-<version>-<os>.tar.zst` bundles e imagens OCI contem binarios,
  configs de exemplo e metadados do perfil Nexus. Operadores consultam
  `docs/source/sora_nexus_operator_onboarding.md` para o fluxo de validacao de artefatos
  de ponta a ponta.
- **Superficie SDK compartilhada:** SDKs de Rust, Python, JS/TS, Swift e Android consomem
  os mesmos esquemas Norito e fixtures de endereco (`fixtures/account/address_vectors.json`)
  para que wallets e automacao possam alternar entre redes Iroha 2 e Nexus sem
  bifurcar formatos.

## 2. Blocos de construcao arquitetural

| Componente | Descricao | Referencias chave |
|-----------|-----------|-------------------|
| **Data Space (DS)** | Dominio de execucao com escopo de governanca que define membros validadores, classe de privacidade, politica de taxas e perfil de disponibilidade de dados. Cada DS possui uma ou mais lanes. | `docs/source/nexus.md`, `docs/source/nexus_transition_notes.md` |
| **Lane** | Shard deterministico de execucao e estado. Manifests de lane declaram conjuntos de validadores, hooks de settlement, metadados de telemetria e permissoes de roteamento. O anel de consenso global ordena commitments de lane. | `docs/source/nexus_lanes.md` |
| **Space Directory** | Contrato de registro (e helpers CLI) que armazena manifests de DS, rotacoes de validadores e grants de capacidade. Mantem manifests historicos assinados para que auditores reconstruam o estado. | `docs/source/nexus.md#space-directory` |
| **Lane Catalog** | Secao de configuracao (`[nexus]` em `config.toml`) que mapeia IDs de lane para alias, politicas de roteamento e knobs de retencao. Operadores podem inspecionar o catalogo efetivo via `irohad --sora --config ... --trace-config`. | `docs/source/sora_nexus_operator_onboarding.md` |
| **Settlement Router** | Roteia movimentos XOR entre lanes (por exemplo, lanes CBDC privadas <-> lanes de liquidez publicas). Politicas padrao estao em `docs/source/cbdc_lane_playbook.md`. | `docs/source/cbdc_lane_playbook.md` |
| **Telemetry & SLOs** | Dashboards e regras de alerta em `dashboards/grafana/nexus_*.json` capturam altura de lanes, backlog de DA, latencia de settlement e profundidade da fila de governanca. O plano de remediacao segue em `docs/source/nexus_telemetry_remediation_plan.md`. | `dashboards/grafana/nexus_lanes.json`, `dashboards/alerts/nexus_audit_rules.yml` |

### Classes de lane e data space

- `default_public` lanes ancoram workloads totalmente publicos sob o Parlamento Sora.
- `public_custom` lanes permitem economias especificas por programa mantendo transparencia.
- `private_permissioned` lanes servem CBDCs ou apps de consorcio; exportam apenas commitments e proofs.
- `hybrid_confidential` lanes combinam provas de conhecimento zero com hooks de divulgacao seletiva.

Cada lane declara:

1. **Manifest de lane:** metadados aprovados por governanca e rastreados no Space Directory.
2. **Politica de disponibilidade de dados:** parametros de erasure coding, hooks de recuperacao e requisitos de auditoria.
3. **Perfil de telemetria:** dashboards e runbooks on-call que devem ser atualizados sempre que a governanca altera uma lane.

## 3. Resumo do cronograma de rollout

| Fase | Foco | Criterios de saida |
|------|------|--------------------|
| **N0 - Closed beta** | Registrador gerido pelo conselho, somente namespace `.sora`, onboarding manual de operadores. | Manifests de DS assinados, catalogo de lanes estatico, ensaios de governanca registrados. |
| **N1 - Public launch** | Adiciona sufixos `.nexus`, leiloes e registrador de autoatendimento. Settlements conectam a tesouraria XOR. | Tests de sincronizacao resolver/gateway verdes, dashboards de reconciliacao de cobranca ativos, exercicio de disputa concluido. |
| **N2 - Expansion** | Habilita `.dao`, APIs de reseller, analitica, portal de disputas, scorecards de stewards. | Artefatos de compliance versionados, toolkit de policy-jury ativo, relatorios de transparencia da tesouraria publicados. |
| **NX-12/13/14 gate** | Motor de compliance, dashboards de telemetria e documentacao devem chegar juntos antes de abrir o piloto de parceiros. | `docs/source/nexus_overview.md` + `docs/source/nexus_operations.md` publicados, dashboards com alertas, motor de politica conectado a governanca. |

## 4. Responsabilidades do operador

| Responsabilidade | Descricao | Evidencia |
|-----------------|-----------|----------|
| Higiene de config | Manter `config/config.toml` sincronizado com o catalogo publicado de lanes e dataspaces; registrar mudancas em tickets. | Saida do `irohad --sora --config ... --trace-config` arquivada com artefatos de release. |
| Rastreamento de manifestos | Monitorar atualizacoes do Space Directory e atualizar caches/allowlists locais. | Bundle de manifestos assinado armazenado com o ticket de onboarding. |
| Cobertura de telemetria | Garantir que os dashboards da Secao 2 estejam acessiveis, alertas conectados ao PagerDuty e revisoes trimestrais registradas. | Minutas de plantao + export do Alertmanager. |
| Relato de incidentes | Seguir a matriz de severidade em `docs/source/nexus_operations.md` e enviar relatorios pos-incidente em cinco dias uteis. | Template pos-incidente arquivado por ID de incidente. |
| Preparacao de governanca | Participar de votos do conselho Nexus quando mudancas de politica de lanes afetarem o deploy; ensaiar instrucoes de rollback trimestralmente. | Presenca do conselho + checklist de ensaio em `docs/source/project_tracker/nexus_config_deltas/`. |

## 5. Mapa de documentacao relacionada

- **Especificacao detalhada:** `docs/source/nexus.md`
- **Geometria de lanes e storage:** `docs/source/nexus_lanes.md`
- **Plano de transicao e roteamento temporario:** `docs/source/nexus_transition_notes.md`
- **Onboarding de operadores:** `docs/source/sora_nexus_operator_onboarding.md`
- **Politica de lanes CBDC e plano de settlement:** `docs/source/cbdc_lane_playbook.md`
- **Remediacao de telemetria e mapa de dashboards:** `docs/source/nexus_telemetry_remediation_plan.md`
- **Runbook / processo de incidentes:** `docs/source/nexus_operations.md`

Manter este resumo alinhado ao item de roadmap NX-14 quando houver mudancas substanciais
nos documentos vinculados ou quando novas classes de lanes ou fluxos de governanca
forem introduzidos.
