# MA-4: Pin Rust toolchain and create debt-gates script

BUILDPLAN M1 / WS-10 (BUILDPLAN id MA-04). Add rust-toolchain.toml pinning 1.92; CI derives toolchain from the file; create scripts/debt-gates.sh (owned here per SPEC R-5; later tickets append gates) including the Dockerfile-base-image-matches-pin check. AC: AC-23. Serialize: ci.yml. Size S.
