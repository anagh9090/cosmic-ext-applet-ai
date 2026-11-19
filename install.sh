#!/usr/bin/env bash
set -euo pipefail

echo "This script uses sudo commands, but is not to be directly run as root!"

if [ "$(id -u)" -eq 0 ]; then
  echo "Error: Do not run this script with sudo or as root." >&2
  exit 2
fi


if [[ ! -r /etc/os-release ]]; then
  echo "Error: /etc/os-release not found. Cannot detect OS." >&2
  exit 1
fi

# Load distro metadata (ID, ID_LIKE, etc.)
# shellcheck source=/dev/null
. /etc/os-release

distro_id="${ID:-}"
distro_like="${ID_LIKE:-}"
combined="${distro_id} ${distro_like}"

install_with_apt() {
  sudo apt-get update
  sudo apt-get install -y just git
}

install_with_dnf() {
  sudo dnf install -y just
}

install_with_pacman() {
  sudo pacman -Sy --noconfirm just
}

clone-applet(){
    echo "Cloning ChatAI Applet"
    cd /tmp/
    git clone https://github.com/anagh9090/cosmic-ext-applet-ai
    echo "Cloning Finished!"
}

build-applet(){
    echo "Building ChatAI Applet from Scratch"
    cd /tmp/cosmic-ext-applet-ai
    just build-release || just build-no-wgpu
    echo "Build Completed Successfully!"
}

install-applet(){
    cd /tmp/cosmic-ext-applet-ai
    sudo just install
    echo "Installation Completed Successfully!"
}

cleanup-applet(){
    echo "Cleaning Up Unnecessary Source Code"
    cd /tmp
    rm -rf cosmic-ext-applet-ai/
    echo "Cleaning Complete!"
}
work-finished(){
    echo "ChatAI Applet Installed, Enable it in COSMIC Settings!"
}

main(){
    clone-applet
    
    build-applet
    
    install-applet
    
    cleanup-applet
    
    work-finished
}

if command -v just >/dev/null 2>&1; then
  echo "'just' is already installed: $(just --version)"
  echo "Continuing To Other Steps!"
  main
fi


case "$combined" in
  *arch*|*manjaro*|*endeavouros*)
    echo "Detected Arch-based distro ($distro_id). Installing just via pacman..."
    install_with_pacman
    ;;
  *debian*|*ubuntu*|*mint*|*pop*)
    echo "Detected Debian-based distro ($distro_id). Installing just via apt..."
    install_with_apt
    ;;
  *fedora*|*rhel*|*centos*|*rocky*|*alma*)
    echo "Detected Fedora/RHEL-based distro ($distro_id). Installing just via dnf..."
    install_with_dnf
    ;;
  *)
    echo "Unsupported or unrecognized distro: ID='$distro_id', ID_LIKE='$distro_like'" >&2
    echo "You can install 'just' via Cargo (cargo install just) or check the official releases." >&2
    exit 1
    ;;
esac

echo "Successfully installed: $(just --version)"

echo "=============================="

exit 0
