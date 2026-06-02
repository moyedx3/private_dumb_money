use std::{
    collections::HashMap,
    io::{self, Read},
};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use zcash_client_backend::{
    data_api::BlockMetadata,
    proto::compact_formats,
    scanning::{scan_block, Nullifiers, ScanningKeyOps, ScanningKeys},
};
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedFullViewingKey, UnifiedIncomingViewingKey};
use zcash_protocol::consensus::Network;
use zip32::Scope;

#[derive(Debug, Deserialize)]
struct ScannerRequest {
    viewing_key: String,
    #[serde(default)]
    viewing_capability_type: String,
    network: String,
    compact_blocks: Vec<CompactBlock>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactBlock {
    proto_version: u32,
    height: u64,
    hash: String,
    prev_hash: String,
    time: u32,
    #[serde(default)]
    chain_metadata: Option<ChainMetadata>,
    #[serde(default)]
    vtx: Vec<CompactTx>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChainMetadata {
    sapling_commitment_tree_size: u32,
    orchard_commitment_tree_size: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactTx {
    index: u64,
    txid: String,
    #[serde(default)]
    fee: u32,
    #[serde(default)]
    spends: Vec<CompactSaplingSpend>,
    #[serde(default)]
    outputs: Vec<CompactSaplingOutput>,
    #[serde(default)]
    actions: Vec<CompactOrchardAction>,
}

#[derive(Debug, Deserialize)]
struct CompactSaplingSpend {
    nf: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactSaplingOutput {
    cmu: String,
    ephemeral_key: String,
    ciphertext: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactOrchardAction {
    nf: String,
    cmx: String,
    ephemeral_key: String,
    ciphertext: String,
}

#[derive(Debug, Serialize)]
struct ScannerResponse {
    owned_commitments: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    derived_addresses: Option<DerivedAddresses>,
}

#[derive(Debug, Serialize)]
struct DerivedAddresses {
    default_unified_address: String,
}

fn main() -> Result<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request: ScannerRequest =
        serde_json::from_str(&input).context("invalid scanner request JSON")?;
    let network = parse_network(&request.network)?;
    let owned_commitments = scan_owned_commitments(&network, &request)?;
    let derived_addresses = derive_default_address(&network, &request);
    println!(
        "{}",
        serde_json::to_string(&ScannerResponse {
            owned_commitments,
            derived_addresses,
        })?
    );
    Ok(())
}

fn parse_network(value: &str) -> Result<Network> {
    match value.to_ascii_lowercase().as_str() {
        "mainnet" | "main" => Ok(Network::MainNetwork),
        "testnet" | "test" => Ok(Network::TestNetwork),
        other => Err(anyhow!(
            "unsupported Zcash network for real scanner: {other}"
        )),
    }
}

fn scan_owned_commitments(network: &Network, request: &ScannerRequest) -> Result<Vec<String>> {
    type AccountId = u32;
    let scanning_keys = decode_scanning_keys(network, request)?;
    let nullifiers = Nullifiers::<AccountId>::empty();
    let mut prior_meta: Option<BlockMetadata> = None;
    let mut owned = Vec::new();

    for source_block in &request.compact_blocks {
        let block = map_compact_block(source_block)?;
        let scanned = scan_block(
            network,
            block,
            &scanning_keys,
            &nullifiers,
            prior_meta.as_ref(),
        )
        .map_err(|e| anyhow!("scan error at height {}: {e}", e.at_height()))?;

        for wallet_tx in scanned.transactions() {
            let tx_index: u64 = wallet_tx.block_index().into();
            let source_tx = source_block
                .vtx
                .iter()
                .find(|tx| tx.index == tx_index)
                .ok_or_else(|| anyhow!("missing source compact tx index {tx_index}"))?;

            for output in wallet_tx.sapling_outputs() {
                let commitment = source_tx
                    .outputs
                    .get(output.index())
                    .ok_or_else(|| anyhow!("missing sapling compact output {}", output.index()))?;
                owned.push(normalize_commitment(&commitment.cmu, "sapling cmu")?);
            }

            for output in wallet_tx.orchard_outputs() {
                let commitment = source_tx
                    .actions
                    .get(output.index())
                    .ok_or_else(|| anyhow!("missing orchard compact action {}", output.index()))?;
                owned.push(normalize_commitment(&commitment.cmx, "orchard cmx")?);
            }
        }

        prior_meta = Some(scanned.to_block_metadata());
    }

    owned.sort();
    owned.dedup();
    Ok(owned)
}

fn decode_scanning_keys(
    network: &Network,
    request: &ScannerRequest,
) -> Result<ScanningKeys<u32, u8>> {
    let viewing_key = normalize_viewing_key(&request.viewing_key);
    match request
        .viewing_capability_type
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "ufvk" | "fvk" => {
            let ufvk = UnifiedFullViewingKey::decode(network, &viewing_key)
                .map_err(|e| anyhow!("invalid UFVK/FVK: {e}"))?;
            Ok(scanning_keys_from_uivk(
                ufvk.to_unified_incoming_viewing_key(),
            ))
        }
        "uivk" | "ivk" => {
            let uivk = UnifiedIncomingViewingKey::decode(network, &viewing_key)
                .map_err(|e| anyhow!("invalid UIVK/IVK: {e}"))?;
            Ok(scanning_keys_from_uivk(uivk))
        }
        other => Err(anyhow!("unsupported viewing_capability_type: {other}")),
    }
}

struct SaplingIvkScanner {
    account_id: u32,
    ivk: sapling::zip32::IncomingViewingKey,
}

impl ScanningKeyOps<sapling::note_encryption::SaplingDomain, u32, sapling::Nullifier>
    for SaplingIvkScanner
{
    fn prepare(&self) -> sapling::note_encryption::PreparedIncomingViewingKey {
        self.ivk.prepare()
    }

    fn account_id(&self) -> &u32 {
        &self.account_id
    }

    fn key_scope(&self) -> Option<Scope> {
        None
    }

    fn nf(
        &self,
        _note: &sapling::Note,
        _note_position: incrementalmerkletree::Position,
    ) -> Option<sapling::Nullifier> {
        None
    }
}

struct OrchardIvkScanner {
    account_id: u32,
    ivk: orchard::keys::IncomingViewingKey,
}

impl ScanningKeyOps<orchard::note_encryption::OrchardDomain, u32, orchard::note::Nullifier>
    for OrchardIvkScanner
{
    fn prepare(&self) -> orchard::keys::PreparedIncomingViewingKey {
        self.ivk.prepare()
    }

    fn account_id(&self) -> &u32 {
        &self.account_id
    }

    fn key_scope(&self) -> Option<Scope> {
        None
    }

    fn nf(
        &self,
        _note: &orchard::note::Note,
        _note_position: incrementalmerkletree::Position,
    ) -> Option<orchard::note::Nullifier> {
        None
    }
}

fn scanning_keys_from_uivk(uivk: UnifiedIncomingViewingKey) -> ScanningKeys<u32, u8> {
    let mut sapling_keys: HashMap<
        u8,
        Box<
            dyn ScanningKeyOps<sapling::note_encryption::SaplingDomain, u32, sapling::Nullifier>
                + Send
                + Sync,
        >,
    > = HashMap::new();
    if let Some(ivk) = uivk.sapling().clone() {
        sapling_keys.insert(1, Box::new(SaplingIvkScanner { account_id: 0, ivk }));
    }

    let mut orchard_keys: HashMap<
        u8,
        Box<
            dyn ScanningKeyOps<
                    orchard::note_encryption::OrchardDomain,
                    u32,
                    orchard::note::Nullifier,
                > + Send
                + Sync,
        >,
    > = HashMap::new();
    if let Some(ivk) = uivk.orchard().clone() {
        orchard_keys.insert(2, Box::new(OrchardIvkScanner { account_id: 0, ivk }));
    }

    ScanningKeys::new(sapling_keys, orchard_keys)
}

fn normalize_viewing_key(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some((ufvk, _suffix)) = trimmed.split_once('|') {
        ufvk.to_string()
    } else {
        trimmed.to_string()
    }
}

fn derive_default_address(network: &Network, request: &ScannerRequest) -> Option<DerivedAddresses> {
    let viewing_key = normalize_viewing_key(&request.viewing_key);
    let address = match request
        .viewing_capability_type
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "ufvk" | "fvk" => UnifiedFullViewingKey::decode(network, &viewing_key)
            .ok()?
            .default_address(UnifiedAddressRequest::AllAvailableKeys)
            .ok()?
            .0
            .encode(network),
        "uivk" | "ivk" => UnifiedIncomingViewingKey::decode(network, &viewing_key)
            .ok()?
            .default_address(UnifiedAddressRequest::AllAvailableKeys)
            .ok()?
            .0
            .encode(network),
        _ => return None,
    };
    Some(DerivedAddresses {
        default_unified_address: address,
    })
}

fn normalize_commitment(hex_value: &str, field: &str) -> Result<String> {
    let trimmed = hex_value
        .trim()
        .strip_prefix("0x")
        .unwrap_or(hex_value.trim());
    if trimmed.len() != 64 {
        return Err(anyhow!("{field} must be a 32-byte hex commitment"));
    }
    let _ = decode_hex(trimmed, field)?;
    Ok(trimmed.to_ascii_lowercase())
}

fn decode_hex(s: &str, field: &str) -> Result<Vec<u8>> {
    hex::decode(s).with_context(|| format!("invalid hex in {field}"))
}

fn map_compact_block(block: &CompactBlock) -> Result<compact_formats::CompactBlock> {
    Ok(compact_formats::CompactBlock {
        proto_version: block.proto_version,
        height: block.height,
        hash: decode_hex(&block.hash, "block hash")?,
        prev_hash: decode_hex(&block.prev_hash, "block prevHash")?,
        time: block.time,
        header: Vec::new(),
        vtx: block
            .vtx
            .iter()
            .map(map_compact_tx)
            .collect::<Result<Vec<_>>>()?,
        chain_metadata: block.chain_metadata.as_ref().map(|metadata| {
            compact_formats::ChainMetadata {
                sapling_commitment_tree_size: metadata.sapling_commitment_tree_size,
                orchard_commitment_tree_size: metadata.orchard_commitment_tree_size,
            }
        }),
    })
}

fn map_compact_tx(tx: &CompactTx) -> Result<compact_formats::CompactTx> {
    Ok(compact_formats::CompactTx {
        index: tx.index,
        txid: decode_hex(&tx.txid, "txid")?,
        fee: tx.fee,
        spends: tx
            .spends
            .iter()
            .map(|spend| {
                Ok(compact_formats::CompactSaplingSpend {
                    nf: decode_hex(&spend.nf, "sapling spend nf")?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        outputs: tx
            .outputs
            .iter()
            .map(|output| {
                Ok(compact_formats::CompactSaplingOutput {
                    cmu: decode_hex(&output.cmu, "sapling output cmu")?,
                    ephemeral_key: decode_hex(
                        &output.ephemeral_key,
                        "sapling output ephemeralKey",
                    )?,
                    ciphertext: decode_hex(&output.ciphertext, "sapling output ciphertext")?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        actions: tx
            .actions
            .iter()
            .map(|action| {
                Ok(compact_formats::CompactOrchardAction {
                    nullifier: decode_hex(&action.nf, "orchard action nf")?,
                    cmx: decode_hex(&action.cmx, "orchard action cmx")?,
                    ephemeral_key: decode_hex(
                        &action.ephemeral_key,
                        "orchard action ephemeralKey",
                    )?,
                    ciphertext: decode_hex(&action.ciphertext, "orchard action ciphertext")?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        vin: Vec::new(),
        vout: Vec::new(),
    })
}
