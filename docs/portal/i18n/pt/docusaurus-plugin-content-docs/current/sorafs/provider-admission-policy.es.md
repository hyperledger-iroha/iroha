---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Adaptado de [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Política de admissão e identidade de provedores SoraFS (Borrador SF-2b)

Esta nota captura los entregables accionables para **SF-2b**: definir y
aplicar o fluxo de admissão, os requisitos de identidade e as cargas úteis de
atestado para provedores de armazenamento SoraFS. Amplia o processo de alto
nível descrito na RFC de Arquitetura de SoraFS e divide o trabalho restante
em tarefas de engenharia trazíveis.

## Objetivos da política

- Garantir que apenas os operadores selecionados possam publicar registros
  `ProviderAdvertV1` que o vermelho aceitará.
- Vincular cada chave de anúncio a um documento de identidade aprovado por
  governança, pontos finais atestados e uma contribuição mínima de participação.
- Experimente ferramentas de verificação determinista para Torii, gateways e
  `sorafs-node` aplique os controles errados.
- Suportar renovação e revogação de emergência sem quebrar o determinismo ni
  a ergonomia do ferramental.

## Requisitos de identidade e participação

| Requisito | Descrição | Entregável |
|-----------|-------------|------------|
| Procedimento da chave de anúncio | Os fornecedores devem registrar um par de chaves Ed25519 que firmam cada anúncio. O pacote de admissão armazena a chave pública junto com uma firma de governo. | Estenda o esquema `ProviderAdmissionProposalV1` com `advert_key` (32 bytes) e referencie-o desde o registro (`sorafs_manifest::provider_admission`). |
| Puntero de aposta | A admissão requer um `StakePointer` sem ser apuntando um pool de staking ativo. | Adiciona validação em `sorafs_manifest::provider_advert::StakePointer::validate()` e expõe erros em CLI/tests. |
| Etiquetas de jurisdição | Os provedores declaram jurisdição + contato legal. | Estenda o esquema de proposta com `jurisdiction_code` (ISO 3166-1 alfa-2) e `contact_uri` opcional. |
| Atestado de endpoint | Cada endpoint anunciado deverá ser respaldado por um relatório de certificado mTLS ou QUIC. | Defina a carga útil Norito `EndpointAttestationV1` e armazene-a por endpoint dentro do pacote de admissão. |

## Fluxo de admissão

1. **Criação de proposta**
   - CLI: añadir `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     produzindo `ProviderAdmissionProposalV1` + pacote de atestado.
   - Validação: garantir campos obrigatórios, stake > 0, identificador canônico de chunker em `profile_id`.
2. **Endoso de governo**
   - O conselho firma `blake3("sorafs-provider-admission-v1" || canonical_bytes)` usando as ferramentas
     do envelope existente (módulo `sorafs_manifest::governance`).
   - O envelope persiste em `governance/providers/<provider_id>/admission.json`.
3. **Ingestão do registro**
   - Implementar um selecionador compartilhado (`sorafs_manifest::provider_admission::validate_envelope`)
     que Torii/gateways/CLI é reutilizado.
   - Atualizar a rota de admissão em Torii para rechazar anúncios cujo resumo ou expiração diferente do envelope.
4. **Renovação e revogação**
   - Adicione `ProviderAdmissionRenewalV1` com atualizações opcionais de endpoint/stake.
   - Expor uma rota CLI `--revoke` que registra o motivo da revogação e envia um evento de governo.

## Tarefas de implementação| Área | Tara | Proprietário(s) | Estado |
|------|-------|----------|--------|
| Esquema | Defina `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) abaixo de `crates/sorafs_manifest/src/provider_admission.rs`. Implementado em `sorafs_manifest::provider_admission` com auxiliares de validação.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Armazenamento / Governança | ✅ Concluído |
| CLI de ferramentas | Extensor `sorafs_manifest_stub` com subcomandos: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | GT Ferramentaria | ✅ |

O fluxo de CLI agora aceita pacotes de certificados intermediários (`--endpoint-attestation-intermediate`), emite bytes canônicos de propuesta/envelope e valida firmas de consejo durante `sign`/`verify`. Os operadores podem fornecer códigos de anúncio diretamente ou reutilizar anúncios firmados, e os arquivos de empresa podem ser fornecidos combinando `--council-signature-public-key` com `--council-signature-file` para facilitar a automatização.

### Referência de CLI

Execute cada comando via `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - Sinalizadores necessários: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, e pelo menos um `--endpoint=<kind:host>`.
  - O endpoint atestado espera `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, um certificado via
    `--endpoint-attestation-leaf=<path>` (mais `--endpoint-attestation-intermediate=<path>`
    opcional para cada elemento da cadeia) e qualquer ID ALPN negociado
    (`--endpoint-attestation-alpn=<token>`). Endpoints QUIC podem fornecer relatórios de transporte com
    `--endpoint-attestation-report[-hex]=...`.
  - Saída: bytes canônicos da proposta Norito (`--proposal-out`) e um resumo JSON
    (stdout por defeito ou `--json-out`).
-`sign`
  - Entradas: uma proposta (`--proposal`), um anúncio firmado (`--advert`), espaço de anúncio opcional
    (`--advert-body`), época de retenção e pelo menos uma firma del consejo. Las firmas pueden
    suministrar inline (`--council-signature=<signer_hex:signature_hex>`) ou via arquivos combinando
    `--council-signature-public-key` com `--council-signature-file=<path>`.
  - Produzir um envelope validado (`--envelope-out`) e um relatório JSON rebaixar as ligações do resumo,
    conteúdo de firmantes e rotas de entrada.
