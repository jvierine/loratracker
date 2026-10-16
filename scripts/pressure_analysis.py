"""Outlier rejection and pressure Reynolds/structure-function diagnostics."""
import numpy as np

def fit_structure(sf):
    from scipy.optimize import least_squares
    full_x=np.asarray(sf['lag_s']);full_y=np.asarray(sf['d2_hpa2']);n=np.asarray(sf['pair_count'])
    ok=(full_x>0)&(full_x<=30.)&(full_y>0)&np.isfinite(full_y)
    x=full_x[ok];y=full_y[ok];n=n[ok]
    if len(x)<3:return {'status':'insufficient lag bins in 0–30 s','fit_range_s':[0.,30.]}
    weights=np.clip(np.sqrt(n/np.median(n)),.3,3)
    q=least_squares(lambda q:weights*(q[0]+q[1]*np.log(x)-np.log(y)),[np.mean(np.log(y)),2/3],loss='soft_l1',f_scale=.3).x
    loga,beta=q;amplitude=float(np.exp(loga))
    rms=float(np.sqrt(np.mean((loga+beta*np.log(x)-np.log(y))**2)))
    fixed_loga=float(np.average(np.log(y)-(2/3)*np.log(x),weights=weights**2))
    fixed_rms=float(np.sqrt(np.mean((fixed_loga+(2/3)*np.log(x)-np.log(y))**2)))
    # A separate empirical crossover estimate; it does not constrain the 0–30 s fit.
    outer=None;plateau=None;tail_slope=None;status='no resolved plateau; outer time unresolved'
    good=(full_x>0)&(full_y>0)&np.isfinite(full_y)
    tx=full_x[good];ty=full_y[good]
    if len(tx)>=6 and beta>0:
        tail=tx>=max(30.,max(tx)/3.)
        if sum(tail)>=4:
            tail_slope=float(np.polyfit(np.log(tx[tail]),np.log(ty[tail]),1)[0])
            if abs(tail_slope)<.25:
                plateau=float(np.exp(np.median(np.log(ty[tail]))))
                crossover=float(np.exp((np.log(plateau)-loga)/beta))
                if min(tx)<=crossover<=max(tx) and max(tx)>=1.5*crossover:
                    outer=crossover;status='empirical power-law/plateau crossover'
    xx=np.geomspace(min(x),30.,100)
    return {'fit_range_s':[0.,30.],'fit_bin_count':len(x),'status':status,'outer_time_s':outer,'amplitude':amplitude,'plateau_hpa2':plateau,'tail_exponent':tail_slope,'log_rms':rms,'fixed_2_3_log_rms':fixed_rms,'lag_s':xx.tolist(),'d2_hpa2':np.exp(loga+beta*np.log(xx)).tolist(),'exponent':float(beta),'model':'A * tau^beta; beta fitted on 0<tau<=30 s; outer time is separate plateau crossover'}

def analyse(rows, window_s=300.):
    channels=[]
    for hz in (433500000,434000000):
        by_time={}
        candidates=[r for r in rows if r['hz']==hz]
        for r in candidates:
            v=r.get('pressure_hpa');t=r['utc']
            if v is None or not np.isfinite(v) or not .1<=v<=1100:continue
            if t not in by_time or (r.get('snr_db') or -100)>(by_time[t].get('snr_db') or -100):by_time[t]=r
        rs=sorted(by_time.values(),key=lambda r:r['utc'])
        t=np.array([r['utc'] for r in rs]);p=np.array([r['pressure_hpa'] for r in rs]);keep=np.ones(len(p),dtype=bool)
        # Hampel screen on local observations, independently for each transmitter.
        for i in range(len(p)):
            a=max(0,i-4);b=min(len(p),i+5);local=p[a:b]
            if len(local)<3:continue
            tt=t[a:b]-t[i]
            dt=tt[None,:]-tt[:,None];dp=local[None,:]-local[:,None]
            pairs=dt>0
            slope=np.median(dp[pairs]/dt[pairs])
            detrended=local-slope*tt
            med=np.median(detrended);mad=1.4826*np.median(np.abs(detrended-med))
            if abs(p[i]-med)>max(.5,6*mad):keep[i]=False
        bad_t=t[~keep];bad_p=p[~keep];t=t[keep];p=p[keep]
        blocks=np.split(np.arange(len(t)),np.flatnonzero(np.diff(t)>20)+1)
        hp_t=[];hp=[];hp_mean=[];hp_seg=[]
        for seg_id,inds in enumerate(blocks):
            if len(inds)<2:continue
            ts=t[inds];ps=p[inds]
            if ts[-1]-ts[0]<window_s:continue
            # 1-s interpolation only within continuous segments; never bridge >20 s.
            grid=np.r_[np.arange(ts[0],ts[-1],1.),ts[-1]]
            pv=np.interp(grid,ts,ps)
            integ=np.r_[0.,np.cumsum((pv[1:]+pv[:-1])*.5*np.diff(grid))]
            eligible=(ts>=ts[0]+window_s/2)&(ts<=ts[-1]-window_s/2)
            tt=ts[eligible]
            mean=(np.interp(tt+window_s/2,grid,integ)-np.interp(tt-window_s/2,grid,integ))/window_s
            hp_t.extend(tt);hp.extend(ps[eligible]-mean);hp_mean.extend(mean);hp_seg.extend([seg_id]*len(tt))
        hp_t=np.array(hp_t);hp=np.array(hp);hp_seg=np.array(hp_seg)
        edges=np.unique(np.r_[np.geomspace(2,300,20),30.])
        def sf(tt,values,segments):
            counts=np.zeros(len(edges)-1);sums=counts.copy();lags=counts.copy()
            for i in range(len(tt)):
                right=np.searchsorted(tt,tt[i]+edges[-1],side='right')
                dt=tt[i+1:right]-tt[i];diff=values[i+1:right]-values[i]
                ok=segments[i+1:right]==segments[i]
                counts+=np.histogram(dt[ok],edges)[0]
                sums+=np.histogram(dt[ok],edges,weights=diff[ok]**2)[0]
                lags+=np.histogram(dt[ok],edges,weights=dt[ok])[0]
            good=counts>=10
            result={'lag_s':(lags[good]/counts[good]).tolist(),'d2_hpa2':(sums[good]/counts[good]).tolist(),'pair_count':counts[good].astype(int).tolist()}
            result['fit']=fit_structure(result)
            return result
        segs=np.zeros(len(t),dtype=int)
        for i,inds in enumerate(blocks):segs[inds]=i
        channels.append({'hz':hz,'utc':t.tolist(),'pressure_hpa':p.tolist(),'outlier_utc':bad_t.tolist(),'outlier_pressure_hpa':bad_p.tolist(),'prime_utc':hp_t.tolist(),'rho_prime_hpa':hp.tolist(),'rho_mean_hpa':list(hp_mean),'raw_structure':sf(t,p,segs),'prime_structure':sf(hp_t,hp,hp_seg),'removed':len(candidates)-len(t),'window_s':window_s})
    return {'channels':channels,'method':'Pressure rho in hPa; rho prime = rho minus centered 300 s boxcar mean. Locally detrended Hampel (Theil-Sen slope), 9 adjacent samples, threshold max(0.5 hPa, 6 scaled MAD); range 0.1..1100 hPa. Interpolate at 1 s within gaps <=20 s. Full-window support only. D2=mean squared increments, actual lag bins 2..300 s, >=10 pairs, no pairs across >20 s gaps.'}
