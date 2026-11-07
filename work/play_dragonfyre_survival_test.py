"""Play the Dragonfyre survival acceptance scenario with GUI input and read-only observations."""
import argparse
import json
import math
import time

from prepare_dragonfyre_survival_test import INGREDIENTS, LAB, ROOT, WORLD, PLAYER
from verify_dragonfyre_journal_launcher import control, point, poll

OUT = ROOT/'outputs/native-stage-10'
CORE = 'cisco_mod:divine_core'
RUBY = 'cisco_mod:radiant_ruby'
COIN = 'cisco_mod:champion_coin'
HAM = 'farmersdelight:honey_glazed_ham'
TRADE_ITEMS = (CORE, RUBY, COIN, HAM)


def call(action):
    return control(LAB/'control.json', action)


def counts(*expected):
    values = call('inventory')['counts']
    return values if all(values.get(item, 0) == count
                         for item, count in zip(TRADE_ITEMS, expected, strict=True)) else None


def wait_counts(*expected):
    return poll(lambda: counts(*expected), f'expected {dict(zip(TRADE_ITEMS, expected))}')


def stable_counts(*expected):
    time.sleep(1.25)
    if counts(*expected) is None:
        raise AssertionError(call('inventory'))


def wait_screen(label):
    return poll(lambda: (s if (s := call('status'))['screen'] == label else None),
                f'expected {label}')


def screenshot(label):
    path = OUT/(label + '.png')
    call(f'screenshot {path}')
    assert path.stat().st_size > 5000


def journal():
    call('key j tap')
    wait_screen('mod')
    time.sleep(1.1)


def close():
    call('key escape tap')
    wait_screen('playing')


def journal_click(x, y):
    x, y = point(call('status')['renderer']['size'], x, y)
    call(f'click-at {x:.3f} {y:.3f}')


def aim(target):
    player = call('status')['player']
    x, y, z = player['position']
    dx, dy, dz = target[0] - x, target[1] - (y + 1.62), target[2] - z
    yaw = math.degrees(math.atan2(-dx, dz))
    pitch = -math.degrees(math.atan2(dy, math.hypot(dx, dz)))
    call(f'look {yaw:.6f} {pitch:.6f}')


def slot_of(snapshot, item):
    slot, = [entry['slot'] for entry in snapshot['slots'] if entry.get('item') == item]
    return slot


def craft(table, coins=2, ham=0):
    if call('screen').get('craft_size') != 3:
        coords = ' '.join(map(str, table))
        if call(f'getblock {coords}')['block'] != 'minecraft:crafting_table':
            call(f'place {coords} --item minecraft:crafting_table')
        aim((table[0]+0.5, table[1]+0.98, table[2]+0.5))
        hit = call('looking-at')['block']
        assert hit and hit['pos'] == list(table), hit
        call('mouse right click')
    screen = poll(lambda: (s if (s := call('screen')).get('craft_size') == 3 else None),
                  'crafting table did not open')
    recipe = (*INGREDIENTS[:4], CORE, *INGREDIENTS[4:8])
    for pos, item in enumerate(recipe):
        slot = slot_of(screen, item)
        call(f'click-slot {slot}')
        call(f'click-slot {100+pos}')
    screen = call('screen')
    assert slot_of(screen, RUBY) == 200, screen
    screenshot('03-ruby-on-workbench')
    call('click-slot 200 shift')
    wait_counts(0, 1, coins, ham)
    close()


