"""Exercise the packaged backend and native decoder without any SDR hardware."""
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import h5py
from build import ROOT, EXE, native_environment

def main():
    with tempfile.TemporaryDirectory(prefix='STRATOS test ') as temp:
        root = Path(temp); first = root/'data one Å'; second = root/'data two'; first.mkdir(); second.mkdir()
        packet = struct.pack('<HHffhB', 13500, 13, 0., 0., 0, 0) + bytes(4) + struct.pack('<f', 100000.) + bytes(4)
        meta = {'utc':1000., 'frequency_hz':433500000, 'payload_hex':packet.hex(), 'sf':12, 'bandwidth_hz':62500, 'has_crc':False, 'crc_valid':None, 'snr_db':10.}
        with h5py.File(first/'flight-fixture.h5', 'w') as file:
            file.create_dataset('packets_json', data=[json.dumps(meta)], dtype=h5py.string_dtype())
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
        env = native_environment(); env.update(STATION_PORT=str(port), STATION_DATA_DIR=str(first), STATION_REVIEW='1')
        url = f'http://127.0.0.1:{port}'
        def req(path, value=None):
            request = urllib.request.Request(url+path, data=json.dumps(value).encode() if value is not None else None, headers={'Content-Type':'application/json'})
            with urllib.request.urlopen(request, timeout=5) as response: return json.load(response)
        child = subprocess.Popen([str(ROOT/'bin'/('balloon-station'+EXE))], cwd=root, env=env, stdout=subprocess.DEVNULL)
        try:
            for attempt in range(100):
                if child.poll() is not None: raise RuntimeError('Test backend exited')
                try: result=req('/api/status'); break
                except urllib.error.URLError: time.sleep(.05)
            else: raise RuntimeError('Test backend did not start')
            assert result['review'] and len(result['packets']) == 1
            assert result['packets'][0]['payload_hex'] == packet.hex()
            assert req('/api/recordings')[0]['name'] == 'flight-fixture.h5'
            with urllib.request.urlopen(url+'/recordings/flight-fixture.h5') as response:
                assert response.read() == (first/'flight-fixture.h5').read_bytes()
            original=result['config']
            changed=req('/api/config', {**original, 'data_dir':str(second), 'recording':True, 'record_iq':True})
            assert not changed['recording'] and not changed['record_iq']
            assert req('/api/status')['packets'] == [] and req('/api/recordings') == []
            try: req('/api/config', {**changed, 'data_dir':str(root/'missing drive')})
            except urllib.error.HTTPError as error: assert error.code == 400
            else: raise AssertionError('Missing directory was accepted')
            assert not (root/'missing drive').exists()
            req('/api/config', original)
            assert len(req('/api/status')['packets']) == 1
            assert Path(json.loads((root/'settings.json').read_text())['data_dir']).samefile(first)
            assert list(second.iterdir()) == [], 'Review wrote into the selected data directory'
            req('/api/shutdown', {}); child.wait(timeout=10)
            assert child.returncode == 0
            print('PASS: isolated review, exact payload/download, directory switching, persistence, missing-drive rejection, no review recording, graceful shutdown')
        finally:
            if child.poll() is None: child.terminate(); child.wait(timeout=10)
        # Run the C++ pipe-only decoder against generated IQ after its platform changes.
        for sf,bw,cr,cfo in [(7,62500,1,0),(7,62500,1,-3400),(9,125000,2,1700),(12,62500,1,0)]:
            iq=root/'loopback.u8'
            subprocess.run([str(ROOT/'bin'/('lora_tx_gen'+EXE)), '-o', str(iq), '-S', str(sf), '-b', str(bw), '-s', '2000000', '-c', str(cr), '-O', str(cfo), '-m', 'CANSAT LOOPBACK'], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            result=subprocess.run([str(ROOT/'bin'/('lora_rx'+EXE)), '-r', str(iq), '-f', '434000000', '-s', '2000000', '-S', str(sf), '-b', str(bw), '-l', '-1', '-w', '0'], check=True, capture_output=True, text=True, timeout=60)
            decoded=[json.loads(row[8:]) for row in result.stdout.splitlines() if row.startswith('packet: ')]
            assert any(row['crc_valid'] and row['payload_hex']==b'CANSAT LOOPBACK'.hex() for row in decoded), (sf,bw,result.stderr)
            print(f'PASS: native pipe decode SF{sf}, BW{bw}, CR4/{4+cr}, CFO{cfo}')

if __name__ == '__main__': main()
