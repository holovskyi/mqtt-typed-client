# PLAN — Сесія A: Pre-publish hygiene (мала, технічна)

**Мета:** закрити дешеві технічні діри перед публікацією 0.2.0. Жодних змін поведінки
рантайму чи публічного API (крім покращення тексту однієї помилки макроса). Усе —
пакування, метадані, лінки, тести, CI-версії.

**Гілка:** `main` напряму. **Один cohesive-коміт** (pre-commit hook усе одно git-add'ить
усі tracked-файли — див. [[mqtt-typed-client-commit-hook-gotcha]]). **Untracked LICENSE-файли
hook НЕ підхопить автоматично — їх треба `git add` вручну.**

### Уточнення після критик-рев'ю
- **Політика версій (B1):** версіонування НЕЗАЛЕЖНЕ (вже вирішено, [[mqtt-typed-client-0-2-0-release]]):
  client-крейти (root/core/macros) = 0.2.0; `mqtt-topic-engine` та `doc-macros` = **0.1.0** (їхній
  перший реліз). У Сесії A **НЕ бампаємо** 0.1.0 → це навмисно, не діра. Порядок публікації (для Сесії C):
  `doc-macros` → `engine` → `core` → `macros` → root (топологічний). Зафіксувати, не виконувати тут.
- **bincode derive (B2):** перевірено — `bincode 2.0.1` має `derive` у **default**-фічах, а `core`
  тягне bincode без `default-features=false` (`cargo tree` підтверджує `derive` активним). Тож
  `#[derive(bincode::Encode, bincode::Decode)]` доступний під `#[cfg(feature="bincode-serializer")]`
  **без змін Cargo.toml**. Жодного кроку додавати не треба.
- **Тестова структура (Z1):** перевикористати наявну `TestMessage { text, id }` з
  `tests/serializers_integration.rs` (derive `Serialize, Deserialize, Encode, Decode`) як зразок —
  одна структура покриває і serde-родину, і bincode (bincode-частина під cfg). Не плодити дублікати.
- **doc-macros README (Z5):** у `doc-macros/README.md` ліцензія згадана **текстом без markdown-лінка**
  → правити лінки там НЕ треба; додати лише LICENSE-файли (Крок 1a).
- **cache (Z6):** не довіряти номерам рядків — зробити глобальну заміну `actions/cache@v3`→`@v4`
  і звірити кількість через `grep -c`.

---

## Знахідки розвідки (стан ДО)

- LICENSE-файли (`LICENSE-MIT`, `LICENSE-APACHE`) є **лише** в root та `mqtt-topic-engine`.
  `core`, `macros`, `doc-macros` — **без** ліцензійних файлів → опубліковані пакети не нестимуть
  тексту ліцензії.
- LICENSE-лінки в README ламаються поза GitHub:
  - `core/README.md:40-41` та `macros/README.md:33-34` → `../LICENSE-*` (битий на crates.io — поза пакетом).
  - root `README.md:12,267-268` та `mqtt-topic-engine/README.md:9,771-772` → відносні `LICENSE-*`;
    обидва README рендеряться як крейт-док на docs.rs (`include_md_transformed!`), де відносний лінк битий.
- `[package.metadata.docs.rs]` — **немає ніде**.
- `actions/cache@v3` — 4 входження в `ci.yml` (рядки 35, 99, 140, 199).
- `core` має **нуль** unit-тестів (`grep #[test]` порожній).
- Макрос: `macros/src/lib.rs:399-413` приймає `serializer = <Path>`; на не-path (generic `T<U>`)
  кидає «Serializer must be a type identifier» — без підказки про обхід.
- Дрібниці: `core/Cargo.toml` має порожню секцію `[lib]`; `mqtt-topic-engine/Cargo.toml` —
  порожню `[dev-dependencies]`; `doc-macros/Cargo.toml` без `categories`.

---

## Обсяг (scope)

### Крок 0. Розвідка пакування (спершу — діагностика)
Прогнати `cargo package --list` для 4 основних крейтів + `doc-macros` (він теж публікується,
бо `engine` залежить від нього з `version`):
```
cargo package --list -p mqtt-typed-client
cargo package --list -p mqtt-typed-client-core
cargo package --list -p mqtt-typed-client-macros
cargo package --list -p mqtt-topic-engine
cargo package --list -p mqtt-typed-client-doc-macros
```
Мета: побачити, які файли реально потраплять у пакет (чи входить README, чи НЕ входить зайве,
чи відсутні LICENSE). `--list` не білдить, тож швидко. Зафіксувати знахідки в TODO.

### Крок 1. LICENSE — файли + лінки
1a. **Додати LICENSE-файли** в `core/`, `macros/`, `doc-macros/` (копії з root). Стандартна
   гігієна: SPDX-поле `license` обовʼязкове й присутнє, але текст ліцензії краще нести у пакеті.
   ⚠️ Untracked → `git add core/LICENSE-* macros/LICENSE-* doc-macros/LICENSE-*` вручну.
1b. **README-лінки → абсолютні GitHub-URL** у всіх 5 README (root, core, macros, engine, doc-macros
   якщо є згадка). Формат:
   `https://github.com/holovskyi/mqtt-typed-client/blob/main/LICENSE-MIT` (і `-APACHE`).
   Стосується і badge-лінка `[![License...]](...)`, і секції «License» внизу.
   Це робить лінки коректними і на crates.io, і на docs.rs, і на GitHub одночасно.

