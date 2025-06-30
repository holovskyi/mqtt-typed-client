## TODO

- [ ] Last will message
- [ ] Protocol compression. Compositional approach - separate serialization and compression. Adaptive mechanism (based on message type and size)
- [ ] mqtt_typed_client::client::async_client::MqttClient
    impl<F> MqttClient<F>
    async fn run(mut event_loop: EventLoop, subscription_manager: SubscriptionManagerHandler<Bytes>)
    При певній кількості помилок, ми виходимо з циклу. Але можливо треба зробити передачу помилки на інші рівні, спідписникам та пудлішерам?
- [ ] В опції макроса додати notypedclient та nolastwill
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