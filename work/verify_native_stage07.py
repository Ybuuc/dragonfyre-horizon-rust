"""Exercise the original Dragonfyre coin exchange in isolated real Horizon worlds."""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT/'outputs/native-stage-07'
OLD = ROOT.parents[1]/'outputs/Dragonfyre-Native-QuestPrototype-Stage06/horizon-client.exe'
RES = Path(r'C:\Users\Dimash\AppData\Roaming\horizon-launcher\versions\nightly-2026-11-05-66f3981\.local\horizon-client\26.3')
A, B = 'DFQOwnerA', 'DFQOwnerB'
RUBY, CORE, TRADE, OBSIDIAN = '5AFA5CB101E31BE4', '288608ED892DC5EB', '2AE651270FD58200', '643A6BD7BE518653'
GEM, STONE, COIN, HAM = 'cisco_mod:radiant_ruby', 'cisco_mod:divine_core', 'cisco_mod:champion_coin', 'farmersdelight:honey_glazed_ham'
MODS = [OUT/f'Dragonfyre-{ns}.hmod' for ns in ('cisco_mod','farmersdelight')]


def uid(name):
    return uuid.UUID(bytes=hashlib.md5(('OfflinePlayer:'+name).encode()).digest(), version=3)


def cmd(s):
    return {'op':'cmd', 'cmd':s}


def tick(n=1):
    return {'op':'tick', 'n':n}


def native(op, quest, expected, cycle=None):
    command = f'dragonfyredev {op} {quest}'
    if cycle is not None:
        command += f' {cycle}'
    return {'op':'native_cmd','cmd':command,'permission':4,'expect':expected}


def state(levels, **items):
    return {'op':'dfq_state','expect':{'levels':levels,'items':items}}


