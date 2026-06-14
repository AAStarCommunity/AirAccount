// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file for details.
// SPDX-License-Identifier: Apache-2.0
//
// Validates the verification crypto path by simulating exactly what the OP-TEE
// attestation PTA does: sign SHA256(nonce || ta_measurement) with RSA-PSS
// (SHA-256, salt 32), then export the modulus/exponent the way GET_PUBKEY does.
//
// Run after `pnpm build`:  node --test

import test from "node:test";
import assert from "node:assert/strict";
import {
  generateKeyPairSync,
  createSign,
  createHash,
  sign as cryptoSign,
  constants,
  randomBytes,
} from "node:crypto";

import { verifyAttestation } from "../dist/index.js";

const TEE_ALG = 0x70414930;

// Build evidence the way the device would, signing with `key`.
function makeEvidence({ nonce, measurement, privateKey, publicKey }) {
  // PTA signs the digest of (nonce || measurement) with PSS/SHA-256/salt=32.
  const signedMessage = Buffer.concat([nonce, measurement]);
  const signature = cryptoSign("sha256", signedMessage, {
    key: privateKey,
    padding: constants.RSA_PKCS1_PSS_PADDING,
    saltLength: 32,
  });
  // GET_PUBKEY returns raw big-endian n and e — recover them from the JWK.
  const jwk = publicKey.export({ format: "jwk" });
  const mod = Buffer.from(jwk.n, "base64url");
  const exp = Buffer.from(jwk.e, "base64url");
  return {
    schema: "airaccount.attestation.v1",
    nonce: nonce.toString("hex"),
    ta_uuid: "4319f3510b244097b65980ee4f824cdd",
    ta_measurement: measurement.toString("hex"),
    signature: signature.toString("hex"),
    attest_pubkey_exp: exp.toString("hex"),
    attest_pubkey_mod: mod.toString("hex"),
    sig_alg: TEE_ALG,
    ree_time_secs: 1_700_000_000,
  };
}

function freshKey() {
  return generateKeyPairSync("rsa", { modulusLength: 2048 });
}

test("valid evidence passes all checks", () => {
  const { privateKey, publicKey } = freshKey();
  const nonce = randomBytes(32);
  const measurement = randomBytes(32);
  const ev = makeEvidence({ nonce, measurement, privateKey, publicKey });
  const fp = createHash("sha256")
    .update(Buffer.from(ev.attest_pubkey_mod, "hex"))
    .digest("hex");

  const r = verifyAttestation(ev, {
    expectedNonceHex: nonce.toString("hex"),
    expectedMeasurementsHex: [measurement.toString("hex")],
    pinnedKeyFingerprintsHex: [fp],
  });
  assert.equal(r.ok, true, r.errors.join("; "));
  assert.equal(r.signatureValid, true);
  assert.equal(r.warnings.length, 0);
});

test("tampered measurement fails the signature", () => {
  const { privateKey, publicKey } = freshKey();
  const nonce = randomBytes(32);
  const measurement = randomBytes(32);
  const ev = makeEvidence({ nonce, measurement, privateKey, publicKey });
  ev.ta_measurement = randomBytes(32).toString("hex"); // flip it after signing
  const r = verifyAttestation(ev, { expectedNonceHex: nonce.toString("hex") });
  assert.equal(r.signatureValid, false);
  assert.equal(r.ok, false);
});

test("replayed nonce is rejected", () => {
  const { privateKey, publicKey } = freshKey();
  const nonce = randomBytes(32);
  const measurement = randomBytes(32);
  const ev = makeEvidence({ nonce, measurement, privateKey, publicKey });
  const r = verifyAttestation(ev, {
    expectedNonceHex: randomBytes(32).toString("hex"), // we asked for a different nonce
  });
  assert.equal(r.ok, false);
  assert.ok(r.errors.some((e) => e.includes("nonce")));
});

test("unpinned key fails when pin list is supplied", () => {
  const { privateKey, publicKey } = freshKey();
  const nonce = randomBytes(32);
  const measurement = randomBytes(32);
  const ev = makeEvidence({ nonce, measurement, privateKey, publicKey });
  const r = verifyAttestation(ev, {
    expectedNonceHex: nonce.toString("hex"),
    expectedMeasurementsHex: [measurement.toString("hex")],
    pinnedKeyFingerprintsHex: ["00".repeat(32)], // wrong pin
  });
  assert.equal(r.signatureValid, true); // sig is fine
  assert.equal(r.ok, false); // but trust root mismatch
  assert.ok(r.errors.some((e) => e.includes("fingerprint")));
});

test("wrong algorithm id is rejected", () => {
  const { privateKey, publicKey } = freshKey();
  const nonce = randomBytes(32);
  const measurement = randomBytes(32);
  const ev = makeEvidence({ nonce, measurement, privateKey, publicKey });
  ev.sig_alg = 0x12345678;
  const r = verifyAttestation(ev, { expectedNonceHex: nonce.toString("hex") });
  assert.equal(r.ok, false);
  assert.ok(r.errors.some((e) => e.includes("sig_alg")));
});
