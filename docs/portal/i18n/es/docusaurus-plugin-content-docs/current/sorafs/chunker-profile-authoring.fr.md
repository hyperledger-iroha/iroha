---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: creación de perfiles fragmentados
título: Guía de creación de perfiles fragmentados SoraFS
sidebar_label: Guía de creación de fragmentadores
descripción: Lista de verificación para proponer nuevos perfiles fragmentados SoraFS y accesorios.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/chunker_profile_authoring.md`. Guarde las dos copias sincronizadas junto con la retraite complète du set Sphinx hérité.
:::

# Guía de creación de perfiles fragmentados SoraFS

Esta guía propone comentarios explícitos y publica nuevos perfiles fragmentados para SoraFS.
El completo RFC de arquitectura (SF-1) y la referencia de registro (SF-2a)
con exigencias de redacción concreta, etapas de validación y modelos de proposición.
Pour un exemple canonique, voir
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
et le log de dry-run associé dans
`docs/source/sorafs/reports/sf1_determinism.md`.

## Vista del conjunto

Chaque profil qui entre dans le registre doit:

- Anuncio de parámetros CDC determinados y ajustes multihash idénticos entre
  arquitecturas;
- Fournir des fixtures rejouables (JSON Rust/Go/TS + corpora fuzz + témoins PoR) que
  Los SDK disponibles se pueden verificar sin herramientas de medición;
- inclure des métadonnées prêtes pour la gouvernance (espacio de nombres, nombre, semver) además de
  des conseils de rollout et des fenêtres opérationnelles; y
- pase la suite de diferencias determinada antes de la revista del consejo.Suivez la checklist ci-dessous para preparar una propuesta que respete estas reglas.

## Descripción de la tabla de registro

Antes de redactar una propuesta, verifique que respete la carta de registro aplicada
par `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Los ID de perfil son todos los aspectos positivos que aumentan la apariencia monótona sin problemas.
- Le handle canonique (`namespace.name@semver`) debe aparecer en la lista de alias
- Aucun alias no puede entrar en colisión con otro identificador canónico ni aparato más de una vez.
- Les alias doivent être non vides et trimés des espaces.

Ayudas útiles CLI:

