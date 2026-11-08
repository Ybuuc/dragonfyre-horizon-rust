"""Build Cisco's owl tower, loot and original crafting tree for Horizon 26.3."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

from dragonfyre_nbt import Tag, decode, encode

ROOT = Path(__file__).resolve().parents[1]
DATA = 'data/cisco_mod/'
RECIPES = (
    'radiant_ruby', 'dark_core_rec', 'ember_core', 'wind_core_rec', 'light_core',
    'thunder_core', 'glacial_core', 'unattuned_core', 'ancient_essence_rec',
    'bright_essence_rec', 'arcane_apparatus_rec', 'brilliant_diamond',
    'brightsteel_alloy', 'brightsteel_alloy_rec',
)
ITEMS = {
    'dark_core': ('Dark Core', 'Тёмное ядро', 'amethyst_shard'),
    'ember_core': ('Ember Core', 'Огненное ядро', 'blaze_powder'),
    'wind_core': ('Wind Core', 'Ядро ветра', 'feather'),
    'light_core': ('Light Core', 'Ядро света', 'glowstone_dust'),
    'thunder_core': ('Thunder Core', 'Грозовое ядро', 'lapis_lazuli'),
    'glacial_core': ('Glacial Core', 'Ледяное ядро', 'prismarine_crystals'),
    'true_dragons_breath': ('True Dragon’s Breath', 'Истинное дыхание дракона', 'dragon_breath'),
    'unattuned_core': ('Unattuned Core', 'Ненастроенное ядро', 'ender_eye'),
    'ancient_essence': ('Ancient Essence', 'Древняя эссенция', 'netherite_scrap'),
    'bright_essence': ('Bright Essence', 'Светлая эссенция', 'glowstone_dust'),
    'arcane_apparatus': ('Arcane Apparatus', 'Магический прибор', 'blaze_rod'),
    'amuletof_vitality': ('Amulet of Vitality', 'Амулет жизненной силы', 'emerald'),
    'brilliant_diamond': ('Brilliant Diamond', 'Сверкающий алмаз', 'diamond'),
    'raw_brightsteel': ('Raw Brightsteel', 'Необработанная светосталь', 'raw_gold'),
    'brightsteel_ingot': ('Brightsteel Ingot', 'Слиток светостали', 'gold_ingot'),
}
TOWER_LOOT = 'cisco_mod:chests/tower_loot'


def json_bytes(data):
    return (json.dumps(data, ensure_ascii=False, indent=2) + '\n').encode()


def recipe(data):
    def ingredient(entry):
        if set(entry) == {'item'}:
            return entry['item']
        if set(entry) == {'tag'}:
            return '#' + entry['tag']
        raise ValueError(f'Unsupported ingredient {entry}')

    if 'key' in data:
        data['key'] = {key: ingredient(entry) for key, entry in data['key'].items()}
    if 'ingredients' in data:
        data['ingredients'] = [ingredient(entry) for entry in data['ingredients']]
    if 'ingredient' in data:
        data['ingredient'] = ingredient(data['ingredient'])
    result = data['result']
    if isinstance(result, str):
        data['result'] = {'id': result}
    else:
        data['result'] = {'id': result['item'], 'count': result.get('count', 1)}
    return data


def tower(data):
    name, root = decode(gzip.decompress(data))
    palette = root.value[b'palette'].value[1]
    for state in palette:
        if state[b'Name'].value == b'minecraft:chain':
            state[b'Name'].value = b'minecraft:iron_chain'
    tagged_chests = 0
    for block in root.value[b'blocks'].value[1]:
        nbt = block.get(b'nbt')
        if nbt is None:
            continue
        if nbt.value.get(b'LootTable') == Tag(8, TOWER_LOOT.encode()):
            tagged_chests += 1
            # A freshly generated chest always uses the new world's own loot roll.
            nbt.value.pop(b'LootTableSeed', None)
        # Legacy item stacks in decorative containers are outside this port.
        nbt.value.pop(b'Items', None)
    if tagged_chests != 6:
        raise ValueError(f'Expected six original tower chests, found {tagged_chests}')
    # The source uses 1.20.1 entities and item frames with legacy data.
    # Keep the original witch positions with a 26.3-compatible entity payload.
    entities = root.value[b'entities'].value[1]
    witches = [entry for entry in entities if entry[b'nbt'].value[b'id'].value == b'minecraft:witch']
    for entry in witches:
        entry[b'nbt'] = Tag(10, {b'id': Tag(8, b'minecraft:witch'), b'PersistenceRequired': Tag(1, 1)})
    root.value[b'entities'] = Tag(9, (10, witches))
    return gzip.compress(encode(name, root), mtime=0)


def content(jar):
    source = ROOT/'native/dragonfyre-chain-items/cisco_mod'
    files = {path.relative_to(source).as_posix(): path.read_bytes()
             for path in source.rglob('*') if path.is_file()}
    files['horizon-mod.toml'] = files['horizon-mod.toml'].replace(b'version = "0.1.0"', b'version = "0.4.0"')

    for item, (_, _, template) in ITEMS.items():
        base = 'minecraft:' + template
        definition = {
            'template': base, 'max_stack_size': 64,
            'components': {
                'minecraft:item_name': {'translate': f'item.cisco_mod.{item}'},
                'minecraft:item_model': base,
            },
        }
        if item == 'arcane_apparatus':
            definition.update(max_stack_size=1, rarity='epic')
            definition['components']['minecraft:max_damage'] = 50
        if item == 'amuletof_vitality':
            definition.update(max_stack_size=1, rarity='rare')
            definition['components']['minecraft:item_model'] = 'cisco_mod:amuletof_vitality'
            definition['components']['minecraft:lore'] = [
                {'translate': 'item.cisco_mod.amuletof_vitality.description'},
            ]
        files[DATA + f'horizon/item/{item}.json'] = json_bytes(definition)
    assets = 'assets/cisco_mod/'
    files[assets + 'items/amuletof_vitality.json'] = json_bytes({
        'model': {'type': 'minecraft:model', 'model': 'cisco_mod:item/amuletof_vitality'},
    })
    for path in ('models/item/amuletof_vitality.json', 'textures/item/amuvitality.png'):
        files[assets + path] = jar.read(assets + path)
    for index, lang in enumerate(('en_us', 'ru_ru')):
        path = f'assets/cisco_mod/lang/{lang}.json'
        words = json.loads(files[path])
        words.update({f'item.cisco_mod.{key}': item[index] for key, item in ITEMS.items()})
        words['item.cisco_mod.amuletof_vitality.description'] = (
            '+10 maximum health while equipped.', '+10 к максимуму здоровья при экипировке.',
        )[index]
        files[path] = json_bytes(words)

    for name in RECIPES:
        original = json.loads(jar.read(DATA + f'recipes/{name}.json'))
        files[DATA + f'recipe/{name}.json'] = json_bytes(recipe(original))
    # Keep the source Ruby recipe intact. The pre-existing dragon breath can
    # supply its unavailable mod-specific ingredient after reaching the End.
    files[DATA + 'recipe/true_dragons_breath.json'] = json_bytes({
        'type': 'minecraft:crafting_shapeless', 'category': 'misc',
        'ingredients': ['minecraft:dragon_breath'] * 4,
        'result': {'id': 'cisco_mod:true_dragons_breath'},
    })
    loot = json.loads(jar.read(DATA + 'loot_tables/chests/tower_loot.json'))
    available = ITEMS.keys() | {'divine_core', 'radiant_ruby', 'champion_coin'}
    for pool in loot['pools']:
        if isinstance(pool['rolls'], dict):
            pool['rolls']['type'] = 'minecraft:uniform'
        for index, entry in enumerate(pool['entries']):
            namespace, item = entry['name'].split(':')
            if namespace == 'cisco_mod' and item not in available:
                # Preserve the weight (and thus the odds of the original drops).
                pool['entries'][index] = {'type': 'minecraft:empty', 'weight': entry['weight']}
                continue
            for function in entry.get('functions', []):
                function['function'] = 'minecraft:set_count'
                count = function['count']
                function['count'] = (count['min'] if count['min'] == count['max']
                                     else {**count, 'type': 'minecraft:uniform'})
    files[DATA + 'loot_table/chests/tower_loot.json'] = json_bytes(loot)
    files[DATA + 'structure/owl_mage_tower.nbt'] = tower(
        jar.read(DATA + 'structures/owl_mage_tower.nbt'))
    files[DATA + 'worldgen/structure/owl_mage_tower.json'] = json_bytes({
        'type': 'minecraft:jigsaw',
        'biomes': '#cisco_mod:has_structure/owl_mage_tower',
        'step': 'surface_structures', 'spawn_overrides': {},
        'terrain_adaptation': 'beard_box', 'size': 1,
        'start_pool': 'cisco_mod:owl_mage_tower',
        'start_height': {'absolute': -1},
        'project_start_to_heightmap': 'WORLD_SURFACE_WG',
        'max_distance_from_center': 80, 'use_expansion_hack': False,
    })
    files[DATA + 'worldgen/template_pool/owl_mage_tower.json'] = json_bytes({
        'fallback': 'minecraft:empty',
        'elements': [{'weight': 1, 'element': {
            'element_type': 'minecraft:single_pool_element',
            'location': 'cisco_mod:owl_mage_tower',
            'processors': {'processors': []}, 'projection': 'rigid',
        }}],
    })
    files[DATA + 'worldgen/structure_set/owl_mage_tower.json'] = json_bytes({
        'structures': [{'structure': 'cisco_mod:owl_mage_tower', 'weight': 1}],
        'placement': {
            'type': 'minecraft:random_spread', 'spacing': 32, 'separation': 12, 'salt': 71358429,
        },
    })
    files[DATA + 'tags/worldgen/biome/has_structure/owl_mage_tower.json'] = json_bytes({
        'values': ['minecraft:' + biome for biome in (
            'plains', 'sunflower_plains', 'forest', 'birch_forest', 'flower_forest',
            'meadow', 'taiga', 'savanna', 'snowy_plains',
        )],
    })
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('jar', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'outputs/native-stage-10/Dragonfyre-cisco_mod.hmod')
    args = parser.parse_args()
    with ZipFile(args.jar) as jar:
        files = content(jar)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(args.output, 'w', compression=ZIP_DEFLATED) as pack:
        for name, data in sorted(files.items()):
            pack.writestr(name, data)
    with ZipFile(args.output) as pack:
        if pack.testzip() is not None:
            raise ValueError('Invalid content pack')
    print(args.output)
    print('SHA256', hashlib.sha256(args.output.read_bytes()).hexdigest())


if __name__ == '__main__':
    main()
