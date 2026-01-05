---
lang: es
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7236dfe86175ff89f660be4cb4dd2c90df20a05f9606d2707407465f639b1a1c
source_last_modified: "2025-12-05T06:21:36.529838+00:00"
translation_last_reviewed: 2026-01-01
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Pruebas de finalizacion de bridge

Este documento describe la superficie inicial de pruebas de finalizacion de bridge para Iroha.
El objetivo es permitir que cadenas externas o light clients verifiquen que un bloque de Iroha
esta finalizado sin computo off-chain ni relays confiables.

## Formato de prueba

`BridgeFinalityProof` (Norito/JSON) contiene:

- `height`: altura del bloque.
- `chain_id`: identificador de cadena de Iroha para evitar replay entre cadenas.
- `block_header`: `BlockHeader` canonico.
- `block_hash`: hash del header (los clientes lo recomputan para validar).
- `commit_certificate`: conjunto de validadores + firmas que finalizan el bloque.

La prueba es autocontenida; no se requieren manifests externos ni blobs opacos.
Retencion: Torii sirve pruebas de finalizacion para la ventana reciente de commit-certificate
(limitada por el cap de historial configurado; por defecto 512 entradas via
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP`). Los clientes deben
cachear o anclar pruebas si necesitan horizontes mas largos.
La tupla canonica es `(block_header, block_hash, commit_certificate)`: el hash del header debe
coincidir con el hash dentro del commit certificate, y el chain id vincula la prueba a un solo
ledger. Los servidores rechazan y registran `CommitCertificateHashMismatch` cuando el certificado
apunta a un hash de bloque distinto.

## Commitment bundle

`BridgeFinalityBundle` (Norito/JSON) extiende la prueba basica con un commitment y justificacion
explicitos:

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`: firmas del authority set sobre el payload del commitment
  (reutiliza las firmas del commit certificate).
- `block_header`, `commit_certificate`: igual que la prueba basica.

Placeholder actual: `mmr_root`/`mmr_peaks` se derivan recomputando un MMR de hash de bloque en
memoria; aun no se retornan pruebas de inclusion. Los clientes aun pueden verificar el mismo hash
via el payload de commitment hoy.

API: `GET /v1/bridge/finality/bundle/{height}` (Norito/JSON).

La verificacion es analoga a la prueba basica: recomputar `block_hash` desde el header, verificar
las firmas del commit certificate y comprobar que los campos del commitment coincidan con el
certificado y el hash del bloque. El bundle agrega el wrapper de commitment/justification para
protocolos de bridge que prefieren la separacion.

## Pasos de verificacion

1. Recompute `block_hash` desde `block_header`; rechaza si no coincide.
2. Verifica que `commit_certificate.block_hash` coincida con el `block_hash` recomputado;
   rechaza pares header/commit certificate no coincidentes.
3. Verifica que `chain_id` coincida con la cadena Iroha esperada.
4. Recompute `validator_set_hash` desde `commit_certificate.validator_set` y verifica que
   coincida con el hash/version registrados.
5. Verifica las firmas en el commit certificate contra el hash del header usando las claves
   publicas e indices de validadores referenciados; aplica quorum (`2f+1` cuando `n>3`,
   si no `n`) y rechaza indices duplicados/fuera de rango.
6. Opcionalmente vincula a un checkpoint de confianza comparando el hash del validator set
   con un valor anclado (anchor de weak-subjectivity).
7. Opcionalmente vincula a un epoch esperado para rechazar pruebas de epochs mas antiguos/nuevos
   hasta que el anchor se rote intencionalmente.

`BridgeFinalityVerifier` (en `iroha_data_model::bridge`) aplica estas comprobaciones, rechazando
chain-id/height drift, desajustes de hash/version del validator set, firmantes duplicados/fuera
de rango, firmas invalidas y epochs inesperados antes de contar quorum para que los light clients
puedan reutilizar un solo verificador.

## Verificador de referencia

`BridgeFinalityVerifier` acepta un `chain_id` esperado mas anchors opcionales de validator set y
epoch. Hace cumplir la tupla header/block-hash/commit-certificate, valida el hash/version del
validator set, verifica firmas/quorum contra el roster de validadores anunciado y rastrea la
ultima altura para rechazar pruebas viejas/saltadas. Cuando se proporcionan anchors rechaza
replays entre epochs/rosters con errores `UnexpectedEpoch`/`UnexpectedValidatorSet`; sin anchors
adopta el hash de validator set y epoch de la primera prueba antes de seguir aplicando errores
deterministas por firmas duplicadas/fuera de rango/insuficientes.

## Superficie de API

- `GET /v1/bridge/finality/{height}` - devuelve `BridgeFinalityProof` para la altura de bloque
  solicitada. La negociacion de contenido via `Accept` soporta Norito o JSON.
- `GET /v1/bridge/finality/bundle/{height}` - devuelve `BridgeFinalityBundle`
  (commitment + justification + header/certificate) para la altura solicitada.

## Notas y follow-ups

- Las pruebas actualmente se derivan de commit certificates almacenados. El historial acotado
  sigue la ventana de retencion del commit certificate; los clientes deben cachear pruebas de
  anclaje si necesitan horizontes mas largos. Las solicitudes fuera de la ventana devuelven
  `CommitCertificateNotFound(height)`; expone el error y vuelve a un checkpoint anclado.
- Una prueba reusada o falsificada con `block_hash` no coincidente (header vs. certificate) se
  rechaza con `CommitCertificateHashMismatch`; los clientes deben hacer la misma comprobacion de
  tupla antes de verificar firmas y descartar payloads no coincidentes.
- Trabajo futuro puede agregar MMR/authority-set commitment chains para reducir el tamano de las
  pruebas en historiales muy largos. El formato sigue preservando el formato previo al envolver
  el commit certificate dentro de envelopes de commitment mas ricos.