def first(chest, table):
    wait_counts(0, 0, 0, 0)
    aim((chest[0]+0.5, chest[1]+0.48, chest[2]+0.5))
    call('mouse right click')
    screen = poll(lambda: (s if (s := call('screen')).get('container') else None),
                  'tower chest did not open')
    core_slot = slot_of(screen, CORE)
    assert core_slot >= 300
    screenshot('01-core-in-tower-chest')
    call(f'click-slot {core_slot} shift')
    wait_counts(1, 0, 0, 0)
    close()
    journal()
    screenshot('02-core-in-inventory')
    journal_click(295, 101)
    journal_click(295, 101)
    wait_counts(1, 0, 2, 0)
    close()
    craft(table)
    journal()
    journal_click(295, 57)
    journal_click(295, 57)
    wait_counts(0, 1, 5, 0)
    time.sleep(1.2)
    journal_click(251, 144)
    wait_counts(0, 1, 3, 0)
    time.sleep(1.2)
    journal_click(315, 144)
    journal_click(315, 144)
    wait_counts(0, 1, 3, 16)
    time.sleep(1.2)
    journal_click(251, 144)
    wait_counts(0, 1, 1, 16)
    stable_counts(0, 1, 1, 16)
    screenshot('04-second-trade-awaits-restart')
    return {'core_looted': True, 'ruby_crafted': True, 'counts_before_restart': counts(0, 1, 1, 16)}


def find_workbench_site():
    x, y, z = map(math.floor, call('status')['player']['position'])
    replaceable = {'minecraft:air', 'minecraft:cave_air', 'minecraft:snow[layers=1]'}
    for dx, dz in ((0, 1), (1, 0), (0, -1), (-1, 0), (0, 2), (2, 0), (0, -2), (-2, 0)):
        table = x + dx, y, z + dz
        if call(f'getblock {table[0]} {table[1]} {table[2]}')['block'] not in replaceable:
            continue
        aim((table[0]+0.5, table[1]-0.02, table[2]+0.5))
        hit = call('looking-at').get('block')
        if hit and hit.get('face') == 'up' and (
            hit.get('pos') == [table[0], table[1]-1, table[2]]
            or (hit.get('pos') == list(table) and hit.get('state') == 'minecraft:snow[layers=1]')
        ):
            return table
    raise AssertionError('No clear space for a workbench next to the player')


def resume(table):
    # A manual pass already claimed the core reward and exchanged its two coins.
    wait_counts(1, 0, 0, 16)
    craft(table or find_workbench_site(), coins=0, ham=16)
    journal()
    time.sleep(4)
    journal_click(295, 57)
    journal_click(295, 57)
    wait_counts(0, 1, 3, 16)
    time.sleep(2)
    journal_click(251, 144)
    wait_counts(0, 1, 1, 16)
    stable_counts(0, 1, 1, 16)
    screenshot('04-second-trade-awaits-restart')
    return {'ruby_crafted': True, 'counts_before_restart': counts(0, 1, 1, 16)}


def second():
    wait_counts(0, 1, 1, 16)
    journal()
    screenshot('05-pending-reward-restored')
    journal_click(295, 57)
    journal_click(295, 101)
    stable_counts(0, 1, 1, 16)
    journal_click(315, 144)
    journal_click(315, 144)
    wait_counts(0, 1, 1, 32)
    stable_counts(0, 1, 1, 32)
    screenshot('06-reward-claimed-once')
    records = WORLD/'dragonfyre-transactions-v1'/PLAYER.hex
    assert len(list(records.glob('*.txn'))) == len(list(records.glob('*.ack'))) == 6
    return {'counts_after_restart': counts(0, 1, 1, 32), 'transactions': 6}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('run', choices=('first', 'resume', 'second'))
    parser.add_argument('--chest', type=int, nargs=3)
    parser.add_argument('--table', type=int, nargs=3)
    args = parser.parse_args()
    poll(lambda: (LAB/'control.json').is_file(), 'control endpoint did not appear', limit=500)
    screens = {'playing', 'inventory'} if args.run == 'resume' else {'playing'}
    start = poll(lambda: (s if (s := call('status'))['screen'] in screens and s.get('player') else None),
                 'world did not load', limit=800)
    assert not start['creative'], start
    if args.run == 'first':
        result = first(args.chest, args.table)
    elif args.run == 'resume':
        result = resume(args.table)
    else:
        result = second()
    report = {'start': start, 'result': result}
    (OUT/f'survival-{args.run}.json').write_text(json.dumps(report, indent=2, ensure_ascii=False)+'\n',
                                                encoding='utf-8')
    call('quit')
    print(json.dumps(result, ensure_ascii=False), flush=True)


if __name__ == '__main__':
    main()
