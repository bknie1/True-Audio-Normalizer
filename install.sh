#!/bin/sh
# TAN installer for macOS/Linux.
# Usage:  curl -fsSL https://raw.githubusercontent.com/bknie1/True-Audio-Normalizer/main/install.sh | sh
set -e

repo="bknie1/True-Audio-Normalizer"
install_dir="$HOME/.local/bin"

case "$(uname -s)" in
    Darwin) platform="macos-arm64" ;;
    Linux) platform="linux-x86_64" ;;
    *) echo "Unsupported platform: $(uname -s)"; exit 1 ;;
esac

echo "Finding latest TAN release..."
asset_url=$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest" \
    | grep "browser_download_url.*tan-$platform" \
    | sed -E 's/.*"(https[^"]+)".*/\1/')

if [ -z "$asset_url" ]; then
    echo "No release found for $platform. Has a release been published yet?" >&2
    exit 1
fi

tmp=$(mktemp -d)
echo "Downloading $asset_url..."
curl -fsSL "$asset_url" -o "$tmp/tan.zip"

mkdir -p "$install_dir"
unzip -oq "$tmp/tan.zip" -d "$install_dir"
chmod +x "$install_dir/tan-cli"
rm -rf "$tmp"

echo ""
echo "Installed tan-cli to $install_dir"
case ":$PATH:" in
    *":$install_dir:"*) ;;
    *) echo "Add it to your PATH:  export PATH=\"\$PATH:$install_dir\"" ;;
esac
echo "Try it:"
echo "  tan-cli gen demo.wav"
echo "  tan-cli process demo.wav out.wav movie --two-pass"
