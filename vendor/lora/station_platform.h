// Portability helpers for MSVC; POSIX builds retain the standard APIs.
#pragma once
#ifdef _WIN32
#include <chrono>
#include <cstring>
#include <cctype>
#include <io.h>
#include <fcntl.h>
static char *optarg = nullptr;
static int optind = 1;
static int getopt(int argc, char **argv, const char *spec) {
    static const char *next = nullptr;
    if (!next || !*next) {
        if (optind >= argc || argv[optind][0] != '-' || !argv[optind][1]) return -1;
        if (!strcmp(argv[optind], "--")) { ++optind; return -1; }
        next = argv[optind++] + 1;
    }
    char option = *next++;
    const char *found = strchr(spec, option);
    if (!found) return '?';
    if (found[1] == ':') {
        if (*next) { optarg = const_cast<char *>(next); next = nullptr; }
        else if (optind < argc) { optarg = argv[optind++]; next = nullptr; }
        else return '?';
    }
    return option;
}
#define strcasecmp _stricmp
static const char *strcasestr(const char *text, const char *pattern) {
    for (; *text; ++text) if (!_strnicmp(text, pattern, strlen(pattern))) return text;
    return nullptr;
}
static int station_popcount(unsigned int value) {
    int count = 0;
    while (value) { value &= value - 1; ++count; }
    return count;
}
#define __builtin_popcount station_popcount
struct timeval { long long tv_sec; long tv_usec; };
static int gettimeofday(timeval *tv, void *) {
    auto us = std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::system_clock::now().time_since_epoch()).count();
    tv->tv_sec = us / 1000000; tv->tv_usec = long(us % 1000000); return 0;
}
#else
#include <strings.h>
#include <sys/time.h>
#include <getopt.h>
#include <unistd.h>
#endif
