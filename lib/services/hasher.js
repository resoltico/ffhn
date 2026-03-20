import { createHash } from 'node:crypto';
import { DEFAULTS } from '../core/constants.js';

export function computeHash(content, algorithm = DEFAULTS.HASH_ALGORITHM) {
    if (typeof content !== 'string') {
        throw new Error('Content must be a string');
    }

    return createHash(algorithm)
        .update(content, 'utf8')
        .digest('hex');
}

export function compareHashes(currentHash, previousHash) {
    return currentHash !== previousHash;
}
