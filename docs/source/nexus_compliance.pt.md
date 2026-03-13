---
lang: pt
direction: ltr
source: docs/source/nexus_compliance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5635794e962a9fb1b94c5ff550dc198a744a64b4f9f05df588cb70621e9237f9
source_last_modified: "2025-11-21T18:31:26.542844+00:00"
translation_last_reviewed: 2026-01-01
---

# Motor de conformidade de lanes Nexus e politica de lista branca (NX-12)

Status: Implementado — este documento captura o modelo de politica em producao e a aplicacao critica
para consenso referenciada pelo item de roadmap **NX-12 - Motor de conformidade de lane e politica de lista branca**.
Ele explica o modelo de dados, os fluxos de governanca, a telemetria e a estrategia de rollout
implementados em `crates/iroha_core/src/compliance` e aplicados tanto na
admissao do Torii quanto na validacao de transacoes de `iroha_core`, para que cada lane e dataspace possa ficar vinculado a
politicas jurisdicionais deterministicas.

## Objetivos

- Permitir que a governanca anexe regras allow/deny, flags jurisdicionais, limites de transferencia CBDC
  e requisitos de auditoria a cada manifesto de lane.
- Avaliar cada transacao contra essas regras durante a admissao no Torii e a execucao do bloco,
  garantindo a aplicacao deterministica da politica entre os nos.
- Produzir uma trilha de auditoria criptograficamente verificavel, com bundles de evidencia Norito
  e telemetria consultavel para reguladores e operadores.
- Manter o modelo flexivel: o mesmo motor de politicas cobre lanes CBDC privadas,
  DS de settlement publicos e dataspaces hibridos de parceiros sem forks sob medida.

## Nao-Objetivos

- Definir procedimentos AML/KYC ou fluxos legais de escalonamento. Isso vive nos playbooks de conformidade
  que consomem a telemetria produzida aqui.
- Introduzir toggles por instrucao na IVM; o motor so controla quais contas/ativos/domains
  podem enviar transacoes ou interagir com uma lane.
- Tornar o Space Directory obsoleto. Os manifestos continuam sendo a fonte autoritativa
  de metadados de DS; a politica de conformidade apenas referencia entradas do Space Directory
  e as complementa.

## Modelo de politica

### Entidades e identificadores

O motor de politicas opera sobre:

- `LaneId` / `DataSpaceId` - identifica o escopo onde as regras se aplicam.
- `UniversalAccountId (UAID)` - permite agrupar identidades cross-lane.
- `JurisdictionFlag` - bitmask que enumera classificacoes reguladoras (p. ex.
  `EU_EEA`, `JP_FIEL`, `US_FED`, `SANCTIONS_SCREENED`).
- `ParticipantSelector` - descreve quem e afetado:
  - `AccountId`, `DomainId` ou `UAID`.
  - Seletores baseados em prefixo (`DomainPrefix`, `UaidPrefix`) para coincidir com registros.
  - `CapabilityTag` para manifestos do Space Directory (p. ex. apenas DS com FX-cleared).
  - gating `privacy_commitments_any_of` para exigir que as lanes anunciem compromissos de privacidade Nexus
    antes que as regras coincidam (reflete a superficie de manifesto NX-10 e e aplicado em
    snapshots de `LanePrivacyRegistry`).

### LaneCompliancePolicy

As politicas sao structs Norito publicadas via governanca:

```text
LaneCompliancePolicy {
    id: LaneCompliancePolicyId,
    version: u32,
    lane_id: LaneId,
    jurisdiction: JurisdictionSet,
    allow: Vec<AllowRule>,
    deny: Vec<DenyRule>,
    transfer_limits: Vec<TransferLimit>,
    audit_controls: AuditControls,
    metadata: MetadataMap,
}
```

- `AllowRule` combina um `ParticipantSelector`, override jurisdicional opcional,
  capability tags e codigos de motivo.
- `DenyRule` espelha a estrutura allow, mas e avaliado primeiro (deny vence).
- `TransferLimit` captura limites especificos por ativo/bucket:
  - `max_notional_xor` e `max_daily_notional_xor`.
  - `asset_limits[{asset_id, per_tx, per_day}]`.
  - `relationship_limits` (p. ex. CBDC retail vs wholesale).
