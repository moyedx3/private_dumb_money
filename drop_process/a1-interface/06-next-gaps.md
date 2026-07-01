# A1 Interface — Known Gaps

## 11. Known gaps / 다음 구현 후보

## 11.1 즉시 필요한 것

```text
1. team/interfaces.md I1에 A1B64 text fallback 문서화
2. PublicCatalogEntry 추가
3. RegisterCreatorDropRequest에 title/price_zec/h_content/deposit_addr 추가
4. list_public_catalog() 추가
5. list_dispatch_keys() 또는 list_recent_dispatches() 추가
```

## 11.2 운영화에 필요한 것

```text
1. HTTP/enclave API adapter
2. TEE sealing key 기반 StateCipher
3. encrypted catalog DB store
4. real bucket adapter
5. production polling loop
6. confirmation depth / reorg handling
7. dispatch save와 state save의 원자성 정책
```

---
