<p align="center">
  <img src="docs/media/icon-256.png" width="128" alt="Horizon">
</p>

<h1 align="center">Horizon для Android / Horizon for Android</h1>

<p align="center">
  Неофициальный порт Horizon (Minecraft: Java Edition на Rust) на Android<br>
  Unofficial Android port of Horizon, Minecraft: Java Edition rewritten in Rust
</p>

<p align="center">
  <a href="https://github.com/Rayan-XOO/horizon/releases/latest"><img src="https://img.shields.io/github/v/release/Rayan-XOO/horizon?label=release" alt="Latest release"></a>
  <a href="https://github.com/Rayan-XOO/horizon/releases"><img src="https://img.shields.io/github/downloads/Rayan-XOO/horizon/total" alt="Downloads"></a>
  <img src="https://img.shields.io/badge/version-0.4.7-2EA043" alt="Version 0.4.7">
  <img src="https://img.shields.io/badge/Android-8.0%2B%20arm64%20%7C%20armv7-3DDC84?logo=android&logoColor=white" alt="Android 8.0+">
  <img src="https://img.shields.io/badge/Minecraft-Java%2026.3-5D9C3F" alt="Minecraft Java 26.3">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/Rayan-XOO/horizon" alt="License"></a>
  <a href="https://discord.gg/zjMy9Nx9T7"><img src="https://img.shields.io/badge/Discord-Horizon%20Android-5865F2?logo=discord&logoColor=white" alt="Discord"></a>
</p>

<p align="center">
  <a href="https://github.com/Rayan-XOO/horizon/releases/latest"><b>Скачать APK / Download APK</b></a> ·
  <a href="https://4pda.to/forum/index.php?showtopic=1128662">4PDA</a> ·
  <a href="https://discord.gg/zjMy9Nx9T7">Discord</a> ·
  <a href="#русский">Русский</a> ·
  <a href="#english">English</a>
</p>

> Ранняя версия, многое ещё не доделано. / Early build, plenty is still unfinished.

## Русский

### Что это

Horizon — это Minecraft: Java Edition 26.3, заново написанный на Rust (нативный клиент, не Java). Этот форк запускает его на Android-телефонах: Vulkan-рендер, сенсорное управление, загрузка файлов игры прямо на телефоне.


