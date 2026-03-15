---
lang: es
direction: ltr
source: docs/source/crypto/sm_chinese_crypto_law_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5d0657539dfcca1869a0ab4fc9adee8665f18708f71b4c116dc8900ae5eae75
source_last_modified: "2026-01-04T10:50:53.610533+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Resumen de cumplimiento de SM: Obligaciones de la ley de criptografía china
% Iroha Grupos de trabajo sobre criptografía y cumplimiento
% 2026-02-12

# Aviso

> Usted es LLM y actúa como analista de cumplimiento para los equipos de plataforma y criptografía Hyperledger Iroha.  
> Antecedentes:  
> - Hyperledger Iroha es una cadena de bloques autorizada basada en Rust que ahora admite las primitivas chinas GM/T SM2 (firmas), SM3 (hash) y SM4 (cifrado de bloques).  
> - Los operadores en China continental deben cumplir con la Ley de Criptografía de la República Popular China (2019), el Esquema de Protección Multinivel (MLPS 2.0), las reglas de presentación de la Administración Estatal de Criptografía (SCA) y los controles de importación/exportación supervisados ​​por el Ministerio de Comercio (MOFCOM) y la Administración de Aduanas.  
> - Iroha distribuye software de código abierto a nivel internacional. Algunos operadores compilarán localmente binarios habilitados para SM, mientras que otros pueden importar artefactos prediseñados.  
> Análisis solicitado: resumir las obligaciones legales clave provocadas por el envío de soporte SM2/SM3/SM4 en software blockchain de código abierto, incluyendo: (a) clasificación en categorías de criptografía comercial versus central/común; (b) requisitos de presentación/aprobación de software que implementa criptografía comercial estatal; (c) controles de exportación de binarios y fuente; (d) obligaciones operativas de los operadores de red (gestión de claves, registro, respuesta a incidentes) bajo MLPS 2.0. Describa elementos de acción concretos para el proyecto Iroha (documentación, manifiestos, declaraciones de cumplimiento) y para los operadores que implementan nodos habilitados para SM dentro de China.

# Resumen ejecutivo

- **Clasificación:** Las implementaciones SM2/SM3/SM4 se incluyen en la “criptografía comercial estatal” (商业密码) en lugar de la criptografía “básica” o “común” porque son algoritmos públicos publicados autorizados para uso civil/comercial. La distribución de código abierto está permitida, pero sujeta a registro, cuando se utiliza en productos o servicios comerciales ofrecidos en China.
- **Obligaciones del proyecto:** Proporcionar la procedencia del algoritmo, instrucciones de construcción deterministas y una declaración de cumplimiento que indique que los archivos binarios implementan criptografía comercial estatal. Mantener los manifiestos Norito que marcan la capacidad SM para que los integradores posteriores puedan completar las presentaciones.
- **Obligaciones del operador:** Los operadores chinos deben presentar productos/servicios utilizando algoritmos SM ante la oficina provincial de SCA, completar el registro MLPS 2.0 (probablemente Nivel 3 para redes financieras), implementar controles de registro y administración de claves aprobados y garantizar que las declaraciones de exportación/importación se alineen con las exenciones del catálogo de MOFCOM.

