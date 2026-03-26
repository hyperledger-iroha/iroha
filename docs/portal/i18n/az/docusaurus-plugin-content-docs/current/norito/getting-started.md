---
lang: az
direction: ltr
source: docs/portal/docs/norito/getting-started.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito Başlanır

Bu sürətli bələdçi Kotodama müqaviləsini tərtib etmək üçün minimal iş axını göstərir,
yaradılan Norito bayt kodunu yoxlamaq, onu yerli olaraq işə salmaq və yerləşdirmək
Iroha qovşağına.

## İlkin şərtlər

1. Rust alətlər silsiləsi (1.76 və ya daha yeni) quraşdırın və bu anbarı yoxlayın.
2. Dəstəkləyən ikili faylları yaradın və ya endirin:
   - `koto_compile` – Kotodama kompilyatoru IVM/Norito bayt kodunu yayan
   - `ivm_run` və `ivm_tool` - yerli icra və yoxlama kommunalları
   - `iroha_cli` - Torii vasitəsilə müqavilə yerləşdirilməsi üçün istifadə olunur

   Makefile deposu bu ikili faylları `PATH`-də gözləyir. Siz də edə bilərsiniz
   əvvəlcədən qurulmuş artefaktları yükləyin və ya onları mənbədən qurun. Əgər tərtib etsəniz
   alətlər silsiləsi yerli olaraq, Makefile köməkçilərini binarlara yönəldin:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Yerləşdirmə mərhələsinə çatdığınız zaman Iroha qovşağının işlədiyinə əmin olun. The
   Aşağıdakı nümunələr Torii-in sizin konfiqurasiya edilmiş URL-də əldə edilə biləcəyini güman edir.
   `iroha_cli` profili (`~/.config/iroha/cli.toml`).

## 1. Kotodama müqaviləsini tərtib edin

Depo minimal "salam dünya" müqaviləsini göndərir
`examples/hello/hello.ko`. Onu Norito/IVM bayt koduna (`.to`) tərtib edin:

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Əsas bayraqlar:

- `--abi 1` müqaviləni ABI versiyası 1-ə bağlayır (yalnız dəstəklənən versiyada
  yazı vaxtı).
- `--max-cycles 0` məhdudiyyətsiz icra tələb edir; bağlamaq üçün müsbət ədəd təyin edin
  sıfır bilik sübutları üçün dövrü doldurma.

## 2. Norito artefaktını yoxlayın (isteğe bağlı)

Başlığı və daxil edilmiş metadatanı yoxlamaq üçün `ivm_tool` istifadə edin:

```sh
ivm_tool inspect target/examples/hello.to
```

Siz ABI versiyasını, aktiv funksiya bayraqlarını və ixrac edilmiş girişi görməlisiniz
xal. Bu, yerləşdirmədən əvvəl sürətli bir ağlı başında olma yoxlanışıdır.

## 3. Müqaviləni yerli olaraq icra edin

Bir işarəyə toxunmadan davranışı təsdiqləmək üçün `ivm_run` ilə bayt kodunu yerinə yetirin.
qovşaq:

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` nümunəsi salamlamanı qeyd edir və `SET_ACCOUNT_DETAIL` sistem zəngi verir.
Lokal olaraq işləmək, dərc etməzdən əvvəl müqavilə məntiqi üzrə iterasiya zamanı faydalıdır
zəncir üzərində.

## 4. `iroha_cli` vasitəsilə yerləşdirin

Müqavilədən razı qaldığınız zaman onu CLI-dən istifadə edərək qovşaqda yerləşdirin.
Səlahiyyət hesabını, onun imza açarını və ya `.to` faylını və ya
Base64 faydalı yükü:

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

Komanda Torii üzərində Norito manifest + bayt kodu paketini təqdim edir və çap edir
nəticədə əməliyyat statusu. Əməliyyat həyata keçirildikdən sonra kod
Cavabda göstərilən hash manifestləri və ya siyahı nümunələrini əldə etmək üçün istifadə edilə bilər:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii-ə qarşı işləyin

Qeydə alınmış bayt kodu ilə siz təlimat təqdim etməklə onu işə sala bilərsiniz
saxlanılan koda istinad edir (məsələn, `iroha_cli ledger transaction submit` vasitəsilə
və ya proqram müştəriniz). Hesab icazələrinin istədiyinizə icazə verdiyinə əmin olun
sistem zəngləri (`set_account_detail`, `transfer_asset` və s.).

## Məsləhətlər və problemlərin aradan qaldırılması

- Təqdim olunan nümunələri bir yerdə tərtib etmək və icra etmək üçün `make examples-run` istifadə edin
  vuruldu. İkili fayllar aktiv deyilsə, `KOTO`/`IVM` mühit dəyişənlərini ləğv edin
  `PATH`.
- `koto_compile` ABI versiyasını rədd edərsə, tərtibçi və node
  hər ikisi ABI v1-i hədəfləyir (siyahı üçün arqumentlər olmadan `koto_compile --abi`-i işə salın
  dəstək).
- CLI hex və ya Base64 imza açarlarını qəbul edir. Test üçün istifadə edə bilərsiniz
  `iroha_cli tools crypto keypair` tərəfindən buraxılan açarlar.
- Norito faydalı yükləri sazlayarkən, `ivm_tool disassemble` alt əmri kömək edir
  təlimatları Kotodama mənbəyi ilə əlaqələndirin.

Bu axın CI-də istifadə olunan addımları və inteqrasiya testlərini əks etdirir. Daha dərin üçün
Kotodama qrammatikasına, sistem zənglərinin təsvirinə və Norito daxili elementlərinə daxil olun, baxın:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`