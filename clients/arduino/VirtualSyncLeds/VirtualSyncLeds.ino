// VIRTUAL Sync → LED strip, for ESP32 (or any Arduino-compatible board with
// Wi-Fi and the FastLED library).
//
// Copy clients/c/virtual_sync.h next to this sketch. The strip shows the
// program image's light zones from the VIRTUAL leader, pulses on every beat
// and dims to the bass when no zones arrive. Set Network sync → Leader in
// VIRTUAL with "Light zones" enabled, and use the same show key here.

#include <FastLED.h>
#include <WiFi.h>
#include <WiFiUdp.h>

#define VIRTUAL_SYNC_IMPLEMENTATION
#include "virtual_sync.h"

// --- Configure ---------------------------------------------------------------
const char *WIFI_SSID = "your-network";
const char *WIFI_PASSWORD = "your-password";
const char *SHOW_KEY = "dance floor";  // "" for an unsigned show
const char *DEVICE_NAME = "ESP32 strip";
constexpr int LED_PIN = 5;
constexpr int LED_COUNT = 120;
// ------------------------------------------------------------------------------

CRGB leds[LED_COUNT];
WiFiUDP udp;
uint8_t packet[VS_MAX_DATAGRAM];
vs_key key;
bool keyed = false;
vs_window window;
IPAddress leader;
uint16_t leaderPort = 0;
uint32_t helloSequence = 0;
unsigned long lastHello = 0;

vs_clock clockState;
bool haveClock = false;
unsigned long clockReceivedMicros = 0;
float bass = 0.0f;
unsigned long lastZones = 0;

void setup() {
  FastLED.addLeds<WS2812B, LED_PIN, GRB>(leds, LED_COUNT);
  FastLED.setBrightness(160);
  keyed = vs_derive_key(SHOW_KEY, &key);
  memset(&window, 0, sizeof window);
  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
  while (WiFi.status() != WL_CONNECTED) delay(100);
  udp.begin(VS_PORT);
}

// Beat phase 0–1 from the last clock, assuming network delay is small.
float beatPhase() {
  if (!haveClock || !clockState.playing) return 0.5f;
  uint64_t leaderNow = clockState.leader_us + (micros() - clockReceivedMicros);
  double beat = vs_beat_at(&clockState, leaderNow);
  return (float)(beat - floor(beat));
}

void handle(const vs_packet &p) {
  switch (p.kind) {
    case VS_ANNOUNCE:
      if (leaderPort == 0) {
        vs_announce a;
        if (vs_read_announce(&p, &a) == VS_OK) {
          leader = udp.remoteIP();
          leaderPort = a.port;
        }
      }
      break;
    case VS_CLOCK:
      if (vs_read_clock(&p, &clockState) == VS_OK) {
        haveClock = true;
        clockReceivedMicros = micros();
      }
      break;
    case VS_AUDIO: {
      vs_audio audio;
      if (vs_read_audio(&p, &audio) == VS_OK) bass = audio.bass;
      break;
    }
    case VS_ZONES: {
      int zones = vs_zones_count(&p);
      if (zones <= 0) break;
      for (int i = 0; i < LED_COUNT; i++) {
        const uint8_t *rgb = vs_zone_rgb(&p, i * zones / LED_COUNT);
        leds[i] = CRGB(rgb[0], rgb[1], rgb[2]);
      }
      lastZones = millis();
      break;
    }
  }
}

void loop() {
  int size;
  while ((size = udp.parsePacket()) > 0) {
    int len = udp.read(packet, sizeof packet);
    vs_packet p;
    if (vs_parse(packet, len, keyed ? &key : nullptr, &p) != VS_OK) continue;
    if (!vs_window_accept(&window, p.session, p.sequence)) continue;
    handle(p);
  }
  // Subscribe by unicast once a second (more reliable than broadcast).
  if (leaderPort && millis() - lastHello > 1000) {
    lastHello = millis();
    uint8_t hello[96];
    size_t n = vs_build_hello(hello, sizeof hello, DEVICE_NAME, VS_NODE_LIGHT,
                              VS_STREAM_CLOCK | VS_STREAM_AUDIO | VS_STREAM_ZONES,
                              (uint32_t)ESP.getEfuseMac(), ++helloSequence, keyed ? &key : nullptr);
    udp.beginPacket(leader, leaderPort);
    udp.write(hello, n);
    udp.endPacket();
  }
  if (millis() - lastZones > 500) {
    // No zones: a warm wash that breathes with the bass.
    fill_solid(leds, LED_COUNT, CHSV(20, 200, (uint8_t)(40 + 215 * bass)));
  }
  // Pulse on the beat: full brightness on the downbeat, decaying through it.
  float pulse = 1.0f - beatPhase();
  FastLED.setBrightness((uint8_t)(60 + 195 * pulse * pulse));
  FastLED.show();
}
