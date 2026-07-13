#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(awk -F\" '/^version = / { print $2; exit }' "$ROOT_DIR/Cargo.toml")"
TARGET="${TARGET:-$(uname -m)-unknown-linux-gnu}"
NAME="gsw-v${VERSION}-${TARGET}"
DIST_DIR="$ROOT_DIR/dist"
PACKAGE_DIR="$DIST_DIR/$NAME"

cargo build --release

rm -rf "$PACKAGE_DIR"
mkdir -p "$PACKAGE_DIR"

cp "$ROOT_DIR/target/release/gsw" "$PACKAGE_DIR/gsw"
cp "$ROOT_DIR/README.md" "$PACKAGE_DIR/README.md"
mkdir -p "$PACKAGE_DIR/config" "$PACKAGE_DIR/packaging/systemd"
cp "$ROOT_DIR/config/gsw.example.toml" "$PACKAGE_DIR/config/gsw.example.toml"
cp "$ROOT_DIR/packaging/systemd/gsw.service" "$PACKAGE_DIR/packaging/systemd/gsw.service"

cat > "$PACKAGE_DIR/INSTALL.txt" <<'EOF'
Install:
  sudo install -m 0755 gsw /usr/local/bin/gsw
  sudo install -d -m 0755 /etc/gsw /var/lib/gsw
  sudo install -m 0644 config/gsw.example.toml /etc/gsw/config.toml
  sudo install -m 0644 packaging/systemd/gsw.service /etc/systemd/system/gsw.service

Enable after reviewing /etc/gsw/config.toml:
  sudo systemctl daemon-reload
  sudo systemctl enable --now gsw

Check:
  gsw --help

Runtime dependency:
  gsw links to libsqlite3.

  Arch/CachyOS:
    sudo pacman -S sqlite

  Ubuntu/Debian:
    sudo apt install libsqlite3-0
EOF

tar -C "$DIST_DIR" -czf "$DIST_DIR/$NAME.tar.gz" "$NAME"
sha256sum "$DIST_DIR/$NAME.tar.gz" > "$DIST_DIR/$NAME.tar.gz.sha256"

echo "$DIST_DIR/$NAME.tar.gz"
