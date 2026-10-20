# Old-config compatibility fixtures

Each `vX.Y.Z/` directory holds the `applet-full.toml` that release of the Super
STT applet persisted (Super TTS's applet wrote the same files). The shared applet
kept the same TOML files, one per side, until it moved to a single
`settings.ron`. Until that file exists, the applet imports the TOML config
written last, so its config tests import every one of these to prove the
current code still reads configs written by older releases: it must load,
migrate, or reset, never crash.

Do not reformat existing files: they represent real on-disk user configs.
