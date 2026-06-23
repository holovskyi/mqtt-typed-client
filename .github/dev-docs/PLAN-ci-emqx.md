# PLAN: CI з EMQX — щоб живі тести РЕАЛЬНО виконувались, а не деградували

Мета: інтеграційні тести в CI справді ганяються проти брокера; за відсутності брокера в CI —
**падіння**, а не тихий зелений. Локально (без брокера) — зберегти м'яку деградацію (DX).
Заразом — полагодити явні баги наявного `ci.yml` і додати покриття engine.

## Контекст (звірено з кодом)
- `.github/workflows/ci.yml` ВЖЕ існує: job'и `test` (matrix stable/beta/nightly/MSRV 1.85.1),
  `integration-test` (services: emqx 1883/8883), `docs`, `security` (cargo-audit + cargo-deny),
  `examples` (services: emqx 1883).
- `dev/docker-compose.yml`: EMQX `latest`, контейнер `mqtt-client-dev-broker`, порти 1883/8883/8083/…
- Тести хардкодять `mqtt://localhost:1883` (`tests/serializers_integration.rs:93,218`;
  `tests/serializer_macro_integration.rs:36`) і НЕ читають env. На `Err` зʼєднання — деградують:
  `serializers_integration` друкує "instantiation successful" і проходить; `serializer_macro_integration`
  робить `return` до round-trip (але ПІСЛЯ конекту — строгий `panic!`).
- `examples/shared/config.rs:24` читає **`MQTT_BROKER`** (не `MQTT_BROKER_URL`), дефолт localhost.
- ci.yml `integration-test` ганяє лише `serializers_integration` + неіснуючий `integration_tls`
  (замаскований `|| echo`); НЕ ганяє `serializer_macro_integration`. Задає `MQTT_BROKER_URL`
  (тести ігнорують) — мертва змінна.
- `test` job: `cargo test --all-features` з кореня → лише root-пакет; engine (86 unit + doctests)
  у CI не виконуються.
- paho: `--all-features` НА КОРЕНІ не тягне paho (paho — опційна dep лише `mqtt-topic-engine`).
  Небезпека лише якщо додати `--workspace --all-features` або `-p mqtt-topic-engine --all-features`
  (тоді paho → нативна C-lib + CMake). Уникати таких комбінацій.
- protobuf: `core` feature `protobuf` = `dep:prost` (runtime, БЕЗ protoc). `--all-features` на корені
  вмикає protobuf — звірити, що збирається без protoc (ймовірно так; перевірити локально).

## Зміни

### 1. Тест-код: env-гейт «вимагати брокер» (ключове)
Додати дешевий хелпер «брокер обовʼязковий?»: `std::env::var("MQTT_REQUIRE_BROKER").is_ok()`.
- `tests/serializers_integration.rs` — в `Err`-гілках `test_serializer_integration` і
  `test_connection_only`: якщо `MQTT_REQUIRE_BROKER` встановлено → `panic!("broker required but
  connect failed: {e}")`; інакше — поточна мʼяка деградація (`println!`).
- `tests/serializer_macro_integration.rs` — в `Err`-гілці (рядок ~41): так само (panic під гейтом,
  інакше `return`).
- (Опційно/узгодити) читати URL з env: `MQTT_BROKER_URL` з дефолтом `mqtt://localhost:1883`, щоб CI
  міг конфігурувати. Якщо не робимо — прибрати мертвий `MQTT_BROKER_URL` з ci.yml, щоб не вводити в
  оману. РІШЕННЯ за замовчуванням: читати env (узгодити назву — `MQTT_BROKER_URL`), бо інакше CI-env
  бреше.

### 2. ci.yml — `integration-test`
- Виставити `MQTT_REQUIRE_BROKER: "1"` (і `MQTT_BROKER_URL: mqtt://localhost:1883` лише якщо п.1
  читає його) — тоді тихе виродження стає падінням.
