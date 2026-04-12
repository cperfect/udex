use crate::authz::Error;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    sub: String, // Subject - typically the user ID - registered claim
    iss: String, // Issuer - registered claim
    aud: String, // Audience - registered claim
    exp: usize,  // Expiration - registered claim
    iat: usize,  // Issued at - registered claim
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
}

impl Claims {
    pub fn new(sub: String, iss: String, aud: String, exp: usize, iat: usize) -> Self {
        Self {
            sub,
            iss,
            aud,
            exp,
            iat,
            extra: std::collections::HashMap::new(),
        }
    }

    pub fn add_extras(&mut self, extras: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in extras {
            self.extra.insert(key, value);
        }
    }

    pub fn get_extras(&self) -> &std::collections::HashMap<String, serde_json::Value> {
        &self.extra
    }

    /// applies custom validation logic to the [registered claims](https://datatracker.ietf.org/doc/html/rfc7519#section-4.1) of the JWT. It is expected that the JWT library has already validated the signature and the registered claims according to the JWT specification.
    /// we could just check everything again here but validation is going to happen *a lot* and the complete validation gets tested in the server integration tests
    pub fn custom_validate_public(&self) -> Result<(), Error> {
        if self.sub.is_empty() {
            return Err(Error::InvalidClaimsError(
                "Subject (sub) claim cannot be empty".to_string(),
            ));
        }
        if self.iat == 0 {
            return Err(Error::InvalidClaimsError(
                "Issued at (iat) claim must be set".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use time::{OffsetDateTime, UtcOffset};

    fn create_valid_claims() -> Claims {
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            now + 3600, // expires in 1 hour
            now,        // issued now
        )
    }

    fn create_claims_with_times(iat: usize, exp: usize) -> Claims {
        Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            exp,
            iat,
        )
    }

    #[test]
    fn test_serialize_full_claims() {
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let mut claims = create_claims_with_times(now, now + 3600);
        claims.add_extras(
            vec![
                ("role".to_string(), json!("admin")),
                ("permissions".to_string(), json!(["read", "write"])),
            ]
            .into_iter()
            .collect(),
        );
        let claims_json = serde_json::to_string(&claims).expect("Failed to serialize claims");
        // Expected JSON structure, with times
        let expected_claims_json = json!({
            "sub": "test-user",
            "iss": "test-issuer",
            "aud": "test-audience",
            "exp": now + 3600,
            "iat": now,
            "role": "admin",
            "permissions": ["read", "write"]
        })
        .to_string();
        //parse them so we can check they are equal json objects
        // e.g. order of keys doesn't matter
        assert_eq!(
            claims_json.parse::<serde_json::Value>().unwrap(),
            expected_claims_json.parse::<serde_json::Value>().unwrap()
        );
    }

    #[test]
    fn test_claims_new() {
        let claims = create_valid_claims();
        assert_eq!(claims.sub, "test-user");
        assert_eq!(claims.iss, "test-issuer");
        assert_eq!(claims.aud, "test-audience");
        assert!(claims.extra.is_empty());
    }

    #[test]
    fn test_add_extras() {
        let mut claims = create_valid_claims();
        claims.add_extras(
            vec![
                ("role".to_string(), json!("admin")),
                ("permissions".to_string(), json!(["read", "write"])),
            ]
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>(),
        );

        assert_eq!(claims.extra.get("role"), Some(&json!("admin")));
        assert_eq!(
            claims.extra.get("permissions"),
            Some(&json!(["read", "write"]))
        );
    }

    #[test]
    fn test_validate_valid_claims() {
        let claims = create_valid_claims();
        assert!(claims.custom_validate_public().is_ok());
    }

    #[test]
    fn test_validate_empty_subject() {
        let mut claims = create_valid_claims();
        claims.sub = "".to_string();
        let result = claims.custom_validate_public();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Invalid claims: Subject (sub) claim cannot be empty"
        );
    }

    #[test]
    fn test_validate_empty_issuer() {
        let mut claims = create_valid_claims();
        claims.iss = "".to_string();
        let result = claims.custom_validate_public();
        // JWT library validates issuer, so custom validation should pass
        assert!(
            result.is_ok(),
            "Custom validation should pass for empty issuer (JWT library handles this)"
        );
    }

    #[test]
    fn test_validate_empty_audience() {
        let mut claims = create_valid_claims();
        claims.aud = "".to_string();
        let result = claims.custom_validate_public();
        // JWT library validates audience, so custom validation should pass
        assert!(
            result.is_ok(),
            "Custom validation should pass for empty audience (JWT library handles this)"
        );
    }

    #[test]
    fn test_validate_zero_expiration() {
        let mut claims = create_valid_claims();
        claims.exp = 0;
        let result = claims.custom_validate_public();
        // JWT library validates expiration, so custom validation should pass
        assert!(
            result.is_ok(),
            "Custom validation should pass for zero expiration (JWT library handles this)"
        );
    }

    #[test]
    fn test_validate_zero_issued_at() {
        let mut claims = create_valid_claims();
        claims.iat = 0;
        let result = claims.custom_validate_public();
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Invalid claims: Issued at (iat) claim must be set"
        );
    }

    #[test]
    fn test_validate_expired_token() {
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = create_claims_with_times(now - 3600, now - 1800); // issued 1 hour ago, expired 30 min ago
        let result = claims.custom_validate_public();
        // JWT library validates expiration timestamps, so custom validation should pass
        assert!(result.is_ok(), "Custom validation should pass for expired token (JWT library handles timestamp validation)");
    }