```bash
# Listing JSON de tous les descripteurs enregistrés (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Émettre des métadonnées pour un profil par défaut candidat (handle canonique + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Ces commandes maintiennent les propositions alignées avec la charte du registre et fournissent
les métadonnées canoniques nécessaires aux listenings de gouvernance.

## Métadonnées requiere| Campeón | Descripción | Ejemplo (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | Regroupement logique de profils liés. | `sorafs` |
| `name` | Libellé lisible. | `sf1` |
| `semver` | Cadena de versión sémántica para el conjunto de parámetros. | `1.0.0` |
| `profile_id` | Identificador numérico monotono atribuido una vez al perfil integrado. Reservez l'id suivant mais ne réutilisez pas les numeros existentes. | `1` |
| `profile.min_size` | Longitud mínima de fragmentos en bytes. | `65536` |
| `profile.target_size` | Longitud posible de fragmentos en bytes. | `262144` |
| `profile.max_size` | Longitud máxima de fragmentos en bytes. | `524288` |
| `profile.break_mask` | Máscara adaptativa utilizada por el hash rodante (hexadecimal). | `0x0000ffff` |
| `profile.polynomial` | Engranaje constante del polinôme (hexagonal). | `0x3da3358b4dc173` |
| `gear_seed` | Semilla utilizada para derivar la mesa de engranajes de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Código multihash para resúmenes por fragmento. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Resumen del paquete canónico de accesorios. | `13fa...c482` |
| `fixtures_root` | Répertorio relativo al contenido de los accesorios regénérées. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Semilla para el échantillonnage PoR déterministe (`splitmix64`). | `0xfeedbeefcafebabe` (ejemplo) |Les métadonnées doivent apparaître à la fois dans le document de proposition et à l'intérieur des
Accesorios generados según el registro, las herramientas CLI y la automatización de gobierno potente
confirmer les valeurs sans recoupements manuels. En caso de duda, ejecute las CLI chunk-store y
manifest avec `--json-out=-` para transmitir los metadonnées calculées dans les notes de revue.

### Puntos de contacto CLI y registro

- `sorafs_manifest_chunk_store --profile=<handle>` — relanzar les métadonnées de chunk,
  le digest du manifest et les checks PoR avec les paramètres proposés.
- `sorafs_manifest_chunk_store --json-out=-` — Streamer le rapport versiones de almacén de fragmentos
  salida estándar para comparaciones automáticas.
- `sorafs_manifest_stub --chunker-profile=<handle>` — confirmer que les manifests et les
  planes CAR embarquent le handle canonique et les alias.
- `sorafs_manifest_stub --plan=-`: reinyecta el `chunk_fetch_specs` anterior para
  verifique las compensaciones/resúmenes después de la modificación.

Consignez la sortie des commandes (resúmenes, racines PoR, hashes de manifest) dans la proposition afin
que los revisores puedan reproducir las palabras para las palabras.

## Lista de verificación de determinismo y validación1. **Regénérer los accesorios**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Ejecutar la suite de parité** — `cargo test -p sorafs_chunker` et le Harness diff
   en varios idiomas (`crates/sorafs_chunker/tests/vectors.rs`) doivent être verts avec les
   accesorios nuevos en su lugar.
3. **Rejouer les corpora fuzz/back-pression** — exécutez `cargo fuzz list` et le Harness de
   streaming (`fuzz/sorafs_chunker`) contre les activos régénérés.
4. **Verificador de témoins Prueba de recuperabilidad** — ejecutar
   `sorafs_manifest_chunk_store --por-sample=<n>` con el perfil propuesto y confirme que les
   corresponsal de racines en el manifiesto de accesorios.
5. **CI de prueba** — invoquez `ci/check_sorafs_fixtures.sh` localement; el guión
   doit réussir avec les nouvelles fixtures et le `manifest_signatures.json` existente.
6. **Confirmación entre tiempos de ejecución**: asegúrese de que los enlaces Go/TS contengan el JSON
   régénéré et émettent des limites et digests identiques.

Documentez les commandes et les digests résultants dans la proposition afin que le Tooling WG puisse
les rejouer sans conjecture.

### Manifiesto de confirmación / PoR

Después de la regeneración de accesorios, ejecute el manifiesto de tubería completo para garantizar que les
métadonnées CAR et les preuves PoR restent cohérentes :

```bash
# Valider les métadonnées chunk + PoR avec le nouveau profil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Générer manifest + CAR et capturer les chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Relancer avec le plan de fetch sauvegardé (évite les offsets obsolètes)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Reemplace el archivo de entrada por un corpus representativo utilizado por sus accesorios
(ej., el flujo determinado de 1 GiB) y junte los resúmenes resultantes de la proposición.

## Modelo de propuestaLas proposiciones son reveladas bajo forma de registros Norito `ChunkerProfileProposalV1` depositadas en
`docs/source/sorafs/proposals/`. La plantilla JSON ci-dessous ilustra la forma asistente
(reemplace sus valores si es necesario):


Fournissez un rapport Markdown corresponsal (`determinism_report`) que captura la salida de
Los comandos, los resúmenes de fragmentos y todas las divergencias se encuentran durante la validación.

## Flujo de gobernanza

1. **Soumettre une PR con propuesta + accesorios.** Incluye los activos generados, la
   proposición Norito et les mises à jour de `chunker_registry_data.rs`.
2. **Revue Tooling WG.** Los revisores agregan la lista de verificación de validación y confirmación
   que la proposición respeta las reglas del registro (pas de réutilisation d'id,
   déterminismo satisfactorio).
3. **Sobre del consejo.** Une fois approuvée, les membres du conseil signent le digest
   de la proposición (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) y añadido
   sus firmas en el sobre del perfil almacenado con los accesorios.
4. **Publicación del registro.** La combinación se realiza cada día en el registro, los documentos y los accesorios.
   El CLI por defecto descansa en el perfil anterior justo cuando el gobierno declara la
   prête de migración.
5. **Suivi de depréciation.** Después de la ventana de migración, conecte el registro para el día siguiente
   de migración.

## Consejos de creación- Prefiera las potencias de dos pares de bornes para minimizar el comportamiento de trituración en la tabla.
- Évitez de changer le code multihash sans coordonner les consommateurs manifest et gateway;
  Incluya una nota operativa cuando haga lo que haga.
- Guarde las semillas de equipos de mesa flexibles y únicas a nivel mundial para simplificar las auditorías.
- Stockez tout artefacto de benchmarking (ej., comparaciones de débito) sous
  `docs/source/sorafs/reports/` para referencia futura.

Para las operaciones operativas pendientes del lanzamiento, consulte el libro mayor de migración
(`docs/source/sorafs/migration_ledger.md`). Para las reglas de conformidad del tiempo de ejecución, ver
`docs/source/sorafs/chunker_conformance.md`.