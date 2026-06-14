// SPDX-License-Identifier: Apache-2.0
// Issue #12 — measurement manifest verification tests. Run after `pnpm build`.

import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { verifyMeasurementManifest } from "../dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(
  readFileSync(join(here, "..", ".well-known-attestation-measurements.json"), "utf8"),
);
const PINNED = manifest.publisher_key; // the real publisher key for these tests
const TA_UUID = "4319f3510b244097b65980ee4f824cdd";

test("valid manifest verifies and yields measurements", () => {
  const r = verifyMeasurementManifest(manifest, PINNED, TA_UUID);
  assert.equal(r.ok, true, r.errors.join("; "));
  assert.ok(r.measurementsHex.includes("188359595eab3c9e25dbd6126f11898a5b8e3fbf8d3a303a1be2676cca6071a1"));
  assert.deepEqual(r.currentMeasurementsHex, [
    "188359595eab3c9e25dbd6126f11898a5b8e3fbf8d3a303a1be2676cca6071a1",
  ]);
});

test("unpinned publisher key is rejected", () => {
  const r = verifyMeasurementManifest(manifest, "00".repeat(32), TA_UUID);
  assert.equal(r.ok, false);
  assert.ok(r.errors.some((e) => e.includes("pinned")));
  assert.deepEqual(r.measurementsHex, []); // no measurements exposed on failure
});

test("tampered measurement breaks the signature", () => {
  const tampered = JSON.parse(JSON.stringify(manifest));
  tampered.body.measurements[0].ta_measurement = "ff".repeat(32);
  const r = verifyMeasurementManifest(tampered, PINNED, TA_UUID);
  assert.equal(r.ok, false);
  assert.ok(r.errors.some((e) => e.includes("signature")));
});

test("wrong ta_uuid is rejected", () => {
  const r = verifyMeasurementManifest(manifest, PINNED, "aa".repeat(16));
  assert.equal(r.ok, false);
  assert.ok(r.errors.some((e) => e.includes("ta_uuid")));
});

test("substituted publisher key (self-signed by attacker) is rejected", () => {
  // Attacker re-signs a tampered manifest with their OWN key and swaps publisher_key.
  // Pinning defeats it: publisher_key != pinned → rejected before/again at sig.
  const forged = JSON.parse(JSON.stringify(manifest));
  forged.publisher_key = "11".repeat(32);
  const r = verifyMeasurementManifest(forged, PINNED, TA_UUID);
  assert.equal(r.ok, false);
  assert.ok(r.errors.some((e) => e.includes("pinned")));
});
