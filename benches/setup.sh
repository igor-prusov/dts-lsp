#!/usr/bin/env bash
# Fetch a pinned Linux kernel DTS subset for benchmarking.
# Uses a blobless sparse clone to download only devicetree sources.
set -euo pipefail

TAG="${DTS_LSP_KERNEL_TAG:-v6.12}"
DEST="${DTS_LSP_CORPUS_DIR:-target/bench-corpus/linux}"

if [ -d "$DEST/.git" ]; then
    echo "Corpus already present at $DEST"
    exit 0
fi

mkdir -p "$(dirname "$DEST")"

git clone \
    --depth 1 \
    --filter=blob:none \
    --sparse \
    --branch "$TAG" \
    https://github.com/torvalds/linux.git \
    "$DEST"

git -C "$DEST" sparse-checkout set \
    arch/arm64/boot/dts \
    arch/arm/boot/dts \
    include/dt-bindings

echo "Corpus ready at $DEST (tag $TAG)"
echo "DTS files: $(find "$DEST/arch" -name '*.dts' -o -name '*.dtsi' | wc -l)"
