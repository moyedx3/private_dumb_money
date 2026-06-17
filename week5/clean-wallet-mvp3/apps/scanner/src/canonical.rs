use sha2::{Digest, Sha256};

pub fn canonicalize(value: &serde_json::Value) -> Result<Vec<u8>, anyhow::Error> {
    Ok(serde_jcs::to_vec(value)?)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/schemas/fixtures")
    }

    fn fixture_names() -> Vec<String> {
        let dir = fixtures_dir();
        let mut names: Vec<String> = fs::read_dir(&dir).unwrap()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".input.json"))
            .map(|n| n.trim_end_matches(".input.json").to_string())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn canonicalization_matches_typescript_for_every_fixture() {
        let dir = fixtures_dir();
        let names = fixture_names();
        assert!(!names.is_empty(), "no fixtures found at {dir:?}");

        for name in &names {
            let input_path = dir.join(format!("{name}.input.json"));
            let canonical_path = dir.join(format!("{name}.canonical.bin"));
            let sha_path = dir.join(format!("{name}.sha256.hex"));

            let input: serde_json::Value =
                serde_json::from_slice(&fs::read(&input_path).unwrap()).unwrap();
            let expected_canonical = fs::read(&canonical_path).unwrap();
            let expected_sha = fs::read_to_string(&sha_path).unwrap().trim().to_string();

            let actual_canonical = canonicalize(&input).unwrap();
            assert_eq!(
                actual_canonical, expected_canonical,
                "canonical bytes differ for fixture {name}"
            );
            assert_eq!(
                sha256_hex(&actual_canonical),
                expected_sha,
                "sha256 differs for fixture {name}"
            );
        }
    }
}
