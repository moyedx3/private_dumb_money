// Decide how to render decrypted content. The catalog (I3-a) carries no content_type, so we sniff
// common image magic bytes; everything else falls back to text/download.
// (Minor gap noted to the team: add `content_type` to I3-a to avoid sniffing.)

export type RenderKind = "image" | "text" | "binary";

export function detectKind(bytes: Uint8Array): RenderKind {
  if (hasPrefix(bytes, [0x89, 0x50, 0x4e, 0x47])) return "image"; // PNG
  if (hasPrefix(bytes, [0xff, 0xd8, 0xff])) return "image"; // JPEG
  if (hasPrefix(bytes, [0x47, 0x49, 0x46, 0x38])) return "image"; // GIF8
  if (hasPrefix(bytes, [0x52, 0x49, 0x46, 0x46]) && hasPrefixAt(bytes, 8, [0x57, 0x45, 0x42, 0x50])) {
    return "image"; // RIFF....WEBP
  }
  return isProbablyUtf8(bytes) ? "text" : "binary";
}

export function mimeFor(bytes: Uint8Array): string {
  if (hasPrefix(bytes, [0x89, 0x50, 0x4e, 0x47])) return "image/png";
  if (hasPrefix(bytes, [0xff, 0xd8, 0xff])) return "image/jpeg";
  if (hasPrefix(bytes, [0x47, 0x49, 0x46, 0x38])) return "image/gif";
  if (hasPrefix(bytes, [0x52, 0x49, 0x46, 0x46])) return "image/webp";
  return "application/octet-stream";
}

function hasPrefix(bytes: Uint8Array, prefix: number[]): boolean {
  return hasPrefixAt(bytes, 0, prefix);
}

function hasPrefixAt(bytes: Uint8Array, offset: number, prefix: number[]): boolean {
  if (bytes.length < offset + prefix.length) return false;
  return prefix.every((b, i) => bytes[offset + i] === b);
}

function isProbablyUtf8(bytes: Uint8Array): boolean {
  try {
    new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    return true;
  } catch {
    return false;
  }
}
