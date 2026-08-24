#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)
cd "$REPO_ROOT"

# ---------------------------------------------------------------------------
# Output mode
# ---------------------------------------------------------------------------
ISO_ONLY=false
ISO_OUT="${ISO_OUT:-demo/uefi-gui.iso}"
UEFI_TARGET="${UEFI_TARGET:-x86_64-unknown-uefi}"
for arg in "$@"; do
    case "$arg" in
        --iso)
            ISO_ONLY=true
            ;;
        --iso=*)
            ISO_ONLY=true
            ISO_OUT=${arg#--iso=}
            if [ -z "$ISO_OUT" ]; then
                echo "ERROR: --iso= requires a non-empty output path."
                exit 2
            fi
            ;;
    esac
done

case "$UEFI_TARGET" in
    x86_64-unknown-uefi)
        BOOT_EFI=BOOTX64.EFI
        ;;
    aarch64-unknown-uefi)
        BOOT_EFI=BOOTAA64.EFI
        ;;
    *)
        echo "ERROR: unsupported UEFI_TARGET: $UEFI_TARGET"
        echo "Supported: x86_64-unknown-uefi, aarch64-unknown-uefi"
        exit 2
        ;;
esac

if ! $ISO_ONLY && [ "$UEFI_TARGET" != "x86_64-unknown-uefi" ]; then
    echo "ERROR: QEMU launch mode supports only x86_64-unknown-uefi."
    echo "Use --iso to export $UEFI_TARGET for another platform."
    exit 2
fi

if $ISO_ONLY; then
    missing_tools=""
    for tool in xorriso mkfs.vfat mmd mcopy truncate; do
        if ! command -v "$tool" >/dev/null 2>&1; then
            missing_tools="$missing_tools $tool"
        fi
    done
    if [ -n "$missing_tools" ]; then
        echo "ERROR: ISO generation requires these missing tools:$missing_tools"
        echo "Install with: sudo apt install xorriso dosfstools mtools"
        echo "              sudo dnf install xorriso dosfstools mtools"
        exit 1
    fi
fi

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
cargo build --release -p uefi-gui-demo --target "$UEFI_TARGET"
EFI="target/$UEFI_TARGET/release/uefi-gui.efi"

# ---------------------------------------------------------------------------
# Optionally generate a UEFI-bootable ISO and stop before locating OVMF/QEMU.
# The El Torito EFI image is a FAT filesystem containing the removable-media
# architecture-specific EFI/BOOT/BOOT*.EFI path. ISO_OUT or --iso=PATH selects
# the output path.
# ---------------------------------------------------------------------------
if $ISO_ONLY; then
    ISO_TMP=$(mktemp -d /tmp/uefi-gui-iso_XXXXXX)
    cleanup_iso() {
        rm -rf -- "$ISO_TMP"
    }
    trap cleanup_iso EXIT

    ISO_ROOT="$ISO_TMP/root"
    ESP_IMAGE="$ISO_ROOT/EFI/BOOT/efiboot.img"
    mkdir -p "$ISO_ROOT/EFI/BOOT"
    truncate -s 4M "$ESP_IMAGE"
    mkfs.vfat -n UEFI_BOOT "$ESP_IMAGE" >/dev/null
    mmd -i "$ESP_IMAGE" ::/EFI ::/EFI/BOOT
    mcopy -i "$ESP_IMAGE" "$EFI" "::/EFI/BOOT/$BOOT_EFI"

    # Also include the executable as a normal ISO file for inspection/copying.
    cp "$EFI" "$ISO_ROOT/EFI/BOOT/$BOOT_EFI"
    mkdir -p "$(dirname -- "$ISO_OUT")"
    xorriso -as mkisofs \
        -R -J \
        -V UEFI_GUI \
        -e EFI/BOOT/efiboot.img \
        -no-emul-boot \
        -o "$ISO_OUT" \
        "$ISO_ROOT"

    echo "UEFI bootable ISO ($UEFI_TARGET, $BOOT_EFI): $ISO_OUT"
    exit 0
fi

# ---------------------------------------------------------------------------
# Assemble a minimal ESP directory for QEMU's built-in vvfat driver.
# ---------------------------------------------------------------------------
mkdir -p demo/esp/EFI/BOOT
cp "$EFI" "demo/esp/EFI/BOOT/$BOOT_EFI"

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
# Pointer device selection
#   default       : virtio tablet for OVMF builds with tablet-capable
#                   VirtioInputDxe.
#   --usb-tablet  : QEMU USB HID tablet on the existing xHCI controller. This
#                   requires firmware with a USB HID tablet DXE driver; stock
#                   OVMF builds commonly enumerate it without publishing a
#                   pointer protocol.
#   --ps2         : q35's built-in i8042 / the library's direct PS/2 fallback.
# ---------------------------------------------------------------------------
POINTER_DEV="-device virtio-tablet-pci"
if [ "${1:-}" = "--ps2" ] || [ "${2:-}" = "--ps2" ]; then
    # q35 includes i8042 even with -nodefaults; adding one would duplicate it.
    POINTER_DEV=""
    echo "Pointer: q35 built-in PS/2 (EFI Simple Pointer or direct i8042 fallback)"
elif [ "${1:-}" = "--usb-tablet" ] || [ "${2:-}" = "--usb-tablet" ]; then
    POINTER_DEV="-device usb-tablet,bus=xhci.0"
    echo "Pointer: USB tablet on xHCI (requires a firmware USB tablet driver)"
else
    echo "Pointer: virtio tablet (requires tablet-capable OVMF VirtioInput)"
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
    ${POINTER_DEV} \
    $DISPLAY_FLAGS \
    -serial stdio \
    -nodefaults \
    -no-reboot

rm -f "$TMP_VARS"
