---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: incorporación-de-operador-nexus
título: Integración de operadores de espacio de datos Sora Nexus
descripción: Miroir de `docs/source/sora_nexus_operator_onboarding.md`, siguiente a la lista de verificación de liberación de combates para los operadores Nexus.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sora_nexus_operator_onboarding.md`. Guarde las dos copias alineadas justo al llegar a las ediciones localizadas en el portal.
:::

# Integración de operadores de espacio de datos Sora Nexus

Esta guía captura el flujo de combate en el que los operadores del espacio de datos Sora Nexus deben seguir una vez con un anuncio de lanzamiento. Complete el runbook a doble vía (`docs/source/release_dual_track_runbook.md`) y la nota de selección de artefactos (`docs/source/release_artifact_selection.md`) en un comentario descriptivo que alinea los paquetes/imágenes telecargados, los manifiestos y las plantillas de configuración con los asistentes globales de líneas antes de establecer un nuevo enlace.## Audiencia y requisitos previos
- Debe aprobar el programa Nexus y recuperar la afectación del espacio de datos (índice de carril, ID/alias del espacio de datos y exigencias políticas de ruta).
- Puede acceder a los artefactos firmados por la versión pública de Release Engineering (archivos comprimidos, imágenes, manifiestos, firmas, claves públicas).
- Tiene genere ou recu le material de cles de production pour su rol de validador/observador (identite de noeud Ed25519; cle de consenso BLS + PoP for les validators; plus tout toggle de fonctionnalite confidentielle).
- Podrás unir los pares Sora Nexus existentes que arrancan tu nuevo dispositivo.

## Etapa 1 - Confirma el perfil de lanzamiento
1. Identifique el alias de reseau o el ID de cadena que esté allí.
2. Lanza `scripts/select_release_profile.py --network <alias>` (o `--chain-id <id>`) en un checkout de este depósito. El ayudante consulta `release/network_profiles.toml` e imprime el perfil en el implementador. Para Sora Nexus la respuesta debe ser `iroha3`. Para todo otro valor, póngase en contacto con Release Engineering.
3. Notez le tag de version reference par l'annonce du release (por ejemplo `iroha3-v3.2.0`); vous l'utiliserez pour recuperer les artefactos et manifests.## Etapa 2 - Recuperar y validar los artefactos
1. Descargue el paquete `iroha3` (`<profile>-<version>-<os>.tar.zst`) y sus archivos compañeros (`.sha256`, opcional `.sig/.pub`, `<profile>-<version>-manifest.json`, e `<profile>-<version>-image.json` si desea implementar los contenidos).
2. Validez l'integrite avant descompresser:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Reemplace `openssl` por el verificador aprobado por la organización si utiliza un material KMS.
3. Inspeccione `PROFILE.toml` en el archivo tar y los manifiestos JSON para confirmar:
   - `profile = "iroha3"`
   - Los campeones `version`, `commit` e `built_at` corresponden al anuncio del lanzamiento.
   - L'OS/arquitectura corresponde a votre cible de deploiement.
4. Si utiliza el contenido de la imagen, repita el hash/firma de verificación para `<profile>-<version>-<os>-image.tar` y confirme la ID de imagen registrada en `<profile>-<version>-image.json`.## Etapa 3 - Preparar la configuración a partir de plantillas
1. Extraiga el paquete y copie `config/` para su ubicación o configuración.
2. Traitez les fichiers sous `config/` como las plantillas:
   - Reemplace `public_key`/`private_key` por sus piezas Ed25519 de producción. Suprima las llaves privadas del disco si no hay fuente después de un HSM; Agregue cada día la configuración para que el puntero se conecte al conector HSM.
   - Ajuste `trusted_peers`, `network.address` e `torii.address` para reflejar las interfaces accesibles y los pares asignados de bootstrap.
   - Conecte el día `client.toml` con el terminal Torii, la cuenta del operador (y comprende la configuración TLS, si corresponde) y los identificadores provisionados para el servicio operativo.
3. Conserve la identificación de la cadena proporcionada en el paquete con instrucciones explícitas de gobernanza: la línea global asiste a un identificador de cadena canónico único.
4. Planifiez le demarrage du noeud avec le flag de profil Sora: `irohad --sora --config <path>`. El cargador de configuración rechaza los parámetros SoraFS o multicarril si la bandera está ausente.## Etapa 4: Alinear los metadatos del espacio de datos y la ruta
1. Edite `config/config.toml` para que la sección `[nexus]` corresponda al catálogo de espacio de datos proporcionado por Nexus Consejo:
   - `lane_count` Mantenga la misma cantidad de líneas activas en la época actual.
   - Cada entrada en `[[nexus.lane_catalog]]` e `[[nexus.dataspace_catalog]]` debe contener un `index`/`id` único y los alias convenus. Ne supprimez pas les entrees globales existentes; Agregue sus alias delegados si le asigna atributos adicionales de espacios de datos.
   - Asegúrese de que cada entrada del espacio de datos incluya `fault_tolerance (f)`; Las dimensiones del relé de carril de los comités son `3f+1`.
2. Mettez a jour `[[nexus.routing_policy.rules]]` pour capturer la politique qui vous a ete attribuee. La plantilla por defecto enruta las instrucciones de gobierno hacia el carril `1` y las implementaciones de contratos versus el carril `2`; Ajuste o modifique las reglas para que el tráfico destinado a su espacio de datos se dirija hacia el carril y el alias correcto. Coordonnez avec Release Engineering antes de cambiar el orden de las reglas.
3. Revoyez les seuils `[nexus.da]`, `[nexus.da.audit]` et `[nexus.da.recovery]`. Los operadores son censes conservan los valores aprobados por el consejo; ajustez-les onlyment si una politique mise a jour a ete ratifiee.4. Registre la configuración final en su rastreador de operaciones. El runbook de liberación a doble vía exige d'attacher le `config.toml` effectif (secrets rediges) au ticket d'onboarding.

##Etape 5 - Prevuelo de validación
1. Ejecute el validador de configuración integrado antes de volver a unirse al archivo:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Cela imprime la configuración resolue y echoue tot si les entrees catalogue/routage sont incoherentes o si genesis et config divergent.
2. Si despliegas los contenidos, ejecuta el meme comando en la imagen después de cargar con `docker load -i <profile>-<version>-<os>-image.tar` (pensez a incluir `--sora`).
3. Verifique los registros para los anuncios en los identificadores de marcador de posición de carril/espacio de datos. Si es así, vuelva a la etapa 4: las implementaciones de producción no deben depender de las ID de los marcadores de posición y de las plantillas.
4. Ejecute su procedimiento local de humo (p. ej. soumettre une requete `FindNetworkStatus` con `iroha_cli`, confirme que los puntos finales telemetrie exponennt `nexus_lane_state_total`, y verifique que las claves de transmisión son giradas o importadas según las exigencias).## Etapa 6 - Transición y traspaso
1. Almacene el `manifest.json` verifique y los artefactos de firma en el boleto de liberación para que los auditores puedan reproducir sus verificaciones.
2. Informez Nexus Operations que le noeud est pret a etre introduit; incluye:
   - Identite du noeud (ID de igual, nombres de host, punto final Torii).
   - Valores efectivos del catálogo lane/data-space y política de ruta.
   - Hashes des binaires/images verifica.
3. Coordonnez l'admission finale des pairs (chismes semillas et afectación de carril) avec `@nexus-core`. Ne rejoignez pas le reseau avant d'avoir recu l'approbation; Sora Nexus aplica una ocupación determinante de las vías y requiere un manifiesto de admisión durante el día.
4. Después de la puesta en línea del noeud, inicie un día con los runbooks con las introducciones de overrides y observe la etiqueta de lanzamiento para la iteración de la prochaine parte de esta línea base.## Lista de verificación de referencia
- [] Perfil de liberación válida como `iroha3`.
- [] Verificaciones de hashes y firmas del paquete/imagen.
- [] Cles, adresses de pairs et endpoints Torii son un día en producción de valores.
- [ ] Catálogo lanes/dataspace et politique de routage Nexus correspondiente a la afectación del consejo.
- [] Validador de configuración (`irohad --sora --config ... --trace-config`) pasado sin anuncios.
- [ ] Archivos de manifiestos/firmas en el ticket de incorporación y notificación de operaciones.

Para un contexto más amplio sobre las fases de migración Nexus y los asistentes de telemetría, consulte [Nexus notas de transición](./nexus-transition-notes).