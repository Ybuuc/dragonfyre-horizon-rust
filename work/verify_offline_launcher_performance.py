"""Run a repeatable arena workload through the installed Horizon Offline Launcher."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import socket
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'outputs/native-stage-08-performance'
LAB = ROOT / 'work/perf-offline-launcher'
TAG = 'nightly-2026-11-05-66f3981'
VERSION = LAB / 'launcher/versions' / TAG
GAMES = LAB / 'game'
INSTALLED = Path(r'C:/Users/Dimash/AppData/Roaming/horizon-launcher/versions') / TAG
PROFILE = Path(r'C:/Users/Dimash/AppData/Roaming/horizon')
LAUNCHER = Path(r'C:/Users/Dimash/AppData/Local/Programs/HorizonOfflineLauncher/horizon-offline-launcher.exe')


def control(path, command):
    info = json.loads(path.read_text(encoding='utf-8'))
    with socket.create_connection(('127.0.0.1', info['port']), timeout=8) as connection:
        stream = connection.makefile('rwb')
        stream.write(f"auth {info['token']}\n".encode())
        stream.flush()
        assert 'ok' in json.loads(stream.readline())
        stream.write((command + '\n').encode())
        stream.flush()
        reply = json.loads(stream.readline())
    if 'error' in reply:
        raise RuntimeError(f"{command}: {reply['error']}")
    return reply


def options(source, destination, overrides):
    keys = set(overrides)
    previous = source.read_text(encoding='utf-8-sig').splitlines() if source.is_file() else []
    rows = [row for row in previous if row.partition(':')[0] not in keys]
    rows.extend(f'{key}:{value}' for key, value in overrides.items())
    destination.write_text('\n'.join(rows) + '\n', encoding='utf-8')


def measure(call, seconds):
    call('stats reset')
    time.sleep(seconds)
    raw = call('stats raw')
    ms = sorted(t for t in raw['frame_ms'] if t > 0)
    assert len(ms) >= 20, f'Only {len(ms)} rendered frames'
    p99 = ms[math.ceil(0.99 * len(ms)) - 1]
    return {
        'fps': 1000 / statistics.mean(ms),
        'fps_p99': 1000 / p99,
        'fps_1pct_low': 1000 / statistics.mean(ms[-max(1, math.ceil(len(ms) * 0.01)):]),
        'p99_frame_ms': p99,
        'frames_beyond_70_fps_budget': sum(t > 1000 / 70 for t in ms),
        'raw': raw,
        'status_after': call('status'),
    }


def setup_fight(call, x, y, z, add, sustained):
    add(f'summon minecraft:wither {x+.5} {y+2} {z+13.5} {{PersistenceRequired:1b,Tags:["dfq_bench"]}}')
    for row in range(3):
        for col in range(8):
            name = 'wither_skeleton' if (row + col) % 2 == 0 else 'husk'
            add(f'summon minecraft:{name} {x-7+2*col+.5} {y+1} {z+6+3*row+.5} '
                '{NoAI:1b,PersistenceRequired:1b,Tags:["dfq_bench"]}')
    for dx in (-5, 0, 5):
        add(f'summon minecraft:iron_golem {x+dx+.5} {y+1} {z+18.5} '
            '{PersistenceRequired:1b,Tags:["dfq_bench"]}')
    if sustained:
        add('effect give @e[tag=dfq_bench] minecraft:resistance 120 4 true')
    add(f'setblock {x} {y-2} {z+5} minecraft:repeating_command_block'
        '{auto:1b,Command:"particle minecraft:flame '
        f'{x+.5} {y+3} {z+10.5} 4 2 4 0.025 16 force'
        '"}')
    return call('status')


def prepare_launcher():
    if (VERSION/'.local/horizon-client/26.3/horizon-resources.json').is_file():
        return
    VERSION.mkdir(parents=True,exist_ok=True)
    for name in ('.installed','.horizon-game-architecture.json','PLAY.txt','research-lock.json'):
        if (INSTALLED/name).is_file():
            shutil.copy2(INSTALLED/name,VERSION/name)
    shutil.copytree(INSTALLED/'.local',VERSION/'.local',dirs_exist_ok=True)


def run(args):
    prepare_launcher()
    home = Path(tempfile.mkdtemp(prefix=f'{args.label}-', dir=LAB))
    binary = Path(args.binary).resolve(strict=True)
    launcher_client = VERSION / 'horizon-client.exe'
    if not launcher_client.is_file() or hashlib.sha256(launcher_client.read_bytes()).digest()!=hashlib.sha256(binary.read_bytes()).digest():
        shutil.copy2(binary, launcher_client)
    GAMES.mkdir(exist_ok=True)
    world = home.name
    warm_cache = LAB/'smoke/.horizon-cache'
    if warm_cache.is_dir() and not (GAMES/'.horizon-cache').exists():
        shutil.copytree(warm_cache,GAMES/'.horizon-cache')
    endpoint = home / 'control.json'
    if args.world_template:
        shutil.copytree(Path(args.world_template).resolve(strict=True),GAMES/'saves'/world,
                        ignore=shutil.ignore_patterns('session.lock'))
    config = {'graphicsPreset':'"custom"', 'renderDistance':args.distance,
              'simulationDistance':args.simulation, 'maxFps':args.fps_limit, 'enableVsync':'false',
              'fullscreen':'false', 'pauseOnLostFocus':'false', 'inactivityFpsLimit':'"minimized"'}
    graphics = {'preset':'custom', 'resolutionScale':args.scale, 'graphicsApi':args.api}
    if args.pacing:
        graphics['framePacing'] = args.pacing
    if args.tuned:
        config.update(particles=1, entityShadows='false', biomeBlendRadius=1, renderClouds='"false"')
        graphics.update(entityCulling='async', particleCulling='async', weather='false')
    profile = LAB/'profile-snapshot'
    profile.mkdir(exist_ok=True)
    for name in ('options.txt','horizon-graphics.txt'):
        if not (profile/name).exists():
            shutil.copy2(PROFILE/name,profile/name)
    options(profile/'options.txt', home/'options.txt', config)
    options(profile/'horizon-graphics.txt', home/'horizon-graphics.txt', graphics)
    env = os.environ.copy()
    env.update(HORIZON_HOME=str(home), HORIZON_LAUNCHER_HOME=str(LAB/'launcher'),
               XDG_DATA_HOME=str(home/'xdg-data'), XDG_CONFIG_HOME=str(home/'xdg-config'),
               HORIZON_LAUNCHER_TRASH=str(home/'trash'), HORIZON_LAUNCHER_CLIPBOARD=str(home/'clipboard'),
               HORIZON_LAUNCHER_DESKTOP=str(home/'desktop'), HORIZON_LAUNCHER_NO_SELF_UPDATE='1',
               HORIZON_NO_AUDIO='1', HORIZON_TELEMETRY_URL='off')
    env.pop('HORIZON_COMMAND_SCRIPT', None)
    cmd = [str(LAUNCHER),'play','--version',TAG,'--world',world,'--creative']
    game = ['--game-dir',str(GAMES),'--home',str(home),'--player-dir',str(home),'--name','DFQPerf',
            '--width','1280','--height','720','--seed','1234',
            '--control-socket',str(endpoint),'--no-pause','--weather','clear','--time','6000']
    if args.offscreen:
        game.append('--offscreen')
    if args.test_terrain:
        game.append('--test-terrain')
    for mod in sorted((ROOT/'outputs/native-stage-07').glob('*.hmod')):
        game.extend(['--dev-mod',str(mod)])
    cmd.extend('--arg='+value for value in game)
    out = OUT/f'{args.label}.json'
    receipt = {'launcher':str(LAUNCHER),'binary':str(binary),
               'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
               'settings':config,'graphics':graphics,'offscreen':args.offscreen,
               'test_terrain':args.test_terrain,'sustained':args.sustained,'world_template':args.world_template,'home':str(home),'command':cmd,'commands':[],
               'mode':'diagnostic' if args.diagnostic else 'comparison'}
    call = lambda command: control(endpoint, command)

    def add(command):
        answer = call('chat /'+command)
        receipt['commands'].append([command, answer])

    def save():
        out.write_text(json.dumps(receipt,indent=2,ensure_ascii=False)+'\n', encoding='utf-8')

    print('Offline Launcher run:',args.label,flush=True)
    with (OUT/f'{args.label}.log').open('w', encoding='utf-8') as log:
        proc = subprocess.Popen(cmd, env=env, cwd=home, stdout=log, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + 1200
            status = None
            while time.monotonic() < deadline:
                if proc.poll() is not None:
                    raise RuntimeError(f'Launcher exited early: {proc.returncode}')
                if endpoint.is_file():
                    try:
                        status = call('status')
                        if status.get('screen')=='playing' and status.get('player') and status.get('renderer'):
                            break
                    except PermissionError:
                        raise
                    except (OSError, ValueError):
                        pass
                time.sleep(1)
            else:
                raise TimeoutError('No rendered world ready through the launcher')
            receipt['entry_status'] = status
            save()
            if args.entry_only:
                return
            for command in ['gamerule spawn_mobs false','gamerule mob_griefing false',
                            'gamerule advance_time false','gamerule command_block_output false',
                            'time set 6000', 'difficulty hard']:
                add(command)
            x, y, z = -526, 97, 64
            add(f'fill {x-14} {y} {z-4} {x+14} {y} {z+28} smooth_stone')
            add(f'setblock {x} {y-2} {z+5} minecraft:air')
            add(f'tp @s {x+.5} {y+2} {z+.5} 0 6')
            deadline = time.monotonic() + args.settle_limit
            min_chunks = int(0.9 * (2 * args.distance + 1)**2)
            stable_samples = 0
            previous = None
            while time.monotonic() < deadline:
                status = call('status')
                chunks = status['chunks']
                if chunks >= min_chunks and status['pending_chunks'] == 0 and chunks == previous:
                    stable_samples += 1
                    if stable_samples >= 3:
                        break
                else:
                    stable_samples = 0
                previous = chunks
                time.sleep(1)
            receipt['world_settled'] = stable_samples >= 3
            receipt['scene_before'] = status
            if not args.diagnostic and not receipt['world_settled']:
                raise TimeoutError('Arena terrain did not settle before measurement')
            assert abs(status['player']['position'][0]-(x+.5)) < 2 and abs(status['player']['position'][1]-(y+2)) < 3, 'Arena teleport did not take effect'
            call('look 0 6')
            time.sleep(5)
            receipt['empty'] = measure(call,args.seconds)
            save()
            setup_fight(call,x,y,z,add,args.sustained)
            time.sleep(3)
            receipt['scene_fight_start'] = call('status')
            receipt['arena_mobs'] = call('mobs 40')
            assert receipt['scene_fight_start']['mobs'] >= 20 and 'wither' in json.dumps(receipt['arena_mobs']), 'Arena entities were not created'
            receipt['fight'] = measure(call,args.seconds)
            receipt['arena_mobs_end'] = call('mobs 40')
            if args.sustained:
                alive = receipt['arena_mobs_end']['near']
                assert sum(e['kind'] in ('minecraft:wither_skeleton','minecraft:husk','minecraft:iron_golem') for e in alive) >= 20
                assert any(e['kind']=='minecraft:wither' for e in alive)
            print(args.label,'arena FPS:', round(receipt['fight']['fps'],1),
                  '1% low:',round(receipt['fight']['fps_1pct_low'],1),flush=True)
        except BaseException as e:
            receipt['error'] = repr(e)
            raise
        finally:
            if proc.poll() is None:
                try:
                    call('quit')
                except (OSError, ValueError, RuntimeError):
                    pass
            try:
                receipt['exit_code'] = proc.wait(timeout=45)
            except subprocess.TimeoutExpired:
                proc.terminate()
                receipt['exit_code'] = proc.wait(timeout=10)
            save()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('label')
    parser.add_argument('binary')
    parser.add_argument('--distance', type=int, default=16)
    parser.add_argument('--simulation', type=int, default=16)
    parser.add_argument('--scale', type=int, default=100)
    parser.add_argument('--fps-limit', type=int, default=120)
    parser.add_argument('--api', default='vulkan')
    parser.add_argument('--pacing', choices=('efficient','precise'))
    parser.add_argument('--seconds', type=int, default=25)
    parser.add_argument('--settle-limit', type=int, default=180)
    parser.add_argument('--world-template')
    parser.add_argument('--tuned', action='store_true')
    parser.add_argument('--sustained', action='store_true', help='Keep arena mobs alive throughout the measurement')
    parser.add_argument('--offscreen', action='store_true')
    parser.add_argument('--test-terrain', action='store_true')
    parser.add_argument('--entry-only', action='store_true')
    parser.add_argument('--diagnostic', action='store_true')
    run(parser.parse_args())
