#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)
cd "$REPO_ROOT"

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
cargo build --release -p uefi-gui-demo
EFI=target/x86_64-unknown-uefi/release/uefi-gui.efi

# ---------------------------------------------------------------------------
# Assemble a minimal ESP (FAT image via QEMU's built-in vvfat driver)
# ---------------------------------------------------------------------------
mkdir -p demo/esp/EFI/BOOT
cp "$EFI" demo/esp/EFI/BOOT/BOOTX64.EFI

# ---------------------------------------------------------------------------
# Locate OVMF firmware
# ---------------------------------------------------------------------------
OVMF_CODE=""
OVMF_VARS=""
for d in \
    /usr/share/OVMF \
    /usr/share/ovmf \
    /usr/share/edk2/ovmf \
    /usr/share/qemu \
    /usr/lib/ovmf; do
    if   [ -f "$d/OVMF_CODE.fd" ]; then
        OVMF_CODE="$d/OVMF_CODE.fd"
        OVMF_VARS="$d/OVMF_VARS.fd"
        break
    elif [ -f "$d/OVMF.fd" ]; then
        OVMF_CODE="$d/OVMF.fd"
        OVMF_VARS=""
        break
    fi
done

if [ -z "$OVMF_CODE" ]; then
    echo "ERROR: OVMF firmware not found."
    echo "Install with: sudo apt install ovmf   (Debian/Ubuntu)"
    echo "              sudo dnf install edk2-ovmf  (Fedora)"
    exit 1
fi

# ---------------------------------------------------------------------------
# KVM (optional — WSL2 usually lacks /dev/kvm)
# ---------------------------------------------------------------------------
KVM=""
#[ -c /dev/kvm ] && KVM="-enable-kvm -cpu host"

# ---------------------------------------------------------------------------
# Mouse device selection
#   default   : USB tablet via legacy USB (EFI_ABSOLUTE_POINTER_PROTOCOL).
#               -usb makes QEMU add an ICH9 EHCI controller and automatically
#               selects the tablet as the active absolute pointer for VNC events.
#   --ps2     : PS/2 mouse via i8042 (EFI_SIMPLE_POINTER_PROTOCOL).
#               Use this if AbsolutePointer still shows no reads.
# ---------------------------------------------------------------------------
LEGACY_USB=""
MOUSE_DEV="-device usb-tablet"
if [ "${1:-}" = "--ps2" ] || [ "${2:-}" = "--ps2" ]; then
    MOUSE_DEV="-device i8042"   # PS/2 controller; QEMU auto-connects a PS/2 mouse
    echo "Mouse: PS/2 (EFI_SIMPLE_POINTER_PROTOCOL)"
else
    LEGACY_USB="-usb"           # required to attach usb-tablet to legacy EHCI
    echo "Mouse: USB tablet on legacy EHCI (EFI_ABSOLUTE_POINTER_PROTOCOL)"
fi

# ---------------------------------------------------------------------------
# Run
# WSL2 + WSLg: use -display gtk.
# VNC: connect any viewer to  localhost:5901  (VNC display :1).
#       Every VNC client supports relative PS/2 movement natively.
# ---------------------------------------------------------------------------
DISPLAY_FLAGS="-display gtk,zoom-to-fit=off"
if [ "${1:-}" = "--vnc" ] || [ "${2:-}" = "--vnc" ]; then
    DISPLAY_FLAGS="-display vnc=127.0.0.1:1"
    echo "VNC server started on loopback only."
    echo "  Connect with:  vncviewer localhost:5901"
    echo "  Or (Windows):  mstsc → use a VNC add-in, or TigerVNC / RealVNC"
fi

# Copy OVMF_VARS to a writable temp file so QEMU can update NVRAM
TMP_VARS=$(mktemp /tmp/OVMF_VARS_XXXXXX.fd)
if [ -n "$OVMF_VARS" ] && [ -f "$OVMF_VARS" ]; then
    cp "$OVMF_VARS" "$TMP_VARS"
    PFLASH_VARS="-drive if=pflash,format=raw,file=$TMP_VARS"
else
    PFLASH_VARS=""
    # Single-file OVMF: pass as pflash read-only
    OVMF_CODE_FLAG="-drive if=pflash,format=raw,readonly=on,file=$OVMF_CODE"
fi

# shellcheck disable=SC2086
qemu-system-x86_64 \
    ${KVM} \
    -machine q35 \
    -m 256M \
    ${OVMF_CODE_FLAG:--drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE"} \
    ${PFLASH_VARS} \
    -drive format=raw,file=fat:rw:demo/esp \
    -device VGA \
    -device qemu-xhci,id=xhci \
    -device usb-kbd,bus=xhci.0 \
    ${LEGACY_USB} \
    ${MOUSE_DEV} \
    $DISPLAY_FLAGS \
    -serial stdio \
    -nodefaults \
    -no-reboot

rm -f "$TMP_VARS"
