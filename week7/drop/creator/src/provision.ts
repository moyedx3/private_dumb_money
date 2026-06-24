import sodium from "libsodium-wrappers";
import { fromHex, toHex, utf8Bytes } from "./bytes";
import { parseSha256Hex } from "./content";

export type ProvisionPayload = {
  drop_id: number;
  price_zat: number;
  k_drop: string;
  creator_ufvk: string;
  h_content: string;
};

export function parseDropId(input: string): number {
  const trimmed = input.trim();
  if (!/^(0|[1-9]\d*)$/.test(trimmed)) {
    throw new Error("drop_id must be a non-negative integer");
  }
  const dropId = Number(trimmed);
  if (!Number.isSafeInteger(dropId)) {
    throw new Error("drop_id exceeds JavaScript safe integer range");
  }
  return dropId;
}

export function parseProvisioningPubkey(input: string): Uint8Array {
  const trimmed = input.trim();
  if (!/^[0-9a-fA-F]{64}$/.test(trimmed)) {
    throw new Error("provisioning_pubkey must be exactly 32 bytes as hex");
  }
  return fromHex(trimmed);
}

export function buildProvisionPayload(args: {
  dropId: number;
  priceZat: number;
  kDrop: Uint8Array;
  creatorUfvk: string;
  hContent: string;
}): ProvisionPayload {
  if (!Number.isSafeInteger(args.dropId) || args.dropId < 0) {
    throw new Error("drop_id must be a non-negative safe integer");
  }
  if (!Number.isSafeInteger(args.priceZat) || args.priceZat < 0) {
    throw new Error("price_zat must be a non-negative safe integer");
  }
  if (args.kDrop.length !== 32) {
    throw new Error("k_drop must be 32 bytes");
  }
  if (!args.creatorUfvk.trim()) {
    throw new Error("creator_ufvk is required");
  }
  const hContent = parseSha256Hex(args.hContent);
  return {
    drop_id: args.dropId,
    price_zat: args.priceZat,
    k_drop: toHex(args.kDrop),
    creator_ufvk: args.creatorUfvk.trim(),
    h_content: hContent
  };
}

export async function sealProvisionPayload(payload: ProvisionPayload, enclavePubkey: Uint8Array): Promise<Uint8Array> {
  if (enclavePubkey.length !== 32) {
    throw new Error("enclave public key must be 32 bytes");
  }
  await sodium.ready;
  const body = utf8Bytes(JSON.stringify(payload));
  return sodium.crypto_box_seal(body, enclavePubkey);
}
