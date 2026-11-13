# Stage 10 — исходные данные: истинное дыхание дракона и лут Эндер-дракона

Документ хранит данные, на которых построен порт рецепта `cisco_mod:true_dragons_breath`, предмета драконьей кожи и лута Эндер-дракона. Оригиналы лежат только на локальном Windows-ПК владельца проекта, поэтому сам контент здесь не публикуется. SHA-256 обоих источников пересчитаны на этом ПК 12 ноября 2026 и совпали с записью `true_dragons_breath_source` в `reports/AI_STATE.json`; механизм подключения лута проверен по байткоду JAR.

## Источники

| № | Файл | SHA-256 | Что взято |
|---|---|---|---|
| 1 | `scripts/Cisco.zs` (оригинальная сборка Forge) | `2faed6595c1cf65d56992c5359e4c2c8747a86c1af2d4d6aec57847f3df559b4` | строки 102–114: рецепт `true_dragons_breath` |
| 2 | `irons_spellbooks-1.20.1-3.16.2.jar`, пути `data/irons_spellbooks/loot_tables/entities/additional_dragon_loot.json` и `data/irons_spellbooks/loot_modifiers/entity_drops/ender_dragon_modifier.json` | `b3a2cc3eac6990dd5badb845bb2e2b74ff89539bc54d793045ee1af6c70cd155` (хеш jar) | параметры двух пулов лута, `append_loot` и условие по типу сущности |

Весь `Cisco.zs` и jar Iron's Spellbooks в Git не публикуются. Ниже — только рецепт и числовые параметры правил.

## Источник №1 — `Cisco.zs`, строки 102–114

```zenscript
craftingTable.addShapeless(
    "true_dragons_breath",
    <item:cisco_mod:true_dragons_breath>,
    [
        <item:irons_spellbooks:dragonskin>,
        <item:irons_spellbooks:dragonskin>,
        <item:irons_spellbooks:dragonskin>,
        <item:irons_spellbooks:dragonskin>,
        <item:minecraft:dragon_breath>,
        <item:minecraft:blaze_powder>,
        <item:minecraft:blaze_powder>
    ]
);
```

Это бесформенный рецепт: 4 × `irons_spellbooks:dragonskin`, 1 × `minecraft:dragon_breath`, 2 × `minecraft:blaze_powder`, результат — один `cisco_mod:true_dragons_breath`. Скрипт `work/check_dragonfyre_dragon_pack.py` извлекает блок выше из этого файла и сверяет его с рецептом, который собирает пакет.

## Источник №2 — `additional_dragon_loot.json` (правила)

Оригинальная таблица (формат Forge 1.20.1) содержит два независимых пула:

| Пул | Предмет | Бросков | `bonus_rolls` | Базовое количество (`minecraft:set_count`, `add: false`) | Бонус `minecraft:looting_enchant` (`count`) |
|---|---|---|---|---|---|
| 1 | `irons_spellbooks:dragonskin` | 1 | 0.0 | `uniform(6, 10)` | `uniform(0, 2)` |
| 2 | `irons_spellbooks:arcane_essence` | 1 | 0.0 | `uniform(28, 45)` | `uniform(0, 3)` |

Функции применяются по порядку: сначала `set_count`, затем бонус за добычу.

## Подключение к Эндер-дракону (проверено по JAR)

Подключение выполнено не в самой таблице, а глобальным loot-модификатором Forge:

- `data/forge/loot_modifiers/global_loot_modifiers.json` перечисляет `irons_spellbooks:entity_drops/ender_dragon_modifier`;
- `data/irons_spellbooks/loot_modifiers/entity_drops/ender_dragon_modifier.json` содержит `"type": "irons_spellbooks:append_loot"`, `"loot_table": "irons_spellbooks:entities/additional_dragon_loot"` и единственное условие `minecraft:entity_properties` с `type: minecraft:ender_dragon`;
- байткод `AppendLootModifier.doApply` просто добавляет результаты указанной таблицы к ванильному луту;
- условия «только первое убийство» нет: таблица добавляется при **каждой** смерти Эндер-дракона, в том числе при повторных убийствах.

`BossLootHandler` в JAR относится только к Dead King и Fire Boss (`DeadKingBoss`, `FireBossEntity`) и к `additional_dragon_loot` не подключён.

