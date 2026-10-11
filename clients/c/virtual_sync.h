/*
 * virtual_sync.h — single-header C99 client for VIRTUAL Sync, version 1.
 *
 * Receive a VIRTUAL show's tempo, beat phase, parameter values, audio bands
 * and light zones over UDP on an ESP32, Arduino (with Wi-Fi), Raspberry Pi or
 * any other device. No allocation, no dependencies beyond <stdint.h>,
 * <stddef.h> and <string.h>. Works on little- and big-endian CPUs.
 *
 *   #define VIRTUAL_SYNC_IMPLEMENTATION   // in exactly one .c/.cpp file
 *   #include "virtual_sync.h"
 *
 * Typical loop: receive a UDP datagram on port VS_PORT (41641), call
 * vs_parse() with your show key (or NULL for an unsigned show), drop it if
 * vs_window_accept() says it is a replay, then switch on packet.kind and use
 * the vs_read_* helpers. See docs/NETWORK_SYNC.md for the format.
 *
 * License: GPL-2.0-or-later, like VIRTUAL.
 */
#ifndef VIRTUAL_SYNC_H
#define VIRTUAL_SYNC_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define VS_PORT 41641
#define VS_VERSION 1
#define VS_HEADER_LEN 20
#define VS_TAG_LEN 8
#define VS_MAX_DATAGRAM 1400
#define VS_SPECTRUM_BANDS 8

/* Message kinds. */
enum {
    VS_ANNOUNCE = 1,
    VS_HELLO = 2,
    VS_CLOCK = 3,
    VS_CONTROLS = 4,
    VS_AUDIO = 5,
    VS_ZONES = 6,
    VS_PING = 7,
    VS_PONG = 8
};

/* Stream bits for hello/announce. */
enum { VS_STREAM_CLOCK = 1, VS_STREAM_CONTROLS = 2, VS_STREAM_AUDIO = 4, VS_STREAM_ZONES = 8 };

/* Node kinds for hello. */
enum { VS_NODE_FOLLOWER = 2, VS_NODE_LIGHT = 3, VS_NODE_CUSTOM = 4 };

/* vs_parse() results. */
enum {
    VS_OK = 0,
    VS_ERR_LENGTH = -1,
    VS_ERR_MAGIC = -2,
    VS_ERR_VERSION = -3,
    VS_ERR_SIGNATURE = -4,
    VS_ERR_PAYLOAD = -5
};

typedef struct {
    uint64_t k0, k1;
} vs_key;

typedef struct {
    uint8_t kind;
    uint32_t session;
    uint32_t sequence;
    const uint8_t *payload;
    uint16_t payload_len;
} vs_packet;

typedef struct {
    uint64_t leader_us; /* leader's clock when `beat` was true */
    double beat;        /* musical position, in beats */
    float bpm;
    uint8_t beats_per_bar;
    uint8_t playing;
} vs_clock;

typedef struct {
    float level, bass, mid, high, hit;
    float bands[VS_SPECTRUM_BANDS];
} vs_audio;

typedef struct {
    char name[64];
    uint8_t streams;
    uint8_t zones;
    uint16_t port; /* leader's port: send hellos here */
} vs_announce;

typedef struct {
    uint32_t session, highest;
    uint64_t seen;
    uint8_t valid;
} vs_window;

uint64_t vs_siphash24(vs_key key, const uint8_t *data, size_t len);
/* Show key from the passphrase typed into VIRTUAL (leading/trailing spaces
 * ignored). Returns 0 for an empty passphrase (unsigned show). */
int vs_derive_key(const char *passphrase, vs_key *out);
/* FNV-1a id of a parameter path, e.g. vs_control_id("deck.0.level"). */
uint32_t vs_control_id(const char *path);

