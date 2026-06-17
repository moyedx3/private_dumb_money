# Security and Privacy Notes

## 민감한 입력

UFVK/FVK/UIVK/IVK는 spending key가 아니지만 지갑 활동을 볼 수 있는 민감한 viewing material입니다.

금지:

- GitHub issue/comment에 붙여넣기
- shell argument로 넘기기
- 로그에 출력하기
- `/proof` plaintext field로 보내기
- generated artifact를 무심코 커밋하기

권장:

- `scripts/encrypt_viewing_capability.py`의 hidden prompt 사용
- service `/attestation` 확인 후 암호화 제출
- local artifact는 `.gitignore`된 `artifacts/*.json`에 저장

## Production 기본값

HTTP service/container 기본값은 Phala mode입니다.

- fixture proof는 기본적으로 거부됩니다.
- plaintext viewing key field는 거부됩니다.
- encrypted viewing capability + lightwalletd chain source만 production path입니다.
- scanner 오류는 `ERROR`가 되며 `PASS`로 승격되지 않습니다.

## Attestation 확인 포인트

사용자/검증자는 최소한 다음을 확인해야 합니다.

1. `/attestation?purpose=enclave-key&nonce=<nonce>` 응답의 nonce가 요청과 일치
2. encryption key descriptor가 `configured`
3. `scheme == x25519-chacha20poly1305-v0`
4. quote `report_data`가 attestation payload hash와 일치
5. proof report의 `report_hash`가 quote `report_data`와 일치
6. `measurement`가 허용된 배포 measurement와 일치
7. `blacklist_manifest_hash`가 검증자가 의도한 manifest와 일치

## PASS의 한계

`PASS`는 다음 범위 안에서만 의미가 있습니다.

- network
- pool
- block range
- submitted viewing scope/capability
- blacklist manifest/root
- enclave measurement policy

따라서 PASS를 “전역적인 clean wallet 증명”으로 표시하면 안 됩니다.

## Local artifact 주의

`scripts/getArtifact.py`는 다음 민감 정보를 파일로 만들 수 있습니다.

- owned commitments
- default unified address
- block summary

이 파일은 테스트 데이터 생성용입니다. 외부 공유/커밋 전에 내용을 확인하세요.
