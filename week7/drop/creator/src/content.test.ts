import { describe, expect, it } from "vitest";
import { sha256Hex, utf8Bytes } from "./bytes";
import { decryptContent, encryptContent, parseSha256Hex } from "./content";

describe("content encryption", () => {
  it("emits I4 nonce || ciphertext || tag and hashes the full blob", async () => {
    const plaintext = utf8Bytes("secret content");
    const encrypted = await encryptContent(plaintext);

    expect(encrypted.kDrop).toHaveLength(32);
    expect(encrypted.blob.length).toBe(12 + plaintext.length + 16);
    expect(encrypted.hContent).toBe(await sha256Hex(encrypted.blob));
    await expect(decryptContent(encrypted.blob, encrypted.kDrop)).resolves.toEqual(plaintext);
  });

  it("parses sha256 hex and normalizes uppercase input", () => {
    expect(parseSha256Hex("AB".repeat(32))).toBe("ab".repeat(32));

    for (const bad of ["", "ab", "ab".repeat(31), "ab".repeat(33), "gg".repeat(32)]) {
      expect(() => parseSha256Hex(bad)).toThrow();
    }
  });
});
