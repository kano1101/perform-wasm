(
    cd `dirname $0`
    cargo test -- --nocapture &&
        wasm-pack test --headless --firefox &&
        (
            cd ./manual_test/
            ./test.sh
        )
)
