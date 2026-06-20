import { bytesToArrayBuffer, concatBytes, sha256Hex } from "./bytes";

export type EncryptedContent = {
  blob: Uint8Array;
  hContent: string;
  kDrop: Uint8Array;
};

export async function encryptContent(plaintext: Uint8Array): Promise<EncryptedContent> {
  const kDrop = crypto.getRandomValues(new Uint8Array(32));
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const key = await crypto.subtle.importKey("raw", bytesToArrayBuffer(kDrop), "AES-GCM", false, ["encrypt"]);
  const ciphertextWithTag = new Uint8Array(
    await crypto.subtle.encrypt(
      { name: "AES-GCM", iv: bytesToArrayBuffer(nonce), tagLength: 128 },
      key,
      bytesToArrayBuffer(plaintext)
    )
  );
  const blob = concatBytes([nonce, ciphertextWithTag]);
  return {
    blob,
    hContent: await sha256Hex(blob),
    kDrop
  };
}

export async function decryptContent(blob: Uint8Array, kDrop: Uint8Array): Promise<Uint8Array> {
  if (blob.length < 12 + 16) {
    throw new Error("content blob is too short");
  }
  const nonce = blob.slice(0, 12);
  const ciphertextWithTag = blob.slice(12);
  const key = await crypto.subtle.importKey("raw", bytesToArrayBuffer(kDrop), "AES-GCM", false, ["decrypt"]);
  return new Uint8Array(
    await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: bytesToArrayBuffer(nonce), tagLength: 128 },
      key,
      bytesToArrayBuffer(ciphertextWithTag)
    )
  );
}
