import type { TranslationKey } from "@decka/shared";

export type CommandBadge = "no_ai" | "ai" | "screen" | "confirm";

export interface CommandExample {
  tool: string;
  ru: string;
  en: string;
  badges: CommandBadge[];
}

export interface CommandGroup {
  id: string;
  titleKey: TranslationKey;
  descKey: TranslationKey;
  examples: CommandExample[];
}

/**
 * The white list of voice tools: the same as `TOOLS` in `agent/core/src/voice/tools.rs` (a test reads that file).
 * Tested to ensure every tool belongs to a group and has at least one example.
 */
export const WHITELISTED_VOICE_TOOLS = [
  "press_button",
  "run_action",
  "open_app",
  "switch_app",
  "close_app",
  "window_other_monitor",
  "which_monitor",
  "volume",
  "media",
  "go_page",
  "web_search",
  "open_site",
  "type_text",
  "dictate",
  "timer",
  "system",
  "text_transform",
  "screen_ask",
  "remind",
  "reminders_show",
  "list_add",
  "list_show",
  "list_remove",
  "day_brief",
  "calendar",
  "memory_add",
  "memory_remove",
  "calc",
  "ask",
  "lyrics_window",
  "status",
  "reply_action",
  "find_on_screen",
  "ui_act",
  "guide",
  "guide_record",
  "guide_control",
  "read_aloud",
  "mode_run",
  "mode_save",
  "screenshot",
  "add_button",
  "system_theme",
  "notes_record",
  "memory_top",
  "ha",
] as const;

