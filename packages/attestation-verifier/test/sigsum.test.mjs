// Created: 2026-06-16
//
// Verify sigsum.ts against a fixture that sigsum-go v0.11.1 itself generated
// (test/fixtures/sigsum-proof.json). Because the proof was produced by the
// authoritative implementation and verified here by ours, agreement proves our
// wire-format transcription (origin / checkpoint / leaf / cosignature) is exact.
import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import {
  verifySigsumProof,
  sigsumOrigin,
  sigsumCheckpointBody,
} from "../dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const fx = JSON.parse(readFileSync(join(here, "fixtures/sigsum-proof.json"), "utf8"));

const toInput = () => ({
  msgHex: fx.msg,
  submitterKeyHex: fx.submitterKey,
  leaf: { signatureHex: fx.leaf.signature, keyHashHex: fx.leaf.keyHash },
  sth: { size: fx.sth.size, rootHashHex: fx.sth.rootHash, signatureHex: fx.sth.signature },
  cosignatures: fx.cosignatures.map((c) => ({
    keyHashHex: c.keyHash,
    timestamp: c.timestamp,
    signatureHex: c.signature,
  })),
});
const policy = (threshold) => ({
  logKeyHex: fx.logKey,
  witnessKeysHex: fx.witnessKeys,
  threshold,
});

// ── format transcription must match sigsum-go byte-for-byte ──
test("origin matches sigsum-go", () => {
  assert.equal(sigsumOrigin(fx.logKey), fx.origin);
});
test("checkpoint body matches sigsum-go", () => {
  assert.equal(sigsumCheckpointBody(fx.origin, fx.sth.size, fx.sth.rootHash), fx.checkpointBody);
});
test("leaf checksum = SHA-256(msg) matches sigsum-go", () => {
  const checksum = createHash("sha256").update(Buffer.from(fx.msg, "hex")).digest("hex");
  assert.equal(checksum, fx.leaf.checksum);
});
test("RFC 6962 leaf hash matches sigsum-go (== size-1 root)", () => {
  const leaf128 = Buffer.concat([
    Buffer.from(fx.leaf.checksum, "hex"),
    Buffer.from(fx.leaf.signature, "hex"),
    Buffer.from(fx.leaf.keyHash, "hex"),
  ]);
  const leafHash = createHash("sha256").update(Buffer.from([0])).update(leaf128).digest("hex");
  assert.equal(leafHash, fx.leafHashHex);
  assert.equal(leafHash, fx.sth.rootHash);
});

// ── end-to-end verification ──
test("valid sigsum proof verifies with 2-of-2 witness policy", () => {
  const r = verifySigsumProof(toInput(), policy(2));
  assert.deepEqual(r.errors, []);
  assert.equal(r.ok, true);
  assert.equal(r.validCosigners, 2);
});

test("threshold above available witnesses fails", () => {
  const r = verifySigsumProof(toInput(), policy(3));
  assert.equal(r.ok, false);
  assert.match(r.errors.join(";"), /insufficient witness cosignatures/);
});

test("tampered root hash breaks log signature AND size-1 root check", () => {
  const inp = toInput();
  const bad = Buffer.from(inp.sth.rootHashHex, "hex");
  bad[0] ^= 0xff;
  inp.sth.rootHashHex = bad.toString("hex");
  const r = verifySigsumProof(inp, policy(2));
  assert.equal(r.ok, false);
  assert.match(r.errors.join(";"), /tree-head signature is invalid/);
});

test("tampered cosignature is not counted (drops below threshold)", () => {
  const inp = toInput();
  const bad = Buffer.from(inp.cosignatures[0].signatureHex, "hex");
  bad[0] ^= 0xff;
  inp.cosignatures[0].signatureHex = bad.toString("hex");
  const r = verifySigsumProof(inp, policy(2));
  assert.equal(r.ok, false);
  assert.equal(r.validCosigners, 1);
});

test("wrong submitter key is rejected (keyHash + signature)", () => {
  const inp = toInput();
  inp.submitterKeyHex = fx.witnessKeys[0]; // a real key, but not the submitter
  const r = verifySigsumProof(inp, policy(2));
  assert.equal(r.ok, false);
  assert.match(r.errors.join(";"), /leaf keyHash|leaf signature/);
});

test("a witness not in policy is not counted", () => {
  const inp = toInput();
  const r = verifySigsumProof(inp, {
    logKeyHex: fx.logKey,
    witnessKeysHex: [fx.witnessKeys[0]], // only one of the two configured
    threshold: 1,
  });
  assert.equal(r.ok, true);
  assert.equal(r.validCosigners, 1);
});