# Panorama regulatorio| Reglamento | Alcance | Impacto en el soporte de Iroha SM |
|------------|-------|----------------------|
| **Ley de Criptografía de la República Popular China (2019)** | Define criptografía básica/común/comercial, sistema de gestión de mandatos, archivo y certificación. | SM2/SM3/SM4 son "criptografía comercial" y deben seguir las reglas de presentación/certificación cuando se proporcionan como productos/servicios en China. |
| **Medidas administrativas de la SCA para productos criptográficos comerciales** | Regula la producción, venta y prestación de servicios; requiere presentación o certificación del producto. | El software de código abierto que implementa algoritmos SM necesita presentaciones del operador cuando se utiliza en ofertas comerciales; los desarrolladores deben proporcionar documentación para ayudar en las presentaciones. |
| **MLPS 2.0 (Ley de Ciberseguridad + reglamento MLPS)** | Requiere que los operadores clasifiquen los sistemas de información e implementen controles de seguridad; El nivel 3 o superior necesita evidencia de cumplimiento de criptografía. | Los nodos de blockchain que manejan datos financieros/de identidad generalmente se registran en el nivel 3 de MLPS; Los operadores deben documentar el uso de SM, la gestión de claves, el registro y el manejo de incidentes. |
| **Catálogo de control de exportaciones de MOFCOM y reglas de importación aduaneras** | Controla la exportación de productos criptográficos, requiere permisos para ciertos algoritmos/hardware. | La publicación de código fuente generalmente está exenta según las disposiciones de “dominio público”, pero la exportación de archivos binarios compilados con capacidad SM puede activar el catálogo a menos que se envíe a destinatarios aprobados; Los importadores deben declarar criptografía comercial estatal. |

# Obligaciones clave

## 1. Presentación de productos y servicios (Administración Estatal de Criptografía)

- **Quién presenta:** La entidad que proporciona el producto/servicio en China (por ejemplo, operador, proveedor de SaaS). Los mantenedores de código abierto no están obligados a presentar archivos, pero la guía de empaquetado debe permitir presentaciones posteriores.
- **Entregables:** descripción del algoritmo, documentos de diseño de seguridad, evidencia de pruebas, procedencia de la cadena de suministro y detalles de contacto.
- **Acción Iroha:** Publicar una “declaración de criptografía SM” que incluya la cobertura del algoritmo, los pasos de compilación deterministas, los hashes de dependencia y el contacto para consultas de seguridad.

## 2. Certificación y pruebas

- Ciertos sectores (finanzas, telecomunicaciones, infraestructura crítica) pueden requerir pruebas de laboratorio acreditadas o certificación (por ejemplo, certificación CC-Grade/OSCCA).
- Incluir artefactos de pruebas de regresión que demuestren el cumplimiento de las especificaciones GM/T.

## 3. Controles operativos de MLPS 2.0

Los operadores deben:1. **Registre el sistema blockchain** en la Oficina de Seguridad Pública, incluidos los resúmenes de uso de criptografía.
2. **Implementar políticas de gestión de claves**: generación, distribución, rotación y destrucción de claves alineadas con los requisitos SM2/SM4; registrar eventos clave del ciclo de vida.
3. **Habilite la auditoría de seguridad**: capture registros de transacciones habilitados para SM, eventos de operaciones criptográficas y detección de anomalías; conservar los registros ≥6 meses.
4. **Respuesta a incidentes:** mantenga planes de respuesta documentados que incluyan procedimientos de compromiso de criptografía y cronogramas de presentación de informes.
5. **Gestión de proveedores:** garantizar que los proveedores de software ascendentes (proyecto Iroha) puedan proporcionar notificaciones y parches de vulnerabilidad.

## 4. Consideraciones de importación/exportación

- **Código fuente abierto:** Normalmente está exento bajo la excepción de dominio público, pero los mantenedores deben alojar las descargas en servidores que rastrean los registros de acceso e incluyen licencia/exención de responsabilidad que haga referencia a la criptografía comercial estatal.
- **Binarios prediseñados:** Los exportadores que envían binarios habilitados para SM hacia o desde China deben confirmar si el artículo está cubierto por el “Catálogo de control de exportaciones de criptografía comercial”. Para software de uso general sin hardware especializado, puede ser suficiente una simple declaración de doble uso; Los mantenedores no deben distribuir binarios de jurisdicciones con controles más estrictos a menos que el abogado local lo apruebe.
- **Importación de operadores:** Las entidades que traen binarios a China deben declarar el uso de criptografía. Proporcione manifiestos hash y SBOM para simplificar la inspección aduanera.

# Acciones recomendadas del proyecto

