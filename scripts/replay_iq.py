"""Replay completed HDF5 IQ segments into the native LoRa receiver without RF hardware."""
import argparse,glob,json,subprocess,sys,pathlib
import h5py
p=argparse.ArgumentParser();p.add_argument('files',nargs='+');p.add_argument('--channels',default='433500000,434000000');p.add_argument('--sf',default='7,8,9,10,11,12');p.add_argument('--bandwidth',default='62500,125000,250000,500000');p.add_argument('--ldro',choices=['-1','0','1'],default='0');p.add_argument('--allow-gaps',action='store_true');a=p.parse_args()
files=sorted(set(x for pattern in a.files for x in glob.glob(pattern)))
if not files:raise SystemExit('No files matched')
with h5py.File(files[0],'r') as h:center=int(h.attrs['center_hz']);rate=int(h.attrs['sample_rate_hz']);session=h.attrs['session']
cmd=['./build/lora_rx','-r','-','-s',str(rate),'-f',str(center),'-C',a.channels,'-b',a.bandwidth,'-S',a.sf,'-l',a.ldro,'-w','0']
print('Decoder:', ' '.join(cmd),file=sys.stderr);child=subprocess.Popen(cmd,stdin=subprocess.PIPE)
prev=None
try:
 for name in files:
  with h5py.File(name,'r') as h:
   start=int(h.attrs['first_sample_index']);n=int(h.attrs['valid_complex_samples'])
   if int(h.attrs['center_hz'])!=center or int(h.attrs['sample_rate_hz'])!=rate or h.attrs['session']!=session:raise RuntimeError(f'{name}: different tuning/rate/session; replay as a separate group')
   if prev is not None and (start!=prev or h.attrs.get('unknown_gap_before',0)) and not a.allow_gaps:raise RuntimeError(f'{name}: sample gap; --allow-gaps explicitly permits concatenation')
   if not h.attrs['complete']:raise RuntimeError(f'{name}: incomplete/active segment; omit it or stop recording before replay')
   for first in range(0,n,65536):child.stdin.write(h['iq_u8'][first:min(first+65536,n)].tobytes())
   prev=start+n
 child.stdin.close();raise SystemExit(child.wait())
finally:
 if child.poll() is None:child.terminate();child.wait()
