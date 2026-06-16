// Created: 2026-06-16
//
// B-2 publish side (issue #87): submit a signed measurement manifest to a Sigsum
// transparency log and emit a verifiable proof sidecar. Run at release time,
// after sign-manifest.mjs produces the Ed25519-signed manifest.
//
// It logs SHA-256(canonical manifest body) (the same value verifyMeasurementManifest
// binds against), waits for the log + ≥quorum witnesses, then converts the wire
// `.proof` into the JSON shape our verifier consumes (SigsumProofInput + policy).
//
// Wraps the official `sigsum-submit` tool (it handles submit → log → witness
// cosigning → policy-satisfying proof collection). Install: see
// test/fixtures/README.md.
//
// Usage:
//   node scripts/submit-manifest-to-sigsum.mjs \
//     --manifest path/to/attestation-measurements.json \
//     --submit-key path/to/submit-key \
//     --policy path/to/policy \
//     [--sigsum-submit sigsum-submit] [--out attestation-measurements-proof.json]
//
// The policy file pins the log + witnesses + quorum (e.g. public sigsum-test1-2025,
// or a production policy). The submit key is the publisher's Sigsum submitter key.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseSigsumProof, canonicalManifestBody } from "../dist/index.js";

function arg(name, def) {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && i + 1 < process.argv.length ? process.argv[i + 1] : def;
}

const manifestPath = arg("manifest");
const submitKey = arg("submit-key");
const policy = arg("policy");
const sigsumSubmit = arg("sigsum-submit", "sigsum-submit");
const outPath = arg("out", "attestation-measurements-proof.json");
if (!manifestPath || !submitKey || !policy) {
  console.error("required: --manifest <file> --submit-key <file> --policy <file>");
  process.exit(2);
}

const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const canon = canonicalManifestBody(manifest.body);
const msg = createHash("sha256").update(Buffer.from(canon, "utf8")).digest(); // 32 bytes

const dir = mkdtempSync(join(tmpdir(), "sigsum-"));
const msgFile = join(dir, "msg.bin");
const proofFile = join(dir, "manifest.proof");
writeFileSync(msgFile, msg);

console.error(`[submit] logging SHA-256(manifest body) = ${msg.toString("hex")}`);
// --raw-hash: msgFile already holds the 32-byte message to log.
execFileSync(sigsumSubmit, ["--raw-hash", "-k", submitKey, "-p", policy, "-o", proofFile, msgFile], {
  stdio: ["ignore", "inherit", "inherit"],
});

// The submitter public key (hex) — verifier needs it to check the leaf keyHash.
const submitterKeyHex = execFileSync("sigsum-key", ["to-hex", "-k", `${submitKey}.pub`])
  .toString()
  .trim();

const proofText = readFileSync(proofFile, "utf8");
const proof = parseSigsumProof(proofText, { msgHex: msg.toString("hex"), submitterKeyHex });

writeFileSync(
  outPath,
  JSON.stringify(
    {
      _note: "Sigsum transparency proof for the measurement manifest. Feed as verifyMeasurementManifest opts.transparency.proof; pair with the matching witness policy.",
      proof,
    },
    null,
    2,
  ) + "\n",
);
console.error(`[submit] wrote ${outPath} (size=${proof.sth.size}, cosignatures=${proof.cosignatures.length})`);
