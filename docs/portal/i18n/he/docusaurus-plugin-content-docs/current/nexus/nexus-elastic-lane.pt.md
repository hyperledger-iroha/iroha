---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-elastic-lane
כותרת: Provisionamento de lane elastico (NX-7)
sidebar_label: Provisionamento de lane elastico
תיאור: Fluxo de bootstrap לקריאת מניפסטים של ליין Nexus, קטלוגים וראיות להפעלה.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/nexus_elastic_lane.md`. Mantenha ambas as copias alinhadas ate que o sweep de localizacao chegue ao פורטל.
:::

# Kit de provisionamento de lane elastico (NX-7)

> ** מפת הדרכים של פריט:** NX-7 - כלי עבודה של מסלול אלסטי  
> **סטטוס:** השלמה של כלי עבודה - גרא מניפסטים, קטעי קטלוג, מטענים Norito, בדיקות עשן,
> e o helper de bundle de load-test agora costura gating de latencia por slot + manifests de evidencia para que as rodadas de carga de validadores
> possam ser publicadas sem scripting sob medida.

Este Guia Leva Operatores pelo novo helper `scripts/nexus_lane_bootstrap.sh` que automatisa a geracao de manifests de lane, snippets of catalogo de lane/dataspace evidencia de rollout. O objetivo e facilitar a criacao de novas lanes Nexus (publicas ou privadas) כמו עורך מדריך למגוון arquivos נגזר מחדש את הגיאומטריה לעשות קטלוג ידני.

## 1. דרישות מוקדמות

1. Aprovacao de governanca para o alias de lane, dataspace, conjunto de validadores, tolerancia a falhas (`f`) e politica de settlement.
2. אומה רשימה סופית של validadores (IDs de conta) ו אומה רשימה של מרחבי שמות protegidos.
3. Acesso ao repositorio de configuracao do node para poder anexar os snippets gerados.
4. Caminhos para o registro de manifests de lane (veja `nexus.registry.manifest_directory` e `cache_directory`).
5. Contatos de telemetria/handles do PagerDuty para o lane, para que os alertas sejam conectados assim que o lane estiver online.

## 2. Gere artefatos de lane

בצע את העוזר לחלק את המאגר:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

דגלים חוצים:

- `--lane-id` deve corresponder ao index da nova entrada em `nexus.lane_catalog`.
- `--dataspace-alias` ו-`--dataspace-id/hash` שליטה בקטלוגים של מרחב הנתונים (פור פאדראו בארה"ב או יד לעשות ליין quando omitido).
- `--validator` pode ser repetido ou lido de `--validators-file`.
- `--route-instruction` / `--route-account` emitem regras de roteamento prontas para colar.
- `--metadata key=value` (או `--telemetry-contact/channel/runbook`) תקצירים של רונבוק עבור לוחות מחוונים של רוב בעלי מערכות הפעלה.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` אדיקונם או הוק לשדרוג זמן ריצה או מניפסט קוונדו או דרישת נתיב בקרת ההפעלה.
- `--encode-space-directory` chama `cargo xtask space-directory encode` אוטומטי. Combine com `--space-directory-out` quando quiser que o arquivo `.to` codificado va para outro lugar alem do default.

O script produz tres artefatos dentro do `--output-dir` (for padrao o diretorio atual), mais um quarto optional quando o קידוד esta habilitado:1. `<slug>.manifest.json` - מניפסט ליין או מניין חוקי, מרחבי שמות מוגנים ואופציות מתאימות לעשות הוק לשדרוג זמן ריצה.
2. `<slug>.catalog.toml` - um snippet TOML com `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` e quaisquer regras de roteamento solicitadas. Garanta que `fault_tolerance` מגדיר את תחום הנתונים במרחב הממדים או ממסר הנתיב (`3f+1`).
3. `<slug>.summary.json` - resumo de auditoria descrevendo a geometria (שבלול, segmentos, metadados) mais os passos de rollout requeridos e o comando exato de `cargo xtask space-directory encode` (em `space_directory_encode.command`). תוספת של JSON או כרטיס כניסה למטוס כמו הוכחות.
4. `<slug>.manifest.to` - emitido quando `--encode-space-directory` esta habilitado; pronto para o fluxo `iroha app space-directory manifest publish` עד Torii.

השתמש ב-`--dry-run` עבור Visualizar OS JSON/snippets sem gravar arquivos e `--force` para sobrescrever artefatos existentes.

