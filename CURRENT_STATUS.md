# Текущее состояние — после первичного импорта, проверка не завершена

**Последнее известное состояние из предыдущего журнала:** Stage 10, приёмочные испытания **НЕ ДОКАЗАНЫ как завершённые**. Дата архивной записи: 7 ноября 2026.

Файл исходной передачи дополнен результатами аудита пользователя. Факт наличия исходников в репозитории подтверждён Git, но **содержимое, воспроизводимость сборки и игровой E2E не проверены**. Этапы остаются `REPORTED`, пока не получены воспроизводимые результаты.

| Этап | Предмет | Статус передачи | Следующая проверка |
|---|---|---|---|
| 01–04 | Инвентаризация и основа Rust/Horizon, mixins | REPORTED | Проверить исходный код и отчёты |
| 05 | Детекция предметов, условия и сохранение квестов | REPORTED | Тесты регрессии |
| 06 | Транзакции, выдача наград, восстановление после сбоя | REPORTED | Журналы и тесты |
| 07 | Три связанных квеста, четыре предмета | REPORTED | Игровая цепочка |
| 08 | Оптимизация графики, FPS | REPORTED / PARTIAL | Повторить FPS и 1% low |
| 09 | Журнал по J, GUI наград, два обмена | REPORTED / PARTIAL | Проверить GUI; звук всё ещё проблема |
| 10 | Рубин, ядро, башня, таблица добычи | PARTIAL / UNKNOWN | Закончить gameplay E2E |

## Stage 10 — активное задание

Из журнала: `work/build_dragonfyre_survival_pack.py`, `work/check_dragonfyre_survival_pack.rs`, `work/prepare_dragonfyre_survival_test.py`, `work/play_dragonfyre_survival_test.py`; выходной пакет `outputs/native-stage-10/Dragonfyre-cisco_mod.hmod`; лог `outputs/native-stage-10/pack-probe.log`.

Есть запись о генерируемой башне Owl Mage Tower (seed `9001`) и таблице добычи Divine Core, однако финального подтверждения сквозного прохождения после перезапуска в журнале **нет**.

Для проверки нужны: генерация башни; сундук; честное получение ядра; крафт рубина; журнал по J; квесты/5 монет; два обмена на 32 порции; перезапуск; отсутствие дублей. **Отдельно** проверяется доступность ингредиентов без заранее подготовленного инвентаря.

## Известные проблемы
- Клиент с аудио падал с `0xc0000008` на двух Windows-устройствах вывода; тестовая конфигурация выключала звук.
- 70–90 FPS с `1% low >= 70` в двух боях не подтверждены; тесты были на визере, не на модовых боссах.
- В Stage 10 временно заменяли неперенесённый лут пустыми результатами с сохранением весов.
- Оставшиеся 458 квестов и крупные модовые системы ещё не подтверждены как перенесённые.

## После импорта
- [x] Подтвердить рабочий корень по локальному PowerShell-аудиту: `C:\Users\Dimash\Documents\Codex\2026-11-06\new-chat-4\work\dragonfyre`.
- [ ] Сверить SHA-256 ключевых источников и изменений относительно исходного Horizon.
- [x] Зафиксировать commit первичного импорта: `37abd08` (`main`, `origin/main` синхронизированы по локальному выводу).
- [ ] Проверить версию фактически запускаемого клиента.
- [ ] Записать выводы тестов Stage 10 (путь + hash).
- [ ] Проверить наличие исходного утверждённого roadmap.
- [ ] Отметить успешные проверки как VERIFIED только после повторного воспроизведения.

**Последняя известная рабочая директория Windows:** `C:\Users\Dimash\Documents\Codex\2026-11-06\new-chat-4\work\dragonfyre`.

## Аудит после импорта (вывод PowerShell пользователя, 8 ноября 2026)

- Ветка `main` «up to date with origin/main»; последний локальный коммит `37abd08` — `Import Dragonfyre native Rust port through Stage 10`.
- В рабочем каталоге присутствуют `DRAGONFYRE.md`, `STAGE07.md`, `STAGE08-PERFORMANCE.md`, `STAGE09-JOURNAL.md`, `STAGE10-SURVIVAL.md`, четыре скрипта Stage 10 и `outputs/native-stage-10/pack-probe.log`.
- `outputs/native-stage-10/survival-first.json` и `outputs/native-stage-10/survival-second.json` по точным проверенным путям **отсутствуют**; возможно, результаты находятся в ином месте — пока не установлено.
- `git status` сообщил о 27 неотслеживаемых каталогах `work/`, включая `horizon-66f3981/`, `mixins-tool/`, `pristine-native-base/`, `stage07-worlds-*/`, `stage07-build-*/`, `economy-stage06-*/` и другие. Это **не доказательство**, что все они временные: требуется инвентаризация скриптов, патчей и независимых изменений. Не использовать массовое удаление или `git add -A` до проверки.
- `scripts/verify-layout.ps1` — проверка существования файлов; не компиляция и не доказательство игрового прохождения.

**Приоритет следующего агента:** (1) инвентаризация неотслеживаемых деревьев и поиск уникального кода/патчей, (2) проверка, что чистый `git clone` может воспроизвести сборку из отслеживаемых исходников и документированных внешних зависимостей, (3) закончить Stage 10 с реальными логами GUI и сохраняемостью.

## Подтверждённая сверка исходников Stage 07 (PowerShell пользователя, 8 ноября 2026)

