// Executable golden vector for AirAccount #70 / #68 payload-commitment binding.
// Recomputes the TA's commitment formula and asserts it byte-for-byte against
// the published vector. CI goes RED on any drift — text agreement hides drift,
// vectors don't.
//
// Run: node --test   (zero dependencies; uses node:crypto / node:test)
import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const vec = JSON.parse(readFileSync(join(here, 'dvt-binding-vectors.json'), 'utf8'));

const buf = (hex) => Buffer.from(hex.replace(/^0x/, ''), 'hex');

test('#68 commitment == SHA-256(nonce || userOpHash) for canonical (u0,u1)', () => {
  const nonce = buf(vec.nonce);
  assert.equal(nonce.length, 32, 'nonce must be 32 bytes');

  for (const v of vec.vectors) {
    const digest = buf(v.userOpHash);
    assert.equal(digest.length, 32, `${v.name}: userOpHash must be 32 bytes`);

    // Mirror the TA exactly: update(nonce) then update(payload) == nonce||payload.
    const got = '0x' + createHash('sha256').update(nonce).update(digest).digest('hex');

    assert.equal(
      got,
      v.expectedChallenge.toLowerCase(),
      `${v.name}: recomputed commitment must equal the published vector — ` +
        `if this fails, the user-authorization binding drifted from the TA (#68) formula`,
    );
  }
});
