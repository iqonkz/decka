# Changelog

All notable changes to Decka. The format follows [Keep a Changelog](https://keepachangelog.com/); versions follow [SemVer](https://semver.org/). One number covers the PC agent, the phone app and the APK.

## Unreleased

## 0.70.0 — 2026-11-27

### Added
- **Wake word "Decka"** (experimental, off by default; Voice settings): say "Decka" or «Декка» instead of pressing the voice key. It runs offline in a process of its own (about 38 MB downloaded the first time, Apache-2.0 model); the sound is not saved or sent. The microphone closes while a note is recorded or a program is in full screen; the tray icon carries a dot while it is on. **Its accuracy and load have not been measured yet.**
- **Calendar** from an ICS link (Google, Outlook), no sign-in: a widget with the next events and **Join** for Zoom, Meet and Teams, "what do I have tomorrow" and "when is my next meeting" by voice, a quiet note five minutes before an event. The link is kept encrypted on the PC and meeting addresses never go to the phone.
- **Mini panel in the tray**: a left click on the icon shows the player, volume, microphone, up to six favourite actions and the next meeting; it does not take the focus and closes with a click outside or Esc.
- **Several PCs on one phone**: pairing a new PC adds it to the list; switch in the phone menu; when the PC is silent the phone offers one that answers.

### Changed
- The left click on the tray icon opens the mini panel; **Open settings** is in the panel and in the right-click menu.
- Phone keys: the icon is 40 % of the key's height, the caption 13 px medium.
- "Other PC" and "Pair again" on the phone no longer forget the saved PC.

## 0.61.0 — 2026-11-27

### Added
- **Check connection** for mail: five steps (server address, connection, encryption, sign-in, inbox) with the time of each; every failure says why (wrong password, app password needed, OAuth only, encryption, server, no answer) and what to do.
- **Save a problem report** (About): a ZIP on the desktop with versions, hardware, the end of the logs without your texts, settings without passwords and the result of "Check voice". Nothing is sent anywhere.
- **Backup** (Settings): buttons, layout, work modes, guides and settings in one `.decka` file without passwords; before restoring, Decka shows what changes and what the file's buttons start, and keeps a copy of the present state.
- **Portrait on the phone**: held upright, a landscape grid turns into portrait with the keys in the same order.

### Changed
- **A calmer, more consistent look** on the PC, the island and the phone: one scale of spacing, type, corners and motion; settings rows with icons; Phosphor icons in the settings window.
- The **phone grid fills the whole screen**; tiles adapt to their size, catalog keys are captioned in the phone's language, page dots are easier to see and touch, a swipe names the page.
- Phone **menu, pairing, toasts, screensaver and hold-to-confirm** redrawn; at most two toasts at the bottom, swipe them away.
- The **first-run guide** is three steps: connect the phone, turn on voice, done.
- **Meeting notes** open with the summary; the full text is folded, the header shows the date and the length.
- The **settings window opens faster**: its pages load when shown and only the icons it uses are included.
- **Look → Theme**: each preview takes the whole width of its card.

### Fixed
- A key made from the catalog keeps its link to it: its caption follows the phone's language until you rename it.
- The digits of a 1×1 **timer** no longer touch its ring.
- **Mail and Home Assistant over HTTPS did not work** in the installed application: setting up the encrypted connection stopped with an internal error, so the mailbox showed nothing.

## 0.60.0 — 2026-11-27

### Added
- A **work mode with focus minutes** starts a focus timer; at its end the island says so and the phone shows a toast.
- **Phone battery** on the PC: the battery widget shows the phone, the voice answers about it, and the island says "Phone: 15%" when it runs low off the charger.
- **What eats the memory**: ask "what is eating the memory?" and the island lists the five heaviest programs with a Close button each. Closing asks first, is polite (the program can save), and force needs a second yes; Windows' own parts, other users' processes and Decka are never offered.
- **Screen help in games and pictures**: when a window says nothing about its buttons, its text is read from a picture of it (Windows' own offline recognition, in memory only), and "where is", "press" and the guide work on the words.
- **Home Assistant**: the address and a token (kept encrypted), "ha" keys that switch a device, button states that follow it live, and voice for the devices you chose (locks and doors ask first).
- **Meeting notes**: say "start a note" (or use the island or a `notes_record` key) and Decka records the microphone and the PC sound, writes it down with the local whisper and sums it up (summary, decisions, tasks, questions) into a Markdown file on this PC. A red dot with a timer stays on the island while it records; nothing leaves the PC; the sound is deleted unless "Keep the sound" is on.

### Changed
- **Voice understands Russian better** (85 % -> 95-97 % of the test phrases; English 92 % -> 97 %): built-in programs by their Russian names, series of reminders, adding to lists, and questions that are answered instead of searched.
- The **step-by-step guide** waits for Windows to say the window changed instead of reading it over and over: about eight times less processor while it waits.
- Without a video card for speech, the **lightweight recognition** is on by default (a choice made by hand stays; an installed large model is not replaced by itself).
- The part of the model that looks at the screen (about 1 GB of memory) is loaded only while **Screen help** is switched on; asking about the screen with it off offers to switch it on.

### Fixed
- Big or slow windows were read as empty by the screen help ("did not find it") when reading took longer than 0.3 s; what was read is now kept.

## 0.51.1 — 2026-11-27

### Added
- **Lightweight recognition** (Settings -> Voice): a small speech model for PCs with 8 GB of memory or less - about 0.4 GB less memory and several times faster without a graphics card, somewhat less accurate.
- **A button added by voice goes to the page on your phone**: the phone now tells the PC which page it shows.
- A webhook key says on the phone which status code came back, or that nothing answered.

### Changed
- **Memory**: hidden island and highlight windows rest in a low memory mode; the highlight window closes 30 seconds after it is done.
- "Where is ...?" and "press ..." asked while the model is still loading wait for it ("Loading the model...") instead of saying "not found".
- A missing icon file shows as a default icon instead of rejecting the whole layout.

### Security
- Webhook header values are stored encrypted (Windows DPAPI); existing ones are converted at the first start.
- The agent answers only requests addressed to an IP address, `localhost`, this PC's name or `<name>.local` (protection from DNS rebinding); other names go into `server.allowedHosts` in `config.json`.
- Voice text commands keep your whole clipboard (pictures and files too) and restore it; the text Decka puts there for a moment stays out of the clipboard history and the cloud clipboard.

## 0.51.0 — 2026-11-26

### Added
- **Dictation orb**: while you dictate there is only a 24 px orb next to the text cursor; if there is nowhere to type, the text waits on the clipboard and a bubble says so.
- **The island rests as a thin bar** at the top edge while music plays; pointing at it opens three lines of lyrics. Settings -> Voice: bar or lyrics, height, left / centre / right.
- **Music is turned down while Decka listens** (to 20 %) and comes back; a setting turns this off.
- **Answers aloud** (off by default): a short answer is read by the Windows voice of your language; a new request or "stop" cuts it.
- **Screenshots**: "take a screenshot", "window screenshot", "screenshot a region" and three new keys; to the clipboard and `%TEMP%\Decka\shots` (kept 7 days).
- **Windows light / dark theme by voice**.
- **Add a button by voice**: ready-made keys of the library, shown as a draft and saved only after "yes".
- **Webhook key**: an HTTP GET or POST set up on the PC; plain `http://` only to this PC and the local network.
- **What you can say**: the full command list with search in Settings -> Voice; a **Screen help** page in the menu; every integration has a card.

### Fixed
- **The island no longer takes the focus**: dictation types into your field and "translate the selected text" works on the selection, not on the words "this text".
- **Dictation and text commands with a Russian or Kazakh keyboard layout**: `Ctrl+C` / `Ctrl+V` were typed as letters. The Windows 11 Notepad no longer garbles dictated text.
- **"Yes" answers a question on the PC**: say it, or press the button inside the island; the Screen help download shows its percent.
- A phrase that the recognition repeated several times is used once; "play music" starts the music instead of toggling it.
- Better contrast and tidier search boxes in the settings window and on the phone.
- "Where is the Save button?" finds Save: the word "button" is no longer searched for as part of the name; the answers of the screen help are in your language.
- The frame of the screen help is drawn: its window had no permission to receive what to highlight (since 0.50.0).

### Changed
- **Memory**: the model and the speech recognition leave the memory after 10 quiet minutes (Settings -> Voice: 1-60 minutes, or keep them loaded); the island says how long the next load takes. The model server takes about 0.5 GB less.

### Security
- `voice.log` no longer holds the texts of answers; texts written by earlier versions are removed at the first start.
- Shutting down, restarting, signing out, sleeping and emptying the recycle bin by voice always ask "yes".
- Updated `url` and `cookie` (RustSec advisories); a download longer than expected is stopped; the island window has fewer permissions.

## 0.50.1 — 2026-11-26

### Added
- **Check voice**: a button in Settings -> Voice tries the chain step by step — microphone level, a spoken phrase turned into text, the assistant, and what the command would do (nothing is pressed) — with the time and the reason of each failed step.
- **Command stages on the island**: what was heard, what was understood, then the action.

### Changed
- **Weak PCs**: the assistant now takes only as much video memory as the card has to spare, runs on the processor when the card is too small or fails to load it (and remembers that), and the settings suggest the light 2B model on a PC with less than 12 GB of memory.
- **Speech recognition without the assistant**: if the assistant cannot start, recognition and the instant commands keep working.
- **Island, music**: three centred lines — previous, current, next — in a 440 px panel with larger text; the height no longer jumps.

### Fixed
- **Voice errors say what is wrong**: video memory, memory, damaged model file (with "Download again"), busy port, stopped process, microphone or recognition, instead of "Voice stopped" or a raw log line.
- **Voice on a PC with 2 GB of video memory**: the model no longer fails with a loading error because everything was put on the card.
- **Settings**: a main button under the mouse pointer no longer turns grey with unreadable text.

## 0.50.0 — 2026-11-26

### Added
- **Show me where**: ask `where is Save` and a frame with a short bubble lights up that button in the active window; `not that one` goes to the next candidate. Nothing is guessed: only elements that really exist in the window can be shown.
- **Press, scroll, close a tab by voice**: `press this`, `press Export on the screen`, `scroll down`, `close the tab`. It acts only when you say so; a button named like Delete, Send, Pay or Buy waits for your "yes" first.
- **Step by step guide**: `how do I save as PDF` leads you through a program one button at a time, with a card in the island (goal, step, hint, Back, Stop) and `next` / `back` / `stop` by voice. While the assistant thinks it shows how long it will take.
- **Saved guides**: after a guide you can keep it (`remember how to ...`), or record your own clicks; the next time it runs without the assistant. They are listed, renamed and deleted in Settings -> Voice. Everything stays on this PC.
- **Read aloud**: `read aloud` reads the selected text (or the text under the mouse) with the Windows voice; `stop` cuts it.
- **Work modes**: `remember the layout as work` saves the open programs and where they are; `work mode` opens what is missing and puts the windows in place. Only the programs of the saved mode are ever started.
- **Notices**: short quiet notes on the island when memory or the processor stays overloaded, a device battery is low, the disk is nearly full or an AI limit is almost used up (once in 30 minutes, never over a full-screen program; a switch in Settings -> Voice).
- **Extended model**: Qwen3.5 9B can be chosen in Settings -> Voice on a PC with 16 GB of memory or 8 GB of video memory (the download size is shown first; one model at a time).
- **Voice phrases** for all of this in 11 languages; "weather in <city>" understands another city in every language.

### Changed
- **Voice accuracy**: reminders named with a clock time are understood without a day; more phrases (which monitor, move the window, summarize, formal, casual) are answered instantly without the assistant; short examples help it choose between look-alike commands.

### Fixed
- **Island readability**: the compact island with song lyrics no longer lets the wallpaper show through; only its text and orb dim at rest.
- **Idle island**: a hidden island stops listening to the music level and reporting its size, so it costs nothing while you don't see it.

## 0.40.1 — 2026-11-25

### Added
- **Island, answers**: the answer buttons are now icons (Copy, Insert, Speak, Try again, Language) and the language menu opens below the panel. A "Copy code" chip appears only when the answer contains a code block.
- **Island, music**: the compact island with a song is a fixed 520 px wide. The sphere is 48 px and moves with the music; on hover Play/Pause appears over the sphere and Next beside it. Before the first lyric line it shows the title and artist with "Lyrics in 0:12", or three dots while the lyrics are being searched for.
- **Voice**: "status battery" answers with the PC and Bluetooth devices.

### Changed
- **Island**: a solo sphere opens into the answer smoothly; the window follows the size of the panel; the panel is a little less transparent (96 %) in both themes, and a light panel has a clearer edge on light wallpapers.
- **Performance**: the sphere is drawn from pre-rendered dots (480 in the solo view, 160 small), at 30 fps while active, 15 fps at rest, and nothing while the window is hidden. The useless blur behind the panel is gone.

### Fixed
- **Island**: a song without a lyric line no longer says "Listening..." and the sphere no longer shows the listening state.
- **Voice**: "weather in Paris" no longer answers with the weather of your city; it goes to the assistant instead. Reminder examples and tool descriptions are clearer for the model.

## 0.40.0 — 2026-11-25

### Added
- **The island**: one small window at the top of the screen replaces the voice pill and the lyrics strip. It shows what the assistant hears, thinks and answers, with a living sphere that reacts to your voice, and the song lyrics: a thin tab when nothing is going on, the current and next line when a new song starts or when you point at it. It lets clicks through everywhere except on itself and hides over full-screen programs (except the voice).
- **The assistant knows your panel**: ask `what's the weather`, `how loaded is the processor`, `how much memory/disk is free`, `what is playing`, `when is my next reminder`, `ai limits`, `phone battery`, `which program is active` - the answer comes from the widgets on this PC (never from the internet), with numbers. No data or the widget is off: it says where to turn it on.
- **Continue the conversation**: for 30 seconds after an answer you can say `shorter`, `more polite`, `and in German`, `another option`, `insert it`, `copy`, `read it aloud`; the island also has buttons (Copy, Insert, Speak, Try again, Language). Nothing is kept on disk.
- **Say the text, not select it**: `how do you say "thank you" in Chinese`; a draft reply to the selected message (`reply`). The selected text is now read through UI Automation first, so your clipboard is left alone when the program supports it.
- **Hide or show lyrics by voice**: `hide the song lyrics` / `show the song lyrics`.
- **Hints by context (off by default)**: when the island opens, `More -> Voice -> Hints from the active program` lets it offer Translate / Explain / Reply (for the selected text; Reply in mail and chat programs) or `What is this error?`. It only looks at the program name, the window title and the selection, once, and stores nothing.
- **Slow answers say so**: before a question about the screen the island shows `Looking at the screen... ~N s` with a bar; N is the average of your last five such questions; `Stop` cancels.
- **Phrases in 11 languages** for the new commands, and a check set for each.

### Changed
- The settings window uses the shared radio and time controls everywhere (arrow keys work in radio groups).
- `tools/voice-eval.mjs` now sends the real prompt of the agent (`voice-prompt-dump`), so the accuracy numbers match what the assistant does.

## 0.30.0 — 2026-11-24

Version 0.22.0 was built but never published; its changes are part of this release.

### Added
- **Recurring reminders in the settings window**: pick `Every day` / `Weekdays` / `Every <weekday>` next to the time; the list shows a repeat badge, and a repeat can be switched off again.
- **First-run wizard restyled**: step bar, one language selector, set cards with the shared checkbox, and an empty-deck screen on the phone that says what to do next.
- **Lyrics on the PC**: while a song with synced lyrics plays, a small click-through strip at the bottom of the screen shows the current and the next line (`Integrations -> Lyrics -> Lyrics on this PC`: `Off` / `Auto` / `Always`, default `Auto`). `Auto` shows it while a song plays (it does not depend on the phone), hides it 3 s after the song is paused, and hides it over full-screen programs; `Always` shows it whenever a song with synced lyrics plays. No new network requests.
- **Phone widgets and screens (0.30)**: every widget has one look in all four themes (a small header, equal corners and padding, quiet empty and waiting states, skeletons, numbers that move smoothly, the cover colour of `Now playing` as the tint of its tile); the service screens (pairing, link lost, update, permission, menu) were redrawn.
- **Hourly weather**: the weather widget shows the next six hours (`HH:mm`, icon, temperature) from Open-Meteo; the brief of the day says `Rain expected around HH:mm` when the data shows precipitation in the next six hours.
- **Song lyrics (off by default)**: `Integrations -> Lyrics` switches a lookup on lrclib.net (only the artist, the title and the length of a song are sent, once per song; nothing at all while it is off). The large `Now playing` widget shows the lines with the current one bright; a long press on the text shows `-0.5 s` / `+0.5 s` to fix a version that does not match, the shift is kept per song. Cache on this PC: at most 500 files.
- **Recurring reminders**: `every day at 9 am`, `по будням в 8:30`, `каждый понедельник в 10 вечера` (Russian and English phrases; other languages through the model). A series fires, then moves to its next time; deleting it removes the whole series; after the PC was off it is one `Missed` notification per series.
- **Keyboard in the settings window**: a visible focus and Tab order on every page, dialogs and pickers close with Esc.
- **Voice phrases for `calc` and `ask`** in all 11 languages; fast-path phrases for opening a site (`открой сайт github.com`), maps search and dictation.
- **New settings window (T23)**: one sidebar menu for everything; a search box that finds a setting by its name in the language of the window and jumps to it (for example `микрофон` scrolls to the microphone row in Voice and highlights it); settings as rows with switches and controls; an integrations catalog with cards; window theme System / Light / Dark with Mica; autosave (400 ms, rolls back on an error); a colour picker; a new reminders list with a time picker (a new reminder defaults to an hour ahead, :00 or :30).
- **Calculator by voice**: `54 умножить на 114` answers `54 × 114 = 6156` (our own parser, no model; `+ − × ÷ ^ %`, brackets, square root, `20 percent of 150`, in 11 languages). Division by zero is answered, not guessed.
- **Short answers by voice**: a question that is not a command (`how many days in a leap year`) is answered by the local model in at most 60 words. When it cannot know offline (news, rates), the pill asks `search the web?` and opens the search only after a yes.
- **Reminder times by voice**: `в 22.14`, `в 2215`, `22:14`, `в 10 вечера`, `через 5 минут` are read exactly and cut from the text of the reminder (they were saved at the wrong minute before).

### Fixed
- **Long press on a key or widget never opens the phone menu or a system menu** (especially the voice button); the menu opens only on empty space. Pressing the microphone with the menu open closes the menu, and a socket that closes during a recording ends it quietly.
- **Settings window**: native checkboxes, selects and `details` replaced by the shared controls everywhere; the `Phone not connected` strip is a banner like the others; repeat of a reminder can be switched off again.
- Text contrast in the settings window and the integration cards.
- **Voice pill stuck in `Listening…`** after several commands in a row: a late microphone-level update re-created the pill after it was closed; all pill commands now go through one ordered queue.
- **Settings search** found nothing for words like `микрофон`: every page and setting is now searchable in all 11 languages.
- **Keys page in a narrow window**: the preview keeps its width and the library becomes a button below 1100 px.
- **Claude / Codex widget 2×1** shows both limits (5 h and weekly) as two small bars.
- **Mail form**: errors are shown in the language of the window.
- **Window theme** is kept when the layout is saved.
- **Voice**: a template such as `убери {item} из {list}` no longer swallows a request to shorten a text; `release:apk` no longer hangs on the Gradle daemon.
- Checks: 500 reconnects of a phone and 50 abrupt drops leave no connection, subscription or task behind; the messages `list_toggle`, `notif_open`, `notif_dismiss`, `perm_answer` and `lyrics_offset` have a limit of 20 per second and ignore ids that the PC did not issue.

## 0.21.0 — 2026-11-23

### Added
- **Smart dictation (T22.10)**: press Ctrl twice quickly (left or right; `Ctrl+C`, `Ctrl+V` and other chords do not count) and speak; the text goes through the local model (`clean`: punctuation and capitals, fillers removed, self-corrections applied, spoken lists as lines, nothing added; a guard falls back to the raw text when the answer is much longer or shorter, brings new words or takes more than 3 s). A personal dictionary (up to 500 words) is handed to speech recognition and to the model. Up to 300 characters are typed, longer text is pasted through the clipboard and your clipboard text comes back after a second. One tap of Ctrl or a pause of 2 s ends it, `Esc` cancels. Trigger (double Ctrl, a shortcut or off), mode (`Clean` / `Raw`) and the dictionary are set in `More -> Voice -> Dictation`. The pill says `Dictating...` with a microphone.
- **Selected text (T22.11)**: `polite`, `shorter`, `fix`, `translate` (11 languages), `formal`, `casual` replace the selection (Ctrl+Z undoes it); `summarize` and `explain` show the answer on the pill and the phone. Nothing selected: `Select text first`; more than 6000 characters: `The text is too long` (11 languages).
- **Help with the screen (T22.12)**: `what does this error mean` looks at the active window only (in memory, scaled to 1280 px, never saved or logged). The optional `Screen help` package (the projector of Qwen3.5, Apache-2.0, about 0.67 GB) is downloaded after you agree; the model server restarts with it.
- **Reminders and lists (T22.13)**: `remind me in 10 minutes ...`, `... tomorrow at nine ...`, `... on Friday at 8:15 ...`; `what are my reminders`; `add milk to the shopping list`, `what is on the list`, `remove milk`, `delete the list` (asks first). A reminder arrives as a Windows and phone notification; what was missed while the PC was off comes once as `Missed: ...`. The phone widgets `Reminders` and `List` (2x1 and up; tick marks answer at once and roll back if the PC refuses; `list_toggle` carries only ids the agent issued) and the window `More -> Reminders` (reminders with `Show done`, lists with their items) edit the same files by hand. A reminder that falls due also shows its text on the pill for 5 s.
- **Brief of the day (T22.14)**: `good morning` / `what do I have today` tells 3-5 sentences from the weather, unread mail, today's reminders, open lists and the limits of Claude and Codex; a number or name that is not in that data is refused and the plain facts are shown.
- **Memory (T22.15)**: `remember that Aigerim is my wife` (only after such a phrase), `forget Aigerim` (asks first); up to 20 fitting facts go to the model with each request. Nothing is learned by itself and no phrase is recorded. `More -> Voice -> Memory` lists the facts (edit in the row, delete, `Forget all` after a question, `N / 200`).
- **Voice phrases for the new tools (T22.10b-T22.15b)**: fast-path phrases and evaluation phrases in all 11 languages (from the second agent), checked for slots that would swallow another command (`open {list}`, `screen {text}` removed).

## 0.20.6 — 2026-11-23

### Fixed
- **Phone microphone**: `RECORD_AUDIO` and `MODIFY_AUDIO_SETTINGS` were missing from the APK of 0.20.5 (the manifest change of T21.17 was not committed), so the voice button could not use the microphone. Both permissions are declared now, and `release:apk` checks the built APK (`aapt dump permissions`: RECORD_AUDIO, CAMERA, INTERNET) and fails if one is missing.

## 0.20.5 — 2026-11-23

### Fixed
- **Phone: a way out of "no link" (T21.15)**: the phone retries by itself (at once, then after 1, 2, 5 and 10 s, then every 10 s, and at once when the app comes back or the network returns). After 8 s without a link a card appears over the dimmed layout: `Can't reach {pc}` with the reason (the PC does not answer / the PC does not know this phone any more) and the buttons `Try again`, `Connect again`, `Use another PC`, plus a folded `Why?`. A rejected key stops the background attempts and keeps the saved PC until a new connection has succeeded. `More → Connect again` is always there; scanning the QR of the same PC again updates its record instead of adding a second one (11 languages).
- **Agent: "not listening, os error 10048" (T21.16)**: a busy port is retried for 15 s (0.5, 1, 2 s), so the old copy still closing after an update no longer leaves the agent deaf. If another program keeps the port, the Phone tab names it (`Port 9247 is used by {app}…`) and `Try again` binds again without restarting the agent.
- **Phone voice button (T21.17)**: the app now asks for the microphone (`RECORD_AUDIO` was missing, so every tap was cancelled after 0.1 s); a refusal says `Allow the microphone for Decka in Android settings`. Releasing the button before the microphone has opened ends the request instead of cancelling it. A long press no longer opens the system context menu.

## 0.20.4 — 2026-11-22

### Changed
- **Stage-based watchdog**: individual timeouts per stage instead of a global 30 s limit: listening (30 s), model loading (up to 120 s), recognition (its own limit, see below), and AI inference (20 s). The watchdog timer resets at each stage transition.
- **Background pre-warming**: `llama-server` and `whisper-server` start in the background immediately upon pressing the hotkey or tapping the voice button, parallel to speech recording. The "Loading the model…" pill is only shown if the server is still loading after the phrase finishes.
- **Speech recognition on the video card**: on an NVIDIA card with 2 GB or more the GPU build (cuBLAS) of `whisper-server` is downloaded (whisper.cpp b5907, `whisper-bin-win-cuda-12.4.0-x64`, about 735 MB; there is no ready Vulkan build); other PCs keep the CPU build. A phrase takes 0.1-0.3 s instead of 12-20 s (RTX 2060 SUPER, large-v3-turbo); the first start on a card compiles its kernels once, inside "Loading the model…". After the update `Turn on voice control` downloads the new runtime once.
- **Faster recognition everywhere**: greedy decoding (beam 1, best of 1), no timestamps, `audio_ctx` by the length of the recording, one thread per physical core (a CPU takes about 4.5 s instead of 13 s for a short phrase).
- **Recognition limit**: 15 s on a GPU, 10 s + 3 x the length of the recording on a CPU (at most 60 s). The pill and the phone show `Recognizing…` (11 languages). `DECKA_VOICE_STT_TIMEOUT_S` is gone.

### Fixed
- **Recording closed at once on the PC**: a single audio buffer glitch (xrun) of the microphone was treated as the end of the device; it is ignored now. A press of the hotkey or a click on the pill within 400 ms of the start no longer ends the recording. The window of the pill no longer holds the microphone back (it used to delay the start by 3-7 s).
- **The pill stayed on the screen (T21.13)**: a late update of the wave could bring the closed pill back, and a close that came while the window was still being created was lost. Going idle now always closes the pill, in every sequence of repeated presses.
- **No sound from the microphone (T21.14)**: a microphone that gives only digital silence is reopened after a second; if it stays silent, the other real microphones are tried in turn (virtual ones such as AudioRelay, Steam Streaming, CABLE, Stereo Mix are skipped) and the one that works is remembered. When none works the pill says "Microphone gives no sound — choose it in More → Voice" (11 languages). `voice.log` names the device that was opened and keeps the last 50 endpoint decisions for NoSpeech and capture failures.
- **Invented text from silence (T21.14)**: a recording without any speech is no longer sent to the recognizer or the language model, even after "stop"; the answer is "Didn't hear anything".
- `voice.log` records the events of a session (names and timings only, no sound, no text).
- **Phone long-press gestures**: long pressing the floating voice button in mobile browsers no longer opens context menus, selects text, or triggers page gestures.

## 0.20.2 — 2026-11-22

### Fixed
- **Voice settings**: the "Keep the model loaded" checkbox and the other voice settings gave `missing required key args` and were not saved. All of them (hotkey, model, microphone, keep loaded, phone button) save again. The "Speak the answers" checkbox is hidden until spoken answers exist.
- **The program froze on "Thinking…"** after pressing stop on the voice pill: closing the microphone waited for the audio thread while that thread waited for the main window thread. Closing now happens on a thread of its own, the pill is updated off the audio thread, and recognition (15 s) and the language model (20 s) have time limits.
- **A phrase that never ended on a real microphone**: the noise level is now taken from the quietest part of the last 3 seconds (speech in the first second no longer counts as noise), and a microphone that raises its gain in a pause no longer keeps the phrase open. The decisions of the first five requests go to `%LOCALAPPDATA%\Decka\logs\voice.log` (numbers only, no sound and no text; set `DECKA_VOICE_DEBUG=1` for all).

## 0.20.1 — 2026-11-22

### Fixed
- **Voice pill stuck in "Listening…"**: the end of a phrase used a fixed loudness limit, so with a quiet microphone the recording never ended and the wave stayed as three dots. The pill now measures the room noise first and ends the phrase itself after 2 s of silence; nothing said for 5 s ends with "Didn't hear anything"; a request can never stay in one state longer than 30 s ("Something went wrong, try again"); a microphone that does not open or gives no sound is reported as "Microphone is unavailable".
- Phone: a tap on the voice button did nothing and a long press opened the page's menu. A tap now listens (the icon turns to stop, the ring follows your voice, silence for 2 s ends it, a second tap ends it at once); holding for 250 ms is a walkie-talkie (release ends it); dragging moves the button. The long-press menu and text selection no longer appear on the button.

### Added
- Faster voice commands in Spanish, Hindi and Kazakh.
- **Stop button on the PC pill** (and a click anywhere on the pill): ends the recording and runs the command at once, without taking the focus from the program you type in.
- The wave of the pill is five bars that follow the loudness of your voice.

## 0.20.0 — 2026-11-22

### Added
- **Voice control (local AI)**: say what you want, with **Ctrl+Alt+Space** on the PC (a small pill shows what was heard and what was done; press again to finish, Esc cancels) or with the round microphone button on the phone (hold to talk, drag to move it to another edge; it can be hidden in the phone menu; there is also a `voice` key for the grid). Switched on in **More → Voice → Turn on voice control**: the speech model (Whisper large-v3-turbo) and the language model (Qwen3.5 4B, or 2B on PCs with little memory) are downloaded once (≈ 3.2 GB), every file checked against a fixed SHA-256, and run as hidden programs on `127.0.0.1` (llama.cpp with Vulkan, whisper.cpp). Nothing you say leaves the PC and no phrase is written to the log. The servers start with the first phrase and stop after 10 minutes of silence (`Keep the model loaded` switches that off).
- **Understanding**: a table of phrases for each of the 11 languages first (instant), then the model with a JSON schema for the rest. The model only proposes a call of one of a fixed list of tools; every call is checked on the PC against what exists (installed programs, your buttons, actions and pages) before anything runs. Tools: press a button or run an action of yours, open / switch to / close a program, move the window to the other monitor, which monitor, volume (up, down, mute, a number), media, go to a page, web search (web, YouTube, maps), open a site, type text, dictate (the next phrase is typed as it is), timer, lock and sleep (these ask `Yes / No` first, by voice, by Enter / Esc or on the phone sheet).
- **What you can say** help card in More → Voice, with examples in your language.
- Phone: the sound travels only inside the encrypted channel (`voice_start`, `voice_chunk`, `voice_end`, `voice_cancel`, `voice_confirm`); one recording per phone, 30 s at most; the phone never sends a command.
- `voice.*` is now a reserved variable prefix (`voice.ready`, `voice.goto`).
- Developer tools: `tools/voice-eval.mjs --check` validates the evaluation sets; `cargo run --release -p decka-core --example voice_eval -- --serve standard|light --settings <dir>` measures accuracy against the installed models; `tools/voice-shots.mjs` draws the pill and the phone button.

### Fixed
- Model downloads from Hugging Face failed on the redirect to its new CDN host (`us.aws.cdn.hf.co`); the whole `*.hf.co` domain is allowed now (the SHA-256 check is unchanged).

### Voice accuracy (measured on this release, text only, no audio)
25 phrases per language that do not repeat the table word for word, through the phrase table and, where it has no match, the real model (Qwen3.5 Q4_K_M on a Vulkan GPU). A call counts as right when the tool and the expected arguments match (text arguments by containment, ignoring case). Targets: en, ru, es, de, fr, pt 90 %, the rest 80 % (DATA_MODEL.md 12.8).

| Language | 4B (standard) | 2B (light) |
|---|---|---|
| en | 96 % | 80 % |
| ru | 92 % | 64 % |
| es | 88 % | 60 % |
| de | 96 % | 60 % |
| fr | 96 % | 60 % |
| pt | 96 % | 68 % |
| zh | 80 % | 60 % |
| hi | 72 % | 48 % |
| ar | 92 % | 56 % |
| ja | 92 % | 60 % |
| kk | 84 % | 44 % |

Understanding time of a phrase (without recording and speech recognition): the table 0.07 ms; the model 545 ms with 4B and 410 ms with 2B on a warm server (the first phrase after a pause also loads the model: ≈ 4 s here).

Voice commands in Spanish and Hindi are experimental with the standard model, and in every language with the light model (use it only when memory is short). Russian and Kazakh phrase tables were written by a language model and are checked by the owner.

## 0.11.0 — 2026-11-21

### Added
- **One-click update of the PC agent**: when a newer release exists, More → About shows `Update to X.Y.Z`. Decka downloads the installer, checks its signature against the update key built into the agent (an unsigned or altered file is refused: `The update is not signed by Decka`), installs it and restarts. `Download` stays as a fallback. The installer is still not a Windows-signed one, so SmartScreen may warn on a manual install.
- **Update the phone app from the PC**: the agent downloads the new APK of the release, checks it against `SHA256SUMS.txt` and the signature of that file, and offers it to paired phones. The phone menu shows `Update Decka to X.Y.Z`; the file travels through the encrypted channel, is checked again on the phone and opened in Android's installer (Android asks once to allow installing from Decka). The APK has the same signature, so it installs over the old one and the pairing stays.
- Release files now include `latest.json` (the update manifest) and `SHA256SUMS.txt.sig`.

### Changed
- **AI limits widget redesigned**: figures and shapes instead of words. 2x1: a ring per provider; 2x2: two concentric rings (short window outside, week inside); from 3x2: two bars per provider. Colour changes at 70% and 90%. `↻ 2h 14m` is the time to the reset, `⚠ 14:20` the forecast when the limit ends before it. Text that does not fit is left out instead of cut. A tap opens a sheet with exact percents, reset times, the forecast, the plan and the sessions.

### Fixed
- **Turning the phone upside down**: `Auto` now reads the angle of the phone itself and flips between the two sides of the layout's orientation; it no longer relies on the system sensor mode, which did nothing on some phones. The layout orientation is never changed by the sensor.

## 0.10.2 — 2026-11-21

### Changed
- The release APK is now a signed release build instead of a debug build: smaller, not `debuggable` (no `adb run-as` access to the device token). **This release is signed with a new key: uninstall the old Decka app first, then install and pair again.**

## 0.10.1 — 2026-11-21

### Fixed
- Library rows show the right hint: the shortcut for a hotkey, the exe for a program, the host for a link; media, audio, OBS and system keys have none. The name gets at least 60% of the row.
- Empty widgets (notifications, ...) show the title and the hint on separate lines, the hint is cut to two lines.
- Dates in languages without `Intl` data (Kazakh showed `M11 21, Sat`) come from the dictionary: `21 қараша, сб`.
- The library panel is 300 px wide and the Kazakh tabs (`Түймелер`, `Жиындар`, `Қолданбалар`) fit whole.

## 0.10.0 — 2026-11-21

### Added
- **11 languages**: English, 中文（简体）, हिन्दी, Español, العربية, Français, Português (Brasil), Русский, Deutsch, 日本語, Қазақша. Pick one in More → Settings → Language or on the first step of the setup guide; **Auto** follows the system language. The window, the phone, the tray menu, the ready-made keys, the sets and the app-strip captions are all translated. Arabic runs right to left in the window and the menus; the grid of keys is never mirrored. The translations were made by a language model: corrections are welcome (README → Translations).
- **Library panel** of the Deck tab, redesigned: a fixed search field (`Esc` clears, `↓` goes to the list), a segmented switch Keys / Sets / Apps, rows with a `+` that adds a key to the first free cell, sticky categories with counts, keyboard navigation.

### Changed
- Thin scrollbars in the whole settings window; rows of buttons no longer push their last button out of the window in long languages.

## 0.9.0 — 2026-11-20

### Added
- **Encrypted connection.** Everything between the phone and the PC is now encrypted, and each side proves it holds the pairing key, so someone on the same Wi-Fi can neither read the traffic nor pose as your PC or phone. Update Decka on the phone and the PC together — the connection is now encrypted; an old app will ask to update.
- **Public networks.** The PC knows which kind of network a phone connects through (Phone page). On a public network it serves only the Decka app and pairing is by QR code or USB; `Allow phones on public networks` turns such connections off completely.
- **Who may answer Claude Code.** `Can answer Claude Code` per phone (phones paired before keep it, new ones start without). A dangerous command needs a one-second hold on the phone and offers no `Always allow`; `Always allow` from a phone applies to one session and ends after 8 hours, and the rules list shows who made each rule.
- **Notification privacy.** Notifications of messengers and mail programs reach the phone as `New notification` with the program name only; choose `Full text`, `App name only` or `Don't show` for every program.
- Every release carries `SHA256SUMS.txt`; the README explains how to check a download.

### Changed
- Shutdown, restart, sign out and hibernate from a phone are refused in the first minute after connecting and are limited to one per 30 seconds.
- Device keys and the passwords and sign-in tokens in `config.json` are stored only under Windows DPAPI (a value typed into the file by hand is encrypted at the next start).
- The phone's app and the PC must run the same minor version; the plain (v1) connection is accepted from this PC only (USB, development).

## 0.8.1 — 2026-11-20

### Added
- **Claude limits from your account** (Integrations → AI coding): a switch that reads the limits through the Claude Code sign-in on this PC, for the VS Code extension, which has no status line. Off by default.
- The empty notifications widget says when Windows notifications are off and where to turn them on.

### Fixed
- Shortcuts with letters (Ctrl+B, Win+V, Win+.) work in Word, browsers and the Windows shell: keys are sent as virtual keys, with short pauses.
- The volume slider no longer jumps between two values.
- The CPU/RAM graph no longer resets when pages are swiped.
- Windows notifications that were already in the Action Center appear when the feature is turned on.
- Material theme has a black background.
- Turn `Auto` follows the sensor on both sides.

## 0.8.0 — 2026-11-19

### Added
- **Windows notifications** on the phone (More → Integrations → Windows notifications): new toasts of Windows appear on the phone, with a list of apps that are not shown. A tap opens the program the notification came from.
- **App strip**: when a program without a page of its own is in front, a strip with the keys of its built-in template slides out at the edge of the phone (it replaces the "Add keys for it?" hint on the phone). Per program: open automatically, only the tab, or never; the phone menu lists the ones turned off.
- **Turn** in the phone menu: Auto, Normal or Upside down, so the phone can stand with the charging port up or down.
- **Battery** widget: the charge of the PC (a laptop) and of Bluetooth devices that report it; variables `battery.pc` and `battery.low`.
- **Mail** widget: unread messages of up to three mailboxes (IMAP over TLS, an app password kept encrypted for this Windows user); variables `mail.unread` and `mail.<id>.unread`. A tap opens the mailbox through an action you choose.
- System actions: shut down, restart, sign out, hibernate, turn the display off, empty the recycle bin; Task Manager, Settings and Explorer now start the program itself instead of a shortcut.
- Shortcut field: switches for Ctrl, Alt, Shift and Win and a searchable key list, for the shortcuts Windows takes before Decka sees them (Win+D, Alt+Tab, Print Screen).

### Changed
- **Memory:** closing the settings window now destroys it (the agent with a closed window is about 14 MB instead of about 230 MB); the window opens again from the tray. Collectors that no page or variable needs are not started, and the disk list is read every 30 seconds instead of every tick.
- Swiping between pages follows the finger without redrawing the pages on every move; the neighbours are prepared in an idle moment after the page settles.

## 0.7.0 — 2026-11-18

### Added
- **App keys** widget: the keys of the program that is in front on the PC appear in a part of the phone screen (no need to swipe to its page). It uses the pages that follow a program (Look → context pages); add it from the library (Widgets) at any size from 2x1.
- New page templates: Outlook, Word, Excel, PowerPoint, Canva, Telegram, WhatsApp and Discord (More → Pages); their shortcuts were checked against the programs' help pages and well-known guides.
- A **Work desk** set: the App keys widget with lock, Task Manager, media and mute keys next to it, so the keys of the running program and the important keys share one screen.
- The AI limits can be added for Claude and for Codex separately (library → Widgets), also at 2x1.
- **Check the connection** in the Phone tab: shows what blocks a phone (server, addresses, Windows Firewall) and repairs the firewall rule with one click.
- Recent questions of Claude Code and Codex: tap the AI limits widget on the phone to see what was allowed or denied and when.
- **Always allow** on the permission sheet for a plain command, edits inside one project, one website or one MCP tool. The rules are listed (and removed) in More → Integrations → AI coding.
- **Deny and say**: under a permission question the phone offers ready answers (editable in More → Integrations → AI coding); the chosen text is told to Claude Code or Codex as the reason of the denial.
- A page can open **at certain hours** of the day (the phone's clock); a page of the program in front still wins.
- In the Pages editor a hint offers the template of the program that is in front when no page follows it yet.
- A **Teams** template; more keys in the **browser** template (bookmark, address bar, find, downloads, history, next tab).
- The phone shows what to check (same Wi-Fi, Decka running, Check the connection) when it cannot reach the PC.
- Twitch: a notification on the phone for every new follower. YouTube: keys **YT go live** and **YT end** for the stream scheduled in YouTube Studio.

### Changed
- A sharper picture in the Windows installer on high-resolution screens.

## 0.6.0 — 2026-11-17

### Added
- YouTube: turn it on in More → Integrations → YouTube. In the Google Cloud console make a project, turn on YouTube Data API v3 and create an OAuth client of the type "TVs and Limited Input devices"; paste its Client ID, set its client secret (stored encrypted for your Windows user), press **Sign in** and type the shown code on google.com/device. The new **YouTube** widget on the phone shows whether you are live, the viewers, the subscribers, the title and the time on air. A new key **YT chat** posts a message of your choice in the chat of your live stream. The numbers are also available to keys as `youtube.live`, `youtube.viewers` and `youtube.subscribers`. Decka looks at the channel once a minute, which fits the free daily quota of YouTube.

## 0.5.0 — 2026-11-17

### Added
- Twitch: turn it on in More → Integrations → Twitch. Register a free application on dev.twitch.tv/console (Client Type: Public), paste its Client ID, press **Sign in** and type the shown code on twitch.tv/activate (no password and no secret are asked; the sign-in is stored encrypted for your Windows user). The new **Twitch** widget on the phone shows whether you are live, the viewers, the followers, the title, the game and the time on air. New keys: **Clip** (a clip of the last seconds of your stream) and **Chat post** (an announcement with the text you choose). The numbers are also available to keys as `twitch.live`, `twitch.viewers` and `twitch.followers`. A **Twitch** set adds the clip key and the widget in one click.

## 0.4.3 — 2026-11-17

### Changed
- A new Decka logo everywhere: the Windows installer (its side picture and the installer file icon), the app and tray icon, the phone icon and launch screen, the pairing screen, the settings window and the README.

## 0.4.2 — 2026-11-17

### Fixed
- The phone could not connect on some PCs and networks (a hotspot of the PC or of another phone counts as a public network for Windows, and Windows Firewall blocked the agent there). The installer now adds a Windows Firewall rule for Decka for private and public networks (Windows asks for administrator rights once; refuse it and the installation goes on). The hint with the firewall command in the Phone tab also appears when the first pairing fails, and its command covers both kinds of network.
- File names of a release carry the version (`Decka-Setup-<version>.exe`, `Decka-<version>.apk`).

### Added
- Codex asks the phone too: **Install** also adds a Codex hook (`%USERPROFILE%\.codex\hooks.json`), so a permission question of Codex opens the same sheet as the one of Claude Code, with the name of the tool that asks. Open Codex once and trust the hook with `/hooks`. Switch it off with **Approve Codex on phone**. The phone gives a short buzz when a question arrives.
- The AI limits widget can show only Claude or only Codex (the widget settings in the editor).

## 0.4.1 — 2026-11-17

### Fixed
- Smoother page swipe on the phone: no micro-freeze when you start a swipe and no small jerk when the page settles (the neighbour pages are now prepared in advance).
- The number badge on the AI limits widget now opens the permission sheet again.

## 0.4.0 — 2026-11-16

### Added
- AI coding limits: turn it on in More → Integrations → AI coding (**Install** adds a status line and a permission hook to Claude Code, **Remove** takes them back; restart running Claude Code sessions after installing). The new **AI limits** widget on the phone shows the 5-hour and weekly limits of Claude Code and the limits of Codex, when they reset, a forecast of when a limit runs out, and your Claude Code sessions. Everything stays on your PC: nothing goes to the internet and no login token is read.
- Approve from the phone: when Claude Code asks for permission (a command, a file change, a web address), a sheet with **Allow** / **Deny** / **Answer on PC** opens over any page of the phone. If no phone is connected, or the wait runs out, the question goes to the terminal as usual. The new **AI coding** set adds the widget in one click, and keys can follow the limits (`ai.claude.5h`, `ai.claude.7d`, `ai.codex.primary`, `ai.pending`).

## 0.3.0 — 2026-11-16

### Added
- OBS Studio: turn it on in More → Integrations → OBS Studio (enable the WebSocket server in OBS, set the password once; it is stored encrypted for your Windows user). Keys for record, go live, replay, scenes and audio input mute, and the OBS widget with `REC` / `LIVE` timers, follow OBS live.
- Weather: turn it on in More → Integrations → Weather, search for your city, pick °C or °F. The PC asks Open-Meteo (the phone never goes online; coordinates are rounded to about 1 km) and the weather widget shows now and three days ahead.
- A library of ready-made keys and sets in the editor (Windows tools, Media, Sound, Stream, Widgets, Navigation, Web, Keys) with search, in English and Russian. Add a key by dragging it onto a cell or double-clicking it; add a whole set (Media remote, Sound, Windows tools, Streaming, Focus) with one click. Keys that need an address, shortcut, OBS scene or page ask for it first. A sequence editor builds a chain of actions with pauses. The first-run guide offers the sets as cards. The cell picker uses the same categories.
- New widgets on the phone: a volume slider, a timer, weather, OBS status, and a CPU / RAM graph for the System widget. A new Back key returns to the page you came from. Keys that follow Decka's own state show it clearly: red for a muted microphone or a recording, the accent colour for shuffle, a blinking dot while recording or on air, and a dimmed key when there is no data (no OBS, no microphone). The widgets get their place in the settings window with the key library.
- New kinds of key actions: mute the microphone, mute the sound, shuffle and repeat of the current player, and a sequence of up to 10 steps with pauses (the buttons for them arrive with the new key library). Decka now follows the volume, the mutes and the player state, so keys can change their look with them.
- Icon picker: a new main tab **Apps & brands** with about 3,400 logos (Discord, Steam, Spotify, OBS, Twitch, Chrome and more); picking one also sets the key to the brand colour if it has none.

- A new Decka logo: in the Windows installer, the app and tray icon, the Android adaptive (and themed) icon, the launch screen, the pairing screen, the settings window and the README.
- Dragging a set over the deck shows a shadow of where it will land, and a hint when it will go to a new page. Steps of a sequence can be reordered by dragging.

### Changed
- Turning the deck to portrait or back keeps every widget at a size it supports (for example a wide weather widget becomes a smaller one instead of an invalid size). Keys dim only when the Decka state they follow is missing, never for your own variables or in the editor preview.
- Variables starting with `audio.`, `mic.`, `media.` and `obs.` now belong to Decka (volume, microphone, player, OBS state): scripts can no longer write them through the HTTP API (`400 reserved variable`), but keys can follow them. The default Mute key no longer needs its own toggle variable.
- Changing the orientation keeps the reading order: widgets first, then keys, each in the order they were on the page, so neighbours like Prev / Play / Next stay together instead of being shuffled.
- Key settings look tidier: two columns in a wide window (icon on the left, name, action and colour on the right), the icon picker opens on the quick set, its grid fills the width, the icon shows its name instead of a file id, rounder controls.

## 0.2.2 — 2026-11-16

### Changed
- Smoother page swipe on the phone: the page you swipe to is no longer rebuilt at the end of the swipe.
- A nicer Windows installer in the Glass style, with a choice of English or Russian.

## 0.2.1 — 2026-11-16

### Added
- **Cut** (right-click menu or `Ctrl+X`) on a key or widget: cut it, open another page and paste it there.

### Fixed
- The icon picker in the installed app failed with "Failed to fetch phosphor icons".

## 0.2.0 — 2026-11-16

### Added
- Russian interface (window, phone, tray menu); **More → Settings → Language** or the switch in the first-run guide. **Auto** follows the system language.
- One version number for the agent and the phone, shown in the phone menu (`App 0.1.0 · PC 0.1.0`) and in **More → About**.
- The agent can check GitHub once a day for a newer release and tells you in the window and the tray; it never downloads or installs anything. Switch it off in **More → About**.
- **Phone** page shows the app version of each paired phone and warns when it differs from the PC version.

- Portrait orientation: **Look → Orientation** turns the whole deck (the grid becomes 4×7 instead of 7×4, buttons keep their places relative to the phone, what does not fit goes to a new page); the phone opens in the chosen orientation at once. The grid picker, page templates and the first-run guide know about it.

### Changed
- Glass look: the wallpaper is blurred once in advance, so it looks smooth and runs faster; **Reduce effects** now shows frosted glass instead of a flat tint.
- Big-icon labels are easier to read on bright icons; icons of programs with small pictures (for example 7-Zip) fill the key.

## 0.1.0

- Turns an old Android phone into a desk panel for a Windows PC: a grid of buttons (launch a program, hotkey, website, media keys, lock and sleep) and live widgets (clock, CPU and memory, statuses, notifications, now playing).
- Phone only ever sends which button was pressed; the PC decides what runs.
- Wi-Fi or USB (`adb reverse`) connection, pairing by QR code or 8-character code.
- Four looks: AMOLED, Studio, Material and Glass, big-icon keys, popup groups, context pages that follow the program in front.
- Screen saving for the phone: dimming, pixel shift, a screen saver clock and a night mode.
- Webhooks: `POST /api/notify` and `POST /api/status`, plus built-in website checks.
- Editor in the PC window with drag and drop, quick picker, undo and redo, autosave.
