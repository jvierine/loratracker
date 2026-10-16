"""Independent bandwidth / chirp-period check of the recorded live RTL-SDR IQ."""
import json, pathlib, urllib.request
import h5py, numpy as np
from scipy import signal
v=json.loads(pathlib.Path('build/verification-session.json').read_text())
with h5py.File(v['path'],'r') as h:
    print('Recorded IQ shape:',h['iq_u8'].shape,'packets:',len(h['packets_json']))
    u=h['iq_u8'][:min(len(h['iq_u8']),12_000_000)]
    packets=[json.loads(s) for s in h['packets_json'].asstr()[:]]
    center=float(h['iq_blocks'][0,3])
fs=2_000_000
z=(u[:,0].astype(np.float32)-127.5)+1j*(u[:,1].astype(np.float32)-127.5)
z*=np.exp(-2j*np.pi*(434_000_000-center)/fs*np.arange(len(z)))
x=signal.resample_poly(z,1,8);fs2=fs/8
# A chirp preamble repeats one symbol. Search the period independently of the decoder.
size=4096
energy=np.convolve(np.abs(x[:len(x)//32*32].reshape(-1,32)).mean(1),np.ones(16)/16,'same')
peak=signal.find_peaks(energy,distance=500)[0]
starts=peak[np.argsort(energy[peak])[-12:]]*32
best=(0,None,None)
for start in starts:
 a=x[max(0,start-4000):start+6000]
 if len(a)<6000:continue
 for lag in range(450,580):
  prod=a[lag:]*np.conj(a[:-lag]);den=np.abs(a[lag:])**2+np.abs(a[:-lag])**2
  w=1024
  cs=np.concatenate(([0j],np.cumsum(prod)));ds=np.concatenate(([0.],np.cumsum(den)));cp=cs[w:]-cs[:-w];dp=ds[w:]-ds[:-w]
  scores=2*np.abs(cp)/np.maximum(dp,1e-12)
  k=int(np.argmax(scores));score=float(scores[k])
  if score>best[0]:best=(score,lag,max(0,start-4000)+k)
score,lag,start=best
period=lag/fs2
segment=x[start:start+2048]
instant=np.angle(segment[1:]*np.conj(segment[:-1]))*fs2/(2*np.pi)
smoothed=signal.medfilt(instant,7)
wraps=signal.find_peaks(-np.diff(smoothed),distance=400,prominence=4000)[0]
slopes=[]
for a,b in zip(wraps[:-1],wraps[1:]):
    a+=15;b-=15
    if b-a>350:
        phase=np.unwrap(np.angle(segment[a:b]));t=np.arange(len(phase))/fs2
        slopes.append(np.polyfit(t,phase,2)[0]/np.pi)
measured_slope=float(np.median(slopes)) if slopes else float('nan')
f,p=signal.welch(segment,fs=fs2,nperseg=512,return_onesided=False)
order=np.argsort(f);f=f[order];p=p[order]
noise=np.median(p[np.abs(f)>70000]);power=np.maximum(p-noise,0);acc=np.cumsum(power);acc/=acc[-1]
lo=float(np.interp(.025,acc,f));hi=float(np.interp(.975,acc,f))
matched=[p for p in packets if p.get('cansat_id')==14000 and 'navn på CanSat;' in p.get('text','') and '�' not in p.get('text','') and p.get('bandwidth_hz')==62500 and p.get('sf')==7]
result=dict(recording=v['path'],carrier_nominal_hz=434000000,observed_95_percent_bandwidth_hz=hi-lo,chirp_period_samples=lag,analysis_sample_rate_hz=fs2,chirp_period_s=period,chirp_slope_hz_per_s=measured_slope,bandwidth_from_chirp_slope_hz=measured_slope*period,preamble_repeat_coherence=score,spreading_factor_from_nominal_bandwidth=float(np.log2(62500*period)),matched_decoded_packets=len(matched),payload_crc_present=False,source_library='reference/andoya/Cansat_RFM96/Cansat_RFM96.cpp')
pathlib.Path('build/air-verification.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
with h5py.File('recordings/air-verification.h5','w') as h:
 for k,val in result.items():h.attrs[k]=val
 h.create_dataset('preamble_iq',data=segment)
 h.create_dataset('frequency_offset_hz',data=f);h.create_dataset('psd_relative',data=p)
 h.create_dataset('matching_packet_json',data=np.array([json.dumps(p) for p in matched],dtype=h5py.string_dtype()))
# Continue telemetry-only recording after closing the verification capture.
c=v['config'];c['recording']=True;c['record_iq']=False
req=urllib.request.Request('http://127.0.0.1:8096/api/config',data=json.dumps(c).encode(),headers={'Content-Type':'application/json'},method='POST');urllib.request.urlopen(req).read()
