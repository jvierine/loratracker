# STRATOS balloon ground station

Local Rust + WebAssembly + WebGL2 receiver for the Andøya RFM96 CanSat.
RTL-SDR hardware access uses librtlsdr's C API directly from Rust. The native
C++ LoRa demodulator receives the same live I/Q stream over a pipe. No GNU Radio.

## Start the GUI

Extract a software ZIP into a writable folder (do not run inside the ZIP).
Install [Miniconda](https://docs.anaconda.com/miniconda/) and run `setup.sh`
(macOS/Linux) or `setup.bat` from an Anaconda/Miniconda prompt (Windows).
The setup scripts install the Python/HDF5 runtime into conda **base** by default.
Set `STRATOS_ENV` to use a different conda environment.

| System | Start |
| --- | --- |
| Windows | Double-click `start-gui.bat` |
| macOS | Double-click `start-gui.command` |
| Linux | Run `./start-gui.sh` |

The launcher opens **http://127.0.0.1:8096/** in the default browser. Keep its
terminal open; Ctrl+C stops the backend and plot helper. The default mode is
**recorded-data review** and does not open the SDR or record new I/Q.
To acquire live data, run the same launcher with `--live`.
`./run.sh` remains a compatibility entry point.

The **Data directory** control in the GUI accepts a path or opens a native
folder chooser with **Browse…**. Press **Use directory** to apply it. The
selection persists in local `settings.json`. Review reads `flight-*.h5` packet
logs from that directory; live operation writes telemetry and 4 MB raw-I/Q
segments there. Changing the directory rotates recording files without retuning
or restarting the decoder. A missing/unmounted directory produces an error;
there is no fallback to another disk. Existing files are never moved or deleted.
Computed plots and HDF5 analysis products live in `build/`, so reviewing a
read-only recording directory is supported.

You can also select the folder at launch:

```sh
./start-gui.sh --data-dir "/path/with spaces/flight"
./start-gui.sh --live --data-dir "/path/to/external-disk/flight"
```

On Windows use `start-gui.bat --data-dir "E:\CanSat\flight"`.
Fresh installations default to `Documents/STRATOS/data`; this checkout retains
its existing `recordings` directory when no other selection has been saved.

Rustplotlib figures require **LaTeX + dvisvgm** in PATH, including the `preview`,
`amsmath`, `amssymb`, `xcolor`, `newtxtext`, and `newtxmath` packages. Use
[TeX Live](https://www.tug.org/texlive/) (Linux/Windows),
[MacTeX](https://www.tug.org/mactex/) (macOS), or
[MiKTeX](https://miktex.org/) (Windows). Missing TeX affects plot rendering;
telemetry review and live recording do not depend on TeX.

Native ZIPs contain the station and plotter for the OS/CPU in their filename.
A **source ZIP** additionally needs [Rust](https://rustup.rs/) and a C/C++
compiler: Xcode Command Line Tools on macOS, GCC/Clang on Linux, or Visual Studio
Build Tools with **Desktop development with C++** on Windows. The launcher builds
missing native binaries automatically. To rebuild explicitly:

```sh
conda run --no-capture-output -n base python scripts/build.py
```

Browser WebAssembly assets are included in ZIPs. A fresh Git checkout must build
them using `scripts/build.py --wasm`; install `wasm-bindgen-cli` **0.2.129** with
`cargo install wasm-bindgen-cli --version 0.2.129 --locked` first.
Rustplotlib is bundled as a small MIT-licensed source snapshot in
`vendor/rustplotlib`; no developer-specific path or separate checkout is needed.

Live mode dynamically loads [librtlsdr](https://osmocom.org/projects/rtl-sdr/wiki/Rtl-sdr)
through its C API. Review requires no SDR driver. Install librtlsdr through
Homebrew (`brew install librtlsdr`), your Linux package manager (for example
`sudo apt install librtlsdr0`), or an appropriate Windows RTL-SDR distribution
and USB driver. On Windows put `rtlsdr.dll` and its matching dependency DLLs on
PATH. `RTLSDR_LIBRARY` can specify a full shared-library path on any OS. Linux
also needs the RTL-SDR USB permissions/udev rules. The native demodulator consumes
I/Q from the Rust process and does not open the USB device itself.

## Software packages and Git

```sh
conda run -n base python scripts/package.py             # source + browser assets
conda run -n base python scripts/package.py --binaries  # current OS/CPU binaries
conda run -n base python scripts/check_git_content.py
```

ZIPs are written to `dist/` with a SHA-256 file manifest. They contain software,
licenses and the small static coastline; **no flight recordings, raw voltage,
local settings, plot caches or storage symlinks**. Conda and TeX are separate
runtime prerequisites, not bundled installers. Git ignores data (`*.h5`, raw IQ),
`telemetry-local`, storage symlinks, generated files, native binaries and ZIPs.
The Git-content check rejects recording files and source files over **1 MiB**;
CI runs it before building Windows, Linux and macOS packages. These CI jobs must
complete on their respective OS before treating those platform builds as tested.
Do not force-add ignored data files. There are no Git commits containing flight
data in this initial repository.

`conda run -n base python scripts/test_portable.py` exercises directory switching,
exact stored-payload reading/downloads, settings persistence, review-only mode,
missing-drive rejection, graceful shutdown, and synthetic native LoRa decoding.
It uses temporary synthetic HDF5 files and needs no receiver hardware.

## Live operation

- Default center 433.750 MHz, 2 MS/s, both tuner and RTL digital AGC enabled.
- Simultaneously searches 433.500 and 434.000 MHz at BW 62.5/125/250/500 kHz,
  SF7–12. The exact coding rate and payload-CRC presence come from the header.
- Tune/lock or click the spectrum to listen to one carrier. Start scan searches
  the 430–436 MHz region in successive tunings and rotating decoder channels.
  The scanner is sequential; short transmissions can be missed between visits.
- Pause display freezes only the message pane. Reception and recording continue.
  Resume shows recent messages. Show rejected reveals invalid/suspect candidates.
- Raw I/Q and the complex-plane plots show actual ADC samples. Spectrum is
  relative dBFS, not a calibrated power/RSSI measurement. AGC changes scaling.

### Verified on-air result, 2026-10-15

434.000 MHz: SF7, BW 62.5 kHz, CR 4/5, explicit header, sync word 0x12 observed
in decoder diagnostics, payload CRC absent. Repeated packets contain CanSat ID
14000 (little-endian b0 36), followed by UTF-8 semicolon-separated text beginning
`navn på CanSat; ...`. 433.500 MHz was subsequently identified as SF12, BW 62.5 kHz, CR 4/8, with CanSat ID 13500. Its legacy Andøya transmitter disables LDRO; enabling LDRO in the receiver corrupts the payload after the header. The native receiver now defaults to LDRO off (`-l 0`), matching that source; use `-l -1` for standards-based automatic LDRO or `-l 1` to force it on.
A CRC tick from a noisy hypothesis does not establish a valid CanSat packet.
Aggressive CRC chase recovery is disabled in live decoding.

Independent recorded-I/Q check (`scripts/verify_air.py`): preamble repetition
512 samples at 250 kS/s = 2.048 ms; quadratic phase fit gives 30.514 MHz/s chirp
slope and 62.493 kHz sweep. BW × period = 128, hence SF7. This validates the
434 MHz modem settings, not the integrity of every no-CRC payload.
The analysis artifact is `recordings/air-verification.h5`.

## Recording format

In live mode, telemetry recording follows the recording control. Files are in the
selected data directory as `flight-*.h5`. Review mode never records.
Recordings are local; nothing is uploaded. Stop recording before downloading
or reading a currently active file. Ctrl-C shuts the server down and closes HDF5.

Each decoded candidate is saved BEFORE display filtering, including bad CRCs:

- `payload_bytes`: variable-length uint8 array per packet, exact recovered bytes,
  including the two CanSat ID bytes. No UTF-8 conversion or filtering here.
- `packet_utc_unix`: host decoder receipt time, Unix UTC seconds.
- `packets_json`: corresponding PHY parameters, exact payload_hex, decoded text,
  CRC presence/status, acceptance flag, and optional parsed fields.
- `iq_u8`: optional raw RTL-SDR uint8 I,Q pairs, shape (complex samples, 2).
- `iq_blocks`: callback UTC time, first complex-sample index, sample count,
  tuner center Hz, AGC flag, requested manual gain dB, PPM correction.
- File attributes describe sample rate, encoding, initial configuration and schema.

Raw I/Q recording costs about 240 MB/min. In live mode its control is independent of telemetry recording and independent of telemetry logging. It rotates into `iq-SESSION-SEQUENCE.h5` files with exactly 4,000,000 IQ bytes each (2,000,000 complex samples, one second at 2 MS/s), plus HDF5 metadata. A stopped recording or hardware-setting change can finish a shorter segment. Each segment stores start UTC, exact sample indices, rate, tuning, AGC/manual gain, complete/partial state, known dropped-sample gaps, and generation changes with unknown USB discontinuities. Files are never overwritten or automatically deleted. Read completed segments while recording continues. The first 16 files and a recent six-file sequence were verified to have exactly continuous indices and 4 MB payloads. It preserves the
waveform for changing the demodulator later; payload_bytes preserves the current
demodulator's byte decisions. Neither is equivalent to the transmitter's original
bytes when reception is corrupted. No-CRC messages remain unverified even if their
structure looks right. The UI requires a matching ID and numeric payload structure;
this is a heuristic for the observed semicolon format, not a universal CanSat schema.
The 27-byte binary layout is partially decoded: ID uint16 at byte 0, time/counter uint16 at 2, latitude float32 little-endian decimal degrees at 4 and longitude float32 little-endian decimal degrees at 8, altitude raw int16 at 12, raw byte at 14, uint16/16 at 15, bytes at 17 and 18, pressure float32 Pa at 19, uint16 at 23 and final bytes 25–26. The pressure and several raw fields agree with the photographed reference receiver output. Coordinate types/offsets are inferred from repeated over-air packets, including 69.295914 N, 16.030746 E. GPS fix status and the meaning/calibration of remaining fields are not verified. The GUI distinguishes zero/invalid coordinates, displays position packet age and repeated coordinates, and holds isolated implausible jumps out of the summary until repeated. These checks do not validate checksum-free packets. Exact bytes are always retained.

Example offline access:

```python
import h5py, json
with h5py.File('recordings/flight-REPLACE.h5', 'r') as f:
    for raw, meta in zip(f['payload_bytes'], f['packets_json'].asstr()):
        packet = bytes(raw)
        settings = json.loads(meta)
        # Your own decoding starts here; packet[0:2] is the Andøya ID.
```

Earlier recordings containing only payload_hex can be augmented without changing
the received bytes using `conda run -n base python scripts/upgrade_recording.py FILE`.

## Andøya source

The official library download is documented in `reference/andoya/sources.json`.
A local downloaded copy may be present in `reference/andoya/Cansat_RFM96/`; it is
excluded from Git and the software packages.
`sources.json` records the download URL and SHA-256. The library confirms BW 62.5
kHz, SF7, CR 4/5 defaults. Its constructor encodes frequency_kHz − 420000 in the
first two little-endian payload bytes. Its `setModem` leaves payload CRC disabled.
The available pressure example is comma-separated millis, counter, pressure;
it is not the exact semicolon-based transmitter program heard on 434 MHz.
The site's separate first-sketch download currently returns 404, so the semantic
meaning of the additional observed payload fields is not claimed as verified.

Official resources:
- https://learn.andoyaspace.no/docs/cansat/teensy/telemetry_system/
- https://learn.andoyaspace.no/docs/cansat/teensy/assets/Cansat_RFM96.zip

## Validation

`./scripts/test_decoder.sh` checks byte-exact native decode and CRC across several
SF/BW/CR settings, including the measured ~−3.4 kHz receiver/transmitter offset.
These synthetic checks are separate from the recorded on-air verification.

## Attribution

`vendor/lora/` adapts mmmaly/LoraReceiverStandalone at commit
beb7bad8554dc78133c90e83d594c2b49d00d1ea, based on EPFL/TCL gr-lora_sdr algorithms
(GPL-3.0). KissFFT retains its BSD-3-Clause notice. Local changes add structured
packet metadata and disable CRC chase in live mode. The upstream source is at
https://github.com/mmmaly/LoraReceiverStandalone . Reference Andøya code retains
its authorship and is not relicensed by this project.

## Continuous raw replay

Use completed files from one session and tuning (shell globs may be quoted):

```sh
conda run -n base --no-capture-output python scripts/replay_iq.py 'recordings/iq-SESSION-*.h5' --channels 433500000,434000000 --bandwidth 62500 --sf 7,12 --ldro 0
```

Replay checks sample-index continuity and refuses tuning/session changes or gaps
unless explicitly permitted. Exclude the currently open segment. Decode one saved
27-byte message using `conda run -n base python scripts/decode_payload.py HEX`.

## Launch operation

Live receiver is left running without restarts. Tuner and digital AGC can be toggled;
manual gain is available when AGC is off. The compact GUI shows all instruments in
one viewport. I/Q recording is independent of GUI pause, reload and decoder restarts.
The receiver's 2 MHz window covers the two configured frequencies simultaneously;
this is not exhaustive simultaneous detection of arbitrary unknown carrier centers.
A caffeinate process inhibits idle system sleep for the running server.

Plots use the bundled `vendor/rustplotlib` crate through
`tools/flight-plots`. Build with `cargo build --release --manifest-path
tools/flight-plots/Cargo.toml`. `run.sh` starts the independent history/plot helper;
it can also be started with `conda run --no-capture-output -n base python
scripts/flight_history.py`. It reads closed HDF5 packet logs plus an immutable
snapshot of the active log and continuously merges the live API packet buffer.
No I/Q arrays are loaded, capture settings are never modified, and both GUI views
keep their WebSocket and plot history live regardless of tab visibility.

Map, pressure/MSIS height, pressure Reynolds decomposition, and structure-function
figures are rendered by rustplotlib as SVG. The `Plots` view scrolls independently.
Drawing a rectangle inside the axes changes numerical axis limits through a loopback-only
plot-control helper on port 8097; no CSS magnification is used. Every figure has a Reset zoom control. Wheel gestures only scroll the panel. Mint is 433.500 MHz; violet is 434.000 MHz. Orange map points fail the
track plausibility screen, but remain available with All coordinates. The default
map bootstraps from consistent consecutive positions and excludes near-zero float
corruption; this is a plausibility screen, not payload validation.

The pressure analysis denotes pressure in hPa by rho, following the requested
notation; it does not infer mass density. A nine-neighbour Hampel screen, after removing a local Theil-Sen linear trend, removes
obvious outliers using max(0.5 hPa, 6 scaled MAD), in addition to the 0.1–1100 hPa
range. Rho prime subtracts a centered 300-second boxcar mean, using 1-second
interpolation only inside continuous segments with no gaps over 20 seconds and
only full-window support. The structure functions average squared increments
in actual-time logarithmic lag bins from 2–300 s with at least 10 pairs per bin;
solid curves use cleaned rho and dashed curves use rho prime. Pairs never cross
segment boundaries. This does not establish a turbulence model or compensate
for changing balloon position/ascent. HDF5 products in the application `build/` cache preserve the
MSIS pressure/height profile and the derived samples, pair counts, outlier masks,
and method description. `web/map-data` JSON files are disposable GUI caches;
original packet bytes and raw I/Q remain in their existing HDF5 recordings.

The right height axis interpolates p(z)=k_B T(z) sum n_thermal(z) from NRLMSIS 2.1
at 69.31 N, 16.04 E on 2026-10-15 at 14:00 UTC; explicit assumed drivers are
F10.7=F10.7a=150 and Ap=4. Anomalous oxygen is excluded from thermal pressure.
This climatological pressure height is an estimate, not GPS altitude.
