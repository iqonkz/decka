# BLOCKERS

## M7 (кроме T7.4) — нет окружения Android

- Найдено 2026-11-14: `JAVA_HOME` задан (`C:\Program Files\Microsoft\jdk-21.0.12.101-hotspot\`, JDK 21), `java` в PATH есть.
- Отсутствуют: Android Studio (`C:\Program Files\Android\Android Studio` нет), Android SDK (`%LOCALAPPDATA%\Android\Sdk` нет), переменная `ANDROID_HOME` пуста, `adb` и `sdkmanager` в PATH не найдены.
- Затронуты задачи: T7.1, T7.2, T7.3 (сборка APK), T7.5 (клиент по USB и проверки на телефоне).
- Сделано из M7: только T7.4 (Rust, без Android).
- Что нужно от владельца: установить Android Studio (SDK + platform-tools), задать `ANDROID_HOME`; затем вернуться к T7.1.
