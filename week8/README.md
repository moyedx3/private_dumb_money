# week8 — AWS 배포 & 시연 가이드 (실 Zcash 결제, TEE 없음)

week7의 `drop`(unlockable content)을 **공개 웹**으로 배포. 청중이 브라우저에서 드롭을 고르고
**실제 Zcash로 결제**하면 복호화 키를 받아 **원본 이미지를 본다.** TEE(Phala/TDX)는 안 씀 —
indexer를 EC2 위 **일반 웹서버**로 돌린다.

```
EC2 1대 (Ubuntu)
 ├─ drop-indexer (Rust) :8080   — dev seed(KMS 대신) + 실 A1 메인넷 스캐너   [systemd]
 └─ Caddy               :443    — buyer SPA 서빙 + /api→:8080, 자동 HTTPS    [systemd]

청중 브라우저 → https://<EC2-IP>.sslip.io
  드롭 Buy → QR(내 e_pub) → Zashi로 실 결제 → A1이 온체인 감지 → dispatch
  → 앱이 폴링 → K_drop → 이미지 복호·표시
```

**왜 TEE 코드 수술이 불필요한가** (자세히: [`../week7/drop/team/mainnet-demo-vs-tee-production.md`](../week7/drop/team/mainnet-demo-vs-tee-production.md)):
indexer는 `A2_DEV_PROVISIONING_SEED_HEX`로 이미 헤드리스 동작(dstack 없이 부팅). `/attest`는 데모서
안 부름. measurement 검증은 creator 앱 코드라 pre-seed로 대체하면 안 돎. → week8 = **배포 + 시드**.

---

## 1. 준비물 (체크리스트)

- [ ] AWS 계정 + EC2 키페어(SSH)
- [ ] **creator 지갑** 드롭 수만큼 (gen-wallet, 아래 §7) — 청중이 여기로 결제, 발표자가 수령
- [ ] **팔 이미지들** (`seed/images/`, 샘플 3장 포함)
- [ ] 테스트용 소액 ZEC (발표자가 본인 시연 결제해볼 것)
- [ ] `SEED_HEX` = 아무 32바이트 hex(64자). indexer·seed 공통.

---

## 2. EC2 인스턴스