## Реализация в нативных пакетах

| Элемент | Файл | Статус |
|---|---|---|
| Оригинальный бесформенный рецепт вместо временного (4 × бутылочка дыхания) | `work/build_dragonfyre_survival_pack.py`, пакет `cisco_mod` 0.9.0 | VERIFIED в Horizon 13.11.2026 — крафт даёт 1 `cisco_mod:true_dragons_breath` (клиент с диагностическим логом, ингредиенты подготовлены, не выживание) |
| Предметы `irons_spellbooks:dragonskin`, `irons_spellbooks:arcane_essence` | `native/dragonfyre-chain-items/irons_spellbooks/` | реализовано; оболочка ванильных предметов-заменителей — `PARTIAL` |
| Таблица `irons_spellbooks:entities/additional_dragon_loot` с обоими пулами | там же, `data/irons_spellbooks/loot_table/entities/` | VERIFIED в Horizon 13.11.2026 — бросок при смерти дракона по правилам ниже, бонус Looting III применён |
| Подключение к `minecraft:entities/ender_dragon` | `native/dragonfyre-quests/horizon-mixin.toml` + `dragon_loot_hook.rs`, native hook `loot_table_load` v1 | VERIFIED 13.11.2026: добавляет в пустую ванильную таблицу пул с вложенной ссылкой на `additional_dragon_loot` при каждой загрузке; `random_sequence: minecraft:entities/ender_dragon` остаётся в таблице; выпадает при каждой смерти дракона, включая повторные |

Ванильная `minecraft:entities/ender_dragon` в Horizon 26.3 содержит только `type` и `random_sequence` и не имеет пулов, поэтому добавление пула не теряет ванильный лут. Поле `random_sequence` обязательно: таблица моба без него в Horizon не выдаёт ничего. Вложенная таблица использует случайный поток родительской, отдельная последовательность ей не нужна.

Horizon запрещает контент-моду добавлять файлы в `data/minecraft/` (разрешены только теги и атласы), поэтому подключение нельзя оформить data-pack-файлом: при попытке загрузчик отклоняет весь пакет Iron's Spellbooks. Native hook `loot_table_load` вызывается для каждой таблицы после data pack'ов и до первого броска; helper добавляет тот же пул, что оригинальный `append_loot`, без условия первого убийства.

### Соответствие функции добычи

Оригинальная `minecraft:looting_enchant` в формате данных Horizon 26.x записывается как `minecraft:enchanted_count_increase` с `"enchantment": "minecraft:looting"` и тем же `count`. Арифметика проверена по байткоду: оригинальный `LootingEnchantFunction` прибавляет `Math.round(level × random(count))`, Horizon делает то же самое. Уровень берётся у убийцы, при убийстве не игроком бонус не применяется.

### Нативный бросок таблицы при смерти дракона

Тик смерти дракона в Horizon порождает только опыт: ванильная таблица `minecraft:entities/ender_dragon` сама не бросается, поэтому у оригинального глобального loot-модификатора Forge не было бы точки применения. Нативный мод `dragonfyre_quests` добавляет бросок явно: хук `loot_table_load` v1 подгружает пул, а бросок делает код игры.

- `dragon_loot.rs` (`drop_loot`) один раз бросает `minecraft:entities/ender_dragon` в точке смерти дракона до того, как анимация смерти уведёт дракона вверх; зачарование добычи читается у предмета в руке убийцы (`game.inventory[game.selected]` — во время хода гостя игра подставляет его инвентарь, так что работает и для хоста, и для LAN-гостя);
- инъекция в `tick_dragon_death` вызывает бросок при `death_time == 0` с зачётом игрока: кредит `hurt_by_player` должен быть не старше 100 тиков, иначе бросок идёт без бонуса за добычу;
- инъекция в `kill_dragon` (команда `/kill`) вызывает тот же бросок при `death_time == 0`, но без зачёта игрока: у исходного `looting_enchant` уровень берётся у убийцы, а у `/kill` его нет (реализовано, отдельной игровой проверки пути `/kill` не проводилось);
- бросок выполняется один раз на смерть, дубликатов нет.