Это неофициальный порт [Horizon](https://github.com/horizon-rust/horizon) на Android. Проект — форк
репозитория https://github.com/horizon-rust/horizon, и вся основная работа - заслуга оригинальных авторов Horizon.
Спасибо им. Я только добавил то, что нужно для запуска на телефоне: сборку APK, сенсорное управление,
экранную клавиатуру и пару мелочей.

Оригинальный README лежит в [README.upstream.md](README.upstream.md).

### Возможности

- Нативная игра на Rust, без Java и эмуляции; рендер через Vulkan (запасной вариант GLES 3).
- Сенсорное управление: джойстик, кнопки, бег, удар с поворотом камеры одним пальцем.
- Три раскладки: «Стандарт», «Как в Bedrock» и своя; редактор кнопок (положение, размер, прозрачность)
  и выбор, крутят ли кнопки камеру.
- Вход в Microsoft по коду для лицензионных серверов (нужен свой одобренный Azure client ID), вход обновляется сам.
- Игра замирает, когда свёрнута, и не тратит батарею.
- Экранная клавиатура в чате, табличках и полях поиска (открывается, только когда нажимаешь на само поле).
- Файлы игры скачиваются в самом приложении, с докачкой после обрыва.
- Папка игры видна в «Файлах»: ресурспаки, шейдеры и миры без провода.

### Важно

- Я **никак не связан** с разработчиками Horizon, с Mojang или Microsoft. Это любительский проект.
- В репозитории и в APK **нет никаких файлов Minecraft**. Нужна своя **легальная копия Minecraft Java Edition**,
  а игровые файлы ты готовишь сам.

### Скриншоты

Скриншоты из оригинального Horizon (на ПК), игра на телефоне выглядит так же.

<p align="center">
  <img src="docs/media/survival.jpg" width="49%" alt="Выживание">
  <img src="docs/media/village.jpg" width="49%" alt="Деревня">
  <img src="docs/media/sunset.jpg" width="49%" alt="Закат">
  <img src="docs/media/underwater.jpg" width="49%" alt="Под водой">
</p>

### Установка

1. Скачай последнюю версию, **0.4.7**: [horizon-0.4.7.apk](https://github.com/Rayan-XOO/horizon/releases/download/android-v0.4.7/horizon-0.4.7.apk) (все версии на странице [Releases](https://github.com/Rayan-XOO/horizon/releases)).
2. Открой его на телефоне (разреши «установку из неизвестных источников») или `adb install -r horizon-0.4.7.apk`.
3. Нужен Android 8.0+, arm64 или 32-битный ARM (armv7).

**Обновление.** С 0.2.1 и новее обновление ставится поверх, данные остаются. Сборка 0.2.8 была подписана другим ключом: её нужно сначала удалить (сохрани папку `saves`, см. ниже), затем установить новую.

**Обновление с 0.2.0 и старше.** С версии 0.2.1 приложение подписано новым ключом, поэтому поверх старой версии оно не встанет. Старую придётся один раз удалить, а при удалении Android стирает данные игры. Перед этим скопируй миры из `Android/data/dev.horizonlab.horizon/files/saves` куда-нибудь в другое место, а после установки верни обратно.

**Если Play Защита ругается.** Google часто предупреждает о любых приложениях не из Play Маркета, которые он ещё не видел. Нажми «Подробнее» → «Всё равно установить». Код приложения открыт, он весь в этом репозитории. Если хочешь, можно отправить приложение Google на перепроверку через [официальную форму](https://support.google.com/googleplay/android-developer/contact/protectappeals).

### Файлы игры

При первом запуске приложение само скачает почти все файлы игры (около 525 МБ, лучше по Wi-Fi). Загрузка идёт в фоне, её можно свернуть, а если оборвётся, продолжится с того же места.

Есть одно но: небольшой архив `horizon-derived.zip` (около 4 МБ) телефон собрать сам не может, его делают инструменты Horizon на Java. Его нужно один раз сделать на ПК и закинуть на телефон. Выложить его сюда я не могу, потому что он получен из файлов игры.

**Как сделать архив (один раз, на ПК с Java 25 и Rust):**

1. Скачай этот репозиторий и открой в нём терминал.
2. Скачай `server.jar` версии 26.3 с официального сайта Minecraft и подготовь файлы игры:
   ```sh
   cargo run --release -p horizon-prepare -- --prepare --server-jar ~/Downloads/server.jar
   ```
3. Упакуй архив:
   ```sh
   bash packaging/android/make_derived_pack.sh
   ```
   Готовый файл появится в `target/android/horizon-derived.zip`.

**Как закинуть на телефон:**

1. Запусти Horizon один раз, чтобы Android создал папку игры.
2. Скопируй `horizon-derived.zip` в папку
   `/storage/emulated/0/Android/data/dev.horizonlab.horizon/files/`
   (через провод, файловый менеджер с доступом к `Android/data` или `adb push horizon-derived.zip /sdcard/Android/data/dev.horizonlab.horizon/files/`).
3. В приложении нажми «Повторить», загрузка дойдёт до конца, и игра запустится.

Если друг уже сделал архив, можно взять у него, он одинаковый для всех на версии 26.3.

Не удаляй приложение без нужды: Android при удалении стирает его папку вместе со скачанными файлами, сохранениями и архивом. Для обновления просто ставь новый APK поверх старого.

Папка игры видна в приложении «Файлы» как «Horizon» (на Samsung через «Файлы» от Google или любой файловый менеджер с SAF). Там лежат `resourcepacks`, `shaderpacks` и `saves`, так что ресурспаки и шейдеры можно просто туда скопировать, без провода и `Android/data`.

**Папка Horizon в корне памяти (с 0.4.7).** Лаунчер предлагает перенести файлы игры (`saves`, `mods`, `resourcepacks`, `shaderpacks`, `logs`, скачанные файлы) в папку `/storage/emulated/0/Horizon`, рядом с `Download` и `Android`. Для этого Android попросит «Доступ ко всем файлам». Всё уже скачанное переносится, ничего не теряется, и эта папка остаётся при удалении приложения. Если доступ не дать, всё остаётся в `Android/data/dev.horizonlab.horizon/files`, как раньше. Сменить можно в лаунчере: «Настройки» → «Папка игры».

Как подготовить все файлы на ПК целиком, без загрузки на телефоне, написано в [ANDROID.md](ANDROID.md).

### Управление

- Джойстик слева для движения, свайп справа — осмотреться.
- Кнопки: прыжок, присед, атака, использование, инвентарь, смена слота.
- Пауза в правом верхнем углу, кнопка **X** закрывает открытый экран.
- Кнопки **F1 / F2 / F3 / F5** (скрыть HUD, скриншот, отладка, камера).
- Экранная клавиатура открывается, когда нажимаешь на поле ввода (поиск, табличка и т.д.), а в чате сразу.
- Редактор раскладки (шестерёнка): двигай кнопки, меняй размер и прозрачность, сохраняй.
- Быстрый тап по мобу — атака.

### Моды

На телефоне работают моды `.hnmod` с готовой Android-библиотекой. Поставить мод: открой файл через Horizon («Открыть с помощью» / «Поделиться»), или **Профиль → Установить мод**, или просто положи его в папку `mods` в папке игры. Включить и выключить: **Настройки → Моды**. Готовые моды лежат в [релизах](https://github.com/Rayan-XOO/horizon/releases).

Моды — это нативный код без песочницы, ставь только то, чему доверяешь.

### Как портировать мод с ПК самому

Моды для ПК (Fabric, Forge и т.д.) на телефоне сами по себе не заработают: их надо переписать под Horizon и собрать под Android. Я моды по заявкам не порчу, но это можно сделать самому на компьютере с помощью ИИ-агента для кода: [Claude Code](https://claude.com/claude-code), [Cursor](https://cursor.com), [Codex](https://openai.com/codex) или любого похожего. Агент сам скачает всё нужное, прочитает документацию и соберёт файл.

Что нужно:

- компьютер на Windows, Linux или macOS, несколько гигабайт места и интернет;
- установленный агент (у большинства есть бесплатный пробный период или дешёвый тариф);
- исходники мода (ссылка на GitHub/GitLab) или хотя бы его `.jar`.

Как:

1. Создай пустую папку, открой в ней агента.
2. Вставь промпт ниже, подставив название мода и ссылку на него.
3. Отвечай агенту, если он что-то спросит, и разрешай ему ставить инструменты (Rust, Android NDK).
4. Когда он закончит, скинь получившийся `.hnmod` на телефон и установи, как написано выше.

Не всё можно перенести: у модов для Android пока нет доступа ко всему, что умеет игра (подробно, что есть, — в [docs/android-mods.md](docs/android-mods.md)). Агент должен сам сказать, что получилось, а что нет. Если мод сделал не ты, уважай его лицензию: оставь автора и ссылку на оригинал, а выкладывать порт можно, только если лицензия это разрешает.

**Промпт (скопируй целиком):**

```text
Помоги портировать мод для Minecraft: Java Edition на Horizon для Android (неофициальный порт Horizon, Minecraft Java 26.3 на Rust). Результат: файл .hnmod, который ставится в Horizon на телефоне.

Мод: <НАЗВАНИЕ МОДА>
Исходники: <ССЫЛКА НА РЕПОЗИТОРИЙ МОДА или путь к .jar>

Порядок работы:
1. Склонируй https://github.com/Rayan-XOO/horizon (ветка android-port) в эту папку. Прочитай docs/android-mods.md, crates/horizon-native-abi, crates/horizon-mod-sdk и примеры mods/hello-android, mods/entity-culling и mods/orbital-laser (orbital-laser — уже портированный мод, ориентируйся на него).
2. Скачай исходники мода. Если есть только .jar, декомпилируй его (CFR или Vineflower) только для того, чтобы понять логику.
3. Прежде чем писать код, дай мне короткий список: что умеет мод, что из этого можно сделать через API Horizon для Android, а что нельзя и почему. Подожди моего ответа.
4. Напиши мод на Rust как cdylib на horizon-mod-sdk (или на horizon-native-abi, если мод простой). Саму игру Horizon не меняй: всё, что нужно моду (код, текстуры, звуки, данные, язык), должно лежать в его .hnmod.
5. Не клади в мод файлы Minecraft/Mojang (текстуры, звуки, код игры). Ресурсы бери только из самого мода, сохрани его LICENSE и автора.
6. Поставь недостающие инструменты: Rust (rustup), rustup target add aarch64-linux-android, cargo install cargo-ndk, Android NDK (задай ANDROID_NDK_HOME), cargo install --path crates/cargo-horizon-mod.
7. Собери: cargo horizon-mod android (в папке мода). Исправляй ошибки, пока сборка не пройдёт. Готовый файл: target/horizon-mod/<id>.hnmod.
8. В конце напиши: где лежит .hnmod, что работает, что не перенеслось и почему, и как проверить мод в игре (команды, клавиши, что должно появиться на экране).
```

Если мод не работает, загляни в **Настройки → Моды**: там написано, почему он не загрузился. Логи можно отправить агенту, он поправит.

### Обновления и новости

- Все версии выходят здесь, на GitHub: [Releases](https://github.com/Rayan-XOO/horizon/releases). Чтобы получать уведомления о новых версиях, нажми вверху **Watch → Custom → Releases**.
- Новый APK ставится поверх старого, данные и миры остаются.
- Тема на 4PDA: [Horizon для Android](https://4pda.to/forum/index.php?showtopic=1128662) — там новости, вопросы и помощь.
- Сервер в Discord: [Horizon Android](https://discord.gg/zjMy9Nx9T7) — общение, помощь, моды и раскладки управления.
- Нашёл баг? Пиши в [Issues](https://github.com/Rayan-XOO/horizon/issues) или в теме на 4PDA и приложи логи (кнопка «Поделиться логами» в лаунчере).

### Поддерживаемые устройства

Порту нужны **arm64** или 32-битный **ARM (armv7)**, **Android 8.0+ (API 26)** и GPU с **Vulkan** (без него игра пойдёт на OpenGL ES 3).
Желательно **4 ГБ ОЗУ и больше**: игра занимает около 2,2 ГБ, на слабых телефонах ставьте дальность прорисовки 4–6.

| Устройство / чип | GPU | Статус |
|---|---|---|
| Samsung Galaxy S25 | Adreno 830 | ✅ Проверено |
| Vivo Y31 4G (Snapdragon 662) | Adreno 610 | ✅ Проверено |
| Samsung Galaxy S24 FE (Exynos 2400e) | Xclipse 940 | ✅ Проверено |
| Samsung Galaxy S20+ (Snapdragon 865) | Adreno 650 | ✅ Проверено |
| Samsung Galaxy S20+ (Exynos 990) | Mali-G77 | ✅ Проверено |
| Samsung Galaxy M54 5G (Exynos 1380) | Mali-G68 | ✅ Проверено |
| Samsung Galaxy S6 (Exynos 7420) | Mali-T760 | ✅ Проверено |


### Известные проблемы

- Ест около **2,2 ГБ ОЗУ**, на слабых телефонах может вылетать. Попробуй дальность прорисовки 4-6.
- Это **ранняя, экспериментальная** версия: баги будут. Логи проще всего отправить кнопкой «Поделиться логами» в лаунчере (zip с логами и отчётами о вылетах). Сам лог: `logs/latest.log` в папке игры (`/storage/emulated/0/Horizon` или `/storage/emulated/0/Android/data/dev.horizonlab.horizon/files`), отчёты о вылетах рядом в `crash-reports/`. Открывается любым файловым менеджером (на новых Android папка Android/data видна не всем менеджерам, тогда нужен менеджер с доступом к ней или `adb pull`).

### Частые вопросы

**Play Защита пишет, что приложение небезопасно.** Так бывает с любым APK не из Play Маркета. «Подробнее» → «Всё равно установить». Весь код открыт в этом репозитории.

**Какие разрешения нужны?** Интернет и состояние сети (скачать файлы игры), уведомления и фоновая работа (чтобы загрузка не прерывалась в свёрнутом виде). «Доступ ко всем файлам» нужен, только если сам захочешь хранить игру в папке `/storage/emulated/0/Horizon`; без него всё лежит в папке приложения.

**Нужна ли лицензия Minecraft?** Да, нужна своя легальная копия Minecraft: Java Edition. В APK нет ни одного файла игры.

**Будет ли работать на моём телефоне?** Нужны Android 8.0+, arm64 и желательно от 4 ГБ ОЗУ. Список устройств — в разделе «Поддерживаемые устройства».

### Сборка

Инструкции по сборке APK в [ANDROID.md](ANDROID.md).

### Благодарности

Вся основная работа над игрой сделана авторами [Horizon](https://github.com/horizon-rust/horizon) ([playhorizon.dev](https://playhorizon.dev)).

### Лицензия

MIT, как и оригинал. Файл [LICENSE](LICENSE) и копирайт оригинальных авторов не тронуты.

---

## English

### What is this

Horizon is Minecraft: Java Edition 26.3 rewritten from scratch in Rust (a native client, no Java). This fork runs it on Android phones: Vulkan rendering, touch controls and an in-app game file download.


This is an unofficial Android port of [Horizon](https://github.com/horizon-rust/horizon). It's a fork of
https://github.com/horizon-rust/horizon, and almost all the real work belongs to the original Horizon authors,
thanks to them. I just added what's needed to run it on a phone: APK packaging, touch controls, an on-screen
keyboard and a few small things.

The original README lives in [README.upstream.md](README.upstream.md).

### Features

- Native Rust game, no Java VM or emulation; Vulkan rendering with a GLES 3 fallback.
- Touch controls: joystick, buttons, sprint toggle, attack while turning the camera with one finger.
- Three layouts: Standard, Bedrock-style and your own; a button editor (position, size, opacity)
  and a switch for whether buttons also turn the camera.
- Microsoft sign-in by code for licensed servers (needs your own approved Azure client ID); it renews by itself.
- The game freezes while it's in the background, so it doesn't drain the battery.
- On-screen keyboard for chat, signs and search fields (it opens only when you tap the field itself).
- Game files are downloaded inside the app and resume after interruptions.
- Game folder shows up in the Files app: resource packs, shaders and worlds without a cable.

### Heads up

- I'm **not affiliated** with the Horizon developers, Mojang or Microsoft. This is a hobby project.
- **No Minecraft files are included** in this repo or the APK. You need your own **legit copy of Minecraft Java
  Edition** and you prepare the game files yourself.

### Screenshots

Screenshots from the original Horizon on PC; the phone build renders the same game.

<p align="center">
  <img src="docs/media/survival.jpg" width="49%" alt="Survival">
  <img src="docs/media/village.jpg" width="49%" alt="Village">
  <img src="docs/media/sunset.jpg" width="49%" alt="Sunset">
  <img src="docs/media/underwater.jpg" width="49%" alt="Underwater">
</p>

### Install

1. Grab the latest version, **0.4.7**: [horizon-0.4.7.apk](https://github.com/Rayan-XOO/horizon/releases/download/android-v0.4.7/horizon-0.4.7.apk) (every version is on [Releases](https://github.com/Rayan-XOO/horizon/releases)).
2. Open it on your phone (allow "install unknown apps") or run `adb install -r horizon-0.4.7.apk`.
3. Needs Android 8.0+ on arm64 or 32-bit ARM (armv7).

**Upgrading.** From 0.2.1 or newer it installs over the top and keeps your data. The 0.2.8 build was signed with a different key: uninstall it first (back up `saves`, see below), then install the new one.

**Upgrading from 0.2.0 or older.** Starting with 0.2.1 the app is signed with a new key, so it won't install over the old one. Uninstall the old version once; Android wipes the game data when you do. Copy your worlds from `Android/data/dev.horizonlab.horizon/files/saves` somewhere safe first and put them back after installing.

**If Play Protect complains.** Google often warns about apps from outside the Play Store that it hasn't seen before. Tap "More details" → "Install anyway". The app is open source; all of the code is in this repo. If you like, you can ask Google to re-check it through the [official form](https://support.google.com/googleplay/android-developer/contact/protectappeals).

### Game files

On first launch the app downloads almost all game files by itself (about 525 MB, Wi-Fi recommended). It runs in the background, and if it gets interrupted it picks up where it left off.

One catch: a small archive, `horizon-derived.zip` (about 4 MB), can't be built on the phone because Horizon makes it with Java tools. You need to build it once on a PC and copy it to the phone. I can't upload it here because it's derived from the game files.

**Building the archive (once, on a PC with Java 25 and Rust):**

1. Clone this repo and open a terminal in it.
2. Download the 26.3 `server.jar` from the official Minecraft site and prepare the game files:
   ```sh
   cargo run --release -p horizon-prepare -- --prepare --server-jar ~/Downloads/server.jar
   ```
3. Pack the archive:
   ```sh
   bash packaging/android/make_derived_pack.sh
   ```
   The file ends up in `target/android/horizon-derived.zip`.

**Copying it to the phone:**

1. Launch Horizon once so Android creates the game folder.
2. Copy `horizon-derived.zip` to
   `/storage/emulated/0/Android/data/dev.horizonlab.horizon/files/`
   (over USB, with a file manager that can open `Android/data`, or `adb push horizon-derived.zip /sdcard/Android/data/dev.horizonlab.horizon/files/`).
3. Tap "Retry" in the app. The download finishes and the game starts.

If a friend already built the archive, you can just take theirs; it's the same for everyone on 26.3.

Don't uninstall the app unless you have to: Android wipes its folder, including downloaded files, saves and the archive. To update, just install the new APK over the old one.

The game folder also shows up in the Files app as "Horizon" (on Samsung use Google's "Files" app or any file manager with SAF). Inside are `resourcepacks`, `shaderpacks` and `saves`, so you can just drop resource packs and shaders in there, no cable or `Android/data` needed.

**A Horizon folder in the storage root (since 0.4.7).** The launcher offers to move the game files (`saves`, `mods`, `resourcepacks`, `shaderpacks`, `logs`, the downloaded files) to `/storage/emulated/0/Horizon`, next to `Download` and `Android`. Android asks for "All files access" for that. Everything already downloaded moves over, nothing is lost, and the folder stays when the app is uninstalled. Without the permission everything stays in `Android/data/dev.horizonlab.horizon/files`, as before. Change it in the launcher: "Settings" → "Game folder".

To prepare everything on a PC without the on-phone download, see [ANDROID.md](ANDROID.md).

### Controls

- Joystick on the left to move, swipe on the right to look around.
- Buttons: jump, sneak, attack, use, inventory, next slot.
- Pause in the top-right corner, **X** closes the open screen.
- **F1 / F2 / F3 / F5** buttons (hide HUD, screenshot, debug, camera).
- The on-screen keyboard opens when you tap a text field (search, sign, etc.); in chat it opens right away.
- Layout editor (gear icon): move buttons, resize them, change opacity, save.
- Quick tap on a mob attacks it.

### Mods

Mods in `.hnmod` files that carry an Android library run on the phone. To install one, open it with Horizon ("Open with" / "Share"), use **Profile → Install a mod**, or drop it into the `mods` folder in the game folder. Turn mods on and off in **Options → Mods**. Ready-made mods are attached to the [releases](https://github.com/Rayan-XOO/horizon/releases).

Mods are native code with no sandbox, so only install ones you trust.

### Porting a PC mod yourself

PC mods (Fabric, Forge and so on) don't just run on the phone: they have to be rewritten for Horizon and built for Android. I don't port mods on request, but you can do it yourself on a computer with an AI coding agent: [Claude Code](https://claude.com/claude-code), [Cursor](https://cursor.com), [Codex](https://openai.com/codex) or anything similar. The agent fetches what it needs, reads the docs and builds the file.

You need:

- a Windows, Linux or macOS computer with a few GB of free space and internet;
- one of those agents installed;
- the mod's source (a GitHub/GitLab link) or at least its `.jar`.

Steps:

1. Make an empty folder and open the agent in it.
2. Paste the prompt below with the mod's name and link filled in.
3. Answer its questions and let it install tools (Rust, the Android NDK).
4. When it's done, copy the `.hnmod` to your phone and install it as above.

Not everything can be ported: Android mods don't yet have access to everything the game can do (what's there is listed in [docs/android-mods.md](docs/android-mods.md)), and the agent should tell you what made it and what didn't. If the mod isn't yours, respect its license: keep the author and a link to the original, and only share the port if the license allows it.

**Prompt (copy all of it):**

```text
Help me port a Minecraft: Java Edition mod to Horizon for Android (an unofficial Android port of Horizon, Minecraft Java 26.3 rewritten in Rust). The result is a .hnmod file that installs in Horizon on the phone.

Mod: <MOD NAME>
Source: <LINK TO THE MOD'S REPOSITORY or path to the .jar>

How to work:
1. Clone https://github.com/Rayan-XOO/horizon (branch android-port) into this folder. Read docs/android-mods.md, crates/horizon-native-abi, crates/horizon-mod-sdk and the examples mods/hello-android, mods/entity-culling and mods/orbital-laser (orbital-laser is an already ported mod, use it as the model).
2. Get the mod's source. If there's only a .jar, decompile it (CFR or Vineflower) just to understand the logic.
3. Before writing code, give me a short list: what the mod does, what of it the Horizon Android API can do, and what it can't and why. Wait for my answer.
4. Write the mod in Rust as a cdylib on horizon-mod-sdk (or horizon-native-abi if the mod is simple). Don't change the Horizon game itself: everything the mod needs (code, textures, sounds, data, language files) goes into its .hnmod.
5. Don't put any Minecraft/Mojang files in the mod (textures, sounds, game code). Take assets only from the mod itself, and keep its LICENSE and author.
6. Install missing tools: Rust (rustup), rustup target add aarch64-linux-android, cargo install cargo-ndk, the Android NDK (set ANDROID_NDK_HOME), cargo install --path crates/cargo-horizon-mod.
7. Build: cargo horizon-mod android (in the mod's folder). Fix errors until the build passes. The file ends up in target/horizon-mod/<id>.hnmod.
8. At the end tell me: where the .hnmod is, what works, what didn't port and why, and how to check the mod in game (commands, keys, what should show up on screen).
```

If the mod doesn't work, check **Options → Mods**: it says why the mod didn't load. You can hand the logs to the agent and it'll fix it.

### Updates and news

- Every version comes out here on GitHub: [Releases](https://github.com/Rayan-XOO/horizon/releases). To get notified about new versions, click **Watch → Custom → Releases** at the top.
- A new APK installs over the old one; your data and worlds stay.
- 4PDA topic (Russian): [Horizon for Android](https://4pda.to/forum/index.php?showtopic=1128662), for news, questions and help.
- Discord server: [Horizon Android](https://discord.gg/zjMy9Nx9T7), for chat, help, mods and control layouts (Russian and English channels).
- Found a bug? Open an [issue](https://github.com/Rayan-XOO/horizon/issues) or post in the 4PDA topic, and attach logs (the launcher's "Share logs" button).

### Supported Devices

The port requires **arm64** or 32-bit **ARM (armv7)**, **Android 8.0+ (API 26)**, and a GPU with **Vulkan** support (without it the game falls back to OpenGL ES 3).
**4 GB of RAM or more** is recommended: the game takes up about 2.2 GB; on lower-end phones, set the draw distance to 4–6.

| Device / Chip | GPU | Status |
|---|---|---|
| Samsung Galaxy S25 | Adreno 830 | ✅ Tested |
| Vivo Y31 4G (Snapdragon 662) | Adreno 610 | ✅ Tested |
| Samsung Galaxy S24 FE (Exynos 2400e) | Xclipse 940 | ✅ Verified |
| Samsung Galaxy S20+ (Snapdragon 865) | Adreno 650 | ✅ Verified |
| Samsung Galaxy S20+ (Exynos 990) | Mali-G77 | ✅ Verified |
| Samsung Galaxy M54 5G (Exynos 1380) | Mali-G68 | ✅ Verified |
| Samsung Galaxy S6 (Exynos 7420) | Mali-T760 | ✅ Verified |

### Known issues

- Uses around **2.2 GB of RAM**, weaker phones may crash. Try render distance 4-6.
- This is an **early, experimental** build, so expect bugs. The easiest way to send logs is the launcher's "Share logs" button (a zip with the logs and crash reports). The log itself: `logs/latest.log` in the game folder (`/storage/emulated/0/Horizon` or `/storage/emulated/0/Android/data/dev.horizonlab.horizon/files`), crash reports next to it in `crash-reports/`. Any file manager can open it (on newer Android, Android/data needs a file manager that has access to it, or use `adb pull`).

### FAQ

**Play Protect says the app is unsafe.** That happens with most APKs from outside the Play Store. Tap "More details" → "Install anyway". All the code is in this repo.

**What permissions does it need?** Internet and network state (to download the game files), notifications and a foreground service (so the download keeps going in the background). "All files access" only if you choose to keep the game in `/storage/emulated/0/Horizon`; without it everything lives in the app's own folder.

**Do I need to own Minecraft?** Yes, your own legit copy of Minecraft: Java Edition. The APK contains no game files.

**Will it run on my phone?** Android 8.0+, arm64, ideally 4 GB of RAM or more. See "Supported devices" above.

### Building

APK build instructions are in [ANDROID.md](ANDROID.md).

### Credits

The game itself is the work of the [Horizon](https://github.com/horizon-rust/horizon) authors ([playhorizon.dev](https://playhorizon.dev)).

### License

MIT, same as the original. The [LICENSE](LICENSE) file and the original authors' copyright are left untouched.

---

Автор порта / Port author: [Rayan-XOO](https://github.com/Rayan-XOO)
