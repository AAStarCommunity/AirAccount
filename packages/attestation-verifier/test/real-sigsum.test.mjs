// Created: 2026-06-16
//
// Verify a REAL proof from the public Sigsum log test.sigsum.org/barreleye
// (policy sigsum-test1-2025), captured 2026-06-16. This exercises the full
// stack against production infrastructure: parse the wire `.proof`, verify a
// real multi-node RFC 6962 inclusion proof (tree size ~181k), a real log
// tree-head signature, and real witness cosignatures. Self-contained — the
// proof + pinned keys verify offline, no network at test time.
import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { parseSigsumProof, verifySigsumProof } from "../dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const proofText = readFileSync(join(here, "fixtures/real-sigsum-proof.proof"), "utf8");
const ctx = JSON.parse(readFileSync(join(here, "fixtures/real-sigsum-context.json"), "utf8"));

test("real public-log proof: logged message == SHA-256(manifest body) [binding]", () => {
  const msg = createHash("sha256").update(Buffer.from(ctx.manifestBodyJSON, "utf8")).digest("hex");
  assert.equal(msg, ctx.msgHex);
});

test("real public-log proof parses and verifies under sigsum-test1-2025 policy", () => {
  const input = parseSigsumProof(proofText, {
    msgHex: ctx.msgHex,
    submitterKeyHex: ctx.submitterKeyHex,
  });
  // It really is a large tree with a real multi-node inclusion path.
  assert.ok(input.sth.size > 1000, "expected a real (large) tree size");
  assert.ok(input.inclusion && input.inclusion.pathHex.length > 1, "expected a multi-node inclusion path");

  const r = verifySigsumProof(input, ctx.policy);
  assert.deepEqual(r.errors, []);
  assert.equal(r.ok, true);
  assert.ok(r.validCosigners >= ctx.policy.threshold, `validCosigners ${r.validCosigners} >= ${ctx.policy.threshold}`);
});

test("real proof: a tampered inclusion node breaks verification", () => {
  const input = parseSigsumProof(proofText, {
    msgHex: ctx.msgHex,
    submitterKeyHex: ctx.submitterKeyHex,
  });
  const bad = Buffer.from(input.inclusion.pathHex[0], "hex");
  bad[0] ^= 0xff;
  input.inclusion.pathHex[0] = bad.toString("hex");
  const r = verifySigsumProof(input, ctx.policy);
  assert.equal(r.ok, false);
  assert.match(r.errors.join(";"), /inclusion proof is invalid/);
});

test("real proof: wrong logged message (binding mismatch upstream) breaks the leaf", () => {
  const input = parseSigsumProof(proofText, {
    msgHex: "00".repeat(32), // not what was logged
    submitterKeyHex: ctx.submitterKeyHex,
  });
  const r = verifySigsumProof(input, ctx.policy);
  assert.equal(r.ok, false); // checksum changes → leaf hash changes → inclusion fails
});
