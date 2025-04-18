#!/usr/bin/env sh
set -eux

docker run \
  --interactive \
  --mount "type=tmpfs,dst=$(pwd)/target" \
  --net="host" \
  --rm \
  --tty \
  --volume "$(pwd):$(pwd):ro" \
  --workdir "$(pwd)" \
  nixos/nix