## 3. Aplique as mudancas

1. העתק את המניפסט של JSON להגדרת `nexus.registry.manifest_directory` (לדוגמה של ספריית מטמון או רישום חבילות רישום). Comite o arquivo se manifestes sao versionados no seu repositorio de configuracao.
2. Anexe o snippet de catalogo em `config/config.toml` (ou no `config.d/*.toml` apropriado). Garanta que `nexus.lane_count` seja pelo menos `lane_id + 1` e atualize quaisquer `nexus.routing_policy.rules` que devam apontar para o novo lane.
3. קידוד (לפי `--encode-space-directory`) ופרסום או מניפסט ללא ספריית חלל usando o comando capturado ללא סיכום (`space_directory_encode.command`). ישו תוצרת או מטען `.manifest.to` que o Torii espera e registra evidencia para auditores; envie דרך `iroha app space-directory manifest publish`.
4. בצע את `irohad --sora --config path/to/config.toml --trace-config` e arquive a saida do trace no ticket de rollout. Isso prova que a nova geometria corresponde ao slug/segmentos de Kura gerados.
5. Reinicie os validadores atribuidos ao lane quando as mudancas de manifest/catalogo estiverem implantadas. מנטה או תקציר JSON ללא כרטיס עבור אודיטוריות עתידיות.

## 4. Monte um bundle de distribuicao do הרישום

Empacote o Manifest Gerado e o Overlay para que Operatores possam distribuir dados de governanca de lanes סאם תצורות עריכה של המארח. העוזר של מניפסטים של צרור עותקים עבור פריסת קנוניקה, תיצור שכבת-על אופציונלית לעשות קטלוג ניהולי עבור `nexus.registry.cache_directory` e pode emitir um tarball para transferencias offline:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

סעידס:

1. `manifests/<slug>.manifest.json` - copie estes arquivos para o `nexus.registry.manifest_directory` configurado.
2. `cache/governance_catalog.json` - coloque em `nexus.registry.cache_directory`. קובץ `--module` מוגדר כהגדרת מודולי plugavel, מאפשר החלפה של מודולי ניהול (NX-2) או אטואלייזר או שכבת מטמון עם עריכה `config.toml`.
3. `summary.json` - כולל גיבוב, מטאדוסים לעשות שכבת-על והוראות להפעלה.
4. אופציונלי `registry_bundle.tar.*` - pronto para SCP, S3 ou trackers de artefatos.

סיקור או מדריך לאיירו (או תקציר) עבור קוד אימות, אקסטרה עם מארח אוויר-gapped e copie os manifestes + overlay de cache para seus caminhos de registry antes de reiniciar o Torii.## 5. מבחני עשן

Depois que o Torii reiniciar, בצע או נובו עוזר דה עשן לבדיקת נתיב דיווח `manifest_ready=true`, ראה כמטריקה אקספואם א קונטאגם esperada de lanes e se o gauge de sealed esta limpo. Lanes que exigem manifests devem expor um `manifest_path` nao vazio; o עוזר אגורה פעלה מיידית quando o caminho falta para que cada deploy NX-7 כולל הוכחות לעשות גילוי נאות:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Adicione `--insecure` ao testar ambientes חתום בעצמו. O script sai com codigo nao zero se o lane estiver ausente, sealed ou se metricas/telemetria divergirem dos valores esperados. השתמש בכפתורי מערכת ההפעלה `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` e `--max-headroom-events` למטרת טלמטריה פור ליין (אלטורה דה בלוקו/סופידה/פיגור/מרווח ראש) dentro doscionais limites opera `--max-slot-p95` / `--max-slot-p99` (mais `--min-slot-samples`) para impor as metas de duracao de slot NX-18 sem sair do helper.

Para validacoes air-gapped (ou CI) voce pode reproduzir uma resposta Torii capturada em vez de acessar um live point end:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Os fixtures gravados em `fixtures/nexus/lanes/` refletem os artefatos produzidos pelo helper de bootstrap para que novos manifests possam ser lintados sem scripting sob medida. A CI executa o mesmo fluxo via `ci/check_nexus_lane_smoke.sh` e `ci/check_nexus_lane_registry_bundle.sh` (כינוי: `make check-nexus-lanes`) para provar que o helper de smoke NX-7 continua compativel com o formato de payload publicado e garantir que oss digestm fivez.