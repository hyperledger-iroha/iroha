---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

> Adaptado de [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Politica de admissao e identidade de provedores SoraFS (Rascunho SF-2b)

Esta nota captura os entregaveis acionaveis para **SF-2b**: definir e
aplicar o fluxo de admissao, os requisitos de identidade e os payloads de
atestacao para provedores de armazenamento SoraFS. Ela amplia o processo de alto
nivel descrito no RFC de Arquitetura de SoraFS e divide o trabalho restante em
tarefas de engenharia rastreaveis.

## Objetivos da politica

- Garantir que apenas operadores verificados possam publicar registros `ProviderAdvertV1` que a rede aceita.
- Vincular cada chave de anuncio a um documento de identidade aprovado pela governanca, endpoints atestados e contribuicao minima de stake.
- Fornecer ferramentas de verificacao deterministica para que Torii, gateways e `sorafs-node` apliquem as mesmas verificacoes.
- Suportar renovacao e revogacao de emergencia sem quebrar o determinismo ou a ergonomia das ferramentas.

## Requisitos de identidade e stake

| Requisito | Descricao | Entregavel |
|-----------|-----------|------------|
| Proveniencia da chave de anuncio | Os provedores devem registrar um par de chaves Ed25519 que assina cada advert. O bundle de admissao armazena a chave publica junto com uma assinatura de governanca. | Estender o esquema `ProviderAdmissionProposalV1` com `advert_key` (32 bytes) e referencia-lo no registro (`sorafs_manifest::provider_admission`). |
| Ponteiro de stake | A admissao requer um `StakePointer` nao zero apontando para um pool de staking ativo. | Adicionar validacao em `sorafs_manifest::provider_advert::StakePointer::validate()` e expor erros em CLI/tests. |
| Tags de jurisdicao | Os provedores declaram jurisdicao + contato legal. | Estender o esquema de proposta com `jurisdiction_code` (ISO 3166-1 alpha-2) e `contact_uri` opcional. |
| Atestacao de endpoint | Cada endpoint anunciado deve ser respaldado por um relatorio de certificado mTLS ou QUIC. | Definir payload Norito `EndpointAttestationV1` e armazena-lo por endpoint dentro do bundle de admissao. |

## Fluxo de admissao

1. **Criacao da proposta**
   - CLI: adicionar `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     produzindo `ProviderAdmissionProposalV1` + bundle de atestacao.
   - Validacao: garantir campos requeridos, stake > 0, handle canonico de chunker em `profile_id`.
2. **Endosso de governanca**
   - O conselho assina `blake3("sorafs-provider-admission-v1" || canonical_bytes)` usando o tooling de envelope existente
     (modulo `sorafs_manifest::governance`).
   - O envelope e persistido em `governance/providers/<provider_id>/admission.json`.
3. **Ingestao no registro**
   - Implementar um verificador compartilhado (`sorafs_manifest::provider_admission::validate_envelope`)
     que Torii/gateways/CLI reutilizam.
   - Atualizar o caminho de admissao do Torii para rejeitar adverts cujo digest ou expiracao difere do envelope.
4. **Renovacao e revogacao**
   - Adicionar `ProviderAdmissionRenewalV1` com atualizacoes opcionais de endpoint/stake.
   - Expor um caminho CLI `--revoke` que registra o motivo da revogacao e envia um evento de governanca.

## Tarefas de implementacao

| Area | Tarefa | Owner(s) | Status |
|------|--------|----------|--------|
| Esquema | Definir `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) em `crates/sorafs_manifest/src/provider_admission.rs`. Implementado em `sorafs_manifest::provider_admission` com helpers de validacao.[F:crates/sorafs_manifest/src/provider_admission.rs#L1] | Storage / Governance | Concluido |
| Ferramentas CLI | Estender `sorafs_manifest_stub` com subcomandos: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Tooling WG | Concluido |

O fluxo de CLI agora aceita bundles de certificados intermediarios (`--endpoint-attestation-intermediate`), emite
bytes canonicos de proposta/envelope e valida assinaturas do conselho durante `sign`/`verify`. Operadores podem
fornecer corpos de advert diretamente ou reutilizar adverts assinados, e arquivos de assinatura podem ser
fornecidos ao combinar `--council-signature-public-key` com `--council-signature-file` para facilitar a automacao.

### Referencia de CLI

Execute cada comando via `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.

- `proposal`
  - Flags requeridas: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, e pelo menos um `--endpoint=<kind:host>`.
  - A atestacao por endpoint espera `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, um certificado via
    `--endpoint-attestation-leaf=<path>` (mais `--endpoint-attestation-intermediate=<path>`
    opcional para cada elemento da cadeia) e quaisquer IDs ALPN negociados
    (`--endpoint-attestation-alpn=<token>`). Endpoints QUIC podem fornecer relatorios de transporte com
    `--endpoint-attestation-report[-hex]=...`.
  - Saida: bytes canonicos de proposta Norito (`--proposal-out`) e um resumo JSON
    (stdout padrao ou `--json-out`).
- `sign`
  - Entradas: uma proposta (`--proposal`), um advert assinado (`--advert`), corpo de advert opcional
    (`--advert-body`), epoca de retencao e pelo menos uma assinatura do conselho. As assinaturas podem
    ser fornecidas inline (`--council-signature=<signer_hex:signature_hex>`) ou via arquivos ao combinar
    `--council-signature-public-key` com `--council-signature-file=<path>`.
  - Produz um envelope validado (`--envelope-out`) e um relatorio JSON indicando vinculacoes de digest,
    contagem de assinantes e caminhos de entrada.
- `verify`
  - Valida um envelope existente (`--envelope`), com verificacao opcional da proposta, advert ou
    corpo de advert correspondente. O relatorio JSON destaca valores de digest, status de verificacao
    de assinaturas e quais artefatos opcionais corresponderam.
- `renewal`
  - Vincula um envelope recem aprovado ao digest previamente ratificado. Requer
    `--previous-envelope=<path>` e o sucessor `--envelope=<path>` (ambos payloads Norito).
    O CLI verifica que aliases de perfil, capacidades e chaves de advert permanecem inalterados,
    enquanto permite atualizacoes de stake, endpoints e metadata. Emite os bytes canonicos
    `ProviderAdmissionRenewalV1` (`--renewal-out`) mais um resumo JSON.
- `revoke`
  - Emite um bundle de emergencia `ProviderAdmissionRevocationV1` para um provedor cujo envelope deve
    ser retirado. Requer `--envelope=<path>`, `--reason=<text>`, pelo menos uma `--council-signature`,
    e `--revoked-at`/`--notes` opcionais. O CLI assina e valida o digest de revogacao, escreve o payload
    Norito via `--revocation-out` e imprime um resumo JSON com o digest e o numero de assinaturas.
| Verificacao | Implementar verificador compartilhado usado por Torii, gateways e `sorafs-node`. Prover testes unitarios + de integracao de CLI.[F:crates/sorafs_manifest/src/provider_admission.rs#L1][F:crates/iroha_torii/src/sorafs/admission.rs#L1] | Networking TL / Storage | Concluido |
| Integracao Torii | Passar o verificador na ingestao de adverts no Torii, rejeitar adverts fora de politica e emitir telemetria. | Networking TL | Concluido | Torii agora carrega envelopes de governanca (`torii.sorafs.admission_envelopes_dir`), verifica correspondencias de digest/assinatura durante a ingestao e expoe telemetria de admissao.[F:crates/iroha_torii/src/sorafs/admission.rs#L1][F:crates/iroha_torii/src/sorafs/discovery.rs#L1][F:crates/iroha_torii/src/sorafs/api.rs#L1] |
| Renovacao | Adicionar esquema de renovacao/revogacao + helpers de CLI, publicar guia de ciclo de vida nos docs (ver o runbook abaixo e comandos CLI em `provider-admission renewal`/`revoke`).[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][docs/source/sorafs/provider_admission_policy.md:120] | Storage / Governance | Concluido |
| Telemetria | Definir dashboards/alertas `provider_admission` (renovacao ausente, expiracao de envelope). | Observability | Em progresso | O contador `torii_sorafs_admission_total{result,reason}` existe; dashboards/alertas pendentes.[F:crates/iroha_telemetry/src/metrics.rs#L3798][F:docs/source/telemetry.md#L614] |

### Runbook de renovacao e revogacao

#### Renovacao agendada (atualizacoes de stake/topologia)
1. Construa o par proposta/advert sucessor com `provider-admission proposal` e `provider-admission sign`,
   aumentando `--retention-epoch` e atualizando stake/endpoints conforme necessario.
2. Execute
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   O comando valida campos de capacidade/perfil inalterados via `AdmissionRecord::apply_renewal`,
   emite `ProviderAdmissionRenewalV1` e imprime digests para o log de governanca.[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][F:crates/sorafs_manifest/src/provider_admission.rs#L422]
3. Substitua o envelope anterior em `torii.sorafs.admission_envelopes_dir`, confirme o Norito/JSON de renovacao
   no repositorio de governanca e adicione o hash de renovacao + retention epoch a `docs/source/sorafs/migration_ledger.md`.
4. Notifique os operadores que o novo envelope esta ativo e monitore
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` para confirmar a ingestao.
5. Regenere e confirme os fixtures canonicos via `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`;
   CI (`ci/check_sorafs_fixtures.sh`) valida que as saidas Norito permanecem estaveis.

#### Revogacao de emergencia
1. Identifique o envelope comprometido e emita uma revogacao:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   O CLI assina o `ProviderAdmissionRevocationV1`, verifica o conjunto de assinaturas via
   `verify_revocation_signatures` e relata o digest de revogacao.[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593][F:crates/sorafs_manifest/src/provider_admission.rs#L486]
2. Remova o envelope de `torii.sorafs.admission_envelopes_dir`, distribua o Norito/JSON de revogacao para caches
   de admissao e registre o hash do motivo nas atas de governanca.
3. Observe `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` para confirmar que os
   caches descartaram o advert revogado; mantenha os artefatos de revogacao nas retrospectivas de incidentes.

## Testes e telemetria

- Adicionar fixtures golden para propostas e envelopes de admissao em
  `fixtures/sorafs_manifest/provider_admission/`.
- Estender CI (`ci/check_sorafs_fixtures.sh`) para regenerar propostas e verificar envelopes.
- Os fixtures gerados incluem `metadata.json` com digests canonicos; testes downstream afirmam
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Prover testes de integracao:
  - Torii rejeita adverts com envelopes de admissao ausentes ou expirados.
  - O CLI faz round-trip de proposta -> envelope -> verificacao.
  - A renovacao de governanca rotaciona a atestacao de endpoint sem alterar o ID do provedor.
- Requisitos de telemetria:
  - Emitir contadores `provider_admission_envelope_{accepted,rejected}` em Torii. `torii_sorafs_admission_total{result,reason}` agora expoe resultados accepted/rejected.
  - Adicionar alertas de expiracao a dashboards de observabilidade (renovacao devida dentro de 7 dias).

## Proximos passos

1. As alteracoes do esquema Norito foram finalizadas e os helpers de validacao foram incorporados em `sorafs_manifest::provider_admission`. Nao ha feature flags.
2. Os fluxos CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) estao documentados e exercitados via testes de integracao; mantenha os scripts de governanca sincronizados com o runbook.
3. Torii admission/discovery ingere os envelopes e expoe contadores de telemetria para aceitacao/rejeicao.
4. Foco em observabilidade: concluir dashboards/alertas de admissao para que renovacoes devidas dentro de sete dias disparem avisos (`torii_sorafs_admission_total`, expiry gauges).
