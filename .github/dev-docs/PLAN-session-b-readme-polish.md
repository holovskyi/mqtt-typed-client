# PLAN — Сесія B: README-поліш (якість + контент)

**Мета:** підняти якість публічних README (root + engine) перед релізом: прибрати
AI-smell (декоративні емодзі), додати чесні «Alternatives», додати відсутні контент-секції
(per-topic serializer, TLS/transport — root), за можливості TOC. README рендеряться як
крейт-доки (`include_md_transformed!`) → усе йде і на docs.rs, і на crates.io.

**Гілка:** `main` напряму. **Один cohesive-коміт** (pre-commit hook git-add'ить tracked —
див. [[mqtt-typed-client-commit-hook-gotcha]]).

---

## КРИТИЧНІ обмеження (інакше зламаємо збірку/CI)

1. **README-блоки ` ```rust ` — це ДОКТЕСТИ.** CI ганяє `cargo test --all-features --doc` +
   `cargo test -p mqtt-topic-engine --doc`. **Будь-який новий код-блок мусить компілюватись**
   (no_run/run/ignore за тим же принципом, що вже в репо). Це вже ловило реальний API-drift —
   не послаблювати. TLS-приклад → `no_run` (під all-features rustls увімкнено); чисто-ілюстративні
   фрагменти без валідного API → ` ```text ` або `ignore`.
2. **Трансформер (`doc-macros`) робить РІВНО одне:** заміняє `[examples/](examples/)` →
   `` [`crate::examples`] `` (+ авто-хедер). Якорі (`#section`) та інші лінки **не чіпає**.
   Тож TOC-якорі трансформер не ламає — ризик лише в тому, як **rustdoc/docs.rs** генерує
   якорі заголовків (перевіряється локально, див. крок 5).
3. **Не фабрикувати «Alternatives».** Реальні крейти **знайти на crates.io** (search), звірити що
   існують і живі; якщо чогось немає — не вигадувати. Тон — чесний, не маркетинговий.
4. **Версії НЕ чіпаємо тут** (див. «Поза обсягом»).

---

## Обсяг (scope)

### Крок 1. Прибрати емодзі
**root README** (`README.md`): зняти декоративні емодзі **з усіх 12 заголовків**
(рядки 3,18,32,112,132,153,183,223,263,272,284,288) + інлайнові декоративні в тексті.
Бейджі shields.io — **лишити** (це не емодзі). Заголовок `# 🦀 MQTT Typed Client` → `# MQTT Typed Client`.

**engine README**: заголовки вже чисті. Інлайнові ✅/❌ — **семантичні** (не декоративні):
- feature-таблиця (рядки 63-67, колонка «default?») → замінити ✅/❌ на текст **`Yes`/`No`**
  (чистіше, без емодзі, та сама інформація).
- pro/con-булети «Use When / Avoid» (612-615, 640-644, 676-679) → замінити ✅/❌ на словесні
  під-заголовки/префікси (напр. «**Good for:**» / «**Avoid for:**») або лишити, ЯКЩО критик
  вважатиме ✅/❌ у компактних pro/con виправданими. Рішення фіналізувати на рев'ю.

