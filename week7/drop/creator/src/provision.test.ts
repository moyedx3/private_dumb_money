import sodium from "libsodium-wrappers";
import { describe, expect, it } from "vitest";
import { toHex } from "./bytes";
import {
  buildProvisionPayload,
  parseDropId,
  parseProvisioningPubkey,
  parseShieldedDepositAddress,
  sealProvisionPayload
} from "./provision";

describe("provision payload", () => {
  it("uses the interfaces.md I5 JSON shape with k_drop bytes encoded as hex", () => {
    const payload = buildProvisionPayload({
      dropId: 1,
      priceZat: 500,
      kDrop: new Uint8Array(32).fill(2),
      creatorUfvk: "uview1demo",
      hContent: "a".repeat(64),
      depositAddr: "u1shieldedreceiver"
    });
    expect(payload).toEqual({
      drop_id: 1,
      price_zat: 500,
      k_drop: "02".repeat(32),
      creator_ufvk: "uview1demo",
      h_content: "a".repeat(64),
      deposit_addr: "u1shieldedreceiver"
    });
  });

  it("normalizes uppercase h_content to lowercase", () => {
    const payload = buildProvisionPayload({
      dropId: 1,
      priceZat: 500,
      kDrop: new Uint8Array(32).fill(2),
      creatorUfvk: "uview1demo",
      hContent: "AB".repeat(32),
      depositAddr: "u1shieldedreceiver"
    });

    expect(payload.h_content).toBe("ab".repeat(32));
  });

  it("rejects transparent deposit addresses before sealing", () => {
    expect(parseShieldedDepositAddress("u1shieldedreceiver")).toBe("u1shieldedreceiver");
    expect(parseShieldedDepositAddress("ztestsaplingreceiver")).toBe("ztestsaplingreceiver");

    for (const bad of ["", " ", "t1TransparentAddress", "tmTransparentTestnetAddress"]) {
      expect(() => parseShieldedDepositAddress(bad)).toThrow(/deposit_addr|shielded/i);
    }
  });

  it("rejects KDrop values that are not exactly 32 bytes", () => {
    for (const kDrop of [new Uint8Array(31), new Uint8Array(33)]) {
      expect(() =>
        buildProvisionPayload({
          dropId: 1,
          priceZat: 500,
          kDrop,
          creatorUfvk: "uview1demo",
          hContent: "a".repeat(64),
          depositAddr: "u1shieldedreceiver"
        })
      ).toThrow("32 bytes");
    }
  });

  it("parses drop IDs from UI strings as safe non-negative integers", () => {
    expect(parseDropId("0")).toBe(0);
    expect(parseDropId("42")).toBe(42);

    for (const bad of ["", " ", "-1", "1.2", "abc", "1e3", "01", "9007199254740992"]) {
      expect(() => parseDropId(bad)).toThrow();
    }
  });

  it("parses provisioning pubkey hex as exactly 32 bytes", () => {
    const key = parseProvisioningPubkey("AB".repeat(32));

    expect(key).toHaveLength(32);
    expect(toHex(key)).toBe("ab".repeat(32));

    for (const bad of ["", "ab", "ab".repeat(31), "ab".repeat(33), "gg".repeat(32)]) {
      expect(() => parseProvisioningPubkey(bad)).toThrow();
    }
  });

  it("seals payload bytes so only the enclave keypair can open them", async () => {
    await sodium.ready;
    const kp = sodium.crypto_box_keypair();
    const payload = buildProvisionPayload({
      dropId: 9,
      priceZat: 1_000_000,
      kDrop: new Uint8Array(32).fill(3),
      creatorUfvk: "uview1demo",
      hContent: "b".repeat(64),
      depositAddr: "u1shieldedreceiver"
    });
    const sealed = await sealProvisionPayload(payload, kp.publicKey);
    expect(sealed.length).toBeGreaterThan(JSON.stringify(payload).length);

    const opened = sodium.crypto_box_seal_open(sealed, kp.publicKey, kp.privateKey);
    expect(JSON.parse(new TextDecoder().decode(opened))).toEqual(payload);
  });

  it("wrong key cannot open sealed payload", async () => {
    await sodium.ready;
    const enclaveKeypair = sodium.crypto_box_keypair();
    const wrongKeypair = sodium.crypto_box_keypair();
    const enclavePubkey = parseProvisioningPubkey(toHex(enclaveKeypair.publicKey));
    const payload = buildProvisionPayload({
      dropId: 9,
      priceZat: 1_000_000,
      kDrop: new Uint8Array(32).fill(3),
      creatorUfvk: "uview1demo",
      hContent: "b".repeat(64),
      depositAddr: "u1shieldedreceiver"
    });

    const sealed = await sealProvisionPayload(payload, enclavePubkey);

    expect(() => sodium.crypto_box_seal_open(sealed, wrongKeypair.publicKey, wrongKeypair.privateKey)).toThrow();
  });
});
