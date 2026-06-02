//! PoW 헤더 체인 검증 단위 테스트 (C1).
//!
//! 검증 항목:
//! 1) target_from_bits: nBits 컴팩트 표현이 LE 32바이트 타깃으로 잘 펼쳐지는지.
//! 2) PowVerifier::verify_header: 실 Zcash mainnet 블록 헤더가 통과하는지.
//! 3) 헤더의 비트 하나라도 건드리면 거부하는지.
//!
//! 헤더 출처: Zcash mainnet block #1,000,000 (blockchair API raw block 의 앞부분).
//! 이 블록은 실제 채굴된 블록이므로 Equihash + target 모두 통과해야 한다.

use zcash_scanner_rs::{target_from_bits, PowVerifier};

/// Zcash mainnet block #1,000,000 의 raw 헤더 (1487 바이트 = 2974 hex chars).
/// 블록 hex 전체에서 앞부분만 잘라낸 것.
const MAINNET_BLOCK_1000000_HEADER_HEX: &str = concat!(
    "04000000",
    "77f36aa43aeba34a284bdb6aeabf55b7035fd490589cf498ea2d510100000000",
    "5addfb1cac535809e025523319a1e3fe65228e837a19bfab0a1f0a250ea0d5a1",
    "d5834caa7ab34a6b3fa8dbd6d2277643654e4f1ee5216ddd4a6e03efab4ab948",
    "4dbb7f5f",
    "a2d0011c",
    "000000000000000000000000000000000000470000000000000000008003f82c",
    "fd4005",
    // 1344 byte equihash solution (2688 hex chars)
    "0051da0866d2c8b7cd1b61d00b6a04731efc5b46d314d4b746c75baf930234e31b",
    "34de8c6263ee79fc360368d4da72d5a6bfcecba8d4befdcc368adb9f0f8c03daf",
    "6726325494754537b5356dc0e32fc301df4551b457b82d069498f5e3186bc56ef",
    "0dee10d7fa3c4d5387339e75a127ef40cd77d3455e4589f8f2123113276bc2104",
    "84f9f837d11a2d4671902192bdcb2ac3e38c89ac7b0943dc7faff05c3af74a1a5",
    "d1ffb7dd44006a0a4070c690d677a10202f0ef2bd1c87537c4a10566f1e41ed25",
    "1f7fd398535a4336fb651ba7f0db103df834386e9deb972f5827beb997b7a5ca6",
    "da287c3260f3dd8d73a811acf85a0188fb5b33074cfd56c206a6bb84f2129b9cf",
    "01e687d47e8d2567dc8551631344cb4a33815a6cd5673d736f76b7de64a217f6f",
    "b0126263f46f1fc0e9efa9220791fd3598c88f921413236739282f231db92a8a3",
    "3a21a3f1e1a701214cd2802e2c91a35ab40976a04913dc0f257850f23ef1ef114",
    "2e3e917a9058d72b0a9574e93c32ddc0d29c4a27121a642ad5cb16775cc0a28bd",
    "07015f5a89c128d802f033cfccc5c8f652773c9e92df4d1672d733d8fac08d983",
    "83a1f186bbbba078318e5f7902e40239bf2e1a47f208a1de92ffffc42250d63ff",
    "d40ece6edd88723082ab865ce7b76bd9bb242ae777802453d55cbd72914fdca2d",
    "103c9d5b2fd2d9a4d08840d54e1f3a89032d07fb7e16233b3732e071232ae1a29",
    "43017a10435628a4fb96fa39b970206b59ae8f24e31edb769140a801d31b048b3",
    "b9aa46a0cc6ff17d091ecd31fa361aff4cccebc7d2caf8780645fb58ad4e9f9db",
    "fed0407e90adf8506be6dc8f1b37c993d834e240a778a77261dd3d18e1c07c547",
    "edc6f7e0f5902e2e225e320e107f855096d6ae85d3ba630e4028f11ac79b37ceb",
    "61686b6bce0884308e9a8d3d636d1fd81377173be3e013c1625eaf019d3baad42",
    "f7bf25086581a56d8e120c6b72cc4a5a7203cebf50d4a8f50ca1af4f951f4a0ce",
    "20783844982416d4d46f575f88582717a3cc2b86a81e3d0fd9b9c0dfb79a88bee",
    "c7123a5c5d9f978503c63189620a112cf0734b11f8e8372b0ebf7c95ea3fc98ef",
    "4bed98c11313e08cb19eeced3cf05ffedee3c706b4f0cd42fc763b6a4c9f1568f",
    "360c9bbe6ba03e4c7450365942efd5b7171329632836b6f09d3b1301882bf9df7",
    "db09df41a902ca0c985e8d23706b18b0572d2f1c4d3afa96189a07db874aa40cd",
    "0cb0e234280dc571e9f6d33bf8e6f4504fbd8b4a66b35dd0965c818c828619583",
    "da5a066a9eaebeec5f645f4e66805bd74c9810c9c29724712ca091ac569b59d1d",
    "3e7d0988d0964fa5fd7df0d0f09eaa1a71cd8df53763db07a786b7ca5adf0ff51",
    "9b3dd8b1f48ea4e4975a96422dc43207094e291d30f34a1307c7b91ca8bd53ef5",
    "020a1763e85a5107e0f5c64213c13923540c5d0e751bae76a3b68ab028b481732",
    "4385a1e1b0c837dc2c7021e2534bf93150b0220958ea7e5d1057dd4b359a127d6",
    "7ad98a91814583c193fb6a44e82f3df09cd0400838104c5831973fc74a812d35d",
    "ef1704dccbd95772198bca9ef4c16f6bac912d7fe99bcae3969f6bc523549db00",
    "b555713eef0a2655343df25652d434388642140d3f32d3278dd7e6e5cbfcca192",
    "982320f1d0f03bad5a3f4c80e1c69123137add41fa776e39c708c0781a0f74e44",
    "baeb968be1a110308198951370d400111079e0e40b0d25a6a29231765316059fb",
    "0f904fa117dcb7c434d4f458bf021e68f4a27058074748565149a7c01036f9a8f",
    "eac2e38d5960f033cb0c9f6ebb3a3e89d3745fa99de1a74469f6497559259b321",
    "dce294d0f9577cffc7af80bd2c40b4a0879cae4fcd68f70b4cf0eb69f9caf1786",
    "196d5053f4e2781d1e53a9",
);

