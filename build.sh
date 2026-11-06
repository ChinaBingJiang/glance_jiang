#!/usr/bin/env bash
# Build Glance (official 0.4.2 source, unmodified).
# Source lives at J:\Projects\glance and this script sits next to Cargo.toml.
#   ./build.sh            -> cargo build --profile fast   (optimised, parallel, for trying changes)
#   ./build.sh release    -> cargo build --release        (LTO + codegen-units=1, for shipping)
#
# Note: the very first cold build on a fresh target\ fails once with
#   "failed to write ...\.fingerprint\glance-<hash>\invoked.timestamp: 拒绝访问 (os error 5)"
# It is not a permission problem - just run it again and it finishes.
set -u
cd "$(dirname "$0")" || exit 1
# shellcheck source=build-env.sh
. ./build-env.sh

if [ "${1:-}" = "release" ]; then
  exec cargo build --release
else
  exec cargo build --profile fast
fi