- `AuditControls` configura:
  - Se o Torii deve persistir cada negacao no log de auditoria.
  - Se decisoes bem sucedidas devem ser amostradas em digests Norito.
  - Janela de retencao exigida para `LaneComplianceDecisionRecord`.

### Armazenamento e distribuicao

- Os hashes de politica mais recentes vivem no manifesto do Space Directory junto com as chaves
  de validadores. `LaneCompliancePolicyReference` (policy id + version + hash) vira um campo
  de manifesto para que validadores e SDKs possam buscar o blob de politica canonico.
- `iroha_config` expoe `compliance.policy_cache_dir` para persistir o payload Norito e sua assinatura
  destacada. Os nos verificam assinaturas antes de aplicar atualizacoes para se proteger contra
  adulteracao.
- As politicas tambem sao embutidas nos manifestos de admissao Norito usados pelo Torii
  para que CI/SDKs possam reproduzir a avaliacao de politicas sem falar com validadores.

## Governanca e ciclo de vida

1. **Proposta** - a governanca envia `ProposeLaneCompliancePolicy` com o payload Norito,
   a justificativa jurisdicional e a epoca de ativacao.
2. **Revisao** - revisores de conformidade assinam `LaneCompliancePolicyReviewEvidence`
   (auditavel, armazenado em `governance::ReviewEvidenceStore`).
3. **Ativacao** - apos a janela de atraso, validadores ingerem a politica chamando
   `ActivateLaneCompliancePolicy`. O manifesto do Space Directory e atualizado de forma atomica
   com a nova referencia de politica.
4. **Emenda/Revogacao** - `AmendLaneCompliancePolicy` carrega metadados de diff enquanto mantem a
   versao anterior para replay forense; `RevokeLaneCompliancePolicy` fixa o policy id em `denied`
   para que o Torii rejeite qualquer trafego direcionado a essa lane ate que um substituto seja ativado.

Torii expoe:

- `GET /v2/lane-compliance/policies/{lane_id}` - busca a referencia de politica mais recente.
- `POST /v2/lane-compliance/policies` - endpoint apenas governanca que espelha os helpers de proposta ISI.
- `GET /v2/lane-compliance/decisions` - log de auditoria paginado com filtros para
  `lane_id`, `decision`, `jurisdiction` e `reason_code`.

Comandos CLI/SDK envolvem essas superficies HTTP para que operadores possam automatizar revisoes
e obter artefatos (blob de politica assinado + atestacoes de revisores).

## Pipeline de enforcement

1. **Admissao (Torii)**
   - `Torii` baixa a politica ativa quando um manifesto de lane muda ou quando a assinatura em cache expira.
   - Cada transacao que entra na fila `/v2/pipeline` e marcada com `LaneComplianceContext`
     (ids de participante, UAID, metadados do manifesto de dataspace, policy id e o snapshot mais recente
     de `LanePrivacyRegistry` descrito em `crates/iroha_core/src/interlane/mod.rs`).
   - Autoridades com UAID devem ter um manifesto ativo do Space Directory para o dataspace roteado;
     o Torii rejeita transacoes quando o UAID nao esta vinculado a esse dataspace antes de avaliar
     qualquer regra de politica.
   - O `compliance::Engine` avalia regras `deny`, depois regras `allow`, e por fim aplica limites
     de transferencia. Transacoes com falha retornam um erro tipado (`ERR_LANE_COMPLIANCE_DENIED`)
     com motivo + policy id para trilhas de auditoria.
   - A admissao e um prefiltro rapido; a validacao de consenso volta a checar as mesmas regras usando
     snapshots do estado para manter a aplicacao deterministica.
2. **Execucao (iroha_core)**
   - Durante a construcao do bloco, `iroha_core::tx::validate_transaction_internal`
     reproduz as mesmas verificacoes de governanca/UAID/privacidade/conformidade de lane usando os
     snapshots de `StateTransaction` (`lane_manifests`, `lane_privacy_registry`,
     `lane_compliance`). Isso mantem a aplicacao critica para consenso mesmo se os caches do Torii
     ficarem obsoletos.
   - Transacoes que mutam manifestos de lane ou politicas de conformidade seguem o mesmo caminho de
     validacao; nao ha bypass apenas na admissao.
3. **Hooks async**
   - RBC gossip e os fetchers de DA anexam o policy id a telemetria para que decisoes tardias
     possam ser rastreadas para a versao correta de regra.
   - `iroha_cli` e os helpers SDK expoem `LaneComplianceDecision::explain()` para que a automacao
     possa renderizar diagnosticos legiveis.