/* Checks framing and, with a key, the signature. `key` NULL = unsigned. */
int vs_parse(const uint8_t *data, size_t len, const vs_key *key, vs_packet *out);
/* 1 if the datagram is new, 0 if it is a duplicate or too old. */
int vs_window_accept(vs_window *window, uint32_t session, uint32_t sequence);

int vs_read_announce(const vs_packet *packet, vs_announce *out);
int vs_read_clock(const vs_packet *packet, vs_clock *out);
int vs_read_audio(const vs_packet *packet, vs_audio *out);
/* Number of control values (or -1); `snapshot` set when non-NULL. */
int vs_controls_count(const vs_packet *packet, uint8_t *snapshot);
int vs_control_at(const vs_packet *packet, int index, uint32_t *id, float *value);
/* Number of zones (or -1); vs_zone_rgb returns a pointer to 3 bytes. */
int vs_zones_count(const vs_packet *packet);
const uint8_t *vs_zone_rgb(const vs_packet *packet, int index);

/* Beat at `leader_now_us`. Without a ping/pong offset, use
 * leader_now_us = clock.leader_us + (micros() - micros_when_received). */
double vs_beat_at(const vs_clock *clock, uint64_t leader_now_us);

/* Builds a hello so the leader also unicasts the chosen streams to this
 * device (send it about once a second to the announce's address and port).
 * Returns the datagram length, or 0 if `capacity` is too small. */
size_t vs_build_hello(uint8_t *out, size_t capacity, const char *name, uint8_t node_kind,
                      uint8_t streams, uint32_t session, uint32_t sequence, const vs_key *key);

#ifdef __cplusplus
}
#endif

#endif /* VIRTUAL_SYNC_H */

#ifdef VIRTUAL_SYNC_IMPLEMENTATION
#include <string.h>

static uint16_t vs__u16(const uint8_t *p) { return (uint16_t)(p[0] | (p[1] << 8)); }
static uint32_t vs__u32(const uint8_t *p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}
static uint64_t vs__u64(const uint8_t *p) {
    return (uint64_t)vs__u32(p) | ((uint64_t)vs__u32(p + 4) << 32);
}
static float vs__f32(const uint8_t *p) {
    uint32_t bits = vs__u32(p);
    float value;
    memcpy(&value, &bits, 4);
    return value;
}
static double vs__f64(const uint8_t *p) {
    uint64_t bits = vs__u64(p);
    double value;
    memcpy(&value, &bits, 8);
    return value;
}
static void vs__put32(uint8_t *p, uint32_t v) {
    p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); p[2] = (uint8_t)(v >> 16); p[3] = (uint8_t)(v >> 24);
}
static void vs__put64(uint8_t *p, uint64_t v) {
    vs__put32(p, (uint32_t)v);
    vs__put32(p + 4, (uint32_t)(v >> 32));
}

#define VS__ROTL(x, b) (uint64_t)(((x) << (b)) | ((x) >> (64 - (b))))
#define VS__ROUND                                                              \
    do {                                                                       \
        v0 += v1; v1 = VS__ROTL(v1, 13); v1 ^= v0; v0 = VS__ROTL(v0, 32);      \
        v2 += v3; v3 = VS__ROTL(v3, 16); v3 ^= v2;                             \
        v0 += v3; v3 = VS__ROTL(v3, 21); v3 ^= v0;                             \
        v2 += v1; v1 = VS__ROTL(v1, 17); v1 ^= v2; v2 = VS__ROTL(v2, 32);      \
    } while (0)

