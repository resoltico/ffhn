import { executeHtmlcut } from './process.js';
import { ERROR_TYPES } from '../core/constants.js';
import { targetFailure } from '../utils/error.js';

const CRLF_LENGTH = 2;

export async function processWithHtmlcut(htmlContent, htmlcutConfig, options = {}) {
    if (typeof htmlContent !== 'string') {
        throw new TypeError('HTML input must be a string');
    }

    const result = await executeHtmlcut(htmlContent, htmlcutConfig, {
        timeoutMs: options.timeoutMs,
        baseUrl: options.baseUrl
    });
    const output = stripTrailingLineEnding(result.stdout);

    if (output.length === 0) {
        throw targetFailure(
            'HTMLCUT_EMPTY_OUTPUT',
            ERROR_TYPES.EXTRACT,
            'HTMLCut produced empty output',
            {
                target: options.targetName || null
            }
        );
    }

    return output;
}

function stripTrailingLineEnding(value) {
    if (value.endsWith('\r\n')) {
        return value.slice(0, -CRLF_LENGTH);
    }

    if (value.endsWith('\n')) {
        return value.slice(0, -1);
    }

    return value;
}
