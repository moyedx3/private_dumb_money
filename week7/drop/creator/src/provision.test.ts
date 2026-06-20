import sodium from "libsodium-wrappers";
import { describe, expect, it } from "vitest";
import { buildProvisionPayload, sealProvisionPayload } from "./provision";

describe("provision payload", () => {
  it("uses the interfaces.md I5 JSON shape with k_drop bytes encoded as hex", () => {
    const payload = buildProvisionPayload({
      dropId: 1,
      priceZat: 500,
      kDrop: new Uint8Array(32).fill(2),
      creatorUfvk: "uview1demo",
      hContent: "a".repeat(64)
    });
    expect(payload).toEqual({
      drop_id: 1,
      price_zat: 500,
      k_drop: "02".repeat(32),
      creator_ufvk: "uview1demo",
      h_content: "a".repeat(64)
    });
  });

  it("seals payload bytes so only the enclave keypair can open them", async () => {
    await sodium.ready;
    const kp = sodium.crypto_box_keypair();
    const payload = buildProvisionPayload({
      dropId: 9,
      priceZat: 1_000_000,
      kDrop: new Uint8Array(32).fill(3),
      creatorUfvk: "uview1demo",
      hContent: "b".repeat(64)
    });
    const sealed = await sealProvisionPayload(payload, kp.publicKey);
    expect(sealed.length).toBeGreaterThan(JSON.stringify(payload).length);

    const opened = sodium.crypto_box_seal_open(sealed, kp.publicKey, kp.privateKey);
    expect(JSON.parse(new TextDecoder().decode(opened))).toEqual(payload);
  });
});