**core/macros README** (короткі, 43/36 рядків): емодзі **відсутні** (перевірено) → крок для них no-op.
Увага: вони **НЕ** підключені через `include_md_transformed!` (лише `src/lib.rs` та engine), тож їхні
` ```rust `-блоки **не є доктестами** (і деякі з них невалідні як код) — **не чіпати, не підключати**.

### Крок 2. «Alternatives» (обидва крейти)
**root**: вже є секція «What mqtt-typed-client adds over rumqttc» (рядок ~223, порівняння з rumqttc —
лишити) і «See Also» (рядок ~288, лінк на rumqttc). Щоб НЕ плодити три майже-однакові секції —
додати короткий «Alternatives» **одразу після** rumqttc-порівняння (або злити з «See Also»):
1-2 речення на крейт — rumqttc (сирий контроль, те що ми обгортаємо), paho-mqtt (C-біндинг/стандарти),
ntex-mqtt (actor-фреймворк) + «бери mqtt-typed-client, коли хочеш типізовану маршрутизацію поверх
rumqttc». **БЕЗ великої конкурентної таблиці** (low-value, high-maintenance, AI-smell). Фінальне
розміщення (після Comparison vs злиття з See Also) — на рев'ю імплементації.

**engine**: додати «Alternatives» — **спершу знайти реальні** Rust-крейти топік-метчингу на crates.io
(користувач каже, є 1-2, не сильно просунуті). Чесно: «engine focuses on X (param extraction +
routing + LRU + QoS aggregation); for simple glob-style matching see <crate>». Якщо реальних
аналогів немає — так і написати («no direct equivalent; closest is the matching inside rumqttc/paho»).
**Перевірити існування кожного згаданого крейта.**

### Крок 3. Секція «Per-topic serializer override» (root)
Документувати синтаксис `#[mqtt_topic("...", serializer = JsonSerializer)]`: коли вживати
(legacy-формати, міграції), обмеження (вимикає TypedClient-генерацію; лише не-generic типи —
для generic завести type alias, як у нашому error-меседжі). Код-блок → **`ignore`** (а не `no_run`):
валідний `no_run` вимагав би оголосити кастомні derive-типи; ілюстративний фрагмент тримаємо `ignore`.
**Джерело істини — `macros/src/lib.rs:242-260`** (там уже є такий приклад як `rust,ignore`) — віддзеркалити.

### Крок 4. Секція «TLS / transport features» (root)
Документувати TLS/transport: фічі `rumqttc-use-rustls` (default, aws-lc),
`rumqttc-use-rustls-no-provider` (свій провайдер, напр. ring — обійти aws-lc),
`rumqttc-use-native-tls`, `rumqttc-websocket`, `rumqttc-proxy`. Згадати re-export
`mqtt_typed_client::{rustls, tokio_rustls}` (version-matched) + послатися на приклад `004`.
Це **Reddit-ask**. TLS-код-блок → `no_run`.
**⚠️ Доктест НЕ має парсити PEM через `rustls_pemfile`** — це лише `dev-dependency` (`Cargo.toml:84`),
недоступний у крейт-доктесті → `--doc` впав би. Показати **лише** `ClientConfig::builder()...
.with_no_client_auth()` + `Transport::tls_with_config(...)`, без `fs::read`/`BufReader`/PEM-парсингу.
Має збиратись і під MSRV 1.85.1 (CI ганяє `--all-features --doc` на всій матриці). Звірити з
`src/lib.rs:27-36` (re-export під rustls-фічами) та `examples/004_hello_world_tls.rs`.

### Крок 5b. Синхронізувати версію в install-сніпетах (виправлено після рев'ю)
**Передумова виявилась хибною:** root `Cargo.toml:12` **уже `0.2.0`**, тож README з `0.1.0`
розсинхронізований ВЖЕ зараз (не залежить від публікації; `="0.1.0"` все одно зламається після
виходу 0.2.0 — semver-несумісність). Тому bump install-сніпетів — **у цій сесії**:
- `README.md:37` `mqtt-typed-client = "0.1.0"` → `"0.2.0"`.
- `README.md:148` `version = "0.1.0"` → `"0.2.0"`.
- `src/lib.rs:328` (doc-блок `_core_only_docs`, `toml`, не доктест) `version = "0.1"` → `"0.2"`.
- **Engine лишається `0.1.0`** (незалежне версіонування — перший реліз; НЕ чіпати).
- **Дата CHANGELOG** і фінальна version-consistency-перевірка — лишаються в **Сесії C** (дата відома
  лише на момент `cargo publish`).

