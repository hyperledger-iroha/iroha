---
lang: es
direction: ltr
source: docs/source/crypto/sm_audit_vendor_landscape.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f39199767280be0fdd582301cdc3e8929497cf372a96f9f300e718f827000a7
source_last_modified: "2026-01-03T18:07:57.038484+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Panorama de proveedores de auditoría SM
% Iroha Grupo de trabajo criptográfico
% 2026-02-12

# Descripción general

El Crypto Working Group necesita un grupo permanente de revisores independientes que
comprender tanto la criptografía Rust como los estándares chinos GM/T (SM2/SM3/SM4).
Esta nota cataloga empresas con referencias relevantes y resume la auditoría.
alcance que normalmente solicitamos para que los ciclos de solicitud de propuestas (RFP) sean rápidos y
consistente.

# Empresas candidatas

## Trail of Bits (práctica de criptografía CN)

- Compromisos documentados: revisión de seguridad de 2023 de Tongsuo de Ant Group
  (distribución OpenSSL habilitada para SM) y auditorías repetidas de sistemas basados en Rust
  cadenas de bloques como Diem/Libra, Sui y Aptos.
- Fortalezas: equipo dedicado a la criptografía de Rust, tiempo constante automatizado
  herramientas de análisis, experiencia en validación de ejecución determinista y hardware
  políticas de despacho.
- Compatible con Iroha: puede ampliar la SOW de auditoría SM actual o realizarla de forma independiente.
  volver a realizar pruebas; funcionamiento cómodo con accesorios Norito y llamada al sistema IVM
  superficies.

## Grupo NCC (Servicios de criptografía APAC)

- Compromisos documentados: exámenes de código gm/T (SM) para pago regional
  proveedores de redes y HSM; revisiones anteriores de Rust para Parity Substrate, Polkadot,
  y componentes de Libra.
- Fortalezas: banco grande de APAC con informes bilingües, capacidad de combinar
  Verificaciones de procesos de estilo de cumplimiento con revisión profunda del código.
- Compatible con Iroha: ideal para evaluaciones de segunda opinión o basadas en la gobernanza
  validación junto con los hallazgos de Trail of Bits.

## Laboratorios SECBIT (Pekín)

- Compromisos documentados: mantenedores de la caja Rust de código abierto `libsm`
  utilizado por Nervos CKB y CITA; Habilitación auditada de Guomi para Nervos, Muta y
  Componentes FISCO BCOS Rust con entregables bilingües.
- Fortalezas: ingenieros que envían activamente primitivas SM en Rust, fuertes
  capacidades de prueba de propiedades, profunda familiaridad con el cumplimiento nacional
  requisitos.
- Apto para Iroha: valioso cuando necesitamos revisores que puedan proporcionar información comparativa
  vectores de prueba y orientación de implementación junto con los hallazgos.

## Seguridad de SlowMist (Chengdu)

- Compromisos documentados: revisiones de seguridad de Substrate/Polkadot Rust, incluidas
  Horquillas Guomi para operadores chinos; evaluaciones de rutina de la billetera SM2/SM3/SM4
  y código puente utilizado por los intercambios.
- Fortalezas: práctica de auditoría centrada en blockchain, respuesta integrada a incidentes,
  orientación que abarca el código de protocolo central y las herramientas del operador.
- Compatible con Iroha: útil para validar la paridad del SDK y los puntos de contacto operativos
  además de cajas centrales.

## Chaitin Tech (Laboratorio de seguridad QAX 404)- Compromisos documentados: contribuyentes al endurecimiento de GmSSL/Tongsuo y SM2/SM3/
  Guía de implementación SM4 para instituciones financieras nacionales; establecido
  Práctica de auditoría de Rust que cubre pilas TLS y bibliotecas criptográficas.
- Fortalezas: experiencia profunda en criptoanálisis, capacidad para emparejar verificación formal
  artefactos con revisión manual, relaciones reguladoras de larga data.
- Apto para Iroha: adecuado cuando se aprueban regulaciones o artefactos de prueba formales
  debe acompañar al informe de revisión del código estándar.

# Alcance y entregables típicos de la auditoría

- **Conformidad de especificaciones:** validar cálculo y firma de SM2 ZA
  canonicalización, relleno/compresión SM3 y programación de claves SM4 y manejo IV
  contra GM/T 0003-2012, GM/T 0004-2012 y GM/T 0002-2012.
- **Determinismo y comportamiento en tiempo constante:** examinar ramificaciones, búsqueda
  mesas y puertas con hardware (p. ej., instrucciones NEON, SM4) para garantizar
  El envío de Rust y FFI sigue siendo determinista en todo el hardware compatible.
- **Integración de FFI y proveedores:** revisar los enlaces de OpenSSL/Tongsuo,
  Adaptadores PKCS#11/HSM y rutas de propagación de errores para la seguridad del consenso.
- **Cobertura de pruebas y accesorios:** evaluar arneses fuzz, viajes de ida y vuelta Norito,
  pruebas de humo deterministas y recomendar pruebas diferenciales donde existan brechas
  aparecer.
- **Revisión de dependencia y cadena de suministro:** confirmar procedencia de la construcción, proveedor
  políticas de parches, precisión de SBOM e instrucciones de compilación reproducibles.
- **Documentación y operaciones:** validar runbooks de operadores, cumplimiento
  resúmenes, valores de configuración predeterminados y procedimientos de reversión.
- **Expectativas de reporte:** resumen ejecutivo con calificación de riesgo, detallado
  hallazgos con referencias de códigos y orientación de corrección, plan de repetición de pruebas y
  certificaciones que cubren garantías de determinismo.

# Próximos pasos

- Utilice esta lista de proveedores durante los ciclos de RFQ; ajuste la lista de verificación del alcance anterior para
  coincidir con el hito de SM activo antes de emitir una RFP.
- Registrar los resultados del compromiso en `docs/source/crypto/sm_audit_brief.md` y
  actualizaciones de estado de superficie en `status.md` una vez que se ejecutan los contratos.