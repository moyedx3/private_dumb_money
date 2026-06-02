---
title: "Phala Live PoC Deployment Followup"
tags: ["phala", "deployment", "live-poc", "handoff", "dstack", "ghcr", "dockerhub", "tdx", "jsonfix"]
created: 2026-06-02T10:52:48.183Z
updated: 2026-06-02T11:42:56.461Z
sources: ["omx_wiki/phala-enclave-ivk-next-step-handoff.md", "omx_wiki/phala-dstack-attestation.md", "README.md", "docker-compose.phala.yml", "Dockerfile", "tests/test_clean_wallet.py", "clean_wallet/scanner.py", "docker build clean-wallet-mvp2:phala-poc-20260602", "docker push ghcr.io/moyedx3/clean-wallet-mvp2:phala-poc-20260602", "docker push ghcr.io/nogie-dev/clean-wallet-mvp2:phala-poc-20260602", "phala status", "phala deploy --help", "phala cvms --help", "clean_wallet/service.py", "/tmp/cw-live/summary.json", "/tmp/cw-live/cvm-get.json", "/tmp/cw-live/proof.json", "/tmp/cw-live/enclave-attestation.json", "docker buildx build --platform linux/amd64 -t docker.io/nogie/clean-wallet-mvp2:phala-poc-20260602-jsonfix-amd64 --push .", "phala deploy --cvm-id clean-wallet-mvp2 -c docker-compose.phala.yml --wait --json"]
links: ["phala-enclave-ivk-next-step-handoff.md", "phala-dstack-attestation.md"]
category: session-log
confidence: high
schemaVersion: 1
---

# Phala Live PoC Deployment Followup

Follow-up goal recorded on 2026-06-02: get the Clean Wallet PoC running on Phala Cloud so it can be validated live through the HTTP service and real Phala/dstack attestation. The near-term PoC validation target is Phala CVM deployment + live endpoint checks + quote/reportData verification. This does not yet mean production Zcash shielded scanning is complete.

Current repo state: Dockerfile exists and defaults to CLEAN_WALLET_ATTESTOR=phala, installs dstack-sdk, exposes port 8080, and runs python -m clean_wallet.service. docker-compose.phala.yml exists for Phala Cloud and mounts /var/run/dstack.sock, but its image is still a placeholder ghcr.io/YOUR_ORG/clean-wallet-mvp2@sha256:REPLACE_WITH_PINNED_DIGEST. The HTTP service exposes /health, /info, /measurement, /attestation, and /proof. Phala/dstack attestation is the default and fails closed without dstack.sock. Mock attestation is explicit opt-in only via CLEAN_WALLET_ATTESTOR=mock.

Validation evidence captured now: python3 -m unittest discover -s tests passes 16 tests. The encrypted viewing capability contract and enclave-key attestation payload are implemented, but CLEAN_WALLET_ENCLAVE_PUBLIC_KEY/private-key decrypt provider and real Zcash scanning remain unwired, so the encrypted production /proof path intentionally returns ERROR rather than PASS.

Work to progress for live Phala PoC:
1. Build the Docker image from Dockerfile and push it to a registry reachable by Phala Cloud.
2. Replace docker-compose.phala.yml image placeholder with the immutable pushed image digest.
3. Deploy the compose file to Phala Cloud and confirm /var/run/dstack.sock is available in the CVM.
4. Hit live /health, /info, /measurement, and /attestation?purpose=enclave-key&nonce=<nonce>; verify the Phala quote through the verifier path or Phala verification API and confirm reportData binds the expected payload hash.
5. Run a bounded /proof fixture request against the live CVM to validate service wiring and report_hash quote binding.
6. Capture the live CVM endpoint, image digest, measurement/compose policy, and verification transcript in the wiki/README.

Known non-goals for this immediate live PoC unless explicitly promoted: real IVK/FVK/UFVK decrypt, lightwalletd compact-block scanning, blacklist source automation, RTMR3 replay policy automation, and Docker provenance automation.

Open tasks: not none. The next concrete task is image build/push + compose digest replacement, followed by Phala deploy and live endpoint/attestation verification.

---

## Update (2026-06-02T10:57:59.107Z)

# Phala Live PoC Deployment Followup

Update 2026-06-02T10:58Z: progressed the next live-PoC step locally. Docker daemon was initially stopped, then Docker Desktop was started and the image was built successfully as `clean-wallet-mvp2:phala-poc-20260602`.

Local image evidence:
- Local image id / manifest digest: `sha256:d6f5630b514bba29c283eda49fb830e4ca3d01f3e5dd8152cec3c8f3085ac27f`
- Local repo digest shown by Docker: `clean-wallet-mvp2@sha256:d6f5630b514bba29c283eda49fb830e4ca3d01f3e5dd8152cec3c8f3085ac27f`
- Image size: 53,499,177 bytes

