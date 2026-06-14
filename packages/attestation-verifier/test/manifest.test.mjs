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
const PINNED = manifest.publisher_key;
const TA_UUID = "4319f3510b244097b65980ee4f824cdd";
const STRICT = "188359595eab3c9e25dbd6126f11898a5b8e3fbf8d3a303a1be2676cca6071a1";
const REVOKED = "00000000000000000000000000000000000000000000000000000000deadbeef";

test("valid manifest verifies and yields current measurement", () => {
  const r = verifyMeasurementManifest(manifest, PINNED, { expectedTaUuidHex: TA_UUID });
  assert.equal(r.ok, true, r.errors.join("; "));
  assert.equal(r.sequence, 1);
  assert.deepEqual(r.currentMeasurementsHex, [STRICT]);
});

test("revoked measurement is excluded from measurementsHex", () => {
  const r = verifyMeasurementManifest(manifest, PINNED, { expectedTaUuidHex: TA_UUID });
  assert.equal(r.ok, true, r.errors.join("; "));
  assert.ok(r.measurementsHex.includes(STRICT));
  assert.ok(!r.measurementsHex.includes(REVOKED), "revoked must not appear");
});

test("downgrade (sequence below floor) is rejected", () => {
  const r = verifyMeasurementManifest(manifest, PINNED, { minSequence: 2 });
  assert.equal(r.ok, false);
  assert.ok(r.errors.some((e) => e.includes("sequence")));
});

test("equal-or-higher sequence passes the floor", () => {
  const r = verifyMeasurementManifest(manifest, PINNED, { minSequence: 1 });
  assert.equal(r.ok, true, r.errors.join("; "));
});

test("unpinned publisher key is rejected (sig checked against PINNED key)", () => {
  const r = verifyMeasurementManifest(manifest, "00".repeat(32), { expectedTaUuidHex: TA_UUID });
  assert.equal(r.ok, false);
  assert.ok(r.errors.some((e) => e.includes("pinned")));
  assert.deepEqual(r.measurementsHex, []);
});

test("tampered measurement breaks the signature", () => {
  const tampered = JSON.parse(JSON.stringify(manifest));
  tampered.body.measurements[0].ta_measurement = "ff".repeat(32);
  const r = verifyMeasurementManifest(tampered, PINNED, { expectedTaUuidHex: TA_UUID });
  assert.equal(r.ok, false);
  assert.ok(r.errors.some((e) => e.includes("signature")));
});

test("wrong schema is rejected", () => {
  const bad = JSON.parse(JSON.stringify(manifest));
  bad.body.schema = "evil.schema.v9";
  const r = verifyMeasurementManifest(bad, PINNED, { expectedTaUuidHex: TA_UUID });
  assert.equal(r.ok, false);
  assert.ok(r.errors.some((e) => e.includes("schema")));
});

test("forged publisher key (attacker self-signs) is rejected", () => {
  const forged = JSON.parse(JSON.stringify(manifest));
  forged.publisher_key = "11".repeat(32);
  const r = verifyMeasurementManifest(forged, PINNED, { expectedTaUuidHex: TA_UUID });
  assert.equal(r.ok, false);
  assert.ok(r.errors.some((e) => e.includes("pinned") || e.includes("signature")));
});

test("wrong ta_uuid is rejected", () => {
  const r = verifyMeasurementManifest(manifest, PINNED, { expectedTaUuidHex: "aa".repeat(16) });
  assert.equal(r.ok, false);
  assert.ok(r.errors.some((e) => e.includes("ta_uuid")));
});
