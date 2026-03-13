---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 20b155bf2418ccdfb4981e52af44816bed8fc256ba8e54a78f6b9b320450b8fc
source_last_modified: "2026-01-22T14:35:36.747633+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-ops
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

## Baxış

Bu runbook SoraFS pin reyestrini və onun təkrarlama xidməti səviyyəli razılaşmalarını (SLAs) necə izləmək və yoxlamaq lazım olduğunu sənədləşdirir. Metriklər `iroha_torii`-dən yaranır və `torii_sorafs_*` ad məkanı altında Prometheus vasitəsilə ixrac edilir. Torii reyestr vəziyyətini fonda 30 saniyəlik intervalda nümunə götürür, beləliklə, heç bir operator `/v2/sorafs/pin/*` son nöqtələrini sorğulamasa belə, tablosuna cari qalır. Aşağıdakı bölmələrə birbaşa xəritələnən istifadəyə hazır Grafana düzümü üçün seçilmiş idarə panelini (`docs/source/grafana_sorafs_pin_registry.json`) idxal edin.

## Metrik Referans

| Metrik | Etiketlər | Təsvir |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Həyat dövrü vəziyyətinə görə zəncirdə manifest inventar. |
| `torii_sorafs_registry_aliases_total` | — | Reyestrdə qeydə alınmış aktiv manifest ləqəblərinin sayı. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Replikasiya sırası geridə qalan siyahı statusa görə seqmentləşdirilmişdir. |
| `torii_sorafs_replication_backlog_total` | — | `pending` sifarişlərini əks etdirən rahatlıq göstəricisi. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA mühasibat uçotu: `met` son tarixdə tamamlanmış sifarişləri hesablayır, `missed` gec tamamlamaları + istifadə müddətini birləşdirir, `pending` yerinə yetirilməmiş sifarişləri əks etdirir. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Ümumi tamamlama gecikməsi (buraxılış və tamamlama arasındakı dövrlər). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Gözləyən sifarişli boş pəncərələr (son tarix minus buraxılış dövrü). |

Bütün ölçülər hər bir snapshot çəkilişində sıfırlanır, beləliklə, tablolar `1m` kadensdə və ya daha sürətli nümunə götürməlidir.

## Grafana İdarə Paneli

İdarə paneli JSON operatorun iş axınlarını əhatə edən yeddi panellə göndərilir. Sifarişli diaqramlar qurmağı üstün tutursunuzsa, sorğular tez istinad üçün aşağıda verilmişdir.

1. **Manifest həyat dövrü** – `torii_sorafs_registry_manifests_total` (`status` ilə qruplaşdırılıb).
2. **Ləqəb kataloqu trend** – `torii_sorafs_registry_aliases_total`.
3. **Status üzrə sifariş növbəsi** – `torii_sorafs_registry_orders_total` (`status` ilə qruplaşdırılıb).
4. **Backlog vs müddəti bitmiş sifarişlər** – səthin doyması üçün `torii_sorafs_replication_backlog_total` və `torii_sorafs_registry_orders_total{status="expired"}`-i birləşdirir.
5. **SLA müvəffəqiyyət nisbəti** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Gecikmə və son tarix gecikməsi** – üst-üstə düşmə `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` və `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Mütləq boş yerə ehtiyacınız olduqda `min_over_time` görünüşlərini əlavə etmək üçün Grafana çevrilmələrindən istifadə edin, məsələn:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Qaçırılmış sifarişlər (1 saatlıq tarif)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Xəbərdarlıq hədləri

- **SLA müvəffəqiyyəti < 0,95 15 dəqiqə**
  - Həddi: `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Fəaliyyət: Səhifə SRE; replikasiya geriləmə triajına başlayın.
- **10-dan yuxarı gözlənilən geriləmə**
  - Həddi: `torii_sorafs_replication_backlog_total > 10` 10 dəqiqə davam etdi
  - Fəaliyyət: Provayderin mövcudluğunu və Torii tutum planlayıcısını yoxlayın.
- **Müddəti bitmiş sifarişlər > 0**
  - Həddi: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Fəaliyyət: Provayderin işdən çıxmasını təsdiqləmək üçün idarəetmə manifestlərini yoxlayın.
- **Tamamlama p95 > son tarix orta hesabla**
  - Həddi: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Fəaliyyət: Provayderlərin son tarixlərdən əvvəl öhdəlik götürdüyünü yoxlayın; yenidən təyinatların verilməsini nəzərdən keçirin.

### Nümunə Prometheus Qaydaları

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Triaj iş axını

1. **Səbəbi müəyyən edin**
   - Əgər gecikmə az qalarkən SLA artımı əldən verirsə, provayderin performansına diqqət yetirin (PoR uğursuzluqları, gec tamamlamalar).
   - Əgər geridə qalma sabit itkilərlə artırsa, şuranın təsdiqini gözləyən manifestləri təsdiqləmək üçün qəbulu (`/v2/sorafs/pin/*`) yoxlayın.
2. **Provayder statusunu doğrulayın**
   - `iroha app sorafs providers list`-i işə salın və reklam edilən imkanların replikasiya tələblərinə uyğunluğunu yoxlayın.
   - Təmin edilmiş GiB və PoR müvəffəqiyyətini təsdiqləmək üçün `torii_sorafs_capacity_*` ölçü cihazlarını yoxlayın.
