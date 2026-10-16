"""Decode the observed Andøya 27-byte packet; unknown fields stay explicitly raw."""
import argparse,struct,json
p=argparse.ArgumentParser();p.add_argument('hex');a=p.parse_args();b=bytes.fromhex(a.hex)
if len(b)!=27:raise SystemExit(f'Expected 27 bytes including CanSat ID, got {len(b)}')
import math
lat,lon=struct.unpack_from('<ff',b,4)
print(json.dumps(dict(cansat_id=struct.unpack_from('<H',b,0)[0],time_counter_u16=struct.unpack_from('<H',b,2)[0],latitude_deg=lat if math.isfinite(lat) else None,longitude_deg=lon if math.isfinite(lon) else None,gps_coordinates_available=math.isfinite(lat) and math.isfinite(lon) and abs(lat)<=90 and abs(lon)<=180 and (lat!=0 or lon!=0),gps_fix_status='unknown; no payload CRC',latitude_raw_i32=struct.unpack_from('<i',b,4)[0],longitude_raw_i32=struct.unpack_from('<i',b,8)[0],altitude_raw_i16=struct.unpack_from('<h',b,12)[0],field14_u8=b[14],field15_u16_div16=struct.unpack_from('<H',b,15)[0]/16,field17_u8=b[17],field18_u8=b[18],pressure_pa=struct.unpack_from('<f',b,19)[0],field23_u16=struct.unpack_from('<H',b,23)[0],field25_u8=b[25],field26_u8=b[26],payload_hex=b.hex(),layout_status='GPS float32 little-endian decimal degrees at offsets 4 and 8 inferred from repeated air packets; fix status, remaining sensor fields and checksum unknown'),indent=2))
