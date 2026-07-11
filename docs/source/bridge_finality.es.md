---
lang: es
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4fba587c1baea74a2af3829c89a9aea82699ebf8837e2ed397d32e54b792ac72
source_last_modified: "2026-07-11T18:13:35+00:00"
translation_last_reviewed: 2026-07-11
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Pruebas de finalidad del bridge

Este documento define el formato de la primera versión. Transporta la evidencia
duradera exacta producida por Sumeragi v2. La envoltura tiene versión de esquema
`1`, pero el protocolo de consenso que contiene es la versión `2`. No existe
proyección, decodificador ni ruta alternativa para Sumeragi v1.

## Formato exacto

`BridgeFinalityProof` (Norito o Norito JSON) contiene exactamente:

```text
{ version, block_header, finality_artifact, validator_set_pops }
```

- `version` debe ser `1`;
- `block_header` es el `BlockHeader` canónico;
- `finality_artifact` es el `V2FinalityArtifact` exacto e inmutable que persiste
  la ruta de aplicación de Sumeragi v2;
- `validator_set_pops` contiene un PoP BLS-normal por entrada, en el orden de
  `finality_artifact.height_context.roster`.

El artefacto es la única fuente de datos de consenso. Incluye versiones de
formato y protocolo, altura, el `HeightContext` inmutable completo, el
`BlockSubject` exacto, hash del bloque, CommitQC y, solo al final de una época,
el snapshot autenticado de la época siguiente. El contexto fija chain id,
límites de época, modo, CommitQC padre, roster ordenado de `ValidatorPower`,
`DualQuorum`, compromiso Nexus/AMX, disposición DA y semilla del líder. El
sujeto liga `parent_block_hash`, `block_hash` y `payload_hash`. La prueba no
admite copias duplicadas de altura, cadena, hash, roster o certificado.

## Fuente duradera y verificación

Después de aplicar el bloque, Sumeragi v2 valida y guarda el artefacto como
sidecar inmutable de Kura. La escritura es idempotente y Kura rechaza un
artefacto conflictivo a la misma altura. La recuperación puede completar un
sidecar ausente sin volver a ejecutar el bloque. El constructor lee el bloque y
el sidecar por altura, verifica su asociación, obtiene los PoP del estado
confirmado y ejecuta el verificador canónico. Nunca reconstruye evidencia desde
estado mutable ni usa una ventana reciente de certificados.

`verify_bridge_finality_proof` exige:

1. esquema `1`, formato del artefacto `1` y protocolo Sumeragi `2`;
2. contexto, roster ponderado, quorum, padre y transición de época válidos;
3. coincidencia exacta de altura, context id, sujeto, hash repetido y CommitQC,
   siempre en fase `Commit`;
4. chain id esperado y altura/hash recalculados del header;
5. un PoP BLS-normal válido para cada miembro del roster;
6. índices de firmantes estrictamente crecientes y dentro de rango;
7. a la vez `floor(2n/3) + 1` firmantes distintos como mínimo y potencia
   firmada estrictamente mayor que dos tercios del total;
8. la firma BLS agregada sobre el preimage exacto del voto v2.

El preimage usa el dominio `iroha:sumeragi:v2:vote` y codifica con Norito
`{ protocol_version: 2, round: { context_id, height, view }, phase: Commit,
subject: { parent_block_hash, block_hash, payload_hash } }`. El índice y la
firma individual quedan fuera; la lista ordenada del CommitQC selecciona claves
y PoP. La verificación BLS/PoP es siempre obligatoria.

## Ancla de confianza y sucesores

Una prueba aislada demuestra coherencia criptográfica bajo el roster que porta,
pero no que ese roster sea canónico. Por ello `BridgeFinalityVerifier` requiere
un `HeightContextId` confiable explícito antes de la primera prueba y nunca
aprende confianza de ella. Después solo acepta la altura inmediata siguiente,
verifica el CommitQC padre con el contexto y los PoP anteriores y aplica las
reglas de transición v2. Fuera de un límite de época se conservan época,
roster, quorum y semilla; en el límite deben coincidir con el
`next_epoch_snapshot` autenticado. Se rechazan alturas antiguas, saltadas o no
enlazadas.

## Límite de confianza de SCCP

`TairaSccpMessageProofV1.finality_proof` es la codificación Norito del mismo
tipo; SCCP no posee otro transcript ni otro cálculo de quorum. El header, la
raíz SCCP y la rama Merkle autentican el mensaje. La prueba cruda solo establece
coherencia bajo su roster congelado.

La confianza proviene del `SccpSoraFinalityAnchorV1` gobernado: red Taira
exacta, protocolo `2`, hash del chain id, altura/hash del checkpoint,
`checkpoint_context_id` y hash con separación de dominio del artefacto durable.
El circuito semántico expone el hash del ancla como último indicador público.
La admisión debe autenticar el artefacto del checkpoint y verificar cada
sucesor inmediato hasta el artefacto del mensaje, o comparar los mismos
artefactos locales confiables. Una firma válida bajo un roster suministrado por
el mensaje no prueba finalidad de Taira.

## Bundle y API

`BridgeFinalityBundle` contiene la prueba exacta, un compromiso
`{ chain_id, height_context_id, block_height, block_hash, mmr_root?,
mmr_leaf_index?, mmr_peaks? }` y una lista separada de firmas históricas,
actualmente vacía. El MMR opcional ayuda a fijar una raíz; no sustituye la
finalidad ni incluye un camino de pertenencia. SCCP usa su rama Merkle tipada y
su ancla gobernada.

- `GET /v1/bridge/finality/{height}` devuelve `BridgeFinalityProof`.
- `GET /v1/bridge/finality/bundle/{height}` devuelve `BridgeFinalityBundle`.

Ambas rutas fallan de forma cerrada si falta o es inválido el bloque o el
sidecar v2 exacto. Los consumidores de la primera versión deben rechazar toda
forma o versión desconocida; no hay compatibilidad alternativa.
