---
lang: es
direction: ltr
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2026-01-03T18:07:57.726224+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Esquema de propuesta del Consejo de Agenda (MINFO-2a)

Referencia de la hoja de ruta: **MINFO-2a — Validador de formato de propuesta.**

El flujo de trabajo del Agenda Council agrupa listas negras y cambios de políticas enviados por ciudadanos
propuestas antes de que los paneles de gobernanza las revisen. Este documento define la
esquema de carga útil canónico, requisitos de evidencia y reglas de detección de duplicaciones
consumido por el nuevo validador (`cargo xtask ministry-agenda validate`) por lo que
Los proponentes pueden vincular los envíos JSON localmente antes de cargarlos en el portal.

## Descripción general de la carga útil

Las propuestas de agenda utilizan el esquema `AgendaProposalV1` Norito
(`iroha_data_model::ministry::AgendaProposalV1`). Los campos se codifican como JSON cuando
envío a través de CLI/superficies de portal.

| Campo | Tipo | Requisitos |
|-------|------|--------------|
| `version` | `1` (u16) | Debe ser igual a `AGENDA_PROPOSAL_VERSION_V1`. |
| `proposal_id` | cadena (`AC-YYYY-###`) | Identificador estable; aplicado durante la validación. |
| `submitted_at_unix_ms` | u64 | Milisegundos desde la época Unix. |
| `language` | cadena | Etiqueta BCP‑47 (`"en"`, `"ja-JP"`, etc.). |
| `action` | enumeración (`add-to-denylist`, `remove-from-denylist`, `amend-policy`) | Solicitó acción al Ministerio. |
| `summary.title` | cadena | Se recomiendan ≤256 caracteres. |
| `summary.motivation` | cadena | Por qué se requiere la acción. |
| `summary.expected_impact` | cadena | Resultados si se acepta la acción. |
| `tags[]` | cadenas minúsculas | Etiquetas de triaje opcionales. Valores permitidos: `csam`, `malware`, `fraud`, `harassment`, `impersonation`, `policy-escalation`, `terrorism`, `spam`. |
| `targets[]` | objetos | Una o más entradas de familia de hash (ver más abajo). |
| `evidence[]` | objetos | Uno o más archivos adjuntos de evidencia (ver más abajo). |
| `submitter.name` | cadena | Nombre para mostrar u organización. |
| `submitter.contact` | cadena | Correo electrónico, dirección de Matrix o teléfono; redactado de paneles públicos. |
| `submitter.organization` | cadena (opcional) | Visible en la interfaz de usuario del revisor. |
| `submitter.pgp_fingerprint` | cadena (opcional) | Huella digital en mayúsculas de 40 hex. |
| `duplicates[]` | cuerdas | Referencias opcionales a ID de propuestas enviadas anteriormente. |

### Entradas de destino (`targets[]`)

Cada objetivo representa un resumen de la familia de hash al que hace referencia la propuesta.

| Campo | Descripción | Validación |
|-------|-------------|------------|
| `label` | Nombre descriptivo para el contexto del revisor. | No vacío. |
| `hash_family` | Identificador hash (`blake3-256`, `sha256`, etc.). | Letras/dígitos ASCII/`-_.`, ≤48 caracteres. |
| `hash_hex` | Resumen codificado en minúsculas hexadecimales. | ≥16 bytes (32 caracteres hexadecimales) y debe ser hexadecimal válido. |
| `reason` | Breve descripción de por qué se debe ejecutar el resumen. | No vacío. |

El validador rechaza pares `hash_family:hash_hex` duplicados dentro del mismo
propuesta e informes entran en conflicto cuando la misma huella digital ya existe en el
registro duplicado (ver más abajo).

### Anexos de evidencia (`evidence[]`)

