# QUICKREF.md — шпаргалка исполнителя (читать вместо полных документов)

Цель файла — экономия токенов: здесь всё, что нужно в каждой задаче. Полные документы открывай только по ссылке из задачи и только указанный раздел.

## Что строим
Телефон (Android 10, альбомная ориентация) = панель 5×3 для Windows-ПК. ПК исполняет действия; телефон шлёт только `actionId`.

## Жёсткие правила
- Телефон NEVER не передаёт команду/путь/аргументы/клавиши/URL. Только `{"t":"press","id":n,"actionId":"..."}`.
- Код из сторонних проектов не копировать (Desko и Xenon — некоммерческие лицензии, OpenDeck — GPL).
- Оболочка — PowerShell 5.1: `;` вместо `&&`, `curl.exe`, JSON только из файлов.
- Версии не менять. API расходится → docs.rs / README установленной версии. Тупик → `docs/BLOCKERS.md` и стоп.
- UI на английском. `/docs` не править (кроме BLOCKERS, MANUAL_CHECKS, RECIPES, TESTING).
- Коммит после каждой задачи: `T<номер>: <название>`.

## Карта репозитория
```
shared/   типы TS, schemas/, fixtures/, validate.ts, pairUrl.ts
client/   React+Vite: app/ net/ store/ screens/ deck/ display/ platform/ ; android/
agent/    src/ (окно настроек)  core/ (crate decka-core + bin decka-headless)  src-tauri/ (crate decka-agent)
tools/    ws-smoke.mjs, make-icon.ps1
```

## Константы
| Что | Значение |
|---|---|
| Порт | 9247 (тесты: 19247) |
| WS | `ws://host:port/ws`, кадр ≤ 4096 байт, рукопожатие ≤ 5 с |
| HTTP тело | ≤ 16384 байт |
| Heartbeat | ping 10 с; клиент рвёт через 25 с тишины, сервер — 30 с |
| Переподключение | 1, 2, 4, 8, 15, 15… с; кандидат ждёт 1,5 с |
| Код сопряжения | 8 символов `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`, TTL 120 с, 5 попыток |
| Токен устройства | 43 символа base64url; на ПК хранится SHA-256 |
| apiToken | `dk_` + 43 символа, заголовок `Authorization: Bearer` |
| Лимиты | press 20/с на сокет; HTTP 60/мин на IP; pair 10/мин на IP |
| Id | `^[a-z0-9_-]{1,64}$`; VarName `^[a-z0-9_.-]{1,64}$` |
| Секции | `layout`, `sys`, `statuses`, `notifications` (≤ 20), `vars` |
| Сетка | 5 столбцов × 3 строки |
| Каталог конфигурации | `%APPDATA%\Decka` или env `DECKA_CONFIG_DIR` |
| Close-коды WS | 4401 unauthorized, 4403 revoked, 4426 version, 4429 rate, 1009 size, 1011 lagged |

## Сообщения (полностью — PROTOCOL.md §3)
- C→S: `pair{v,code,client}`, `hello{v,token,client}`, `press{id,actionId}`, `notif_dismiss{notifId}`, `ping{ts}`
- S→C: `paired{deviceId,token}`, `welcome{v,agent,deviceId,rev,state}`, `patch{rev,section,value}`, `press_result{id,ok,error?}`, `pong{ts}`, `error{code,message,fatal,supported?}`
- HTTP: `GET /api/health`; `POST /api/notify`; `POST /api/status`; `DELETE /api/status/{id}`; `PUT|DELETE /api/vars/{name}`

## Действия (DATA_MODEL.md §2)
`hotkey{keys[]}`, `media{op}`, `launch{program,args?,cwd?,hidden?}`, `open_url{url}`, `system{op: lock|sleep}`; общие поля `name`, `toggleVar?`. Запуск — `Command` без shell, дочерние процессы с `CREATE_NO_WINDOW`.

## Элементы раскладки (DATA_MODEL.md §3)
`button` (1×1; `actionId` xor `navigate`; `confirm?`; `bind?`), `clock`, `sys`, `statuses`, `status`, `notifications`. Иконки: `lucide:<имя>` из набора 60 штук или `emoji:<символ>`.

## Где что искать
| Нужно | Документ |
|---|---|
| Точные типы, схемы, примеры, коды ошибок, IPC | PROTOCOL.md §3–§9 |
| Форматы файлов, валидация, значения по умолчанию, поведение виджетов | DATA_MODEL.md |
| Модули, API `Agent`, потоки данных, безопасность, цвета, версии пакетов | ARCHITECTURE.md §4–§8 |
| Почему так, а не иначе | DECISIONS.md |
| Макросы проверки H-BUILD / H-START / H-STOP / H-TOKEN / H-API | TASKS.md §0 |

## Команды
```
npm run typecheck          npm test                    cargo test -p decka-core
npm run build -w client    npm run build:ui            cargo build -p decka-agent
npm run tauri dev          npm run release:agent       npm run release:apk
node tools/ws-smoke.mjs pair|state|press|watch|raw <host:port> ...
```
