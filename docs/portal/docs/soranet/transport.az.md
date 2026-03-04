---
lang: az
direction: ltr
source: docs/portal/docs/soranet/transport.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6113bf3d608167f409a9a20b80770bb3ba1d3c050e97492070ee2f9ac706d567
source_last_modified: "2026-01-05T09:28:11.916816+00:00"
translation_last_reviewed: 2026-02-07
id: transport
title: SoraNet transport overview
sidebar_label: Transport Overview
description: Handshake, salt rotation, and capability guidance for the SoraNet anonymity overlay.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

SoraNet SoraFS diapazonunu, Norito RPC axınını və gələcək Nexus məlumat zolaqlarını dəstəkləyən anonimlik örtüyüdür. Nəqliyyat proqramı (yol xəritəsi elementləri **SNNet-1**, **SNNet-1a** və **SNNet-1b**) deterministik əl sıxma, post-kvant (PQ) qabiliyyəti danışıqları və duz fırlanma planını müəyyən etdi ki, hər rele, müştəri və şlüz eyni təhlükəsizlik duruşunu müşahidə etsin.

## Məqsədlər və şəbəkə modeli

- QUIC v1 üzərində üç-hoplu sxemlər (giriş → orta → çıxış) qurun ki, təhqiramiz həmyaşıdlar heç vaxt birbaşa Torii-ə çatmasın.
- Sessiya açarlarını TLS transkriptinə bağlamaq üçün QUIC/TLS-in üstünə Noise XX *hibrid* əl sıxma (Curve25519 + Kyber768) qat edin.
- PQ KEM/imza dəstəyini, relay rolunu və protokol versiyasını reklam edən qabiliyyət TLV-lərini tələb edin; Gələcək genişləndirmələrin yerləşdirilə bilməsi üçün naməlum növləri yağlayın.
- Kor məzmunlu duzları gündəlik fırladın və 30 gün ərzində qoruyucu releləri bağlayın ki, kataloq çaxnaşması müştəriləri deanonimləşdirməsin.
- Hüceyrələri 1024B-də sabit saxlayın, doldurma/dummy xanaları yeridin və deterministik telemetriyanı ixrac edin ki, səviyyəni endirmə cəhdləri tez tutulsun.

## Əl sıxma boru kəməri (SNNet-1a)

1. **QUIC/TLS zərfi** – müştərilər QUIC v1 üzərindən releləri yığırlar və idarəetmə CA tərəfindən imzalanmış Ed25519 sertifikatlarından istifadə edərək TLS1.3 əl sıxışmasını tamamlayırlar. TLS ixracatçısı (`tls-exporter("soranet handshake", 64)`) Səs-küy qatını toxumlayır, beləliklə transkriptlər ayrılmazdır.
2. **Noise XX hibrid** – proloq = TLS ixracatçısı olan `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` protokol sətri. Mesaj axını:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Curve25519 DH çıxışı və hər iki Kyber encapsulations son simmetrik düymələrə qarışdırılır. PQ materialının müzakirə edilməməsi əl sıxma prosesini tamamilə dayandırır - heç bir klassik geri dönüşə icazə verilmir.

3. **Puzzle biletləri və nişanlar** – relaylar `ClientHello`-dən əvvəl Argon2id iş sübutu biletini tələb edə bilər. Biletlər hashed Argon2 həllini daşıyan və siyasət hüdudlarında müddəti bitən uzunluqlu prefiksli çərçivələrdir:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Emitentdən ML-DSA-44 imzası aktiv siyasətə və ləğvetmə siyahısına qarşı doğrulandıqda, `SNTK` prefiksli qəbul nişanları tapmacalardan keçər.

4. **Qabiliyyət TLV mübadiləsi** – son Səs yükü aşağıda təsvir edilən qabiliyyət TLV-lərini nəql edir. Müştərilər hər hansı məcburi imkan (PQ KEM/imza, rol və ya versiya) yoxdursa və ya kataloq girişi ilə uyğun gəlmirsə, əlaqəni dayandırır.

5. **Transkript qeydi** – relelər aşağı səviyyəli detektorları və uyğunluq boru kəmərlərini qidalandırmaq üçün transkript hashini, TLS barmaq izini və TLV məzmununu qeyd edir.

## Qabiliyyətli TLV (SNNet-1c)

