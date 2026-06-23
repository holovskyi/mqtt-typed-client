# Completed Tasks

## Features
- [x] Examples Last Will message and clean Retaition
- [x] Додати серіалізатори json, та serde related.
- [x] Обробляти пусте повідомлення яке скидає retain. Зараз повертається помилка десеріалізації.
- [x] publish_retained(data) як shortcut для with_retain(true).publish(data) та publish_temporary(data) як shortcut для with_retain(false).publish(data)? Перший точно можна. А другий варіант навіщо не зрозуміло.
- [x] Додати серіалізатори дефолтні
- [x] Додати умовну компіляцію для підключення серіалізаторів
- [x] Last will message
- [x] Спробувати як працює коли нас два модуля, та макро в одному модулі, а використання в другому
- [x] А якщо в макросі генерувати ext trait для MqttClient
- [x] Update topic for subscription. With same wildcards and param names
- [x] Update topic for publisher

- [x] You're using build.rs for generating markdown documentation in your repository, that is not what it's for, and considering the supply chain security implications of build.rs, if you can avoid having one, you should. And you can definitely avoid it when it is just for generating your readme. You can use Make or Just etc.