"""Build the two namespace-specific registry packs for the Dragonfyre exchange."""
import argparse
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir',type=Path,default=root/'outputs/native-stage-07')
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True,exist_ok=True)
    for namespace, count in [('cisco_mod',6),('farmersdelight',4)]:
        source = root/'native/dragonfyre-chain-items'/namespace
        files = sorted(p for p in source.rglob('*') if p.is_file())
        assert (source/'horizon-mod.toml') in files and len(files)==count
        output = args.output_dir/f'Dragonfyre-{namespace}.hmod'
        with ZipFile(output,'w',compression=ZIP_DEFLATED) as z:
            for f in files:
                z.write(f,f.relative_to(source).as_posix())
        with ZipFile(output) as z:
            assert z.testzip() is None
        print(output)


if __name__=='__main__':
    main()
