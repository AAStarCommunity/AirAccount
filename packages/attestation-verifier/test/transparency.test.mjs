// Created: 2026-06-16
//
// Cross-check the RFC 6962 inclusion-proof verifier (transparency.ts) against an
// INDEPENDENT recursive reference implementation of the Merkle tree head + audit
// path (RFC 6962 §2.1). Two different code paths agreeing on every (size, index)
// gives high confidence the proof-chain algorithm is correct.
import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  hashLeaf,
  rootFromInclusionProof,
  verifyInclusionProof,
} from "../dist/index.js";

// ── Independent RFC 6962 reference (recursive; different code path) ──
const refLeaf = (d) =>
  createHash("sha256").update(Buffer.from([0x00])).update(d).digest();
const refNode = (l, r) =>
  createHash("sha256").update(Buffer.from([0x01])).update(l).update(r).digest();

// Largest power of two strictly less than n (n >= 2).
function splitK(n) {
  let p = 1;
  while (p * 2 < n) p *= 2;
  return p;
}
// RFC 6962 Merkle Tree Hash over an array of leaf-data Buffers.
function refRoot(leaves) {
  if (leaves.length === 1) return refLeaf(leaves[0]);
  const k = splitK(leaves.length);
  return refNode(refRoot(leaves.slice(0, k)), refRoot(leaves.slice(k)));
}
// RFC 6962 audit path for leaf m within `leaves`.
function refPath(m, leaves) {
  if (leaves.length === 1) return [];
  const k = splitK(leaves.length);
  if (m < k) return [...refPath(m, leaves.slice(0, k)), refRoot(leaves.slice(k))];
  return [...refPath(m - k, leaves.slice(k)), refRoot(leaves.slice(0, k))];
}

const mkLeaves = (n) =>
  Array.from({ length: n }, (_, i) => Buffer.from(`leaf-${i}`, "utf8"));

test("rootFromInclusionProof matches independent RFC 6962 reference for all (size, index) up to 16", () => {
  for (let n = 1; n <= 16; n++) {
    const leaves = mkLeaves(n);
    const root = refRoot(leaves);
    for (let m = 0; m < n; m++) {
      const input = {
        leafHash: hashLeaf(leaves[m]),
        index: BigInt(m),
        treeSize: BigInt(n),
        proof: refPath(m, leaves),
      };
      assert.deepEqual(
        rootFromInclusionProof(input),
        root,
        `reconstructed root mismatch at size=${n} index=${m}`,
      );
      assert.equal(verifyInclusionProof(input, root), true, `verify failed size=${n} index=${m}`);
    }
  }
});

test("single-leaf tree: empty proof, root == leaf hash", () => {
  const leaf = Buffer.from("only", "utf8");
  const input = { leafHash: hashLeaf(leaf), index: 0n, treeSize: 1n, proof: [] };
  assert.deepEqual(rootFromInclusionProof(input), refLeaf(leaf));
  assert.equal(verifyInclusionProof(input, refLeaf(leaf)), true);
});

test("tampered root is rejected", () => {
  const leaves = mkLeaves(7);
  const root = refRoot(leaves);
  const input = {
    leafHash: hashLeaf(leaves[3]),
    index: 3n,
    treeSize: 7n,
    proof: refPath(3, leaves),
  };
  const bad = Buffer.from(root);
  bad[0] ^= 0xff;
  assert.equal(verifyInclusionProof(input, bad), false);
});

test("wrong index for the same leaf is rejected", () => {
  const leaves = mkLeaves(8);
  const root = refRoot(leaves);
  // proof built for index 3, but claim index 4 → must not verify
  const input = {
    leafHash: hashLeaf(leaves[3]),
    index: 4n,
    treeSize: 8n,
    proof: refPath(3, leaves),
  };
  assert.equal(verifyInclusionProof(input, root), false);
});

test("malformed proof length is rejected (not mistaken for mismatch)", () => {
  const leaves = mkLeaves(5);
  const root = refRoot(leaves);
  const proof = refPath(2, leaves);
  const input = {
    leafHash: hashLeaf(leaves[2]),
    index: 2n,
    treeSize: 5n,
    proof: proof.slice(0, proof.length - 1), // drop one hash
  };
  assert.equal(verifyInclusionProof(input, root), false);
  assert.throws(() => rootFromInclusionProof(input), /malformed proof/);
});

test("index >= treeSize throws", () => {
  assert.throws(
    () => rootFromInclusionProof({ leafHash: hashLeaf(Buffer.from("x")), index: 5n, treeSize: 5n, proof: [] }),
    /index out of range/,
  );
});
