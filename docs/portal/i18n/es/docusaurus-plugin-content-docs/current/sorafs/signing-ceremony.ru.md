---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ceremonia de firma
título: Замена церемонии подписания
descripción: Как Парламент Sora утверждает and распространяет fragmenter SoraFS (SF-1b).
sidebar_label: Церемония подписания
---

> Hoja de ruta: **SF-1b — Calendario de утверждения Парламента Sora.**
> El flujo de trabajo parlamentario заменяет устаревшую оффлайн "церемонию подписания совета".

Ручной ритуал подписания accesorios fragmentador SoraFS завершен. Все утверждения теперь
проходят через **Parlament Sora** — DAO на основе жеребьевки, управляющую Nexus.
Члены парламента бондят XOR для получения гражданства, ротируются по панелям и
голосуют on-chain за утверждение, отклонение или откат выпусков luminarias. Esto
гайд объясняет процесс и herramientas para разработчиков.

## Обзор парламента

- **Гражданство** — Los operadores bloquean los TREBуемый XOR, cuáles son los principales y
  получить право на жеребьевку.
- **Panel** — Ответственность распределена между вращающимися панелями
  (Infraestructuras, Moderación, Tesorería,...). Панель Infraestructura отвечает
  за утверждения accesorios SoraFS.
- **Жеребьевка и ротация** — Места в панелях перераспределяются с периодичностью,
  заданной конституцией парламента, чтобы ни одна группа не монополизировала
  утверждения.

## Accesorios de Поток утверждения1. **Отправка предложения**
   - Herramientas WG загружает кандидатный paquete `manifest_blake3.json` y accesorio diferencial
     en el registro en cadena de `sorafs.fixtureProposal`.
   - Предложение фиксирует BLAKE3 digest, семантическую версию и заметки об изменениях.
2. **Ревью и голосование**
   - Панель Infraestructura получает назначение через очередь задач парламента.
   - Paneles completos que utilizan artefactos CI, pruebas de paridad y búsqueda en cadena
     взвешенными голосами.
3. **Finalización**
   - Después de la configuración del tiempo de ejecución del quórum, se emite una conexión con el resumen canónico.
     manifiesto y compromiso de Merkle en el accesorio de carga útil.
   - Событие зеркалируется в registro SoraFS, чтобы клиенты могли получить последний
     manifiesto, утвержденный парламентом.
4. **Producción**
   - CLI helpers (`cargo xtask sorafs-fetch-fixture`) que permiten el manifiesto completo
     Para Nexus RPC. Constantes JSON/TS/Go en repositorios sincronizados
     запуском `export_vectors` y проверкой digest относительно записи en cadena.

## Flujo de trabajo разработчика

- Calendario completo:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Utilice el ayudante de búsqueda de parámetros, descargue el sobre externo y proporcione
  подписи обновить локальные accesorios. Inserte `--signatures` en un sobre, de uso público
  парламентом; ayudante найдет сопутствующий manifiesto, пересчитает BLAKE3 resumen y применит
  perfil canónico `sorafs.sf1@1.0.0`.```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Consulte `--manifest`, o el manifiesto de la URL del sitio. Sobre без подписей
отклоняются, если не задан `--allow-unsigned` для локальных smoke run.

- El manifiesto de validación de la puerta de enlace provisional utiliza Torii en todos los locales
  cargas útiles:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Локальный CI больше не требует roster `signer.json`.
  `ci/check_sorafs_fixtures.sh` сравнивает состояние репозитория с последним on-chain
  compromiso и падает при расхождениях.

## Замечания по gobernancia

- Configuración del quórum superior, rotación y escalamiento — configuración
  на уровне caja не нужна.
- Reversión de emergencia обрабатывается через панель модерации парламента. panel
  La infraestructura puede revertir la propuesta con el manifiesto de resumen anterior,
  и релиз заменяется после утверждения.
- Исторические утверждения сохраняются в registro SoraFS para reproducción forense.

## Preguntas frecuentes

- **¿Куда делся `signer.json`?**  
  Он удален. Все авторство подписей хранится en cadena; `manifest_signatures.json`
  en los repositorios — este accesorio de desarrollador, que se adapta a sus necesidades
  событием утверждения.

- **¿Hay algún lugar local en Ed25519?**  
  No. Утверждения парламента хранятся как on-chain артефакты. Calendario local
  нужны для воспроизводимости, но проверяются по digerir parlamento.- **¿Qué comandos monitorizan el exterior?**  
  Подпишитесь на событие `ParliamentFixtureApproved` или запросите registro через
  Nexus RPC, que requiere un resumen de manifiesto y paneles de texto.