uint64_t vs_siphash24(vs_key key, const uint8_t *data, size_t len) {
    uint64_t v0 = 0x736f6d6570736575ULL ^ key.k0;
    uint64_t v1 = 0x646f72616e646f6dULL ^ key.k1;
    uint64_t v2 = 0x6c7967656e657261ULL ^ key.k0;
    uint64_t v3 = 0x7465646279746573ULL ^ key.k1;
    size_t full = len - (len % 8), i;
    uint8_t last[8] = {0};
    uint64_t m;
    for (i = 0; i < full; i += 8) {
        m = vs__u64(data + i);
        v3 ^= m; VS__ROUND; VS__ROUND; v0 ^= m;
    }
    memcpy(last, data + full, len - full);
    last[7] = (uint8_t)len;
    m = vs__u64(last);
    v3 ^= m; VS__ROUND; VS__ROUND; v0 ^= m;
    v2 ^= 0xff;
    VS__ROUND; VS__ROUND; VS__ROUND; VS__ROUND;
    return v0 ^ v1 ^ v2 ^ v3;
}

int vs_derive_key(const char *passphrase, vs_key *out) {
    static const vs_key salt_a = {0x5649525455414c31ULL, 0x73796e636b657961ULL};
    static const vs_key salt_b = {0x5649525455414c32ULL, 0x73796e636b657962ULL};
    const char *start = passphrase;
    const char *end = passphrase + strlen(passphrase);
    while (start < end && (*start == ' ' || *start == '\t' || *start == '\n' || *start == '\r')) start++;
    while (end > start && (end[-1] == ' ' || end[-1] == '\t' || end[-1] == '\n' || end[-1] == '\r')) end--;
    if (start == end) return 0;
    out->k0 = vs_siphash24(salt_a, (const uint8_t *)start, (size_t)(end - start));
    out->k1 = vs_siphash24(salt_b, (const uint8_t *)start, (size_t)(end - start));
    return 1;
}

uint32_t vs_control_id(const char *path) {
    uint32_t hash = 0x811c9dc5u;
    while (*path) {
        hash ^= (uint8_t)*path++;
        hash *= 0x01000193u;
    }
    return hash;
}

int vs_parse(const uint8_t *data, size_t len, const vs_key *key, vs_packet *out) {
    size_t body_end;
    int is_signed;
    if (len < VS_HEADER_LEN || len > VS_MAX_DATAGRAM) return VS_ERR_LENGTH;
    if (memcmp(data, "VSYN", 4) != 0) return VS_ERR_MAGIC;
    if (data[4] != VS_VERSION) return VS_ERR_VERSION;
    is_signed = (data[6] & 1) != 0;
    out->kind = data[5];
    out->session = vs__u32(data + 8);
    out->sequence = vs__u32(data + 12);
    out->payload_len = vs__u16(data + 16);
    out->payload = data + VS_HEADER_LEN;
    body_end = VS_HEADER_LEN + out->payload_len;
    if (len != body_end + (is_signed ? VS_TAG_LEN : 0)) return VS_ERR_LENGTH;
    if ((key != NULL) != is_signed) return VS_ERR_SIGNATURE;
    if (key && vs_siphash24(*key, data, body_end) != vs__u64(data + body_end)) return VS_ERR_SIGNATURE;
    return VS_OK;
}

int vs_window_accept(vs_window *w, uint32_t session, uint32_t sequence) {
    uint32_t age;
    if (!w->valid || w->session != session) {
        w->valid = 1; w->session = session; w->highest = sequence; w->seen = 1;
        return 1;
    }
    if (sequence > w->highest) {
        uint32_t shift = sequence - w->highest;
        w->seen = shift >= 64 ? 0 : w->seen << shift;
        w->seen |= 1;
        w->highest = sequence;
        return 1;
    }
    age = w->highest - sequence;
    if (age >= 64 || (w->seen & ((uint64_t)1 << age))) return 0;
    w->seen |= (uint64_t)1 << age;
    return 1;
}

int vs_read_announce(const vs_packet *p, vs_announce *out) {
    uint8_t n;
    if (p->kind != VS_ANNOUNCE || p->payload_len < 1) return VS_ERR_PAYLOAD;
    n = p->payload[0];
    if (n > 63 || p->payload_len != 1 + n + 5) return VS_ERR_PAYLOAD;
    memcpy(out->name, p->payload + 1, n);
    out->name[n] = 0;
    out->streams = p->payload[2 + n];
    out->zones = p->payload[3 + n];
    out->port = vs__u16(p->payload + 4 + n);
    return VS_OK;
}