fn header_bytes() -> Vec<u8> {
    hex::decode(MAINNET_BLOCK_1000000_HEADER_HEX).expect("valid hex")
}

#[test]
fn header_바이트_길이는_1487이다() {
    let b = header_bytes();
    assert_eq!(b.len(), 1487, "Zcash 헤더 = 140 + 3 + 1344 = 1487");
}

// --- target_from_bits ---

#[test]
fn target_from_bits_difficulty_1_타깃은_표준값() {
    // Bitcoin/Zcash 의 difficulty-1 타깃: nBits = 0x1d00ffff
    // BE 표현: 00000000 ffff 0000 0000 0000 0000 0000 0000 ...
    // LE: target[26] = 0xff, target[27] = 0xff, 나머지 0.
    let target = target_from_bits(0x1d00ffff);
    let mut expected = [0u8; 32];
    expected[26] = 0xff;
    expected[27] = 0xff;
    assert_eq!(target, expected);
}

#[test]
fn target_from_bits_mainnet_블록_타깃() {
    // mainnet block #1,000,000 의 bits = 0x1c01d0a2 (BE).
    // exp = 0x1c = 28, mant = 0x0001d0a2
    // shift = 25 → target[25] = 0xa2, target[26] = 0xd0, target[27] = 0x01
    let target = target_from_bits(0x1c01d0a2);
    assert_eq!(target[25], 0xa2);
    assert_eq!(target[26], 0xd0);
    assert_eq!(target[27], 0x01);
    // 나머지 위치는 0.
    for (i, b) in target.iter().enumerate() {
        if i == 25 || i == 26 || i == 27 {
            continue;
        }
        assert_eq!(*b, 0u8, "target[{}] should be 0, got {:#x}", i, b);
    }
}

