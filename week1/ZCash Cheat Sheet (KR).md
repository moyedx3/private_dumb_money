# Zcash 치트시트

## Zcash란?

Zcash는 사실상 암호화된 비트코인이다. 2,100만 개 하드캡, 반감기 스케줄, UTXO 모델, Proof of Work 합의 방식 전부 동일하지만 암호화가 추가되어 있다. 비트코인은 투명한 돈, Zcash는 프라이빗한 돈이다.

Zcash에는 **transparent pool** (비트코인과 동일하게 작동, 주소가 `t`로 시작)과 **shielded pool** (완전 암호화, 주소가 `z`로 시작)이 있다. transparent pool은 호환성, 선택권, 감사 가능성을 위해 존재한다. 두 풀은 완전히 독립적인 시스템으로 서로 영향을 주지 않는다. ZEC의 99%가 transparent에 있더라도, shielded 1%의 프라이버시는 오직 shielded pool 자체에 의해서만 결정된다.

**Shielded pool 세대:**
- **Sprout (2016):** 1세대. 프라이빗 크립토가 가능하다는 것을 증명. Trusted setup 필요. 느림 (증명 생성 40초). 현재 deprecated.
- **Sapling (2018):** 모바일에서 실용적. Viewing key와 diversified address 도입. 여전히 trusted setup 필요 (Powers of Tau 세레모니).
- **Orchard (2022):** Halo 2 proving system 기반. Trusted setup 없음, toxic waste 없음, 신뢰 가정 없음. Zcash가 원래 목표했던 풀.

**Zcash가 답하는 근본적 질문:** 네트워크가 트랜잭션을 볼 수 없는데 어떻게 유효성을 검증할 수 있는가? 송신자가 zk-SNARK(암호학적 증명)를 제공하여 기저 정보를 드러내지 않으면서 유효성을 입증한다.

---

## 핵심 기술 개념

### Notes (암호화된 UTXO)
- **Note**는 특정 양의 ZEC를 나타내는 암호화된 객체
- Shielded ZEC를 수신하면 생성됨
- 사용(spend)하면 소멸되고, 수신자용 + 잔돈용 새 note가 생성됨
- 소유자(와 viewing key를 공유받은 사람)만 내용을 볼 수 있음

### Commitments
- Commitment = note 필드들의 해시값 (`addr`, `v`, `rho`, `psi`, `rcm`)
- 모든 commitment는 전체 note commitment를 담고 있는 글로벌 **Merkle tree**에 추가됨
- 사용 시, zk-SNARK 내에서 자신이 알고 있는 commitment과 현재 root까지의 유효한 Merkle path를 증명하되, 어떤 commitment인지는 드러내지 않음
- Commitment는 절대 삭제되지 않음 (append-only tree). 사용된 note도 트리에 영구 보존
- 익명성 집합(anonymity set) = 지금까지 생성된 모든 shielded note (수백만 개)

### Nullifiers
- 실제 사용하는 commitment를 가리킬 수 없음 (그러면 해당 note가 모든 미래 트랜잭션에 연결되어 프라이버시가 깨짐)
- 대신 **nullifier** 사용: `nullifier = Hash(nk, rho, psi)`
  - `nk`: nullifier deriving key (비밀, 본인만 보유)
  - `rho`, `psi`: note 자체에서 가져온 값
- 사용 시, nullifier를 공개(publish)
- 네트워크는 지금까지 공개된 모든 nullifier의 **nullifier set**을 유지
- nullifier가 이미 set에 있으면 트랜잭션 거부 (이중 지불 방지)
- **결정적(Deterministic):** 각 note는 정확히 하나의 nullifier만 생성. 같은 note를 두 번 쓰면 = 같은 nullifier = 거부
- **연결 불가(Unlinkable):** private key 없이는 nullifier에서 commitment를 역추적할 수 없음

### Key 계층 구조
```
spending key (sk)           -- 마스터 비밀키, 모든 것이 가능
  +-- full viewing key (fvk)    -- 지갑 활동 전체 조회, 사용 불가
  |     +-- incoming viewing key (ivk) -- 수신 내역만 확인
  |     +-- outgoing viewing key (ovk) -- 송신 내역 확인
  |     +-- addresses (diversifier 기반) -- 수십억 개의 연결 불가능한 주소 생성
  +-- nullifier deriving key (nk)  -- 사용 시 nullifier 계산용
```

---

## 트랜잭션 생명주기

1. **지갑 동기화:** 블록체인을 스캔하며 incoming viewing key로 모든 shielded output의 복호화를 시도. 성공한 것들을 저장.
2. **Merkle path 조회:** 사용할 note의 Merkle path를 가져옴. zk-SNARK 내에서 commitment가 트리에 존재함을 증명하되 실제 commitment이나 path는 드러내지 않음. **Anchor** (path 조회 시점의 Merkle root)를 기록.
3. **Nullifier 계산:** 사용할 각 note에 대해 `nullifier = Hash(nk, rho, psi)` 계산.
4. **Output note 생성:** note 구성요소 생성 (`rho`, `psi`, `rcm`), commitment 계산, note 암호화 (수신자 주소로 `encCiphertext`, 송신자 OVK로 `outCiphertext`).
5. **zk-SNARK 증명 생성:**
   - Input note가 존재함 (유효한 Merkle path)
   - 송신자가 input을 통제함 (spending key 보유)
   - Nullifier가 실제 note에서 올바르게 도출됨
   - Input 합계 = Output 합계 + 수수료
   - Output commitment가 올바르게 구성됨
