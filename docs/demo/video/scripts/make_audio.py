"""Synthesize the demo's music bed and sound effects.

Everything is generated from code with a fixed random seed, so the audio is
reproducible and carries no third-party licence. The music is an upbeat pop
rock bed at 130 BPM: double-tracked distorted power chords panned left and
right, a rock kit, a driven bass, and a lead hook. It follows the film:
palm-muted verse while the setup is typed, a ringing break during the
Actions run, an open chorus over the report, and a final hit on the closing
card. The cue times below mirror the click and typing beats authored in
compositions/*.html (root seconds).

Usage: python3 scripts/make_audio.py  (writes assets/audio/*.wav)
"""

import wave
from pathlib import Path

import numpy as np

SR = 48000
LENGTH = 64.0
BPM = 130.0
BEAT = 60.0 / BPM
BAR = BEAT * 4
OUT = Path(__file__).resolve().parent.parent / "assets" / "audio"
rng = np.random.default_rng(20261013)

# Arrangement, in root seconds.
VERSE = 4.3  # first scene: the band comes in on a downbeat here
BREAK = (30.0, 39.0)  # Actions run
END = 58.0  # closing card
GRID = VERSE - 2 * BAR  # bar 0 starts here so a bar lands exactly on VERSE


def n(seconds):
    return int(round(seconds * SR))


def buf():
    return np.zeros(n(LENGTH) + 4 * SR, dtype=np.float64)