Манифест: `native/dragonfyre-quests/horizon-mixin.toml` — 15 инъекций / 8 модулей, SHA-256 `e714d8014986574f74d811b05d906f87c6fcec19cc28f539af55cac8892b17a8`. Хелперы: `dragon_loot.rs` `cecf0b5d24183940b7e41dc49baa8ccb0dee0bc877eb0ac5a87ded64859ad369`, `dragon_loot_death.rs` `6db552a37bbfcfe79b528ecb0cbc80f8961af8dc64f9eee823b944653bfa842a`, `dragon_loot_hook.rs` `0394efb31d33c3550447bb129691a141ae91b44af7d7d6933d54c885f0151fac`, `dragon_loot_kill.rs` `85962ecf83b5d2d8f5da7f73eaf3b1e78255f677e14ccd6691121956c73ac5b6`. Применение: `python scripts/apply-dragonfyre-native.py` → запуск `outputs/native-reproduction/dragonfyre-native-0_6jcdcu`, статус ok, 15 инъекций / 8 модулей, 0 расхождений с GitHub-исходниками движка (диагностическая запись броска в `%TEMP%` убрана из исходника до сборки).

### Заменители внешнего вида (`PARTIAL`)

Текстур и моделей Iron's Spellbooks в пакете нет (jar недоступен в облачной среде, лицензия на распространение не проверена). Драконья кожа использует модель `minecraft:phantom_membrane`, магическая эссенция — `minecraft:prismarine_shard`. Игровая механика предметов — стопка 64 без собственных свойств; русские названия — проектные переводы, не из локализации Iron's Spellbooks.

## Проверка в Horizon (13 ноября 2026)

Клиент проверки — release-сборка с временной диагностической записью броска в `%TEMP%\horizon-dragon-loot-debug.txt`, SHA-256 `e7aab6a944822ba4f303a0d8961544f694dedf1b5530c9721b3e164e83b3a89f`. Профиль `Dragonfyre`, мир `Stage10DragonPack`, звук выключен (`HORIZON_NO_AUDIO=1`, известный краш `0xc0000008`). Оба пакета загружены и совпадают по хешу с собранными. Перед тестами лута сохранён бэкап мира: `world-backup-before-looting-test-20261113.zip` (SHA-256 `52f541a082923056d7c6070c748adb54cdc2228826d543e6bfaa6ea78a5b80e8`).

**VERIFIED — рецепт.** Ингредиенты выданы заранее (подготовленный инвентарь, не естественное выживание). На верстаке (сетка 3×3) разложены 4 кожи, 1 дыхание, 2 огненных порошка; забор результата дал 1 `cisco_mod:true_dragons_breath`; журнал предметов показывает −4 кожи, −1 дыхания, −2 порошка; `dragon_breath` оставил стеклянную бутылку (ванильный остаток).

**Расхождение с оригиналом (fidelity gap).** `matches_all` в Horizon сверяет каждый ингредиент shapeless-рецепта с ОТДЕЛЬНОЙ стопкой: 7 ингредиентов = 7 стопок. Ваниль Minecraft принимает одну стопку из 2, закрывающую две записи. Следствие: 2 `blaze_powder` одной стопкой рецепт не собирают — их надо класть двумя стопками по 1. Ограничение движка зафиксировано, правило рецепта не менялось.

**VERIFIED — лут с Looting III.** Первое убийство дракона выполнено ударом игрока (реальные клики): кредит `("guest", 2869)`, `killed=true`, один бросок на смерть — 15 драконьих кож и 46 магической эссенции. Оба числа выше базового максимума (10 и 45), то есть бонус Looting III применён. Дропы подняты игроком; после сохранения, выхода и повторного запуска инвентарь восстановился (34 кожи, обе стопки эссенции, оба меча с Looting III).

**Ограничение — устаревший кредит урона.** При повторных убийствах кредит может оказаться устаревшим: рассинхрон неуязвимости клиент/сервер (`hurt_server_canceled` → `HURT_SERVER` → `native_living_incoming_damage`, зазор кредита >100 тиков) даёт `killed=false`, и бросок идёт по базовым числам без бонуса. Воспроизводится нестабильно: 2 из 3 повторных убийств дали базовые 8/32 и 7/44. Это ограничение движка Horizon, а не правила лута; приёмочное убийство повторяют до свежего кредита.

