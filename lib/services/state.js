import { FORMAT, SCHEMA_VERSION, STATUS, ERROR_TYPES } from '../core/constants.js';
import { pathExists, readTextFile, writeTextFileAtomic } from '../adapters/filesystem.js';
import { targetFailure } from '../utils/error.js';

export async function readState(statePath) {
    if (!(await pathExists(statePath))) {
        return createDefaultState();
    }

    let rawState;
    try {
        rawState = await readTextFile(statePath);
    } catch (error) {
        throw targetFailure(
            'STATE_READ_FAILED',
            ERROR_TYPES.STATE,
            `Failed to read state file: ${statePath}`,
            {
                state_path: statePath
            },
            error
        );
    }

    let parsedState;
    try {
        parsedState = JSON.parse(rawState);
    } catch (error) {
        throw targetFailure(
            'STATE_PARSE_FAILED',
            ERROR_TYPES.STATE,
            `State file is not valid JSON: ${statePath}`,
            {
                state_path: statePath
            },
            error
        );
    }

    validateState(parsedState, statePath);
    return parsedState;
}

export async function writeState(statePath, state) {
    validateState(state, statePath);
    await writeTextFileAtomic(statePath, JSON.stringify(state, null, FORMAT.JSON_INDENT));
}

export function createDefaultState(now = new Date().toISOString()) {
    return {
        schema_version: SCHEMA_VERSION,
        created_at: now,
        last_run_at: null,
        last_success_at: null,
        last_change_at: null,
        last_error: null,
        current_hash: null,
        previous_hash: null,
        stats: {
            runs: 0,
            successes: 0,
            failures: 0,
            changes: 0,
            consecutive_failures: 0,
            consecutive_changes: 0
        }
    };
}

export function recordSuccess(state, currentHash, status, timestamp = new Date().toISOString()) {
    const priorHash = state.current_hash;

    state.last_run_at = timestamp;
    state.last_success_at = timestamp;
    state.last_error = null;
    state.current_hash = currentHash;
    state.stats.runs++;
    state.stats.successes++;
    state.stats.consecutive_failures = 0;

    if (status === STATUS.CHANGED) {
        state.previous_hash = priorHash;
        state.last_change_at = timestamp;
        state.stats.changes++;
        state.stats.consecutive_changes++;
        return state;
    }

    if (status === STATUS.UNCHANGED) {
        state.stats.consecutive_changes = 0;
        return state;
    }

    if (status === STATUS.INITIALIZED) {
        state.previous_hash = null;
        state.stats.consecutive_changes = 0;
        return state;
    }

    throw new Error(`Unsupported success status: ${status}`);
}

export function recordFailure(state, error, timestamp = new Date().toISOString()) {
    state.last_run_at = timestamp;
    state.last_error = {
        code: error.code || 'UNKNOWN_TARGET_FAILURE',
        type: error.type || ERROR_TYPES.INTERNAL,
        message: error.message,
        at: timestamp
    };
    state.stats.runs++;
    state.stats.failures++;
    state.stats.consecutive_failures++;
    state.stats.consecutive_changes = 0;

    return state;
}

export function validateState(state, statePath = null) {
    if (!state || typeof state !== 'object' || Array.isArray(state)) {
        throw stateValidationError('State must be an object', statePath);
    }

    if (state.schema_version !== SCHEMA_VERSION) {
        throw stateValidationError(
            `State schema_version must be ${SCHEMA_VERSION}`,
            statePath
        );
    }

    validateNullableString(state.created_at, 'created_at', statePath, false);
    validateNullableString(state.last_run_at, 'last_run_at', statePath);
    validateNullableString(state.last_success_at, 'last_success_at', statePath);
    validateNullableString(state.last_change_at, 'last_change_at', statePath);
    validateNullableString(state.current_hash, 'current_hash', statePath);
    validateNullableString(state.previous_hash, 'previous_hash', statePath);
    validateLastError(state.last_error, statePath);

    if (!state.stats || typeof state.stats !== 'object' || Array.isArray(state.stats)) {
        throw stateValidationError('State stats must be an object', statePath);
    }

    for (const key of [
        'runs',
        'successes',
        'failures',
        'changes',
        'consecutive_failures',
        'consecutive_changes'
    ]) {
        if (!Number.isInteger(state.stats[key]) || state.stats[key] < 0) {
            throw stateValidationError(`State stats.${key} must be a non-negative integer`, statePath);
        }
    }

    validateStateSemantics(state, statePath);
}