def hz(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


def add(track, start, sig):
    i = n(start)
    if i < 0:
        sig = sig[-i:]
        i = 0
    if i >= len(track):
        return
    sig = sig[: len(track) - i]
    track[i : i + len(sig)] += sig


def env(length, attack, decay):
    t = np.arange(n(length)) / SR
    a = np.clip(t / max(attack, 1e-4), 0, 1)
    return a * np.exp(-t / decay)


def tone_filter(x, low=None, high=None, peak=None):
    """First-order high/low-pass and an optional bell, in the frequency domain."""
    spec = np.fft.rfft(x)
    f = np.fft.rfftfreq(len(x), 1 / SR) + 1e-9
    gain = np.ones_like(f)
    if high:
        gain /= np.sqrt(1 + (f / high) ** 2)
    if low:
        gain *= (f / low) / np.sqrt(1 + (f / low) ** 2)
    if peak:
        centre, boost, width = peak
        gain *= 1 + boost * np.exp(-((np.log2(f / centre)) ** 2) / (2 * width**2))
    return np.fft.irfft(spec * gain, len(x))


def reverb(x, seconds=1.4, mix=0.14, seed=7):
    m = n(seconds)
    t = np.arange(m) / SR
    ir = np.random.default_rng(seed).standard_normal(m) * np.exp(-t / (seconds / 6.5))
    ir = tone_filter(ir, high=4000)
    ir[: n(0.015)] = 0
    ir /= np.sqrt(np.sum(ir**2))
    size = 1 << int(np.ceil(np.log2(len(x) + m)))
    wet = np.fft.irfft(np.fft.rfft(x, size) * np.fft.rfft(ir, size), size)[: len(x)]
    return (1 - mix) * x + mix * wet * 2.0


def noise_burst(length, decay, low=None, high=None):
    x = rng.standard_normal(n(length)) * env(length, 0.0005, decay)
    # Two passes make the filters second-order, so noise sits where it should.
    return tone_filter(tone_filter(x, low=low, high=high), low=low, high=high)


# ---------- instruments ----------


def saw(f, t, partials=40, detune=0.0):
    out = np.zeros_like(t)
    for k in range(1, partials + 1):
        if k * f > 9000:
            break
        out += np.sin(2 * np.pi * k * f * (1 + detune) * t + 0.37 * k) / k
    return out


def power_chord(root, length, muted, detune, seed):
    """Root, fifth, and octave through a fuzz and a guitar-cabinet curve."""
    t = np.arange(n(length)) / SR
    voices = ((root, 1.0), (root + 7, 0.8), (root + 12, 0.6))
    x = sum(saw(hz(m), t, detune=detune * (j + 1)) * g for j, (m, g) in enumerate(voices))
    x *= np.clip(t / 0.004, 0, 1)
    x = np.tanh(6.0 * x) * 0.6
    if muted:
        x *= np.exp(-t / 0.09)
        x = tone_filter(x, low=90, high=1800, peak=(900, 0.8, 0.8))
    else:
        x *= 0.75 + 0.25 * np.exp(-t / 0.5)
        x = tone_filter(x, low=90, high=3800, peak=(1500, 0.9, 0.9))
    pick = np.random.default_rng(seed).standard_normal(len(t)) * np.exp(-t / 0.004) * 0.15
    pick = tone_filter(pick, low=800, high=3000)
    return (x + pick) * np.clip((length - t) / 0.02, 0, 1)


def bass_note(midi, length):
    f = hz(midi)
    t = np.arange(n(length)) / SR
    x = saw(f, t, partials=12)
    x = np.tanh(2.2 * x) * np.clip(t / 0.003, 0, 1) * (0.6 + 0.4 * np.exp(-t / 0.15))
    x = tone_filter(x, low=35, high=900)
    return x * np.clip((length - t) / 0.02, 0, 1)


def lead_note(midi, length, bend=0.0):
    """Overdriven lead with vibrato and an optional bend into the note."""
    t = np.arange(n(length)) / SR
    f = hz(midi) * (1 + 0.006 * np.sin(2 * np.pi * 5.5 * t) * np.clip((t - 0.15) / 0.2, 0, 1))
    if bend:
        f *= 2 ** (bend * np.exp(-t / 0.06) / 12)
    phase = 2 * np.pi * np.cumsum(f) / SR
    x = sum(np.sin(k * phase) / k for k in range(1, 12))
    x = np.tanh(4.0 * x) * np.clip(t / 0.006, 0, 1) * (0.7 + 0.3 * np.exp(-t / 0.3))
    x = tone_filter(x, low=200, high=4200, peak=(1800, 1.0, 0.7))
    return x * np.clip((length - t) / 0.05, 0, 1)


def kick():
    t = np.arange(n(0.35)) / SR
    f = 48 + 110 * np.exp(-t / 0.025)
    body = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.16)
    beater = noise_burst(0.35, 0.004, low=1500, high=4000) * 0.15
    return body + beater


def snare():
    t = np.arange(n(0.35)) / SR
    body = (np.sin(2 * np.pi * 185 * t) + 0.5 * np.sin(2 * np.pi * 330 * t)) * np.exp(-t / 0.05)
    wires = noise_burst(0.35, 0.08, low=1200, high=5000)
    return 1.0 * body + 0.6 * wires


def hat(open_=False):
    return noise_burst(0.5 if open_ else 0.08, 0.12 if open_ else 0.02, low=6000, high=11000) * 0.6


def crash():
    return noise_burst(3.0, 0.6, low=3500, high=9000) * 0.4


def tom(midi):
    t = np.arange(n(0.4)) / SR
    f = hz(midi) * (1 + 0.4 * np.exp(-t / 0.04))
    return np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.18)


# vi - IV - I - V in G (E5, C5, G5, D5), guitar register.
ROOTS = [40, 36, 43, 38]
# Chorus hook over four bars: (bar in cycle, eighth index, midi, length in eighths, bend).
HOOK = [
    (0, 0, 71, 2, 0), (0, 2, 74, 2, 0), (0, 4, 76, 3, -2), (0, 7, 74, 1, 0),
    (1, 0, 72, 3, 0), (1, 4, 71, 2, 0), (1, 6, 67, 2, 0),
    (2, 0, 74, 2, 0), (2, 2, 76, 2, 0), (2, 4, 79, 3, -2), (2, 7, 78, 1, 0),
    (3, 0, 76, 4, -1), (3, 4, 74, 2, 0), (3, 6, 71, 2, 0),
]


