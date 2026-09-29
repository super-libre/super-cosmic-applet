# Super Applet

<img src="resources/icons/hicolor/scalable/apps/super-cosmic-applet.svg" width="96" height="96" alt="Super Applet">

A COSMIC panel applet for [Super STT](https://github.com/jorge-menjivar/super-stt)
and [Super TTS](https://github.com/super-libre/super-tts). It shows what either
one is doing:

1. While Super STT records, it draws your microphone. While it transcribes, it
   shows a working animation.
2. While Super TTS speaks, it draws what is playing. While it prepares the first
   audio, it shows the same working animation.
3. When both are active at once, it switches between them every few seconds.
4. The rest of the time it shows its icon. Click it for settings and each
   product's status.

The applet talks to each daemon over its Unix socket, with its own session, and
works with either product installed or both.

## Install

Both products' installers install the applet. To try a local build:

```sh
just install     # build, then copy into /usr/local (asks for sudo)
just uninstall
```

Then add "Super Applet" to a panel in COSMIC Settings. Pick Full, or the Left
and Right halves for either side of the panel's center.

## Develop

Run every check through `just`:

```sh
just ci          # format, clippy, tests
just check       # clippy only
just test
just run         # run the full applet against the running daemons
```

The applet takes each product's names from that product's own crate, and
super-engine's client for the rest. `Cargo.toml` explains why all three must
name the same super-engine revision.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
