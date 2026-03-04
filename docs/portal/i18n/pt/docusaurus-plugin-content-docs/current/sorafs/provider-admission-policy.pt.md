---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Adaptado de [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Política de admissão e identidade de provedores SoraFS (Rascunho SF-2b)

Esta nota captura os entregaveis acionaveis para **SF-2b**: definir e
aplicar o fluxo de admissão, os requisitos de identidade e as cargas úteis de
atestado para provedores de armazenamento SoraFS. Ela amplia o processo de alto
nível descrito no RFC de Arquitetura de SoraFS e divide o trabalho restante em
tarefas de engenharia rastreáveis.

## Objetivos da política

- Garantir que apenas os operadores selecionados possam publicar os registros `ProviderAdvertV1` que a rede aceita.
- Vincular cada chave de anúncio a um documento de identidade aprovado pela governança, pontos finais atestados e contribuição mínima de participação.
- Fornecer ferramentas de verificação determinística para que Torii, gateways e `sorafs-node` apliquem as mesmas verificações.
- Apoiar a renovação e revogação de emergência sem quebrar o determinismo ou a ergonomia das ferramentas.

## Requisitos de identidade e participação

| Requisito | Descrição | Entregavel |
|-----------|-----------|-----------|
| Proveniência da chave de anúncio | Os provedores devem registrar um par de chaves Ed25519 que assinam cada anúncio. O pacote de admissão armazena a chave pública junto com uma assinatura de governança. | Estender o esquema `ProviderAdmissionProposalV1` com `advert_key` (32 bytes) e referenciá-lo no registro (`sorafs_manifest::provider_admission`). |
| Ponteiro de estaca | A admissão requer um `StakePointer` e não zero apontar para um pool de staking ativo. | Adicione validação em `sorafs_manifest::provider_advert::StakePointer::validate()` e exponha erros em CLI/testes. |
| Etiquetas de jurisdição | Os provedores declaram jurisdição + contato legal. | Estender o esquema de proposta com `jurisdiction_code` (ISO 3166-1 alpha-2) e `contact_uri` opcional. |
| Atestação de endpoint | Cada endpoint anunciado deverá ser respaldado por um relatório de certificado mTLS ou QUIC. | Defina payload Norito `EndpointAttestationV1` e armazena-lo por endpoint dentro do pacote de admissão. |

## Fluxo de admissão

1. **Criação da proposta**
   - CLI: adicionar `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     produzindo `ProviderAdmissionProposalV1` + pacote de atestado.
   - Validação: garantir campos requeridos, stake > 0, handle canonico de chunker em `profile_id`.
2. **Endosso de governança**
   - O conselho assina `blake3("sorafs-provider-admission-v1" || canonical_bytes)` usando as ferramentas do envelope existente
     (módulo `sorafs_manifest::governance`).
   - O envelope e persistido em `governance/providers/<provider_id>/admission.json`.
3. **Ingestão sem registro**
   - Implementar um selecionador compartilhado (`sorafs_manifest::provider_admission::validate_envelope`)
     que Torii/gateways/CLI reutilizam.
   - Atualizar o caminho de admissão do Torii para rejeitar anúncios cujo resumo ou expiração difere do envelope.
4. **Renovação e revogação**
   - Adicionar `ProviderAdmissionRenewalV1` com atualizações adicionais de endpoint/stake.
   - Expor um caminho CLI `--revoke` que registra o motivo da revogação e envia um evento de governança.

## Tarefas de implementação| Área | Tarefa | Proprietário(s) | Estado |
|------|--------|----------|--------|
| Esquema | Defina `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) em `crates/sorafs_manifest/src/provider_admission.rs`. Implementado em `sorafs_manifest::provider_admission` com helpers de validação.[F:crates/sorafs_manifest/src/provider_admission.rs#L1] | Armazenamento / Governança | Concluído |
| Ferramentas CLI | Estender `sorafs_manifest_stub` com subcomandos: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | GT Ferramentaria | Concluído |

O fluxo de CLI agora aceita pacotes de certificados intermediários (`--endpoint-attestation-intermediate`), emite
bytes canônicos de proposta/envelope e valida assinaturas do conselho durante `sign`/`verify`. Operadores podem
fornecer corpos de anúncio diretamente ou reutilizar anúncios assinados, e arquivos de assinatura podem ser
fornecido ao combinar `--council-signature-public-key` com `--council-signature-file` para facilitar a automação.

### Referência de CLI

Execute cada comando via `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - Flags requeridas: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, e pelo menos um `--endpoint=<kind:host>`.
  - Um atestado de endpoint espera `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, um certificado via
    `--endpoint-attestation-leaf=<path>` (mais `--endpoint-attestation-intermediate=<path>`
    opcional para cada elemento da cadeia) e quaisquer IDs ALPN negociados
    (`--endpoint-attestation-alpn=<token>`). Endpoints QUIC podem fornecer relatórios de transporte com
    `--endpoint-attestation-report[-hex]=...`.
  - Saida: bytes canônicos da proposta Norito (`--proposal-out`) e um resumo JSON
    (padrão padrão ou `--json-out`).
-`sign`
  - Entradas: uma proposta (`--proposal`), um anúncio aprovado (`--advert`), corpo de anúncio opcional
    (`--advert-body`), época de retenção e pelo menos uma assinatura do conselho. As assinaturas podem
    ser fornecido inline (`--council-signature=<signer_hex:signature_hex>`) ou via arquivos ao combinar
    `--council-signature-public-key` com `--council-signature-file=<path>`.
  - Produz um envelope validado (`--envelope-out`) e um relatório JSON rebaixando vinculações de resumo,
    contagem de assinantes e caminhos de entrada.
-`verify`
  - Valida um envelope existente (`--envelope`), com verificação opcional da proposta, anúncio ou
    corpo do anúncio correspondente. O relatório JSON destaca valores de resumo, status de verificação
    de assinaturas e quais artefatos correspondiam.
-`renewal`
  - Vincula um envelope recentemente aprovado ao resumo previamente ratificado. Solicitar
    `--previous-envelope=<path>` e o sucessor `--envelope=<path>` (ambos payloads Norito).
    O CLI verifica que aliases de perfil, capacidades e chaves de anúncio permanecem inalterados,
    enquanto permite atualizações de stake, endpoints e metadados. Emite os bytes canônicos
    `ProviderAdmissionRenewalV1` (`--renewal-out`) mais um resumo JSON.
-`revoke`
  - Emite um pacote de emergência `ProviderAdmissionRevocationV1` para um provedor cujo envelope deve
    ser retirado. Solicite `--envelope=<path>`, `--reason=<text>`, pelo menos uma `--council-signature`,
    e `--revoked-at`/`--notes` perguntas. O CLI assina e valida o resumo de revogação, escreve o payload
    Norito via `--revocation-out` e imprima um resumo JSON com o resumo e o número de assinaturas.
| Verificação | Implementar verificador compartilhado usado por Torii, gateways e `sorafs-node`. Prover testes unitários + de integração de CLI.[F:crates/sorafs_manifest/src/provider_admission.rs#L1][F:crates/iroha_torii/src/sorafs/admission.rs#L1] | Rede TL / Armazenamento | Concluído |
| Integração Torii | Passar ou selecionar na ingestão de anúncios no Torii, rejeitar anúncios fora de política e emitir telemetria. | Rede TL | Concluído | Torii agora carrega envelopes de governança (`torii.sorafs.admission_envelopes_dir`), verifica correspondências de digest/assinatura durante a ingestão e exposição de telemetria de admissao.[F:crates/iroha_torii/src/sorafs/admission.rs#L1][F:crates/iroha_torii/src/sorafs/discovery.rs#L1][F:crates/iroha_torii/src/sorafs/api.rs#L1] || Renovação | Adicione esquema de renovação/revogação + helpers de CLI, publique guia de ciclo de vida nos documentos (veja o runbook abaixo e comandos CLI em `provider-admission renewal`/`revoke`).[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][docs/source/sorafs/provider_admission_policy.md:120] | Armazenamento / Governança | Concluído |
| Telemetria | dashboards/alertas `provider_admission` (renovação ausente, expiração de envelope). | Observabilidade | Em progresso | O contador `torii_sorafs_admission_total{result,reason}` existe; dashboards/alertas pendentes.[F:crates/iroha_telemetry/src/metrics.rs#L3798][F:docs/source/telemetry.md#L614] |

### Runbook de renovação e revogação

#### Renovação agendada (atualizações de estaca/topologia)
1. Construa o par proposta/anúncio sucessor com `provider-admission proposal` e `provider-admission sign`,
   aumentando `--retention-epoch` e atualizando stake/endpoints conforme necessário.
2. Executar
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
   emite `ProviderAdmissionRenewalV1` e imprime digests para o log de governança.[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][F:crates/sorafs_manifest/src/provider_admission.rs#L422]
3. Substitua o envelope anterior em `torii.sorafs.admission_envelopes_dir`, confirme o Norito/JSON de renovação
   no repositório de governança e adicionado o hash de renovação + época de retenção a `docs/source/sorafs/migration_ledger.md`.
4. Notifique os operadores de que o novo envelope está ativo e monitore
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` para confirmar a ingestão.
5. Regenere e confirme os fixtures canônicos via `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`;
   CI (`ci/check_sorafs_fixtures.sh`) valida que as saidas Norito permanecem estaveis.

#### Revogação de emergência
1. Identifique o envelope comprometido e emita uma revogação:
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
   `verify_revocation_signatures` e relata o resumo de revogação.[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593][F:crates/sorafs_manifest/src/provider_admission.rs#L486]
2. Remova o envelope de `torii.sorafs.admission_envelopes_dir`, distribuindo o Norito/JSON de revogação para caches
   de admissão e registro o hash do motivo nas atas de governança.
3. Observe `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` para confirmar que os
   caches descartaram o anúncio revogado; mantenha os artistas de revogação nas retrospectivas de incidentes.

## Testes e telemetria- Adicionar luminárias douradas para propostas e envelopes de admissão em
  `fixtures/sorafs_manifest/provider_admission/`.
- Estender CI (`ci/check_sorafs_fixtures.sh`) para regenerar propostas e verificar envelopes.
- Os fixtures gerados incluem `metadata.json` com resumos canônicos; testes a jusante afirmam
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Provar testes de integração:
  - Torii rejeita anúncios com envelopes de admissão ausentes ou expirados.
  - O CLI faz ida e volta de proposta -> envelope -> verificação.
  - Uma renovação de governança rotaciona uma atestação de endpoint sem alteração do ID do provedor.
- Requisitos de telemetria:
  - Emitir contadores `provider_admission_envelope_{accepted,rejected}` em Torii. `torii_sorafs_admission_total{result,reason}` agora expoe resultados aceitos/rejeitados.
  - Adicionar alertas de expiração a dashboards de observabilidade (renovação de dívida em 7 dias).

## Próximos passos

1. As alterações do esquema Norito foram finalizadas e os auxiliares de validação foram incorporados em `sorafs_manifest::provider_admission`. Nao ha apresenta bandeiras.
2. Os fluxos CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) estão documentados e exercitados via testes de integração; mantenha os scripts de governança sincronizados com o runbook.
3. Torii admissão/descoberta ingere os envelopes e expõe contadores de telemetria para aceitação/rejeição.
4. Foco em observabilidade: concluir dashboards/alertas de admissão para que renovações devidas dentro de sete dias disparem avisos (`torii_sorafs_admission_total`, medidores de validade).