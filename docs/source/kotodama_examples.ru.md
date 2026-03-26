---
lang: ru
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T15:34:14.183250+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama Обзор примеров

На этой странице показаны краткие примеры Kotodama и то, как они сопоставляются с системными вызовами IVM и аргументами указателя-ABI. См. также:
- `examples/` для работоспособных источников
- `docs/source/ivm_syscalls.md` для канонического системного вызова ABI.
- `kotodama_grammar.md` для полной спецификации языка.

## Здравствуйте + данные аккаунта

Источник: `examples/hello/hello.ko`

```
seiyaku Hello {
  hajimari() { info("Hello from Kotodama"); }

  kotoage fn write_detail() permission(Admin) {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
```

Сопоставление (указатель-ABI):
- `authority()` → `SCALL 0xA4` (хост записывает `&AccountId` в `r10`)
- `set_account_detail(a, k, v)` → переместите `r10=&AccountId`, `r11=&Name`, `r12=&Json`, затем `SCALL 0x1A`

## Передача активов

Источник: `examples/transfer/transfer.ko`

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
      account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```

Сопоставление (указатель-ABI):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount`, затем `SCALL 0x24`

## Создание NFT + передача

Источник: `examples/nft/nft.ko`

```
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let recipient = account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

Сопоставление (указатель-ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, `SCALL 0x26`

## Указатель Norito Помощники

Устойчивое состояние со значением указателя требует преобразования типизированных TLV в и из
`NoritoBytes` конверт, в котором хосты сохраняются. Kotodama теперь подключает этих помощников.
напрямую через компилятор, чтобы разработчики могли использовать значения указателя по умолчанию и отображать
поиск без ручного клея FFI:

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let first = get_or_insert_default(Owners, 7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    let again = get_or_insert_default(Owners, 7, bob);
    assert(again == alice);
  }
}
```

Понижение:

- По умолчанию указатели выдают `POINTER_TO_NORITO` после публикации типизированного TLV, поэтому
  хост получает каноническую полезную нагрузку `NoritoBytes` для хранения.
- Чтение выполняет обратную операцию с `POINTER_FROM_NORITO`, предоставляя
  ожидаемый идентификатор типа указателя в `r11`.
- Оба пути автоматически публикуют буквальные TLV в область INPUT, что позволяет
  контракты для прозрачного смешивания строковых литералов и указателей времени выполнения.

См. `crates/ivm/tests/kotodama_pointer_args.rs` для регрессии времени выполнения, которая
выполняет обратный обход против `MockWorldStateView`.

## Итерация детерминированной карты (проектирование)

Детерминированная карта для каждого требует границы. Итерация с несколькими входами требует карты состояний; компилятор принимает `.take(n)` или заявленную максимальную длину.

```
// design example (iteration requires bounds and state storage)
state M: Map<int, int>;

fn sum_first_two() -> int {
  let s = 0;
  for (k, v) in M.take(2) {
    s = s + v;
  }
  return s;
}
```

Семантика:
- Набор итераций представляет собой снимок при входе в цикл; Порядок лексикографический по Norito байтам ключа.
- Структурные мутации `M` в петлевой ловушке с `E_ITER_MUTATION`.
- Без ограничений компилятор выдает `E_UNBOUNDED_ITERATION`.

## Внутренние компоненты компилятора/хоста (Rust, а не исходный код Kotodama)

Приведенные ниже фрагменты находятся на стороне Rust цепочки инструментов. Они иллюстрируют помощники компилятора и механизм понижения уровня виртуальной машины и **не** действительный источник Kotodama `.ko`.

## Обновления фрагментированных кадров с широким кодом операции

Помощники широкого кода операции Kotodama нацелены на 8-битную структуру операндов, используемую IVM.
широкая кодировка. Загрузки и сохранения, которые перемещают 128-битные значения, повторно используют третий операнд.
слот для старшего регистра, поэтому базовый регистр уже должен содержать конечный регистр.
адрес. Настройте базу с помощью `ADDI` перед выполнением загрузки/сохранения:

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```Обновления фрагментированных кадров продвигают базу шагами по 16 байт, гарантируя, что регистр
пара, зафиксированная `STORE128`, попадает на требуемую границу выравнивания. То же самое
шаблон применим к `LOAD128`; выдача `ADDI` с желаемым шагом перед
при каждой загрузке старший регистр назначения привязан к третьему слоту операнда.
Ловушка несовмещенных адресов с `VMError::MisalignedAccess`, соответствующая виртуальной машине
поведение, реализованное в `crates/ivm/tests/wide_memory128.rs`.

Программы, которые создают эти 128-битные помощники, должны объявлять векторные возможности.
Компилятор Kotodama автоматически включает бит режима `VECTOR` всякий раз, когда
Появляются `LOAD128`/`STORE128`; ловушка виртуальной машины с
`VMError::VectorExtensionDisabled`, если программа пытается их выполнить.
без этого бита.

## Опускание широкой условной ветки

Когда Kotodama понижает `if`/`else` или троичную ветвь до широкого байт-кода, он выдает
фиксированная последовательность `BNE cond, zero, +2`, за которой следует пара инструкций `JAL`:

1. Короткий `BNE` удерживает условную ветвь в пределах 8-битной непосредственной полосы.
   перепрыгнув через провал `JAL`.
2. Первый `JAL` нацелен на блок `else` (выполняется, когда условие
   ложь).
3. Второй `JAL` переходит к блоку `then` (берётся при выполнении условия
   правда).

Этот шаблон гарантирует, что при проверке условия никогда не потребуется кодировать смещения большего размера.
более ±127 слов, сохраняя при этом поддержку произвольно больших тел для `then`
и блоки `else` через широкий помощник `JAL`. См.
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` для
регрессионный тест, фиксирующий последовательность.

### Пример понижения

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

Компилируется в следующий широкий скелет команд (номера регистров и
абсолютные смещения зависят от охватывающей функции):

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

Последующие инструкции материализуют константы и записывают возвращаемое значение.
Поскольку `BNE` перескакивает через первый `JAL`, условное смещение всегда равно
`+2`, сохраняя ветвь в пределах диапазона, даже когда тела блока расширяются.