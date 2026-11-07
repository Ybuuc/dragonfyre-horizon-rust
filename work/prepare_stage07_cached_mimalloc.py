"""Reuse the exact mimalloc library previously compiled for the pinned Horizon tree."""
import hashlib
import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT/'work'
OUT = ROOT/'outputs/native-stage-07'
REL = Path('third_party/libmimalloc-sys')


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def main():
    stage = Path(json.loads((OUT/'applied.json').read_text())['path'])
    pristine = WORK/'native-package-pristine'/REL
    old = WORK/'native-package-applied-stage06-final'/REL
    new = stage/REL
    entries = sorted(p.relative_to(pristine) for p in pristine.rglob('*') if p.is_file())
    assert len(entries)>25 and Path('build.rs') in entries
    hashes = {str(p):sha(pristine/p) for p in entries}
    assert all(sha(old/p)==sha(new/p)==hashes[str(p)] for p in entries)
    cached = WORK/'native-game-target/debug/build/libmimalloc-sys-683d837d71216dc5'
    output = (cached/'output').read_text()
    links = [l.removeprefix('cargo:rustc-link-lib=') for l in output.splitlines() if l.startswith('cargo:rustc-link-lib=')]
    assert links==['static=mimalloc','psapi','shell32','user32','advapi32','bcrypt'], links
    assert 'x86_64-pc-windows-gnu' in output
    previous = cached/'out/libmimalloc.a'
    assert previous.read_bytes().startswith(b'!<arch>\n')
    lib_dir = OUT/'verified-build-dependencies'
    lib_dir.mkdir(exist_ok=True)
    library = lib_dir/previous.name
    shutil.copyfile(previous,library)
    assert sha(library)==sha(previous)
    target = OUT/'build-overrides.toml'
    def path_text(path):
        return json.dumps(str(path).replace('\\','/'))
    target.write_text('[target.x86_64-pc-windows-gnu.mimalloc]\n'
                      f'rustc-link-lib = {json.dumps(links)}\n'
                      f'rustc-link-search = [{json.dumps("native="+library.parent.as_posix())}]\n'
                      f'include_dir = {path_text(new/"c_src/mimalloc/v3/include")}\n',encoding='utf-8')
    (OUT/'build-dependency-verification.json').write_text(json.dumps({
        'reason':'Missing external GCC in locally installed MinGW',
        'source_stage':'Stage 06',
        'source_archive':str(previous),
        'sha256':sha(library),
        'target':'x86_64-pc-windows-gnu',
        'vendor_sources_checked':len(hashes),
        'vendor_sources_sha256':hashes,
        'build_metadata':links,
    },indent=2))
    print(f'PASS: {len(hashes)} byte-identical mimalloc source files; Stage 06 archive {sha(library)}')


if __name__=='__main__':
    main()
