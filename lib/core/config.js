import { readFile } from 'node:fs/promises';
import TOML from 'smol-toml';
import { DEFAULTS } from './constants.js';
import { validateTargetConfig } from '../utils/validation.js';
import { configError } from '../utils/error.js';

export async function loadTargetConfig(configPath) {
    let rawContent;
    try {
        rawContent = await readFile(configPath, 'utf8');
    } catch (error) {
        if (error.code === 'ENOENT') {
            throw configError('TARGET_CONFIG_MISSING', `Target config not found: ${configPath}`, {
                config_path: configPath
            });
        }

        throw configError('TARGET_CONFIG_READ_FAILED', `Failed to read target config: ${configPath}`, {
            config_path: configPath
        }, error);
    }

    let parsedConfig;
    try {
        parsedConfig = TOML.parse(rawContent);
    } catch (error) {
        const parseDetails = buildTomlParseDetails(configPath, error);
        throw configError(
            'TARGET_CONFIG_PARSE_FAILED',
            buildTomlParseMessage(configPath, parseDetails),
            parseDetails,
            error
        );
    }

    try {
        validateTargetConfig(parsedConfig);
    } catch (error) {
        throw configError('TARGET_CONFIG_INVALID', error.message, {
            config_path: configPath
        }, error);
    }

    return normalizeTargetConfig(parsedConfig);
}

export function buildTomlParseMessage(configPath, details) {
    const location = details.line === null || details.column === null
        ? ''
        : ` at line ${details.line}, column ${details.column}`;
    const reason = details.parse_summary === null
        ? ''
        : `: ${details.parse_summary}`;

    return `Failed to parse target config: ${configPath}${location}${reason}`;
}

export function buildTomlParseDetails(configPath, error) {
    const normalizedMessage = typeof error?.message === 'string'
        ? error.message.trim()
        : String(error);

    return {
        config_path: configPath,
        line: Number.isInteger(error?.line) ? error.line : null,
        column: Number.isInteger(error?.column) ? error.column : null,
        parse_summary: extractTomlSummary(normalizedMessage),
        parse_message: normalizedMessage,
        code_frame: typeof error?.codeblock === 'string' && error.codeblock.length > 0
            ? error.codeblock
            : null
    };
}

export function extractTomlSummary(message) {
    const [firstLine] = message.split('\n');
    if (!firstLine) {
        return null;
    }

    const prefix = 'Invalid TOML document: ';
    return firstLine.startsWith(prefix)
        ? firstLine.slice(prefix.length)
        : firstLine;
}

export function normalizeTargetConfig(config) {
    const request = config.request || {};

    return {
        url: config.target.url,
        extract: {
            from: config.extract.from,
            to: config.extract.to,
            pattern: config.extract.pattern ?? 'literal',
            flags: config.extract.flags ?? null,
            capture: config.extract.capture ?? 'inner',
            all: config.extract.all ?? false
        },
        request: {
            timeoutMs: request.timeout_ms ?? DEFAULTS.TIMEOUT_MS,
            maxAttempts: request.max_attempts ?? DEFAULTS.MAX_ATTEMPTS,
            retryDelayMs: request.retry_delay_ms ?? DEFAULTS.RETRY_DELAY_MS,
            userAgent: request.user_agent ?? DEFAULTS.USER_AGENT
        }
    };
}