### Крок 2. docs.rs metadata
Додати `[package.metadata.docs.rs]`:
- **root** (`Cargo.toml`): `all-features = true` (root не залежить від paho — безпечно; документує
  всі серіалізатори + TLS re-export).
- **core** (`core/Cargo.toml`): `all-features = true` (core не має paho).
- **engine** (`mqtt-topic-engine/Cargo.toml`): `features = ["rumqttc", "ntex-mqtt"]`
  (НЕ `all-features` — `paho-mqtt` лінкує нативну C-бібліотеку, docs.rs-збірка впаде; обидві обрані
  фічі — чистий Rust). Це покриває interop-модулі + дефолтні `router`/`lru-cache`.
- **macros, doc-macros**: пропустити (фіч немає; `doc-macros` — внутрішній).

### Крок 3. CI cache@v3 → v4
Замінити всі 4 `actions/cache@v3` → `actions/cache@v4` у `.github/workflows/ci.yml`.
Лише версія екшена; ключі/шляхи не чіпати.

### Крок 4. (А) Кращий error-меседж для generic-серіалізатора
У `macros/src/lib.rs` (гілка `else` на `assign.right`, ~рядок 408-412) розширити меседж:
пояснити, що приймається лише простий шлях-тип, а для generic-серіалізатора слід завести
type alias, напр.:
```
"Serializer must be a simple type path (e.g. `JsonSerializer`). For a generic \
 serializer, declare a type alias first: `type MySer = MySerializer<Foo>;` then \
 use `serializer = MySer`."
```
Без зміни логіки парсингу (варіант Б — реальна підтримка generics — НЕ в цій сесії).

### Крок 5. Мінімальні unit-тести для core
Додати `#[cfg(test)] mod tests` у `core/src/message_serializer.rs` — round-trip
(serialize → deserialize → `assert_eq`) для серіалізаторів, кожен під своїм `#[cfg(feature=...)]`,
що збігається з cfg самого серіалізатора:
- serde-родина (`json`, `messagepack`, `cbor`, `postcard`, `ron`, `flexbuffers`): спільна тест-структура
  `#[derive(Serialize, Deserialize, PartialEq, Debug)]` (serde з derive уже є залежністю core).
- `bincode-serializer`: окрема структура `#[derive(bincode::Encode, bincode::Decode, PartialEq, Debug)]`.
- `protobuf`: **пропустити** (потребує `prost::Message`/згенерований тип — не «мінімально»; зафіксувати в TODO як виняток).
Тести детерміновані, без брокера. Перевіряємо і `serialize` (Ok), і рівність після `deserialize`.

### Крок 6. Косметика Cargo.toml (опційно, дешево)
- Прибрати порожні `[lib]` (core) і `[dev-dependencies]` (engine) — або лишити, якщо чіпати ризиковано.
- Додати `categories = ["development-tools::procedural-macro-helpers"]` у `doc-macros/Cargo.toml`
  (прибирає soft-warning crates.io). Опційно.

---

## Поза обсягом (НЕ в цій сесії)
- Реальна підтримка generic-серіалізатора в парсері (варіант Б) — пост-0.2.0.
- `compile_error!` для TLS-конфлікту — лишаємо до емпіричного підтвердження конфлікту.
- README-секції «Per-topic serializer» / «TLS» та bump 0.1.0→0.2.0 в доках — **Сесія B**.
- `examples`-job, що маскує помилки (`|| echo`) — відкладено.
- Будь-яка реальна публікація — **Сесія C**.

---

## Верифікація (наприкінці, перед комітом)
```
cargo +nightly fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-features
cargo test --workspace --lib                       # включно з новими core-тестами
cargo test --all-features --lib && cargo test --all-features --doc
cargo test -p mqtt-topic-engine --features rumqttc,ntex-mqtt --lib
cargo doc --no-deps --all-features                 # RUSTDOCFLAGS=-D warnings як у CI
cargo package --list -p <кожен з 5>                # перепідтвердити, що LICENSE тепер входить
```
Очікування: core-тести зелені; `cargo package --list` для core/macros/doc-macros тепер показує
`LICENSE-MIT`/`LICENSE-APACHE`; жодних нових clippy/doc-ворнінгів.

---

## Ризики
- **engine docs.rs `features`**: переконатися, що `rumqttc + ntex-mqtt` компілюються разом
  (обидва чистий Rust — мають). НЕ додавати `paho-mqtt`.
- **LICENSE untracked**: hook не додасть сам — явний `git add` (інакше пакети знову без ліцензії).
- **bincode-тест**: derive `Encode/Decode` доступні лише під `bincode-serializer` (дефолтна фіча) — cfg-гейт обовʼязковий, інакше build без фічі зламається.
- **Косметика Cargo.toml**: зміна секцій не має зачепити resolve; якщо сумнів — лишити як є.

---

## Коміт (один, cohesive)
Повідомлення (багатосекційне тіло), приклад заголовка:
```
chore(release): pre-publish hygiene for 0.2.0
```
Тіло: LICENSE-файли+лінки, docs.rs metadata, cache@v4, кращий serializer error, core unit-тести.
Завершити трейлером `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
Оновити чекбокси в `.github/dev-docs/TODO-0.2.0-release.md` (включно з protobuf-винятком у тестах).

Workflow: цей план → критик-рев'ю плану → фікси → імплементація → критик-рев'ю імплементації →
фікси → коміт. Див. [[workflow-plan-critique-scheme]], [[mqtt-typed-client-0-2-0-release]].
