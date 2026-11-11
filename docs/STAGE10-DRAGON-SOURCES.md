# Stage 10 — исходные данные: истинное дыхание дракона и лут Эндер-дракона

Документ хранит данные, на которых построен порт рецепта `cisco_mod:true_dragons_breath`, предмета драконьей кожи и лута Эндер-дракона. Оригиналы лежат только на локальном Windows-ПК владельца проекта. В облачной среде их нет, поэтому хеши здесь **не пересчитывались**: статус данных — `REPORTED` (передано владельцем проекта 12 ноября 2026 после чтения оригиналов). Хеши совпадают с записью `true_dragons_breath_source` в `reports/AI_STATE.json` от 8 ноября.

## Источники

| № | Файл | SHA-256 | Что взято |
|---|---|---|---|
| 1 | `scripts/Cisco.zs` (оригинальная сборка Forge) | `2faed6595c1cf65d56992c5359e4c2c8747a86c1af2d4d6aec57847f3df559b4` | строки 102–114: рецепт `true_dragons_breath` |
| 2 | `irons_spellbooks-1.20.1-3.16.2.jar`, путь `data/irons_spellbooks/loot_tables/entities/additional_dragon_loot.json` | `b3a2cc3eac6990dd5badb845bb2e2b74ff89539bc54d793045ee1af6c70cd155` (хеш jar) | параметры двух пулов лута |

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

## Что неизвестно

- Сам JSON **не определяет**, как таблица подключается к `minecraft:ender_dragon` и что происходит при повторном убийстве дракона (каждое убийство или только условное). Код подключения Iron's Spellbooks 3.16.2 не прочитан: страницы исходников недоступны автоматическим запросам, jar есть только у владельца. Механика **не подтверждена**.
- Поле `type` таблицы и прочие поля вне перечисленных правил не переданы. Файл в пакете — реконструкция по правилам выше, а не побайтовая копия оригинала.

## Реализация в нативных пакетах

| Элемент | Файл | Статус |
|---|---|---|
| Оригинальный бесформенный рецепт вместо временного (4 × бутылочка дыхания) | `work/build_dragonfyre_survival_pack.py`, пакет `cisco_mod` 0.9.0 | реализовано, не загружено в Horizon |
| Предметы `irons_spellbooks:dragonskin`, `irons_spellbooks:arcane_essence` | `native/dragonfyre-chain-items/irons_spellbooks/` | реализовано; оболочка ванильных предметов-заменителей — `PARTIAL` |
| Таблица `irons_spellbooks:entities/additional_dragon_loot` с обоими пулами | там же, `data/irons_spellbooks/loot_table/entities/` | реализовано, не загружено в Horizon |
| Подключение к `minecraft:entities/ender_dragon` | там же, `data/minecraft/loot_table/entities/ender_dragon.json` | **предположение**: выпадает при каждой смерти дракона; условия оригинала неизвестны |

### Соответствие функции добычи

Оригинальная `minecraft:looting_enchant` в формате данных Horizon 26.x (в нём провайдеры чисел требуют поле `type`, как видно по конвертации таблицы башни) записывается как `minecraft:enchanted_count_increase` с `"enchantment": "minecraft:looting"` и тем же `count`. По ванильной реализации это та же арифметика: к уже выставленному количеству добавляется `round(уровень добычи × случайное из count)`, уровень берётся у убийцы. Принятие этой функции загрузчиком Horizon **не проверено**.

### Заменители внешнего вида (`PARTIAL`)

Текстур и моделей Iron's Spellbooks в пакете нет (jar недоступен в облачной среде, лицензия на распространение не проверена). Драконья кожа использует модель `minecraft:phantom_membrane`, магическая эссенция — `minecraft:prismarine_shard`. Игровая механика предметов — стопка 64 без собственных свойств; русские названия — проектные переводы, не из локализации Iron's Spellbooks.

## Установка и откат

Пакет `Dragonfyre-irons_spellbooks.hmod` нужен вместе с `Dragonfyre-cisco_mod.hmod` 0.9.0: рецепт Cisco ссылается на предметы `irons_spellbooks`. Перед заменой пакетов в профиле `Dragonfyre` сохранить копию `Dragonfyre-cisco_mod.hmod` 0.8.0 (SHA-256 `187815c8ca91b313264fd235a45b626eea7bf829d27b1e5f127f4a8811813aa8`): с ним возвращается временный рецепт.