### Крок 5. TOC (умовно — після перевірки якорів)
⚠️ **Робити ЛИШЕ після кроку 1 (де-емодзі):** rustdoc генерує `id` заголовків із тексту, а емодзі
змінюють якір (напр. `## ✨ Key Features` дає інший id, ніж `## Key Features`). TOC будувати під
вже-очищені заголовки.
**Спершу** зібрати локально `cargo doc -p mqtt-topic-engine --features rumqttc,ntex-mqtt --no-deps`
і **подивитись у згенерованому HTML**, які `id=` rustdoc дає заголовкам крейт-доку, та чи працює
`[text](#anchor)` всередині крейт-доку на docs.rs-рендері. ЯКЩО якорі стабільні/коректні:
- **engine README (780 рядків) → додати TOC** на початку (високий пріоритет, довгий док).
- **root README (292) → TOC опційно** (GitHub сам дає TOC; docs.rs виграє). Додати, якщо якорі ок.
ЯКЩО якорі не резолвляться — TOC **не додавати** (краще без, ніж з битими лінками); зафіксувати в TODO.

---

## Поза обсягом (НЕ в цій сесії)
- **Фіналізація дати CHANGELOG + наскрізна version-consistency-перевірка → Сесія C** (дата відома
  лише на момент `cargo publish`). Сам bump install-сніпетів root до `0.2.0` РОБИМО тут (крок 5b) —
  бо README вже розсинхронізований з маніфестом.
- Перейменування папок крейтів (обговорено — лишаємо).
- Реальна підтримка generic-серіалізатора в парсері; `compile_error!` TLS.

---

## Верифікація (перед комітом)
```
cargo +nightly fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --doc          # root доктести (нові serializer/TLS блоки)
cargo test -p mqtt-topic-engine --doc    # engine доктести
cargo test -p mqtt-topic-engine --features rumqttc,ntex-mqtt --doc
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
RUSTDOCFLAGS="-D warnings" cargo doc -p mqtt-topic-engine --features rumqttc,ntex-mqtt --no-deps
```
Очікування: усі доктести зелені; doc-білд без warning; (якщо TOC) якорі ведуть куди треба.
Око: відрендерити обидва README локально (GitHub-preview/`grip` чи перегляд `target/doc`) —
переконатись, що без емодзі виглядає охайно й нічого не з'їхало.

---

## Ризики
- **Доктести нових блоків** — найімовірніше джерело поломки. Писати під реальний API, гейтити
  `no_run`/`ignore` свідомо; ганяти `--doc` локально до коміту.
- **TOC-якорі на docs.rs** ≠ GitHub — тому крок 5 умовний і перевіряється емпірично.
- **«Alternatives» фабрикація** — обов'язкова перевірка існування крейтів (search crates.io).
- **Зняття ✅/❌ в engine** — не втратити інформативність (тому → `Yes/No` у таблиці, а не просто видалити стовпець).
- README — доктести: зняття емодзі з ` ```rust `-рядків НЕ чіпати (там емодзі й нема, але пильнувати).

---

## Коміт (один, cohesive)
Заголовок-приклад: `docs: de-emoji READMEs, add alternatives + TLS/serializer sections`
Тіло: емодзі прибрано (root заголовки + engine ✅/❌→Yes/No), «Alternatives» (root+engine),
секції Per-topic serializer + TLS (root), install-сніпети root → 0.2.0 (+ `src/lib.rs:328`),
TOC (якщо якорі ок). Engine лишається 0.1.0; дата CHANGELOG — Сесія C.
Трейлер `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
Оновити чекбокси `TODO-0.2.0-release.md` (+ лишити дату CHANGELOG за Сесією C).

Workflow: цей план → критик-рев'ю плану → фікси → імплементація → критик-рев'ю імплементації →
фікси → коміт. Див. [[workflow-plan-critique-scheme]], [[mqtt-typed-client-0-2-0-release]].
