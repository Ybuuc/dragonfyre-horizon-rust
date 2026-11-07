from pathlib import Path
import hashlib, json, os, shutil, subprocess, time

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT/'work'
OUT = ROOT/'outputs/native-stage-09'
STAGE = WORK/'stage09-journal'
PRIOR = WORK/'stage07-build-4337axm1/applied'
SOURCE = ROOT/'native/dragonfyre-quests'
TOOLS = ROOT.parents[2]/'referenced-chatgpt-conversation-this-is-an/work/build-tools'
RUST = TOOLS/'rustup/toolchains/stable-x86_64-pc-windows-gnu/bin'

def main():
    OUT.mkdir(exist_ok=True,parents=True)
    STAGE.mkdir(exist_ok=True)
    env=os.environ.copy()
    env['PATH']=os.pathsep.join([str(RUST),r'C:/msys64/mingw64/bin',env['PATH']])
    env.update(CARGO_HOME=str(WORK/'native-cargo'),CARGO_TARGET_DIR=str(WORK/'native-game-target'),
               CARGO_BUILD_JOBS='1',CARGO_INCREMENTAL='0',CARGO_NET_OFFLINE='true',
               TEMP=str(WORK/'rust-temp'),TMP=str(WORK/'rust-temp'))
    mods=STAGE/'mods'
    shutil.copytree(SOURCE,mods/'dragonfyre_quests',dirs_exist_ok=True,ignore=shutil.ignore_patterns('target','Cargo.lock'))
    applied=STAGE/'applied'
    command=[str(WORK/'mixins-target/debug/horizon-mixins.exe'),'apply','--workspace',str(WORK/'native-package-pristine'),
             '--mods',str(mods),'--out',str(applied)]
    result=subprocess.run(command,cwd=ROOT,env=env,capture_output=True,text=True,encoding='utf-8')
    (OUT/'apply.log').write_text(result.stdout+result.stderr,encoding='utf-8')
    if result.returncode:
        raise SystemExit(result.stdout+result.stderr)
    receipt=json.loads(result.stdout)
    assert receipt['injections']==8 and receipt['modules']==3, receipt
    (OUT/'applied.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    # Keep Cargo's fingerprints for untouched upstream files and dependencies.
    preserved=0
    for p in applied.rglob('*'):
        if p.is_file():
            old=PRIOR/p.relative_to(applied)
            if old.is_file() and old.stat().st_size==p.stat().st_size and old.read_bytes()==p.read_bytes():
                stat=old.stat()
                os.utime(p,ns=(stat.st_atime_ns,stat.st_mtime_ns))
                preserved+=1
    print(f'Applied quest journal hooks. Reused timestamps for {preserved} unchanged files.',flush=True)
    command=[str(RUST/'cargo.exe'),'build','--manifest-path',str(applied/'Cargo.toml'),
             '--release','--locked','--offline','-p','horizon-client','--no-default-features','--features','audio',
             '--config',str(ROOT/'outputs/native-stage-08-performance/release-overrides.toml')]
    start=time.monotonic()
    with (OUT/'release-build.log').open('w',encoding='utf-8') as log:
        code=subprocess.run(command,env=env,cwd=applied,stdout=log,stderr=subprocess.STDOUT).returncode
    data={'command':command,'exit_code':code,'audio_feature':True,'seconds':round(time.monotonic()-start,2)}
    if code==0:
        binary=OUT/'horizon-client-release.exe'
        shutil.copy2(WORK/'native-game-target/release/horizon-client.exe',binary)
        with binary.open('rb') as f:
            data['binary_sha256']=hashlib.file_digest(f,'sha256').hexdigest()
        data['binary']=str(binary)
    (OUT/'release-build.json').write_text(json.dumps(data,indent=2)+'\n',encoding='utf-8')
    print('Stage 09 build exit:',code,flush=True)
    raise SystemExit(code)

if __name__=='__main__':
    main()