int vs_read_clock(const vs_packet *p, vs_clock *out) {
    if (p->kind != VS_CLOCK || p->payload_len != 24) return VS_ERR_PAYLOAD;
    out->leader_us = vs__u64(p->payload);
    out->beat = vs__f64(p->payload + 8);
    out->bpm = vs__f32(p->payload + 16);
    out->beats_per_bar = p->payload[20];
    out->playing = p->payload[21];
    return VS_OK;
}

int vs_read_audio(const vs_packet *p, vs_audio *out) {
    int i;
    if (p->kind != VS_AUDIO || p->payload_len != 52) return VS_ERR_PAYLOAD;
    out->level = vs__f32(p->payload);
    out->bass = vs__f32(p->payload + 4);
    out->mid = vs__f32(p->payload + 8);
    out->high = vs__f32(p->payload + 12);
    out->hit = vs__f32(p->payload + 16);
    for (i = 0; i < VS_SPECTRUM_BANDS; i++) out->bands[i] = vs__f32(p->payload + 20 + 4 * i);
    return VS_OK;
}

int vs_controls_count(const vs_packet *p, uint8_t *snapshot) {
    uint16_t count;
    if (p->kind != VS_CONTROLS || p->payload_len < 4) return -1;
    count = vs__u16(p->payload + 2);
    if (p->payload_len != 4 + 8 * (size_t)count) return -1;
    if (snapshot) *snapshot = p->payload[0];
    return count;
}

int vs_control_at(const vs_packet *p, int index, uint32_t *id, float *value) {
    int count = vs_controls_count(p, NULL);
    if (index < 0 || index >= count) return VS_ERR_PAYLOAD;
    *id = vs__u32(p->payload + 4 + 8 * index);
    *value = vs__f32(p->payload + 8 + 8 * index);
    return VS_OK;
}

int vs_zones_count(const vs_packet *p) {
    if (p->kind != VS_ZONES || p->payload_len < 4) return -1;
    if (p->payload_len != 4 + 3 * (size_t)p->payload[0]) return -1;
    return p->payload[0];
}

const uint8_t *vs_zone_rgb(const vs_packet *p, int index) {
    int count = vs_zones_count(p);
    if (index < 0 || index >= count) return NULL;
    return p->payload + 4 + 3 * index;
}

double vs_beat_at(const vs_clock *c, uint64_t leader_now_us) {
    double elapsed = (double)((int64_t)(leader_now_us - c->leader_us)) / 60.0e6;
    return c->beat + elapsed * c->bpm;
}

size_t vs_build_hello(uint8_t *out, size_t capacity, const char *name, uint8_t node_kind,
                      uint8_t streams, uint32_t session, uint32_t sequence, const vs_key *key) {
    size_t n = strlen(name), body, total;
    if (n > 63) n = 63;
    body = 1 + n + 2;
    total = VS_HEADER_LEN + body + (key ? VS_TAG_LEN : 0);
    if (capacity < total) return 0;
    memcpy(out, "VSYN", 4);
    out[4] = VS_VERSION;
    out[5] = VS_HELLO;
    out[6] = key ? 1 : 0;
    out[7] = 0;
    vs__put32(out + 8, session);
    vs__put32(out + 12, sequence);
    out[16] = (uint8_t)body;
    out[17] = 0;
    out[18] = out[19] = 0;
    out[20] = (uint8_t)n;
    memcpy(out + 21, name, n);
    out[21 + n] = node_kind;
    out[22 + n] = streams;
    if (key) vs__put64(out + VS_HEADER_LEN + body, vs_siphash24(*key, out, VS_HEADER_LEN + body));
    return total;
}
#endif /* VIRTUAL_SYNC_IMPLEMENTATION */
