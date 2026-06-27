# PLAN — Сесія C: Реліз 0.2.0 (публікація на crates.io)

**Мета:** опублікувати 0.2.0. Це **незворотний** крок (crates.io не можна видалити, лише `yank`).
Тому: спершу повна підготовка + dry-run, і лише потім реальний `cargo publish`.
**`cargo publish` виконує КОРИСТУВАЧ** (Artem) — я готую все до нього й перевіряю; сам не публікую.

> **⚠️ ОНОВЛЕННЯ (Сесія D): крейт `doc-macros` ВИДАЛЕНО до публікації.** Публікується **4 крейти**,
> не 5: engine → core → macros → root. Усі згадки `doc-macros` нижче — застарілі; актуальні дані —
> у `TODO-0.2.0-release.md` (банер Сесії D). Деталі рішення там само.

**Гілка:** `main`. Версії незалежні: **root/core/macros = 0.2.0**, **engine = 0.1.0**
(перший реліз). Див. [[mqtt-typed-client-0-2-0-release]].

---

## Порядок публікації (топологічний — КРИТИЧНО)

Залежності (path+version): `engine` ← `core` ← `macros`; root ← core+macros.
Тому публікувати **строго**:

1. `mqtt-topic-engine` (0.1.0) — лист, без внутрішніх залежностей
2. `mqtt-typed-client-core` (0.2.0) — залежить від engine
3. `mqtt-typed-client-macros` (0.2.0) — залежить від core
4. `mqtt-typed-client` (0.2.0, root) — залежить від core+macros

**Між кроками — пауза на пропагацію індексу crates.io** (зазвичай секунди–хвилина). `cargo publish`
дедалі частіше сам чекає, але закладати паузу/повтор варто: наступний крейт не збереться, доки його
залежність не з'явиться в індексі.

⚠️ **Гача dry-run:** `cargo publish --dry-run` для крейта з path+version-залежністю на ще-НЕ-опублі­ковану
версію може впасти на verify-білді (cargo для запакованого крейта бачить лише `version`, шлях стирається,
а версії в реєстрі ще немає). Тому:
- **engine** — повний `--dry-run` працює (лист).
- Решта — або (а) робити `--dry-run` КОЖНОГО **безпосередньо перед** його реальним publish (коли
  залежності вже в індексі), або (б) до публікації перевіряти лише `cargo package --list`
  (пакування) + `cargo build`. План використовує **інкрементальний** підхід: dry-run → publish по
  одному, зверху вниз.

---

## Крок 0. Pre-flight (до будь-чого)
- `git status` чисте; `main` == `origin/main`; **CI зелений** на останньому коміті (включно з
  щойно доданими per-crate CHANGELOG — їх ще треба запушити).
- **Перевірити доступність НОВОГО імені на crates.io:** `mqtt-topic-engine` — чи вільне / чи нашого
  власника. (root/core/macros вже наші з 0.1.0.)
  Якщо `mqtt-topic-engine` зайнятий кимось — СТОП, перейменування (окреме рішення).
- `cargo login` виконаний (токен crates.io). Це робить користувач.
- Версії узгоджені (перевірено: root/core/macros=0.2.0, engine=0.1.0; install-сніпети
  root README=0.2.0, engine README=0.1.0). Усі `version=` у path-deps звірені дослівно
  (core→engine=0.1.0; macros→core=0.2.0; root→core/macros=0.2.0).
- **Lockfile для відтворюваності:** `Cargo.lock` gitignored — згенерувати `cargo generate-lockfile`
  і **далі весь Крок 2 і Крок 3 ганяти з `--locked`**, щоб verify-білд публікації йшов проти ТОГО
  САМОГО дерева залежностей, що й тести. (Комітити lock не обов'язково — library-крейт; головне —
  стабільний стан від тестів до публікації в межах сесії.)
- **Фікс гігієни (W1):** `core/Cargo.toml` має dev-dep `mqtt-typed-client-macros = { path = "../macros" }`
  БЕЗ `version` (на відміну від решти внутрішніх deps). publish не блокує (cargo стрипає path-dev-deps
  без version), але додати `version = "0.2.0"` для консистентності (як у `macros`→core). Дешево.
- **`publish = false` має бути ВІДСУТНІЙ** у всіх 4 (перевірено — його ніде немає; усі 4 публікуються).
- **🔴 Верифікація tarball на витік (критично після інциденту ad8e3d5):**
  `cargo package --list --allow-dirty -p mqtt-typed-client` і ВРУЧНУ переконатись, що `dev/`
  (а надто `dev/certs/key.pem`) та `.github/` у списку **НЕМАЄ** (їх ріже `exclude`).

