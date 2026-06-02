//! gen-testnet-wallet — Zcash testnet 지갑 생성 도구.
//!
//! 새 BIP-39 24단어 mnemonic 을 생성하고 ZIP-32 derivation 으로 testnet
//! UnifiedSpendingKey / UnifiedFullViewingKey 와 첫 수신 주소들을 출력한다.
//!
//! 사용:
//!   cargo run --release --bin gen-testnet-wallet
//!       → 새 mnemonic 생성.
//!   cargo run --release --bin gen-testnet-wallet -- --mnemonic "word1 word2 ..."
//!       → 기존 mnemonic 으로부터 동일 키 재현.
//!   cargo run --release --bin gen-testnet-wallet -- --seed-hex deadbeef...
//!       → raw seed bytes (>= 32) 로부터.
//!
//! 출력:
//!   - mnemonic (BIP-39 24단어) — Zashi/Ywallet 같은 외부 지갑에 import 가능.
//!     ZEC 송금하려면 외부 지갑 필요 (이 도구는 키 생성만).
//!   - seed hex (검증용 — 보관 안 해도 됨, mnemonic 이 진짜 비밀).
//!   - UnifiedFullViewingKey (uviewtest1...) — submit-ufvk 에 줘서 스캔.
//!   - 첫 transparent receive 주소 (tmXX..., faucet 으로 ZEC 받기용).
//!   - 첫 unified receive 주소 (utest...).
//!
//! 흐름 (e2e 테스트용):
//!   1) 이 도구로 지갑 생성 → mnemonic 보관.
//!   2) mnemonic 을 Zashi testnet (또는 호환 지갑) 에 import.
//!   3) 출력된 transparent 주소로 testnet ZEC faucet 에서 받기.
//!   4) Zashi 에서 ZEC 를 다른 testnet 주소로 송금 (= outgoing tx).
//!   5) 우리 attested scanner 에 UFVK 전달 → outgoing 수취인 검출 검증.

use std::io::Write;

use anyhow::{anyhow, bail, Context, Result};
use bip39::{Language, Mnemonic};
use rand::RngCore;
use zcash_keys::address::Address;
use zcash_keys::encoding::encode_extended_full_viewing_key;
use zcash_keys::keys::{ReceiverRequirement, UnifiedAddressRequest, UnifiedSpendingKey};
use zcash_protocol::consensus::{Network, NetworkConstants};
use zcash_transparent::keys::{IncomingViewingKey, NonHardenedChildIndex};
use zip32::AccountId;

struct Args {
    mnemonic_in: Option<String>,
    seed_hex: Option<String>,
    network: Network,
}

fn parse_args() -> Result<Args> {
    let argv: Vec<String> = std::env::args().collect();
    let mut mnemonic_in: Option<String> = None;
    let mut seed_hex: Option<String> = None;
    let mut network: Network = Network::TestNetwork;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--mnemonic" => {
                mnemonic_in = Some(
                    argv.get(i + 1)
                        .ok_or_else(|| anyhow!("--mnemonic 뒤에 단어들 (큰따옴표로 묶어서)"))?
                        .clone(),
                );
                i += 2;
            }
            "--seed-hex" => {
                seed_hex = Some(
                    argv.get(i + 1)
                        .ok_or_else(|| anyhow!("--seed-hex 뒤에 hex"))?
                        .clone(),
                );
                i += 2;
            }
            "--network" => {
                let v = argv
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--network 뒤에 main 또는 test"))?;
                network = match v.as_str() {
                    "main" | "mainnet" => Network::MainNetwork,
                    "test" | "testnet" => Network::TestNetwork,
                    other => bail!("--network 는 main|test 만 — 받은: {}", other),
                };
                i += 2;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => bail!("알 수 없는 옵션: {}", other),
        }
    }
    if mnemonic_in.is_some() && seed_hex.is_some() {
        bail!("--mnemonic 와 --seed-hex 는 동시에 사용 불가");
    }
    Ok(Args {
        mnemonic_in,
        seed_hex,
        network,
    })
}

