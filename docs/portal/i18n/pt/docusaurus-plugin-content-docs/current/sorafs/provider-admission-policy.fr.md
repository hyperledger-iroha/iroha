---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Adaptado de [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Política de admissão e identidade de provedores SoraFS (Brouillon SF-2b)

Esta nota captura os recursos acionáveis ​​para **SF-2b**: definir e
aplique o fluxo de trabalho de admissão, as exigências de identidade e as cargas úteis
atestado para fornecedores de armazenamento SoraFS. Elle étend le processus
alto nível de crédito na arquitetura RFC SoraFS e redução do trabalho
restant en tâches d'ingénierie traçables.

## Objetivos da política

- Garantir que seus operadores verificados possam publicar registros
  `ProviderAdvertV1` aceito pela rede.
- Lier chaque clé d'annonce a un document d'identité aprovado pelo governo,
  os endpoints são atestados e uma contribuição mínima de participação.
- Faça uma verificação de verificação determinada por Torii, os gateways
  e `sorafs-node` aplica os mesmos controles.
- Apoie o renovo e a revogação de urgência sem cassete
  determinação e ergonomia das ferramentas.

## Exigências de identidade e participação

| Exigência | Descrição | Livrável |
|----------|-------------|----------|
| Proveniência da clé d'annonce | Os fornecedores devem registrar um par de clés Ed25519 que assinam cada anúncio. Le bundle d'admission stocke la clé publique com uma assinatura de governo. | Crie o esquema `ProviderAdmissionProposalV1` com `advert_key` (32 bytes) e a referência a partir do registro (`sorafs_manifest::provider_admission`). |
| Ponteiro de aposta | A admissão requer um `StakePointer` não nulo para um pool de staking ativo. | Adicione a validação em `sorafs_manifest::provider_advert::StakePointer::validate()` e remonte os erros em CLI/tests. |
| Etiquetas de jurisdição | Os provedores declaram a jurisdição + o contato legal. | Crie o esquema de proposta com `jurisdiction_code` (ISO 3166-1 alfa-2) e a opção `contact_uri`. |
| Atestado de endpoint | Cada endpoint anunciado deve ser fornecido por um relatório de certificado mTLS ou QUIC. | Defina a carga útil Norito `EndpointAttestationV1` e o armazenamento por endpoint no pacote de admissão. |

## Fluxo de trabalho de admissão1. **Criação de proposta**
   - CLI: acrescentador `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     produtor `ProviderAdmissionProposalV1` + pacote de atestado.
   - Validação: s'assurer des champs requis, aposta > 0, identificador chunker canonique em `profile_id`.
2. **Endossamento de governo**
   - Le aconselhas signe `blake3("sorafs-provider-admission-v1" || canonical_bytes)` via l'outillage
     d'envelope existente (módulo `sorafs_manifest::governance`).
   - O envelope é mantido em `governance/providers/<provider_id>/admission.json`.
3. **Ingestão do registro**
   - Implementar um verificador de parte (`sorafs_manifest::provider_admission::validate_envelope`)
     reutilizado por Torii/gateways/CLI.
   - Encontre no dia o caminho de admissão Torii para rejeitar os anúncios, não o resumo ou a expiração
     diferente do envelope.
4. **Renovação e revogação**
   - Adicionado `ProviderAdmissionRenewalV1` com algumas opções de endpoint/stake.
   - Exponha um caminho CLI `--revoke` que registra o motivo da revogação e permite um evento de governo.

## Passos de implementação

| Domínio | Tache | Proprietário(s) | Estatuto |
|--------|------|----------|--------|
| Esquema | Defina `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) sob `crates/sorafs_manifest/src/provider_admission.rs`. Implementado em `sorafs_manifest::provider_admission` com ajudantes de validação.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Armazenamento / Governança | ✅ Terminé |
| CLI de saída | Use `sorafs_manifest_stub` com os subcomandos: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | GT Ferramentaria | ✅ |

O fluxo CLI aceita pacotes de certificados intermediários desordenados (`--endpoint-attestation-intermediate`), emite bytes canônicos de proposta/envelope e valida as assinaturas do conselho pendente `sign`/`verify`. Os operadores podem fornecer o corpo de anúncios diretamente, ou usar anúncios assinados, e os arquivos de assinatura podem ser fornecidos em combinação com `--council-signature-public-key` com `--council-signature-file` para facilitar a automação.

### Referência CLI

Execute cada comando via `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - Requisitos de sinalizadores: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, e pelo menos um `--endpoint=<kind:host>`.
  - O atestado por endpoint atende `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, um certificado via
    `--endpoint-attestation-leaf=<path>` (mais `--endpoint-attestation-intermediate=<path>`
    opção para cada elemento de corrente) e todo ID ALPN negociado
    (`--endpoint-attestation-alpn=<token>`). Os endpoints QUIC podem fornecer relatórios de transporte via
    `--endpoint-attestation-report[-hex]=...`.
  - Sortie: bytes canônicos de proposição Norito (`--proposal-out`) e um currículo JSON
    (padrão padrão ou `--json-out`).
-`sign`
  - Entradas: uma proposta (`--proposal`), um anúncio assinado (`--advert`), um corpo de anúncio opcional
    (`--advert-body`), época de retenção e com menos assinatura de conselho. As assinaturas podem ser
    fournies inline (`--council-signature=<signer_hex:signature_hex>`) ou através de arquivos combinados
    `--council-signature-public-key` com `--council-signature-file=<path>`.
  - Produziu um envelope válido (`--envelope-out`) e um relatório JSON indicando as ligações de resumo,
    o nome dos signatários e os caminhos de entrada.
