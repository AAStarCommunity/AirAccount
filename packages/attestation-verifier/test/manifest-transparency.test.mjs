// Created: 2026-06-16
//
// Tier-2 integration (#87 B): verifyMeasurementManifest with the transparency
// gate. The Sigsum fixture's logged message == SHA-256(canonical body) of THIS
// exact manifest body (see test/fixtures/gen-sigsum-fixture.go, MSG_HEX), so the
// proof genuinely covers this manifest — and a proof for any other body is
// rejected by the binding check.
import test from "node:test";
import assert from "node:assert/strict";
import { generateKeyPairSync, sign as edSign } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { verifyMeasurementManifest } from "../dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const fx = JSON.parse(readFileSync(join(here, "fixtures/sigsum-proof.json"), "utf8"));

// EXACT body whose SHA-256(JSON.stringify(body)) the fixture was generated for.
// Key order here defines the canonical bytes — must not change without regenerating.
const BODY = {
  schema: "airaccount.attestation-measurements.v1",
  updated: "2026-06-16",
  sequence: 1,
  ta_uuid: "4319f3510b244097b65980ee4f824cdd",
  measurements: [
    {
      version: "v0.22.0",
      ta_measurement: "aa11bb22cc33dd44ee55ff6677889900aabbccddeeff00112233445566778899",
      status: "current",
    },
  ],
};

// Build a validly-signed manifest with a fresh publisher key (the manifest
// publisher and the Sigsum submitter are DISTINCT roles).
function signedManifest(body) {
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  const pubRaw = Buffer.from(publicKey.export({ format: "jwk" }).x, "base64url");
  const canon = Buffer.from(JSON.stringify(body), "utf8");
  const signature = edSign(null, canon, privateKey);
  return {
    manifest: { body, publisher_key: pubRaw.toString("hex"), signature: signature.toString("hex") },
    pinnedKeyHex: pubRaw.toString("hex"),
  };
}

const proof = () => ({
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
const policy = { logKeyHex: fx.logKey, witnessKeysHex: fx.witnessKeys, threshold: 2 };

test("Tier-2: manifest with a valid bound transparency proof passes", () => {
  const { manifest, pinnedKeyHex } = signedManifest(BODY);
  const r = verifyMeasurementManifest(manifest, pinnedKeyHex, {
    expectedTaUuidHex: BODY.ta_uuid,
    transparency: { proof: proof(), policy },
  });
  assert.deepEqual(r.errors, []);
  assert.equal(r.ok, true);
});

test("Tier-2 binding: a proof for a DIFFERENT body is rejected", () => {
  // Same proof, but the manifest body differs → SHA-256(body) != logged msg.
  const tampered = structuredClone(BODY);
  tampered.measurements[0].ta_measurement = "00".repeat(32);
  const { manifest, pinnedKeyHex } = signedManifest(tampered);
  const r = verifyMeasurementManifest(manifest, pinnedKeyHex, {
    transparency: { proof: proof(), policy },
  });
  assert.equal(r.ok, false);
  assert.match(r.errors.join(";"), /transparency proof does not cover this manifest/);
});

test("Tier-2: a tampered cosignature fails the gate (below threshold)", () => {
  const { manifest, pinnedKeyHex } = signedManifest(BODY);
  const p = proof();
  const bad = Buffer.from(p.cosignatures[0].signatureHex, "hex");
  bad[0] ^= 0xff;
  p.cosignatures[0].signatureHex = bad.toString("hex");
  const r = verifyMeasurementManifest(manifest, pinnedKeyHex, {
    transparency: { proof: p, policy },
  });
  assert.equal(r.ok, false);
  assert.match(r.errors.join(";"), /transparency: insufficient witness cosignatures/);
});

test("Tier-1 still works when no transparency option is supplied", () => {
  const { manifest, pinnedKeyHex } = signedManifest(BODY);
  const r = verifyMeasurementManifest(manifest, pinnedKeyHex, { expectedTaUuidHex: BODY.ta_uuid });
  assert.equal(r.ok, true);
  assert.equal(r.currentMeasurementsHex.length, 1);
});
