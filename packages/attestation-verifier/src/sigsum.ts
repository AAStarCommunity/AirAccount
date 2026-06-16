// Created: 2026-06-16
//
// Sigsum proof verification (issue #87, B: measurement transparency log).
// Implements the EXACT wire formats from sigsum-go pkg/types + pkg/proof
// (transcribed from authoritative source, validated against test vectors that
// sigsum-go v0.11.1 itself generated — see test/fixtures/):
//
//   leaf            = checksum(32) || signature(64) || keyHash(32)   (128 bytes)
//   leaf checksum   = SHA-256(msg)            (msg = the logged 32-byte message)
//   leaf signature  = Ed25519 over "sigsum.org/v1/tree-leaf" || 0x00 || checksum
//   leaf keyHash    = SHA-256(submitterKey)
//   leaf hash (RFC 6962) = SHA-256(0x00 || leaf)
//   origin          = "sigsum.org/v1/tree/" || hex(SHA-256(logKey))
//   checkpoint body = `${origin}\n${size}\n${base64std(rootHash)}\n`  (log-signed)
//   cosignature     = Ed25519 over `cosignature/v1\ntime ${ts}\n${checkpointBody}`
//
// Zero runtime deps (node:crypto only). Merkle inclusion reuses transparency.ts.

import { createHash, createPublicKey, verify as edVerify } from "node:crypto";
import { verifyInclusionProof } from "./transparency.js";

const LEAF_NAMESPACE = "sigsum.org/v1/tree-leaf";
const COSIG_NAMESPACE = "cosignature/v1";
const CHECKPOINT_PREFIX = "sigsum.org/v1/tree/";

function sha256(...parts: Buffer[]): Buffer {
  const h = createHash("sha256");
  for (const p of parts) h.update(p);
  return h.digest();
}

function ed25519PublicKey(rawHex: string) {
  const raw = Buffer.from(rawHex, "hex");
  if (raw.length !== 32) throw new Error("Ed25519 key must be 32 bytes");
  return createPublicKey({
    key: { kty: "OKP", crv: "Ed25519", x: raw.toString("base64url") },
    format: "jwk",
  });
}

function edOk(keyHex: string, msg: Buffer, sigHex: string): boolean {
  try {
    return edVerify(null, msg, ed25519PublicKey(keyHex), Buffer.from(sigHex, "hex"));
  } catch {
    return false;
  }
}

const hexEq = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** Sigsum checkpoint origin = "sigsum.org/v1/tree/" + hex(SHA-256(logKey)). */
export function sigsumOrigin(logKeyHex: string): string {
  return CHECKPOINT_PREFIX + sha256(Buffer.from(logKeyHex, "hex")).toString("hex");
}

/** The log-signed checkpoint body: `${origin}\n${size}\n${base64std(rootHash)}\n`. */
export function sigsumCheckpointBody(origin: string, size: number, rootHashHex: string): string {
  const root64 = Buffer.from(rootHashHex, "hex").toString("base64");
  return `${origin}\n${size}\n${root64}\n`;
}

export interface SigsumWitnessPolicy {
  /** The Sigsum log's Ed25519 public key (hex). */
  logKeyHex: string;
  /** Trusted witness Ed25519 public keys (hex). */
  witnessKeysHex: string[];
  /** Minimum number of distinct valid witness cosignatures required. */
  threshold: number;
}

export interface SigsumProofInput {
  /** The logged 32-byte message (hex); leaf checksum = SHA-256(msg). */
  msgHex: string;
  /** Expected submitter Ed25519 key (hex), pinned by the caller. */
  submitterKeyHex: string;
  leaf: { signatureHex: string; keyHashHex: string };
  sth: { size: number; rootHashHex: string; signatureHex: string };
  cosignatures: { keyHashHex: string; timestamp: number; signatureHex: string }[];
  /** Inclusion proof; omit when size === 1 (root === leafHash). */
  inclusion?: { leafIndex: number; pathHex: string[] };
}

export interface SigsumVerifyResult {
  ok: boolean;
  errors: string[];
  /** Number of distinct configured witnesses whose cosignature verified. */
  validCosigners: number;
}

