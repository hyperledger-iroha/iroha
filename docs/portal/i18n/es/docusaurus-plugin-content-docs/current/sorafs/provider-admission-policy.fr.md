---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Adaptado de [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Política de admisión e identidad de proveedores SoraFS (Brouillon SF-2b)

Esta nota captura los espacios habitables para **SF-2b**: definir y
aplicar el flujo de trabajo de admisión, las exigencias de identidad y las cargas útiles
Certificación para proveedores de almacenamiento SoraFS. Elle étend le processus
Alto nivel descrito en el RFC de arquitectura SoraFS y découpe le travail.
restant en tâches d'ingénierie traçables.

## Objetivos de la política

- Garantía que solo los operadores verificados pueden publicar registros
  `ProviderAdvertV1` aceptado por la red.
- Lier chaque clé d'annonce à un document d'identité approuvé par la gouvernance,
  Puntos finales atestiguados y una contribución mínima de participación.
- Utilice una herramienta de verificación determinada según Torii, las puertas de enlace
  y `sorafs-node` aplican los mismos controles.
- Supporter le renouvellement et la révocation d'urgence sans casser le
  déterminismo ni la ergonomía de los utensilios.

## Exigencias de identidad y de juego| Exigencia | Descripción | Habitable |
|----------|-------------|----------|
| Procedencia de la clave de anuncio | Los proveedores no deben registrar un par de claves Ed25519 que firmen cada anuncio. El paquete de admisión almacena la clave pública con una firma de gobierno. | Tenga en cuenta el esquema `ProviderAdmissionProposalV1` con `advert_key` (32 bytes) y la referencia después del registro (`sorafs_manifest::provider_admission`). |
| Puntero de estaca | La admisión requiere un `StakePointer` no nulo frente a un grupo de participación activo. | Agregue la validación en `sorafs_manifest::provider_advert::StakePointer::validate()` y elimine los errores en CLI/tests. |
| Etiquetas de jurisdicción | Les proveedores declaran la jurisdicción + el contacto legal. | Tenga en cuenta el esquema de propuesta con `jurisdiction_code` (ISO 3166-1 alfa-2) y `contact_uri` opcional. |
| Atestación de punto final | Cada punto final anunciado debe estar disponible mediante una relación de certificación mTLS o QUIC. | Defina la carga útil Norito `EndpointAttestationV1` y el almacenamiento por punto final en el paquete de admisión. |

## Flujo de trabajo de admisión1. **Creación de propuesta**
   - CLI: añadido `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     Producto `ProviderAdmissionProposalV1` + paquete de certificación.
   - Validación: s'assurer des champs requis, apuesta > 0, maneja el fragmentador canónico en `profile_id`.
2. **Aprobación de gobierno**
   - Le conseil signe `blake3("sorafs-provider-admission-v1" || canonical_bytes)` vía l'outillage
     d'sobre existente (módulo `sorafs_manifest::governance`).
   - El sobre se conserva en `governance/providers/<provider_id>/admission.json`.
3. **Ingestión del registro**
   - Implementador de un verificador de participación (`sorafs_manifest::provider_admission::validate_envelope`)
     Reutilizado por Torii/gateways/CLI.
   - Mettre à jour le chemin d'admission Torii para rechazar los anuncios que no se digieren o que caduquen
     diferente del sobre.
4. **Renovación y revocación**
   - Agregar `ProviderAdmissionRenewalV1` con las últimas opciones de endpoint/stake.
   - Expositor un chemin CLI `--revoke` que registra la razón de la revocación y pousse un evento de gobierno.

## Tareas de implementación| Dominio | tache | Propietario(s) | Estatuto |
|--------|------|----------|--------|
| Esquema | Definir `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) en `crates/sorafs_manifest/src/provider_admission.rs`. Implementado en `sorafs_manifest::provider_admission` con ayudantes de validación. 【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Almacenamiento / Gobernanza | ✅ Terminación |
| CLI de eliminación | Étendre `sorafs_manifest_stub` con los subcomandos: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Grupo de Trabajo sobre Herramientas | ✅ |

