from pathlib import Path
import hashlib, json, os, shutil, subprocess, time
root=Path.cwd()
work=root/'work'
out=root/'outputs/native-stage-08-performance'
out.mkdir(exist_ok=True)
stage=work/'stage07-build-4337axm1/applied'
tools=root.parents[2]/'referenced-chatgpt-conversation-this-is-an/work/build-tools'
bin=tools/'rustup/toolchains/stable-x86_64-pc-windows-gnu/bin'
cache=work/'native-game-target'
old=out/'horizon-client-previous-release.exe'
if not old.exists() and (cache/'release/horizon-client.exe').is_file():
    shutil.copy2(cache/'release/horizon-client.exe',old)
fixture=Path('crates/horizon-client/src/game/command_harness.rs')
pristine=(work/'native-package-pristine'/fixture).read_bytes()
if (stage/fixture).read_bytes()!=pristine:
    (stage/fixture).write_bytes(pristine)
env=os.environ.copy()
env['PATH']=os.pathsep.join([str(bin),r'C:/msys64/mingw64/bin',str(tools/'mingw64/bin'),env['PATH']])
env.update(CARGO_HOME=str(work/'native-cargo'),CARGO_TARGET_DIR=str(cache),CARGO_BUILD_JOBS='1',
           CARGO_INCREMENTAL='0',CARGO_NET_OFFLINE='true',TEMP=str(work/'rust-temp'),TMP=str(work/'rust-temp'))
dependencies=out/'verified-release-dependencies'
dependencies.mkdir(exist_ok=True)
libraries = [
    ('ring',cache/'release/build/ring-daae7903a14952d9/out','libring_core_0_17_14_.a'),
    ('ring',cache/'release/build/ring-daae7903a14952d9/out','libring_core_0_17_14__test.a'),
    ('mimalloc',cache/'release/build/libmimalloc-sys-f47d56d4c20bbe26/out','libmimalloc.a'),
]
hashes={}
for _, folder, name in libraries:
    original=folder/name
    destination=dependencies/name
    if not destination.exists():
        shutil.copy2(original,destination)
    digest=hashlib.sha256(original.read_bytes()).hexdigest()
    assert destination.stat().st_size>10000 and hashlib.sha256(destination.read_bytes()).hexdigest()==digest
    hashes[name]={'source':str(original),'sha256':digest}
assert 'links = "ring_core_0_17_14_"' in next((work/'native-cargo/registry/src').glob('*/ring-0.17.14/Cargo.toml')).read_text()
mimalloc_inputs=stage/'third_party/libmimalloc-sys/c_src'
pristine_inputs=work/'native-package-pristine/third_party/libmimalloc-sys/c_src'
assert all(p.read_bytes()==(pristine_inputs/p.relative_to(mimalloc_inputs)).read_bytes()
           for p in mimalloc_inputs.rglob('*') if p.is_file())
(out/'release-dependencies.json').write_text(json.dumps(hashes,indent=2)+'\n',encoding='utf-8')
config=out/'release-overrides.toml'
config.write_text(f'''[target.x86_64-pc-windows-gnu.mimalloc]
rustc-link-lib = ["static=mimalloc", "psapi", "shell32", "user32", "advapi32", "bcrypt"]
rustc-link-search = ["native={dependencies.as_posix()}"]
include_dir = "{(mimalloc_inputs/'mimalloc/v3/include').as_posix()}"
[target.x86_64-pc-windows-gnu.ring_core_0_17_14_]
rustc-link-lib = ["static=ring_core_0_17_14_", "static=ring_core_0_17_14__test"]
rustc-link-search = ["native={dependencies.as_posix()}"]
''',encoding='utf-8')
cmd=[str(bin/'cargo.exe'),'build','--manifest-path',str(stage/'Cargo.toml'),
     '--release','--locked','--offline','-p','horizon-client','--no-default-features','--config',str(config)]
start=time.monotonic()
print('Building optimized Stage 07 with standard release profile',flush=True)
with (out/'release-build.log').open('w',encoding='utf-8') as f:
    code=subprocess.run(cmd,env=env,cwd=stage,stdout=f,stderr=subprocess.STDOUT).returncode
receipt={'profile':'release','exit_code':code,'seconds':round(time.monotonic()-start,2),'command':cmd,
         'original_harness':hashlib.sha256(pristine).hexdigest()}
if code==0:
    binary=out/'horizon-client-release.exe'
    shutil.copy2(cache/'release/horizon-client.exe',binary)
    receipt['sha256']=hashlib.sha256(binary.read_bytes()).hexdigest()
(out/'release-build.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
print('Build exit:',code,flush=True)
raise SystemExit(code)
