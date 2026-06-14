// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements. SPDX-License-Identifier: Apache-2.0
//
// Issue #12 — sign an attestation measurement manifest (Ed25519).
//
// The publisher key signs the manifest BODY (JSON.stringify of the body object,
// matching `canonicalManifestBody` in the verifier). The private key is the
// publishing trust anchor — keep it offline; commit/publish only the PUBLIC key
// (printed below) so verifiers can pin it.
//
// Usage:
//   node scripts/sign-manifest.mjs <body.json> [signing-key.pem] [out.json]
//
//   - <body.json>      : the manifest body (schema/updated/ta_uuid/measurements)
//   - signing-key.pem  : Ed25519 PKCS#8 private key. If absent/missing, a fresh
//                        key is generated and written here (default:
//                        manifest-signing-key.pem, gitignored). USE A STABLE KEY
//                        in production so the pinned publisher key never changes.
//   - out.json         : signed manifest output (default: <body dir>/attestation-measurements.json)

import {
  generateKeyPairSync,
  createPrivateKey,
  createPublicKey,
  sign as edSign,
} from "node:crypto";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";

const [, , bodyPath, keyPathArg, outArg] = process.argv;
if (!bodyPath) {
  console.error("usage: node sign-manifest.mjs <body.json> [signing-key.pem] [out.json]");
  process.exit(1);
}
const keyPath = keyPathArg || "manifest-signing-key.pem";
const outPath = outArg || join(dirname(bodyPath), "attestation-measurements.json");

// Load or generate the Ed25519 signing key.
let privateKey;
if (existsSync(keyPath)) {
  privateKey = createPrivateKey(readFileSync(keyPath, "utf8"));
  console.error(`[sign-manifest] loaded signing key from ${keyPath}`);
} else {
  const kp = generateKeyPairSync("ed25519");
  privateKey = kp.privateKey;
  writeFileSync(keyPath, privateKey.export({ type: "pkcs8", format: "pem" }));
  console.error(`[sign-manifest] generated NEW signing key → ${keyPath} (keep offline!)`);
}

// Derive the raw 32-byte public key (hex) — this is what verifiers PIN.
const pub = createPublicKey(privateKey);
const jwk = pub.export({ format: "jwk" }); // { kty:'OKP', crv:'Ed25519', x: base64url }
const publisherKeyHex = Buffer.from(jwk.x, "base64url").toString("hex");

const body = JSON.parse(readFileSync(bodyPath, "utf8"));
// Sign EXACTLY JSON.stringify(body) — must match verifier's canonicalManifestBody.
const msg = Buffer.from(JSON.stringify(body), "utf8");
const signature = edSign(null, msg, privateKey).toString("hex");

const manifest = { body, publisher_key: publisherKeyHex, signature };
writeFileSync(outPath, JSON.stringify(manifest, null, 2) + "\n");

console.error(`[sign-manifest] wrote ${outPath}`);
console.log(`publisher_key (pin this in verifiers): ${publisherKeyHex}`);