O motor e deterministico e puro; ele nunca contata sistemas externos apos baixar o manifesto/politica.
Isso mantem os fixtures de CI e a reproducao multi-no simples.

## Auditoria e telemetria

- **Metricas**
  - `nexus_lane_policy_decisions_total{lane_id,decision,reason}`.
  - `nexus_lane_policy_rate_limited_total{lane_id,limit_kind}`.
  - `nexus_lane_policy_cache_age_seconds{lane_id}` (deve ficar < atraso de ativacao).
- **Logs**
  - Registros estruturados capturam `policy_id`, `version`, `participant`, `UAID`,
    flags jurisdicionais e o hash Norito da transacao infratora.
  - `LaneComplianceDecisionRecord` e codificado em Norito e persistido sob
    `world.compliance_logs::<lane_id>::<ts>::<nonce>` quando `AuditControls`
    solicita armazenamento duravel.
- **Bundles de evidencia**
  - `cargo xtask nexus-lane-audit` ganha um modo `--lane-compliance <path>` que mescla a politica,
    assinaturas de revisores, snapshot de metricas e o log de auditoria mais recente nas saidas
    JSON + Parquet. O flag espera um payload JSON no formato:

    ```json
    {
      "lanes": [
        {
          "lane_id": 12,
          "policy": { "...": "LaneCompliancePolicy JSON blob" },
          "reviewer_signatures": [
            {
              "reviewer": "auditor@example.com",
              "signature_hex": "deadbeef",
              "signed_at": "2026-02-12T09:00:00Z",
              "notes": "Q1 regulator packet"
            }
          ],
          "metrics_snapshot": {
            "nexus_lane_policy_decisions_total": {
              "allow": 42,
              "deny": 1
            }
          },
          "audit_log": [
            {
              "decision": "allow",
              "policy_id": "lane-12-policy",
              "recorded_at": "2026-02-12T09:00:00Z"
            }
          ]
        }
      ]
    }
    ```

    O CLI valida que cada blob `policy` combine com o `lane_id` listado no registro antes de embedar,
    evitando evidencia obsoleta ou desalinhada em pacotes reguladores e dashboards do roadmap.
  - `--markdown-out` (padrao `artifacts/nexus_lane_audit.md`) agora renderiza um resumo legivel
    que destaca lanes atrasadas, backlog nao zero, manifestos pendentes e evidencia de conformidade
    faltante para que os pacotes annex incluam tanto artefatos machine-readable quanto uma superficie
    rapida de revisao.

## Plano de rollout

1. **P0 - Somente observabilidade**
   - Enviar os tipos de politica, armazenamento, endpoints do Torii e metricas.
   - Torii avalia politicas em modo `audit` (sem enforcement) para coletar dados.
2. **P1 - Enforcement de deny/allow**
   - Habilitar falhas duras no Torii + execucao quando regras deny disparam.
   - Exigir politicas para todas as lanes CBDC; DS publicos podem continuar em modo audit.
3. **P2 - Limites e overrides jurisdicionais**
   - Ativar enforcement de limites de transferencia e flags jurisdicionais.
   - Alimentar a telemetria em `dashboards/grafana/nexus_lanes.json`.
4. **P3 - Automacao completa de conformidade**
   - Integrar exports de auditoria com consumidores `SpaceDirectoryEvent`.
   - Amarrar atualizacoes de politica aos runbooks de governanca e a automacao de releases.

## Aceitacao e testes

- Testes de integracao em `integration_tests/tests/nexus/compliance.rs` cobrem:
  - combinacoes allow/deny, overrides jurisdicionais e limites de transferencia;
  - corridas de ativacao manifesto/politica; e
  - paridade de decisao Torii vs `iroha_core` em execucoes multi-no.
- Testes unitarios em `crates/iroha_core/src/compliance` validam o motor de avaliacao puro,
  timers de invalidacao de cache e parsing de metadados.
- Atualizacoes de Docs/SDK (Torii + CLI) devem demonstrar a busca de politicas,
  envio de propostas de governanca, interpretacao de codigos de erro e coleta de evidencia
  de auditoria.

Encerrar o NX-12 exige os artefatos acima mais atualizacoes de status em
`status.md`/`roadmap.md` quando o enforcement estiver ativo nos clusters de staging.
