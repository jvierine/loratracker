#!/bin/zsh
set -eu
cd "${0:A:h:h}"
task_tmp=$(mktemp -d)
trap 'rm -rf "$task_tmp"' EXIT
for settings in '7 62500 1 0' '7 62500 1 -3400' '9 125000 2 1700' '12 62500 1 0'; do
  read -r sf bw cr cfo <<< "$settings"
  ./build/lora_tx_gen -o "$task_tmp/test.u8" -S "$sf" -b "$bw" -s 2000000 -c "$cr" -O "$cfo" -m 'CANSAT LOOPBACK' 2>/dev/null
  ./build/lora_rx -r "$task_tmp/test.u8" -f 434000000 -s 2000000 -S "$sf" -b "$bw" -l -1 -w 0 > "$task_tmp/decoded" 2>/dev/null
  rg -q '"crc_valid":true,"payload_hex":"43414e534154204c4f4f504241434b"' "$task_tmp/decoded"
  echo "PASS: SF$sf, BW $bw Hz, CR 4/$((cr+4)), CFO $cfo Hz"
done
