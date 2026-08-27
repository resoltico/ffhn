//! Deterministic symmetric retry timing over immutable delivery snapshots.

use rust_decimal::{Decimal, prelude::ToPrimitive};
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::CoreError;

use super::{DeliveryPolicy, GraphRouteId};

/// Computes the retry instant from only immutable record identity/policy and a commit time.
///
/// `attempt` is the just-recorded one-based failure count. There is no random source, wall-clock
/// key material, or mutable configuration input.
pub fn deterministic_retry_at(
    event_id: &str,
    route_id: &GraphRouteId,
    attempt: u32,
    policy: &DeliveryPolicy,
    retry_state_commit_time_utc: &str,
) -> Result<String, CoreError> {
    if event_id.len() != 64
        || !event_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || attempt == 0
    {
        return Err(CoreError::contract(
            "delivery retry requires a lowercase SHA-256 event id and positive attempt",
        ));
    }
    policy.validate()?;
    let delay = capped_delay(policy, attempt)?;
    let win = (Decimal::from(delay) * policy.jitter_ratio()?)
        .floor()
        .to_u64()
        .ok_or_else(|| CoreError::internal("delivery jitter window is outside u64"))?;
    let modulus = win
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| CoreError::internal("delivery jitter modulus overflowed"))?;
    let raw = deterministic_u64(event_id, route_id, attempt) % modulus;
    let offset = i128::from(raw) - i128::from(win);
    let delayed = i128::from(delay)
        .checked_add(offset)
        .ok_or_else(|| CoreError::internal("delivery jitter delay overflowed"))?
        .max(0);
    let milliseconds = i64::try_from(delayed)
        .map_err(|_| CoreError::internal("delivery delay is outside UTC duration range"))?;
    let base = OffsetDateTime::parse(retry_state_commit_time_utc, &Rfc3339)?;
    base.checked_add(Duration::milliseconds(milliseconds))
        .ok_or_else(|| CoreError::contract("delivery retry instant overflows UTC clock range"))?
        .format(&Rfc3339)
        .map_err(CoreError::from)
}

fn capped_delay(policy: &DeliveryPolicy, attempt: u32) -> Result<u64, CoreError> {
    let shifts = attempt.saturating_sub(1);
    let multiplier = 1_u128.checked_shl(shifts).unwrap_or(u128::MAX);
    let expanded = u128::from(policy.base_backoff_ms()).saturating_mul(multiplier);
    Ok(u64::try_from(expanded)
        .unwrap_or(u64::MAX)
        .min(policy.max_backoff_ms()))
}

fn deterministic_u64(event_id: &str, route_id: &GraphRouteId, attempt: u32) -> u64 {
    let mut hash = Sha256::new();
    hash.update(event_id.as_bytes());
    hash.update(route_id.as_str().as_bytes());
    hash.update(attempt.to_le_bytes());
    let digest = hash.finalize();
    u64::from_le_bytes(
        digest[..8]
            .try_into()
            .expect("SHA-256 prefix has eight bytes"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(jitter: &str) -> DeliveryPolicy {
        toml::from_str(&format!(
            "max_pending = 1\nmax_attempts = 4\nbase_backoff_ms = 100\nmax_backoff_ms = 250\njitter_ratio = \"{jitter}\"\n"
        ))
        .expect("policy")
    }

    #[test]
    fn retry_is_deterministic_symmetric_and_capped_by_its_snapshot() {
        let route = GraphRouteId::new("critical").expect("route");
        let event = "a".repeat(64);
        assert_eq!(
            deterministic_retry_at(&event, &route, 1, &policy("0"), "2026-08-25T00:00:00Z")
                .expect("retry"),
            "2026-08-25T00:00:00.1Z"
        );
        let left =
            deterministic_retry_at(&event, &route, 4, &policy("0.1"), "2026-08-25T00:00:00Z")
                .expect("left");
        let right =
            deterministic_retry_at(&event, &route, 4, &policy("0.1"), "2026-08-25T00:00:00Z")
                .expect("right");
        assert_eq!(left, right);
        let timestamp = OffsetDateTime::parse(&left, &Rfc3339).expect("timestamp");
        let base = OffsetDateTime::parse("2026-08-25T00:00:00Z", &Rfc3339).expect("base");
        let milliseconds = (timestamp - base).whole_milliseconds();
        assert!((225..=275).contains(&milliseconds));

        for (event, attempt) in [
            ("short".to_owned(), 1),
            ("A".repeat(64), 1),
            ("a".repeat(64), 0),
        ] {
            assert!(
                deterministic_retry_at(
                    &event,
                    &route,
                    attempt,
                    &policy("0"),
                    "2026-08-25T00:00:00Z",
                )
                .is_err()
            );
        }
        assert!(deterministic_retry_at(&event, &route, 1, &policy("0"), "not-a-time",).is_err());
        assert_eq!(capped_delay(&policy("0"), 1).expect("attempt one"), 100);
        assert_eq!(capped_delay(&policy("0"), 2).expect("attempt two"), 200);
        assert_eq!(capped_delay(&policy("0"), 3).expect("attempt three"), 250);
        assert_eq!(capped_delay(&policy("0"), u32::MAX).expect("capped"), 250);
        assert_eq!(
            deterministic_u64(&event, &route, 1),
            5_017_460_807_600_687_615
        );
    }
}
