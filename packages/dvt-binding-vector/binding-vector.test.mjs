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

// Canonical userOpHash values pinned from the cross-repo source of truth:
// airaccount-contract/test/HashToG2Golden.t.sol (vec1 / vec2). The whole point
// of u0/u1 is that they are byte-identical to those, so the #68 user-auth
// binding and the on-chain BLS hashToG2 vectors refer to the SAME operation.
// Pinning them here makes that alignment MACHINE-CHECKED, not just prose: edit
// the JSON's userOpHash and this test fails (reviewer note on PR #81).
// A shared cross-repo vector file imported by airaccount-contract / SDK is the
// stronger follow-up — tracked on #42, not built here (we don't author other
// repos' CI).
const CANONICAL_HASH_TO_G2_GOLDEN = {
  u0: '0x1111111111111111111111111111111111111111111111111111111111111111', // vec1
  u1: '0x8bb1b199f427dfc49e5fe40f2f3278cb1a48587824b78263051c8c4d81d77a81', // vec2
};

test('u0/u1 stay byte-identical to airaccount-contract HashToG2Golden vec1/vec2', () => {
  const seen = new Set();
  for (const v of vec.vectors) {
    const canonical = CANONICAL_HASH_TO_G2_GOLDEN[v.name];
    assert.ok(canonical, `unexpected vector name '${v.name}' (expected u0/u1)`);
    assert.equal(
      v.userOpHash.toLowerCase(),
      canonical,
      `${v.name} drifted from the canonical HashToG2Golden vector — ` +
        `the cross-repo op identity (user-auth binding vs on-chain BLS) is broken`,
    );
    seen.add(v.name);
  }
  // Both canonical vectors must be present (guards against silently dropping one).
  for (const name of Object.keys(CANONICAL_HASH_TO_G2_GOLDEN)) {
    assert.ok(seen.has(name), `missing canonical vector '${name}'`);
  }
});

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
