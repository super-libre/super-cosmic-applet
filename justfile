applet := 'super-cosmic-applet'
bin_dir := '/usr/local/bin'
share_dir := '/usr/local/share'

# Clippy, with the same gate as CI and as Super STT and Super TTS: library and
# binary, not the tests
check *args:
    cargo clippy {{ args }} -- -W clippy::pedantic -D warnings -D unused_must_use

# Check formatting without modifying files
fmt-check:
    cargo fmt --all -- --check

# Format the code
fmt:
    cargo fmt --all

test *args:
    cargo test {{ args }}

build-release *args:
    cargo build --release {{ args }}

# Fail if the applet binary carries the wgpu renderer. A GPU renderer in a
# panel applet is ~85 MB of RSS per instance and no visible symptom, and
# nothing in the manifest can refuse the feature that brings it: only the
# artifact tells.
check-renderer: build-release
    #!/usr/bin/env bash
    set -euo pipefail
    if grep -aq iced_wgpu target/release/{{ applet }}; then
        echo "target/release/{{ applet }} links iced_wgpu. The applet must render with tiny-skia." >&2
        exit 1
    fi
    echo "target/release/{{ applet }} renders with tiny-skia, without wgpu."

# Everything CI runs
ci: fmt-check check test check-renderer

# Install the applet, its launcher entries and its icon under /usr/local, as
# the Super STT and Super TTS installers do. Escalates with sudo for the copy.
install: build-release
    sudo install -Dm0755 target/release/{{ applet }} {{ bin_dir }}/{{ applet }}
    for side in full left right; do \
      sudo install -Dm0644 resources/{{ applet }}-$side.desktop {{ share_dir }}/applications/{{ applet }}-$side.desktop; \
    done
    sudo install -Dm0644 resources/icons/hicolor/scalable/apps/{{ applet }}.svg {{ share_dir }}/icons/hicolor/scalable/apps/{{ applet }}.svg

# Remove what `install` put in place
uninstall:
    sudo rm -f {{ bin_dir }}/{{ applet }}
    sudo rm -f {{ share_dir }}/applications/{{ applet }}-full.desktop {{ share_dir }}/applications/{{ applet }}-left.desktop {{ share_dir }}/applications/{{ applet }}-right.desktop
    sudo rm -f {{ share_dir }}/icons/hicolor/scalable/apps/{{ applet }}.svg

# Run one side against the live panel's daemons, for development
run side="full":
    cargo run -- --side {{ side }}
