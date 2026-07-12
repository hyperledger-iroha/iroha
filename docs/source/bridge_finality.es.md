---
lang: es
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e28e5c38283ad6be40a0fc48e0312797f490542a143f4cefdd209aaf8099ac5
source_last_modified: "2026-07-11T20:38:35.470900+00:00"
translation_last_reviewed: 2026-07-12
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

`BridgeFinalityProof` (Norito o Norito JSON) contiene exactamente tres campos:

```text
{ version, block_header, finality_artifact }
```

- `version` debe ser `1`;
- `block_header` es el `BlockHeader` canónico;
- `finality_artifact` es el `V2FinalityArtifact` exacto e inmutable que persiste
  la ruta de aplicación de Sumeragi v2; incluye de forma duradera un PoP
  BLS-normal por entrada, en el orden de su roster.

El artefacto es la única fuente de datos de consenso. Incluye versiones de
formato y protocolo, altura, el `HeightContext` inmutable completo, el
`BlockSubject` exacto, hash del bloque, CommitQC y los PoP alineados al roster.
El contexto fija chain id,
límites de época, modo, CommitQC padre, roster ordenado de `ValidatorPower`,
`DualQuorum`, compromiso Nexus/AMX, disposición DA y semilla del líder. El
contexto del padre que termina una época también incluye el
`next_epoch_snapshot` opcional; como forma parte del context id, el CommitQC
del padre lo autentica antes de que pueda autorizar el roster hijo. El snapshot
finalizado también vincula `epoch_end_height` y los `validator_set_pops`
alineados del roster siguiente, además de sus parámetros. El sujeto liga
`parent_block_hash`, `block_hash` y `payload_hash`. La prueba no
admite copias duplicadas de altura, cadena, hash, roster o certificado.

## Fuente duradera y verificación

Después de aplicar el bloque, Sumeragi v2 valida y guarda el artefacto como
sidecar inmutable de Kura. La escritura es idempotente y Kura rechaza un
artefacto conflictivo a la misma altura. La recuperación puede completar un
sidecar ausente sin volver a ejecutar el bloque. El constructor lee el bloque y
el sidecar por altura, verifica su asociación y ejecuta el verificador
canónico. Los PoP históricos se leen del sidecar; nunca se sustituyen por el
estado mundial mutable ni se usa una ventana reciente de certificados.

`verify_bridge_finality_proof` exige:

1. esquema `1`, formato del artefacto `1` y protocolo Sumeragi `2`;
2. contexto, roster ponderado, quorum, padre y transición de época válidos;
3. coincidencia exacta de altura, context id, sujeto, hash repetido y CommitQC,
   siempre en fase `Commit`;
4. chain id esperado y altura, hash, predecesor y view recalculados del header,
   todos vinculados exactamente al artefacto;
5. un PoP BLS-normal duradero y válido en el artefacto para cada miembro del roster;
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
reglas de transición v2. Dentro de una época, el hijo copia los PoP alineados
del artefacto anterior; en el límite, época, roster, quorum, semilla y PoP deben
coincidir con el `next_epoch_snapshot` del contexto padre, incluido su
`epoch_end_height`, todo autenticado por el CommitQC padre. Se rechazan alturas
antiguas, saltadas o no enlazadas.

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

`BridgeFinalityBundle` contiene exactamente `{ commitment, finality_proof }`.
El compromiso es exactamente
`{ chain_id, height_context_id, block_height, block_hash }`. SCCP usa su rama
Merkle tipada y su ancla gobernada.

- `GET /v1/bridge/finality/{height}` devuelve `BridgeFinalityProof`.
- `GET /v1/bridge/finality/bundle/{height}` devuelve `BridgeFinalityBundle`.

Ambas rutas fallan de forma cerrada si falta o es inválido el bloque o el
sidecar v2 exacto. Los consumidores de la primera versión deben rechazar toda
forma o versión desconocida; no hay compatibilidad alternativa.