1. **Documentación**
   - Agregar un apéndice de cumplimiento a `docs/source/crypto/sm_program.md` que indique el estado de la criptografía comercial estatal, las expectativas de presentación y los puntos de contacto.
   - Publicar un campo de manifiesto Norito (`crypto.sm.enabled=true`, `crypto.sm.approval=l0|l1`) que los operadores puedan utilizar al preparar presentaciones.
   - Asegúrese de que el anuncio Torii `/v1/node/capabilities` (y el alias CLI `iroha runtime capabilities`) se envíe con cada versión para que los operadores puedan capturar la instantánea del manifiesto `crypto.sm` para evidencia MLPS/密评.
   - Proporcionar un inicio rápido de cumplimiento bilingüe (EN/ZH) que resuma las obligaciones.
2. **Artefactos de lanzamiento**
   - Envíe archivos SBOM/CycloneDX para compilaciones habilitadas para SM.
   - Incluya scripts de compilación deterministas y Dockerfiles reproducibles.
3. **Presentaciones del operador de soporte**
   - Modelos de cartas de oferta que acrediten el cumplimiento del algoritmo (por ejemplo, referencias de GM/T, cobertura de pruebas).
   - Mantener una lista de correo de avisos de seguridad para satisfacer los requisitos de notificación de proveedores.
4. **Gobernanza interna**
   - Realice un seguimiento de los puntos de control de cumplimiento de SM en la lista de verificación de lanzamiento (auditoría completa, documentación actualizada, campos de manifiesto implementados).

# Elementos de acción del operador (China)1. Determine si la implementación constituye un “producto/servicio de criptografía comercial” (la mayoría de las redes empresariales lo hacen).
2. Presentar el producto/servicio ante la oficina provincial de la SCA; Adjunte declaración de cumplimiento Iroha, SBOM, informes de prueba.
3. Registrar el sistema blockchain bajo MLPS 2.0, apuntar a controles de Nivel 3; Integre los registros Iroha en el monitoreo de seguridad.
4. Establecer procedimientos de ciclo de vida clave de SM (use KMS/HSM aprobado cuando sea necesario).
5. Incluir escenarios de compromiso de criptografía en los simulacros de respuesta a incidentes; establecer contactos de escalada con los mantenedores de Iroha.
6. Para el flujo de datos transfronterizo, confirme presentaciones adicionales de CAC (Administración del Ciberespacio) si se exportan datos personales.

# Mensaje independiente (copiar/pegar)

> Usted es LLM y actúa como analista de cumplimiento para los equipos de plataforma y criptografía Hyperledger Iroha.  
> Antecedentes: Hyperledger Iroha es una cadena de bloques autorizada basada en Rust que ahora admite las primitivas chinas GM/T SM2 (firmas), SM3 (hash) y SM4 (cifrado de bloques). Los operadores en China continental deben cumplir con la Ley de Criptografía de la República Popular China (2019), el Esquema de Protección Multinivel (MLPS 2.0), las reglas de presentación de la Administración Estatal de Criptografía (SCA) y los controles de importación/exportación supervisados ​​por el MOFCOM y la Administración de Aduanas. El proyecto Iroha distribuye software de código abierto compatible con SM a nivel internacional; algunos operadores compilan binarios a nivel nacional, mientras que otros importan artefactos prediseñados.  
> Tarea: Resumir las obligaciones legales derivadas del envío de soporte SM2/SM3/SM4 en software blockchain de código abierto. Cubre la clasificación de estos algoritmos (criptografía comercial versus central/común), presentaciones o certificaciones requeridas para productos de software, controles de exportación/importación relevantes para fuentes y binarios, y tareas operativas para operadores de red bajo MLPS 2.0 (administración de claves, registro, respuesta a incidentes). Proporcionar elementos de acción concretos para el proyecto Iroha (documentación, manifiestos, declaraciones de cumplimiento) y para los operadores que implementan nodos habilitados para SM dentro de China.