    #[test]
    fn test_validate_future_issued_token() {
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
        let claims = create_claims_with_times(now + 3600, now + 7200); // issued 1 hour in future, expires 2 hours in future
        let result = claims.custom_validate_public();
        // JWT library doesn't validate future iat, and custom validation doesn't either
        assert!(result.is_ok(), "Custom validation should pass for future issued token (timestamp validation delegated to JWT library)");
    }

    #[test]
    fn test_timezone_utc_claims() {
        // Test with UTC timezone
        let utc_time = OffsetDateTime::now_utc();
        let iat = utc_time.unix_timestamp() as usize;
        let exp = (utc_time.unix_timestamp() + 3600) as usize; // 1 hour later

        let claims = create_claims_with_times(iat, exp);
        assert!(
            claims.custom_validate_public().is_ok(),
            "UTC claims should be valid"
        );
    }

    #[test]
    fn test_timezone_different_offset_claims() {
        // Test with different timezone offsets - simulate claims generated in different timezones
        let base_utc = OffsetDateTime::now_utc();

        // Simulate token generated in EST (UTC-5)
        let est_offset = UtcOffset::from_hms(-5, 0, 0).unwrap();
        let est_time = base_utc.to_offset(est_offset);
        let est_iat = est_time.unix_timestamp() as usize;
        let est_exp = (est_time.unix_timestamp() + 3600) as usize;

        let est_claims = create_claims_with_times(est_iat, est_exp);

        // Simulate token generated in JST (UTC+9)
        let jst_offset = UtcOffset::from_hms(9, 0, 0).unwrap();
        let jst_time = base_utc.to_offset(jst_offset);
        let jst_iat = jst_time.unix_timestamp() as usize;
        let jst_exp = (jst_time.unix_timestamp() + 3600) as usize;

        let jst_claims = create_claims_with_times(jst_iat, jst_exp);

        // Both should be valid when validated against UTC (since validation uses UTC internally)
        assert!(
            est_claims.custom_validate_public().is_ok(),
            "EST timezone claims should be valid"
        );
        assert!(
            jst_claims.custom_validate_public().is_ok(),
            "JST timezone claims should be valid"
        );

        // The actual unix timestamps should be the same since they represent the same moment
        assert_eq!(
            est_iat, jst_iat,
            "Unix timestamps should be identical regardless of timezone representation"
        );
        assert_eq!(
            est_exp, jst_exp,
            "Unix timestamps should be identical regardless of timezone representation"
        );
    }

    #[test]
    fn test_timezone_edge_cases() {
        let now_utc = OffsetDateTime::now_utc();

        // Test with extreme timezones
        let utc_plus_12 = UtcOffset::from_hms(12, 0, 0).unwrap();
        let utc_minus_12 = UtcOffset::from_hms(-12, 0, 0).unwrap();

        let plus_12_time = now_utc.to_offset(utc_plus_12);
        let minus_12_time = now_utc.to_offset(utc_minus_12);

        let plus_12_claims = create_claims_with_times(
            plus_12_time.unix_timestamp() as usize,
            (plus_12_time.unix_timestamp() + 3600) as usize,
        );

        let minus_12_claims = create_claims_with_times(
            minus_12_time.unix_timestamp() as usize,
            (minus_12_time.unix_timestamp() + 3600) as usize,
        );

        assert!(
            plus_12_claims.custom_validate_public().is_ok(),
            "UTC+12 claims should be valid"
        );
        assert!(
            minus_12_claims.custom_validate_public().is_ok(),
            "UTC-12 claims should be valid"
        );
    }

    #[test]
    fn test_serialization_deserialization() {
        let mut claims = create_valid_claims();
        claims.add_extras(
            vec![
                ("role".to_string(), json!("admin")),
                ("level".to_string(), json!(5)),
            ]
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>(),
        );

        // Serialize to JSON
        let json_str = serde_json::to_string(&claims).expect("Failed to serialize claims");

        // Deserialize back
        let deserialized: Claims =
            serde_json::from_str(&json_str).expect("Failed to deserialize claims");

        assert_eq!(claims.sub, deserialized.sub);
        assert_eq!(claims.iss, deserialized.iss);
        assert_eq!(claims.aud, deserialized.aud);
        assert_eq!(claims.exp, deserialized.exp);
        assert_eq!(claims.iat, deserialized.iat);
        assert_eq!(claims.extra, deserialized.extra);
    }

    #[test]
    fn test_claims_with_fractional_seconds() {
        // Test that fractional seconds in timestamps don't cause issues
        let now_millis = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000; // milliseconds
        let iat = (now_millis / 1000) as usize; // convert to seconds
        let exp = iat + 3600;

        let claims = create_claims_with_times(iat, exp);
        assert!(
            claims.custom_validate_public().is_ok(),
            "Claims with fractional second precision should be valid"
        );
    }

    #[test]
    fn test_boundary_expiration() {
        let now = OffsetDateTime::now_utc().unix_timestamp() as usize;

        // Test token that expires exactly now (JWT library handles expiration validation)
        let exactly_now_claims = create_claims_with_times(now - 1, now);
        assert!(
            exactly_now_claims.custom_validate_public().is_ok(),
            "Custom validation should pass (JWT library handles expiration)"
        );

        // Test token that expires 1 second from now (should be valid)
        let almost_expired_claims = create_claims_with_times(now - 1, now + 1);
        assert!(
            almost_expired_claims.custom_validate_public().is_ok(),
            "Token expiring in 1 second should be valid"
        );
    }
}