#[test]
fn target_from_bits_매우_작은_exp() {
    // exp <= 3 일 때 mantissa 를 시프트해서 저장.
    // bits = 0x03000001: exp=3, mant=1 → target[0] = 1, 나머지 0.
    let target = target_from_bits(0x03000001);
    let mut expected = [0u8; 32];
    expected[0] = 1;
    assert_eq!(target, expected);
}

// --- PowVerifier::verify_header ---

#[test]
fn verify_header_은_실_mainnet_블록을_통과시킨다() {
    let verifier = PowVerifier::new();
    let bytes = header_bytes();
    let r = verifier
        .verify_header(&bytes)
        .expect("실 mainnet block #1,000,000 헤더는 PoW + target 모두 통과해야 한다");
    // sanity 체크: prev_hash 가 0이 아닌 32바이트.
    assert_eq!(r.prev_hash.len(), 32);
    assert_ne!(r.prev_hash, [0u8; 32]);
    // 해시도 0이 아니어야 한다.
    assert_ne!(r.hash, [0u8; 32]);
}

#[test]
fn verify_header_는_nonce_를_바꾸면_거부() {
    let verifier = PowVerifier::new();
    let mut bytes = header_bytes();
    // nonce 의 한 바이트를 뒤집는다 (offset 108 ~ 140).
    bytes[120] ^= 0x01;
    let err = verifier
        .verify_header(&bytes)
        .expect_err("바뀐 nonce 로는 Equihash 가 invalid 해야 한다");
    let msg = format!("{:#}", err);
    assert!(
        msg.contains("equihash") || msg.contains("Equihash") || msg.contains("PoW") || msg.contains("target"),
        "예상 메시지가 아님: {}",
        msg
    );
}

#[test]
fn verify_header_는_solution_을_바꾸면_거부() {
    let verifier = PowVerifier::new();
    let mut bytes = header_bytes();
    // solution 의 첫 데이터 바이트(=position 143, length-prefix 다음)를 뒤집는다.
    bytes[143] ^= 0xff;
    let err = verifier
        .verify_header(&bytes)
        .expect_err("바뀐 solution 으로는 invalid 해야 한다");
    let msg = format!("{:#}", err);
    assert!(
        msg.contains("equihash") || msg.contains("PoW") || msg.contains("target"),
        "예상 메시지가 아님: {}",
        msg
    );
}

#[test]
fn verify_header_는_bits_를_더_쉽게_바꿔도_target_은_여전히_검사() {
    // bits 를 1d00ffff (Bitcoin difficulty-1, 더 큰 타깃) 로 바꾸면 target check 는 쉽게 통과하지만
    // 이번엔 Equihash input(첫 108 바이트) 이 바뀌어서 솔루션이 invalid 해진다. 핵심은
    // "어떤 한 바이트만 바꿔도 검증이 깨진다" — tamper resistance.
    let verifier = PowVerifier::new();
    let mut bytes = header_bytes();
    // bits 는 offset 104..108 (LE).
    bytes[104] = 0xff;
    bytes[105] = 0xff;
    bytes[106] = 0x00;
    bytes[107] = 0x1d;
    let err = verifier
        .verify_header(&bytes)
        .expect_err("bits 가 바뀌면 Equihash input 도 바뀌어 솔루션 invalid");
    let _ = err;
}

#[test]
fn verify_header_는_너무_짧으면_거부() {
    let verifier = PowVerifier::new();
    let short = vec![0u8; 50];
    assert!(verifier.verify_header(&short).is_err());
}
