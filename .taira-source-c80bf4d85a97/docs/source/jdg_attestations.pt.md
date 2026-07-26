---
lang: pt
direction: ltr
source: docs/source/jdg_attestations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 459e8ed4612da7cfa68053e4e299b2f68e7620d4f3b98a8a721ebf8327829ea1
source_last_modified: "2026-01-09T07:05:10.922933+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Atestados JDG: Guarda, Rotação e Retenção

Esta nota documenta o protetor de atestado JDG v1 que agora é enviado em `iroha_core`.

- **Manifestos do comitê:** Pacotes `JdgCommitteeManifest` codificados em Norito transportam rotação por espaço de dados
  programações (`committee_id`, membros ordenados, limite, `activation_height`, `retire_height`).
  Os manifestos são carregados com `JdgCommitteeSchedule::from_path` e impõem aumento estrito
  alturas de ativação com uma sobreposição de tolerância opcional (`grace_blocks`) entre desativação/ativação
  comitês.
- **Proteção de atestado:** `JdgAttestationGuard` impõe vinculação de espaço de dados, expiração, limites obsoletos,
  correspondência de id/limite de comitê, associação de signatário, esquemas de assinatura suportados e opcional
  Validação SDN via `JdgSdnEnforcer`. Limites de tamanho, atraso máximo e esquemas de assinatura permitidos são
  parâmetros do construtor; `validate(attestation, dataspace, current_height)` retorna o ativo
  comitê ou um erro estruturado.
  - `scheme_id = 1` (`simple_threshold`): assinaturas por signatário, bitmap de signatário opcional.
  - `scheme_id = 2` (`bls_normal_aggregate`): assinatura normal BLS pré-agregada única sobre o
    hash de atestado; bitmap do signatário opcional, o padrão é todos os signatários no atestado. BLS
    a validação agregada requer um PoP válido por membro do comitê no manifesto; faltando ou
    PoPs inválidos rejeitam o atestado.
  Configure a lista de permissões por meio de `governance.jdg_signature_schemes`.
- **Armazenamento de retenção:** `JdgAttestationStore` rastreia atestados por espaço de dados com um configurável
  limite por espaço de dados, eliminando as entradas mais antigas na inserção. Ligue para `for_dataspace` ou
  `for_dataspace_and_epoch` para recuperar pacotes configuráveis de auditoria/reprodução.
- **Testes:** A cobertura da unidade agora exerce seleção de comitê válido, rejeição de signatário desconhecido, obsoleto
  rejeição de atestado, ids de esquema não suportados e remoção de retenção. Veja
  `crates/iroha_core/src/jurisdiction.rs`.

O guarda rejeita esquemas fora da lista de permissões configurada.