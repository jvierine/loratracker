"""Build the station, pipe decoder and rustplotlib helper on the current OS."""
import argparse
import os
from pathlib import Path
import shutil
import shlex
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
EXE = '.exe' if os.name == 'nt' else ''

def install_copy(source, destination):
    # Replacing an executable in place can leave stale macOS code-signature
    # cache entries, and can disturb a running instance. Publish a fresh inode.
    temporary = destination.with_name(destination.name + '.new')
    shutil.copy2(source, temporary)
    temporary.replace(destination)

def native_environment():
    env = os.environ.copy()
    env['PATH'] = str(Path.home()/'.cargo/bin') + os.pathsep + env.get('PATH', '')
    prefix = Path(env.get('HDF5_DIR', str(Path(sys.prefix)/'Library' if os.name == 'nt' else Path(sys.prefix))))
    env['HDF5_DIR'] = str(prefix)
    # Keep native linker options out of the WebAssembly build.
    flags = env.get('CARGO_ENCODED_RUSTFLAGS', '').split('\x1f') if env.get('CARGO_ENCODED_RUSTFLAGS') else shlex.split(env.get('RUSTFLAGS', ''))
    if os.name != 'nt':
        flags += ['-C', f'link-arg=-Wl,-rpath,{prefix}/lib']
        env['LIBRARY_PATH'] = str(prefix/'lib') + os.pathsep + env.get('LIBRARY_PATH', '')
        # Release ZIPs can be unpacked under a different conda installation
        # from the build runner. Resolve HDF5/libc++ from the user's runtime.
        runtime_path = 'DYLD_LIBRARY_PATH' if sys.platform == 'darwin' else 'LD_LIBRARY_PATH'
        env[runtime_path] = str(prefix/'lib') + os.pathsep + env.get(runtime_path, '')
    else:
        env['PATH'] = str(prefix/'bin') + os.pathsep + env['PATH']
    env.pop('RUSTFLAGS', None)
    env['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(flags)
    return env

def build(wasm=False):
    os.chdir(ROOT)
    env = native_environment()
    for executable in ['cargo', 'cmake']:
        if not shutil.which(executable, path=env['PATH']):
            raise RuntimeError(f'{executable} is required to build from source. See README.md installation instructions.')
    (ROOT/'bin').mkdir(exist_ok=True)
    def run(args, **kwargs): subprocess.run(args, cwd=ROOT, env=env, check=True, **kwargs)
    run(['cmake', '-S', '.', '-B', 'build/native', '-DCMAKE_BUILD_TYPE=Release'])
    run(['cmake', '--build', 'build/native', '--config', 'Release', '--parallel'])
    run(['cmake', '--install', 'build/native', '--config', 'Release', '--prefix', str(ROOT/'build/install')])
    for name in ['lora_rx', 'lora_tx_gen']:
        install_copy(ROOT/'build/install/bin'/(name+EXE), ROOT/'bin'/(name+EXE))
    run(['cargo', 'build', '--locked', '--release', '-p', 'balloon-station'])
    run(['cargo', 'build', '--locked', '--release', '--manifest-path', 'tools/flight-plots/Cargo.toml'])
    for source, name in [(ROOT/'target/release'/('balloon-station'+EXE), 'balloon-station'), (ROOT/'tools/flight-plots/target/release'/('flight-plots'+EXE), 'flight-plots')]:
        install_copy(source, ROOT/'bin'/(name+EXE))
    # Preserve the command-line replay/test tools' existing locations.
    for name in ['lora_rx', 'lora_tx_gen']:
        install_copy(ROOT/'bin'/(name+EXE), ROOT/'build'/(name+EXE))
    if wasm or not (ROOT/'web/pkg/station_display_bg.wasm').exists():
        wasm_env = env.copy(); wasm_env.pop('RUSTFLAGS', None); wasm_env.pop('CARGO_ENCODED_RUSTFLAGS', None)
        subprocess.run(['rustup', 'target', 'add', 'wasm32-unknown-unknown'], env=wasm_env, check=True)
        subprocess.run(['cargo', 'build', '--locked', '--release', '--target', 'wasm32-unknown-unknown', '-p', 'station-display'], env=wasm_env, check=True)
        subprocess.run(['wasm-bindgen', 'target/wasm32-unknown-unknown/release/station_display.wasm', '--target', 'web', '--out-dir', 'web/pkg'], env=wasm_env, check=True)
    print('Built STRATOS in bin/. Start with start-gui.command, start-gui.sh or start-gui.bat.')

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--wasm', action='store_true', help='Rebuild the included browser WebAssembly assets (wasm-bindgen-cli 0.2.129 required)')
    args = parser.parse_args()
    try: build(args.wasm)
    except (RuntimeError, subprocess.CalledProcessError) as error:
        print(f'Build failed: {error}', file=sys.stderr); sys.exit(1)