## Крок 1. Виставити дати CHANGELOG (атомарно, 5 файлів)
Замінити `- TBD` на дату релізу (формат `YYYY-MM-DD`, реальний день публікації) у:
`CHANGELOG.md` (`[0.2.0]`), `mqtt-topic-engine/CHANGELOG.md` (`[0.1.0]`),
`core/CHANGELOG.md` (`[0.2.0]`), `macros/CHANGELOG.md` (`[0.2.0]`).
`[Unreleased]`-секції лишити порожніми (стандарт).
Коміт `chore(release): set 0.2.0 changelog date` → push → дочекатись зеленого CI.
(Дата невідома до цього дня — тому крок саме тут, не раніше.)

## Крок 2. Фінальна локальна верифікація (як CI, перед публікацією) — усе з `--locked`
```
cargo +nightly fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo build --locked --all-features
cargo test --locked --workspace --lib
cargo test --locked --all-features --lib && cargo test --locked --all-features --doc
cargo test --locked -p mqtt-topic-engine --features rumqttc,ntex-mqtt --lib
cargo test --locked -p mqtt-topic-engine --doc
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --all-features
```
+ (якщо є піднятий брокер) інтеграційні тести з `MQTT_REQUIRE_BROKER=1`.

**🔴 ОБОВ'ЯЗКОВО ДО будь-якої публікації — для ВСІХ 4 крейтів** (зловити проблеми пакування/
компіляції ДО незворотного кроку):
```
cargo package --list --locked -p <crate>     # що саме потрапить у tarball
cargo build --locked -p <crate>              # компілюється
```
Повний `cargo publish --dry-run --locked` тут пройде лише для `engine` (лист); для решти
dry-run чекаємо до Кроку 3 (їхні залежності ще не в реєстрі — це очікувано, див. «гача»).

## Крок 3. Публікація (інкрементально, по одному; виконує користувач)
Для кожного крейта в порядку 1→4:
```
cargo publish --dry-run --locked -p <crate>   # коли залежності вже в індексі
cargo publish --locked -p <crate>             # реальний публіш
# дочекатись появи в індексі перед наступним (cargo зазвичай чекає сам)
```
Послідовність:
1. `cargo publish -p mqtt-topic-engine`
2. `cargo publish -p mqtt-typed-client-core`
3. `cargo publish -p mqtt-typed-client-macros`
4. `cargo publish -p mqtt-typed-client`
**Якщо будь-який крок упав — СТОП, розібратись** (опубліковані крейти вже незворотні; не можна
просто «відкотити»).

## Крок 4. Тег + післяреліз
- `git tag -a v0.2.0 -m "Release 0.2.0"` → `git push origin v0.2.0`.
- Перевірити docs.rs-білд кожного крейта (особливо engine з `features=["rumqttc","ntex-mqtt"]` —
  щоб paho не зачепило) та сторінки crates.io (README, бейджі, лінки).
- (Опційно) GitHub Release з нотатками з CHANGELOG.

---

## Поза обсягом / окремі рішення
- **`yank mqtt-typed-client@0.1.0`** через витеклий localhost dev-ключ — НЕ обов'язково (ключ
  throwaway, файли все одно лишаться). Рішення користувача; за замовчуванням — не робимо.
- `release.yml.disabled` — у репо є **вимкнений** release-workflow; НЕ покладатись на нього,
  публікуємо вручну. (Можна окремо вирішити, чи вмикати автоматизацію згодом.)
- `[Unreleased]`-секції лишаються для майбутнього.

---

## Ризики
- **Незворотність publish** — головний. Тому інкрементально + dry-run перед кожним + СТОП на помилці.
- **Часткове падіння (напр. 3 з 5 опубліковано):** опубліковані крейти НЕЗВОРОТНІ. Якщо після цього
  знайдеться дефект у вже-опублікованому — виправлення піде лише з **новим** номером версії
  (напр. core 0.2.0 вже на crates.io → фікс = 0.2.1), не перепублікацією. Тому Крок 2 (повна
  верифікація + package/build усіх 4) — обов'язковий ДО першого publish.
- **Compare-лінки CHANGELOG:** тег один — `v0.2.0`. Перевірити, що engine CHANGELOG
  (виходить 0.1.0) НЕ містить битих `compare/v0.1.0...`-лінків на неіснуючі теги (наразі не містить).
- **Зайняте ім'я `mqtt-topic-engine`** — перевірити в Кроці 0 ДО публікації.
- **Пропагація індексу** — між крейтами; не публікувати наступний, доки попередній не видно в реєстрі.
- **Path+version неузгодженість** — кожна `version` у path-deps має точно відповідати опублікованій
  (engine = 0.1.0; core = 0.2.0). Перевірено, але звірити ще раз у Кроці 0.
- **docs.rs після релізу** — engine не має тягнути paho; root/core all-features безпечні (перевірено в А).

---

## Коміти цієї сесії
- `chore(release): set 0.2.0 changelog date` (Крок 1).
- (тег `v0.2.0` — не коміт, окремо після публікації.)
Трейлер `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
Оновити фінальний чеклист у `TODO-0.2.0-release.md`.

Workflow: цей план → критик-рев'ю плану → фікси → (виконання Кроків 0-2 мною; publish — користувачем)
→ тег. Див. [[workflow-plan-critique-scheme]].
