## TODO

- [x] Last will message
- [ ] Protocol compression. Compositional approach - separate serialization and compression. Adaptive mechanism (based on message type and size)
- [ ] mqtt_typed_client::client::async_client::MqttClient
    impl<F> MqttClient<F>
    async fn run(mut event_loop: EventLoop, subscription_manager: SubscriptionManagerHandler<Bytes>)
    При певній кількості помилок, ми виходимо з циклу. Але можливо треба зробити передачу помилки на інші рівні, спідписникам та пудлішерам?
- [ ] В опції макроса додати notypedclient та nolastwill
- [x] розібратися в типах лицензії
- [ ] Чи бачить клієнт згенерованний тип на кшталт SensorReadingSubscriptionBuilderExt? Чи варто скорочувати назву?
- [ ] автогенерація client_id
- [ ] Дефолтний серіалізатор Bincode
- [ ] let mut subscriber = topic_client.subscribe().await?; - subscriber.topic() and .pattern() methods
- [ ] Проблема з кріптік помилкою, коли ми оголошуємо структуру для payload, але забуваємо виставити в ній derive для того щоб потім кастомний серіалізатор міг працювати з структурою. Наприклад для BincodeSerializer треба Encode та Decode. Але маємо помилку в якій складно визначити що саме потрібно
- [ ] Додати підтвердженну підписку. Зараз subscribe не аналізує результату підписки. Треба додати в event_loop механізм який по Outgoing(Subscribe(1)) отримує id пакета для підписки. А поттім чекає Incoming(SubAck(SubAck { pkid: 1, return_codes: [Success(AtLeastOnce)] })), з результатом підписки, та надає цей результат користувачу. Але тут є проблеа того що зараз Outgoing(Subscribe()), має тількі pkid, без конкретних фільтрів, тож нам треба форкнути rumqttc та у src/lib.rs змінити Outgoing::Subscribe(u16) => outgoing::Subscribe(Subscribe). 
- [x] Спробувати як працює коли  нас два модуля, та макро в одному модулі, 
    а використання в другому
- [x] А якщо в макросі генерувати ext trait для MqttClient
    Щоб працювати з конкретними типами повідомленнь замість 
    SensorMessage::publish(&client, sensor_id, &_test_data).await
    Чи це реально? Я тут бачу виклик що може бути багато типів на кшталт SensorMessage, і чи компілятор зможе вивести і знайти необхідний trait?
- [x] Update topic for subscription. With same wildcards and param names
let mut subscriber = TemperatureSensor::subscribe_with_pattern(&client, "data/{building}/{floor}/t/{sensor_id}").await?;
- [x] Update topic for publisher


## Publish Checklist

- [x] Remove all TODOs from public API
- [ ] Examples Simple, Middle, Avanced
- [ ] Write README.md with clear getting started guide
- [ ] Fix Cargo.toml (repository, keywords, description)
- [ ] Comprehensive testing - cover main scenarios
- [ ] API documentation - all public elements documented

## Should-have (highly desirable)

- [ ] GitHub CI/CD - automated testing
- [ ] Examples cleanup - use builder pattern for parameters
- [ ] Error handling improvements - custom error types
- [ ] Benchmarks - for performance-critical parts
- [ ] CHANGELOG.md

## Nice-to-have

- [ ] Integration tests with real MQTT broker
- [ ] Docs.rs compatibility check
- [ ] Minimal Supported Rust Version (MSRV)