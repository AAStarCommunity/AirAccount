// Created: 2026-06-16
//
// B-4 transparency monitor (issue #87). Run on a schedule (cron GitHub Action),
// NOT a daemon. It makes log-recorded misuse *noticed*:
//
//   1. fetch the LIVE served manifest + proof from kms.aastar.io/.well-known/
//   2. verify Tier-2: pinned publisher key + Sigsum transparency proof (witness
//      policy) + body binding (proof covers exactly this manifest)
//   3. cross-check the served manifest equals the repo-committed reference
//      (git source of truth) — detects a swapped / unlogged / rogue manifest
//
// Any failure exits non-zero so the scheduled workflow goes red + alerts.
//
// Usage:
//   node scripts/monitor-manifest.mjs \
//     [--base-url https://kms.aastar.io] \
//     --ref-manifest ../../kms/host/attestation-measurements.json \
//     --ref-proof    ../../kms/host/attestation-measurements-proof.json \
//     [--publisher-key <hex>] [--local]
//
// --local: skip the network fetch and verify the reference files directly
// (used in unit tests / before the endpoint is live).
import { readFileSync } from "node:fs";
import { verifyMeasurementManifest } from "../dist/index.js";

function arg(name, def) {
  const i = process.argv.indexOf(`--${name}`);
  if (i >= 0 && i + 1 < process.argv.length && !process.argv[i + 1].startsWith("--")) return process.argv[i + 1];
  return process.argv.includes(`--${name}`) ? true : def;
}

// Pinned, public trust parameters (sigsum-test1-2025 public test policy).
const DEFAULT_PUBLISHER_KEY = "d0c380e835db4a7accff631961eb860980491af069278933037f970d5801711b";
const POLICY = {
  logKeyHex: "4644af2abd40f4895a003bca350f9d5912ab301a49c77f13e5b6d905c20a5fe6",
  witnessKeysHex: [
    "1c25f8a44c635457e2e391d1efbca7d4c2951a0aef06225a881e46b98962ac6c",
    "28c92a5a3a054d317c86fc2eeb6a7ab2054d6217100d0be67ded5b74323c5806",
    "f4855a0f46e8a3e23bb40faf260ee57ab8a18249fa402f2ca2d28a60e1a3130e",
  ],
  threshold: 2,
};

const baseUrl = arg("base-url", "https://kms.aastar.io");
const refManifestPath = arg("ref-manifest");
const refProofPath = arg("ref-proof");
const publisherKey = arg("publisher-key", DEFAULT_PUBLISHER_KEY);
const local = arg("local", false);

if (!refManifestPath || !refProofPath) {
  console.error("required: --ref-manifest <file> --ref-proof <file>");
  process.exit(2);
}

const fail = (msg) => {
  console.error(`MONITOR FAIL: ${msg}`);
  process.exit(1);
};

async function getJson(url) {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`HTTP ${r.status} for ${url}`);
  return r.json();
}

const refManifest = JSON.parse(readFileSync(refManifestPath, "utf8"));
const refProof = JSON.parse(readFileSync(refProofPath, "utf8"));

let manifest = refManifest;
let proofSidecar = refProof;

if (!local) {
  manifest = await getJson(`${baseUrl}/.well-known/attestation-measurements.json`);
  proofSidecar = await getJson(`${baseUrl}/.well-known/attestation-measurements-proof.json`);
  // Served must equal the repo source of truth (detects a swap).
  if (JSON.stringify(manifest) !== JSON.stringify(refManifest)) {
    fail("served manifest differs from the repo-committed reference (possible swap)");
  }
}

const proof = proofSidecar.proof ?? proofSidecar;
const r = verifyMeasurementManifest(manifest, publisherKey, {
  expectedTaUuidHex: manifest.body?.ta_uuid,
  transparency: { proof, policy: POLICY },
});
if (!r.ok) fail(`Tier-2 verification failed: ${r.errors.join("; ")}`);

console.log(
  `MONITOR OK: manifest seq ${r.sequence}, ${r.measurementsHex.length} active measurement(s), ` +
    `transparency proof valid (size ${proof.sth.size}, cosigners verified).`,
);
