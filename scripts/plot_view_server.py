"""Loopback-only plot view controls; never changes SDR settings or recordings."""
import json,math,threading,subprocess,time,os
from file_lock import FileLock
from pathlib import Path
from http.server import ThreadingHTTPServer,BaseHTTPRequestHandler
ROOT=Path(__file__).resolve().parents[1]
view_lock=threading.Lock()
def render():
    with FileLock(ROOT/'build/plot-render.lock'):
        exe='flight-plots.exe' if os.name=='nt' else 'flight-plots'
        binary=ROOT/'bin'/exe
        if not binary.exists():binary=ROOT/'tools/flight-plots/target/release'/exe
        result=subprocess.run([str(binary)],cwd=ROOT,capture_output=True,timeout=90)
        if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace')[-4000:])
class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def reply_headers(self,status):
        self.send_response(status);self.send_header('Access-Control-Allow-Origin','http://127.0.0.1:8096');self.send_header('Content-Type','application/json');self.end_headers()
    def do_OPTIONS(self):
        self.send_response(204);self.send_header('Access-Control-Allow-Origin','http://127.0.0.1:8096');self.send_header('Access-Control-Allow-Methods','POST, OPTIONS');self.send_header('Access-Control-Allow-Headers','Content-Type');self.end_headers()
    def do_POST(self):
        try:
            if self.path!='/view' or self.headers.get('Origin','http://127.0.0.1:8096')!='http://127.0.0.1:8096':raise ValueError('Unsupported request')
            size=int(self.headers.get('Content-Length','0'))
            if not 0<size<4096:raise ValueError('Invalid request size')
            request=json.loads(self.rfile.read(size));name=request.get('plot','visibility')
            if name not in ('map','map-all','pressure','reynolds','structure','visibility'):raise ValueError('Unknown plot')
            path=ROOT/'web/map-data/views.json'
            with view_lock:
                views=json.loads(path.read_text()) if path.exists() else {}
                if name=='visibility':
                    views['visible_hz']=[hz for hz in [433500000,434000000] if hz in request.get('visible_hz',[])]
                    for key in ['raw_structure','prime_structure']:
                        if key in request:views[key]=bool(request[key])
                elif request.get('reset'):views.pop(name,None)
                else:
                    bounds={key:[float(x) for x in request[key]] for key in ('xlim','ylim')}
                    if any(len(v)!=2 or not all(math.isfinite(x) and abs(x)<1e12 for x in v) or not v[1]>v[0] for v in bounds.values()):raise ValueError('Invalid limits')
                    views[name]=bounds
                tmp=path.with_suffix('.tmp');tmp.write_text(json.dumps(views));tmp.replace(path)
            render()
            axes=json.loads((ROOT/f'web/map-data/{name}.axes.json').read_text()) if name!='visibility' else None
            self.reply_headers(200);self.wfile.write(json.dumps({'axes':axes,'nonce':time.time()}).encode())
        except Exception as e:
            self.reply_headers(400);self.wfile.write(json.dumps({'error':str(e)}).encode())
def start():
    server=ThreadingHTTPServer(('127.0.0.1',8097),Handler)
    threading.Thread(target=server.serve_forever,daemon=True).start()
