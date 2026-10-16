"""Read-only HDF5 history + live API into a disposable GUI cache; no receiver restart."""
import os,json,time,struct,math,shutil,datetime,subprocess,traceback
from pathlib import Path
import h5py,numpy as np,pymsis
from pressure_analysis import analyse
from plot_view_server import start,render
ROOT=Path(__file__).resolve().parents[1]
os.chdir(ROOT)
OUT=ROOT/'web/map-data';OUT.mkdir(exist_ok=True)
from file_lock import FileLock
lock=FileLock(ROOT/'build/flight-history.lock',blocking=False)
try:lock.acquire()
except BlockingIOError:raise SystemExit(0)
start()
seen={};errors=[]
def add(p):
    try:
        hx=p.get('payload_hex',''); b=bytes.fromhex(hx);hz=int(p.get('frequency_hz',0))
        if len(b)<4 or hz not in (433500000,434000000):return
        ident,counter=struct.unpack_from('<HH',b)
        # Keep every matching physical packet, including weak checksum-free ones.
        if ident!=hz//1000-420000 or p.get('crc_valid') is False:return
        lat=lon=None;field14=None
        if len(b)==27:
            lat,lon=struct.unpack_from('<ff',b,4);pressure=struct.unpack_from('<f',b,19)[0];field14=b[14]
        else:
            cols=b[2:].decode('utf-8').strip().split(';')
            if len(cols)<9 or not all(math.isfinite(float(v)) for v in cols[1:]):return
            pressure=float(cols[-1]);counter=int(float(cols[1]))
            if not 10<pressure<110000:return
        seen[(p['utc'],hz,hx)]={'utc':p['utc'],'hz':hz,'hex':hx,'lat':lat if lat is not None and math.isfinite(lat) else None,'lon':lon if lon is not None and math.isfinite(lon) else None,'pressure_hpa':pressure/100 if math.isfinite(pressure) else None,'field14':field14,'counter':counter,'snr_db':p.get('snr_db')}
    except (ValueError,KeyError,struct.error):pass
# Read packet datasets only; never load the large I/Q arrays. Snapshot the live log.
import urllib.request
for attempt in range(30):
    try:
        status=json.load(urllib.request.urlopen('http://127.0.0.1:8096/api/status',timeout=5));break
    except Exception:time.sleep(2)
else:raise SystemExit('Receiver API unavailable')
loaded_dir=None
def read_history(status):
    global loaded_dir
    directory=Path(status['config']['data_dir'])
    if loaded_dir==directory:return
    seen.clear();errors.clear();loaded_dir=directory
    active_path=status['status'].get('recording');active=Path(active_path) if active_path else None
    for path in sorted(directory.glob('flight-*.h5')):
        source=path
        try:
            if active is not None and path.resolve()==active.resolve():
                source=ROOT/'build/map-history-snapshot.h5';shutil.copyfile(path,source)
            with h5py.File(source,'r',locking=False) as h:
                if 'packets_json' in h:
                    for raw in h['packets_json'].asstr():add(json.loads(raw))
            print('History read:',path.name,flush=True)
        except Exception as e:errors.append(path.name+': '+str(e));print(errors[-1],flush=True)
read_history(status)
# MSIS climatology: explicit assumed drivers, no claim of observed solar weather.
z=np.linspace(0,60,1201)
result=pymsis.calculate(['2026-10-15T14:00'],[16.04],[69.31],z,f107s=[150],f107as=[150],aps=[[4]*7],version=2.1).reshape(-1,11)
p=np.nansum(result[:,[1,2,3,4,5,6,7,9]],axis=1)*1.380649e-23*result[:,10]/100
assert np.all(np.diff(p)<0) and 800<p[0]<1200
with h5py.File('build/msis-pressure-height.h5','w') as h:
    h['altitude_km']=z;h['pressure_hpa']=p;h['temperature_k']=result[:,10]
    h.attrs['model']='NRLMSIS 2.1 via pymsis';h.attrs['date_utc']='2026-10-15T14:00:00Z';h.attrs['latitude_deg']=69.31;h.attrs['longitude_deg']=16.04;h.attrs['f107_assumed']=150.;h.attrs['f107a_assumed']=150.;h.attrs['ap_assumed']=4.;h.attrs['pressure_method']='k_B T sum thermal species number densities; anomalous oxygen excluded'
profile={'z':z.tolist(),'p':p.tolist(),'model':'NRLMSIS 2.1','assumptions':'69.31 N, 16.04 E; 15 Oct 14:00 UTC; assumed F10.7/F10.7a=150, Ap=4; climatological estimate'}
while True:
    try:
        status=json.load(urllib.request.urlopen('http://127.0.0.1:8096/api/status',timeout=4))
        read_history(status)
        for packet in status['packets']:add(packet)
        rows=sorted(seen.values(),key=lambda v:v['utc'])
        analysis=analyse(rows)
        with h5py.File('build/pressure-analysis.tmp.h5','w') as h:
            h.attrs['method']=analysis['method'];h.attrs['generated_utc_unix']=time.time()
            for ch in analysis['channels']:
                group=h.create_group(str(ch['hz']))
                for key in ['utc','pressure_hpa','outlier_utc','outlier_pressure_hpa','prime_utc','rho_prime_hpa','rho_mean_hpa']:group[key]=ch[key]
                for kind in ['raw_structure','prime_structure']:
                    g=group.create_group(kind)
                    for key,values in ch[kind].items():
                        if isinstance(values,dict):g.attrs[key+'_json']=json.dumps(values)
                        else:g[key]=values
                group.attrs['removed']=ch['removed'];group.attrs['window_s']=ch['window_s']
        os.replace('build/pressure-analysis.tmp.h5','build/pressure-analysis.h5')
        data={'generated_utc':time.time(),'packets':rows,'profile':profile,'history_errors':errors,'analysis':analysis,'data_dir':str(loaded_dir)}
        temp=OUT/'history.tmp';temp.write_text(json.dumps(data,allow_nan=False));temp.replace(OUT/'history.json')
        render()
    except Exception:traceback.print_exc()
    time.sleep(5)