-`verify`
  - Valide um envelope existente (`--envelope`), com opção de verificação da proposta,
    de l'advert ou du corps d'advert correspondente. O relacionamento JSON encontrado antes dos valores de resumo,
    o estado de verificação da assinatura e os artefatos opcionais correspondentes.
-`renewal`
  - Lie un nouvel envelope approuvé au digest précédemment ratifié. Solicitante
    `--previous-envelope=<path>` e o sucessor `--envelope=<path>` (duas cargas úteis Norito).
    A CLI verifica se os aliases de perfil, as capacidades e as chaves de anúncio permanecem alteradas,
    Você pode autorizar erros diários de jogo, endpoints e metadados. Definiu bytes canônicos
    `ProviderAdmissionRenewalV1` (`--renewal-out`) também como um currículo JSON.
-`revoke`
  - Crie um pacote de emergência `ProviderAdmissionRevocationV1` para um provedor que não faça o envelope
    seja aposentado. Requer `--envelope=<path>`, `--reason=<text>`, pelo menos um
    `--council-signature`, e a opção `--revoked-at`/`--notes`. A CLI assina e valida
    resumo da revogação, escreva a carga útil Norito via `--revocation-out` e imprima um relatório JSON
    com o resumo e o nome das assinaturas.
| Verificação | Implemente um verificador compartilhado usado por Torii, gateways e `sorafs-node`. Fornece testes unitários + integração CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Rede TL / Armazenamento | ✅ Terminé || Integração Torii | Injete o verificador na ingestão de anúncios Torii, rejeite os anúncios fora da política e emita o telefone. | Rede TL | ✅ Terminé | Torii carrega os envelopes de administração (`torii.sorafs.admission_envelopes_dir`), verifica o resumo/assinatura das correspondências durante a ingestão e expõe a telefonia de admissão.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Renovação | Adicione o esquema de renovação/revogação + CLI de ajudantes, publique um guia de ciclo de vida nos documentos (veja o runbook ci-dessous e os comandos CLI `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Armazenamento / Governança | ✅ Terminé |
| Telemetria | Definir painéis/alertas `provider_admission` (renovação de quantidade, expiração de envelope). | Observabilidade | 🟠 Durante o curso | O computador `torii_sorafs_admission_total{result,reason}` existe; dashboards/alertes em atenção.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook de renovação e revogação

#### Programa de renovação (mises à jour de stake/topologia)
1. Construa a dupla proposta/anúncio sucessor com `provider-admission proposal` e `provider-admission sign`, e aumente `--retention-epoch` e uma meta atual/endpoints, se necessário.
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
   O comando valida os campos de capacidade/perfil aumentados via
   `AdmissionRecord::apply_renewal`, émet `ProviderAdmissionRenewalV1`, e imprima os resumos para
   le journal de gouvernance.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Substitua o envelope anterior em `torii.sorafs.admission_envelopes_dir`, comprometa o Norito/JSON de renovação no depósito de governo e adicione o hash de renovação + época de retenção para `docs/source/sorafs/migration_ledger.md`.
4. Notifique os operadores de que o novo envelope está ativo e observe `torii_sorafs_admission_total{result="accepted",reason="stored"}` para confirmar a ingestão.
5. Regenere e comprometa os fixtures canônicos via `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` ; CI (`ci/check_sorafs_fixtures.sh`) é válido para as saídas Norito restantes estáveis.

#### Revogação de urgência
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
   Le CLI signe `ProviderAdmissionRevocationV1`, verifique o conjunto de assinaturas via
   `verify_revocation_signatures`, e relata o resumo da revogação.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Suprima o envelope de `torii.sorafs.admission_envelopes_dir`, distribua o Norito/JSON de revogação nos caches de admissão e registre o hash do motivo nos minutos de governança.
3. Verifique `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` para confirmar que os caches abandonaram o anúncio revogado; conserve os artefatos de revogação nas retrospectivas de incidentes.

## Testes e telemetria- Adicionar luminárias douradas para propostas e envelopes de admissão sob
  `fixtures/sorafs_manifest/provider_admission/`.
- Abra o CI (`ci/check_sorafs_fixtures.sh`) para registrar propostas e verificar envelopes.
- Os equipamentos genéricos incluem `metadata.json` com resumos canônicos; os testes downstream são válidos
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Fornecer testes de integração:
  - Torii rejeita os anúncios com envelopes de admissão vencidos ou vencidos.
  - Le CLI faz uma proposta de retorno → envelope → verificação.
  - A renovação do governo faz girar o atestado do ponto de extremidade sem alterar o ID do provedor.
- Exigências de télémétrie:
  - Insira os computadores `provider_admission_envelope_{accepted,rejected}` em Torii. ✅ `torii_sorafs_admission_total{result,reason}` expõe os resultados aceitos/rejeitados desordenados.
  - Adicionar alertas de expiração nos painéis de observação (renovados em 7 dias).

## Prochaines étapes

1. ✅ Finalize as modificações do esquema Norito e integre os auxiliares de validação em
   `sorafs_manifest::provider_admission`. Requisito de sinalização de recurso Aucun.
2. ✅ Os fluxos de trabalho CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) são documentados e realizados por meio de testes de integração; gardez os scripts de governança sincronizados com o runbook.
3. ✅ A admissão/descoberta Torii ingere os envelopes e expõe os compteurs de télémétrie d'acceptation/rejet.
4. Foco na observação: encerra os painéis/alertas de admissão para que os avisos sejam atualizados durante os dias de setembro após o fim dos avisos (`torii_sorafs_admission_total`, medidores de validade).