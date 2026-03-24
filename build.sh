#!/bin/bash
set -e

REMOTE="largitdata-wifi-pool"
REMOTE_DIR="/home/largitdata/project/largitdata-wifi-pool-ui"

echo "==> Syncing source to remote..."
rsync -av --exclude target --exclude .git \
  /Users/joe/project/largitdata-wifi-pool-ui/ \
  ${REMOTE}:${REMOTE_DIR}/

echo "==> Building release on remote..."
ssh ${REMOTE} "source ~/.cargo/env && cd ${REMOTE_DIR} && cargo build --release 2>&1"

echo "==> Done! Binary at ${REMOTE_DIR}/target/release/largitdata-wifi-pool-ui"
echo ""
echo "To restart service:"
echo "  ssh ${REMOTE} 'systemctl --user restart largitdata-wifi-pool-ui'"
