"""Verify the Dragonfyre journal via real window events and the Offline Launcher."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
LAB = ROOT/'work/stage09-journal'
OUT = ROOT/'outputs/native-stage-09'
TAG = 'nightly-2026-11-05-66f3981'
VERSION = LAB/'launcher/versions'/TAG
LAUNCHER = Path(r'C:/Users/Dimash/AppData/Local/Programs/HorizonOfflineLauncher/horizon-offline-launcher.exe')
RUBY, CORE, COIN, HAM = ('cisco_mod:radiant_ruby','cisco_mod:divine_core',
                        'cisco_mod:champion_coin','farmersdelight:honey_glazed_ham')
NAME = 'DFQJournalA'
QUESTS = (0x5AFA5CB101E31BE4,0x288608ED892DC5EB,0x2AE651270FD58200)


def uid(name):
    return uuid.UUID(bytes=hashlib.md5(('OfflinePlayer:'+name).encode()).digest(),version=3)


def control(endpoint, line):
    info = json.loads(endpoint.read_text(encoding='utf-8'))
    with socket.create_connection(('127.0.0.1',info['port']),timeout=15) as conn:
        conn.settimeout(60)
        stream=conn.makefile('rwb')
        stream.write(f"auth {info['token']}\n".encode())
        stream.flush()
        assert json.loads(stream.readline()).get('ok')
        stream.write((line+'\n').encode())
        stream.flush()
        reply=json.loads(stream.readline())
    if 'error' in reply:
        raise RuntimeError(f"{line}: {reply['error']}")
    return reply


def poll(check, description, limit=30):
    deadline=time.monotonic()+limit
    while time.monotonic()<deadline:
        try:
            if result:=check():
                return result
        except PermissionError:
            raise
        except (OSError,ValueError):
            pass
        time.sleep(0.2)
    raise TimeoutError(description)


def point(size,x,y):
    # The test options fix Horizon's GUI scale at 2 when the window allows it.
    scale=2 if size[0]//2>=320 and size[1]//2>=240 else 1
    w,h=(math.ceil(side/scale) for side in size)
    fit=min(w/372,h/208,1)
    return (w-360*fit)/2+x*fit,(h-196*fit)/2+y*fit


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mute',action='store_true')
    parser.add_argument('--scratch',type=Path)
    args=parser.parse_args()
    binary=OUT/'horizon-client-release.exe'
    build=json.loads((OUT/'release-build.json').read_text(encoding='utf-8'))
    with binary.open('rb') as f:
        digest=hashlib.file_digest(f,'sha256').hexdigest()
    assert build['exit_code']==0 and build['audio_feature'] and digest==build['binary_sha256']
    assert (VERSION/'.copy-complete').is_file()
    target=VERSION/'horizon-client.exe'
    if not target.is_file() or hashlib.sha256(target.read_bytes()).hexdigest()!=digest:
        shutil.copy2(binary,target)
    scratch=args.scratch.resolve() if args.scratch else Path(tempfile.mkdtemp(prefix='journal-test-',dir=LAB))
    assert scratch.is_relative_to(LAB) and scratch.name.startswith('journal-test-')
    game=scratch/'game'
    game.mkdir(exist_ok=True)
    assert not (game/'saves/JournalTest/dragonfyre-transactions-v1').exists(), 'choose a fresh test world'
    (game/'options.txt').write_text(
        'lang:ru_ru\ngraphicsPreset:"custom"\nguiScale:2\nrenderDistance:4\nsimulationDistance:5\n'
        'maxFps:90\nenableVsync:false\npauseOnLostFocus:false\nsoundCategory_master:0.25\n',encoding='utf-8')
    (game/'horizon-graphics.txt').write_text('preset:custom\ngraphicsApi:vulkan\n',encoding='utf-8')
    env=os.environ.copy()
    for key,directory in {'HORIZON_HOME':'game','HORIZON_LAUNCHER_HOME':'launcher',
                          'XDG_DATA_HOME':'data','XDG_CONFIG_HOME':'config','HORIZON_LAUNCHER_TRASH':'trash',
                          'HORIZON_LAUNCHER_CLIPBOARD':'clipboard','HORIZON_LAUNCHER_DESKTOP':'desktop'}.items():
        env[key]=str(LAB/directory if key=='HORIZON_LAUNCHER_HOME' else scratch/directory)
    for key in ('HORIZON_NO_AUDIO','HORIZON_COMMAND_SCRIPT','HORIZON_COMMAND_OUTPUT',
                'HORIZON_DFQ_ECONOMY_FIXTURE','HORIZON_DFQ_CRASH','HORIZON_DFQ_CRASH_OP'):
        env.pop(key,None)
    env.update(HORIZON_AUDIO='1',HORIZON_LAUNCHER_NO_SELF_UPDATE='1',HORIZON_TELEMETRY_URL='off')
    if args.mute:
        env['HORIZON_NO_AUDIO']='1'
    out=OUT/('launcher-journal-muted-checks' if args.mute else 'launcher-journal-checks')
    out.mkdir(exist_ok=True)
    report={'success':False,'binary_sha256':digest,'scratch':str(scratch),'runs':[],'checks':[]}
    endpoint=scratch/'control.json'
    call=lambda line:control(endpoint,line)

    def count(want):
        inv=call('inventory')['counts']
        return inv if all(inv.get(k,0)==n for k,n in want.items()) else None

    def wait_counts(**want):
        return poll(lambda:count(want),f'inventory did not reach {want}')

    def screenshot(name):
        path=out/(name+'.png')
        call('screenshot '+str(path))
        assert path.stat().st_size>5000

    def screen(name):
        return poll(lambda:(s if (s:=call('status'))['screen']==name else None),f'expected screen {name}')

    def click(x,y):
        size=call('status')['renderer']['size']
        x,y=point(size,x,y)
        return call(f'click-at {x:.3f} {y:.3f}')

    def open_journal():
        call('key j tap')
        screen('mod')
        time.sleep(1.1)

    def same_counts(**want):
        time.sleep(1.3)
        assert count(want) is not None, (want,call('inventory')['counts'])

    def run(label,scenario):
        endpoint.unlink(missing_ok=True)
        args=[str(LAUNCHER),'play','--version',TAG,'--world','JournalTest']
        cli=['--game-dir',str(game),'--home',str(game),'--player-dir',str(game),'--name',NAME,
             '--width','1280','--height','720','--seed','1234','--no-pause','--test-terrain',
             '--control-socket',str(endpoint)]
        for name in ('cisco_mod','farmersdelight'):
            cli+=['--dev-mod',str(ROOT/'outputs/native-stage-07'/f'Dragonfyre-{name}.hmod')]
        args.extend('--arg='+part for part in cli)
        receipt={'label':label,'command':args}
        report['runs'].append(receipt)
        with (out/(label+'.log')).open('w',encoding='utf-8') as log:
            proc=subprocess.Popen(args,cwd=game,env=env,stdout=log,stderr=subprocess.STDOUT)
            try:
                def ready():
                    if proc.poll() is not None:
                        raise RuntimeError(f'Launcher stopped: {proc.returncode}')
                    if endpoint.is_file():
                        s=call('status')
                        if s.get('screen')=='playing' and s.get('player') and s.get('renderer'):
                            return s
                    return None
                start=poll(ready,'launcher world did not load',limit=1100)
                assert not start['creative'], start
                receipt['start']=start
                scenario()
            finally:
                if proc.poll() is None:
                    try:
                        call('quit')
                    except (OSError,RuntimeError,ValueError):
                        pass
                try:
                    receipt['exit_code']=proc.wait(timeout=45)
                except subprocess.TimeoutExpired:
                    proc.terminate()
                    receipt['exit_code']=proc.wait(timeout=10)
        assert receipt['exit_code']==0

    def first():
        poll(lambda:call('status').get('player',{}).get('on_ground'),'player never landed',limit=45)
        before=call('status')['sounds'].get('decoded',0)
        movement=call('move forward 1.4')
        sounds=call('status')
        report['audio']={'decoded_before':before,'movement':movement,'status':sounds['sounds'],'heard':sounds.get('heard',{})}
        open_journal()
        screenshot('01-empty-journal')
        click(295,57)
        click(315,144)
        same_counts(**{COIN:0,HAM:0})
        call('key escape tap')
        screen('playing')
        assert call('give '+RUBY)['given']==1
        assert call('give '+CORE)['given']==1
        wait_counts(**{RUBY:1,CORE:1})
        open_journal()
        time.sleep(1.8)
        screenshot('02-rewards-ready')
        click(295,57)
        click(295,57)
        wait_counts(**{COIN:3})
        time.sleep(1.1)
        click(295,101)
        wait_counts(**{COIN:5})
        time.sleep(1.1)
        click(315,144)
        same_counts(**{COIN:5,HAM:0})
        click(251,144)
        click(251,144)
        wait_counts(**{COIN:3,HAM:0})
        time.sleep(1.1)
        screenshot('03-trade-submitted')
        click(315,144)
        click(315,144)
        wait_counts(**{COIN:3,HAM:16})
        time.sleep(1.1)
        click(251,144)
        wait_counts(**{COIN:1,HAM:16})
        time.sleep(1.1)
        screenshot('04-second-cycle-pending')
        report['checks'].append('J/Esc, inactive buttons, both original item rewards, debit, claim, duplicate clicks and second submission in survival')

    def restarted():
        wait_counts(**{RUBY:1,CORE:1,COIN:1,HAM:16})
        open_journal()
        screenshot('05-second-cycle-restored')
        click(251,144)
        same_counts(**{COIN:1,HAM:16})
        click(315,144)
        click(315,144)
        wait_counts(**{COIN:1,HAM:32})
        same_counts(**{COIN:1,HAM:32})
        screenshot('06-both-cycles-complete')
        call('key escape tap')
        screen('playing')
        report['checks'].append('Restart kept the pending cycle and both quest rewards; its reward was claimed once via the journal')

    try:
        run('01-journal',first)
        run('02-restart',restarted)
        world=game/'saves/JournalTest'
        actor=uid(NAME)
        folder=world/'dragonfyre-transactions-v1'/actor.hex
        records=sorted(folder.glob('*.txn'))
        assert len(records)==len(list(folder.glob('*.ack')))==6
        actual=[]
        for path in records:
            data=path.read_bytes()
            assert data[:6]==b'DFQTX1' and int.from_bytes(data[70:86],'little')==actor.int
            _,quest,cycle,kind=struct.unpack_from('<QQQB',data,86)
            actual.append((quest,cycle,kind))
        assert actual==[(QUESTS[0],0,1),(QUESTS[1],0,1),(QUESTS[2],1,2),(QUESTS[2],1,1),(QUESTS[2],2,2),(QUESTS[2],2,1)]
        report['transactions']=[[f'{q:016X}',cycle,kind] for q,cycle,kind in actual]
        logs=[(out/(entry['label']+'.log')).read_text(encoding='utf-8',errors='replace') for entry in report['runs']]
        assert all('dragonfyredev' not in log for log in logs)
        if not args.mute:
            assert all('Horizon: audio output ' in log for log in logs), 'audio output did not open'
            assert report['audio']['status']['decoded']>0 and report['audio']['heard'], report['audio']
            report['checks'].append('Default Windows audio output opened and decoded game sounds')
        report['success']=True
    except BaseException as e:
        report['error']=repr(e)
        raise
    finally:
        (out/'verification.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
    print('PASS: Offline Launcher, journal clicks, saved exchange' + ('' if args.mute else ' and audio output'),flush=True)

if __name__=='__main__':
    main()
