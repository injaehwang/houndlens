#!/bin/sh
# omnilens installer
# Usage: curl -fsSL https://omnilens.dev/install.sh | sh
#    or: curl -fsSL https://raw.githubusercontent.com/injaehwang/omnilens/main/install.sh | sh

set -eu

REPO="injaehwang/omnilens"
INSTALL_DIR="${OMNILENS_INSTALL_DIR:-$HOME/.omnilens/bin}"

# Detect platform.
detect_platform() {
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    ARCH=$(uname -m)

    case "$OS" in
        linux)  OS="linux" ;;
        darwin) OS="darwin" ;;
        mingw*|msys*|cygwin*) OS="win32" ;;
        *) echo "Unsupported OS: $OS"; exit 1 ;;
    esac

    case "$ARCH" in
        x86_64|amd64)   ARCH="x64" ;;
        aarch64|arm64)  ARCH="arm64" ;;
        *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
    esac

    echo "${OS}-${ARCH}"
}

# Get latest release tag.
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name": *"//;s/".*//'
}

main() {
    PLATFORM=$(detect_platform)
    echo "Detected platform: ${PLATFORM}"

    VERSION="${OMNILENS_VERSION:-$(get_latest_version)}"
    if [ -z "$VERSION" ]; then
        echo "Could not determine latest version. Set OMNILENS_VERSION manually."
        exit 1
    fi
    echo "Installing omnilens ${VERSION}..."

    # Determine download URL.
    ARTIFACT="omnilens-${PLATFORM}"
    if [ "$PLATFORM" = "win32-x64" ]; then
        URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARTIFACT}.zip"
    else
        URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARTIFACT}.tar.gz"
    fi

    # Create install directory.
    mkdir -p "$INSTALL_DIR"

    # Download and extract.
    TMPDIR=$(mktemp -d)
    echo "Downloading ${URL}..."

    if [ "$PLATFORM" = "win32-x64" ]; then
        curl -fsSL "$URL" -o "${TMPDIR}/omnilens.zip"
        unzip -o "${TMPDIR}/omnilens.zip" -d "${TMPDIR}" > /dev/null
    else
        curl -fsSL "$URL" | tar xz -C "${TMPDIR}"
    fi

    # Install binary.
    if [ -f "${TMPDIR}/omnilens" ]; then
        mv "${TMPDIR}/omnilens" "${INSTALL_DIR}/omnilens"
        chmod +x "${INSTALL_DIR}/omnilens"
    elif [ -f "${TMPDIR}/omnilens.exe" ]; then
        mv "${TMPDIR}/omnilens.exe" "${INSTALL_DIR}/omnilens.exe"
    else
        echo "Error: binary not found in download"
        exit 1
    fi

    rm -rf "${TMPDIR}"

    # Check if install dir is in PATH.
    case ":$PATH:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            SHELL_NAME=$(basename "$SHELL" 2>/dev/null || echo "bash")
            case "$SHELL_NAME" in
                zsh)  RC="$HOME/.zshrc" ;;
                fish) RC="$HOME/.config/fish/config.fish" ;;
                *)    RC="$HOME/.bashrc" ;;
            esac

            echo ""
            echo "Add omnilens to your PATH:"
            echo ""
            if [ "$SHELL_NAME" = "fish" ]; then
                echo "  fish_add_path ${INSTALL_DIR}"
            else
                echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ${RC}"
                echo "  source ${RC}"
            fi
            ;;
    esac

    echo ""
    echo "✓ omnilens ${VERSION} installed to ${INSTALL_DIR}"
    echo ""
    echo "Get started:"
    echo "  cd your-project"
    echo "  omnilens init && omnilens index"
    echo "  omnilens verify --diff HEAD~1"
}

main "$@"
