# A1 ↔ Lane A2 Enclave Interface

## A1이 A2와 연결해야 하는 민감 데이터

- creator `UFVK/IVK`
- `K_drop`
- creator/drop 등록값
- scan cursor / seen txid state
- dispatch 생성 로직

## 최종 보안 목표

운영자도 다음 plaintext를 볼 수 없어야 한다.

- UFVK/IVK
- K_drop
- scan state
- sealed creator/drop DB

## 현재 A1 구현 상태

- `StateCipher` trait로 encrypted state 경계가 있음
- dev용 `SecretboxStateCipher`가 있음
- `EncryptedFileScanState`로 cursor/replay state 암호화 저장 가능
- production에서는 이 trait 구현체를 enclave sealing key 기반으로 교체해야 함

## A2에 필요한 interface

```text
attest enclave
→ sealed provision endpoint로 creator secret 등록
→ enclave 내부에서 UFVK/K_drop 복호화
→ scan 및 dispatch 생성
→ 외부에는 encrypted state와 dispatch blob만 반환
```