Smoke verification evidence:
- `python3 -m unittest discover -s tests` passed: 16 tests OK.
- Running the image in default Phala mode without `/var/run/dstack.sock` returned `/health` HTTP 503 with error `Unix socket file /var/run/dstack.sock does not exist`, confirming fail-closed behavior outside the CVM.
- Running the image with `CLEAN_WALLET_ATTESTOR=mock` returned `/health ok=true`.
- Mock fixture `/proof` returned HTTP 200, `result=PASS`, `report_hash_present=True`, `signature_or_quote.mode=mock-tee-v0`.
- Encrypted viewing capability `/proof` returned HTTP 200, `result=ERROR`, with error that the production Zcash scanner/decrypt path is not wired. This confirms real scanning is not yet implemented and the image does not falsely mint PASS for encrypted IVK production requests.

Push/deploy blocker encountered:
- Tagged image as `ghcr.io/moyedx3/clean-wallet-mvp2:phala-poc-20260602` based on git remote owner.
- `docker push ghcr.io/moyedx3/clean-wallet-mvp2:phala-poc-20260602` failed with registry error `denied`.
- Therefore `docker-compose.phala.yml` must not yet be replaced with a pushed immutable GHCR digest, and Phala Cloud deploy cannot proceed from this environment until registry auth/permission is available or another reachable registry target is provided.

Scanner status answer:
- Fixture scanning is included and operational in the built image.
- Production real Zcash scanning is not included. `ZcashViewingKeyScanner.scan()` still intentionally returns ERROR until a TEE-local decrypt provider plus lightwalletd/full-node/compact-bundle scanner is implemented.

Remaining live-PoC tasks:
1. Authenticate Docker to a registry with push permission, or choose a reachable registry/image name.
2. Push `ghcr.io/moyedx3/clean-wallet-mvp2:phala-poc-20260602` or equivalent.
3. Resolve the pushed immutable digest with `docker buildx imagetools inspect <image>:<tag>` or registry output.
4. Replace `docker-compose.phala.yml` image placeholder with `<registry>/<image>@sha256:<pushed_digest>`.
5. Deploy to Phala Cloud.
6. Verify live `/health`, `/info`, `/measurement`, `/attestation?purpose=enclave-key&nonce=<nonce>`, and a fixture `/proof` request against the CVM.
7. Capture live endpoint, image digest, measurement/compose policy, and quote verification transcript.

Open production-scanning tasks remain separate: implement TEE-local viewing capability decrypt/key provider, implement concrete Zcash compact block scanner, and verify real IVK/FVK/UFVK trial-decrypt behavior without logging or returning sensitive wallet data.

---

## Update (2026-06-02T11:14:17.845Z)

# Phala Live PoC Deployment Followup

Update 2026-06-02T11:04Z: user reported GHCR Docker login complete as `nogie-dev`, target image `ghcr.io/moyedx3/clean-wallet-mvp2`, intended public package. Push was retried.

Push attempts:
- `docker push ghcr.io/moyedx3/clean-wallet-mvp2:phala-poc-20260602` failed: `permission_denied: The token provided does not match expected scopes.`
- Fallback attempt to the login user's namespace, `docker push ghcr.io/nogie-dev/clean-wallet-mvp2:phala-poc-20260602`, failed with the same `permission_denied: The token provided does not match expected scopes.`

Phala readiness:
- `phala` CLI is installed: `v1.1.19+d2300dd`.
- `phala status` succeeds: Integrated API `https://cloud-api.phala.com/api/v1`, API Version `2026-01-21`, logged in as `nogie-dev`, workspace/profile `zcashhh`.
- `phala deploy --help` confirms deployment command supports `-c/--compose`, `-n/--name`, and `--wait`.
- `phala cvms --help` confirms `attestation`, `get`, and management commands are available.

Current blocker:
- Phala deploy cannot proceed until a registry image exists that Phala Cloud can pull. The local image is built and smoke-tested, but no pushed immutable digest exists because GHCR token/package permission is insufficient.

Needed from user to continue:
1. Re-login Docker to GHCR with a GitHub PAT that has `write:packages` and `read:packages`; if the package is under org/user `moyedx3`, ensure `nogie-dev` has package write permission and SSO authorization if applicable. If the repository/package is private, include `repo` scope as needed.
2. Or provide a different public registry target and credentials (Docker Hub, GHCR under a namespace with write permission, etc.).

Once push succeeds, next agent steps are deterministic: inspect pushed digest, replace compose image with `<registry>/<image>@sha256:<digest>`, run `phala deploy -c docker-compose.phala.yml -n clean-wallet-mvp2 --wait`, then collect `phala cvms get ... --json`, `phala logs --cvm-id ...`, `phala cvms attestation ... --json`, and live HTTP endpoint checks.

---

## Update (2026-06-02T11:42:56.461Z)