/**
 * Verify a Sigsum proof: the submitter signed the leaf, the log signed the tree
 * head, ≥ `threshold` configured witnesses cosigned it, and the leaf is included
 * in that tree. Returns false (never throws) on any failure, so it is safe as a
 * gate. This is the trust-anchor-removing core of (B): a manifest the publisher
 * signed but did NOT publicly log cannot satisfy this.
 */
export function verifySigsumProof(
  p: SigsumProofInput,
  policy: SigsumWitnessPolicy,
): SigsumVerifyResult {
  const errors: string[] = [];
  let validCosigners = 0;
  try {
    // 1. Leaf: checksum = SHA-256(msg); submitter keyHash + signature.
    const checksum = sha256(Buffer.from(p.msgHex, "hex"));
    const expectKeyHash = sha256(Buffer.from(p.submitterKeyHex, "hex")).toString("hex");
    if (!hexEq(expectKeyHash, p.leaf.keyHashHex)) {
      errors.push("leaf keyHash does not match SHA-256(submitterKey)");
    }
    const leafSigned = Buffer.concat([
      Buffer.from(LEAF_NAMESPACE, "utf8"),
      Buffer.from([0]),
      checksum,
    ]);
    if (!edOk(p.submitterKeyHex, leafSigned, p.leaf.signatureHex)) {
      errors.push("leaf signature (submitter) is invalid");
    }

    // 2. RFC 6962 leaf hash over the 128-byte binary leaf.
    const leaf128 = Buffer.concat([
      checksum,
      Buffer.from(p.leaf.signatureHex, "hex"),
      Buffer.from(p.leaf.keyHashHex, "hex"),
    ]);
    if (leaf128.length !== 128) errors.push("leaf binary is not 128 bytes");
    const leafHash = sha256(Buffer.from([0]), leaf128);

    // 3. Log-signed tree head over the checkpoint body.
    const origin = sigsumOrigin(policy.logKeyHex);
    const body = Buffer.from(
      sigsumCheckpointBody(origin, p.sth.size, p.sth.rootHashHex),
      "utf8",
    );
    if (!edOk(policy.logKeyHex, body, p.sth.signatureHex)) {
      errors.push("log tree-head signature is invalid");
    }

    // 4. Inclusion: size 1 → root must equal leafHash; else verify the path.
    const root = Buffer.from(p.sth.rootHashHex, "hex");
    if (p.sth.size === 1) {
      if (!root.equals(leafHash)) errors.push("size-1 tree: root != leafHash");
    } else if (!p.inclusion) {
      errors.push("missing inclusion proof for size > 1");
    } else {
      const ok = verifyInclusionProof(
        {
          leafHash,
          index: BigInt(p.inclusion.leafIndex),
          treeSize: BigInt(p.sth.size),
          proof: p.inclusion.pathHex.map((h) => Buffer.from(h, "hex")),
        },
        root,
      );
      if (!ok) errors.push("inclusion proof is invalid");
    }

    // 5. Witness cosignatures: ≥ threshold DISTINCT configured witnesses.
    const witnessByHash = new Map<string, string>();
    for (const wk of policy.witnessKeysHex) {
      witnessByHash.set(sha256(Buffer.from(wk, "hex")).toString("hex"), wk);
    }
    const counted = new Set<string>();
    for (const cs of p.cosignatures) {
      const kh = cs.keyHashHex.toLowerCase();
      const wk = witnessByHash.get(kh);
      if (!wk || counted.has(kh)) continue;
      const cosigned = Buffer.from(
        `${COSIG_NAMESPACE}\ntime ${cs.timestamp}\n${sigsumCheckpointBody(origin, p.sth.size, p.sth.rootHashHex)}`,
        "utf8",
      );
      if (edOk(wk, cosigned, cs.signatureHex)) {
        counted.add(kh);
        validCosigners++;
      }
    }
    if (validCosigners < policy.threshold) {
      errors.push(
        `insufficient witness cosignatures: ${validCosigners} < threshold ${policy.threshold}`,
      );
    }
  } catch (e) {
    errors.push((e as Error).message);
  }
  return { ok: errors.length === 0, errors, validCosigners };
}
