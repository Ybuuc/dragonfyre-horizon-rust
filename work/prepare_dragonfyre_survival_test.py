"""Prepare the single disposable survival test world from the archived Stage 09 metadata."""
import argparse
import gzip
import uuid
from pathlib import Path
from shutil import copy2
from struct import unpack

from dragonfyre_nbt import Tag, decode, encode

ROOT = Path(__file__).resolve().parents[1]
LAB = ROOT/'work/stage10-survival'
SOURCE = ROOT/'work/stage09-journal/journal-test-f06684mn/game/saves/JournalTest'
WORLD = LAB/'game/instances/dragonfyre/saves/Stage10Survival'
PLAYER = uuid.UUID('8327259b-dfac-39f0-bbf5-5cf432f636c7')
INGREDIENTS = (
    'cisco_mod:dark_core', 'cisco_mod:ember_core', 'cisco_mod:wind_core',
    'cisco_mod:true_dragons_breath', 'minecraft:nether_star', 'cisco_mod:light_core',
    'cisco_mod:thunder_core', 'cisco_mod:glacial_core', 'minecraft:crafting_table',
)


def read(path):
    return decode(gzip.decompress(path.read_bytes()))


def write(path, name, tag):
    path.write_bytes(gzip.compress(encode(name, tag), mtime=0))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('seed', type=int)
    parser.add_argument('position', type=float, nargs=3)
    args = parser.parse_args()
    WORLD.mkdir(parents=True)
    name, root = read(SOURCE/'level.dat')
    level = root.value[b'Data'].value
    level[b'LevelName'] = Tag(8, b'Dragonfyre - survival verification')
    level[b'GameType'] = Tag(3, 0)
    level[b'allowCommands'] = Tag(1, 0)
    level[b'singleplayer_uuid'] = Tag(11, list(unpack('>4i', PLAYER.bytes)))
    level[b'spawn'].value[b'pos'] = Tag(11, [int(x // 1) for x in args.position])
    # Peaceful mode is only for the fixture. The tower's loot table stays identical.
    difficulty = level[b'difficulty_settings'].value
    difficulty[b'difficulty'] = Tag(8, b'peaceful')
    write(WORLD/'level.dat', name, root)
    (WORLD/'data/minecraft').mkdir(parents=True)
    name, root = read(SOURCE/'data/minecraft/world_gen_settings.dat')
    root.value[b'data'].value[b'seed'] = Tag(4, args.seed)
    write(WORLD/'data/minecraft/world_gen_settings.dat', name, root)
    copy2(SOURCE/'data/minecraft/game_rules.dat', WORLD/'data/minecraft/game_rules.dat')
    players = WORLD/'players/data'
    players.mkdir(parents=True)
    name, root = read(SOURCE/'players/data'/f'{PLAYER}.dat')
    p = root.value
    p[b'Pos'] = Tag(9, (6, args.position))
    p[b'Rotation'] = Tag(9, (5, [0.0, 0.0]))
    p[b'Motion'] = Tag(9, (6, [0.0, 0.0, 0.0]))
    p[b'Inventory'] = Tag(9, (10, [
        {b'Slot': Tag(1, slot), b'count': Tag(3, 1), b'id': Tag(8, item.encode())}
        for slot, item in enumerate(INGREDIENTS)
    ]))
    p[b'recipeBook'] = Tag(10, {})
    p.pop(b'horizon:attachments', None)
    p[b'playerGameType'] = Tag(3, 0)
    p[b'Health'] = Tag(5, 20.0)
    p[b'foodLevel'] = Tag(3, 20)
    p[b'SelectedItemSlot'] = Tag(3, 0)
    write(players/f'{PLAYER}.dat', name, root)
    print(WORLD)


if __name__ == '__main__':
    main()
