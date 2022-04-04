#!/usr/bin/env bash

bold=$(tput bold)
normal=$(tput sgr0)

export CARGO_INCREMENTAL=0
export RUSTFLAGS="-Zprofile -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Coverflow-checks=off -Zpanic_abort_tests -Cpanic=abort"
export RUSTDOCFLAGS="-Cpanic=abort"

function check_all() {
    if ! command -v rustup &> /dev/null
    then
        echo -n "${bold}error: ${normal}"
        echo "rustup could not be found!"
        echo
        echo "Read the Docs:" 
        echo "    https://rustup.rs/"
        exit 1
    fi
    
    if ! command -v cargo &> /dev/null
    then
        echo -n "${bold}error: ${normal}"
        echo "cargo could not be found!"
        echo
        echo "Read the Docs:" 
        echo "    https://doc.rust-lang.org/cargo/"
        exit 1
    fi
    
    if ! command -v grcov &> /dev/null
    then
        echo -n "${bold}error: ${normal}"
        echo "grcov could not be found!"
        echo
        echo "Read the Docs:" 
        echo "    https://github.com/mozilla/grcov"
        exit 1
    fi
    
    if ! (rustup toolchain list | grep nightly) &> /dev/null;
    then
        echo -n "${bold}error: ${normal}"
        echo "Rust nightly channel could not be found!"
        echo
        echo "Run and try again:" 
        echo "    rustup toolchain install nightly"
        exit 1
    fi
}

check_all

cargo clean && cargo +nightly test -- --test-threads=1
grcov . -s . --binary-path ./target/debug/ -t html --ignore-not-existing -o ./target/debug/coverage/ && open ./target/debug/coverage/index.html
