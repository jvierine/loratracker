use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        State,
        Path as UrlPath,
    },
    routing::{get, post},
    Json, Router,
};
mod rtl;
use rtl::*;
use std::path::{Path, PathBuf};
use rustfft::{num_complex::Complex32, FftPlanner};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    ffi::c_void,
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::broadcast;
use tower_http::services::ServeDir;
#[cfg(unix)]
unsafe extern "C" {
    fn H5Fget_vfd_handle(file: i64, plist: i64, handle: *mut *mut c_void) -> i32;
    fn fcntl(fd: i32, cmd: i32, arg: i32) -> i32;
}
const RATE: u32 = 2_000_000;
const FFT: usize = 2048;
fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct Config {
    data_dir: String,
    center_hz: u32,
    carrier_hz: u32,
    agc: bool,
    gain_db: f64,
    ppm: i32,
    scanning: bool,
    dwell_s: u64,
    bandwidth: u32,
    sf: u8,
    ldro: i8,
    recording: bool,
    record_iq: bool,
    channels_hz: Vec<u32>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: std::env::var("STATION_DATA_DIR").unwrap_or_else(|_| "recordings".into()),
            center_hz: 433_750_000,
            carrier_hz: 433_500_000,
            agc: true,
            gain_db: 28.0,
            ppm: 0,
            scanning: false,
            dwell_s: 12,
            bandwidth: 0,
            sf: 0,
            ldro: 0,
            recording: true,
            record_iq: true,
            channels_hz: vec![433500000, 434000000],
        }
    }
}
#[derive(Clone)]
struct App {
    cfg: Arc<Mutex<Config>>,
    status: Arc<Mutex<Value>>,
    packets: Arc<Mutex<Vec<Value>>>,
    events: broadcast::Sender<String>,
    dropped: Arc<AtomicU64>,
    running: Arc<AtomicBool>,
    review: bool,
    shutdown: Arc<tokio::sync::Notify>,
}
struct Block {
    first_sample_index: u64,
    data: Vec<u8>,
    cfg: Config,
    utc: f64,
    generation: u64,
}
struct Capture {
    sample_counter: u64,
    app: App,
    tx: std::sync::mpsc::SyncSender<Block>,
    dev: *mut c_void,
    active: Config,
    settle: usize,
    generation: u64,
}
unsafe extern "C" fn callback(buf: *mut u8, len: u32, ctx: *mut c_void) {
    let c = &mut *(ctx as *mut Capture);
    let first_sample_index = c.sample_counter;
    c.sample_counter += (len / 2) as u64;
    if !c.app.running.load(Ordering::Relaxed) {
        rtlsdr_cancel_async(c.dev);
        return;
    }
    let req = c.app.cfg.lock().unwrap().clone();
    if req.center_hz != c.active.center_hz
        || req.agc != c.active.agc
        || req.gain_db != c.active.gain_db
        || req.ppm != c.active.ppm
    {
        rtlsdr_cancel_async(c.dev);
        return;
    }
    c.active = req.clone();
    if c.settle > 0 {
        c.settle -= 1;
        return;
    }
    let b = Block {
        first_sample_index,
        data: std::slice::from_raw_parts(buf, len as usize).to_vec(),
        cfg: req,
        utc: now(),
        generation: c.generation,
    };
    if c.tx.try_send(b).is_err() {
        c.app.dropped.fetch_add(1, Ordering::Relaxed);
    }
}
fn capture(app: App, tx: std::sync::mpsc::SyncSender<Block>) {
    if let Err(error) = rtl::load() {
        *app.status.lock().unwrap() = json!({"hardware":error});
        return;
    }
    unsafe {
        if rtlsdr_get_device_count() == 0 {
            *app.status.lock().unwrap() = json!({"hardware":"No RTL-SDR connected"});
            return;
        }
        let mut dev = std::ptr::null_mut();
        if rtlsdr_open(&mut dev, 0) < 0 {
            *app.status.lock().unwrap() =
                json!({"hardware":"Cannot open RTL-SDR; device may be busy"});
            return;
        }
        let mut c = Box::new(Capture {
            sample_counter: 0,
            active: Config {
                center_hz: 0,
                agc: false,
                ..Config::default()
            },
            app: app.clone(),
            tx,
            dev,
            settle: 40,
            generation: 0,
        });
        if rtlsdr_set_sample_rate(dev, RATE) < 0 {
            *app.status.lock().unwrap() = json!({"hardware":"Sample-rate configuration failed"});
            rtlsdr_close(dev);
            return;
        }
        rtlsdr_reset_buffer(dev);
        *app.status.lock().unwrap() = json!({"hardware":"RTL2838UHIDIR · R820T","sample_rate_hz":RATE,"message":"Connected; AGC enabled"});
        loop {
            if !app.running.load(Ordering::Relaxed) {
                break;
            }
            let req = app.cfg.lock().unwrap().clone();
            let ppm_rc = rtlsdr_set_freq_correction(dev, req.ppm);
            let results = [
                rtlsdr_set_center_freq(dev, req.center_hz),
                if ppm_rc == -2 { 0 } else { ppm_rc },
                rtlsdr_set_tuner_gain_mode(dev, if req.agc { 0 } else { 1 }),
                rtlsdr_set_agc_mode(dev, req.agc as i32),
            ];
            if results.iter().any(|&rc| rc < 0) {
                *app.status.lock().unwrap() =
                    json!({"hardware":"RTL-SDR configuration failed","codes":results});
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            if !req.agc {
                rtlsdr_set_tuner_gain(dev, (req.gain_db * 10.) as i32);
            }
            c.active = req;
            c.generation += 1;
            c.settle = 40;
            rtlsdr_reset_buffer(dev);
            let rc = rtlsdr_read_async(
                dev,
                callback,
                (&mut *c) as *mut Capture as *mut c_void,
                0,
                16384,
            );
            if rc < 0 {
                *app.status.lock().unwrap() = json!({"hardware":"USB capture failed","code":rc});
                break;
            }
        }
        rtlsdr_close(dev);
        *app.status.lock().unwrap() = json!({"hardware":"Capture stopped"});
    }
}
struct Recorder {
    file: hdf5::File,
    packets: hdf5::Dataset,
    payloads: hdf5::Dataset,
    utc: hdf5::Dataset,
    n: usize,
    path: String,
    last_flush: Instant,
}
impl Recorder {
    fn open(cfg: &Config) -> hdf5::Result<Self> {
        std::fs::create_dir_all(&cfg.data_dir).ok();
        let path = Path::new(&cfg.data_dir).join(format!("flight-{:.0}.h5", now() * 1000.)).to_string_lossy().into_owned();
        let file = hdf5::File::create(&path)?;
        close_on_exec(&file);
        file.new_attr::<u32>()
            .create("schema_version")?
            .write_scalar(&1)?;
        file.new_attr::<u32>()
            .create("sample_rate_hz")?
            .write_scalar(&RATE)?;
        use std::str::FromStr;
        for (key,value) in [("source","RTL-SDR librtlsdr C API; live USB samples".to_string()),("initial_config_json",serde_json::to_string(cfg).unwrap()),("iq_encoding","uint8 interleaved I,Q; normalized=(ADC-127.5)/128".into()),("iq_blocks_columns","callback_utc_unix,first_complex_sample,count,center_hz,agc_enabled,requested_manual_gain_db,ppm".into()),("timestamp_note","UTC timestamps refer to host callback/decoder receipt, not calibrated RF arrival".into()),("packet_format","JSON with exact payload_hex, PHY settings, CRC presence/status, acceptance flag and parsed CanSat fields; absent CRC means content unverified".into())] {let v=hdf5::types::VarLenUnicode::from_str(&value).unwrap();file.new_attr::<hdf5::types::VarLenUnicode>().create(key)?.write_scalar(&v)?;}
        let packets = file
            .new_dataset::<hdf5::types::VarLenUnicode>()
            .shape(0..)
            .chunk(64)
            .create("packets_json")?;
        let payloads = file
            .new_dataset::<hdf5::types::VarLenArray<u8>>()
            .shape(0..)
            .chunk(64)
            .create("payload_bytes")?;
        let utc = file
            .new_dataset::<f64>()
            .shape(0..)
            .chunk(64)
            .create("packet_utc_unix")?;
        Ok(Self {
            file,
            packets,
            payloads,
            utc,
            n: 0,
            path,
            last_flush: Instant::now(),
        })
    }
    fn packet(&mut self, v: &Value) -> hdf5::Result<()> {
        use std::str::FromStr;
        let s = hdf5::types::VarLenUnicode::from_str(&v.to_string()).unwrap();
        let hex = v["payload_hex"].as_str().unwrap_or("");
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
            .collect();
        self.payloads.resize(self.n + 1)?;
        self.payloads.write_slice(
            &[hdf5::types::VarLenArray::from_slice(&bytes)],
            self.n..self.n + 1,
        )?;
        self.utc.resize(self.n + 1)?;
        self.utc
            .write_slice(&[v["utc"].as_f64().unwrap_or(0.)], self.n..self.n + 1)?;
        self.packets.resize(self.n + 1)?;
        self.packets.write_slice(&[s], self.n..self.n + 1)?;
        self.n += 1;
        self.file.flush()?;
        Ok(())
    }
}
// Raw ADC recording is independent of packet decoding, UI pause and telemetry logs.
const RAW_FILE_BYTES: usize = 4_000_000;
fn close_on_exec(file: &hdf5::File) {
    #[cfg(not(unix))]
    let _ = file;
    #[cfg(unix)]
    unsafe {
        let mut handle = std::ptr::null_mut();
        if H5Fget_vfd_handle(file.id(), 0, &mut handle) >= 0 && !handle.is_null() {
            fcntl(*(handle as *const i32), 2, 1);
        }
    }
}
fn attr_text(file: &hdf5::File, key: &str, value: &str) -> hdf5::Result<()> {
    use std::str::FromStr;
    let v = hdf5::types::VarLenUnicode::from_str(value).unwrap();
    file.new_attr::<hdf5::types::VarLenUnicode>()
        .create(key)?
        .write_scalar(&v)
}
struct RawPart {
    file: hdf5::File,
    iq: hdf5::Dataset,
    blocks: hdf5::Dataset,
    n: usize,
    nb: usize,
    path: String,
}
impl RawPart {
    fn open(
        b: &Block,
        session: &str,
        seq: u64,
        first: u64,
        utc: f64,
        gap: u64,
        unknown: bool,
    ) -> hdf5::Result<Self> {
        let path = Path::new(&b.cfg.data_dir).join(format!("iq-{session}-{seq:08}.h5")).to_string_lossy().into_owned();
        let file = hdf5::File::create(&path)?;
        close_on_exec(&file);
        for (k, v) in [
            ("sample_rate_hz", RATE as u64),
            ("center_hz", b.cfg.center_hz as u64),
            ("sequence_index", seq),
            ("first_sample_index", first),
            ("valid_complex_samples", 0),
            ("target_iq_bytes", RAW_FILE_BYTES as u64),
            ("gap_before_samples", gap),
            ("stream_generation", b.generation),
        ] {
            file.new_attr::<u64>().create(k)?.write_scalar(&v)?;
        }
        for (k, v) in [
            ("start_utc_unix", utc),
            ("last_sample_utc_unix", utc),
            ("agc_enabled", b.cfg.agc as u8 as f64),
            ("requested_gain_db", b.cfg.gain_db),
            ("ppm", b.cfg.ppm as f64),
        ] {
            file.new_attr::<f64>().create(k)?.write_scalar(&v)?;
        }
        for (k, v) in [("complete", 0u8), ("unknown_gap_before", unknown as u8)] {
            file.new_attr::<u8>().create(k)?.write_scalar(&v)?;
        }
        attr_text(
            &file,
            "encoding",
            "unsigned uint8 I,Q; normalization=(value-127.5)/128; no compression or decimation",
        )?;
        attr_text(&file,"clock","UTC anchored to first USB callback minus its block duration; subsequent timestamps follow nominal sample rate. USB delivery latency and oscillator error are not calibrated.")?;
        attr_text(&file, "session", session)?;
        attr_text(
            &file,
            "config_json",
            &serde_json::to_string(&b.cfg).unwrap(),
        )?;
        attr_text(
            &file,
            "blocks_columns",
            "first_sample_index,count,callback_utc_unix,first_sample_utc_estimate",
        )?;
        let iq = file
            .new_dataset::<u8>()
            .shape((0.., 2))
            .chunk((8192, 2))
            .create("iq_u8")?;
        let blocks = file
            .new_dataset::<f64>()
            .shape((0.., 4))
            .chunk((128, 4))
            .create("blocks")?;
        Ok(Self {
            file,
            iq,
            blocks,
            n: 0,
            nb: 0,
            path,
        })
    }
    fn write(&mut self, bytes: &[u8], first: u64, callback: f64, utc: f64) -> hdf5::Result<()> {
        let n = bytes.len() / 2;
        self.iq.resize((self.n + n, 2))?;
        self.iq.write_slice(
            ndarray::ArrayView2::from_shape((n, 2), bytes).unwrap(),
            (self.n..self.n + n, ..),
        )?;
        self.blocks.resize((self.nb + 1, 4))?;
        let row = [first as f64, n as f64, callback, utc];
        self.blocks.write_slice(
            ndarray::ArrayView2::from_shape((1, 4), &row).unwrap(),
            (self.nb..self.nb + 1, ..),
        )?;
        self.n += n;
        self.nb += 1;
        self.file
            .attr("valid_complex_samples")?
            .write_scalar(&(self.n as u64))?;
        self.file
            .attr("last_sample_utc_unix")?
            .write_scalar(&(utc + (n - 1) as f64 / RATE as f64))?;
        Ok(())
    }
    fn finish(&self) -> hdf5::Result<()> {
        self.file
            .attr("complete")?
            .write_scalar(&((self.n * 2 == RAW_FILE_BYTES) as u8))?;
        self.file.flush()
    }
}
impl Drop for RawPart {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
struct RawRecorder {
    part: Option<RawPart>,
    session: String,
    seq: u64,
    expected: Option<u64>,
    generation: Option<u64>,
    anchor: f64,
    bytes: u64,
    error: Option<String>,
    latest: String,
}
impl RawRecorder {
    fn new() -> Self {
        Self {
            part: None,
            session: format!("{:.0}", now() * 1000.),
            seq: 0,
            expected: None,
            generation: None,
            anchor: 0.,
            bytes: 0,
            error: None,
            latest: String::new(),
        }
    }
    fn push(&mut self, b: &Block) -> hdf5::Result<()> {
        let count = (b.data.len() / 2) as u64;
        let gap = self
            .expected
            .map(|e| b.first_sample_index.saturating_sub(e))
            .unwrap_or(0);
        let changed = self.generation.is_some_and(|g| g != b.generation);
        let first = self.expected.is_none();
        if first || gap > 0 || changed {
            self.part = None;
            self.anchor = b.utc - (b.first_sample_index + count) as f64 / RATE as f64;
        }
        let mut offset = 0;
        while offset < b.data.len() {
            if self.part.is_none() {
                let index = b.first_sample_index + (offset / 2) as u64;
                let p = RawPart::open(
                    b,
                    &self.session,
                    self.seq,
                    index,
                    self.anchor + index as f64 / RATE as f64,
                    if offset == 0 { gap } else { 0 },
                    changed && offset == 0,
                )?;
                self.latest = p.path.clone();
                self.seq += 1;
                self.part = Some(p);
            }
            let p = self.part.as_mut().unwrap();
            let take = (RAW_FILE_BYTES - p.n * 2).min(b.data.len() - offset);
            let index = b.first_sample_index + (offset / 2) as u64;
            p.write(
                &b.data[offset..offset + take],
                index,
                b.utc,
                self.anchor + index as f64 / RATE as f64,
            )?;
            offset += take;
            self.bytes += take as u64;
            if p.n * 2 == RAW_FILE_BYTES {
                self.part = None;
            }
        }
        self.expected = Some(b.first_sample_index + count);
        self.generation = Some(b.generation);
        Ok(())
    }
    fn pause(&mut self) {
        self.part = None;
        self.expected = None;
        self.generation = None;
    }
}

struct Decoder {
    child: Child,
    input: Option<std::process::ChildStdin>,
    key: String,
}
impl Drop for Decoder {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn start_decoder(
    cfg: &Config,
    app: App,
    record_tx: std::sync::mpsc::Sender<Value>,
    channels: &[u32],
) -> Option<Decoder> {
    let sfs = if cfg.sf == 0 {
        "7,8,9,10,11,12".to_string()
    } else {
        cfg.sf.to_string()
    };
    let key = format!(
        "{}:{}:{}:{:?}:{}",
        cfg.center_hz,
        cfg.bandwidth,
        sfs,
        channels,
        format!("{}:{}", cfg.scanning, cfg.ldro)
    );
    let bws = if cfg.bandwidth == 0 {
        "62500,125000,250000,500000".to_string()
    } else {
        cfg.bandwidth.to_string()
    };
    let chans = channels
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join(",");
    // HDF5 drivers may open extra file descriptors after file creation.
    // Keep every non-stdio parent descriptor out of the decoder subprocess.
    #[cfg(unix)]
    unsafe {
        for fd in 3..1024 {
            let flags = fcntl(fd, 1, 0);
            if flags >= 0 {
                fcntl(fd, 2, flags | 1);
            }
        }
    }
    let mut child = Command::new(std::env::var_os("STATION_DECODER").unwrap_or_else(|| if cfg!(windows) {"./build/lora_rx.exe".into()} else {"./build/lora_rx".into()}))
        .args([
            "-r",
            "-",
            "-f",
            &cfg.center_hz.to_string(),
            "-C",
            &chans,
            "-s",
            &RATE.to_string(),
            "-S",
            &sfs,
            "-b",
            &bws,
            "-l",
            &cfg.ldro.to_string(),
            "-w",
            "0",
            "-p",
            "0",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("build/decoder.log")
                .ok()?,
        ))
        .spawn()
        .ok()?;
    let input = child.stdin.take();
    let out = child.stdout.take()?;
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let Some(raw) = line.strip_prefix("packet: ") {
                if let Ok(mut p) = serde_json::from_str::<Value>(raw) {
                    enrich(&mut p);
                    let mut store = app.packets.lock().unwrap();
                    store.push(p.clone());
                    if store.len() > 500 {
                        store.remove(0);
                    }
                    drop(store);
                    let _ = record_tx.send(p.clone());
                    let _ = app
                        .events
                        .send(json!({"type":"packet","packet":p}).to_string());
                }
            }
        }
    });
    Some(Decoder { child, input, key })
}
fn enrich(p: &mut Value) {
    let raw = p["payload_hex"].as_str().unwrap_or("");
    let bytes: Vec<u8> = (0..raw.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&raw[i..i + 2], 16).ok())
        .collect();
    let id = if bytes.len() > 1 {
        u16::from_le_bytes([bytes[0], bytes[1]])
    } else {
        0
    };
    let is_cansat = (10000..16001).contains(&id);
    let data = if is_cansat { &bytes[2..] } else { &bytes[..] };
    p["cansat_id"] = if is_cansat { json!(id) } else { Value::Null };
    p["text"] = json!(String::from_utf8_lossy(data).to_string());
    p["cansat_match"] = json!(is_cansat);
    if is_cansat {
        p["id_frequency_hz"] = json!((420000 + id as u32) * 1000);
    }
    let readable = std::str::from_utf8(data).is_ok()
        && data
            .iter()
            .filter(|&&x| x < 32 && ![9, 10, 13].contains(&x))
            .count()
            == 0;
    let freq_match = p["id_frequency_hz"]
        .as_u64()
        .is_some_and(|f| f.abs_diff(p["frequency_hz"].as_u64().unwrap_or(0)) < 40000);
    let numeric_shape = std::str::from_utf8(data).ok().is_some_and(|t| {
        t.trim().split(';').skip(1).count() >= 2
            && t.trim()
                .split(';')
                .skip(1)
                .all(|v| v.trim().parse::<f64>().is_ok_and(f64::is_finite))
    });
    let plausible = is_cansat && readable && freq_match && numeric_shape;
    let binary_candidate = is_cansat
        && freq_match
        && !readable
        && p["snr_db"].as_f64().unwrap_or(-100.) > -5.
        && bytes.len() >= 6;
    p["payload_format"] = json!(if readable { "text" } else { "binary" });
    p["accepted"] = json!((plausible || binary_candidate) && p["crc_valid"] != false);
    p["integrity"] = json!(if p["has_crc"] == false {
        "no payload CRC; content is unverified"
    } else if p["crc_valid"] == true {
        "CRC verified"
    } else {
        "CRC failed"
    });
    if bytes.len() == 27 && [13500, 14000].contains(&id) {
        let u16at = |i| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        let i32at = |i| i32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        let latitude = f32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let longitude = f32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let gps_available = latitude.is_finite() && longitude.is_finite() && latitude.abs() <= 90. && longitude.abs() <= 180. && (latitude != 0. || longitude != 0.);
        let pressure = f32::from_le_bytes(bytes[19..23].try_into().unwrap());
        if pressure.is_finite() {
            p["fields"] = json!({"schema":"andoya_binary27_partial","time_counter_u16":u16at(2),"latitude_deg":if latitude.is_finite(){Some(latitude)}else{None},"longitude_deg":if longitude.is_finite(){Some(longitude)}else{None},"gps_coordinates_available":gps_available,"gps_fix_status":"unknown; payload CRC absent and field14 meaning unconfirmed","latitude_raw_i32":i32at(4),"longitude_raw_i32":i32at(8),"altitude_raw_i16":i16::from_le_bytes([bytes[12],bytes[13]]),"field14_u8":bytes[14],"field15_u16_div16":u16at(15)as f64/16.,"field17_u8":bytes[17],"field18_u8":bytes[18],"pressure_pa":pressure,"field23_u16":u16at(23),"field25_u8":bytes[25],"field26_u8":bytes[26],"layout_status":"GPS float32 little-endian decimal degrees at offsets 4 and 8 inferred from repeated air packets; fix status and final sensor/checksum fields unconfirmed"});
        }
        return;
    }
    if let Ok(v) = serde_json::from_slice::<Value>(data) {
        p["fields"] = v;
    } else if plausible {
        let text = String::from_utf8_lossy(data);
        let cols: Vec<&str> = text.trim().split(';').map(str::trim).collect();
        p["fields"] = json!({"label":cols[0],"values":cols[1..].iter().map(|s|s.parse::<f64>().ok()).collect::<Vec<_>>()});
    }
}
fn dsp(app: App, rx: std::sync::mpsc::Receiver<Block>) {
    let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT);
    let win: Vec<f32> = (0..FFT)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FFT as f32).cos())
        .collect();
    let (ptx, prx) = std::sync::mpsc::channel();
    let mut recorder: Option<Recorder> = None;
    let mut raw = RawRecorder::new();
    let mut storage_dir = String::new();
    let mut decoder: Option<Decoder> = None;
    let mut generation = u64::MAX;
    let mut last = Instant::now();
    let mut powers = vec![0f32; FFT];
    let mut count = 0;
    let mut samples = 0u64;
    let mut candidates = vec![];
    let mut decode_at = Instant::now();
    let mut chan_cursor = 0usize;
    for b in rx {
        if storage_dir != b.cfg.data_dir {
            recorder = None;
            raw.pause();
            raw.error = None;
            storage_dir = b.cfg.data_dir.clone();
        }
        if generation != b.generation {
            decoder = None;
            generation = b.generation;
            powers.fill(0.);
            count = 0;
            candidates.clear();
            decode_at = Instant::now();
        }
        if b.cfg.recording && recorder.is_none() {
            match Recorder::open(&b.cfg) {
                Ok(r) => {
                    let _ = app
                        .events
                        .send(json!({"type":"recording","path":r.path}).to_string());
                    recorder = Some(r)
                }
                Err(e) => {
                    app.status.lock().unwrap()["recording_error"] = json!(e.to_string());
                }
            }
        }
        if !b.cfg.recording {
            recorder = None;
        }
        while let Ok(p) = prx.try_recv() {
            if let Some(r) = recorder.as_mut() {
                if let Err(e) = r.packet(&p) {
                    app.status.lock().unwrap()["recording_error"] = json!(e.to_string());
                    recorder = None;
                    break;
                }
            }
        }
        if b.cfg.record_iq && raw.error.is_none() {
            if let Err(e) = raw.push(&b) {
                raw.error = Some(e.to_string());
                raw.pause();
            }
        } else if !b.cfg.record_iq {
            raw.pause();
        }

        if let Some(r) = recorder.as_mut() {
            if r.last_flush.elapsed() > Duration::from_secs(2) {
                let _ = r.file.flush();
                r.last_flush = Instant::now();
            }
        }
        samples += (b.data.len() / 2) as u64;
        for chunk in b.data.chunks_exact(FFT * 2).step_by(2) {
            let mut bins: Vec<Complex32> = chunk
                .chunks_exact(2)
                .zip(&win)
                .map(|(q, w)| {
                    Complex32::new(
                        (q[0] as f32 - 127.5) / 128. * w,
                        (q[1] as f32 - 127.5) / 128. * w,
                    )
                })
                .collect();
            fft.process(&mut bins);
            for (i, x) in bins.iter().enumerate() {
                powers[(i + FFT / 2) % FFT] += x.norm_sqr();
            }
            count += 1;
        }
        if let Some(d) = decoder.as_mut() {
            if d.child.try_wait().ok().flatten().is_some()
                || d.input
                    .as_mut()
                    .is_some_and(|i| i.write_all(&b.data).is_err())
            {
                decoder = None;
            }
        }
        if last.elapsed() > Duration::from_millis(90) && count > 0 {
            let spec: Vec<f32> = powers
                .iter()
                .map(|x| {
                    10. * (x / count as f32 / (FFT as f32 * FFT as f32))
                        .max(1e-14)
                        .log10()
                })
                .collect();
            if b.cfg.scanning && decode_at.elapsed() > Duration::from_secs(3) {
                // Rotate a frequency raster; every carrier in the usable band is searched.
                let half = 650000i64;
                let step = (b.cfg.bandwidth / 2).max(15625) as i64;
                let lo = b.cfg.center_hz as i64 - half;
                let n = (2 * half / step) as usize;
                candidates = (0..2)
                    .map(|j| (lo + ((chan_cursor + j) % n) as i64 * step) as u32)
                    .collect();
                chan_cursor = (chan_cursor + 2) % n;
                decoder = None;
                decode_at = Instant::now();
            }
            if !b.cfg.scanning {
                candidates = if b.cfg.channels_hz.is_empty() {
                    vec![b.cfg.carrier_hz]
                } else {
                    b.cfg.channels_hz.clone()
                };
            }
            if candidates.is_empty() {
                candidates = if b.cfg.channels_hz.is_empty() {
                    vec![b.cfg.carrier_hz]
                } else {
                    b.cfg.channels_hz.clone()
                };
            }
            let key = format!(
                "{}:{}:{}:{:?}:{}",
                b.cfg.center_hz,
                b.cfg.bandwidth,
                if b.cfg.sf == 0 {
                    "7,8,9,10,11,12".into()
                } else {
                    b.cfg.sf.to_string()
                },
                candidates,
                format!("{}:{}", b.cfg.scanning, b.cfg.ldro)
            );
            if decoder.as_ref().is_none_or(|d| d.key != key) {
                decoder = start_decoder(&b.cfg, app.clone(), ptx.clone(), &candidates);
            }
            let stride = 1;
            let iq: Vec<[f32; 2]> = b
                .data
                .chunks_exact(2)
                .step_by(stride)
                .take(512)
                .map(|v| [(v[0] as f32 - 127.5) / 128., (v[1] as f32 - 127.5) / 128.])
                .collect();
            let clip =
                b.data.iter().filter(|&&v| v < 3 || v > 252).count() as f64 / b.data.len() as f64;
            let rec = recorder.as_ref().map(|r| r.path.clone());
            let _=app.events.send(json!({"type":"frame","utc":b.utc,"center_hz":b.cfg.center_hz,"rate":RATE,"spectrum":spec,"iq":iq,"clip_fraction":clip,"samples":samples,"dropped_blocks":app.dropped.load(Ordering::Relaxed),"recording":rec,"iq_recording":if b.cfg.record_iq{Some(raw.latest.clone())}else{None},"iq_bytes":raw.bytes,"iq_error":raw.error,"decoder_channels":candidates,"config":b.cfg}).to_string());
            *app.status.lock().unwrap() = json!({"hardware":"RTL2838UHIDIR · R820T","sample_rate_hz":RATE,"samples":samples,"center_hz":b.cfg.center_hz,"recording":rec,"iq_recording":if b.cfg.record_iq{Some(raw.latest.clone())}else{None},"iq_bytes":raw.bytes,"iq_error":raw.error,"decoder_running":decoder.is_some(),"dropped_blocks":app.dropped.load(Ordering::Relaxed),"clip_fraction":clip});
            powers.fill(0.);
            count = 0;
            last = Instant::now();
        }
    }
}
async fn status(State(a): State<App>) -> Json<Value> {
    Json(
        json!({"status":a.status.lock().unwrap().clone(),"config":a.cfg.lock().unwrap().clone(),"review":a.review,"packets":a.packets.lock().unwrap().clone()}),
    )
}
async fn config(
    State(a): State<App>,
    Json(mut c): Json<Config>,
) -> Result<Json<Config>, (axum::http::StatusCode, String)> {
    if ![-1, 0, 1].contains(&c.ldro)
        || !(420_000_000..=450_000_000).contains(&c.center_hz)
        || !(420_000_000..=450_000_000).contains(&c.carrier_hz)
        || ![0, 62500, 125000, 250000, 500000].contains(&c.bandwidth)
        || !(c.sf == 0 || (7..=12).contains(&c.sf))
        || c.dwell_s < 5
        || c.dwell_s > 600
        || !c.gain_db.is_finite()
        || !(0.0..=49.6).contains(&c.gain_db)
        || c.channels_hz.len() > 4
        || c.channels_hz
            .iter()
            .any(|&f| !(420_000_000..=450_000_000).contains(&f))
        || c.ppm.unsigned_abs() > 200
    {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid receiver settings".into(),
        ));
    }
    c.data_dir = prepare_data_dir(&c.data_dir, a.review).map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e))?;
    if a.review { c.record_iq=false; c.recording=false; c.scanning=false; }
    let changed = a.cfg.lock().unwrap().data_dir != c.data_dir;
    std::fs::write("settings.json", serde_json::to_vec_pretty(&c).unwrap())
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    *a.cfg.lock().unwrap() = c.clone();
    if changed && a.review {
        let dir = c.data_dir.clone();
        let packets = tokio::task::spawn_blocking(move || review_packets(&dir)).await
            .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        *a.packets.lock().unwrap() = packets;
    }
    Ok(Json(c))
}
async fn websocket(
    ws: WebSocketUpgrade,
    State(a): State<App>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |mut sock| async move {
        let mut rx = a.events.subscribe();
        while let Ok(s) = rx.recv().await {
            if sock.send(Message::Text(s.into())).await.is_err() {
                break;
            }
        }
    })
}
async fn recordings(State(a): State<App>) -> Json<Value> {
    let mut v = vec![];
    if let Ok(it) = std::fs::read_dir(&a.cfg.lock().unwrap().data_dir) {
        for e in it.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "h5") {
                v.push(json!({"name":e.file_name().to_string_lossy(),"bytes":e.metadata().map(|m|m.len()).unwrap_or(0)}));
            }
        }
    }
    Json(json!(v))
}
fn prepare_data_dir(value: &str, review: bool) -> Result<String, String> {
    if value.trim().is_empty() { return Err("Choose a data directory".into()); }
    let path = PathBuf::from(value);
    // Selecting a missing external drive must fail; never silently fall back.
    if !review { std::fs::create_dir_all(&path).map_err(|e| e.to_string())?; }
    if !path.is_dir() { return Err("Data directory does not exist or is not mounted".into()); }
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}
fn review_packets(dir: &str) -> Vec<Value> {
    let mut packets = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("flight-") || !name.ends_with(".h5") { continue; }
            let read = || -> hdf5::Result<Vec<hdf5::types::VarLenUnicode>> {
                hdf5::File::open(entry.path())?.dataset("packets_json")?.read_raw()
            };
            match read() {
                Ok(rows) => for row in rows { if let Ok(mut p) = serde_json::from_str::<Value>(row.as_str()) { enrich(&mut p); packets.push(p); } },
                Err(error) => eprintln!("Cannot read {}: {error}", entry.path().display()),
            }
        }
    }
    packets.sort_by(|a,b| a["utc"].as_f64().unwrap_or(0.).total_cmp(&b["utc"].as_f64().unwrap_or(0.)));
    packets.dedup_by(|a,b| a["utc"]==b["utc"] && a["frequency_hz"]==b["frequency_hz"] && a["payload_hex"]==b["payload_hex"]);
    if packets.len()>500 { packets.drain(..packets.len()-500); }
    packets
}
async fn recording_file(State(a): State<App>, UrlPath(name): UrlPath<String>) -> Result<axum::response::Response, (axum::http::StatusCode, String)> {
    use axum::http::StatusCode;
    if name.contains(['/', '\\']) || !name.ends_with(".h5") || name.starts_with('.') { return Err((StatusCode::NOT_FOUND,"Unknown recording".into())); }
    let root = PathBuf::from(&a.cfg.lock().unwrap().data_dir);
    let path = root.join(&name).canonicalize().map_err(|e| (StatusCode::NOT_FOUND,e.to_string()))?;
    if !path.starts_with(&root) { return Err((StatusCode::NOT_FOUND,"Unknown recording".into())); }
    let file = tokio::fs::File::open(path).await.map_err(|e| (StatusCode::NOT_FOUND,e.to_string()))?;
    Ok(axum::response::Response::builder().header("Content-Type","application/x-hdf5")
        .body(axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(file))).unwrap())
}
async fn browse_directory(State(a): State<App>) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    let current = a.cfg.lock().unwrap().data_dir.clone();
    let python = std::env::var_os("STATION_PYTHON").ok_or_else(|| (axum::http::StatusCode::SERVICE_UNAVAILABLE,"Start using the GUI launch script to enable Browse; you can also enter a path.".into()))?;
    let output = tokio::process::Command::new(python).args(["scripts/directory_picker.py", &current]).output().await
        .map_err(|e| (axum::http::StatusCode::SERVICE_UNAVAILABLE,e.to_string()))?;
    if !output.status.success() { return Err((axum::http::StatusCode::SERVICE_UNAVAILABLE,String::from_utf8_lossy(&output.stderr).into_owned())); }
    let value = serde_json::from_slice(&output.stdout).map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR,e.to_string()))?;
    Ok(Json(value))
}
async fn shutdown(State(a): State<App>, Json(_): Json<Value>) -> Json<Value> {
    a.shutdown.notify_one();
    Json(json!({"stopping":true}))
}
#[tokio::main]
async fn main() {
    std::fs::create_dir_all("build").unwrap();
    let review = std::env::var("STATION_REVIEW").as_deref() != Ok("0");
    let mut initial: Config = std::fs::read("settings.json").ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    if let Ok(path) = std::env::var("STATION_DATA_DIR") { initial.data_dir=path; }
    initial.data_dir = prepare_data_dir(&initial.data_dir, review).expect("Cannot open data directory");
    let (events, _) = broadcast::channel(32);
    let app = App {
        cfg: Arc::new(Mutex::new(initial)),
        status: Arc::new(Mutex::new(json!({"hardware":"Connecting"}))),
        packets: Arc::new(Mutex::new(vec![])),
        events,
        dropped: Arc::new(AtomicU64::new(0)),
        running: Arc::new(AtomicBool::new(true)),
        review,
        shutdown: Arc::new(tokio::sync::Notify::new()),
    };
    let (capture_thread, dsp_thread) = if review {
        *app.packets.lock().unwrap() = review_packets(&app.cfg.lock().unwrap().data_dir);
        { let mut c = app.cfg.lock().unwrap(); c.record_iq=false; c.recording=false; c.scanning=false; }
        *app.status.lock().unwrap()=json!({"hardware":"Recorded flight review", "decoder_running":false,"recording":null,"iq_recording":null,"iq_bytes":0,"samples":0,"dropped_blocks":0,"sample_rate_hz":RATE,"center_hz":433450000,"review":true});
        (None,None)
    } else {
        let (tx, rx) = std::sync::mpsc::sync_channel(512);
        let a = app.clone(); let capture_thread=std::thread::spawn(move || capture(a,tx));
        let a = app.clone(); let dsp_thread=std::thread::spawn(move || dsp(a,rx));
        (Some(capture_thread),Some(dsp_thread))
    };
    let a = app.clone();
    std::thread::spawn(move || {
        let centers = [
            433800000, 434300000, 432700000, 431300000, 430700000, 435300000,
        ];
        let mut i = 0;
        let mut t = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(200));
            let mut c = a.cfg.lock().unwrap();
            if !c.scanning {
                t = Instant::now();
                continue;
            }
            if t.elapsed().as_secs() >= c.dwell_s {
                i = (i + 1) % centers.len();
                c.center_hz = centers[i];
                t = Instant::now();
            }
        }
    });
    let router = Router::new()
        .route("/api/status", get(status))
        .route("/api/config", post(config))
        .route("/api/recordings", get(recordings))
        .route("/api/storage/browse", post(browse_directory))
        .route("/api/shutdown", post(shutdown))
        .route("/recordings/{name}", get(recording_file))
        .route("/ws", get(websocket))
        .fallback_service(ServeDir::new("web"))
        .with_state(app.clone());
    let port: u16 = std::env::var("STATION_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(8096);
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .unwrap();
    println!("Balloon station: http://127.0.0.1:{port}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = app.shutdown.notified() => {},
            }
            app.running.store(false, Ordering::Relaxed);
        })
        .await
        .unwrap();
    if let Some(thread)=capture_thread {let _=thread.join();}
    if let Some(thread)=dsp_thread {let _=thread.join();}
}