3. **Replikasiyanı yenidən təyin edin**
   - `sorafs_manifest_stub capacity replication-order` vasitəsilə yeni sifarişlər verin (`stat="avg"`) 5 dövrdən aşağı düşdükdə (manifest/CAR qablaşdırması `iroha app sorafs toolkit pack` istifadə edir).
   - Əgər ləqəblərdə aktiv manifest bağlamaları yoxdursa, idarəetməni xəbərdar edin (`torii_sorafs_registry_aliases_total` gözlənilmədən azalır).
4. **Sənədin nəticəsi**
   - SoraFS əməliyyatlar jurnalında insident qeydlərini vaxt ştampları və təsirlənmiş manifest həzmləri ilə qeyd edin.
   - Yeni uğursuzluq rejimləri və ya idarə panelləri təqdim edilərsə, bu runbook-u yeniləyin.

## Yayım Planı

İstehsalda ləqəb keş siyasətini aktivləşdirərkən və ya sərtləşdirərkən bu mərhələli prosedura əməl edin:

1. **Konfiqurasiyanı hazırlayın**
   - `torii.sorafs_alias_cache`-i `iroha_config`-də (istifadəçi → faktiki) razılaşdırılmış TTL-lər və lütf pəncərələri ilə yeniləyin: `positive_ttl`, `refresh_window`, `hard_expiry`, I10802, I10802 `revocation_ttl`, `rotation_max_age`, `successor_grace` və `governance_grace`. Defolt parametrlər `docs/source/sorafs_alias_policy.md` siyasətinə uyğun gəlir.
   - SDK-lar üçün eyni dəyərləri onların konfiqurasiya təbəqələri (Rust / NAPI / Python bağlamalarında `AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)`) vasitəsilə paylayın ki, müştəri tətbiqi şlüzlə uyğun olsun.
2. **Təhsildə quru qaçış**
   - Konfiqurasiya dəyişikliyini istehsal topologiyasını əks etdirən bir quruluş klasterinə yerləşdirin.
   - `cargo xtask sorafs-pin-fixtures`-i işə salın, kanonik ləqəb qurğularının hələ də deşifrəni və gediş-gəlişini təsdiqləmək; hər hansı uyğunsuzluq ilk növbədə həll edilməli olan yuxarıya doğru açıq sürüşməni nəzərdə tutur.
   - `/v2/sorafs/pin/{digest}` və `/v2/sorafs/aliases` son nöqtələrini təzə, təzələmə pəncərəsi, vaxtı keçmiş və müddəti bitmiş halları əhatə edən sintetik sübutlarla istifadə edin. HTTP status kodlarını, başlıqları (`Sora-Proof-Status`, `Retry-After`, `Warning`) və JSON əsas sahələrini bu runbook ilə təsdiqləyin.
3. **İstehsalda aktivləşdirin**
   - Standart dəyişiklik pəncərəsi vasitəsilə yeni konfiqurasiyanı işə salın. Əvvəlcə onu Torii-ə tətbiq edin, sonra qovşaq qeydlərdə yeni siyasəti təsdiqlədikdən sonra şlüzləri/SDK xidmətlərini yenidən başladın.
   - `docs/source/grafana_sorafs_pin_registry.json`-i Grafana-ə idxal edin (və ya mövcud tablosunu yeniləyin) və ləqəbli keş yeniləmə panellərini NOC iş sahəsinə yapışdırın.
4. **Yerləşdirmədən sonra doğrulama**
   - 30 dəqiqə ərzində `torii_sorafs_alias_cache_refresh_total` və `torii_sorafs_alias_cache_age_seconds` monitorinqi. `error`/`expired` əyrilərindəki sünbüllər siyasət yeniləmə pəncərələri ilə əlaqələndirilməlidir; gözlənilməz artım o deməkdir ki, operatorlar davam etməzdən əvvəl ləqəb sübutlarını və provayderin sağlamlığını yoxlamalıdır.
   - Müştəri tərəfi qeydlərinin eyni siyasət qərarlarını göstərdiyini təsdiqləyin (sübut köhnəldikdə və ya müddəti bitdikdə SDK-lar səhvləri üzə çıxaracaq). Müştəri xəbərdarlıqlarının olmaması səhv konfiqurasiya olduğunu göstərir.
5. **Geri dönüş**
   - Əgər ləqəb verilməsi geridə qalırsa və yeniləmə pəncərəsi tez-tez işə düşürsə, konfiqurasiyada `refresh_window` və `positive_ttl` artıraraq siyasəti müvəqqəti rahatlaşdırın, sonra yenidən yerləşdirin. `hard_expiry`-i toxunulmaz saxlayın ki, həqiqətən köhnəlmiş sübutlar hələ də rədd edilsin.
   - Telemetriya yüksək `error` saylarını göstərməyə davam edərsə, əvvəlki `iroha_config` snapshotını bərpa etməklə əvvəlki konfiqurasiyaya qayıdın, sonra ləqəb yaratmaq gecikmələrini izləmək üçün insident açın.

## Əlaqədar Materiallar

- `docs/source/sorafs/pin_registry_plan.md` — icra yol xəritəsi və idarəetmə konteksti.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — yaddaş işçisi əməliyyatları, bu reyestr kitabçasını tamamlayır.