---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Adaptado de [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Política de admisión e identidad de proveedores SoraFS (Rascunho SF-2b)

Esta nota captura os entregaveis acionaveis para **SF-2b**: definir e
aplicar el flujo de admisión, los requisitos de identidad y las cargas útiles de
atestacao para proveedores de armazenamento SoraFS. Ela amplia o proceso de alto
nivel descrito en RFC de Arquitetura de SoraFS y divide el trabajo restante en
tarefas de engenharia rastreaveis.

## Objetivos de la política

- Garantir que apenas operadores verificados possam publicar registros `ProviderAdvertV1` que a rede aceita.
- Vincular cada chave de anuncio a un documento de identidad aprobado por la gobernancia, puntos finales atestiguados y contribuciones mínimas de participación.
- Fornecer ferramentas de verificación determinística para que Torii, gateways e `sorafs-node` apliquen como mesmas verificacoes.
- Apoyar la renovación y revocación de emergencia sin quebrar el determinismo o la ergonomía de las herramientas.

## Requisitos de identidad y participación| Requisito | Descripción | Entregavel |
|-----------|-----------|------------|
| Proveniencia de la chave de anuncio | Os provedores devem registrador um par de chaves Ed25519 que assina cada anuncio. O Bundle de Admissao Armazena a Chave Publica junto con una assinatura de Gobernanza. | Estender o esquema `ProviderAdmissionProposalV1` com `advert_key` (32 bytes) y referencia-lo no registro (`sorafs_manifest::provider_admission`). |
| Puenteiro de estaca | La solicitud de admisión es `StakePointer` y no hay respuesta para un grupo de apuestas activo. | Agregar validación en `sorafs_manifest::provider_advert::StakePointer::validate()` y exportar errores en CLI/tests. |
| Etiquetas jurídicas | Os provedores declaram jurisdicao + contacto legal. | Estender o esquema de propuesta con `jurisdiction_code` (ISO 3166-1 alpha-2) e `contact_uri` opcional. |
| Atestacao de endpoint | Cada punto final anunciado debe ser respaldado por un informe de certificado mTLS o QUIC. | Definir la carga útil Norito `EndpointAttestationV1` y armazena-lo por endpoint dentro del paquete de admisión. |

## Flujo de admisión1. **Criaçao da proposta**
   - CLI: agregar `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     produzindo `ProviderAdmissionProposalV1` + paquete de atestacao.
   - Validacao: garantizar campos requeridos, participación > 0, manejar canonico de fragmentador en `profile_id`.
2. **Endosso de gobierno**
   - O conselho assina `blake3("sorafs-provider-admission-v1" || canonical_bytes)` usando o herramientas de sobre existente
     (módulo `sorafs_manifest::governance`).
   - O sobre e persistido en `governance/providers/<provider_id>/admission.json`.
3. **Ingesta no registro**
   - Implementar un verificador compartido (`sorafs_manifest::provider_admission::validate_envelope`)
     que Torii/gateways/CLI se reutiliza.
   - Actualizar el camino de admisión de Torii para rejeitar anuncios cujo digest o expiracao difere do sobre.
4. **Renovación y revocación**
   - Agregar `ProviderAdmissionRenewalV1` con opciones actualizadas de endpoint/stake.
   - Exporte un camino CLI `--revoke` que registre o motivo da revogacao e envíe un evento de gobierno.

## Tarefas de implementación

| Área | Tarefa | Propietario(s) | Estado |
|------|--------|----------|--------|
| Esquema | Definir `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) en `crates/sorafs_manifest/src/provider_admission.rs`. Implementado en `sorafs_manifest::provider_admission` con ayudantes de validación.[F:crates/sorafs_manifest/src/provider_admission.rs#L1] | Almacenamiento / Gobernanza | Concluido |
| Ferramentas CLI | Estender `sorafs_manifest_stub` con subcomandos: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Grupo de Trabajo sobre Herramientas | Concluido |El flujo de CLI ahora aceita paquetes de certificados intermediarios (`--endpoint-attestation-intermediate`), emite
bytes canónicos de propuesta/sobre y valida assinaturas do conselho durante `sign`/`verify`. Operadores podem
Fornecer corpos de advert directamente o reutilizar adverts assinados, e arquivos de assinatura podem ser
fornecidos ao combine `--council-signature-public-key` com `--council-signature-file` para facilitar un automacao.

### Referencia de CLI

Ejecute cada comando a través de `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.- `proposal`
  - Banderas requeridas: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, y pelo menos um `--endpoint=<kind:host>`.
  - A atestacao por endpoint espera `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, un certificado vía
    `--endpoint-attestation-leaf=<path>` (más `--endpoint-attestation-intermediate=<path>`
    opcional para cada elemento de la cadeia) y quaisquer ID ALPN negociados
    (`--endpoint-attestation-alpn=<token>`). Endpoints QUIC podem fornecer relatorios de transporte com
    `--endpoint-attestation-report[-hex]=...`.
  - Dicho: bytes canónicos de propuesta Norito (`--proposal-out`) y un resumen JSON
    (estándar o `--json-out`).
- `sign`
  - Entradas: una propuesta (`--proposal`), un anuncio assinado (`--advert`), corpo de anuncio opcional
    (`--advert-body`), epoca de retencao e pelo menos uma assinatura do conselho. Como assinaturas podem
    Ser fornecidas inline (`--council-signature=<signer_hex:signature_hex>`) o vía archivos para combinar.
    `--council-signature-public-key` con `--council-signature-file=<path>`.
  - Produzca un sobre validado (`--envelope-out`) y un relato JSON indicando vinculacoes de digest,
    contagio de asesinos y caminos de entrada.
- `verify`
  - Valida un sobre existente (`--envelope`), con verificación opcional de propuesta, anuncio o
    corpo de advert corresponsal. El informe JSON destaca los valores de resumen y el estado de verificación.
    de assinaturas e quais artefatos opcionais correspondam.
- `renewal`- Vincula um sobre recem aprobado ao digest previamente ratificado. Solicitar
    `--previous-envelope=<path>` y el sucesor `--envelope=<path>` (ambas cargas útiles Norito).
    O CLI verifica que alias de perfil, capacidades y chaves de anuncio permanecen inalterados,
    Mientras tanto, permite actualizar la participación, los puntos finales y los metadatos. Emitir bytes canónicos
    `ProviderAdmissionRenewalV1` (`--renewal-out`) más un resumen JSON.
- `revoke`
  - Emite un paquete de emergencia `ProviderAdmissionRevocationV1` para un proveedor cujo sobre deve
    ser retirado. Solicite `--envelope=<path>`, `--reason=<text>`, pelo menos uma `--council-signature`,
    y `--revoked-at`/`--notes` opcionales. La CLI assina y valida el resumen de revogacao, elimina la carga útil
    Norito vía `--revocation-out` e imprime un resumen JSON con el resumen y el número de assinaturas.
| Verificación | Implementar verificador compartilhado usado por Torii, gateways e `sorafs-node`. Prover testes unitarios + de integracao de CLI.[F:crates/sorafs_manifest/src/provider_admission.rs#L1][F:crates/iroha_torii/src/sorafs/admission.rs#L1] | Redes TL / Almacenamiento | Concluido || Integracao Torii | Passar o verificador na ingestao de adverts no Torii, rejeitar adverts fora de politica y emitir telemetria. | Redes TL | Concluido | Torii agora carrega sobres de gobierno (`torii.sorafs.admission_envelopes_dir`), verifica correspondencias de digest/assinatura durante a ingestao e expoe telemetria de admissao.[F:crates/iroha_torii/src/sorafs/admission.rs#L1][F:crates/iroha_torii/src/sorafs/discovery.rs#L1][F:crates/iroha_torii/src/sorafs/api.rs#L1] |
| Renovação | Agregar esquema de renovacao/revogacao + helpers de CLI, publicar guía de ciclo de vida en los documentos (ver o runbook abajo y comandos CLI en `provider-admission renewal`/`revoke`).[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][docs/source/sorafs/provider_admission_policy.md:120] | Almacenamiento / Gobernanza | Concluido |
| Telemetria | Definir tableros/alertas `provider_admission` (renovación ausente, expiración de sobre). | Observabilidad | En progreso | El contador `torii_sorafs_admission_total{result,reason}` existe; paneles/alertas pendientes.[F:crates/iroha_telemetry/src/metrics.rs#L3798][F:docs/source/telemetry.md#L614] |

### Runbook de renovación y revogacao#### Renovacao agendada (actualizaciones de estaca/topología)
1. Construa o par proposta/advert sucessor com `provider-admission proposal` e `provider-admission sign`,
   aumentando `--retention-epoch` y actualizando la participación/puntos finales conforme sea necesario.
2. Ejecutar
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   O comando valida campos de capacidade/perfil inalterados vía `AdmissionRecord::apply_renewal`,
   emite `ProviderAdmissionRenewalV1` e imprime resúmenes para el registro de gobierno.
3. Sustituya el sobre anterior en `torii.sorafs.admission_envelopes_dir`, confirme o Norito/JSON de renovación
   no repositorio de gobierno y adición de hash de renovación + época de retención a `docs/source/sorafs/migration_ledger.md`.
4. Notifique os operadores que o novo sobre esta activo e monitore
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` para confirmar a ingesta.
5. Regenere y confirme los accesorios canónicos a través de `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`;
   CI (`ci/check_sorafs_fixtures.sh`) valida que como dijo Norito permanecem estaveis.#### Revogaçao de emergencia
1. Identifique el sobre comprometido y emita una revogaçao:
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
   O CLI assina o `ProviderAdmissionRevocationV1`, verifica el conjunto de assinaturas vía
   `verify_revocation_signatures` y relata el resumen de revocación.[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593][F:crates/sorafs_manifest/src/provider_admission.rs#L486]
2. Elimina el sobre de `torii.sorafs.admission_envelopes_dir`, distribuye el Norito/JSON de revogacao para cachés
   de admisao e registre o hash do motivo nas atas degobernanza.
3. Observe `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` para confirmar que os
   cachés descartaram o anuncio revogado; mantenha os artefatos de revogacao nas retrospectivas de incidentes.

## Testículos y telemetría- Accesorios adicionales dorados para propuestas y sobres de admisión en
  `fixtures/sorafs_manifest/provider_admission/`.
- Estender CI (`ci/check_sorafs_fixtures.sh`) para regenerar propuestas y verificar sobres.
- Los accesorios gerados incluyen `metadata.json` com resúmenes canónicos; testículos aguas abajo afirmam
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Probador de testículos de integracao:
  - Torii rejeita anuncios con sobres de admisión ausentes o vencidos.
  - O CLI faz ida y vuelta de propuesta -> sobre -> verificacao.
  - Una renovación de la gobernanza rotatoria a la certificación de punto final sin alterar o ID del proveedor.
- Requisitos de telemetría:
  - Emitir contadores `provider_admission_envelope_{accepted,rejected}` en Torii. `torii_sorafs_admission_total{result,reason}` ahora expoe resultados aceptados/rechazados.
  - Agregar alertas de vencimiento a paneles de observabilidad (renovación de vida dentro de 7 días).

## Próximos pasos1. Como alteracoes do esquema Norito foram finalizados y os helpers de validacao foram incorporados em `sorafs_manifest::provider_admission`. Nao ha presenta banderas.
2. Os fluxos CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) están documentados y ejercitados vía testes de integracao; Mantenga los scripts de gobierno sincronizados con el runbook.
3. Torii admisión/descubrimiento ingere os sobres e expoe contadores de telemetria para aceitacao/rejeicao.
4. Foco em observabilidade: concluir Dashboards/alertas de admissao para que renovacoes devidas dentro de sete dias disparem avisos (`torii_sorafs_admission_total`, expiry calibres).