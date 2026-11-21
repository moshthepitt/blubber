#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$root"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$root/target"}
scratch="$CARGO_TARGET_DIR/reproduce"
mkdir -p "$scratch"

require_version() {
    local expected=$1 output
    shift
    output=$("$@" --version)
    if ! grep -Fxq "$expected" <<< "$output"; then
        printf 'Expected %s from %s; got:\n%s\n' "$expected" "$*" "$output" >&2
        return 1
    fi
}

host() {
    cargo +1.97.1 "$@"
}

verus_version() {
    export RUSTUP_TOOLCHAIN=1.99.1
    require_version '  Version: 0.2026.10.18.7d82994' verus
    if [[ $(rustc --version) != 'rustc 1.99.1 '* ]]; then
        echo 'Verus requires Rust 1.99.1.' >&2
        return 1
    fi
}

sbf_version() {
    require_version 'cargo-build-sbf 4.0.0' cargo-build-sbf
    require_version 'platform-tools v1.53' cargo-build-sbf
    if [[ -n ${BLUBBER_SBF_RUST:-} ]]; then
        "$BLUBBER_SBF_RUST/bin/rustc" -vV | grep -Fx 'LLVM version: 20.1.7'
    fi
    source scripts/sbf-stack.sh
}

build() {
    local name=$1 output=$2 package=blubber-$1
    if [[ $name == blubber-* ]]; then package=$name; fi
    shift 2
    fresh_sbf_build "programs/$name/Cargo.toml" "$package" "$arch" "$output" \
        "$scratch/build-$arch-${output##*/}-$name.log" "$@"
}

build_policy_fuzz_fixtures() {
    local arch fixtures
    fixtures=$(mktemp -d "$scratch/policy-fixtures.XXXXXX")
    sbf_version
    for arch in v0 v3; do
        build policy "$fixtures/$arch" --features corrupt
    done
    export BLUBBER_POLICY_ELF="$fixtures/v0/blubber_policy"
    export BLUBBER_POLICY_ELF_V3="$fixtures/v3/blubber_policy"
}

quickstart() {
    local arch out
    for arch in v0 v3; do
        out="$scratch/quickstart-$arch"
        build blubber-quickstart "$out"
        RUST_LOG=${RUST_LOG:-error} SBF_OUT_DIR="$out" host test --locked \
            -p blubber-mollusk --features sbf-tests --test quickstart
    done
}