**BLOCKED — запуск через GUI лаунчера.** Окно лаунчера открывается, страница Dragonfyre и кнопка «Играть» на месте, но скриптованные Win32-клики не регистрируются (масштаб 1.25, GPUI-окно, 0 кнопок в UI Automation). CLI `horizon-launcher play` воспроизводит требование: `error: Sign in with a Microsoft account that owns Minecraft: Java Edition to play`. Тестовые запуски выполняются установленным клиентом через `horizon-ctl`. Свидетельства: `outputs/native-stage-10-dragon-0.9.0/play20261113-err.txt`, `launcher-gui-20261113.png`, `launcher-play-click-20261113.png`, `launcher-play-result-20261113.png`.

## Установка и откат

Пакет `Dragonfyre-irons_spellbooks.hmod` нужен вместе с `Dragonfyre-cisco_mod.hmod` 0.9.0: рецепт Cisco ссылается на предметы `irons_spellbooks`. Собранные пакеты (13 ноября 2026): Cisco 0.9.0 — SHA-256 `1187f30ec22ec5769d8f558a7729672ea932801cbcb34c427968b491f11ebac1`, Iron's Spellbooks — `1eb5550303b64d7d1b9f2299939e453314db422dfe2789956f95857443d5a97f`; установленные в профиль `Dragonfyre` копии совпадают по хешу. Хеш архива зависит от времени записи файлов в zip, поэтому при повторной сборке он меняется; содержимое проверяется чекером.

Перед заменой пакетов в профиле `Dragonfyre` сохранена копия предыдущего установленного `Dragonfyre-cisco_mod.hmod` (в отчётах — 0.8.0, в манифесте пакета `version = "0.2.0"`; SHA-256 `2abe823aace6cfe47eb2493102d129a283285a09c0d36054772941bc548f831d`): с ним возвращается временный рецепт из четырёх бутылочек дыхания. Ранее указанный в документе хеш `187815c8…` не совпал с фактическим файлом и заменён на проверенный.

## Stage 10 — проверка установленной release-сборки (13 ноября 2026)

- Чистый клиент успешно собран в профиле release (код выхода 0, 81 939 033 байта, SHA-256 `3c6f5cb243635fb2aaeb2164ea11d217f0bec6eb99bb3a1adea5d3ca6a667303`). SHA-256 установленного `horizon-client.exe` в `versions/dragonfyre` совпал с отчётом сборки.
- Хеши установленных Cisco 0.9.0 и Iron's Spellbooks совпали с хешами сборки пакетов. Проверка пакета и хуков: **78/78**. При запуске тестовой копии с включёнными модами журнал чистого клиента показал `content from 3 mod(s)` (Cisco, Iron's Spellbooks и Farmer's Delight в конфигурации). Вход в мир подтверждён отдельным запуском ниже.
- В текущей Windows-среде horizon-ctl не смог подключиться к локальному сокету (OS 10013), а доступ к окну Horizon завершился ошибкой foreground window did not report a process id. Крафт, Looting III, подбор и повторный вход на чистой сборке остаются на приёмке. Ранее подтверждённые игровые результаты относятся к диагностической сборке. Stage 10 — **PARTIAL**.

На установленном чистом release-клиенте открыть тестовую копию Stage10DragonPack с пакетами Cisco 0.9.0 и Iron's Spellbooks. Проверить регистрацию предметов и крафт с двумя отдельными стопками blaze_powder, затем убить дракона с Looting III, измерить дроп (базовые максимумы 10/45), подобрать его, сохранить мир и сверить инвентарь после перезапуска. При недоступном horizon-ctl выполнить игровой сценарий вручную. Обычный запуск через «Играть» и натуральный survival E2E остаются отдельными проверками.

### Проверка входа в мир на чистом release-клиенте

Чистый release-клиент с `--quickPlaySingleplayer Stage10DragonPack --world-frames 120` вошёл в тестовую копию мира с тремя модами и завершился с кодом 0. `smoke.json`: `error: null`, 168 кадров, 117 чанков, 1683 зарегистрированных предмета по журналу загрузки. Звук выключен. Отдельная регистрация предметов пакетов, крафт, Looting III, подбор и проверка инвентаря после перезапуска остаются на приёмке.
