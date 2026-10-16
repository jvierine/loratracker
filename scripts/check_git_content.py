"""Reject flight recordings and files over 1 MiB in the proposed Git content."""
import argparse
from pathlib import Path
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
FORBIDDEN={'.h5','.hdf5','.hdf','.iq','.u8','.c16','.heic','.zip','.dmg','.msi'}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--staged',action='store_true')
    args=parser.parse_args()
    if args.staged:
        names=subprocess.check_output(['git','diff','--cached','--name-only','--diff-filter=ACM','-z'],cwd=ROOT).split(b'\0')
    else:
        names=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z'],cwd=ROOT).split(b'\0')
    errors=[];largest=(0,'');count=0
    for raw in set(names):
        if not raw:continue
        name=raw.decode('utf-8');path=ROOT/name
        if not path.exists() and not path.is_symlink():continue
        if args.staged:
            size=int(subprocess.check_output(['git','cat-file','-s',':'+name],cwd=ROOT))
        else:size=path.lstat().st_size
        count+=1;largest=max(largest,(size,name))
        if path.suffix.lower() in FORBIDDEN or name.startswith(('telemetry-local/','recordings/','raw-recordings-external')):
            errors.append(name+': recording/local data is forbidden')
        if size>1024*1024:errors.append(name+f': {size/1e6:.2f} MB exceeds the 1 MiB source limit')
    if errors:
        print('\n'.join(errors),file=sys.stderr);return 1
    print(f'PASS: {count} Git source files; no recordings; largest {largest[1]} ({largest[0]/1024:.0f} KiB)')
    return 0

if __name__=='__main__':sys.exit(main())
