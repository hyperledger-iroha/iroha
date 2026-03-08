---
lang: es
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Esta página refleja `docs/source/sns/bulk_onboarding_toolkit.md` afin que les
Los operadores externos acceden a la guía SN-3b sin clonar el depósito.
:::

# Kit de herramientas de incorporación del macizo SNS (SN-3b)

**Hoja de ruta de referencia:** SN-3b "Herramientas de incorporación masiva"  
**Artefactos:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Les grands registradors preparent souvent des centaines de Registrations `.sora` ou
`.nexus` con los memes approbations de gouvernance et rails de asentamiento.
Construya las cargas útiles JSON en el main o relancer la CLI ne scale pas, donc SN-3b
Libro de un constructor determinista CSV vers Norito que prepara las estructuras.
`RegisterNameRequestV1` para Torii o CLI. L'helper valide chaque ligne en
amont, emet a la fois un manifeste agrege et du JSON delimitate par nouvelles
Líneas opcionales y pueden cargar automáticamente todas las cargas útiles.
registrar las estructuras de recursos para las auditorías.

## 1. Esquema CSV

El parseur exige la línea de ente siguiente (el orden es flexible):| columna | Requisitos | Descripción |
|---------|--------|-------------|
| `label` | Sí | Libelle demande (caso mixto aceptado; la herramienta se normaliza según la norma v1 y UTS-46). |
| `suffix_id` | Sí | Número de sufijo identificador (decimal o `0x` hexadecimal). |
| `owner` | Sí | Chaine AccountId codificado sin dominio (IH58 preferido, compressed aceptado; se rechaza el sufijo @domain) para el propietario del registro. |
| `term_years` | Sí | Entier `1..=255`. |
| `payment_asset_id` | Sí | Actif de liquidación (por ejemplo `xor#sora`). |
| `payment_gross` / `payment_net` | Sí | Entiers non signes representant des unites nativos de l'actif. |
| `settlement_tx` | Sí | Valeur JSON o cadena literaria que decrivan la transacción de pago o hash. |
| `payment_payer` | Sí | AccountId que autoriza el pago. |
| `payment_signature` | Sí | JSON o cadena literaria que contenga la firma del mayordomo o la tesorería. |
| `controllers` | Opcional | Lista separada por puntos vírgenes o vírgenes de direcciones de cuenta controladora. Por defecto `[owner]` si omis. |
| `metadata` | Opcional | JSON en línea o `@path/to/file.json` proporciona sugerencias de resolución, registros TXT, etc. Por defecto, `{}`. |
| `governance` | Opcional | JSON en línea o `@path` apuntando a un `GovernanceHookV1`. `--require-governance` impone esta columna. |Todas las columnas pueden hacer referencia a un archivo externo prefijando el valor del celular
par `@`. Los caminos están resueltos en relación con el archivo CSV.

## 2. Ejecutor o ayudante

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Opciones de elementos:

- `--require-governance` Rejette les lignes sans hook de gouvernance (utilice pour
  les encheres premium ou les afectations reservees).
- `--default-controllers {owner,none}` decide si les cellules controladores vides
  retombent sur le cuenta propietario.
- `--controllers-column`, `--metadata-column` y `--governance-column` permanentes
  de renommer les colonnes optionnelles lors d'exports amont.

En caso de éxito, el guión escribe un manifiesto agregado:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "ih58...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"ih58...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"xor#sora",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"ih58...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

Si `--ndjson` está disponible, cada vez que `RegisterNameRequestV1` está escrito como un
documento JSON en una línea para que las automatizaciones puedan transmitir archivos
solicitudes directement vers Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/registrations
  done
```

## 3. Soumissions automatizadas

### 3.1 Modo Torii DESCANSO

Especifique `--submit-torii-url` más `--submit-token` o `--submit-token-file` para
pousser chaque entree du manifeste directement vers Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```- L'helper emet un `POST /v1/sns/registrations` par requete et s'arrete au premier
  Error HTTP. Las respuestas se amplían en el registro como registros NDJSON.
