import test from 'node:test';
import assert from 'node:assert/strict';

import pkg from '../package.json' with { type: 'json' };
import { VERSION } from '../lib/core/version.js';
import { DEFAULTS, EXIT_CODES, STATUS, ERROR_TYPES, SCHEMA_VERSION } from '../lib/core/constants.js';

test('version module tracks package.json', () => {
    assert.equal(VERSION, pkg.version);
});

test('constants expose expected core values', () => {
    assert.equal(SCHEMA_VERSION, 1);
    assert.equal(DEFAULTS.HASH_ALGORITHM, 'sha256');
    assert.equal(DEFAULTS.STATE_FILE, 'state.json');
    assert.equal(DEFAULTS.USER_AGENT, `ffhn/${pkg.version}`);
    assert.equal(EXIT_CODES.SUCCESS, 0);
    assert.equal(EXIT_CODES.TARGET_FAILURE, 4);
    assert.equal(STATUS.INITIALIZED, 'initialized');
    assert.equal(ERROR_TYPES.EXTRACT, 'extract');
});
