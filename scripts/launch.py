"""Portable GUI launcher. Owns the backend and history helper until Ctrl+C."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
import urllib.request
import webbrowser
from build import ROOT, EXE, build, native_environment

URL = 'http://127.0.0.1:8096'

def status():
    try:
        with urllib.request.urlopen(URL+'/api/status', timeout=1) as response:
            return json.load(response)
    except Exception: return None

def request(path, value):
    req = urllib.request.Request(URL+path, data=json.dumps(value).encode(), headers={'Content-Type':'application/json'}, method='POST')
    with urllib.request.urlopen(req, timeout=30) as response: return json.load(response)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--live', action='store_true', help='Enable the RTL-SDR receiver (default: recorded-data review)')
    parser.add_argument('--data-dir', type=Path, help='Folder containing flight-*.h5 and/or destination for new recordings')
    parser.add_argument('--no-browser', action='store_true')
    parser.add_argument('--build', action='store_true', help='Build the native applications before launching')
    args = parser.parse_args()
    os.chdir(ROOT)
    existing = status()
    if existing:
        if args.live == bool(existing.get('review', existing.get('status', {}).get('review', False))):
            raise RuntimeError('STRATOS is already running in a different mode. Stop its launcher before changing modes.')
        if args.data_dir:
            request('/api/config', {**existing['config'], 'data_dir':str(args.data_dir.expanduser().resolve())})
        print('STRATOS is already running: '+URL)
        if not args.no_browser: webbrowser.open(URL)
        return
    # Fail with useful dependency instructions before starting either service.
    for module in ['numpy', 'h5py', 'pymsis']:
        try: __import__(module)
        except ImportError as error:
            raise RuntimeError('Run setup.sh / setup.bat to install the Python runtime dependencies.') from error
    backend = ROOT/'bin'/('balloon-station'+EXE)
    plotter = ROOT/'bin'/('flight-plots'+EXE)
    if args.build or not backend.exists() or not plotter.exists() or not (ROOT/'bin'/('storage-picker'+EXE)).exists(): build()
    (ROOT/'build').mkdir(exist_ok=True)
    data_dir = args.data_dir.expanduser().resolve() if args.data_dir else None
    settings = ROOT/'settings.json'
    if data_dir is None:
        saved = json.loads(settings.read_text()) if settings.exists() else {}
        if saved.get('data_dir'): data_dir = Path(saved['data_dir']).expanduser().resolve()
        elif (ROOT/'recordings').is_dir(): data_dir = (ROOT/'recordings').resolve()
        else:
            data_dir = Path.home()/'Documents/STRATOS/data'
            data_dir.mkdir(parents=True, exist_ok=True)
    if not data_dir.is_dir(): raise RuntimeError(f'Data directory is missing or not mounted: {data_dir}. Select an existing folder with --data-dir.')
    env = native_environment()
    env.update(STATION_PYTHON=sys.executable, STATION_REVIEW='0' if args.live else '1', STATION_DATA_DIR=str(data_dir), STATION_DECODER=str(ROOT/'bin'/('lora_rx'+EXE)), PYTHONUNBUFFERED='1')
    # TeX distributions normally register PATH; cover macOS Finder launches too.
    if sys.platform == 'darwin' and Path('/Library/TeX/texbin').is_dir():
        env['PATH'] += os.pathsep+'/Library/TeX/texbin'
    children = []
    helper_log = open(ROOT/'build/flight-history.log', 'a', encoding='utf-8')
    # Ctrl+C is handled by the launcher; backend receives a graceful HTTP shutdown.
    group = {'creationflags':subprocess.CREATE_NEW_PROCESS_GROUP} if os.name == 'nt' else {'start_new_session':True}
    try:
        children.append(subprocess.Popen([str(backend)], cwd=ROOT, env=env, **group))
        for _ in range(150):
            if children[0].poll() is not None: raise RuntimeError('Station exited; see the message above.')
            if status(): break
            time.sleep(.2)
        else: raise RuntimeError('Station did not become ready on port 8096.')
        children.append(subprocess.Popen([sys.executable, 'scripts/flight_history.py'], cwd=ROOT, env=env, stdout=helper_log, stderr=subprocess.STDOUT, **group))
        print(f'STRATOS {"LIVE" if args.live else "DATA REVIEW"} · {URL}\nData: {data_dir}\nKeep this window open. Ctrl+C stops STRATOS.', flush=True)
        if not args.no_browser: webbrowser.open(URL)
        while children[0].poll() is None:
            if children[1].poll() is not None: raise RuntimeError('History/plot helper stopped. See build/flight-history.log.')
            time.sleep(.5)
    except KeyboardInterrupt: pass
    finally:
        if children and children[0].poll() is None:
            try: request('/api/shutdown', {})
            except Exception: pass
            try: children[0].wait(timeout=15)
            except subprocess.TimeoutExpired: children[0].terminate()
        for child in children[1:]:
            if child.poll() is None:
                if os.name == 'nt': child.terminate()
                else: os.killpg(child.pid, signal.SIGTERM)
                try: child.wait(timeout=5)
                except subprocess.TimeoutExpired: child.kill()
        helper_log.close()

if __name__ == '__main__':
    try: main()
    except Exception as error:
        print(f'STRATOS: {error}', file=sys.stderr); sys.exit(1)
