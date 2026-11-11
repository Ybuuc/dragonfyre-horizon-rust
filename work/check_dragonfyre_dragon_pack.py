"""Structural check of the original True Dragon's Breath recipe, dragonskin items and dragon loot.

Compares the packed JSON with the owner-supplied Cisco.zs block and loot rules stored in
docs/STAGE10-DRAGON-SOURCES.md. It does NOT load anything in Horizon and does not evaluate the
loot table with Horizon's engine; that still needs the Windows/Horizon check.

Usage: python work/check_dragonfyre_dragon_pack.py [Dragonfyre-cisco_mod.hmod]
"""
import json
import re
import sys
import tempfile
from collections import Counter
from pathlib import Path
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build_dragonfyre_survival_pack as pack  # noqa: E402

ROOT = pack.ROOT
SOURCES = ROOT/'docs/STAGE10-DRAGON-SOURCES.md'
TABLE = 'irons_spellbooks:entities/additional_dragon_loot'
# Owner-supplied rules: (item, base min, base max, looting min, looting max), one pool each.
RULES = (
    ('irons_spellbooks:dragonskin', 6, 10, 0, 2),
    ('irons_spellbooks:arcane_essence', 28, 45, 0, 3),
)
EXPECTED_FILES = {
    'horizon-mod.toml',
    'data/irons_spellbooks/horizon/item/dragonskin.json',
    'data/irons_spellbooks/horizon/item/arcane_essence.json',
    'data/irons_spellbooks/loot_table/entities/additional_dragon_loot.json',
    'data/minecraft/loot_table/entities/ender_dragon.json',
    'assets/irons_spellbooks/lang/en_us.json',
    'assets/irons_spellbooks/lang/ru_ru.json',
}
checks = 0


def check(condition, message):
    global checks
    if not condition:
        raise SystemExit(f'FAIL: {message}')
    checks += 1


def uniform(provider, low, high):
    return provider == {'type': 'minecraft:uniform', 'min': low, 'max': high}


def zs_recipe():
    text = SOURCES.read_text(encoding='utf-8')
    block = re.search(r'```zenscript\n(.*?)```', text, re.S)
    check(block is not None, 'docs have no zenscript block')
    call = re.search(r'addShapeless\(\s*"([^"]+)",\s*<item:([^>]+)>,\s*\[(.*?)\]\s*\)',
                     block.group(1), re.S)
    check(call is not None, 'zenscript block is not an addShapeless call')
    return call.group(1), call.group(2), re.findall(r'<item:([^>]+)>', call.group(3))


def check_recipe(recipe):
    name, result, ingredients = zs_recipe()
    check(name == 'true_dragons_breath', 'recipe name differs from Cisco.zs')
    check(recipe['type'] == 'minecraft:crafting_shapeless', 'recipe is not shapeless')
    check(recipe['result'] == {'id': result}, 'recipe result differs from Cisco.zs')
    check(Counter(recipe['ingredients']) == Counter(ingredients),
          f'ingredients {recipe["ingredients"]} differ from Cisco.zs {ingredients}')
    check(Counter(ingredients) == Counter({
        'irons_spellbooks:dragonskin': 4, 'minecraft:dragon_breath': 1,
        'minecraft:blaze_powder': 2}), 'docs block no longer holds 4 skin + 1 breath + 2 powder')


def check_loot(table):
    check(table.get('type') == 'minecraft:entity', 'dragon loot is not an entity table')
    check(set(table) == {'type', 'pools'}, 'unexpected top-level fields in dragon loot')
    pools = table['pools']
    check(len(pools) == len(RULES), 'dragon loot must have exactly two pools')
    for pool, (item, base_min, base_max, loot_min, loot_max) in zip(pools, RULES):
        check(set(pool) == {'rolls', 'bonus_rolls', 'entries'}, f'{item}: invented pool fields')
        check(pool['rolls'] == 1 and pool['bonus_rolls'] == 0.0, f'{item}: rolls/bonus_rolls')
        (entry,) = pool['entries']
        check(entry['type'] == 'minecraft:item' and entry['name'] == item, f'{item}: entry')
        check(set(entry) == {'type', 'name', 'functions'}, f'{item}: invented entry fields')
        count, looting = entry['functions']
        check(count['function'] == 'minecraft:set_count' and count['add'] is False,
              f'{item}: set_count must come first with add=false')
        check(uniform(count['count'], base_min, base_max), f'{item}: base count')
        check(looting['function'] == 'minecraft:enchanted_count_increase'
              and looting['enchantment'] == 'minecraft:looting', f'{item}: looting function')
        check(uniform(looting['count'], loot_min, loot_max), f'{item}: looting count')
        check('conditions' not in count and 'conditions' not in looting,
              f'{item}: conditions are not part of the owner-supplied rules')


