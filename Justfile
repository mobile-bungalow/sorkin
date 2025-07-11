
target_dir := env_var_or_default("CARGO_TARGET_DIR", "./target")
source_dir := source_directory()
package_name := env_var_or_default("CARGO_PKG_NAME", "sorkin")
bundle_dir := target_dir / (package_name + "_addon")

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

default: bundle

build:
    cargo build --release

bundle: build bundle-platform

[macos]
bundle-platform:
    mkdir -p {{bundle_dir}}/bin
    cp -r assets {{bundle_dir}}
    cp {{target_dir}}/release/lib{{package_name}}.dylib {{bundle_dir}}/bin/lib{{package_name}}.dylib

[linux]
bundle-platform:
    mkdir -p {{bundle_dir}}/bin
    cp -r assets {{bundle_dir}}
    cp {{target_dir}}/release/lib{{package_name}}.so {{bundle_dir}}/bin/lib{{package_name}}.so

[windows]
bundle-platform:
    New-Item -ItemType Directory -Force -Path "{{bundle_dir}}\bin"
    Copy-Item -Recurse -Force "assets" "{{bundle_dir}}"
    Copy-Item -Force "{{target_dir}}\release\{{package_name}}.dll" "{{bundle_dir}}\bin\{{package_name}}.dll"

[windows]
clean:
    cargo clean
    if (Test-Path "{{target_dir}}\bundle") { Remove-Item -Recurse -Force "{{target_dir}}\bundle" }

[macos]
clean:
    cargo clean
    rm -rf {{target_dir}}/bundle

[linux]
clean:
    cargo clean
    rm -rf {{target_dir}}/bundle
