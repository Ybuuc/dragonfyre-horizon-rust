# Сборка Dragonfyre из чистого checkout: состояние и план устранения зависимостей

**Статус: НЕ ВОСПРОИЗВЕДЕНА из чистого клона.** Ни наличие файлов, ни прежние успешные отчёты не доказывают, что новый агент сможет собрать порт на другой машине.

## Подтверждено выводом Windows PowerShell (8 ноября 2026)

Проверка `scripts/check-stage07-build-inputs.ps1` на основном рабочем компьютере показала `Exists=True` для всех требований с `Required=True`:
- `native/dragonfyre-quests/horizon-mixin.toml`
- `native/dragonfyre-quests/engine/Cargo.toml`
- `work/build_native_stage07.py`
- `work/native-package-pristine/Cargo.toml`
- `work/mixins-target/debug/horizon-mixins.exe`
- `C:/Users/Dimash/Documents/Codex/2026-11-06/referenced-chatgpt-conversation-this-is-an/work/build-tools/rustup/toolchains/stable-x86_64-pc-windows-gnu/bin/cargo.exe`

Дополнительно присутствуют `work/native-package-applied-stage06-final/crates/horizon-client/src/game/command_harness.rs` и `work/stage07-build-4337axm1/applied/Cargo.toml`.

`gcc.exe` и `strip.exe` **не найдены исключительно по одному указанному пути** `.../build-tools/mingw64/bin/`; в проверочном сценарии они помечены как `Required=False`. Возможная установка `C:/msys64/mingw64/bin`, `C:/msys64/ucrt64/bin` или PATH **пока не исследована**. `strip` в скрипте Stage07 применяется по наличию, `gcc` может требоваться под Windows GNU при фактической линковке зависимостей.

## Почему чистый clone пока недостаточен

1. Текущий `work/build_native_stage07.py` **жёстко вычисляет** внешний каталог `referenced-chatgpt-conversation-this-is-an/work/build-tools` по относительному положению другого проекта. `cargo.exe` вне GitHub.
2. Скрипт требует уже подготовленный `work/native-package-pristine/` с Horizon и `work/mixins-target/debug/horizon-mixins.exe`, исключённые из коммитов как крупные зависимости/сборки.
3. Для instrumented сценария Stage07 скрипт читает `work/native-package-applied-stage06-final/crates/horizon-client/src/game/command_harness.rs` и подменяет generated file; это историческая диагностическая зависимость, а не доказательство самостоятельного release build.
4. Cargo используется в `--locked --offline` режиме с `CARGO_HOME=work/native-cargo`; чистый checkout без подготовленного crate cache и фиксированных native libraries не обязан успешно собираться.
5. В `work/build_dragonfyre_release.py` Stage08 также присутствуют абсолютные/исторические пути, зависимости от каталогов `work/native-game-target` и локально собранных статических библиотек `ring` и `mimalloc`.
6. В `work/verify_native_stage07.py` задан абсолютный путь к локально установленным ресурсам Horizon и уже собранному клиенту. Нужно параметризовать его для чужой машины.
7. Stage09 использует `work/native-package-pristine`, mixins, выходные и кеши Stage07/08 — это дополнительная цепочка.

## Неизменяемые версии (из прошлых отчётов, перепроверить)
- Horizon: commit `66f3981f522b24c250b21523a70080965b940424`, ресурсы `26.3`.
- horizon-mixins: из локальной папки `horizon-mixins-e8cd0aa38cffef49cb2e088ef716baa107148c02` — подтвердить origin, license и точный commit.
- Rust: Windows GNU `stable-x86_64-pc-windows-gnu`; **полная `rustc -vV`, `cargo -V` и linker config не сняты**.

## Предпочтительный конечный процесс

1. Агент клонирует `Ybuuc/dragonfyre-horizon-rust` и читает `AGENTS.md`/`ROADMAP.md`/`CURRENT_STATUS.md`.
2. Скрипт bootstrap принимает явные параметры `--horizon-source`, `--mixin-executable`, `--cargo`, `--resource-dir` или соответствующие env vars; не зависит от папок предыдущих сессий.
3. Получает pinned upstream Horizon, pinned horizon-mixins и только разрешённые к хранению входные ресурсы. Проверяет URL, license, SHA/commit, отсутствие неподтверждённых изменений.
4. Получает/проверяет Rust toolchain, linker, Cargo.lock и зависимости; локально собирает mixin tool (не выгружает чужие binary в Git).
5. Применяет **отслеживаемый** `native/dragonfyre-quests/horizon-mixin.toml` и все helper modules в рабочую временную директорию.
6. Явно разделяет режим instrumented development harness Stage06/07 и нормальный клиент; **не смешивает их**. Не переносит случайные файлы старой тестовой сборки.
7. Запускает offline/online сборку по документированной конфигурации, выполняет тесты и фиксирует sha256, exit codes и журналы.
8. Проверяет обычный Horizon Offline Launcher → Dragonfyre → Играть, затем сохранения/перезапуск. Только после этого утверждает статус VERIFIED.

## Ближайшие действия

- [x] Проверка локального наличия обязательных файлов Stage07.
- [x] Подтверждение SHA-256 равенства quest engine GitHub и generated Stage07.
- [ ] Найти фактический Windows linker/compiler, версии и конфигурацию `PATH`.
- [ ] Подтвердить первоисточники Horizon и mixins, отделить baseline от патчей.
- [ ] Составить манифест внешних входов с контрольными суммами и реальными ссылками.
- [ ] Параметризовать сборочные скрипты, убрать зависимость от `ROOT.parents[2]`.
- [ ] Выполнить сборку в **отдельном чистом клоне** (текущий профиль и миры не трогать).
- [ ] Отдельно выполнить Stage10 E2E и исправление аудио; это не часть проверки файлов Stage07.

**Запрет:** не удалять старые каталоги `work/` до доказательства, что все изменения сохранены и сборка воспроизводима без них.

## Обнаружение MSYS2 MinGW64 (вывод пользователя, 8 ноября 2026)

На текущем Windows ПК найдено:
- `C:/msys64/mingw64/bin/gcc.exe` — найден
- `C:/msys64/mingw64/bin/strip.exe` — найден
- `C:/msys64/mingw64/bin/x86_64-w64-mingw32-gcc.exe` — найден

Вывод `Get-Command gcc, strip, clang, x86_64-w64-mingw32-gcc` **пустой**: каталог, вероятно, отсутствует в текущем `PATH` PowerShell. Наличие файла не устанавливает работоспособность, версии или совместимость. Предпочтительный способ проверить — вызвать executables по абсолютному пути, вывести `gcc -dumpmachine`, `gcc --version`, `strip --version`, `rustc -vV`, `cargo -V`, а затем добавить папки временно в `$env:Path` для текущего терминала. **Не устанавливать дубликаты компилятора и не менять глобальный PATH без необходимости.**

Старая отметка "gcc/strip не найдены по build-tools/mingw64/bin" остаётся истинной исключительно для того пути; требование найти GCC на компьютере выполнено.