def check_hook(hook):
    (pool,) = hook['pools']
    (entry,) = pool['entries']
    check(entry == {'type': 'minecraft:loot_table', 'value': TABLE}, 'dragon hook target')
    check('conditions' not in pool and 'conditions' not in entry,
          'repeat-kill conditions are unknown and must not be invented')


def check_irons_pack(path):
    with ZipFile(path) as archive:
        check(archive.testzip() is None, 'irons_spellbooks hmod is corrupt')
        check(set(archive.namelist()) == EXPECTED_FILES, f'file set {sorted(archive.namelist())}')
        read = lambda name: archive.read(name).decode('utf-8')
        check('id = "irons_spellbooks"' in read('horizon-mod.toml'), 'manifest id')
        langs = {code: json.loads(read(f'assets/irons_spellbooks/lang/{code}.json'))
                 for code in ('en_us', 'ru_ru')}
        items = set()
        for name in archive.namelist():
            if '/horizon/item/' not in name:
                continue
            item = Path(name).stem
            items.add(f'irons_spellbooks:{item}')
            data = json.loads(read(name))
            key = f'item.irons_spellbooks.{item}'
            check(data['components']['minecraft:item_name'] == {'translate': key}, f'{item} name key')
            check(data['components']['minecraft:item_model'] == data['template'], f'{item} model')
            check(all(key in lang for lang in langs.values()), f'{item} missing translation')
        check(items == {rule[0] for rule in RULES}, f'items {items}')
        check_loot(json.loads(read('data/irons_spellbooks/loot_table/entities/additional_dragon_loot.json')))
        check_hook(json.loads(read('data/minecraft/loot_table/entities/ender_dragon.json')))
    return items


def check_cisco_pack(path, items):
    with ZipFile(path) as archive:
        check('version = "0.9.0"' in archive.read('horizon-mod.toml').decode(), 'cisco version')
        recipe = json.loads(archive.read('data/cisco_mod/recipe/true_dragons_breath.json'))
    check(recipe == pack.true_dragons_breath_recipe(), 'packed recipe differs from the builder')
    used = {i for i in recipe['ingredients'] if i.startswith('irons_spellbooks:')}
    check(used <= items, f'recipe uses items the irons pack does not define: {used - items}')


def main():
    recipe = pack.true_dragons_breath_recipe()
    check_recipe(recipe)
    # Earlier stage packs are static sources; their file counts must stay as package_stage07_items.py expects.
    base = ROOT/'native/dragonfyre-chain-items'
    for namespace, count in (('cisco_mod', 6), ('farmersdelight', 4)):
        files = [p for p in (base/namespace).rglob('*') if p.is_file()]
        check(len(files) == count, f'{namespace} static pack changed: {len(files)} files')
    with tempfile.TemporaryDirectory() as scratch:
        irons = pack.irons_spellbooks_pack(Path(scratch)/'Dragonfyre-irons_spellbooks.hmod')
        items = check_irons_pack(irons)
    check({i for i in recipe['ingredients'] if i.startswith('irons_spellbooks:')} <= items,
          'recipe ingredient missing from irons pack')
    if len(sys.argv) > 1:
        check_cisco_pack(Path(sys.argv[1]), items)
    print(f'OK: {checks} checks passed' + ('' if len(sys.argv) > 1 else
          ' (Cisco hmod not given; its packed recipe was not inspected)'))


if __name__ == '__main__':
    main()
