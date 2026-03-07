target_dir := env_var_or_default("CARGO_TARGET_DIR", "./target")
source_dir := source_directory()
package_name := env_var_or_default("CARGO_PKG_NAME", "sorkin")
bundle_dir := target_dir / (package_name + "_addon")
addon_dir := bundle_dir / "addons" / package_name

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

default: bundle

build:
    cargo build --release

bundle: build bundle-platform

[macos]
bundle-platform:
    #!/usr/bin/env bash
    set -e
    FRAMEWORK="{{addon_dir}}/bin/lib{{package_name}}.framework"
    BINARY="$FRAMEWORK/lib{{package_name}}"
    mkdir -p "{{addon_dir}}/bin"
    mkdir -p "$FRAMEWORK/Resources"
    cp -r assets/sorkin/* "{{addon_dir}}/"
    cp assets/sorkin.gdextension "{{addon_dir}}/"
    cp "{{target_dir}}/release/lib{{package_name}}.dylib" "$BINARY"
    cp assets/Info.plist.template "$FRAMEWORK/Resources/Info.plist"
    # Bundle Homebrew dylib dependencies and rewrite paths to @loader_path
    for dep in $(otool -L "$BINARY" | awk '/\/opt\/homebrew/{print $1}'); do
        lib=$(basename "$dep")
        cp "$dep" "$FRAMEWORK/$lib"
        install_name_tool -change "$dep" "@loader_path/$lib" "$BINARY"
    done
    echo "✓ Framework created at $FRAMEWORK"

[linux]
bundle-platform:
    mkdir -p {{addon_dir}}/bin
    cp -r assets/sorkin/* {{addon_dir}}/
    cp assets/sorkin.gdextension {{addon_dir}}/
    cp {{target_dir}}/release/lib{{package_name}}.so {{addon_dir}}/bin/lib{{package_name}}.linux.x86_64.so

[windows]
bundle-platform:
    New-Item -ItemType Directory -Force -Path "{{addon_dir}}\bin"
    Copy-Item -Recurse -Force "assets\sorkin\*" "{{addon_dir}}"
    Copy-Item -Force "assets\sorkin.gdextension" "{{addon_dir}}"
    Copy-Item -Force "{{target_dir}}\release\{{package_name}}.dll" "{{addon_dir}}\bin\{{package_name}}.windows.x86_64.dll"
    if (Test-Path env:FFMPEG_DIR) { foreach ($p in @('avcodec-*','avdevice-*','avformat-*','avutil-*','libvpx*','opus*','libopus*')) { Get-ChildItem "$env:FFMPEG_DIR\bin\$p.dll" -EA SilentlyContinue | Copy-Item -Destination "{{addon_dir}}\bin\" } }

clean:
    cargo clean
    rm -rf {{bundle_dir}}