- `--poll-status` reinterroge `/v1/sns/registrations/{selector}` después de cada
  Soumission (jusqu'a `--poll-attempts`, predeterminado 5) para confirmar que
  El registro está visible. Fournissez `--suffix-map` (JSON de `suffix_id`
  vers des valeurs "suffix") para que la herramienta derive las letras
  `{label}.{suffix}` para el sondeo.
- Ajustables: `--submit-timeout`, `--poll-attempts`, et `--poll-interval`.

### 3.2 Modo iroha CLI

Pour faire passer chaque entree du manifeste par la CLI, fournissez le chemin du
binario:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Los controladores deben estar entre las entradas `Account` (`controller_type.kind = "Account"`)
  Car la CLI expone la base única de los controladores en las cuentas.
- Los metadatos y la gobernanza de los blobs están escritos en los archivos temporales por par
  requete et transmis a `iroha sns register --metadata-json ... --governance-json ...`.
- Le stdout et stderr de la CLI ainsi que les codes de sortie sont periodises;
  Los códigos distintos de cero interrumpen la ejecución.

Los dos modos de transmisión pueden funcionar en conjunto (Torii y CLI) para
Haga clic en las implementaciones del registrador o repetidor de fallas.

### 3.3 Recus de soumission

Cuando se incluye `--submission-log <path>`, el script se agrega a las entradas NDJSON
capturante:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```Les reponses Torii reussies incluyen los campos estructuras extraits de
`NameRecordV1` o `RegisterNameResponseV1` (por ejemplo `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) según los paneles y los
informes de gobierno pueden analizar el registro sin inspeccionar el texto libre.
Joignez ce log aux tickets registrador avec le manifeste pour una evidencia
reproducible.

## 4. Automatización de la liberación del portal

Les jobs CI et portail apelante `docs/portal/scripts/sns_bulk_release.sh`, qui
encapsule l'helper et stocke les artefactos sous
`artifacts/sns/releases/<timestamp>/`:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

El guión:

1. Construya `registrations.manifest.json`, `registrations.ndjson` y copie el archivo.
   CSV original en el repertorio de lanzamiento.
2. Soumet le manifeste via Torii et/ou la CLI (cuando configure), y escriba
   `submissions.log` con estructuras recus ci-dessus.
3. Emet `summary.json` decrivant la liberación (caminos, URL Torii, camino CLI,
   marca de tiempo) después de la automatización del portal después de cargar las versiones del paquete
   le stockage d'artefacts.
4. Producto `metrics.prom` (anulación mediante `--metrics`) contenido de los ordenadores
   en formato Prometheus para el total de solicitudes, la distribución de sufijos,
   les totaux d'asset et les resultats de soumission. Le JSON reanudar pointe vers
   ce archivo.Les flujos de trabajo archivan simplemente el repertorio de lanzamiento como un solo artefacto,
qui contient desormais tout ce dont la gouvernance a besoin pour l'audit.

## 5. Telemetría y paneles de control

Le fichier de mériques genere par `sns_bulk_release.sh` exponen les series
siguientes:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Inyecte `metrics.prom` en su sidecar Prometheus (por ejemplo a través de Promtail o
un lote de importación) para registradores de alineadores, administradores y pares de gobierno en
El avance en masa. El cuadro Grafana
`dashboards/grafana/sns_bulk_release.json` visualizar les memes donnees avec des
paneles para las cuentas por sufijo, el volumen de pago y los ratios de
reussite/echec des soumissions. El filtro de tabla par `release` para que les
Los auditores pueden concentrarse en una sola ejecución CSV.

## 6. Validación y modos de revisión- **Normalización de etiquetas:** las entradas están normalizadas con Python IDNA plus
  minúsculas y filtros de caracteres Norm v1. Les etiquetas invalides echouent vite
  avant tout appel reseau.
- **Números variados:** ID de sufijo, años de plazo y sugerencias de precios.
  rester dans les bornes `u16` et `u8`. Los campeones de pago aceptante des
  números decimales o hexadecimales `i64::MAX`.
- **Análisis de metadatos o gobernanza:** el JSON en línea se analiza directamente; les
  Las referencias a los archivos son resoluciones relativas a la ubicación del CSV.
  Los metadatos no objeto producen un error de validación.
- **Controladores:** les cellules vides respectent `--default-controllers`. Fournissez
  des listes explicites (por ejemplo `ih58...;ih58...`) quand vous deleguez a des
  actores no propietarios.

Les echecs sont signales avec des numeros de ligne contextuels (por ejemplo
`error: row 12 term_years must be between 1 and 255`). Le script sort avec le
code `1` sur erreurs de validation et `2` lorsque le chemin CSV manque.

## 7. Pruebas y procedencia- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` busca el análisis CSV,
  La emisión NDJSON, la gobernanza de la aplicación y los caminos de emisión CLI o Torii.
- L'helper est du Python pur (aucune dependencia adicional) et tourne partout
  o `python3` está disponible. L'historique des commits est suivi aux cotes de la
  CLI en el depósito principal para la reproducibilidad.

Para las ejecuciones de producción, junte el manifiesto genere y el paquete NDJSON au
ticket du registrador afin que les stewards puissent rejouer les payloads exactas
entonces es Torii.