El flujo CLI acepta generar paquetes de certificados intermedios (`--endpoint-attestation-intermediate`), enviar bytes canónicos propuesta/sobre y validar las firmas del consejo colgante `sign`/`verify`. Los operadores pueden almacenar el cuerpo de publicidad directa, o reutilizar los anuncios firmados y los archivos de firma pueden almacenarse combinando `--council-signature-public-key` con `--council-signature-file` para facilitar la automatización.

### Referencia CLI

Ejecute cada comando a través de `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.- `proposal`
  - Requisitos de banderas: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, y al menos un `--endpoint=<kind:host>`.
  - La atestación por punto final atiende a `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, certificado vía
    `--endpoint-attestation-leaf=<path>` (más `--endpoint-attestation-intermediate=<path>`
    Opciones para cada elemento de cadena) y todo ID ALPN negociado.
    (`--endpoint-attestation-alpn=<token>`). Los puntos finales QUIC pueden establecer relaciones de transporte a través de
    `--endpoint-attestation-report[-hex]=...`.
  - Clasificación: bytes canónicos de la proposición Norito (`--proposal-out`) y un currículum JSON
    (salida estándar por defecto o `--json-out`).
- `sign`
  - Entradas: una propuesta (`--proposal`), un anuncio firmado (`--advert`), un cuerpo de anuncio opcional
    (`--advert-body`), época de retención y menos firma del consejo. Las firmas pueden existir
    fournies en línea (`--council-signature=<signer_hex:signature_hex>`) o mediante archivos combinados
    `--council-signature-public-key` con `--council-signature-file=<path>`.
  - Produzca un sobre válido (`--envelope-out`) y una relación JSON que indique los enlaces de resumen,
    el nombre de los signataires et les chemins d'entrée.
-`verify`
  - Validar un sobre existente (`--envelope`), con opción de verificación de la propuesta,
    de l'advert ou du corps d'advert corresponsal. La relación JSON se reunió antes de los valores de digestión,El estado de verificación de la firma y los artefactos opcionales correspondientes.
- `renewal`
  - Lie un nuevo sobre aprobado en el resumen previo ratificado. Requerir
    `--previous-envelope=<path>` y el sucesor `--envelope=<path>` (dos cargas útiles Norito).
    La CLI verifica si los alias de perfil, las capacidades y las claves de anuncio permanecen incambiados.
    Todo ello autoriza las actualizaciones diarias de los puntos finales y los metadatos. Émet les bytes canónicos
    `ProviderAdmissionRenewalV1` (`--renewal-out`) además de un currículum JSON.
- `revoke`
  - Émet un paquete de urgencia `ProviderAdmissionRevocationV1` para un proveedor no lo hace el sobre
    Estás retirado. Requiere `--envelope=<path>`, `--reason=<text>`, al menos un
    `--council-signature` y opción `--revoked-at`/`--notes`. La CLI firma y valida el archivo
    resumen de revocación, escriba la carga útil Norito a través de `--revocation-out` e imprima una relación JSON
    con el resumen y el nombre de firmas.
