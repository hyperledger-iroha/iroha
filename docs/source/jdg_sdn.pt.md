---
lang: pt
direction: ltr
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2026-01-03T18:07:58.621058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% de atestados e rotação JDG-SDN

Esta nota captura o modelo de aplicação para atestados de Secret Data Node (SDN)
usado pelo fluxo Jurisdiction Data Guardian (JDG).

## Formato de compromisso
- `JdgSdnCommitment` vincula o escopo (`JdgAttestationScope`), o criptografado
  hash de carga útil e a chave pública SDN. Selos são assinaturas digitadas
  (`SignatureOf<JdgSdnCommitmentSignable>`) sobre a carga útil marcada com domínio
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`.
- A validação estrutural (`validate_basic`) impõe:
  -`version == JDG_SDN_COMMITMENT_VERSION_V1`
  - intervalos de blocos válidos
  - selos não vazios
  - igualdade de escopo em relação ao atestado quando executado via
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- A desduplicação é tratada pelo validador de atestado (signatário + hash de carga útil
  exclusividade) para evitar compromissos retidos/duplicados.

## Política de registro e rotação
- As chaves SDN residem em `JdgSdnRegistry`, codificadas por `(Algorithm, public_key_bytes)`.
- `JdgSdnKeyRecord` registra a altura de ativação, altura de retirada opcional,
  e chave pai opcional.
- A rotação é regida por `JdgSdnRotationPolicy` (atualmente: `dual_publish_blocks`
  janela de sobreposição). O registro de uma chave secundária atualiza a aposentadoria dos pais para
  `child.activation + dual_publish_blocks`, com guarda-corpos:
  - pais desaparecidos são rejeitados
  - as ativações devem estar aumentando estritamente
  - as sobreposições que excedem a janela de carência são rejeitadas
- Auxiliares de registro exibem os registros instalados (`record`, `keys`) para status
  e exposição à API.

## Fluxo de validação
- `JdgAttestation::validate_with_sdn_registry` envolve a estrutura
  verificações de atestado e aplicação de SDN. Tópicos `JdgSdnPolicy`:
  - `require_commitments`: impõe presença para cargas úteis PII/secretas
  - `rotation`: janela de carência usada ao atualizar a aposentadoria dos pais
- Cada compromisso é verificado quanto a:
  - validade estrutural + correspondência de escopo de atestado
  - presença de chave registrada
  - janela ativa cobrindo a faixa de bloqueio atestada (limites de aposentadoria já
    incluem a graça de publicação dupla)
  - selo válido sobre o corpo de compromisso marcado com domínio
- Erros estáveis aparecem no índice para evidência do operador:
  `MissingSdnCommitments`, `UnknownSdnKey`, `InactiveSdnKey`, `InvalidSeal`,
  ou falhas estruturais `Commitment`/`ScopeMismatch`.

## Runbook do operador
- **Provisão:** registre a primeira chave SDN com `activated_at` no ou antes do
  altura do primeiro bloco secreto. Publique a impressão digital da chave para os operadores JDG.
- **Rodar:** gerar a chave sucessora, registrá-la com `rotation_parent`
  apontando para a chave atual e confirme se a aposentadoria pai é igual
  `child_activation + dual_publish_blocks`. Selar novamente os compromissos de carga útil com
  a chave ativa durante a janela de sobreposição.
- **Auditoria:** expor instantâneos de registro (`record`, `keys`) via Torii/status
  superfícies para que os auditores possam confirmar a chave ativa e as janelas de retirada. Alerta
  se o intervalo atestado estiver fora da janela ativa.
- **Recuperação:** `UnknownSdnKey` → garantir que o registro inclua a chave de selamento;
  `InactiveSdnKey` → girar ou ajustar alturas de acionamento; `InvalidSeal` →
  selar novamente cargas úteis e atualizar atestados.## Ajudante de tempo de execução
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) empacota uma política +
  registro e valida atestados via `validate_with_sdn_registry`.
- Os registros podem ser carregados a partir de pacotes `JdgSdnKeyRecord` codificados em Norito (consulte
  `JdgSdnEnforcer::from_reader`/`from_path`) ou montado com
  `from_records`, que aplica os guarda-corpos de rotação durante o cadastro.
- Os operadores podem persistir o pacote Norito como evidência para Torii/status
  emergindo enquanto a mesma carga alimenta o executor usado pela admissão e
  guardas de consenso. Um único aplicador global pode ser inicializado na inicialização via
  `init_enforcer_from_path` e `enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  expor a política ativa + registros de chave para superfícies status/Torii.

## Testes
- Cobertura de regressão em `crates/iroha_data_model/src/jurisdiction.rs`:
  `sdn_registry_accepts_active_commitment`, `sdn_registry_rejects_unknown_key`,
  `sdn_registry_rejects_inactive_key`, `sdn_registry_rejects_bad_signature`,
  `sdn_registry_sets_parent_retirement_window`,
  `sdn_registry_rejects_overlap_beyond_policy`, juntamente com o existente
  testes de atestação estrutural/validação SDN.