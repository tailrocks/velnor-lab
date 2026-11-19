'use strict';
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { tokenData } = require('./index.js');

// An unsigned token with the claims a job token carries. The signature and the
// identity claims are fakes that must never appear in the observation.
function tokenWith(claims) {
  const part = (value) => Buffer.from(JSON.stringify(value)).toString('base64url');
  return `${part({ alg: 'none' })}.${part(claims)}.SIGNATURE-VALUE`;
}

const claims = {
  repository: 'tailrocks/velnor-lab',
  ref: 'refs/pull/1234/merge',
  defaultRef: 'refs/heads/main',
  sid: 'SECRET-SID-VALUE',
  nameid: 'SECRET-NAME-VALUE',
  ac: JSON.stringify([
    { Permission: 3, Scope: 'refs/pull/1234/merge' },
    { Permission: 1, Scope: 'refs/heads/main' },
  ]),
  scp: 'Actions.Results:1:2 Actions.Runner:1:3 Actions.Results:1:4',
  exp: 4102444800,
};

test('the repository, ref and defaultRef claims are recorded with their values', () => {
  const data = tokenData(tokenWith(claims));
  assert.equal(data.claims.repository, 'tailrocks/velnor-lab');
  assert.equal(data.claims.ref, 'refs/pull/1234/merge');
  assert.equal(data.claims.defaultRef, 'refs/heads/main');
});

test('the cache scopes and the scope names are recorded with their values', () => {
  const data = tokenData(tokenWith(claims));
  assert.deepEqual(data.cache_scopes, [
    { permission: 3, scope: 'refs/pull/1234/merge' },
    { permission: 1, scope: 'refs/heads/main' },
  ]);
  assert.deepEqual(data.scp, ['Actions.Results', 'Actions.Runner']);
});

test('the other claims are recorded by type only', () => {
  const data = tokenData(tokenWith(claims));
  assert.equal(data.claims.sid, 'string');
  assert.equal(data.claims.nameid, 'string');
  assert.equal(data.claims.exp, 'number');
});

test('a value claim the token does not carry is recorded as absent', () => {
  const { defaultRef, ...rest } = claims;
  const data = tokenData(tokenWith(rest));
  assert.equal(data.claims.defaultRef, 'absent');
});

test('the observation never contains the token, its signature or the identity claims', () => {
  const token = tokenWith(claims);
  const text = JSON.stringify(tokenData(token));
  assert.ok(!text.includes(token));
  assert.ok(!text.includes('SIGNATURE-VALUE'));
  assert.ok(!text.includes('SECRET-SID-VALUE'));
  assert.ok(!text.includes('SECRET-NAME-VALUE'));
  assert.ok(!text.includes(token.split('.')[1]));
});

test('a token whose payload cannot be decoded is an error', () => {
  assert.throws(() => tokenData('not-a-token'), /could not be decoded/);
  assert.throws(() => tokenData(undefined), /could not be decoded/);
});
