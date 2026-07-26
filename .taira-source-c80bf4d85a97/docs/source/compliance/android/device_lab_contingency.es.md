---
lang: es
direction: ltr
source: docs/source/compliance/android/device_lab_contingency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4016b82d86dc61a9de5e345950d02aeadf26db4cc26777c60db336c57479ba15
source_last_modified: "2026-01-03T18:07:59.250950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Registro de contingencias del laboratorio de dispositivos

Registre aquí cada activación del plan de contingencia del laboratorio de dispositivos Android.
Incluya suficientes detalles para revisiones de cumplimiento y futuras auditorías de preparación.

| Fecha | Gatillo | Acciones tomadas | Seguimientos | Propietario |
|------|---------|---------------|------------|-------|
| 2026-02-11 | La capacidad cayó al 78 % después de la interrupción del carril del Pixel8 Pro y el retraso en la entrega del Pixel8a (consulte `android_strongbox_device_matrix.md`). | Se promovió el carril Pixel7 al objetivo principal de CI, se tomó prestada la flota compartida de Pixel6, se programaron pruebas de humo del Firebase Test Lab para muestras de billeteras minoristas y se involucró el laboratorio StrongBox externo según el plan AND6. | Reemplace el concentrador USB-C defectuoso para Pixel8 Pro (fecha prevista para el 15 de febrero de 2026); Confirme la llegada de Pixel8a y el informe de capacidad de referencia. | Líder de laboratorio de hardware |
| 2026-02-13 | Se reemplazó el concentrador Pixel8 Pro y se aprobó el GalaxyS24, lo que restableció la capacidad al 85 %. | Se devolvió el carril Pixel7 al trabajo Buildkite `android-strongbox-attestation` secundario, se volvió a habilitar con las etiquetas `pixel8pro-strongbox-a` e `s24-strongbox-a`, se actualizó la matriz de preparación y el registro de evidencia. | Monitorear la ETA de entrega de Pixel8a (aún pendiente); Mantenga documentado el inventario de bujes de repuesto. | Líder de laboratorio de hardware |