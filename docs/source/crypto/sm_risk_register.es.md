---
lang: es
direction: ltr
source: docs/source/crypto/sm_risk_register.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ba5f4fdc9221210a793fd0c2120d8cfb68487d7ddcbe67c208976798446ca5db
source_last_modified: "2026-01-03T18:07:57.078074+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Registro de riesgos del programa SM para habilitación SM2/SM3/SM4.

# Registro de Riesgos del Programa SM

Última actualización: 2025-03-12.

Este registro amplía el resumen en `sm_program.md`, emparejando cada riesgo con
propiedad, los desencadenantes del monitoreo y el estado actual de mitigación. El Grupo de Trabajo sobre Cripto
y los líderes de la Plataforma Central revisan este registro en la cadencia SM semanal; cambios
se reflejan tanto aquí como en la hoja de ruta pública.

## Resumen de riesgos

| identificación | Riesgo | Categoría | Probabilidad | Impacto | Gravedad | Propietario | Mitigación | Estado | Desencadenantes |
|----|------|----------|-------------|--------|----------|-------|------------|--------|----------|
| R1 | Auditoría externa para cajas RustCrypto SM no ejecutada antes de que el validador firmara GA | Cadena de suministro | Medio | Alto | Alto | Grupo de Trabajo sobre Cripto | Contract Trail of Bits/NCC Group, mantenga la postura de solo verificación hasta que se acepte el informe | Mitigación en progreso | Auditoría SOW sin firmar antes del 15 de abril de 2025 o informe de auditoría retrasado después del 1 de junio de 2025 |
| R2 | Regresiones nonce deterministas entre SDK | Implementación | Medio | Alto | Alto | Líderes del programa SDK | Comparta dispositivos en SDK CI, aplique codificación r∥s canónica, agregue pruebas de manipulación entre SDK | Monitoreo | Se detectó desviación de dispositivos en la versión CI o SDK sin dispositivos SM |
| R3 | Errores específicos de ISA en intrínsecos (NEON/SIMD) | Rendimiento | Bajo | Medio | Medio | Grupo de Trabajo sobre Rendimiento | Los intrínsecos de la puerta detrás de los indicadores de funciones requieren cobertura de CI en ARM y mantienen el respaldo escalar | Mitigación en progreso | Los bancos NEON fallan o se descubre la regresión del hardware en la matriz de rendimiento SM |
| R4 | La ambigüedad en el cumplimiento retrasa la adopción de SM | Gobernanza | Medio | Medio | Medio | Documentos y enlace legal | Publicar informe de cumplimiento, lista de verificación de operadores, enlace con asesores legales antes de GA | Mitigación en progreso | Revisión legal pendiente después del 1 de mayo de 2025 o actualizaciones de la lista de verificación faltantes |
| R5 | Deriva del backend de FFI con actualizaciones de proveedores | Integración | Medio | Medio | Medio | Operaciones de plataforma | Fijar versiones de proveedores, agregar pruebas de paridad, mantener la opción de vista previa de OpenSSL/Tongsuo | Monitoreo | Actualización de paquete fusionada sin ejecución de paridad o vista previa habilitada fuera del alcance piloto |

## Revisar cadencia

- Sincronización semanal de Crypto WG (tema permanente de la agenda).
- Revisión conjunta mensual con Platform Ops y Docs para confirmar la postura de cumplimiento.
- Punto de control previo al lanzamiento: congelación del registro de riesgos y certificación incluida con GA
  artefactos.

## Aprobación

| Rol | Representante | Fecha | Notas |
|------|----------------|------|-------|
| Líder del Grupo de Trabajo sobre Cripto | (firma en el archivo) | 2025-03-12 | Aprobado para publicación y compartido con el trabajo pendiente del Grupo de Trabajo. |
| Líder de plataforma central | (firma en el archivo) | 2025-03-12 | Mitigaciones aceptadas y cadencia de seguimiento. |

Para aprobaciones históricas y actas de reuniones, consulte `docs/source/crypto/sm_program.md`
(`Communication Plan`) y el archivo de agenda SM vinculado desde Crypto WG
espacio de trabajo.