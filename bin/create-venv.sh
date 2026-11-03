#!/bin/sh
# Create a virtual environment for development on this project.
#
# Usage:
#     bin/create-venv.sh [DIR]      # DIR defaults to ./venv
#     . init-env.sh                 # activate (run `deactivate` to undo)
set -eu

# Keep these in sync with:
# - .devcontainer/acap-native-sdk-12-aarch64/devcontainer.json
# - .devcontainer/acap-native-sdk-12-armv7hf/devcontainer.json
# - .github/workflows/fuzz.yaml
# - .github/workflows/main.yaml
# - flake.nix
SDK_IMAGE="axisecp/acap-native-sdk"
SDK_VERSION="12.11.0"
SDK_UBUNTU="ubuntu24.04"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENV_DIR="$REPO_ROOT/venv"
SDK_DIR="$VENV_DIR/acap-native-sdk"
INIT_ENV="$REPO_ROOT/init-env.sh"

mkdir -p "$SDK_DIR"

for arch in armv7hf aarch64; do
  docker run --platform linux/amd64 $SDK_IMAGE:$SDK_VERSION-$arch-$SDK_UBUNTU tar \
    --create \
    --directory /opt/ \
    --file - \
    --mode ugo+rwX \
    axis \
  | tar \
    --directory "${SDK_DIR}" \
    --extract \
    --file - \
    --strip-components 1
done

cat >"$INIT_ENV" <<EOF
# Activate the ACAP virtual environment created by bin/create-venv.sh.
# Source this file; do not execute it:
#     . init-env.sh

export ACAP_VENV="$VENV_DIR"
export ACAP_SDK_LOCATION="$SDK_DIR"
EOF