Update 2026-06-02T11:45Z: Docker Hub fallback succeeded and Phala live PoC is deployed/verified after one live bug fix.

Image/deploy state:
- Final Phala image is digest-pinned in docker-compose.phala.yml as `docker.io/nogie/clean-wallet-mvp2@sha256:1ef2fbb1745a93954d832d7322c9e746214f624a7c5f403559ba65b82414e5e4`.
- buildx inspect confirms that index digest includes linux/amd64 manifest `sha256:0d3c428e3bbc00a8df64a2504042de38f44936f263f614da70e0dc2b453048e5`; the earlier `sha256:9729e...` deployment was superseded because `/info` and `/measurement` exposed a JSON serialization bug.
- Existing Phala CVM `clean-wallet-mvp2` was updated with `phala deploy --cvm-id clean-wallet-mvp2 -c docker-compose.phala.yml --wait --json`.
- CVM status after update: `running`; app_id `1cc48311ccb81c6982687095b840021bce576eb9`; vm_uuid `7738ed10-fec9-4d95-a6c8-fc3da9928ad1`; node `prod5`; region `US-WEST-1`; compose_hash `fbc0db6598ff30fcfe8147b239f3b3116c16c1b99d10c7c115c5472b35ca8820`.
- Live endpoint: `https://1cc48311ccb81c6982687095b840021bce576eb9-8080.dstack-pha-prod5.phala.network`.
- Phala logs show service startup: `clean-wallet TEE service listening on 0.0.0.0:8080 attestor=phala`.

Live bug fixed during verification:
- Symptom: `/health` returned 200 but `/info` and `/measurement` returned `curl: (52) Empty reply from server` on the initial live deployment.
- Cause: Phala/dstack SDK objects and bytes were not JSON-serializable at the HTTP boundary; `_json_response()` attempted raw `json.dumps(payload)`.
- Fix: added `clean_wallet.service._json_safe()` to recursively convert dict/list/tuple/set, bytes to hex, `to_dict()` SDK objects, and public `__dict__` values before JSON rendering; `_public_dstack_info()` now uses the same sanitizer.
- Regression test added: `ServiceContractTests.test_json_safe_converts_sdk_style_objects`.
- Unit verification: `python3 -m unittest discover -s tests` passes 17 tests.

Live HTTP verification evidence on the updated CVM:
- `GET /health`: HTTP 200, `ok=true`, `attestor=phala`.
- `GET /info`: HTTP 200, 47,524 bytes, app_id `1cc48311ccb81c6982687095b840021bce576eb9`, instance_id `2a93ab1817d8493d9fc129e3d822ac626931a3f6`.
- `GET /measurement`: HTTP 200, 47,524 bytes, measurement `5cd00493c949a31dcac1d9846d64fa600bf2b175f4210aea7d7f82bee1f45b00ff1b7c01db66ef1ced1fec6c6a370929`, encryption_key.status `unconfigured` as expected until real enclave public-key provisioning is wired.
- `GET /attestation?purpose=enclave-key&nonce=clean-wallet-live-verify-20260602-jsonfix`: HTTP 200, payload_hash `b895ab61d6f8670bb66920640e32f0ea288d03a17de4a42de00d7e5d7c66ee08`, quote mode `phala-dstack-tdx-v0`.
- `POST /proof` with legacy PASS fixture: HTTP 200, result `PASS`, report_hash `08b2a24cfaaeb8ca9996a8b6409233b1cdcc048cf4c1d8d962a0878d6547f861`, quote mode `phala-dstack-tdx-v0`, measurement matches `/measurement`.
- Raw TDX quote reportData check: for both the proof quote and enclave-key quote, the expected 32-byte hash is present at `quote[48+520:48+520+64]` and the remaining 32 bytes are zero padding. The proof quote binds `08b2a24c...6547f861`; the enclave-key quote binds `b895ab61...7c66ee08`.

Verification caveat:
- The repo’s `PhalaDstackVerifier` default cloud endpoint `https://cloud-api.phala.com/api/v1/attestations/verify` returned HTTP 403 both unauthenticated and with the local Phala CLI token (`Bearer` and `X-API-Key`). Therefore cryptographic verification through that public API remains an integration gap. For this handoff, quote binding is verified by dstack quote metadata plus raw TDX quote reportData position; full external DCAP/Phala verification should be wired once the correct Phala verify endpoint/auth contract is known.

Current non-goals still unchanged:
- Real IVK/FVK/UFVK decrypt/key provider is not wired; `/measurement` correctly reports encryption key unconfigured.
- Real Zcash compact-block/lightwalletd scanning is not implemented; fixture proof works, encrypted production scan path still intentionally returns ERROR rather than minting PASS.
- Production readiness still requires external quote verification path, key provisioning, real scanner, and stricter measurement/compose allowlist policy.