def music():
    gl = buf()
    gr = buf()
    bass = buf()
    lead = buf()
    drums = buf()
    KICK, SNARE, HAT, OHAT, CRASH = kick(), snare(), hat(), hat(True), crash()
    cache = {}

    def chord(root, length, muted, side):
        key = (root, round(length, 3), muted, side)
        if key not in cache:
            cache[key] = power_chord(root, length, muted, 0.0015 if side else -0.0012, seed=len(cache))
        return cache[key]

    def strum(at, root, length, muted, vel=1.0):
        # Double-tracked: slightly different takes, timing, and pitch per side.
        add(gl, at, vel * chord(root, length, muted, 0))
        add(gr, at + 0.007, vel * chord(root, length, muted, 1))

    # Intro: muted chugs swelling in, then a tom and snare fill into the verse.
    for i in range(12):
        at = GRID + i * BEAT / 2
        if at < 0.2:
            continue
        strum(at, ROOTS[0], BEAT / 2, True, 0.25 + 0.05 * i)
    fill = VERSE - BEAT
    for k, m in enumerate((50, 47, 45, 43)):
        add(drums, fill + k * BEAT / 4, 0.8 * tom(m))
    add(drums, fill + BEAT / 2, 0.7 * SNARE)
    add(drums, fill + 3 * BEAT / 4, 0.8 * SNARE)

    bar = 2
    while True:
        t0 = GRID + bar * BAR
        if t0 >= END - 0.01:
            break
        root = ROOTS[bar % 4]
        if BREAK[0] <= t0 < BREAK[1]:
            section = "break"
        elif t0 >= BREAK[1]:
            section = "chorus"
        else:
            section = "verse"
        if (bar - 2) % 8 == 0 or (section == "chorus" and (bar - 2) % 4 == 0):
            add(drums, t0, 0.8 * CRASH)
        if section == "verse":
            for e in range(8):
                strum(t0 + e * BEAT / 2, root, BEAT / 2, True, 0.95 if e % 2 == 0 else 0.75)
        elif section == "break":
            strum(t0, root, BAR, False, 0.75)
        else:
            for e, length in ((0, 1.5), (3, 1.0), (4, 1.5), (6, 1.0), (7, 0.5)):
                strum(t0 + e * BEAT / 2, root, length * BEAT, False, 0.9)
        # Bass: driving eighths, octave pops in the chorus.
        for e in range(8):
            if section == "break" and e % 2:
                continue
            step = 12 if section == "chorus" and e in (3, 7) else 0
            vel = 0.9 if e % 2 == 0 else 0.7
            add(bass, t0 + e * BEAT / 2, vel * bass_note(root - 12 + step, BEAT / 2 * 0.9))
        # Drums.
        for beat in range(4):
            at = t0 + beat * BEAT
            if section == "break":
                add(drums, at, 0.75 * KICK)
                add(drums, at, 0.35 * HAT)
                continue
            if beat in (0, 2):
                add(drums, at, KICK)
            if beat == 2 or (section == "chorus" and beat == 0):
                add(drums, at + BEAT / 2, 0.7 * KICK)
            if beat in (1, 3):
                add(drums, at, 0.9 * SNARE)
            for e in range(2):
                open_ = section == "chorus" and e == 1 and beat == 3
                add(drums, at + e * BEAT / 2, (0.45 if e == 0 else 0.3) * (OHAT if open_ else HAT))
        # Snare build into the chorus.
        if section == "break" and t0 + BAR > BREAK[1] - 0.01:
            for s in range(8):
                add(drums, t0 + BAR / 2 + s * BEAT / 4, (0.3 + 0.08 * s) * SNARE)
        if section == "chorus":
            for cb, eighth, m, length, bend in HOOK:
                if cb == (bar - 2) % 4:
                    add(lead, t0 + eighth * BEAT / 2, lead_note(m, length * BEAT / 2 * 0.95, bend))
        bar += 1

    # Closing hit: everything on one ringing E5, then let it decay.
    add(drums, END, 1.1 * KICK)
    add(drums, END, CRASH)
    add(gl, END, chord(ROOTS[0], 4.5, False, 0))
    add(gr, END + 0.007, chord(ROOTS[0], 4.5, False, 1))
    add(bass, END, bass_note(ROOTS[0] - 12, 3.0))

    drums = reverb(drums, 0.9, 0.12, seed=3)
    lead = reverb(lead, 1.6, 0.25, seed=5)
    left = 0.40 * drums + 0.44 * gl + 0.14 * gr + 0.36 * bass + 0.24 * lead
    right = 0.40 * drums + 0.14 * gl + 0.44 * gr + 0.36 * bass + 0.24 * lead
    tt = np.arange(len(left)) / SR
    fade = np.clip(tt / 0.3, 0, 1) * np.clip((LENGTH - tt) / 2.5, 0, 1)
    return np.stack([left * fade, right * fade], axis=1)


