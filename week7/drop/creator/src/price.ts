const ZATOSHI_PER_ZEC = 100_000_000n;

export function priceZecToZat(input: string): bigint {
  const trimmed = input.trim();
  if (!/^(0|[1-9]\d*)(\.\d{1,8})?$/.test(trimmed)) {
    throw new Error("price must be a non-negative ZEC amount with at most 8 decimals");
  }
  const [whole, frac = ""] = trimmed.split(".");
  const fracPadded = (frac + "00000000").slice(0, 8);
  return BigInt(whole) * ZATOSHI_PER_ZEC + BigInt(fracPadded);
}

export function priceZecToZatNumber(input: string): number {
  const value = priceZecToZat(input);
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error("price_zat exceeds JavaScript safe integer range");
  }
  return Number(value);
}
