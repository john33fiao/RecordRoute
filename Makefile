.PHONY: run build test clippy fmt fmt-check clean

run:
\tcargo run

build:
\tcargo build

test:
\tcargo test

clippy:
\tcargo clippy --all-targets --all-features

fmt:
\tcargo fmt

fmt-check:
\tcargo fmt --all -- --check

clean:
\tcargo clean