İmkanlar sabit `typ/length/value` TLV zərfindən təkrar istifadə edir:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Bu gün müəyyən edilmiş növlər:

- `snnet.pqkem` – Kyber səviyyəsi (cari buraxılış üçün `kyber768`).
- `snnet.pqsig` – PQ imza dəsti (`ml-dsa-44`).
- `snnet.role` – rele rolu (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` – protokol versiyası identifikatoru.
- `snnet.grease` – gələcək TLV-lərə yol verilməsini təmin etmək üçün qorunan diapazonda təsadüfi doldurucu daxiletmələr.

Müştərilər tələb olunan TLV-lərin icazəli siyahısını saxlayır və onları buraxan və ya aşağı səviyyəyə salan uğursuz əl sıxmalarını saxlayır. Röleler eyni dəsti öz kataloq mikrodeskriptorunda dərc edir, beləliklə, doğrulama deterministikdir.

## Duz fırlanması və CID-nin korlanması (SNNet-1b)

- İdarəetmə `(epoch_id, salt, valid_after, valid_until)` dəyərləri ilə `SaltRotationScheduleV1` qeydini dərc edir. Rele və şlüzlər imzalanmış cədvəli kataloq naşirindən alır.
- Müştərilər yeni duzu `valid_after`-də tətbiq edir, əvvəlki duzu 12 saat güzəşt müddəti üçün saxlayır və gecikmiş yeniləmələrə dözmək üçün 7 dövr tarixini saxlayır.
- Kanonik kor identifikatorlar istifadə edir:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Şlüzlər korlanmış açarı `Sora-Req-Blinded-CID` vasitəsilə qəbul edir və onu `Sora-Content-CID`-də əks etdirir. `iroha_crypto::soranet::blinding`-də dövrə/sorğu korlaması (`CircuitBlindingKey::derive`) göndərilir.
- Əgər rele bir dövrü qaçırarsa, o, cədvəli endirənə qədər yeni dövrələri dayandırır və çağırışda olan tablosunun peyqinq siqnalı kimi qəbul etdiyi `SaltRecoveryEventV1` siqnalını buraxır.

## Kataloq məlumatları və mühafizə siyasəti

- Mikrodeskriptorlar relay identifikasiyası (Ed25519 + ML-DSA-65), PQ düymələri, qabiliyyət TLV-ləri, region teqləri, mühafizə uyğunluğu və hazırda reklam edilən duz dövrünü daşıyır.
- Müştərilər mühafizə dəstlərini 30 gün ərzində bağlayır və imzalanmış kataloq snapşotunun yanında `guard_set` keşlərini saxlayırlar. CLI və SDK sarğıları keş barmaq izini üzə çıxarır ki, rəyləri dəyişdirmək üçün təqdimat sübutları əlavə olunsun.

## Telemetriya və yayım yoxlama siyahısı

- İstehsaldan əvvəl ixrac üçün ölçülər:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Xəbərdarlıq hədləri duz fırlanma SOP SLO matrisi (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) ilə yanaşı yaşayır və şəbəkə təşviq edilməzdən əvvəl Alertmanager-də əks etdirilməlidir.
- Xəbərdarlıqlar: 5 dəqiqə ərzində >5% uğursuzluq dərəcəsi, duz gecikməsi >15 dəqiqə və ya istehsalda müşahidə olunan qabiliyyət uyğunsuzluğu.
- Yayım addımları:
  1. Hibrid əl sıxma və PQ yığınını aktivləşdirərək səhnələşdirmə üzrə relay/müştəri qarşılıqlı fəaliyyət testlərini həyata keçirin.
  2. Duz fırlanma SOP-u (`docs/source/soranet_salt_plan.md`) təkrarlayın və qazma artefaktlarını dəyişiklik qeydinə əlavə edin.
  3. Kataloqda qabiliyyət danışıqlarını aktivləşdirin, sonra giriş relelərinə, orta relelərə, çıxışlara və nəhayət müştərilərə keçin.
  4. Hər bir mərhələ üçün qoruyucu keş barmaq izlərini, duz cədvəllərini və telemetriya panellərini qeyd edin; sübut paketini `status.md`-ə əlavə edin.

Bu yoxlama siyahısından sonra operator, müştəri və SDK komandalarına SNNet yol xəritəsində əks olunmuş determinizm və audit tələblərinə cavab verən SoraNet daşımaları blokadada qəbul etməyə imkan verir.