| Verificación | Implemente un verificador compartido utilizado por Torii, gateways e `sorafs-node`. Cuatro pruebas unitarias + integración CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Redes TL / Almacenamiento | ✅ Terminación || Integración Torii | Inyector del verificador en la ingestión de anuncios Torii, rechazo de anuncios fuera de la política y emisión de televisión. | Redes TL | ✅ Terminación | Torii carga los sobres de gobierno (`torii.sorafs.admission_envelopes_dir`), verifica las correspondencias, resumen/firma durante la ingestión y expone la télémétrie de admisión.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Renovación | Agregar el esquema de renovación/revocación + ayudantes CLI, publicar una guía de ciclo de vida en los documentos (ver runbook ci-dessous et comandos CLI `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Almacenamiento / Gobernanza | ✅ Terminación |
| Telemetría | Definir paneles/alertas `provider_admission` (renovación manquant, vencimiento del sobre). | Observabilidad | 🟠 En curso | El ordenador `torii_sorafs_admission_total{result,reason}` existe; paneles/alertas en atención.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook de renovación y revocación#### Programa de renovación (mises à jour de stake/topologie)
1. Construya la pareja de propuesta/anuncio sucesor con `provider-admission proposal` y `provider-admission sign`, y aumente `--retention-epoch` y agregue cada vez los puntos finales/estacas si es necesario.
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
   El comando valide les champs de capacité/profil inchangés via
   `AdmissionRecord::apply_renewal`, abra `ProviderAdmissionRenewalV1` e imprima los resúmenes para
   le journal de gouvernance.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Reemplace el sobre anterior en `torii.sorafs.admission_envelopes_dir`, envíe el Norito/JSON de renovación en el depósito de gobierno y agregue el hash de renovación + época de retención en `docs/source/sorafs/migration_ledger.md`.
4. Notifique a los operadores que el nuevo sobre está activo y vigilará `torii_sorafs_admission_total{result="accepted",reason="stored"}` para confirmar la ingestión.
5. Régénérez et commitez les fixtures canonices vía `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli`; CI (`ci/check_sorafs_fixtures.sh`) valide que les sorties Norito restent stables.#### Revocación de urgencia
1. Identifique el sobre comprometido y envíe una revocación:
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
   La CLI signe `ProviderAdmissionRevocationV1`, verifica el conjunto de firmas vía
   `verify_revocation_signatures`, y informe el resumen de revocación.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Suprima el sobre de `torii.sorafs.admission_envelopes_dir`, distribuya el Norito/JSON de revocación en cachés de admisión y registre el hash de la razón en las actas de gobierno.
3. Vigile `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` para confirmar que los cachés abandonan el anuncio revocado; conservez les artefactos de révocación dans les rétrospectives d'incident.

## Pruebas y télémétrie- Ajouter des iluminación dorada para las propuestas y sobres de admisión sous
  `fixtures/sorafs_manifest/provider_admission/`.
- Éndre le CI (`ci/check_sorafs_fixtures.sh`) para regenerar las propuestas y verificar los sobres.
- Los dispositivos generados incluyen `metadata.json` con los resúmenes canónicos; las pruebas posteriores son válidas
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Cuatro pruebas de integración:
  - Torii Rechace los anuncios con sobres de admisión manquants o caducados.
  - Le CLI hace una propuesta de devolución → sobre → verificación.
  - La renovación del gobierno hace pivotar la certificación del punto final sin cambiar el ID del proveedor.
- Exigencias de télémétrie:
  - Inserte los ordenadores `provider_admission_envelope_{accepted,rejected}` en Torii. ✅ `torii_sorafs_admission_total{result,reason}` expone désormais les résultats Acceptés/Rejetés.
  - Agregar alertas de vencimiento en los paneles de observación (renovación dentro de los 7 días).

## Prochaines étapes1. ✅ Finalice las modificaciones del esquema Norito e integre los ayudantes de validación en
   `sorafs_manifest::provider_admission`. Requisitos de bandera de función de Aucun.
2. ✅ Los flujos de trabajo CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) están documentados y ejercitados mediante pruebas de integración; Guarde los scripts de gobierno sincronizados con el runbook.
3. ✅ L'admission/discovery Torii ingère les sobres y expone les compteurs de télémétrie d'acceptation/rejet.
4. Enfoque de observabilidad: terminar los paneles/alertas de admisión para que las actualizaciones de los últimos días se hayan reducido las advertencias (`torii_sorafs_admission_total`, indicadores de vencimiento).