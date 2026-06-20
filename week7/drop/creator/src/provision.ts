import sodium from "libsodium-wrappers";
import { toHex, utf8Bytes } from "./bytes";

export type ProvisionPayload = {
  drop_id: number;
  price_zat: number;
  k_drop: string;
  creator_ufvk: string;
  h_content: string;
};

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
  if (!/^[0-9a-f]{64}$/i.test(args.hContent)) {
    throw new Error("h_content must be a sha256 hex string");
  }
  return {
    drop_id: args.dropId,
    price_zat: args.priceZat,
    k_drop: toHex(args.kDrop),
    creator_ufvk: args.creatorUfvk.trim(),
    h_content: args.hContent.toLowerCase()
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