6. **트랜잭션 조립:** Orchard "action"으로 번들링 (각 action은 정확히 1개의 spend + 1개의 output을 쌍으로 묶음. 더미로 빈 자리를 채워 트랜잭션 구조를 숨김). Anchor, nullifier, commitment, 암호화된 payload, 증명 (~1.5 KB), binding signature 포함.
7. **브로드캐스트:** 노드가 검증: 증명 검증, anchor 확인, nullifier 확인 (set에 없는지), 구조적 유효성. 유효하면 mempool에 추가.
8. **블록 포함:** 마이너가 트랜잭션 선택, 블록 채굴 (PoW, Equihash). Commitment tree 성장 (새 leaf), nullifier set 확장, 블록 보상 지급. 블록 시간 약 75초.
9. **수신자 탐지:** 수신자 지갑이 모든 shielded output을 trial-decrypt. 복호화 성공 시 note 데이터 확인. 온체인 commitment과 일치하는지 검증 후, 사용 가능한 note로 저장.

---

## Zcash vs. 다른 프로젝트들

| | Zcash | Monero | Tornado Cash / Mixers |
|---|---|---|---|
| **방식** | 암호화 (zk-SNARKs) | 난독화 (ring signatures, 16개 decoy) | 믹싱 (공유 풀에 입출금) |
| **익명성 집합** | 생성된 모든 shielded note (수백만) | 트랜잭션당 16명의 가능한 송신자 | 고정 denomination 풀 |
| **시간이 지나면 약해지는가?** | 아니오. 암호학적, 확률적이 아님 | 예. 분석으로 decoy 제거 가능 | 예. 타이밍/금액 상관관계 공격 |
| **풀 내 기능** | 완전한 화폐 시스템 (송금, 수신, 보유, 잔돈) | 완전 (모든 tx가 shielded) | 없음. 자금 사용하려면 인출 필요 |
| **거래소 가용성** | Coinbase, Gemini 등 | 대부분 주요 거래소에서 상장폐지 | 해당 없음 (제재 대상) |

**Aztec / Private L2s:** 다른 문제를 해결 (프라이빗 프로그래밍 / 암호화된 DeFi). Zcash는 돈, 즉 프라이빗한 가치 저장 수단이다. 가치 저장 수단에는 린디 효과 (Zcash는 약 9년), 밈적 강점 ("암호화된 비트코인"), 프라이버시를 타협 불가 원칙으로 여기는 커뮤니티가 필요하다.

---

## 생태계와 경제

**4대 주요 조직:**
- **ECC -> ZODL:** 프로토콜 개발, Zashi 지갑
- **Zcash Foundation:** Zebra 노드 (Rust 독립 구현체), 그랜트
- **Shielded Labs:** 연구, 스위스 기반
- **Tachyon:** 스케일링, Halo 2 설계자 Sean Bowe가 이끔

**펀딩 역사:**
- **Founders' Reward (2016-2020):** 블록 보상의 20%가 설립자/투자자/직원에게
- **Dev Fund (2020-2024):** 20% 분배 (ECC 7%, Foundation 5%, 커뮤니티 그랜트 8%)
- **Extended Dev Fund (2024-2025):** 미래 거버넌스를 위한 lockbox 포함

**Turnstiles:** shielded pool 안에서는 코인 총량을 셀 수 없으므로, 각 풀이 ZEC 유입량 vs. 유출량을 추적. 유입량보다 더 많이 인출할 수 없음. 위조 시도를 인출 시점에 탐지 (예방이 아닌 탐지).

**Network Sustainability Mechanism (NSM):**
- 1 ZEC를 소각하면 향후 4년에 걸쳐 0.5 ZEC가 추가 발행 (반감기 스케줄에 맞춘 지수적 감소)
- 단기적으로 유통량 감소, 장기적으로 마이너 인센티브 유지, 2,100만 캡 초과 없음
- ZIP 233 (자발적 소각), ZIP 234 (부드러운 발행 곡선), ZIP 235 (트랜잭션 수수료의 60% 소각)

**Zcash x Near Intents:** shielded ZEC로 결제하면 수신자가 다른 체인에서 코인을 받음. Shielded pool을 떠나지 않고 브릿지 가능.

---

## 앞으로의 로드맵

### Project Tachyon
세 가지 스케일링 병목 해결:
1. **이중 지불 방지:** 현재 모든 검증 노드가 전체 nullifier set을 영구 저장해야 함. Tachyon은 "oblivious synchronization"을 사용. 서비스가 당신을 대신해 증명을 구성하되, 어떤 nullifier를 사용하는지 알 수 없음. 검증자는 더 이상 전체 nullifier 이력이 필요 없음.
2. **블록체인 스캐닝:** 모든 트랜잭션을 trial-decrypt하는 방식 대신 더 효율적인 결제 프로토콜로 대체. In-band secret distribution도 제거 ("harvest now, decrypt later" 양자 위협 해결).
3. **트랜잭션 크기:** 재귀적 증명(recursive proofs)으로 크기와 검증 시간을 비트코인 수준까지 축소.

### 양자 저항성 (Quantum Resistance)
- **이미 보호되는 것:** 온체인 익명성. Nullifier는 대칭 암호화 사용 (양자 안전). Commitment는 perfectly hiding. 대칭 암호화는 포스트 양자 키 사이즈.
- **프라이버시 위협:** 적이 오늘 암호화된 tx 데이터를 수집하고 나중에 복호화 ("harvest now, decrypt later"). Tachyon이 in-band secret distribution을 완전 제거하여 해결.
- **건전성(Soundness) 위협:** 타원곡선 암호가 깨질 수 있음. 프로토콜의 모듈식 설계로 취약한 요소만 교체 가능. 양자 복구(quantum recoverability) 메커니즘 개발 중 (2026), 사용자가 양자 적으로부터 안전하게 자금 복구 가능.
