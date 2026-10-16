// The packaged decoder consumes stdin/files. Rust owns the live librtlsdr C API.
// Explicit stubs keep the standalone upstream hardware paths out of the build.
#pragma once
#include <cstdint>
struct rtlsdr_dev_t {};
inline uint32_t rtlsdr_get_device_count() { return 0; }
inline const char *rtlsdr_get_device_name(uint32_t) { return "Pipe-only station decoder"; }
inline int rtlsdr_get_device_usb_strings(uint32_t, char *, char *, char *) { return -1; }
inline int rtlsdr_open(rtlsdr_dev_t **, uint32_t) { return -1; }
inline int rtlsdr_close(rtlsdr_dev_t *) { return -1; }
inline int rtlsdr_cancel_async(rtlsdr_dev_t *) { return -1; }
inline int rtlsdr_set_sample_rate(rtlsdr_dev_t *, uint32_t) { return -1; }
inline int rtlsdr_set_center_freq(rtlsdr_dev_t *, uint32_t) { return -1; }
inline int rtlsdr_set_freq_correction(rtlsdr_dev_t *, int) { return -1; }
inline int rtlsdr_set_tuner_gain_mode(rtlsdr_dev_t *, int) { return -1; }
inline int rtlsdr_set_tuner_gain(rtlsdr_dev_t *, int) { return -1; }
inline int rtlsdr_get_tuner_gain(rtlsdr_dev_t *) { return 0; }
inline int rtlsdr_set_agc_mode(rtlsdr_dev_t *, int) { return -1; }
inline int rtlsdr_reset_buffer(rtlsdr_dev_t *) { return -1; }
inline int rtlsdr_read_async(rtlsdr_dev_t *, void (*)(unsigned char *, uint32_t, void *), void *, uint32_t, uint32_t) { return -1; }
