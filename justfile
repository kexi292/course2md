# Install this checkout as /Applications/course2md.app and open it.
install:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "$(uname -s)" != Darwin ]]; then
        echo "just install is macOS-only." >&2
        exit 1
    fi
    if [[ ! -d desktop/.deps/zed || ! -d desktop/.deps/component ]]; then
        echo "Run python3 desktop/scripts/sources.py first." >&2
        exit 1
    fi
    cargo build --release
    cargo build --release --manifest-path desktop/Cargo.toml

    app=desktop/target/install/course2md.app
    bin="$app/Contents/MacOS"
    res="$app/Contents/Resources"
    rm -rf "$app"
    mkdir -p "$bin" "$res"
    python3 - "$app" <<'PY'
    from pathlib import Path
    import plistlib, re, tomllib, sys
    app = Path(sys.argv[1])
    version = tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"]
    match = re.fullmatch(r"(\d+\.\d+\.\d+)(?:-(alpha|beta|rc)\.([1-9]\d*))?", version)
    if not match:
        raise SystemExit(f"unsupported version: {version}")
    short, channel, number = match.groups()
    build = short + ({"alpha": "a", "beta": "b", "rc": "fc"}[channel] + number if channel else "")
    plistlib.dump({
        "CFBundleName": "course2md",
        "CFBundleDisplayName": "course2md",
        "CFBundleIdentifier": "dev.course2md.desktop",
        "CFBundleExecutable": "course2md-desktop",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": short,
        "CFBundleVersion": build,
        "Course2mdVersion": version,
        "NSHighResolutionCapable": True,
        "NSPrincipalClass": "NSApplication",
        "LSMinimumSystemVersion": "14.0",
        "CFBundleIconFile": "course2md.icns",
    }, (app / "Contents/Info.plist").open("wb"))
    PY
    cp desktop/assets/icon.icns "$res/course2md.icns"
    cp target/release/course2md "$bin/course2md"
    cp desktop/target/release/course2md-desktop "$bin/course2md-desktop"
    chmod 755 "$bin/course2md" "$bin/course2md-desktop"
    if [[ -f target/release/mlx.metallib ]]; then
        cp target/release/mlx.metallib "$res/mlx.metallib"
        ln -s ../Resources/mlx.metallib "$bin/mlx.metallib"
    elif [[ "$(uname -m)" == arm64 && -z "${COURSE2MD_NO_APPLE:-}" ]]; then
        echo "missing mlx.metallib from the CLI release build" >&2
        exit 1
    fi
    codesign --force --sign - "$bin/course2md"
    codesign --force --sign - "$bin/course2md-desktop"
    codesign --force --sign - "$app"
    codesign --verify --deep --strict "$app"

    dest=/Applications/course2md.app
    if pgrep -f "$dest/Contents/MacOS/course2md-desktop" >/dev/null; then
        osascript -e 'tell application "course2md" to quit' >/dev/null 2>&1 || true
        for _ in {1..30}; do
            pgrep -f "$dest/Contents/MacOS/course2md-desktop" >/dev/null || break
            sleep 0.3
        done
    fi
    staging=/Applications/.course2md.app.installing
    rm -rf "$staging"
    ditto "$app" "$staging"
    rm -rf "$dest"
    mv "$staging" "$dest"
    open "$dest"