fn print_usage() {
    eprintln!(
        "gen-testnet-wallet — Zcash 지갑 생성 (testnet 기본, mainnet 옵션)

사용법:
  gen-testnet-wallet                                (testnet 새 24단어 mnemonic)
  gen-testnet-wallet --network main                 (mainnet 새 mnemonic)
  gen-testnet-wallet --mnemonic \"word1 word2 ...\"  (기존 mnemonic 복원)
  gen-testnet-wallet --network main --mnemonic \"...\"   (같은 mnemonic mainnet derive)
  gen-testnet-wallet --seed-hex <hex>               (raw seed >= 64 chars)

출력:
  mnemonic, seed hex, UFVK, t-addr, unified addr

⚠️  mnemonic 은 비밀. 안전하게 보관.
⚠️  mainnet 은 진짜 자금이 묶임. 외부 지갑에 import 한 mnemonic 을 어디 노출하지 말 것."
    );
}

fn main() -> Result<()> {
    let args = parse_args().context("argument parsing")?;
    let network = args.network;
    let net_label = match network {
        Network::MainNetwork => "mainnet",
        Network::TestNetwork => "testnet",
    };

    // 1) seed 결정.
    let (mnemonic_str, seed_bytes): (Option<String>, Vec<u8>) = if let Some(m) = &args.mnemonic_in {
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, m.trim())
            .map_err(|e| anyhow!("mnemonic 파싱 실패: {}", e))?;
        let seed = mnemonic.to_seed(""); // BIP-39 passphrase 비움
        (Some(mnemonic.to_string()), seed.to_vec())
    } else if let Some(h) = &args.seed_hex {
        let seed = hex::decode(h.trim()).context("--seed-hex 디코드")?;
        if seed.len() < 32 {
            bail!(
                "seed 길이 부족 — 32바이트 이상이어야 함 (받은 {})",
                seed.len()
            );
        }
        (None, seed)
    } else {
        // 새 24단어 mnemonic 생성: 256비트 entropy → bip39::Mnemonic::from_entropy_in.
        let mut entropy = [0u8; 32];
        rand::rng().fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
            .map_err(|e| anyhow!("mnemonic 생성 실패: {}", e))?;
        let seed = mnemonic.to_seed("");
        (Some(mnemonic.to_string()), seed.to_vec())
    };

    // 2) UnifiedSpendingKey + UFVK derive.
    let account = AccountId::ZERO;
    let usk = UnifiedSpendingKey::from_seed(&network, &seed_bytes, account)
        .map_err(|e| anyhow!("UnifiedSpendingKey::from_seed 실패: {:?}", e))?;
    let ufvk = usk.to_unified_full_viewing_key();
    let ufvk_str = ufvk.encode(&network);

    // 2b) Sapling Extended FVK 추출 — 구식 explorer / lightclient 가 UFVK 못 받고
    // sapling-only EFVK 만 받는 경우용. USK 의 sapling extended spending key 에서 EFVK 변환.
    let sapling_efvk = usk.sapling().to_extended_full_viewing_key();
    let sapling_efvk_str = encode_extended_full_viewing_key(
        network.hrp_sapling_extended_full_viewing_key(),
        &sapling_efvk,
    );

    // 3) 첫 transparent receive 주소 (external scope, index 0).
    let t_addr_obj = ufvk
        .transparent()
        .ok_or_else(|| anyhow!("UFVK 에 transparent component 가 없음"))?
        .derive_external_ivk()
        .map_err(|e| anyhow!("derive external IVK: {}", e))?
        .derive_address(NonHardenedChildIndex::from_index(0).expect("index 0 valid"))
        .map_err(|e| anyhow!("derive transparent addr index 0: {}", e))?;
    let t_addr_str = Address::Transparent(t_addr_obj).encode(&network);

    // 4) 첫 unified address 두 가지 + 그 안에 박혀있는 transparent receiver.
    //   (a) ALLOW_ALL: orchard + sapling + transparent 모두 허용 (우리 기본).
    //   (b) Vizor 호환: Require Orchard + Require Sapling + Omit Transparent.
    //   동일 UFVK 에서 derive 되더라도 receiver 정책이 다르면 인코딩이 완전히 다르다 — 다른 지갑이 아니라 같은 지갑의
    //   다른 UA 표현일 뿐.
    //   추가로 (a) UA 의 transparent receiver 도 별도 추출 — Vizor 의 'transparent receive' 화면이 보여주는
    //   주소가 standalone BIP-44 index 0 (transparent_address_index_0) 이 아니라 UA 의 transparent receiver
    //   (diversifier index 와 묶인) 일 수 있어서. 그것까지 보여줘야 비교 가능.
    let (unified_addr_all, all_div_idx) = ufvk
        .default_address(UnifiedAddressRequest::ALLOW_ALL)
        .map_err(|e| anyhow!("UFVK default_address ALLOW_ALL: {:?}", e))?;
    let ua_transparent_receiver_str = unified_addr_all
        .transparent()
        .map(|t| Address::Transparent(*t).encode(&network));
    let unified_addr_all_str = Address::Unified(unified_addr_all).encode(&network);

    let vizor_request = UnifiedAddressRequest::unsafe_custom(
        ReceiverRequirement::Require, // Orchard
        ReceiverRequirement::Require, // Sapling
        ReceiverRequirement::Omit,    // Transparent
    );
    let (unified_addr_vizor, _) = ufvk
        .default_address(vizor_request)
        .map_err(|e| anyhow!("UFVK default_address (Vizor compat): {:?}", e))?;
    let unified_addr_vizor_str = Address::Unified(unified_addr_vizor).encode(&network);

    // Orchard-only UA — 송금자가 Orchard pool 로 강제로 보내게 할 때 사용.
    // 받는 사람이 Orchard 만 지원해도 동작 (sapling/transparent 못 쓰는 환경).
    let orchard_only_request = UnifiedAddressRequest::unsafe_custom(
        ReceiverRequirement::Require, // Orchard
        ReceiverRequirement::Omit,    // Sapling
        ReceiverRequirement::Omit,    // Transparent
    );
    let (unified_addr_orchard, _) = ufvk
        .default_address(orchard_only_request)
        .map_err(|e| anyhow!("UFVK default_address (Orchard-only): {:?}", e))?;
    let unified_addr_orchard_str = Address::Unified(unified_addr_orchard).encode(&network);

    // diversifier index → BIP-44 child index 매핑 정보 — 디버그용.
    let div_idx_bytes = all_div_idx.as_bytes();
    let div_idx_low_u32 = u32::from_le_bytes([
        div_idx_bytes[0],
        div_idx_bytes[1],
        div_idx_bytes[2],
        div_idx_bytes[3],
    ]);

    // 5) 출력.
    let mut out = std::io::stdout().lock();
    writeln!(out, "# Zcash {} wallet", net_label)?;
    writeln!(out, "# (이 파일을 안전한 곳에 보관 — mnemonic 이 진짜 비밀)")?;
    if matches!(network, Network::MainNetwork) {
        writeln!(out, "# ⚠️  MAINNET — 진짜 자금. mnemonic 노출 = 자금 손실. 화면 캡처/공유 금지.")?;
    }
    writeln!(out)?;
    writeln!(out, "network: {}", net_label)?;
    writeln!(out, "account: 0")?;
    writeln!(out)?;
    if let Some(m) = &mnemonic_str {
        writeln!(out, "mnemonic (BIP-39, 영어): {}", m)?;
    } else {
        writeln!(
            out,
            "mnemonic: (none — --seed-hex 로 들어와서 mnemonic 가 없다)"
        )?;
    }
    writeln!(out, "seed_hex: {}", hex::encode(&seed_bytes))?;
    writeln!(out)?;
    writeln!(out, "ufvk: {}", ufvk_str)?;
    writeln!(out, "  → 최신 형식 (ZIP-316), sapling+orchard+transparent 통합. 우리 scanner 에 이걸 사용.")?;
    writeln!(out)?;
    writeln!(out, "sapling_extended_fvk: {}", sapling_efvk_str)?;
    writeln!(out, "  → 구식 sapling-only EFVK. testnet.zcashexplorer.app/vk 같은 구식 도구가 UFVK 안 받으면 이걸 넣기.")?;
    writeln!(out, "  → 단, sapling 노트만 볼 수 있음 (orchard/transparent 못 봄).")?;
    writeln!(out)?;
    let coin_type = match network {
        Network::MainNetwork => 133,
        Network::TestNetwork => 1,
    };
    writeln!(out, "transparent_address_index_0 (BIP-44 standalone, m/44'/{}'/0'/0/0): {}", coin_type, t_addr_str)?;
    writeln!(out, "  → 옛 wallet 의 standalone t-addr. Zashi/Vizor 가 보여주는 t-addr 와 *다를 수 있다* (그건 UA 안의 receiver).")?;
    writeln!(out)?;
    if let Some(tx_in_ua) = &ua_transparent_receiver_str {
        writeln!(out, "ua_transparent_receiver (UA diversifier-bound, Vizor 호환): {}", tx_in_ua)?;
        writeln!(out, "  → Vizor 의 'Transparent receive' 화면이 보여주는 t-addr 와 *이게* 일치해야 한다.")?;
        writeln!(out, "  → 내부적으로 diversifier_index={} → BIP-44 m/44'/{}'/0'/0/{} 에서 derive.", div_idx_low_u32, coin_type, div_idx_low_u32)?;
        writeln!(out)?;
    }
    writeln!(out, "unified_address_allow_all: {}", unified_addr_all_str)?;
    writeln!(out, "  → orchard+sapling+transparent 모두 포함된 UA.")?;
    writeln!(out)?;
    writeln!(out, "unified_address_vizor_compat: {}", unified_addr_vizor_str)?;
    writeln!(out, "  → Vizor 가 기본으로 보여주는 형태 (orchard+sapling, transparent 제외). Vizor 의 'Receive' 화면 utest1... 과 같아야 한다.")?;
    writeln!(out)?;
    writeln!(out, "unified_address_orchard_only: {}", unified_addr_orchard_str)?;
    writeln!(out, "  → Orchard receiver 만 들어 있는 UA. 송금자가 자동으로 Orchard pool 로 보내게 됨.")?;
    writeln!(out, "    Orchard 만 지원하는 환경에서도 받을 수 있음. shielded pool 명시 강제용.")?;
    writeln!(out)?;
    writeln!(out, "검증 (같은 mnemonic 으로 import 한 다른 지갑이 *다른 주소* 를 보여줘도 같은 지갑일 수 있다 — receiver 정책 차이):")?;
    writeln!(out, "  - 두 UA(allow_all vs vizor_compat) 는 같은 UFVK 에서 derive — 둘 다 우리 지갑의 유효한 주소.")?;
    writeln!(out, "  - Vizor 의 transparent 받기 주소는 ua_transparent_receiver 와 매칭돼야 한다 (standalone index_0 이 아님).")?;
    writeln!(out, "  - 또는 외부 지갑의 export UFVK 가 위 ufvk 와 같은지.")?;
    writeln!(out)?;
    writeln!(out, "다음 단계 (e2e 테스트):")?;
    if matches!(network, Network::MainNetwork) {
        writeln!(out, "  1. 위 mnemonic 을 Vizor (mainnet 기본 빌드) 에 import")?;
        writeln!(out, "  2. 자기 mainnet 자금이 보이는지 확인 (Vizor sync 후)")?;
        writeln!(out, "  3. Vizor 에서 임의의 다른 mainnet 주소로 작게 송금 (outgoing)")?;
        writeln!(out, "  4. submit-ufvk 도구로 ufvk + block range 를 scanner 에 보내 검증")?;
        writeln!(out, "     (--network main + 적절한 mainnet lightwalletd URL)")?;
    } else {
        writeln!(out, "  1. 위 mnemonic 을 Vizor testnet 빌드에 import")?;
        writeln!(out, "  2. faucet 에서 utest1... 또는 transparent 주소로 ZEC 받기")?;
        writeln!(out, "  3. Vizor 에서 임의의 다른 testnet 주소로 송금 (outgoing)")?;
        writeln!(out, "  4. submit-ufvk 도구로 ufvk + 해당 block range 를 scanner 에 보내 검증")?;
    }
    Ok(())
}
