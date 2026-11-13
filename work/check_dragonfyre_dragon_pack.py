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
import tomllib
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
    'assets/irons_spellbooks/lang/en_us.json',
    'assets/irons_spellbooks/lang/ru_ru.json',
}
MIXIN = ROOT/'native/dragonfyre-quests/horizon-mixin.toml'
HOOK_SOURCE = ROOT/'native/dragonfyre-quests/dragon_loot_hook.rs'
LOOT_MODULE = ROOT/'native/dragonfyre-quests/dragon_loot.rs'
LOOT_DEATH = ROOT/'native/dragonfyre-quests/dragon_loot_death.rs'
LOOT_KILL = ROOT/'native/dragonfyre-quests/dragon_loot_kill.rs'
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
        # Horizon 26.3 stores entry functions under `modifier` with a `type`; the 1.20.1
        # `functions`/`function` keys load but never run, which silently yields one item.
        check(set(entry) == {'type', 'name', 'modifier'}, f'{item}: invented entry fields')
        check('functions' not in entry and 'function' not in entry,
              f'{item}: legacy 1.20.1 function keys must be converted')
        count, looting = entry['modifier']
        check(count['type'] == 'minecraft:set_count' and count['add'] is False,
              f'{item}: set_count must come first with add=false')
        check(uniform(count['count'], base_min, base_max), f'{item}: base count')
        check(looting['type'] == 'minecraft:enchanted_count_increase'
              and looting['enchantment'] == 'minecraft:looting', f'{item}: looting function')
        check(uniform(looting['count'], loot_min, loot_max), f'{item}: looting count')
        check('conditions' not in count and 'conditions' not in looting,
              f'{item}: conditions are not part of the owner-supplied rules')


def check_native_hook():
    # The original is a Forge global loot modifier appended on every death. Horizon refuses
    # mod files under data/minecraft/, so the table is attached at load time through the
    # supported native hook instead. The vanilla table keeps its random_sequence; a mob table
    # without one drops nothing. No first-kill condition exists in the original.
    with MIXIN.open('rb') as stream:
        manifest = tomllib.load(stream)
    hooks = [entry for entry in manifest.get('injection', [])
             if entry.get('hook') == 'loot_table_load']
    check(len(hooks) == 1, 'expected exactly one loot_table_load injection')
    hook = hooks[0]
    check(hook.get('hook_version') == 1, 'loot_table_load must use hook version 1')
    check(hook.get('kind') == 'HEAD', 'loot_table_load injection must be HEAD')
    check(hook.get('helper') == 'dragon_loot_hook.rs', 'loot hook helper name')
    check(hook.get('bindings') == [
        {'name': 'id', 'ty': '&str', 'value': 'id'},
        {'name': 'table', 'ty': '&mut Value', 'value': '&mut *table'},
    ], 'loot hook bindings must match native_loot_table_load')
    source = HOOK_SOURCE.read_text(encoding='utf-8')
    check('minecraft:entities/ender_dragon' in source, 'hook must target the Ender Dragon table')
    check(TABLE in source, 'hook must reference the original additional loot table')
    check('"pools"' in source and '"entries"' in source and '"minecraft:loot_table"' in source,
          'hook must append a nested loot_table pool')


def check_dragon_drop():
    # Horizon's dragon death only spawns experience; the vanilla table is never rolled, so the
    # Forge global loot modifier has nothing to append to. The two injections roll the same
    # table at the dragon's death: the death flight for a player kill (with Looting from the
    # held item) and `/kill`, which removes a live dragon without the death flight.
    with MIXIN.open('rb') as stream:
        manifest = tomllib.load(stream)
    modules = {entry.get('id'): entry for entry in manifest.get('module', [])}
    check('dragon_loot' in modules, 'dragon loot module is missing from the mixin manifest')
    module = modules['dragon_loot']
    check(module.get('file') == 'crates/horizon-client/src/game/dragon.rs',
          'dragon loot module must be injected into the dragon source')
    check(module.get('name') == 'dragonfyre_dragon_loot', 'dragon loot module name')
    check(module.get('source') == 'dragon_loot.rs', 'dragon loot module source')

    injections = {entry.get('id'): entry for entry in manifest.get('injection', [])}
    expected = (
        ('drop_dragon_loot_on_death', 'tick_dragon_death', 'dragon_loot_death.rs', 'true'),
        ('drop_dragon_loot_on_kill', 'kill_dragon', 'dragon_loot_kill.rs', 'false'),
    )
    for injection_id, function, helper, credit in expected:
        check(injection_id in injections, f'{injection_id}: injection is missing')
        injection = injections[injection_id]
        check(injection.get('target')
              == f"horizon_client::game::dragon::Game<'_>::{function}",
              f'{injection_id}: target must be the dragon {function}')
        check(injection.get('kind') == 'HEAD', f'{injection_id}: only a HEAD injection runs once')
        check(injection.get('helper') == helper, f'{injection_id}: helper name')
        check(injection.get('bindings') == [
            {'name': 'game', 'ty': '&mut crate::game::Game<\'_>', 'value': '&mut *self'},
        ], f'{injection_id}: bindings must pass the game')
        helper_source = (ROOT/'native/dragonfyre-quests'/helper).read_text(encoding='utf-8')
        check('death_time == 0' in helper_source, f'{injection_id}: must roll only once')
        check(f'drop_loot(game, {credit})' in helper_source,
              f'{injection_id}: player credit must be {credit}')

    source = LOOT_MODULE.read_text(encoding='utf-8')
    check('minecraft:entities/ender_dragon' in source, 'drop must roll the vanilla dragon table')
    check('killed_by_player' in source and 'held.as_ref()' in source,
          'drop must pass the player kill and held item to the loot context')
    check('spawn_item' in source, 'drop must spawn the rolled stacks in the level')
    check('death_time' not in source, 'the module must not own the once-per-death guard')
    check(LOOT_DEATH.is_file() and LOOT_KILL.is_file(), 'dragon drop helpers are missing')


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
        check(not any(name.startswith('data/minecraft/') for name in archive.namelist()),
              'a mod may not add files under data/minecraft/')
        check_loot(json.loads(read('data/irons_spellbooks/loot_table/entities/additional_dragon_loot.json')))
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
    check_native_hook()
    check_dragon_drop()
    if len(sys.argv) > 1:
        check_cisco_pack(Path(sys.argv[1]), items)
    print(f'OK: {checks} checks passed' + ('' if len(sys.argv) > 1 else
          ' (Cisco hmod not given; its packed recipe was not inspected)'))


if __name__ == '__main__':
    main()