Documento de entradas de evidencia donde los revisores pueden obtener contexto de respaldo.| Campo | Tipo | Notas |
|-------|------|-------|
| `kind` | enumeración (`url`, `torii-case`, `sorafs-cid`, `attachment`) | Determina los requisitos de digestión. |
| `uri` | cadena | URL HTTP(S), ID de caso Torii o URI SoraFS. |
| `digest_blake3_hex` | cadena | Requerido para los tipos `sorafs-cid` e `attachment`; opcional para otros. |
| `description` | cadena | Texto de formato libre opcional para revisores. |

### Registro duplicado

Los operadores pueden mantener un registro de huellas dactilares existentes para evitar duplicaciones
casos. El validador acepta un archivo JSON con la forma:

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

Cuando un objetivo de propuesta coincide con una entrada, el validador cancela a menos que
Se especifica `--allow-registry-conflicts` (aún se emiten advertencias).
Utilice [`cargo xtask ministry-agenda impact`](impact_assessment_tooling.md) para
generar el resumen listo para el referéndum que haga referencia cruzada al duplicado
instantáneas de registro y políticas.

## Uso de CLI

Lint una sola propuesta y compárala con un registro duplicado:

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

Pase `--allow-registry-conflicts` para reducir las visitas duplicadas a advertencias cuando
realizar auditorías históricas.

La CLI se basa en el mismo esquema Norito y en los ayudantes de validación incluidos
`iroha_data_model`, para que los SDK/portales puedan reutilizar el `AgendaProposalV1::validate`
método para un comportamiento consistente.

## CLI de clasificación (MINFO-2b)

Referencia de la hoja de ruta: **MINFO-2b: Registro de auditoría y clasificación de múltiples ranuras.**

