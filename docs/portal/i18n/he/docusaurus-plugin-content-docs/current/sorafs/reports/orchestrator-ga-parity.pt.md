---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Relatorio de paridade GA do Orchestrator SoraFS

A paridade deterministica de multi-fetch agora e monitorada por SDK para que engenheiros de release confirmem que
מטען בתים, קבלות נתח, דוחות ספק ותוצאות לוח תוצאות קבועות
ליישם. קאדה רתמה לצריכה או צרור קנוניקו מרובה ספקים
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`, que empacota o plano SF1, מטא נתונים של ספק, תמונת מצב של טלמטריה ו
opcoes לעשות מתזמר.

## חלודה בסיס

- **קומנדו:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Escopo:** Executa o plano `MultiPeerFixture` duas vezes via o orchestrator in process, verificando
  בתים של מטען מטען, קבלות נתח, דוחות ספק ותוצאות לוח תוצאות. אינסטרומנטקאו
  tambem acompanha a concorrencia de pico e o tamanho efetivo do working-set (`max_parallel x max_chunk_length`).
- **משמר ביצועים:** Cada execucao deve concluir em 2 s no hardware de CI.
- **תקרת סט עבודה:** Com o perfil SF1 o harness aplica `max_parallel = 3`, resultando em uma
  Janela <= 196608 בתים.

דוגמה של יומן:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## רתום לעשות SDK JavaScript

- **קומנדו:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Escopo:** Reproduz o mesmo מתקן דרך `iroha_js_host::sorafsMultiFetchLocal`, השוואת מטענים,
  קבלות, דוחות ספק וצילומי מצב עושים את לוח התוצאות לפני הביצועים ברציפות.
- **שומר ביצועים:** Cada execucao deve finalizar em 2 s; o לרתום imprime a duracao medida e o
  teto de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

דוגמה לקורות חיים:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## רתום לעשות SDK Swift

- **קומנדו:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Escopo:** Executa a suite de paridade definida em `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`,
  מתקן SF1 duas vezes pelo bridge Norito (`sorafsLocalFetch`). הו רתום אימות בתים של מטען,
  נתח קבלות, דוחות ספקים ו-entradas doboard score usando ספק mesma metadata deterministica
  צילומי טלמטריה das suites Rust/JS.
- **רצועת אתחול של גשר:** O Ratt descompacta `dist/NoritoBridge.xcframework.zip` יבבה דרישה e Carrega או חתך macOS באמצעות
  `dlopen`. Quando o xcframework esta ausente ou nao tem bindings SoraFS, faz fallback para
  `cargo build -p connect_norito_bridge --release` e linka contra `target/release/libconnect_norito_bridge.dylib`,
  entao nenhum מדריך להתקנה הכרחי עבור CI.
- **משמר ביצועים:** Cada execucao deve terminar em 2 s no hardware de CI; o לרתום imprime a duracao medida e o
  teto de bytes reservados (`max_parallel = 3`, `peak_reserved_bytes <= 196608`).

דוגמה לקורות חיים:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## רתום את הכריכות Python- **קומנדו:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Escopo:** Exercita o wrapper de alto nivel `iroha_python.sorafs.multi_fetch_local` e suas dataclasses
  tipadas para que o fixture canonico passe pela mesma API que consumidores de wheel usam. הו אשך
  שחזור מטא-נתונים של ספק ב-`providers.json`, הכנס תמונת מצב של טלמטריה ואימות
  בתים של מטען, קבלות נתח, דוחות ספקים ולוח תוצאות רגילים כמו חבילות Rust/JS/Swift.
- ** דרישה מוקדמת:** בצע את `maturin develop --release` (או התקנת גלגל) עבור `_crypto` exponha o
  מחייב `sorafs_multi_fetch_local` antes de chamar pytest; o רתמה ל-auto-ignora quando o כריכה
  nao estiver disponivel.
- **משמר הופעה:** Mesmo orcamento <= 2 s da suite Rust; pytest registra a contagem de bytes
  montados e o resumo de participacao de providers para o artefato de release.

O release gating deve capturar o פלט סיכום של הרתמה (Rust, Python, JS, Swift) עבור
relatorio arquivado possa השוואת קבלות של מטען e metricas de forma uniforme antes de promotor
אממ לבנות. בצע את `ci/sdk_sorafs_orchestrator.sh` para rodar todas as suites de paridade (Rust, Python
כריכות, JS, Swift) em uma unica passada; artefatos de CI devem anexar o trecho de log desse helper
mais o `matrix.md` gerado (tabela SDK/status/duration) ao ticket de release para que reviewers possam
auditar a matriz de paridade sem reexecutar a suite localmente.