export const VOICE_COMMAND_GROUPS: CommandGroup[] = [
  {
    id: "music_sound",
    titleKey: "m29.cmd_g1_title",
    descKey: "m29.cmd_g1_desc",
    examples: [
      { tool: "media", ru: "Поставь музыку на паузу", en: "Pause the music", badges: ["no_ai"] },
      { tool: "media", ru: "Включи следующий трек", en: "Next track", badges: ["no_ai"] },
      { tool: "volume", ru: "Сделай потише на 15%", en: "Turn volume down by 15%", badges: ["no_ai"] },
      { tool: "volume", ru: "Выключи звук", en: "Mute audio", badges: ["no_ai"] },
      { tool: "lyrics_window", ru: "Покажи текст песни", en: "Show song lyrics", badges: ["no_ai"] },
    ],
  },
  {
    id: "windows_apps",
    titleKey: "m29.cmd_g2_title",
    descKey: "m29.cmd_g2_desc",
    examples: [
      { tool: "open_app", ru: "Открой Калькулятор", en: "Open Calculator", badges: ["no_ai"] },
      { tool: "switch_app", ru: "Переключись на браузер", en: "Switch to browser", badges: ["no_ai"] },
      { tool: "close_app", ru: "Закрой Блокнот", en: "Close Notepad", badges: ["no_ai", "confirm"] },
      { tool: "window_other_monitor", ru: "Перенеси окно на другой монитор", en: "Move window to other monitor", badges: ["no_ai"] },
      { tool: "which_monitor", ru: "На каком мониторе это окно?", en: "Which monitor is this window on?", badges: ["no_ai"] },
    ],
  },
  {
    id: "text_work",
    titleKey: "m29.cmd_g3_title",
    descKey: "m29.cmd_g3_desc",
    examples: [
      { tool: "text_transform", ru: "Переведи выделенный текст на английский", en: "Translate selected text to English", badges: ["ai"] },
      { tool: "text_transform", ru: "Исправь ошибки в тексте", en: "Fix grammar in the text", badges: ["ai"] },
      { tool: "reply_action", ru: "Напиши короткий вежливый ответ", en: "Write a short polite reply", badges: ["ai"] },
      { tool: "reply_action", ru: "Вставь это", en: "Insert it", badges: ["no_ai"] },
    ],
  },
  {
    id: "dictation",
    titleKey: "m29.cmd_g4_title",
    descKey: "m29.cmd_g4_desc",
    examples: [
      { tool: "dictate", ru: "Продиктуй: встреча переносится на завтра", en: "Dictate: the meeting is moved to tomorrow", badges: ["no_ai"] },
      { tool: "type_text", ru: "Напечатай привет всем", en: "Type hello everyone", badges: ["no_ai"] },
      { tool: "dictate", ru: "Двойной Ctrl — диктовка прямо в курсор", en: "Double Ctrl — dictate right at the cursor", badges: ["no_ai"] },
      { tool: "type_text", ru: "Введи адрес почты", en: "Type my email address", badges: ["no_ai"] },
    ],
  },
  {
    id: "screen",
    titleKey: "m29.cmd_g5_title",
    descKey: "m29.cmd_g5_desc",
    examples: [
      { tool: "screen_ask", ru: "Что сейчас на экране?", en: "What is on my screen?", badges: ["ai", "screen"] },
      { tool: "find_on_screen", ru: "Где кнопка «Сохранить»?", en: "Where is the Save button?", badges: ["ai", "screen"] },
      { tool: "ui_act", ru: "Нажми на кнопку «Отправить»", en: "Click the Send button", badges: ["ai", "screen"] },
      { tool: "guide", ru: "Покажи по шагам, как экспортировать файл", en: "Guide me step by step to export a file", badges: ["ai", "screen"] },
      { tool: "guide_record", ru: "Запиши этот сценарий как гид", en: "Record this flow as a guide", badges: ["ai", "screen"] },
      { tool: "guide_control", ru: "Дальше", en: "Next", badges: ["no_ai"] },
    ],
  },
  {
    id: "reminders_lists",
    titleKey: "m29.cmd_g6_title",
    descKey: "m29.cmd_g6_desc",
    examples: [
      { tool: "timer", ru: "Поставь таймер на 10 минут", en: "Set a timer for 10 minutes", badges: ["no_ai"] },
      { tool: "remind", ru: "Напомни позвонить в 18:00", en: "Remind me to call at 6 PM", badges: ["no_ai"] },
      { tool: "reminders_show", ru: "Какие у меня напоминания?", en: "What are my reminders?", badges: ["no_ai"] },
      { tool: "list_add", ru: "Добавь молоко в список покупок", en: "Add milk to shopping list", badges: ["no_ai"] },
      { tool: "list_show", ru: "Покажи список дел", en: "Show my to-do list", badges: ["no_ai"] },
      { tool: "list_remove", ru: "Убери молоко из списка", en: "Remove milk from the list", badges: ["no_ai"] },
      { tool: "day_brief", ru: "Сводка дня", en: "Daily brief", badges: ["ai"] },
      { tool: "day_brief", ru: "Сводка дня", en: "Daily brief", badges: ["ai"] },
      { tool: "calendar", ru: "Что у меня сегодня по календарю?", en: "What is on my calendar today?", badges: ["no_ai"] },
      { tool: "calendar", ru: "Что у меня завтра?", en: "What is on my calendar tomorrow?", badges: ["no_ai"] },
      { tool: "calendar", ru: "Когда следующая встреча?", en: "When is my next meeting?", badges: ["no_ai"] },
      { tool: "memory_add", ru: "Запомни, что код от двери 4815", en: "Remember that the door code is 4815", badges: ["ai"] },
      { tool: "memory_remove", ru: "Забудь про код от двери", en: "Forget the door code", badges: ["ai"] },
    ],
  },
  {
    id: "deck_widgets",
    titleKey: "m29.cmd_g7_title",
    descKey: "m29.cmd_g7_desc",
    examples: [
      { tool: "status", ru: "Какая погода?", en: "What is the weather?", badges: ["no_ai"] },
      { tool: "status", ru: "Сколько заряда и свободной ОЗУ?", en: "How much battery and free RAM?", badges: ["no_ai"] },
      { tool: "calc", ru: "Сколько будет 18% от 2400?", en: "What is 18% of 2400?", badges: ["no_ai"] },
      { tool: "ask", ru: "Погода в Токио сейчас", en: "Weather in Tokyo right now", badges: ["ai"] },
      { tool: "web_search", ru: "Найди в интернете рецепт пасты", en: "Search the web for pasta recipe", badges: ["no_ai"] },
      { tool: "open_site", ru: "Открой github.com", en: "Open github.com", badges: ["no_ai"] },
    ],
  },
  {
    id: "system",
    titleKey: "m29.cmd_g8_title",
    descKey: "m29.cmd_g8_desc",
    examples: [
      { tool: "system_theme", ru: "Включи тёмную тему Windows", en: "Turn on Windows dark theme", badges: ["no_ai"] },
      { tool: "system_theme", ru: "Включи светлую тему Windows", en: "Turn on Windows light theme", badges: ["no_ai"] },
      { tool: "screenshot", ru: "Сделай скриншот", en: "Take a screenshot", badges: ["no_ai"] },
      { tool: "notes_record", ru: "Начни заметку", en: "Start a meeting note", badges: ["no_ai"] },
      { tool: "memory_top", ru: "Что ест память?", en: "What is eating the memory?", badges: ["no_ai"] },
      { tool: "ha", ru: "Включи свет в спальне", en: "Turn on the bedroom light", badges: ["no_ai"] },
      { tool: "system", ru: "Заблокируй компьютер", en: "Lock the computer", badges: ["no_ai", "confirm"] },
    ],
  },
  {
    id: "work_modes",
    titleKey: "m29.cmd_g9_title",
    descKey: "m29.cmd_g9_desc",
    examples: [
      { tool: "mode_run", ru: "Включи режим работы «Стрим»", en: "Start work mode Streaming", badges: ["no_ai"] },
      { tool: "mode_run", ru: "Запусти рабочий режим «Код»", en: "Run Coding work mode", badges: ["no_ai"] },
      { tool: "mode_save", ru: "Сохрани текущие окна как режим работы", en: "Save current windows as a work mode", badges: ["no_ai"] },
      { tool: "mode_save", ru: "Запомни этот набор программ", en: "Save this app layout", badges: ["no_ai"] },
    ],
  },
  {
    id: "buttons_actions",
    titleKey: "m29.cmd_g10_title",
    descKey: "m29.cmd_g10_desc",
    examples: [
      { tool: "press_button", ru: "Нажми кнопку «Микрофон»", en: "Press the Mute button", badges: ["no_ai"] },
      { tool: "run_action", ru: "Выполни действие «Запись OBS»", en: "Run action Start Recording", badges: ["no_ai"] },
      { tool: "go_page", ru: "Открой страницу «Стрим»", en: "Go to Streaming page", badges: ["no_ai"] },
      { tool: "add_button", ru: "Добавь кнопку «Калькулятор»", en: "Add button Calculator", badges: ["no_ai"] },
      { tool: "run_action", ru: "Запусти вебхук", en: "Run the webhook", badges: ["no_ai"] },
    ],
  },
  {
    id: "read_aloud",
    titleKey: "m29.cmd_g11_title",
    descKey: "m29.cmd_g11_desc",
    examples: [
      { tool: "read_aloud", ru: "Прочитай выделенный текст вслух", en: "Read selected text aloud", badges: ["no_ai"] },
      { tool: "read_aloud", ru: "Озвучь этот абзац", en: "Read this paragraph aloud", badges: ["no_ai"] },
      { tool: "read_aloud", ru: "Останови чтение вслух", en: "Stop reading aloud", badges: ["no_ai"] },
      { tool: "read_aloud", ru: "Прочитай ответ голосом", en: "Speak the reply aloud", badges: ["no_ai"] },
    ],
  },
];