# ---------- sound effects ----------


def click():
    t = np.arange(n(0.05)) / SR
    blip = np.sin(2 * np.pi * 2300 * t) * np.exp(-t / 0.006)
    return 0.55 * blip + 0.6 * noise_burst(0.05, 0.004, low=1500)


def key_tick():
    t = np.arange(n(0.03)) / SR
    tick = np.sin(2 * np.pi * 3100 * t) * np.exp(-t / 0.004)
    return 0.25 * tick + 0.3 * noise_burst(0.03, 0.003, low=2500)


def mallet(midi, length=1.4, vel=1.0):
    f = hz(midi)
    t = np.arange(n(length)) / SR
    body = np.sin(2 * np.pi * f * t) + 0.22 * np.sin(2 * np.pi * 4.0 * f * t) * np.exp(-t / 0.05)
    return vel * body * env(length, 0.003, 0.32)


def chime(notes, gap=0.11):
    out = np.zeros(n(1.6 + gap * len(notes)))
    for i, m in enumerate(notes):
        tone = mallet(m, vel=0.9)
        out[n(i * gap) : n(i * gap) + len(tone)] += tone
    return out


# Root-timeline cues (scene start + local time in each composition).
CLICKS = [5.65, 7.15, 14.5, 15.95, 17.35, 22.55, 23.95, 28.75, 31.0, 56.55, 57.05]
TYPING = [(5.75, 6.55), (7.25, 12.0), (16.05, 16.85), (17.45, 20.9), (24.1, 25.25)]
FAIL_AT = 33.6
PASS_AT = 55.6
MERGED_AT = 57.2


def sfx():
    track = buf()
    for at in CLICKS:
        add(track, at, click())
    for start, end in TYPING:
        at = start
        while at < end:
            add(track, at, (0.6 + 0.4 * rng.random()) * key_tick())
            at += 0.055 + 0.03 * rng.random()
    add(track, FAIL_AT, 0.5 * chime([64, 60], gap=0.14))
    add(track, PASS_AT, 0.45 * chime([79, 84]))
    add(track, MERGED_AT, 0.45 * chime([76, 79, 84, 88], gap=0.09))
    return np.stack([track, track], axis=1)


def write(path, stereo):
    peak = np.max(np.abs(stereo)) or 1.0
    data = np.tanh(1.2 * stereo / peak) * 0.89
    data = data[: n(LENGTH)]
    pcm = (data * 32767).astype("<i2")
    with wave.open(str(path), "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    write(OUT / "music.wav", music())
    write(OUT / "sfx.wav", sfx())
    print("wrote", OUT / "music.wav", OUT / "sfx.wav")


if __name__ == "__main__":
    main()