La lista del Consejo de Agenda ahora se gestiona mediante clasificación determinista para que los ciudadanos
puede auditar de forma independiente cada sorteo. Utilice el nuevo comando:

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster`: archivo JSON que describe a cada miembro elegible:

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  El archivo de ejemplo se encuentra en
  `docs/examples/ministry/agenda_council_roster.json`. Campos opcionales (rol,
  organización, contacto, metadatos) se capturan en la hoja Merkle para que los auditores
  Puede probar la plantilla que alimentó el sorteo.

- `--slots`: número de escaños del consejo a cubrir.
- `--seed`: semilla BLAKE3 de 32 bytes (64 caracteres hexadecimales en minúscula) registrada en el
  Acta de gobierno del sorteo.
- `--out`: ruta de salida opcional. Cuando se omite, el resumen JSON se imprime en
  salida estándar.

### Resumen de salida

El comando emite un blob JSON `SortitionSummary`. La salida de muestra se almacena en
`docs/examples/ministry/agenda_sortition_summary_example.json`. Campos clave:

| Campo | Descripción |
|-------|-------------|
| `algorithm` | Etiqueta de clasificación (`agenda-sortition-blake3-v1`). |
| `roster_digest` | BLAKE3 + SHA-256 resúmenes del archivo de lista (utilizado para confirmar que las auditorías operan sobre la misma lista de miembros). |
| `seed_hex` / `slots` | Haga eco de las entradas de la CLI para que los auditores puedan reproducir el sorteo. |
| `merkle_root_hex` | Raíz del árbol Merkle de la lista (ayudantes `hash_node`/`hash_leaf` en `xtask/src/ministry_agenda.rs`). |
| `selected[]` | Entradas para cada espacio, incluidos los metadatos de los miembros canónicos, el índice elegible, el índice de la lista original, la entropía de dibujo determinista, el hash de hoja y los hermanos a prueba de Merkle. |

### Verificando un sorteo1. Obtenga la lista a la que hace referencia `roster_path` y verifique su BLAKE3/SHA-256
   los resúmenes coinciden con el resumen.
2. Vuelva a ejecutar la CLI con la misma semilla/ranuras/lista; el `selected[].member_id` resultante
   El orden debe coincidir con el resumen publicado.
3. Para un miembro específico, calcule la hoja de Merkle utilizando el JSON del miembro serializado.
   (`norito::json::to_vec(&sortition_member)`) y doble cada hash de prueba. la final
   El resumen debe ser igual a `merkle_root_hex`. El asistente en el resumen del ejemplo muestra
   cómo combinar `eligible_index`, `leaf_hash_hex` e `merkle_proof[]`.

Estos artefactos satisfacen el requisito MINFO-2b de aleatoriedad verificable,
selección de k-of-m y registros de auditoría de solo agregar hasta que la API en cadena esté conectada.

## Referencia de error de validación

`AgendaProposalV1::validate` emite variantes `AgendaProposalValidationError`
cada vez que una carga útil falla. La siguiente tabla resume los más comunes
errores para que los revisores del portal puedan traducir los resultados de la CLI en una guía práctica.| Error | Significado | Remediación |
|-------|---------|-------------|
| `UnsupportedVersion { expected, found }` | La carga útil `version` difiere del esquema admitido por el validador. | Vuelva a generar el JSON utilizando el paquete de esquemas más reciente para que la versión coincida con `expected`. |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` está vacío o no en el formato `AC-YYYY-###`. | Complete un identificador único siguiendo el formato documentado antes de volver a enviarlo. |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` es cero o falta. | Registre la marca de tiempo de envío en milisegundos de Unix. |
| `InvalidLanguageTag { value }` | `language` no es una etiqueta BCP‑47 válida. | Utilice una etiqueta estándar como `en`, `ja-JP` u otra configuración regional reconocida por BCP‑47. |
| `MissingSummaryField { field }` | Uno de `summary.title`, `.motivation` o `.expected_impact` está vacío. | Proporcione texto que no esté vacío para el campo de resumen indicado. |
| `MissingSubmitterField { field }` | Falta `submitter.name` o `submitter.contact`. | Proporcione los metadatos del remitente que faltan para que los revisores puedan comunicarse con el proponente. |
| `InvalidTag { value }` | La entrada `tags[]` no está en la lista de permitidos. | Elimine o cambie el nombre de la etiqueta a uno de los valores documentados (`csam`, `malware`, etc.). |
| `MissingTargets` | La matriz `targets[]` está vacía. | Proporcione al menos una entrada de familia de hash de destino. |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` | A la entrada de destino le faltan los campos `label` o `reason`. | Complete el campo obligatorio para la entrada indexada antes de volver a enviarla. |
| `InvalidHashFamily { index, value }` | Etiqueta `hash_family` no compatible. | Restrinja los nombres de familia hash a caracteres alfanuméricos ASCII más `-_`. |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` | El resumen no es hexadecimal válido o tiene menos de 16 bytes. | Proporcione un resumen hexadecimal en minúsculas (≥32 caracteres hexadecimales) para el objetivo indexado. |
| `DuplicateTarget { index, fingerprint }` | El resumen de destino duplica una entrada anterior o una huella digital del registro. | Elimine duplicados o combine la evidencia de respaldo en un solo objetivo. |
| `MissingEvidence` | No se proporcionaron anexos de pruebas. | Adjunte al menos un registro de evidencia que enlace al material de reproducción. |
| `MissingEvidenceUri { index }` | A la entrada de evidencia le falta el campo `uri`. | Proporcione el URI recuperable o el identificador de caso para la entrada de evidencia indexada. |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` | Falta la entrada de evidencia que requiere un resumen (SoraFS CID o archivo adjunto) o tiene un `digest_blake3_hex` no válido. | Proporcione un resumen BLAKE3 en minúsculas de 64 caracteres para la entrada indexada. |

## Ejemplos

- `docs/examples/ministry/agenda_proposal_example.json` — canónico,
  Carga útil de propuesta sin pelusa con dos archivos adjuntos de evidencia.
- `docs/examples/ministry/agenda_duplicate_registry.json` — registro de inicio
  que contiene una única huella digital BLAKE3 y su justificación.

Reutilice estos archivos como plantillas al integrar herramientas del portal o escribir CI
comprobaciones de envíos automáticos.