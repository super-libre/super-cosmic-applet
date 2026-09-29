# Old-config compatibility fixtures

Each `vX.Y.Z/` directory holds the `applet-full.toml` that release of the Super
STT applet persisted (Super TTS's applet wrote the same files). The shared applet
imports these files on its first run, so its config tests load every one of them
to prove the current code still reads configs written by older releases: it must
load, migrate, or reset, never crash.

Do not reformat existing files: they represent real on-disk user configs.
