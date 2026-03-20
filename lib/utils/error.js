import { EXIT_CODES, ERROR_TYPES } from '../core/constants.js';

export class FfhnError extends Error {
    constructor(code, message, options = {}) {
        const {
            type = ERROR_TYPES.INTERNAL,
            exitCode = EXIT_CODES.INTERNAL_ERROR,
            details = null,
            cause = undefined
        } = options;

        super(message, cause === undefined ? undefined : { cause });
        this.name = 'FfhnError';
        this.code = code;
        this.type = type;
        this.exitCode = exitCode;
        this.details = details;
    }
}

export function normalizeError(error) {
    if (error instanceof Error) {
        return error;
    }

    if (typeof error === 'string') {
        return new Error(error);
    }

    if (error && typeof error === 'object') {
        const message = typeof error.message === 'string' && error.message.length > 0
            ? error.message
            : String(error);
        const normalizedError = new Error(message);

        if (typeof error.name === 'string' && error.name.length > 0) {
            normalizedError.name = error.name;
        }
        if (Object.hasOwn(error, 'status')) {
            normalizedError.status = error.status;
        }
        if (Object.hasOwn(error, 'code')) {
            normalizedError.code = error.code;
        }
        if (Object.hasOwn(error, 'type')) {
            normalizedError.type = error.type;
        }
        if (Object.hasOwn(error, 'exitCode')) {
            normalizedError.exitCode = error.exitCode;
        }
        if (Object.hasOwn(error, 'details')) {
            normalizedError.details = error.details;
        }

        return normalizedError;
    }

    return new Error(String(error));
}

export function toFfhnError(error, fallback) {
    if (error instanceof FfhnError) {
        return error;
    }

    const normalizedError = normalizeError(error);
    return new FfhnError(
        fallback.code,
        normalizedError.message,
        {
            type: fallback.type,
            exitCode: fallback.exitCode,
            details: fallback.details ?? null,
            cause: normalizedError
        }
    );
}

export function serializeError(error) {
    const normalizedError = normalizeError(error);
    return {
        code: normalizedError.code || 'INTERNAL_ERROR',
        type: normalizedError.type || ERROR_TYPES.INTERNAL,
        message: normalizedError.message,
        details: normalizedError.details || null
    };
}

export function usageError(code, message, details = null, cause = undefined) {
    return new FfhnError(code, message, {
        type: ERROR_TYPES.USAGE,
        exitCode: EXIT_CODES.USAGE_ERROR,
        details,
        cause
    });
}

export function configError(code, message, details = null, cause = undefined) {
    return new FfhnError(code, message, {
        type: ERROR_TYPES.CONFIG,
        exitCode: EXIT_CODES.CONFIG_ERROR,
        details,
        cause
    });
}

export function dependencyError(code, message, details = null, cause = undefined) {
    return new FfhnError(code, message, {
        type: ERROR_TYPES.DEPENDENCY,
        exitCode: EXIT_CODES.DEPENDENCY_ERROR,
        details,
        cause
    });
}

export function targetFailure(code, type, message, details = null, cause = undefined) {
    return new FfhnError(code, message, {
        type,
        exitCode: EXIT_CODES.TARGET_FAILURE,
        details,
        cause
    });
}
