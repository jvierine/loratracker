"""Add raw payload byte datasets to an earlier, CLOSED station recording."""
import argparse,json
import h5py,numpy as np
p=argparse.ArgumentParser();p.add_argument('recording');a=p.parse_args()
with h5py.File(a.recording,'r+') as h:
    packets=[json.loads(s) for s in h['packets_json'].asstr()[:]]
    if 'payload_bytes' not in h:
        ds=h.create_dataset('payload_bytes',(len(packets),),maxshape=(None,),dtype=h5py.vlen_dtype(np.dtype('uint8')),chunks=True)
        for i,v in enumerate(packets):ds[i]=np.frombuffer(bytes.fromhex(v['payload_hex']),dtype=np.uint8)
        h.create_dataset('packet_utc_unix',data=np.array([v['utc'] for v in packets]),maxshape=(None,),chunks=True)
        h.attrs['payload_bytes_provenance']='Exact bytes recovered from packets_json.payload_hex; includes CanSat ID prefix'
    for i,v in enumerate(packets):assert bytes(h['payload_bytes'][i]).hex()==v['payload_hex']
    print(f'{len(packets)} payloads stored and verified byte-for-byte in {a.recording}')
