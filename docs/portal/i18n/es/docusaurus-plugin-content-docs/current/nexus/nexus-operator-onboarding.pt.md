---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: incorporación-de-operador-nexus
título: Integracao de operadores de data-space Sora Nexus
descripción: Espelho de `docs/source/sora_nexus_operator_onboarding.md`, acompañando una lista de verificación de liberación de extremo a extremo para operadores Nexus.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sora_nexus_operator_onboarding.md`. Mantenha as duas copias alinhadas ate que as edicoes localizadas cheguem ao portal.
:::

# Integracao de operadores de data-space Sora Nexus

Esta guía de captura de flujo de extremo a extremo que los operadores de espacio de datos Sora Nexus deben seguir cuando se publique y se anuncie. El complemento al runbook de dupla via (`docs/source/release_dual_track_runbook.md`) y una nota de selección de artefatos (`docs/source/release_artifact_selection.md`) descrevendo como alinhar bundles/imagens baixados, manifests y templates de configuracao com as expectativas de lanes globais antes de colocar um no em producao.## Audiencia y requisitos previos
- Voce foi aprovado pelo Programa Nexus y recebeu sua atribuicao de data-space (índice de carril, ID/alias de espacio de datos y requisitos de política de enrutamiento).
- Voce consegue acessar os artefatos assinados do release publicados pelo Release Engineering (tarballs, imagens, manifests, assinaturas, chaves publicas).
- Voce grou ou recebeu material de chaves de producao para su papel de validador/observador (identidad de no Ed25519; chave de consenso BLS + PoP para validadores; mais quaisquer toggles de recursos confidenciais).
- Voce consegue alcancar os peers Sora Nexus existente que farao bootstrap do seu no.

## Etapa 1 - Confirmar o perfil de lanzamiento
1. Identifique o alias de red o ID de cadena fornecido.
2. Ejecute `scripts/select_release_profile.py --network <alias>` (o `--chain-id <id>`) en un checkout de este repositorio. O ayudante consulta `release/network_profiles.toml` e imprime o perfil para implementar. Para Sora Nexus una respuesta debe ser `iroha3`. Para cualquier otro valor, póngase en contacto con Release Engineering.
3. Anote o tag de versao referenciado no anuncio do release (por ejemplo `iroha3-v3.2.0`); voce o usara para buscar artefatos e manifests.## Etapa 2 - Recuperar y validar artefactos
1. Baje el paquete `iroha3` (`<profile>-<version>-<os>.tar.zst`) y sus archivos compañeros (`.sha256`, opcional `.sig/.pub`, `<profile>-<version>-manifest.json`, e `<profile>-<version>-image.json` se voce fizer desplegar con contenedores).
2. Valide a integridade antes de descompactar:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Sustituya `openssl` por el verificador aprobado por la organización que usa un KMS con hardware.
3. Inspeccione `PROFILE.toml` dentro del tarball y los manifiestos JSON para confirmar:
   - `profile = "iroha3"`
   - Los campos `version`, `commit` e `built_at` corresponden al anuncio del lanzamiento.
   - O OS/arquitetura corresponde al momento de implementar.
4. Si usa una imagen de contenedor, repita la verificación de hash/assinatura para `<profile>-<version>-<os>-image.tar` y confirme la ID de imagen registrada en `<profile>-<version>-image.json`.## Etapa 3 - Preparar configuración a partir de dos plantillas
1. Extraiga el paquete y copie `config/` para la conexión local o no vai ler sua configuración.
2. Trate los archivos sollozo `config/` como plantillas:
   - Sustituye `public_key`/`private_key` por sus chaves Ed25519 de producción. Elimina chaves privadas de la discoteca se o no vai obtelas de um HSM; Realice una configuración para colocar el conector HSM.
   - Ajuste `trusted_peers`, `network.address` e `torii.address` para reflejar sus interfaces alcancavas y los pares de bootstrap que el foro atribuido.
   - Actualizar `client.toml` con el punto final Torii para operadores (incluida la configuración TLS y aplicavel) y como credenciales provisionadas para herramientas operativas.
3. Mantenha o chain ID fornecido no bundle a menos que Governance instrua explícitamente - a lane global espera um unico identificador canonico de cadeia.
4. Planeje iniciar o no com o flag de perfil Sora: `irohad --sora --config <path>`. El cargador de configuración rejeitara las configuraciones SoraFS o multicarril si o la bandera está ausente.## Etapa 4 - Alinhar metadatos de espacio de datos y enrutamiento
1. Edite `config/config.toml` para que a secao `[nexus]` corresponda al catálogo de espacio de datos fornecido pelo Nexus Consejo:
   - `lane_count` debe ser igual al total de carriles habilitados en la época actual.
   - Cada entrada en `[[nexus.lane_catalog]]` e `[[nexus.dataspace_catalog]]` debe contener un `index`/`id` único y los alias acordados. Nao apague as entradas globalis existentes; adicione seus alias delegados se o conselho atribuiu data-spaces adicionais.
   - Garantía que cada entrada de dataspace incluye `fault_tolerance (f)`; comités lane-relay sao dimensionados en `3f+1`.
2. Atualize `[[nexus.routing_policy.rules]]` para capturar a politica que lhe foi dada. O template padrao roteia instrucoes de Governanca para a lane `1` e implementa de contratos para a lane `2`; Agregue o modifique registros para que el tráfico destinado a su espacio de datos seja encaminado para un carril y un alias de corretos. Coordene con Release Engineering antes de alterar el orden de las grabaciones.
3. Revise los límites de `[nexus.da]`, `[nexus.da.audit]` e `[nexus.da.recovery]`. Espera-se que os operadores mantengan os valores aprobados por el consejo; ajuste apenas se uma política atualizada foi ratificada.4. Registre la configuración final en su rastreador de operaciones. O runbook de liberación de dupla vía exige anexar o `config.toml` efectivo (con secretos redigidos) al ticket de onboarding.

## Etapa 5 - Prevuelo de Validacao
1. Ejecute el validador de configuración embutido antes de entrar a la red:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Esto imprime una configuración resuelta y falta cedo si las entradas de catálogo/enrutamiento son inconsistentes o si génesis y configuración divergirem.
2. Se voce fizer desplegar com contenedores, ejecute o mesmo comando dentro de la imagen apos carregala com `docker load -i <profile>-<version>-<os>-image.tar` (lembre de incluir `--sora`).
3. Verifique los registros para avisos sobre identificadores de marcador de posición de carril/espacio de datos. Luego, regrese a la Etapa 4: implementa la producción que depende de los marcadores de posición de ID que acompañan a las plantillas.
4. Ejecute su procedimiento local de smoke (por ejemplo, envíe una consulta `FindNetworkStatus` con `iroha_cli`, confirme que los puntos finales de telemetría expoem `nexus_lane_state_total`, y verifique que as chaves de streaming foram rotacionadas ou importadas conforme sea necesario).## Etapa 6 - Transición y traspaso
1. Guarde o `manifest.json` verificado e os artefactos de assinatura no ticket de liberación para que los auditores possam reproduzir sus verificacoes.
2. Notifique Nexus Operations de que o no esta pronto para ser introducido; incluye:
   - Identidade no (ID de par, nombres de host, punto final Torii).
   - Valores efectivos del catálogo de lane/data-space y política de enrutamiento.
   - Hashes de dos binarios/imagens verificados.
3. Coordene a admissao final de peers (gossip seeds e atribuicao de lane) con `@nexus-core`. Nao entre na rede ate receber aprovacao; Sora Nexus aplica la ocupación determinística de carriles y exige un manifiesto de admisión actualizado.
4. Después de no estiver ativo, actualice sus runbooks con quaisquer overrides que voce introduziu y anote la etiqueta de lanzamiento para que una próxima iteración comience a partir de esta línea de base.

## Lista de verificación de referencia
- [ ] Perfil de lanzamiento validado como `iroha3`.
- [] Hashes y combinaciones de paquetes/imagen verificados.
- [ ] Chaves, enderecos de peers e endpoints Torii atualizados para valores de producción.
- [] Catálogo de carriles/espacio de datos y política de enrutamiento de Nexus correspondiente a la atribución del consejo.
- [ ] Validador de configuración (`irohad --sora --config ... --trace-config`) pasa sin avisos.
- [ ] Manifiestos/assinaturas archivadas sin ticket de onboarding e Ops notificado.Para contexto más amplio sobre fases de migración de Nexus y expectativas de telemetría, revise las [notas de transición de Nexus](./nexus-transition-notes).