-`verify`
  - Valide um envelope existente (`--envelope`), com verificação opcional da propriedade,
    anúncio ou corpo de anúncio correspondente. O relatório JSON destaca valores de resumo, estado
    de verificação de firmas e quais artefatos opcionais coincidem.
-`renewal`
  - Vincula um envelope recebido aprovado ao resumo previamente ratificado. Requer
    `--previous-envelope=<path>` e o sucessor `--envelope=<path>` (ambos cargas úteis Norito).
    El CLI verifica que os aliases de perfil, capacidades e chaves de anúncio permanecem sem mudanças,
    enquanto permite atualizações de participação, endpoints e metadados. Emite os bytes canônicos
    `ProviderAdmissionRenewalV1` (`--renewal-out`) mais um resumo JSON.
-`revoke`
  - Emita um pacote de emergência `ProviderAdmissionRevocationV1` para um provedor cujo envelope deve ser
    retirar. Requer `--envelope=<path>`, `--reason=<text>`, pelo menos um
    `--council-signature`, e opcionalmente `--revoked-at`/`--notes`. A CLI firma e valida o
    resumo da revogação, escreva a carga útil Norito via `--revocation-out` e imprima um relatório JSON
    com o resumo e o conteúdo de firmas.
| Verificação | Implementar verificador compartido usado por Torii, gateways e `sorafs-node`. Experimente testes unitários + integração de CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Rede TL / Armazenamento | ✅ Concluído || Integração Torii | Envie o selecionado na ingestão de anúncios em Torii, recuse anúncios fora da política e emita telemetria. | Rede TL | ✅ Concluído | Torii agora carrega envelopes de governo (`torii.sorafs.admission_envelopes_dir`), verifica coincidências de digest/firma durante a ingestão e expõe telemetria de admissão.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Renovação | Adicione o esquema de renovação/revocação + ajudantes de CLI, publique o guia de ciclo de vida em documentos (ver runbook abaixo e comandos CLI em `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Armazenamento / Governança | ✅ Concluído |
| Telemetria | Definir painéis/alertas `provider_admission` (renovação faltante, expiração de envelope). | Observabilidade | 🟠 Em progresso | O contador `torii_sorafs_admission_total{result,reason}` existe; dashboards/alertas pendentes.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook de renovação e revogação

#### Renovação programada (atualizações de estaca/topologia)
1. Construa a proposta/anúncio sucessor com `provider-admission proposal` e `provider-admission sign`, incrementando `--retention-epoch` e atualizando estacas/endpoints conforme necessário.
2. Ejecuta
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   O comando valida campos de capacidade/perfil sem mudanças via
   `AdmissionRecord::apply_renewal`, emite `ProviderAdmissionRenewalV1` e imprime resumos para o
   log de gobernanza.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Substitua o envelope anterior em `torii.sorafs.admission_envelopes_dir`, confirme o Norito/JSON de renovação no repositório de governo e adicione o hash de renovação + época de retenção em `docs/source/sorafs/migration_ledger.md`.
4. Notifique as operadoras de que o novo envelope está ativo e monitore `torii_sorafs_admission_total{result="accepted",reason="stored"}` para confirmar a ingestão.
5. Regenera e confirma os fixtures canônicos via `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`; CI (`ci/check_sorafs_fixtures.sh`) valida que as saídas Norito permanecem estáveis.

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
   A firma CLI `ProviderAdmissionRevocationV1`, verifica o conjunto de firmas via
   `verify_revocation_signatures`, e relata o resumo da revogação.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Retire o envelope de `torii.sorafs.admission_envelopes_dir`, distribua o Norito/JSON de revogação em caches de admissão e registre o hash do motivo nos atos de governança.
3. Observe `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` para confirmar que os caches descartados e o anúncio revogado; conservar os artefatos de revogação em retrospectivas de incidentes.

## Testes e telemetria- Añadir luminárias douradas para propostas e envelopes de admissão abaixo
  `fixtures/sorafs_manifest/provider_admission/`.
- Extender CI (`ci/check_sorafs_fixtures.sh`) para regenerar propostas e verificar envelopes.
- Os equipamentos gerados incluem `metadata.json` com resumos canônicos; teste de afirmação downstream
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Experimente testes de integração:
  - Torii rechaza anúncios com envelopes de admissão faltantes ou expirados.
  - El CLI faz ida e volta de proposta → envelope → verificação.
  - A renovação da governança rotaciona o atestado de endpoint sem alterar o ID do provedor.
- Requisitos de telemetria:
  - Emitir contadores `provider_admission_envelope_{accepted,rejected}` e Torii. ✅ `torii_sorafs_admission_total{result,reason}` agora exponha resultados aceitos/rejeitados.
  - Adicionar alertas de expiração a painéis de observação (renovação obrigatória em 7 dias).

## Próximos passos

1. ✅ Finalizamos as modificações do esquema Norito e incorporamos os auxiliares de validação em
   `sorafs_manifest::provider_admission`. Não são necessários sinalizadores de recursos.
2. ✅ Os fluxos CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) estão documentados e executados por meio de testes de integração; mantenha os scripts de governança sincronizados com o runbook.
3. ✅ Torii admissão/descoberta registra os envelopes e expõe contadores de telemetria para aceitação/rechazo.
4. Foco na observância: encerrar painéis/alertas de admissão para que as reformas devidas dentro de sete dias disparem avisos (`torii_sorafs_admission_total`, medidores de validade).