runtime() {
    local arch
    for arch in v0 v3; do
        local out="$scratch/runtime-$arch" variants="$scratch/variants-$arch"
        build blubber-demo "$out"
        build blubber-quickstart "$out"
        build borrow-resize "$out"
        build cu-fixture "$out"
        build borrow-resize "$variants" --features alloc
        cp "$variants/blubber_borrow_resize.so" "$out/blubber_borrow_resize_alloc.so"
        build borrow-resize "$variants" --features runtime
        cp "$variants/blubber_borrow_resize.so" "$out/blubber_borrow_resize_runtime.so"
        build cu-fixture "$variants" --features depth16
        cp "$variants/blubber_cu_fixture.so" "$out/blubber_cu_fixture_16.so"
        build cu-fixture "$variants" --features matrix
        cp "$variants/blubber_cu_fixture.so" "$out/blubber_cu_runtime.so"
        build cu-fixture "$variants" --features matrix,alloc
        cp "$variants/blubber_cu_fixture.so" "$out/blubber_cu_runtime_alloc.so"
        local depth width mode
        for depth in 8 16; do
            for width in narrow medium wide; do
                for mode in no-alloc alloc; do
                    local -a features=() flags=()
                    [[ $depth == 8 ]] || features+=(depth16)
                    [[ $width == narrow ]] || features+=("$width")
                    [[ $mode == no-alloc ]] || features+=(alloc)
                    if (( ${#features[@]} )); then
                        flags=(--features "$(IFS=,; echo "${features[*]}")")
                    fi
                    build stack-matrix "$variants" "${flags[@]}"
                    cp "$variants/blubber_stack_matrix.so" \
                        "$out/blubber_stack_matrix_${depth}_${width}_${mode}.so"
                done
            done
        done
        RUST_LOG=${RUST_LOG:-error} SBF_OUT_DIR="$out" host test --locked -p blubber-mollusk --features sbf-tests
        RUST_LOG=${RUST_LOG:-error} SBF_OUT_DIR="$out" host test --locked -p blubber-mollusk --features sbf-tests,alloc
    done
}

measure() {
    unset BLUBBER_CEILINGS_PATH BLUBBER_CU_METRICS SBF_TRACE_DIR SBF_TRACE_DISASSEMBLE
    local arch mode features artifact
    for arch in v0 v3; do
        local out="$scratch/cu-$arch" variants="$scratch/cu-variants-$arch"
        build cu-fixture "$out"
        build blubber-demo "$out"
        build borrow-resize "$out"
        build borrow-resize "$variants" --features runtime
        cp "$variants/blubber_borrow_resize.so" "$out/blubber_borrow_resize_runtime.so"
        for mode in plain alloc; do
            features=matrix
            artifact=blubber_cu_matrix
            if [[ $mode == alloc ]]; then features+=,alloc; artifact+=_alloc; fi
            build cu-fixture "$variants" --features "$features"
            cp "$variants/blubber_cu_fixture.so" "$out/$artifact.so"
        done
        BLUBBER_BENCH_OUT="$out" SBF_OUT_DIR="$out" RUST_LOG=error \
            host bench --locked -p blubber-mollusk --bench compute_units
    done
}

compare() {
    local arch mode out
    local -a flags
    local features=baselines,zeropod,wincode,pinapod,light,manifest
    export COMPARE_LIBS=0,1,2,3,4,5,6,7,8,9,10,11,12,13,14
    unset COMPARE_HEAP COMPARE_CALIBRATE
    for arch in v0 v3; do
        for mode in whole heap; do
            out="$scratch/compare-$arch-$mode"
            flags=()
            local program_features=$features
            if [[ $mode == heap ]]; then
                export COMPARE_HEAP=1
                flags=(--features heap)
                program_features+=,heap
            fi
            fresh_sbf_build benchmarks/compare/program/Cargo.toml compare-program "$arch" "$out" \
                "$out/program.log" --features "$program_features"
            fresh_sbf_build benchmarks/compare/star/Cargo.toml compare-star "$arch" "$out" \
                "$out/star.log" "${flags[@]}"
            fresh_sbf_build benchmarks/compare/slab/Cargo.toml compare-slab "$arch" "$out" \
                "$out/slab.log" "${flags[@]}"
            COMPARE_ARCH="$arch" COMPARE_OUT="$out/rows.json" SBF_OUT_DIR="$out" RUST_LOG=error \
                host run --release --locked --manifest-path benchmarks/compare/Cargo.toml \
                -p compare-harness --features competitors
            unset COMPARE_HEAP
        done
        out="$scratch/compare-$arch-calibration"
        build cu-fixture "$out" --features mixed-operations
        COMPARE_CALIBRATE=1 COMPARE_ARCH="$arch" COMPARE_OUT="$out/rows.json" \
            SBF_OUT_DIR="$out" RUST_LOG=error \
            host run --release --locked --manifest-path benchmarks/compare/Cargo.toml \
            -p compare-harness --features competitors
    done
}

case "${1:-}" in
    test)
        [[ $# == 1 ]]
        host test --workspace --locked
        host test -p blubber --doc --no-default-features --locked
        host test -p blubber -p blubber-doc-examples --doc --all-features --locked
        ;;
    docs)
        [[ $# == 1 ]]
        host fetch --manifest-path scripts/api-listing/Cargo.toml --locked
        python3 scripts/public-api.py
        if [[ -f verification/docs-inputs.json ]]; then
            python3 scripts/docs-check.py
        else
            python3 scripts/check-docs.py
        fi
        ;;
    install)
        [[ $# == 1 ]]
        python3 scripts/check-install.py --scratch "$scratch"
        ;;
    verus)
        [[ $# == 1 ]]
        verus_version
        cargo verus verify -p blubber-core --locked --fwd-verus-args-to roots \
            -- --no-cheating --rlimit 60
        ;;
    kani)
        [[ $# == 1 ]]
        require_version 'Kani Rust Verifier 0.68.0 (standalone)' kani
        require_version 'CBMC 6.11.0' kani
        verus_version
        verus verification/map-key-array.rs --crate-type lib --no-cheating --rlimit 60 \
            --out-dir "$scratch"
        unset RUSTUP_TOOLCHAIN
        manifest=verification/kani/Cargo.toml
        lock=$(sha256sum verification/kani/Cargo.lock)
        host metadata --manifest-path "$manifest" --locked --format-version 1 > /dev/null
        for group in arithmetic leaf derive_encode error_codes map_key fixed_struct record typed_plan; do
            cargo kani --manifest-path "$manifest" --force-build --harness "$group::" \
                --jobs 1 -Z unstable-options --harness-timeout 600s --output-format terse
        done
        [[ $(sha256sum verification/kani/Cargo.lock) == "$lock" ]]
        ;;
    miri)
        [[ $# == 1 ]]
        export RUSTUP_TOOLCHAIN=nightly-2025-12-08
        require_version 'miri 0.1.0 (ba2142a19c 2025-12-07)' cargo miri
        require_version 'rustc 1.94.0-nightly (ba2142a19 2025-12-07)' rustc
        tests=() skips=()
        while IFS=$'\t' read -r crate module selection skip _; do
            [[ $crate == blubber && $selection == included ]] || continue
            tests+=(--test "$module")
            if [[ $skip != - ]]; then
                IFS=, read -ra names <<< "$skip"
                for name in "${names[@]}"; do skips+=(--skip "$name"); done
            fi
        done < verification/miri-modules.tsv
        for flags in '-Zmiri-strict-provenance' '-Zmiri-strict-provenance -Zmiri-tree-borrows'; do
            MIRIFLAGS="$flags" cargo miri test -p blubber --locked --no-default-features \
                --features alloc,derive --lib "${tests[@]}" -- "${skips[@]}"
        done
        ;;
    sbf)
        [[ $# -le 2 ]]
        case "${2:-}" in
            '') sbf_version; runtime ;;
            smoke) sbf_version; quickstart ;;
            *) echo 'Expected sbf or sbf smoke.' >&2; exit 2 ;;
        esac
        ;;
    fuzz)
        [[ $# -le 3 ]]
        target=${2:-validate}
        seconds=300
        if [[ $# == 3 ]]; then
            if [[ $target != validate || $3 != smoke ]]; then
                echo 'Expected fuzz validate smoke or fuzz [TARGET].' >&2
                exit 2
            fi
            seconds=15
        fi
        case "$target" in
            validate|edit)
                export RUSTUP_TOOLCHAIN=nightly-2025-12-08
                require_version 'rustc 1.94.0-nightly (ba2142a19 2025-12-07)' rustc
                require_version 'cargo-fuzz 0.12.0' cargo fuzz
                features=()
                seed=34
                rss=2048
                ;;
            policy_read|policy_edit)
                export RUSTUP_TOOLCHAIN=1.97.1
                require_version 'rustc 1.97.1 (8bab26f4f 2026-07-14)' rustc
                require_version 'cargo-fuzz 0.12.0' cargo fuzz
                build_policy_fuzz_fixtures
                export RUSTC_BOOTSTRAP=1
                features=(--features sdk-fuzz)
                seed=217
                rss=4096
                ;;
            *) echo "Unknown fuzz target: $target" >&2; exit 2 ;;
        esac
        if [[ $seconds == 15 ]]; then
            cargo fuzz check --fuzz-dir fuzz
        fi
        corpus=$(mktemp -d "$scratch/corpus-$target.XXXXXX")
        cp fuzz/corpus/"$target"/* "$corpus/"
        cargo fuzz run "$target" "$corpus" "${features[@]}" --fuzz-dir fuzz -- \
            -max_total_time="$seconds" -max_len=65536 -seed="$seed" -timeout=30 -rss_limit_mb="$rss" \
            -artifact_prefix="$corpus/"
        ;;
    cu)
        [[ $# -le 2 ]]
        sbf_version
        case "${2:-}" in
            '') measure ;;
            compare) compare ;;
            *) echo 'Expected cu or cu compare.' >&2; exit 2 ;;
        esac
        ;;
    *)
        echo 'Usage: reproduce.sh test|docs|install|verus|kani|miri|sbf [smoke]|fuzz [TARGET [smoke]]|cu [compare]' >&2
        exit 2
        ;;
esac
