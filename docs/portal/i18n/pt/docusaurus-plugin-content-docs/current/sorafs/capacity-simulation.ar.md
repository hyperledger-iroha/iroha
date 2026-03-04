---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: simulação de capacidade
título: دليل تشغيل محاكاة سعة SoraFS
sidebar_label: دليل محاكاة السعة
description: تشغيل مجموعة أدوات محاكاة سوق السعة SF-2c باستخدام fixtures قابلة لإعادة الإنتاج وصادرات Prometheus e Grafana.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. حافظ على تزامن النسختين إلى أن تُنقَل مجموعة توثيق Sphinx القديمة بالكامل.
:::

Você pode usar o SF-2c e o SF-2c para usá-lo. Use o failover e o slashing para usar os fixtures no site. `docs/examples/sorafs_capacity_simulation/`. As cargas úteis são definidas como `sorafs_manifest_stub capacity`; Use `iroha app sorafs toolkit pack` para definir o manifesto/CAR.

## 1. Crie artefatos na CLI

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

Use `run_cli.sh` para `sorafs_manifest_stub capacity` para carregar cargas úteis e blobs de Norito em base64 e usar Torii A configuração JSON é:

- Você pode usar a máquina de lavar roupa em qualquer lugar do mundo.
- أمر نسخ يوزّع المانيفست المجهز عبر أولئك المزوّدين.
- Isso permite que você execute o failover e o failover.
- carga útil não é cortada بعد الانقطاع المُحاكَى.

A chave de segurança é `./artifacts` (não importa qual seja o valor do produto). Instale o `_summary.json` para remover o excesso de peso.

## 2. تجميع النتائج وإصدار المقاييس

```bash
./analyze.py --artifacts ./artifacts
```

يقوم المحلل بإنتاج:

- `capacity_simulation_report.json` - تخصيصات مجمعة, failover de failover, وبيانات نزاع وصفية.
- `capacity_simulation.prom` - O arquivo de texto usado para Prometheus (`sorafs_simulation_*`) é o arquivo de texto do coletor de arquivo de texto, o exportador de nó e o trabalho de raspagem.

Você pode raspar em Prometheus:

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

O coletor de arquivo de texto é `capacity_simulation.prom` (o node-exporter é o exportador de nó `--collector.textfile.directory`).

## 3. Instale o Grafana

1. Em Grafana, use `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Limpe o disco `Prometheus` para raspar o disco.
3. تحقق من اللوحات:
   - **Alocação de cota (GiB)** يعرض الأرصدة الملتزم بها/المخصصة لكل مزوّد.
   - **Failover Trigger** é definido como *Failover Active* e não funciona.
   - **Queda de tempo de atividade durante interrupção** يرسم نسبة الفقدان للمزوّد `alpha`.
   - **Porcentagem de barra solicitada** يعرض نسبة المعالجة المستخرجة من fixture النزاع.

## 4. الفحوصات المتوقعة

- `sorafs_simulation_quota_total_gib{scope="assigned"}` é igual a `600`, cujo valor é >=600.
- `sorafs_simulation_failover_triggered` يعرض `1` ويبرز مقياس المزوّد البديل `beta`.
- `sorafs_simulation_slash_requested` يعرض `0.15` (‏15% de barra) لمعرّف المزوّد `alpha`.

Use `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para usar fixtures usando a CLI.