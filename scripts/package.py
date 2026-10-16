"""Create a source ZIP or a native ZIP without recordings or local configuration."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import stat
import zipfile

ROOT = Path(__file__).resolve().parents[1]
VERSION = '0.1.0'
TOP = ['Cargo.toml','Cargo.lock','CMakeLists.txt','LICENSE','README.md','THIRD_PARTY.md','environment.yml','requirements.txt','.gitignore','.gitattributes','run.sh','start-gui.sh','start-gui.command','start-gui.bat','setup.sh','setup.bat']

def sources():
    paths = [ROOT/name for name in TOP if (ROOT/name).is_file()]
    for folder in ['src','scripts','vendor/lora','vendor/rustplotlib','web-ui','tools/flight-plots','web','.github']:
        for path in (ROOT/folder).rglob('*'):
            if not path.is_file() or path.is_symlink(): continue
            relative = path.relative_to(ROOT)
            if relative.as_posix() == 'scripts/archive_for_review.py': continue
            if any(part in ('target','build','__pycache__','.git','.latex-cache') for part in relative.parts): continue
            if path.suffix.lower() in ('.h5','.hdf5','.iq','.u8','.c16','.heic','.pyc','.log'): continue
            if relative.parts[:2] == ('web','map-data') and path.name not in ('land.geojson','NOTICE.txt'): continue
            paths.append(path)
    paths.append(ROOT/'reference/andoya/sources.json')
    return sorted(set(path for path in paths if path.exists()))

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binaries', action='store_true', help='Include bin/ for this operating system and CPU')
    args = parser.parse_args()
    label = f'{platform.system().lower()}-{platform.machine().lower()}' if args.binaries else 'source'
    name = f'stratos-{VERSION}-{label}'
    files = sources()
    if not (ROOT/'web/pkg/station_display_bg.wasm').is_file(): raise SystemExit('Build the browser assets before packaging: scripts/build.py --wasm')
    if args.binaries:
        exe = '.exe' if os.name == 'nt' else ''
        for binary in ['balloon-station','flight-plots','lora_rx','lora_tx_gen']:
            path = ROOT/'bin'/(binary+exe)
            if not path.is_file(): raise SystemExit(f'Missing {path}; run scripts/build.py first')
            files.append(path)
    output = ROOT/'dist'/(name+'.zip'); output.parent.mkdir(exist_ok=True)
    manifest = {'version':VERSION,'platform':label,'note':'Software only. Flight recordings, settings, symlinks and generated plot/history caches are excluded. Conda runtime and TeX are installed separately; see README.md.','sha256':{path.relative_to(ROOT).as_posix():hashlib.sha256(path.read_bytes()).hexdigest() for path in files}}
    with zipfile.ZipFile(output,'w',zipfile.ZIP_DEFLATED,compresslevel=9) as archive:
        for path in files:
            # Zipfile preserves executable modes for Unix launchers/native binaries.
            archive.write(path, name+'/'+path.relative_to(ROOT).as_posix())
        archive.writestr(name+'/PACKAGE-MANIFEST.json',json.dumps(manifest,indent=2)+'\n')
    print(f'{output}\n{output.stat().st_size/1e6:.2f} MB · {len(files)} software files · no flight data')

if __name__ == '__main__': main()
