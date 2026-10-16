# Third-party sources

The application is distributed under the GNU GPL v3 in [LICENSE](LICENSE).
The following bundled components retain their own notices and licenses.

| Component | Source | License / attribution |
| --- | --- | --- |
| Native LoRa demodulator and synthetic frame generator | [mmmaly/LoraReceiverStandalone](https://github.com/mmmaly/LoraReceiverStandalone/tree/beb7bad8554dc78133c90e83d594c2b49d00d1ea), based on [EPFL/TCL gr-lora_sdr](https://github.com/tapparelj/gr-lora_sdr) | GPL-3.0; local changes add structured packet metadata, legacy LDRO control, disable live CRC chase recovery, and provide a portable pipe receiver. |
| KissFFT | [Mark Borgerding / kissfft](https://github.com/mborgerding/kissfft) | BSD-3-Clause; see [vendor/lora/KISSFFT-LICENSE](vendor/lora/KISSFFT-LICENSE) and source headers. |
| rustplotlib snapshot | [jvierine/rustplotlib](https://github.com/jvierine/rustplotlib) | MIT; see [vendor/rustplotlib/LICENSE](vendor/rustplotlib/LICENSE) and [SNAPSHOT.json](vendor/rustplotlib/SNAPSHOT.json). |
| Leaflet 1.9.4 | [Leaflet](https://leafletjs.com/) | BSD-2-Clause; see [web/vendor/leaflet/LICENSE](web/vendor/leaflet/LICENSE). |
| Cropped coastline | [Natural Earth 1:10m land](https://github.com/nvkelso/natural-earth-vector/blob/master/geojson/ne_10m_land.geojson) | Public domain; see [web/map-data/NOTICE.txt](web/map-data/NOTICE.txt). |

Andøya transmitter reference code is available from the official resources linked
in the README. The local downloaded copy is excluded from this repository;
[reference/andoya/sources.json](reference/andoya/sources.json) preserves its source
URL and archive SHA-256. It is not required to build or run the receiver.

Rust dependencies are recorded in the Cargo lockfiles; their licenses remain
with their respective authors. Local flight recordings and generated plot caches
are excluded from the repository.
