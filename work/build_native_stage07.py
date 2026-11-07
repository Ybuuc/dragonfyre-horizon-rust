"""Build Dragonfyre Stage 07 from an official horizon-mixins application."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT/'work'
OUT = ROOT/'outputs/native-stage-07'
TOOLS = ROOT.parents[2]/'referenced-chatgpt-conversation-this-is-an/work/build-tools'
BIN = TOOLS/'rustup/toolchains/stable-x86_64-pc-windows-gnu/bin'
PRISTINE = WORK/'native-package-pristine'
HARN = Path('crates/horizon-client/src/game/command_harness.rs')


def sha(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f,'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apply-only', action='store_true')
    parser.add_argument('--pure', action='store_true')
    parser.add_argument('--applied', type=Path)
    parser.add_argument('--cargo-config', type=Path)
    args = parser.parse_args()
    OUT.mkdir(exist_ok=True,parents=True)
    env = os.environ.copy()
    env['PATH'] = os.pathsep.join([str(BIN),str(TOOLS/'mingw64/bin'),env['PATH']])
    env.update(CARGO_HOME=str(WORK/'native-cargo'),CARGO_TARGET_DIR=str(WORK/'native-game-target'),
               CARGO_BUILD_JOBS='1',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',
               CARGO_PROFILE_DEV_OPT_LEVEL='0',CARGO_NET_OFFLINE='true',
               TEMP=str(WORK/'rust-temp'),TMP=str(WORK/'rust-temp'))
    stage = args.applied.resolve() if args.applied else None
    src = ROOT/'native/dragonfyre-quests'
    if stage is None:
        run = Path(tempfile.mkdtemp(prefix='stage07-build-',dir=WORK))
        mods,stage = run/'mods',run/'applied'
        for name in ['horizon-mixin.toml','economy.rs','perform.rs','register.rs','recover.rs','tick.rs','profile.rs',
                     'engine/Cargo.toml','engine/src/catalog.rs','engine/src/economy_catalog.rs','data/messages.ru.json']:
            assert (src/name).is_file(),name
        shutil.copytree(src,mods/'dragonfyre_quests',ignore=shutil.ignore_patterns('target','Cargo.lock'))
        command = [str(WORK/'mixins-target/debug/horizon-mixins.exe'),'apply','--workspace',str(PRISTINE),
                   '--mods',str(mods),'--out',str(stage)]
        result = subprocess.run(command,cwd=ROOT,env=env,capture_output=True,text=True,encoding='utf8')
        (OUT/'apply.log').write_text(result.stdout+result.stderr,encoding='utf8')
        if result.returncode:
            raise SystemExit(result.stderr)
        receipt = json.loads(result.stdout)
        assert receipt['status']=='ok' and receipt['injections']==5 and receipt['crates']==receipt['data']==1
        (OUT/'applied.json').write_text(json.dumps({'path':str(stage),'result':receipt},indent=2))
        print('Applied five injections:',stage,flush=True)
    if args.apply_only:
        return
    assert stage.is_relative_to(WORK) and stage.name=='applied'
    assert 'isolate_integrated_owner_profile' in (stage/'crates/horizon-client/src/game/lan.rs').read_text(encoding='utf8')
    reference = WORK/'native-package-applied-stage06-final'/HARN
    fixture = reference.read_bytes()
    assert b'"dfq_state"' in fixture and b'"native_cmd"' in fixture
    target = stage/HARN
    target.write_bytes((PRISTINE/HARN).read_bytes() if args.pure else fixture)
    mode = 'pure' if args.pure else 'instrumented'
    command = [str(BIN/'cargo.exe'),'build','--manifest-path',str(stage/'Cargo.toml'),
               '--locked','--offline','-p','horizon-client','--no-default-features']
    if args.cargo_config:
        command.extend(['--config',str(args.cargo_config.resolve())])
    start = time.monotonic()
    print('Building',mode,flush=True)
    with (OUT/f'{mode}-build.log').open('w',encoding='utf8') as f:
        result = subprocess.run(command,cwd=stage,env=env,stdout=f,stderr=subprocess.STDOUT)
    receipt = dict(exit_code=result.returncode,command=command,seconds=round(time.monotonic()-start,2))
    if result.returncode==0:
        binary = OUT/f'horizon-client-{mode}.exe'
        shutil.copyfile(WORK/'native-game-target/debug/horizon-client.exe',binary)
        strip = TOOLS/'mingw64/bin/strip.exe'
        if strip.is_file():
            subprocess.run([str(strip),'--strip-all',str(binary)],check=True)
        receipt['stripped'] = strip.is_file()
        receipt.update(binary=str(binary),binary_sha256=sha(binary),harness_sha256=sha(target))
    (OUT/f'{mode}-build.json').write_text(json.dumps(receipt,indent=2))
    raise SystemExit(result.returncode)


if __name__=='__main__':
    main()