- AMI: **Ubuntu 22.04/24.04 LTS**, 타입: **t3.small**(2GB) 이상. (Rust 빌드에 1GB는 빠듯 — swap 주거나 t3.small.)
- 스토리지: 20GB.
- **보안그룹 인바운드: 22(SSH), 80(HTTP), 443(HTTPS).** (80은 Let's Encrypt 챌린지 필수.)
- 퍼블릭 IP 확인 → 이걸로 `SITE_ADDRESS=<IP>.sslip.io` (예: `13.209.1.2.sslip.io`). **도메인 구매 불필요** — sslip.io가 IP로 해석되고 Caddy가 무료 인증서 자동 발급.

```bash
ssh -i <key>.pem ubuntu@<EC2-IP>
```

---

## 3. 의존성 설치

```bash
sudo apt update && sudo apt install -y build-essential pkg-config git curl
# Rust
curl https://sh.rustup.rs -sSf | sh -s -- -y && . "$HOME/.cargo/env"
# Node 22
curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash - && sudo apt install -y nodejs
# Caddy
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update && sudo apt install -y caddy
```

(1GB 인스턴스면 swap: `sudo fallocate -l 2G /swapfile && sudo chmod 600 /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile`)

---

## 4. 코드 가져오기 + 설정

**브랜치 주의** — XP 테마 + live-only buyer는 `feat/lane-b-live-indexer-ui`에 있음:

```bash
cd ~ && git clone <repo-url> private_dumb_money
cd private_dumb_money && git checkout feat/lane-b-live-indexer-ui

# 설정값 (이후 명령에서 씀)
export SEED_HEX=1111111111111111111111111111111111111111111111111111111111111111   # 아무 64 hex
export SITE_ADDRESS=<EC2-IP>.sslip.io
```

---

## 5. 빌드

```bash
# indexer (Rust) — 첫 빌드 몇 분 (zcash 의존성)
cargo build --release --manifest-path week7/drop/indexer/Cargo.toml --bin drop-indexer

# buyer SPA — API를 /api로 (같은 도메인, Caddy가 프록시). Linux에선 /api 그대로 임베드됨.
cd week7/drop/buyer && npm ci && VITE_DROP_INDEXER_URL=/api npm run build && cd ~/private_dumb_money
sudo mkdir -p /srv/buyer && sudo cp -r week7/drop/buyer/dist/* /srv/buyer/
```

---

## 6. 서비스 실행 (systemd)

**indexer:**
```bash
sudo cp week8/drop-indexer.service /etc/systemd/system/drop-indexer.service
sudo sed -i "s/REPLACE_WITH_SEED_HEX/$SEED_HEX/" /etc/systemd/system/drop-indexer.service
mkdir -p ~/drop-bucket
sudo systemctl daemon-reload && sudo systemctl enable --now drop-indexer
sudo systemctl status drop-indexer --no-pager        # active 확인
```

**Caddy** (buyer 서빙 + /api 프록시 + 자동 HTTPS):
```bash
sudo cp week8/Caddyfile /etc/caddy/Caddyfile
sudo sed -i "s/{\$SITE_ADDRESS}/$SITE_ADDRESS/" /etc/caddy/Caddyfile
sudo systemctl restart caddy
sudo journalctl -u caddy -n 30 --no-pager            # 인증서 발급 로그 확인
```
→ 1~2분 뒤 `https://<IP>.sslip.io` 접속 가능. (**HTTPS 필수** — buyer가 WebCrypto 씀.)

---

## 7. 드롭 시드

### (a) creator 지갑 생성 — 드롭당 1개
```bash
cargo run --release --manifest-path week7/drop/indexer/Cargo.toml --example gen-wallet
```
출력의 **UFVK**(`uview1…`)·**DEPOSIT ADDRESS**(`u1…`) 저장. (creator는 받기만 하니 펀딩 불필요.)

### (b) drops.json
```bash
cd week8/seed && cp drops.example.json drops.json && nano drops.json
```
각 드롭에 (a)의 UFVK·deposit_addr, 이미지 경로, 가격 채움:
```json
[{ "drop_id": 1, "title": "고양이", "price_zec": "0.0005",
   "image": "images/cat.png", "creator_ufvk": "uview1…", "deposit_addr": "u1…" }]
```

### (c) 시드 실행
```bash
npm install
SEED_HEX=$SEED_HEX INDEXER_URL=http://localhost:8080 node seed.mjs drops.json
```
```
✅ drop 1 "고양이" — 0.0005 ZEC → u1qpm76q2jaz…  (h_content 1e50ce45…)
3/3 provisioned.
```

---

## 8. 검증

```bash
curl -s https://$SITE_ADDRESS/api/catalog | head        # 드롭 JSON
```
- 브라우저: `https://<IP>.sslip.io` → 카탈로그(XP 창) 뜨는지.
- 발표자 본인이 소액 ZEC로 한 드롭 결제 → 1~2분 뒤 이미지 언락되는지 리허설.

---

## 9. 시연 당일 운영

- **링크 공유**: `https://<IP>.sslip.io` (QR로 띄워두면 청중이 폰으로 바로 접속).
- **결제 감지 모니터**: `sudo journalctl -u drop-indexer -f` — 결제 들어오면
  `A1 scan pass … incoming_notes=1 decoded_memos=1 dispatches=1` / `PUBLISHED dispatch blob` 뜸.
- **청중 흐름**: 드롭 Buy → QR을 **Zashi로 스캔·결제**(memo 기본 A1B64 텍스트) → ~1–2분 후 자동 언락·이미지.
  - 안 열리면 앱 하단 **Manual unlock**에 recovery file 올려 복구.
- **재시드** (indexer 재시작으로 드롭 소실 시): `cd week8/seed && SEED_HEX=$SEED_HEX node seed.mjs drops.json`

> 청중이 실 결제하려면 **Zashi + ZEC** 필요(장벽) → 가격 아주 낮게(0.0005 ZEC), 발표자 시연 + 여유 있는 청중만.

---

## 10. 트러블슈팅

| 증상 | 원인 / 해결 |
|---|---|
| HTTPS 안 됨 / 인증서 실패 | 80·443 보안그룹 열렸나. `journalctl -u caddy`. sslip.io가 IP로 해석되는지(`dig <IP>.sslip.io`). |
| 카탈로그 빔 | indexer 떴나(`systemctl status drop-indexer`). 시드 돌렸나. `curl localhost:8080/catalog`. |
| 브라우저서 복호 안 됨 | HTTP로 접속했나? **HTTPS**여야 WebCrypto 됨. |
| 결제했는데 언락 안 됨 | memo가 **A1B64 텍스트**로 실렸나(raw는 Zashi가 떨굼). `journalctl -u drop-indexer -f`서 `incoming_notes` 뜨나. tx가 채굴됐나(1–2분). |
| provision 400 | drops.json의 UFVK가 유효 `uview1…`인가, deposit_addr가 `u1…`(shielded)인가. |
| indexer 재시작 후 드롭 사라짐 | 카탈로그 in-memory — 재시드(§9). |

---

## 11. 종료 / 비용

- t3.small ≈ 월 $15, 데모 몇 시간이면 소액. **끝나면 인스턴스 중지(stop) 또는 종료(terminate).**
- 서비스만 멈추려면: `sudo systemctl stop drop-indexer caddy`.

---

## 12. 보안 주의 (데모 전용)

- **dev seed** 사용 → 호스트가 provisioning 비밀키를 앎 = K_drop 복호 가능. **TEE 아님, 실서비스 금지.**
- 청중 결제는 **실 메인넷 ZEC** — deposit_addr(발표자 지갑)로 들어옴. 소액 유지.
- `drops.json`엔 creator UFVK(뷰잉키) 들어감 → **커밋 금지**(`.gitignore` 처리됨).
- creator 페이지를 라이브로 띄우려면 `/attest`·QVL 스텁 추가 필요(현재 pre-seed라 생략).