def write_json(path, obj):
    path.write_text(json.dumps(obj, indent=2, ensure_ascii=False)+'\n', encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', action='store_true')
    parser.add_argument('--quick', action='store_true', help='Restart and UUID checks in three launches; omit the injected crash')
    parser.add_argument('--client', type=Path)
    parser.add_argument('--resources', type=Path, default=RES)
    args = parser.parse_args()
    client = args.client or (OLD if args.baseline else OUT/'horizon-client-instrumented.exe')
    scratch = Path(tempfile.mkdtemp(prefix='stage07-worlds-', dir=ROOT/'work'))
    out = OUT/('baseline' if args.baseline else ('quick-game-checks' if args.quick else 'game-checks'))
    out.mkdir(parents=True, exist_ok=True)
    game = scratch/'game'
    game.mkdir()
    (game/'options.txt').write_text('renderDistance:2\nsimulationDistance:5\n')
    messages = json.loads((ROOT/'native/dragonfyre-quests/data/messages.ru.json').read_text(encoding='utf-8'))
    report = {'success':False,'mode':('baseline' if args.baseline else ('quick' if args.quick else 'full')),'client':str(client),'client_sha256':hashlib.sha256(client.read_bytes()).hexdigest(),
              'scratch':str(scratch),'players':{n:str(uid(n)) for n in (A,B)},'runs':[]}

    def run(label, name, ops, *, world='Coins', crash=None, expected_exit=0, client_override=None):
        print('RUN',label,flush=True)
        script, reply, log, cfg = (out/f'{label}.{suffix}' for suffix in ('script.jsonl','replies.jsonl','log','client.json'))
        script.write_text(''.join(json.dumps({k:v for k,v in op.items() if k!='expect'})+'\n' for op in ops), encoding='utf-8')
        reply.unlink(missing_ok=True)
        env = os.environ.copy()
        for key, value in {'HORIZON_HOME':'home','HORIZON_LAUNCHER_HOME':'launcher','XDG_DATA_HOME':'data','XDG_CONFIG_HOME':'config',
                           'HORIZON_LAUNCHER_TRASH':'trash','HORIZON_LAUNCHER_CLIPBOARD':'clipboard',
                           'HORIZON_LAUNCHER_DESKTOP':'desktop'}.items():
            env[key] = str(scratch/value)
        for key in ('HORIZON_DFQ_CRASH','HORIZON_DFQ_CRASH_OP','HORIZON_DFQ_ECONOMY_FIXTURE'):
            env.pop(key,None)
        env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_COMMAND_SCRIPT=str(script),
                   HORIZON_COMMAND_OUTPUT=str(reply),HORIZON_HARNESS_CLIENT_CONFIG=str(cfg),
                   HORIZON_HARNESS_TIMEOUT='600',HORIZON_HARNESS_TERRAIN_ONLY='1',HORIZON_PLAYER_NAME=name)
        if crash:
            env.update(HORIZON_DFQ_CRASH=crash[0],HORIZON_DFQ_CRASH_OP=str(crash[1]))
        command = [str((client_override or client).resolve()),'--resources',str(args.resources.resolve()),'--game-dir',str(game),
                   '--world',world,'--creative','--seed','7','--render-distance','2','--headless','--test-terrain','--no-pause','--name',name]
        if not args.baseline:
            for mod in MODS:
                assert mod.is_file(), mod
                command += ['--dev-mod',str(mod)]
        receipt = {'phase':label,'actor':name,'world':world,'command':command,'expected_exit':expected_exit,'success':False}
        report['runs'].append(receipt)
        start = time.monotonic()
        with log.open('w', encoding='utf-8') as f:
            result = subprocess.run(command,env=env,cwd=game,stdout=f,stderr=subprocess.STDOUT,timeout=900,
                                    creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
        receipt.update(exit_code=result.returncode,seconds=round(time.monotonic()-start,2))
        log_text = log.read_text(encoding='utf-8',errors='replace')
        assert result.returncode == expected_exit, (label,result.returncode,log_text[-6000:])
        if expected_exit:
            reason = 'DFQ injected process exit: '+crash[0] if crash else 'transaction marker belongs to another actor/pack'
            assert reason in log_text, (label,log_text[-4000:])
        else:
            expected = [op for op in ops if 'expect' in op]
            rows = [json.loads(s) for s in reply.read_text(encoding='utf-8').splitlines()]
            rows = [row for row in rows if row.get('cmd')=='#dfq_state' or row.get('cmd','').startswith('dragonfyredev ')]
            assert len(rows) == len(expected), (label,rows,expected)
            for op, row in zip(expected,rows,strict=True):
                want = op['expect']
                if op['op']=='native_cmd':
                    assert row['cmd'] == op['cmd'] and row['native_claimed'] and row['actor']==uid(name).hex and row['messages'][0][0][0] == messages['dragonfyre_quests.dev.'+want], (label,op,row)
                else:
                    assert row['cmd']=='#dfq_state', (label,op,row)
                    actual = row['state']
                    amounts = Counter()
                    for stack in actual['inventory']:
                        amounts[stack['item']] += stack['count']
                        if stack['item']==HAM:
                            assert 0 < stack['count'] <= 16, (label,actual)
                    assert actual['levels']==want['levels'], (label,want,actual)
                    assert all(amounts[k]==v for k,v in want['items'].items()), (label,want,actual)
        receipt['success']=True

    def tx(name, world='Coins'):
        folder = game/'saves'/world/'dragonfyre-transactions-v1'/uid(name).hex
        records = sorted(folder.glob('*.txn'))
        assert len(records)==len(list(folder.glob('*.ack')))
        result = []
        for index,path in enumerate(records,1):
            b = path.read_bytes()
            assert b[:6]==b'DFQTX1' and int.from_bytes(b[70:86],'little')==uid(name).int
            sequence, quest, slot, kind = struct.unpack_from('<QQQB',b,86)
            assert sequence==index and (folder/f'{sequence:020}.ack').is_file()
            result.append([f'{quest:016X}',slot,kind])
        return result

    def setup(name, item, slot=0, count=1):
        return cmd(f'item replace entity {name} hotbar.{slot} with {item} {count}')

    def economic(op, q, msg='queued', cycle=None):
        return native(op,q,msg,cycle)

    def award_inputs(name):
        return [setup(name,GEM),setup(name,STONE,1),tick(25),
                economic('economy',RUBY,'ready'),economic('economy',CORE,'ready'),
                economic('claim',RUBY),economic('claim',CORE),tick(),
                state(0,**{GEM:1,STONE:1,COIN:5,HAM:0})]

    try:
        if args.baseline:
            run('01-old-A',A,[setup(A,'minecraft:obsidian'),tick(25),economic('claim',OBSIDIAN),tick(),
                              state(5,**{'minecraft:flint_and_steel':1})])
            run('02-old-B',B,[state(5,**{'minecraft:flint_and_steel':1})])
            report['checks']=['Stage 06: B inherited the inventory and XP of A in the same world']
        else:
            prepare = [economic('economy',RUBY,'locked'),economic('claim',TRADE),tick(),
                state(0,**{COIN:0,HAM:0}),*award_inputs(A),economic('economy',TRADE,'locked',2),
                economic('submit',TRADE,cycle=2),tick(),state(0,**{COIN:5,HAM:0}),
                economic('submit',TRADE),economic('submit',TRADE,cycle=1),tick(),
                economic('economy',TRADE,'ready'),state(0,**{GEM:1,STONE:1,COIN:3,HAM:0})]
            first_records = [[RUBY,0,1],[CORE,0,1],[TRADE,1,2]]
            if args.quick:
                prepare += [economic('claim',TRADE),economic('claim',TRADE,cycle=1),tick(),
                    economic('economy',TRADE,'claimed'),state(0,**{COIN:3,HAM:16}),
                    economic('submit',TRADE,cycle=2),tick(),economic('economy',TRADE,'ready',2),
                    state(0,**{COIN:1,HAM:16})]
                first_records += [[TRADE,1,1],[TRADE,2,2]]
            run('01-A-earn-and-submit',A,prepare)
            assert tx(A)==first_records
            run('02-B-separate-profile',B,[state(0,**{GEM:0,STONE:0,COIN:0,HAM:0}),
                economic('economy',TRADE,'awaiting_items'),economic('economy',RUBY,'locked'),
                setup(B,'minecraft:obsidian'),tick(25),economic('claim',OBSIDIAN),tick(),
                state(5,**{'minecraft:flint_and_steel':1,COIN:0,HAM:0})])
            assert tx(B)==[[OBSIDIAN,0,1]]
            if args.quick:
                run('03-A-reopens-with-reward',A,[economic('economy',TRADE,'claimed',1),
                    economic('economy',TRADE,'ready',2),economic('economy',RUBY,'claimed'),
                    economic('economy',CORE,'claimed'),
                    state(0,**{GEM:1,STONE:1,COIN:1,HAM:16,'minecraft:flint_and_steel':0}),
                    economic('submit',TRADE,cycle=1),economic('claim',TRADE,cycle=1),
                    economic('submit',TRADE,cycle=2),tick(),state(0,**{COIN:1,HAM:16}),
                    economic('claim',TRADE,cycle=2),economic('claim',TRADE,cycle=2),tick(),
                    economic('economy',TRADE,'claimed',2),state(0,**{COIN:1,HAM:32})])
            else:
                run('03-A-after-restart-and-switch',A,[economic('economy',TRADE,'ready'),
                    economic('economy',RUBY,'claimed'),economic('economy',CORE,'claimed'),
                    state(0,**{GEM:1,STONE:1,COIN:3,HAM:0,'minecraft:flint_and_steel':0}),
                    economic('claim',TRADE),economic('claim',TRADE,cycle=1),tick(),
                    economic('economy',TRADE,'claimed'),state(0,**{COIN:3,HAM:16}),
                    economic('submit',TRADE,cycle=2),tick(),economic('economy',TRADE,'ready',2),
                    state(0,**{COIN:1,HAM:16})])
                run('04-A-crash-reward-cycle-2',A,[economic('claim',TRADE,cycle=2),tick()],
                    crash=('after_intent',1),expected_exit=86)
                run('05-A-recovers-after-crash',A,[economic('economy',TRADE,'claimed',2),
                    state(0,**{GEM:1,STONE:1,COIN:1,HAM:32}),economic('submit',TRADE,cycle=2),
                    economic('claim',TRADE,cycle=2),economic('claim',RUBY),economic('submit',TRADE,cycle=3),tick(),
                    economic('economy',TRADE,'awaiting_items',3),state(0,**{COIN:1,HAM:32})])
                run('06-B-reopens-own-save',B,[economic('economy',OBSIDIAN,'claimed'),
                    economic('economy',TRADE,'awaiting_items'),state(5,**{GEM:0,STONE:0,COIN:0,HAM:0,'minecraft:flint_and_steel':1})])
                run('07-A-reopens-own-save',A,[economic('economy',TRADE,'claimed',2),
                    state(0,**{GEM:1,STONE:1,COIN:1,HAM:32,'minecraft:flint_and_steel':0})])
            a = tx(A)
            assert a==[[RUBY,0,1],[CORE,0,1],[TRADE,1,2],[TRADE,1,1],[TRADE,2,2],[TRADE,2,1]],a
            assert tx(B)==[[OBSIDIAN,0,1]]
            playerdir = game/'saves/Coins/players/data'
            for name in (A,B):
                assert (playerdir/f'{uid(name)}.dat').is_file()
            report['transactions']={A:a,B:tx(B)}
            report['checks']=['five Champion Coins claimed from the two original Cisco item quests',
                'two cycles deducted two coins each and awarded 16 servings each; duplicate and skipped cycles rejected',
                'submitted exchange and prior award survived a complete process restart',
                'owner A/B inventory, XP, progress, journals and player saves remained separate']
            if not args.quick:
                report['checks'].append('award recovered from a process exit after intent publication, without duplication')
        report['success']=True
    except Exception as e:
        report['error']=repr(e)
        raise
    finally:
        write_json(out/'verification.json',report)
    print('PASS',len(report['runs']),'launches',flush=True)


if __name__=='__main__':
    main()
