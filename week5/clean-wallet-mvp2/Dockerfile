FROM rust:1.88-slim AS scanner-builder

WORKDIR /scanner
COPY zcash_scanner/Cargo.toml zcash_scanner/Cargo.lock* ./
COPY zcash_scanner/src ./src
RUN cargo build --release

FROM python:3.12-slim

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    CLEAN_WALLET_HOST=0.0.0.0 \
    CLEAN_WALLET_PORT=8080 \
    CLEAN_WALLET_ATTESTOR=phala \
    CLEAN_WALLET_ZCASH_SCANNER_CMD=/app/bin/clean-wallet-zcash-scanner

WORKDIR /app
COPY clean_wallet ./clean_wallet
COPY fixtures ./fixtures
COPY README.md ./README.md
COPY --from=scanner-builder /scanner/target/release/clean-wallet-zcash-scanner /app/bin/clean-wallet-zcash-scanner

# Phala/dstack runtime dependency. The service defaults to
# CLEAN_WALLET_ATTESTOR=phala and will fail closed if dstack-sdk or
# /var/run/dstack.sock is unavailable in production CVM deployment.
RUN pip install --no-cache-dir dstack-sdk grpcio grpcio-tools cryptography

EXPOSE 8080
CMD ["python", "-m", "clean_wallet.service"]
