// Offsets include the two-byte Andoya radio ID. Preserve the original bytes.
export function decodeBinary27(hex) {
  if (typeof hex !== 'string' || !/^[0-9a-f]{54}$/i.test(hex)) return null;
  const b=Uint8Array.from(hex.match(/../g),x=>parseInt(x,16));
  const d=new DataView(b.buffer);
  const lat=d.getFloat32(4,true), lon=d.getFloat32(8,true);
  return {
    schema:'andoya_binary27_partial',time_counter_u16:d.getUint16(2,true),
    latitude_deg:Number.isFinite(lat)?lat:null,
    longitude_deg:Number.isFinite(lon)?lon:null,
    gps_coordinates_available:Number.isFinite(lat)&&Number.isFinite(lon)&&Math.abs(lat)<=90&&Math.abs(lon)<=180&&(lat!==0||lon!==0),
    gps_fix_status:'unknown',
    latitude_raw_i32:d.getInt32(4,true),longitude_raw_i32:d.getInt32(8,true),
    altitude_raw_i16:d.getInt16(12,true),field14_u8:d.getUint8(14),
    pressure_pa:d.getFloat32(19,true),field15_u16_div16:d.getUint16(15,true)/16,
    field17_u8:d.getUint8(17),field18_u8:d.getUint8(18),field23_u16:d.getUint16(23,true)
  };
}
export function gpsText(f) {
  const label=f.gps_coordinates_available
    ? `GPS ${Math.abs(f.latitude_deg).toFixed(6)}° ${f.latitude_deg<0?'S':'N'}, ${Math.abs(f.longitude_deg).toFixed(6)}° ${f.longitude_deg<0?'W':'E'}`
    : f.latitude_deg===0&&f.longitude_deg===0?'GPS unavailable (0, 0)':'GPS invalid coordinates';
  return `${label} · fix unverified · f14=${f.field14_u8}`;
}
// A corrupted float can still be inside the geographic range. Keep jumps as
// candidates until repeated; this is a plausibility check, not a checksum.
export function trackGps(prev,p) {
  if(prev&&prev.lastPacketUtc>=p.utc)return prev;
  const f=p.fields;
  if(!f.gps_coordinates_available){
    return {...(prev||{utc:p.utc,fields:f,unchangedSince:p.utc}),lastPacketUtc:p.utc,warning:'latest GPS unavailable/invalid'};
  }
  if(prev?.fields.gps_coordinates_available){
    const r=Math.PI/180, a=prev.fields, dlat=(f.latitude_deg-a.latitude_deg)*r,dlon=(f.longitude_deg-a.longitude_deg)*r;
    const h=Math.sin(dlat/2)**2+Math.cos(a.latitude_deg*r)*Math.cos(f.latitude_deg*r)*Math.sin(dlon/2)**2;
    const metres=12742000*Math.asin(Math.sqrt(Math.min(1,h)));
    const jump=metres>500+400*Math.max(0,p.utc-prev.utc);
    const repeated=prev.candidate?.latitude_deg===f.latitude_deg&&prev.candidate?.longitude_deg===f.longitude_deg;
    if(jump&&!repeated)return {...prev,lastPacketUtc:p.utc,candidate:f,warning:'GPS jump awaiting repeat'};
  }
  const same=prev?.fields.latitude_deg===f.latitude_deg&&prev?.fields.longitude_deg===f.longitude_deg;
  return {utc:p.utc,lastPacketUtc:p.utc,fields:f,unchangedSince:same?prev.unchangedSince:p.utc,warning:''};
}
