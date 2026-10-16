export const copy=v=>JSON.parse(JSON.stringify(v));
export function zoomLimits(a,factor,fx=.5,fy=.5,map=false){
 const out=copy(a),xc=a.xlim[0]+fx*(a.xlim[1]-a.xlim[0]),yc=a.ylim[0]+fy*(a.ylim[1]-a.ylim[0]);
 out.xlim=a.xlim.map(v=>xc+(v-xc)*factor);
 if(map)out.ylim=a.ylim.map(v=>yc+(v-yc)*factor);
 const pts=(a.points||[]).filter(p=>p.every(Number.isFinite));
 if(!pts.length)return out;
 if(!map){const low=Math.min(...pts.map(p=>p[0])),high=Math.max(...pts.map(p=>p[0]));const maxSpan=Math.max((high-low)*1.3,1e-4);if(out.xlim[1]-out.xlim[0]>maxSpan){const center=(low+high)/2;out.xlim=[center-maxSpan/2,center+maxSpan/2];}}
 let visible=pts.filter(p=>p[0]>=out.xlim[0]&&p[0]<=out.xlim[1]&&(!map||p[1]>=out.ylim[0]&&p[1]<=out.ylim[1]));
 if(!visible.length){
  // Keep zoom on the nearest observation rather than vanishing into a gap.
  const dx=a.xlim[1]-a.xlim[0],dy=a.ylim[1]-a.ylim[0];
  const nearest=pts.reduce((best,p)=>{const score=p=>((p[0]-xc)/dx)**2+(map?((p[1]-yc)/dy)**2:0);return score(p)<score(best)?p:best;});
  const w=out.xlim[1]-out.xlim[0];out.xlim=[nearest[0]-w/2,nearest[0]+w/2];
  if(map){const h=out.ylim[1]-out.ylim[0];out.ylim=[nearest[1]-h/2,nearest[1]+h/2];}
  visible=pts.filter(p=>p[0]>=out.xlim[0]&&p[0]<=out.xlim[1]);
 }
 if(!map&&visible.length){let lo=Math.min(...visible.map(p=>p[1])),hi=Math.max(...visible.map(p=>p[1]));const pad=Math.max((hi-lo)*.1,(a.ylim[1]-a.ylim[0])*.005,1e-6);out.ylim=[lo-pad,hi+pad];}
 return out;
}