- Ганяти ОБИДВА інтеграційні тести: додати
  `cargo test --test serializer_macro_integration --features macros,bincode,json -- --test-threads=1`.
  (`serializer_macro_integration` має required-features `macros,bincode,json` — без `--all-features`,
  щоб не тягнути зайве/protobuf.)
- Прибрати мертвий блок `integration_tls` (`|| echo "not yet implemented"`).
- `serializers_integration` лишити, але обрати фічі свідомо: він тестує всі серіалізатори →
  потребує `bincode,json,messagepack,cbor,postcard,ron,flexbuffers,protobuf`. Якщо `--all-features`
  на корені збирається без protoc — лишити `--all-features`; інакше перелічити фічі без protobuf і
  залогувати, що protobuf-гілка пропущена.

### 3. ci.yml — покриття engine у job `test`
Додати крок, що реально ганяє engine (БЕЗ paho, щоб уникнути нативного лінку):
- `cargo test -p mqtt-topic-engine` (default: router+lru-cache)
- `cargo test -p mqtt-topic-engine --features router,lru-cache,rumqttc,ntex-mqtt` (pure-Rust interop)
- `cargo test -p mqtt-topic-engine --no-default-features`
Альтернатива: `cargo test --workspace` БЕЗ `--all-features` (дефолти не вмикають paho) — простіше,
покриває всі члени. РІШЕННЯ: `--workspace` (default features) + окремо engine no-default-features.
НЕ використовувати `--workspace --all-features` (paho).

### 4. ci.yml — дрібні узгодження
- `examples` job: env має бути `MQTT_BROKER` (не `MQTT_BROKER_URL`), бо `shared/config.rs` читає
  `MQTT_BROKER`. Інакше приклади мовчки беруть дефолт (працює, але змінна-привид).
- Перевірити, що `actions/cache@v3` не застарів критично (v4 доступний) — НЕ блокер, можна лишити.

## Поза скоупом
- TLS-інтеграційний тест (`integration_tls`) — окремо; зараз лише прибрати мертве посилання.
- docs.rs тех-борг (відносні лінки, metadata) — окремий пункт TODO.
- Перехід cache@v3→v4, MSRV-узгодження engine(1.82)/root(1.85.1) — не чіпаємо в цьому блоці.

## Відкриті питання для критика
1. Чи `--all-features` на КОРЕНІ реально збирається в CI без protoc (protobuf=prost-runtime)?
   Якщо ні — як гейтити protobuf-крок.
2. Чи правильний підхід `MQTT_REQUIRE_BROKER` (panic під гейтом) — чи краще завжди-строго в CI через
   окремий required-тест? Чи не зламає це локальний `cargo test` (де гейта немає → деградація лишається)?
3. GitHub `services:` + job на runner (не в контейнері): порт 1883 на `localhost` доступний — підтвердити.
   Чи треба чекати healthcheck перед тестами (EMQX може стартувати повільно → перші конекти падуть під
   гейтом)? Можливо потрібен retry/wait-крок.
4. Чи `cargo test --workspace` (default) не тягне нічого нативного (paho off) — підтвердити.
5. Чи `serializer_macro_integration` з `--features macros,bincode,json` коректно компілюється в CI
   (required-features vs явні --features на root-пакеті).
6. Чи healthcheck EMQX (`emqx ping`) достатній сигнал готовності listener'а 1883 (ping ≠ listener up).

## Перевірка
- Локально (брокер піднятий): `MQTT_REQUIRE_BROKER=1 cargo test --test serializers_integration
  --all-features -- --test-threads=1` — проходить; без брокера з гейтом — ПАДАЄ; без гейта — деградує.
- `cargo test --test serializer_macro_integration --features macros,bincode,json` — round-trip ок.
- `cargo test --workspace` (default) — зелене, нічого нативного.
- `yamllint`/візуальна перевірка ci.yml; (за можливості) `act` або хоча б `--dry`. 
- Сам CI підтвердиться лише після push — тож максимум локальної впевненості перед комітом.

## Коміт
Один коміт (hook). Повідомлення — секція про enforcement живих тестів + фікси CI. Co-Authored-By.
