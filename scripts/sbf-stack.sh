#!/usr/bin/env bash
# Rebuild final crates before checking stack diagnostics.

fresh_sbf_build() {
    local manifest=$1 package=$2 arch=$3 out=$4 log=$5
    shift 5
    local target=sbpf-solana-solana
    if [[ $arch == v3 ]]; then target=sbpfv3-solana-solana; fi
    mkdir -p "$out" "$(dirname -- "$log")"
    : > "$log"
    local rustc=${BLUBBER_SBF_RUST:+$BLUBBER_SBF_RUST/bin/rustc}
    if [[ -z $rustc ]]; then
        if ! rustc=$(rustup which --toolchain 1.89.0-sbpf-solana-v1.53 rustc 2>/dev/null); then
            if ! cargo-build-sbf --tools-version v1.53 --install-only >> "$log" 2>&1; then
                cat "$log" >&2
                return 1
            fi
            rustc="$HOME/.cache/solana/v1.53/platform-tools/rust/bin/rustc"
        fi
    fi
    if [[ ! -x $rustc ]] || ! "$rustc" -vV | grep -Fxq 'LLVM version: 20.1.7'; then
        echo 'Expected the v1.53 SBF compiler with LLVM 20.1.7.' >&2
        return 1
    fi
    local -a compiler=(env RUSTC="$rustc" PATH="$(dirname -- "$rustc"):$PATH")
    if ! "${compiler[@]}" cargo clean --manifest-path "$manifest" --release --target "$target" \
        --package "$package" --locked >> "$log" 2>&1; then
        cat "$log" >&2
        return 1
    fi
    if ! "${compiler[@]}" cargo-build-sbf --tools-version v1.53 --skip-tools-install --no-rustup-override \
        --jobs "${CARGO_BUILD_JOBS:-4}" --arch "$arch" --manifest-path "$manifest" --sbf-out-dir "$out" \
        "$@" -- --locked >> "$log" 2>&1; then
        cat "$log" >&2
        return 1
    fi
    if grep -Eqi 'overflows the maximum|stack offset|stack frame.*exceed|overwrites values in the frame' "$log"; then
        echo 'SBF stack diagnostic rejected (4,096-byte frame limit).' >> "$log"
        cat "$log" >&2
        return 1
    fi
}

check_stack_rejection() {
    local attempt log
    # First build starts clean; second starts with the rejected build cached.
    # Both must recompile and reject LLVM's nonfatal overflow diagnostics.
    for attempt in fresh cached; do
        log=".tmp/stack-rejection-$attempt.log"
        if fresh_sbf_build programs/stack-matrix/Cargo.toml blubber-stack-matrix \
            v0 "$CARGO_TARGET_DIR/stack-overflow" "$log" --features overflow \
            > ".tmp/stack-rejection-$attempt-output.log" 2>&1; then
            echo "Stack gate accepted overflowing fixture ($attempt)." >&2
            return 1
        fi
        if ! rg -q 'SBF stack diagnostic rejected' "$log"; then
            cat "$log" >&2
            echo "Overflow fixture failed without a stack diagnostic ($attempt)." >&2
            return 1
        fi
    done
    echo 'Stack gate rejected the overflowing fixture twice (fresh and cached).'
}


if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
    set -euo pipefail
    if [[ $# -lt 5 || ! $3 =~ ^(v0|v3)$ ]]; then
        echo 'Usage: sbf-stack.sh MANIFEST PACKAGE v0|v3 OUTPUT_DIR LOG [SBF_FLAGS...]' >&2
        exit 2
    fi
    fresh_sbf_build "$@"
fi
