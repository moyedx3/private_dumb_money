//! Adapter from our generated lightwalletd protobuf structs to `zecscope-scanner`'s
//! documented compact-block input types.
//!
//! `zecscope-scanner` consumes JSON-friendly hex strings, while the gRPC client
//! returns raw protobuf byte fields.  Keep the conversion isolated so the A1
//! scanner path can swap compact-block scanning implementations without touching
//! lightwalletd transport code.

use crate::lightwalletd::proto::compact as lw;
use zecscope_scanner as zecscope;

pub fn to_zecscope_blocks(blocks: &[lw::CompactBlock]) -> Vec<zecscope::CompactBlock> {
    blocks.iter().map(to_zecscope_block).collect()
}

pub fn to_zecscope_block(block: &lw::CompactBlock) -> zecscope::CompactBlock {
    zecscope::CompactBlock {
        proto_version: block.proto_version,
        height: block.height,
        hash: hex::encode(&block.hash),
        prev_hash: hex::encode(&block.prev_hash),
        time: block.time,
        vtx: block.vtx.iter().map(to_zecscope_tx).collect(),
        chain_metadata: block.chain_metadata.as_ref().map(to_zecscope_metadata),
    }
}

fn to_zecscope_metadata(metadata: &lw::ChainMetadata) -> zecscope::ChainMetadata {
    zecscope::ChainMetadata {
        sapling_commitment_tree_size: metadata.sapling_commitment_tree_size,
        orchard_commitment_tree_size: Some(metadata.orchard_commitment_tree_size),
    }
}

fn to_zecscope_tx(tx: &lw::CompactTx) -> zecscope::CompactTx {
    zecscope::CompactTx {
        index: tx.index,
        // lightwalletd already returns protocol-order txid bytes; zecscope's
        // documented type wants the same bytes represented as hex so it can
        // decode them back into compact protobuf fields.
        txid: hex::encode(&tx.txid),
        fee: Some(tx.fee),
        spends: tx.spends.iter().map(to_zecscope_sapling_spend).collect(),
        outputs: tx.outputs.iter().map(to_zecscope_sapling_output).collect(),
        actions: tx.actions.iter().map(to_zecscope_orchard_action).collect(),
    }
}

fn to_zecscope_sapling_spend(spend: &lw::CompactSaplingSpend) -> zecscope::CompactSaplingSpend {
    zecscope::CompactSaplingSpend {
        nf: hex::encode(&spend.nf),
    }
}

fn to_zecscope_sapling_output(output: &lw::CompactSaplingOutput) -> zecscope::CompactSaplingOutput {
    zecscope::CompactSaplingOutput {
        cmu: hex::encode(&output.cmu),
        ephemeral_key: hex::encode(&output.ephemeral_key),
        ciphertext: hex::encode(&output.ciphertext),
    }
}

fn to_zecscope_orchard_action(action: &lw::CompactOrchardAction) -> zecscope::CompactOrchardAction {
    zecscope::CompactOrchardAction {
        nf: hex::encode(&action.nullifier),
        cmx: hex::encode(&action.cmx),
        ephemeral_key: hex::encode(&action.ephemeral_key),
        ciphertext: hex::encode(&action.ciphertext),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_lightwalletd_bytes_to_zecscope_hex() {
        let block = lw::CompactBlock {
            proto_version: 1,
            height: 3363067,
            hash: vec![0xaa; 32],
            prev_hash: vec![0xbb; 32],
            time: 1_700_000_000,
            chain_metadata: Some(lw::ChainMetadata {
                sapling_commitment_tree_size: 7,
                orchard_commitment_tree_size: 9,
            }),
            vtx: vec![lw::CompactTx {
                index: 3,
                txid: vec![0x11; 32],
                fee: 1234,
                spends: vec![lw::CompactSaplingSpend { nf: vec![0x22; 32] }],
                outputs: vec![lw::CompactSaplingOutput {
                    cmu: vec![0x33; 32],
                    ephemeral_key: vec![0x44; 32],
                    ciphertext: vec![0x55; 52],
                }],
                actions: vec![lw::CompactOrchardAction {
                    nullifier: vec![0x66; 32],
                    cmx: vec![0x77; 32],
                    ephemeral_key: vec![0x88; 32],
                    ciphertext: vec![0x99; 52],
                }],
                ..Default::default()
            }],
            ..Default::default()
        };

        let converted = to_zecscope_block(&block);

        assert_eq!(converted.proto_version, 1);
        assert_eq!(converted.height, 3363067);
        assert_eq!(converted.hash, "aa".repeat(32));
        assert_eq!(converted.prev_hash, "bb".repeat(32));
        assert_eq!(converted.time, 1_700_000_000);
        assert_eq!(
            converted
                .chain_metadata
                .unwrap()
                .sapling_commitment_tree_size,
            7
        );
        assert_eq!(converted.vtx.len(), 1);

        let tx = &converted.vtx[0];
        assert_eq!(tx.index, 3);
        assert_eq!(tx.txid, "11".repeat(32));
        assert_eq!(tx.fee, Some(1234));
        assert_eq!(tx.spends[0].nf, "22".repeat(32));
        assert_eq!(tx.outputs[0].cmu, "33".repeat(32));
        assert_eq!(tx.outputs[0].ephemeral_key, "44".repeat(32));
        assert_eq!(tx.outputs[0].ciphertext, "55".repeat(52));
        assert_eq!(tx.actions[0].nf, "66".repeat(32));
        assert_eq!(tx.actions[0].cmx, "77".repeat(32));
        assert_eq!(tx.actions[0].ephemeral_key, "88".repeat(32));
        assert_eq!(tx.actions[0].ciphertext, "99".repeat(52));
    }
}
