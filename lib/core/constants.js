import { VERSION } from './version.js';

const PREVIEW_CHARS = 160;
const PREVIEW_LINES = 5;
const CHANGE_PREVIEW_ITEMS = 5;

export const CLI = {
    ARGV_OFFSET: 2
};

export const SCHEMA_VERSION = 1;

export const DEFAULTS = {
    WATCHLIST_DIR: 'watchlist',
    TARGET_CONFIG: 'target.toml',
    STATE_FILE: 'state.json',
    CURRENT_FILE: 'current.txt',
    PREVIOUS_FILE: 'previous.txt',
    PREVIEW_CHARS,
    PREVIEW_LINES,
    CHANGE_PREVIEW_ITEMS,
    HASH_ALGORITHM: 'sha256',
    TIMEOUT_MS: 30000,
    MAX_ATTEMPTS: 3,
    RETRY_DELAY_MS: 750,
    RETRY_BACKOFF_FACTOR: 2,
    CONCURRENCY: 4,
    USER_AGENT: `ffhn/${VERSION}`,
    HTMLCUT_COMMAND: 'htmlcut',
    HTMLCUT_TIMEOUT_MS: 30000
};

export const FILES = {
    CONFIG: DEFAULTS.TARGET_CONFIG,
    STATE: DEFAULTS.STATE_FILE,
    CURRENT: DEFAULTS.CURRENT_FILE,
    PREVIOUS: DEFAULTS.PREVIOUS_FILE
};

export const EXIT_CODES = {
    SUCCESS: 0,
    USAGE_ERROR: 1,
    CONFIG_ERROR: 2,
    DEPENDENCY_ERROR: 3,
    TARGET_FAILURE: 4,
    INTERNAL_ERROR: 5
};

export const HTTP_STATUS = {
    TOO_MANY_REQUESTS: 429,
    SERVER_ERROR_START: 500
};

export const FORMAT = {
    JSON_INDENT: 2
};

export const LIMITS = {
    MAX_NAME_LENGTH: 255
};

export const STATUS = {
    INITIALIZED: 'initialized',
    CHANGED: 'changed',
    UNCHANGED: 'unchanged',
    FAILED: 'failed'
};

export const ERROR_TYPES = {
    USAGE: 'usage',
    CONFIG: 'config',
    DEPENDENCY: 'dependency',
    NETWORK: 'network',
    EXTRACT: 'extract',
    STATE: 'state',
    FILESYSTEM: 'filesystem',
    INTERNAL: 'internal'
};

export const COMMANDS = {
    RUN: 'run',
    INIT: 'init',
    STATUS: 'status',
    HELP: 'help',
    VERSION: 'version'
};

export const INIT = {
    TARGET_CONFIG_TEMPLATE: `[target]
url = "https://example.com"

[extract]
from = "<main\\\\b[^>]*>"
to = "</main>"
pattern = "regex"
capture = "inner"
all = false

[request]
timeout_ms = 30000
max_attempts = 3
retry_delay_ms = 750
`
};