function validateNullableString(value, label, statePath, nullable = true) {
    if (value === null && nullable) {
        return;
    }

    if (typeof value !== 'string' || value.length === 0) {
        throw stateValidationError(`State ${label} must be ${nullable ? 'null or a non-empty string' : 'a non-empty string'}`, statePath);
    }
}

function validateLastError(lastError, statePath) {
    if (lastError === null) {
        return;
    }

    if (!lastError || typeof lastError !== 'object' || Array.isArray(lastError)) {
        throw stateValidationError('State last_error must be null or an object', statePath);
    }

    for (const key of ['code', 'type', 'message', 'at']) {
        if (typeof lastError[key] !== 'string' || lastError[key].length === 0) {
            throw stateValidationError(`State last_error.${key} must be a non-empty string`, statePath);
        }
    }
}

function validateStateSemantics(state, statePath) {
    const {
        runs,
        successes,
        failures,
        changes,
        consecutive_failures: consecutiveFailures,
        consecutive_changes: consecutiveChanges
    } = state.stats;

    if (successes > runs) {
        throw stateValidationError('State stats.successes must not exceed stats.runs', statePath);
    }

    if (failures > runs) {
        throw stateValidationError('State stats.failures must not exceed stats.runs', statePath);
    }

    if (changes > successes) {
        throw stateValidationError('State stats.changes must not exceed stats.successes', statePath);
    }

    if (consecutiveFailures > failures) {
        throw stateValidationError('State stats.consecutive_failures must not exceed stats.failures', statePath);
    }

    if (consecutiveChanges > changes) {
        throw stateValidationError('State stats.consecutive_changes must not exceed stats.changes', statePath);
    }

    if (successes === 0 && state.last_success_at !== null) {
        throw stateValidationError('State last_success_at must be null when stats.successes is 0', statePath);
    }

    if (successes > 0 && state.last_success_at === null) {
        throw stateValidationError('State last_success_at must be set when stats.successes is greater than 0', statePath);
    }

    if (runs === 0 && state.last_run_at !== null) {
        throw stateValidationError('State last_run_at must be null when stats.runs is 0', statePath);
    }

    if (runs > 0 && state.last_run_at === null) {
        throw stateValidationError('State last_run_at must be set when stats.runs is greater than 0', statePath);
    }

    if (changes === 0 && state.last_change_at !== null) {
        throw stateValidationError('State last_change_at must be null when stats.changes is 0', statePath);
    }

    if (changes > 0 && state.last_change_at === null) {
        throw stateValidationError('State last_change_at must be set when stats.changes is greater than 0', statePath);
    }

    if (successes === 0 && state.current_hash !== null) {
        throw stateValidationError('State current_hash must be null when stats.successes is 0', statePath);
    }

    if (successes > 0 && state.current_hash === null) {
        throw stateValidationError('State current_hash must be set when stats.successes is greater than 0', statePath);
    }

    if (state.current_hash === null && state.previous_hash !== null) {
        throw stateValidationError('State previous_hash must be null when current_hash is null', statePath);
    }

    if (changes === 0 && state.previous_hash !== null) {
        throw stateValidationError('State previous_hash must be null when stats.changes is 0', statePath);
    }

    if (state.last_error !== null && state.last_error.at !== state.last_run_at) {
        throw stateValidationError('State last_error.at must match last_run_at', statePath);
    }

    if (runs > 0 && state.last_error === null && state.last_success_at !== state.last_run_at) {
        throw stateValidationError('State last_success_at must match last_run_at when last_error is null', statePath);
    }
}

function stateValidationError(message, statePath) {
    return targetFailure(
        'STATE_INVALID',
        ERROR_TYPES.STATE,
        statePath ? `${message}: ${statePath}` : message,
        statePath ? { state_path: statePath } : null
    );
}