- **VERIFIED (file SHA equality):** опубликованный Rust-квестовый движок `native/dragonfyre-quests/engine` версии `0.4.0-dev` соответствует сгенерированному движку `work/stage07-build-4337axm1/applied/native-crates/dragonfyre_quests/engine` по восьми исходным файлам (Cargo.toml и семь модулей src). `Cargo.lock` присутствует только в отдельном дереве GitHub engine; источник lock для Stage07 надо подтвердить.
- `work/horizon-66f3981/crates/dragonfyre-quests-engine` — ранняя версия `0.2.0-dev`; не перезаписывать ею GitHub 0.4.0.
- **OPEN:** интеграционные различия `lan.rs`, `native_hooks.rs` и остальных файлов ещё не соотнесены полностью с mixin-инъекциями и подготовленной базой. Базовая папка `work/horizon-66f3981` уже содержит Dragonfyre-код; это не доказанно чистый upstream.
- **OPEN:** `work/build_native_stage07.py` ссылается на внешние build-tools, `work/native-package-pristine`, `work/mixins-target/debug/horizon-mixins.exe` и `work/native-package-applied-stage06-final`. Чистый clone **ещё не доказан как самодостаточный**.
- **Не выполнено:** финальная E2E-проверка Stage 10, включая `survival-first.json` и `survival-second.json` (по проверенным путям отсутствовали).

См. [Issue #2](https://github.com/Ybuuc/dragonfyre-horizon-rust/issues/2). Выше приведённые подтверждения — сравнение исходных файлов, **не успешная сборка**.

## Аудит локальных prerequisites для Stage07 (пользователь, 8 ноября 2026)

**VERIFIED — наличие на текущем Windows ПК:** `horizon-mixin.toml`, `engine/Cargo.toml`, `work/build_native_stage07.py`, `work/native-package-pristine/Cargo.toml`, `work/mixins-target/debug/horizon-mixins.exe`, внешняя Windows GNU `cargo.exe`. Также доступны необязательный Stage06 diagnostic harness и `stage07-build-4337axm1/applied/Cargo.toml`.

**UNKNOWN — compiler availability:** `gcc.exe` и `strip.exe` не обнаружены по `.../build-tools/mingw64/bin/` (оба помечены `Required=False`); другие пути, включая MSYS2, не проверены. Не делать вывод «MinGW отсутствует» по одной папке.

**BLOCKER для другого ИИ:** build script использует внешний исторический путь, prepared Horizon, compiled mixins, offline Cargo cache и Stage06/08 diagnostic artifacts, отсутствующие в GitHub. Clean clone build не проходил. Подробности: [docs/BUILD_REPRODUCIBILITY.md](docs/BUILD_REPRODUCIBILITY.md).

## Аудит MSYS2 — gcc/strip найдены (PowerShell пользователя, 8 ноября 2026)

**VERIFIED (file exists):** `C:\\msys64\\mingw64\\bin\\gcc.exe`, `strip.exe`, `x86_64-w64-mingw32-gcc.exe` доступны в каталоге MSYS2 MinGW64. `Get-Command` ничего не вывел: в текущей PowerShell-сессии инструменты не разрешаются по PATH. Версии и способность к линковке ещё не проверялись. Повторная установка не требуется. Отсутствие в старой папке `build-tools/mingw64/bin` не является блокером само по себе.

**NEXT:** проверить вывод версий и GNU target, временно установить PATH, затем устранить жёстко заданные внешние пути и проверить clean checkout без доступа к старым временным деревьям.

## Подтверждение доступности Windows GNU toolchain (пользователь, 8 ноября 2026)

**VERIFIED: launch + version/target info:** MSYS2 GCC `16.2.0` (`x86_64-w64-mingw32`), strip `2.47.20260726`, Rust `1.99.1` (`x86_64-pc-windows-gnu`, LLVM `23.1.1`), Cargo `1.99.1`. Временное добавление каталогов в `PATH` позволило всем четырём утилитам определяться через `Get-Command`.

**VERIFIED (локальная Cargo-сборка и unit-тесты):** автономный `dragonfyre_quests_engine` `0.4.0-dev` скомпилирован с Rust/Cargo 1.99.1; 34/34 unit tests PASS. **NOT VERIFIED:** сборка всего Horizon-клиента, gameplay Stage10 и clean clone. См. запись ниже.

## Автономное Rust-ядро: 34/34 теста PASSED (PowerShell пользователя, 8 ноября 2026)

**VERIFIED (по предоставленному полному выводу команды):** после синхронизации `main` до `9a1edff` пользователь запустил `scripts/test-quest-engine.ps1 -RustBin "$rust" -MinGwBin "$mingw"`.

```text
Compiling dragonfyre_quests_engine v0.4.0-dev
Finished test profile in 4.62s
running 34 tests
... 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.47s
running 0 tests (src/main.rs) ... ok
Doc-tests ... running 0 tests ... ok
PASS: standalone quest engine tests passed.
```

`cargo test --manifest-path native/dragonfyre-quests/engine/Cargo.toml --locked --offline`; Rust/Cargo 1.99.1; host `x86_64-pc-windows-gnu`. Проверены транзакции, ограничения, условия предметных задач, журнал и отказоустойчивость ядра. **Тесты не охватывают клиент Horizon, интегрированный сервер, GUI, Stage10 и аудио.** Отдельно проверить clean GitHub clone и пути сборки в другом каталоге.

Добавлен CI workflow [quest-engine-tests.yml](.github/workflows/quest-engine-tests.yml) для автономного пакета Rust. Выполнение и статус GitHub Actions требуют отдельной проверки.
