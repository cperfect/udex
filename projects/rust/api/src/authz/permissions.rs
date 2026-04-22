use super::glob::matches_glob;
use crate::authz::{claims::Claims, Error};
use lazy_static::lazy_static;
use prost::Message;
use regex::Regex;

/// Trait to define the permissions required for a message
pub trait Permissable<M>
where
    M: Message,
{
    fn required_permissions(&self) -> Vec<String>;
}

lazy_static! {
    /// Valid permissions are colon-separated segments of lowercase alphanumerics,
    /// hyphens, and at most two consecutive wildcards. E.g. `udex:entry:v1:my-index:read`.
    static ref PERMISSION_REGEX: Regex =
        Regex::new(r"^([a-z0-9-]+\*{0,2}|\*{1,2})(:(([a-z0-9-]+\*{0,2})|\*{1,2}))*$").unwrap();
}

fn validate_permission(permission: &str) -> Result<(), Error> {
    if permission.is_empty() {
        return Err(Error::InvalidPermissionError(
            "Permission cannot be empty".to_string(),
        ));
    }
    if !PERMISSION_REGEX.is_match(permission) {
        return Err(Error::InvalidPermissionError(format!(
            "Invalid permission format: {}",
            permission
        )));
    }
    Ok(())
}

/// Extract `udex:` permissions from the RFC 8693 `scope` claim.
///
/// Splits `claims.scope` on whitespace and retains only values prefixed with
/// `udex:`. Non-`udex:` values (e.g. `openid`, `profile`) are silently
/// discarded. Each retained value is validated against the permission format;
/// an invalid format returns an error.
fn extract_udex_permissions(claims: &Claims) -> Result<Vec<&str>, Error> {
    let mut permissions = Vec::new();
    for scope_value in claims.scope.split_whitespace() {
        if scope_value.starts_with("udex:") {
            validate_permission(scope_value)?;
            permissions.push(scope_value);
        }
        // Non-udex: scope values (openid, profile, email, etc.) are silently ignored.
    }
    Ok(permissions)
}

pub fn is_permitted<M>(message: &M, claims: &Claims) -> Result<bool, Error>
where
    M: Message + Permissable<M>,
{
    let required_permissions = message.required_permissions();
    let user_permissions = extract_udex_permissions(claims)?;

    for required in required_permissions {
        if !user_permissions.iter().any(|p| matches_glob(p, &required)) {
            return Ok(false);
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct TestSinglePermissionMessage {
        #[prost(string, tag = "1")]
        pub data: String,
    }

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct TestMultiplePermissionsMessage {
        #[prost(string, tag = "1")]
        pub data: String,
    }

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct TestNoPermissionsMessage {
        #[prost(string, tag = "1")]
        pub data: String,
    }

    impl Permissable<TestSinglePermissionMessage> for TestSinglePermissionMessage {
        fn required_permissions(&self) -> Vec<String> {
            vec!["udex:test:read".to_string()]
        }
    }

    impl Permissable<TestMultiplePermissionsMessage> for TestMultiplePermissionsMessage {
        fn required_permissions(&self) -> Vec<String> {
            vec![
                "udex:test:read".to_string(),
                "udex:test:write".to_string(),
                "udex:test:admin".to_string(),
            ]
        }
    }

    impl Permissable<TestNoPermissionsMessage> for TestNoPermissionsMessage {
        fn required_permissions(&self) -> Vec<String> {
            vec![]
        }
    }

    fn claims_with_scope(scope: &str) -> Claims {
        Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1700000000,
            1600000000,
        )
        .with_scope(scope)
    }

    fn claims_no_scope() -> Claims {
        Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1700000000,
            1600000000,
        )
    }

    #[test]
    fn test_single_permission_granted() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        assert!(is_permitted(&msg, &claims_with_scope("udex:test:read")).unwrap());
    }

    #[test]
    fn test_single_permission_denied() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        assert!(!is_permitted(&msg, &claims_with_scope("udex:test:write")).unwrap());
    }

    #[test]
    fn test_single_permission_with_multiple_user_permissions() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        assert!(is_permitted(
            &msg,
            &claims_with_scope("udex:test:read udex:test:write udex:test:admin")
        )
        .unwrap());
    }

    #[test]
    fn test_multiple_permissions_all_granted() {
        let msg = TestMultiplePermissionsMessage {
            data: String::new(),
        };
        assert!(is_permitted(
            &msg,
            &claims_with_scope("udex:test:read udex:test:write udex:test:admin")
        )
        .unwrap());
    }

    #[test]
    fn test_multiple_permissions_partially_granted() {
        let msg = TestMultiplePermissionsMessage {
            data: String::new(),
        };
        assert!(!is_permitted(&msg, &claims_with_scope("udex:test:read udex:test:write")).unwrap());
    }

    #[test]
    fn test_multiple_permissions_none_granted() {
        let msg = TestMultiplePermissionsMessage {
            data: String::new(),
        };
        assert!(!is_permitted(&msg, &claims_with_scope("udex:other:permission")).unwrap());
    }

    #[test]
    fn test_multiple_permissions_with_extra_user_permissions() {
        let msg = TestMultiplePermissionsMessage {
            data: String::new(),
        };
        assert!(is_permitted(
            &msg,
            &claims_with_scope("udex:test:read udex:test:write udex:test:admin udex:extra:perm")
        )
        .unwrap());
    }

    #[test]
    fn test_no_scope_in_claims_denies_access() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        assert!(!is_permitted(&msg, &claims_no_scope()).unwrap());
    }

    #[test]
    fn test_empty_scope_denies_access() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        assert!(!is_permitted(&msg, &claims_with_scope("")).unwrap());
    }

    #[test]
    fn test_non_udex_scopes_are_silently_discarded() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        assert!(!is_permitted(&msg, &claims_with_scope("openid profile email")).unwrap());
    }

    #[test]
    fn test_mixed_scope_non_udex_ignored_udex_grants_access() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        assert!(is_permitted(
            &msg,
            &claims_with_scope("openid profile email udex:test:read")
        )
        .unwrap());
    }

    #[test]
    fn test_no_required_permissions_always_granted() {
        let msg = TestNoPermissionsMessage {
            data: String::new(),
        };
        assert!(is_permitted(&msg, &claims_no_scope()).unwrap());
    }

    #[test]
    fn test_no_required_permissions_with_scope() {
        let msg = TestNoPermissionsMessage {
            data: String::new(),
        };
        assert!(is_permitted(&msg, &claims_with_scope("udex:test:read udex:test:write")).unwrap());
    }

    #[test]
    fn test_permissions_case_sensitive_uppercase_is_invalid() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        let err = is_permitted(&msg, &claims_with_scope("udex:TEST:READ")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid permission: Invalid permission format: udex:TEST:READ"
        );
    }

    #[test]
    fn test_permissions_exact_match() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        assert!(!is_permitted(&msg, &claims_with_scope("udex:test:read:extra")).unwrap());
    }

    #[test]
    fn test_invalid_permissions_bad_chars() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        let err = is_permitted(
            &msg,
            &claims_with_scope("udex:valid:permission udex:invalid:CAPS_bad?"),
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid permission: Invalid permission format: udex:invalid:CAPS_bad?"
        );
    }

    #[test]
    fn test_invalid_permission_three_wildcards() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        let err = is_permitted(&msg, &claims_with_scope("udex:test:read:write:***")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid permission: Invalid permission format: udex:test:read:write:***"
        );
    }

    #[test]
    fn test_regex_patterns() {
        assert!(PERMISSION_REGEX.is_match("udex:test:read"));
        assert!(PERMISSION_REGEX.is_match("udex:test:*"));
        assert!(PERMISSION_REGEX.is_match("udex:index:v1:read"));
        assert!(PERMISSION_REGEX.is_match("foo:bar*"));
        assert!(PERMISSION_REGEX.is_match("foo:**"));

        assert!(!PERMISSION_REGEX.is_match("invalid:CAPS"));
        assert!(!PERMISSION_REGEX.is_match("test:***"));
        assert!(!PERMISSION_REGEX.is_match("test:bad?"));
    }

    #[test]
    fn test_single_wildcard_permissions() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        // udex:test:* matches required udex:test:read
        assert!(is_permitted(&msg, &claims_with_scope("udex:test:*")).unwrap());
    }

    #[test]
    fn test_double_wildcard_permissions() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        // udex:test:** matches required udex:test:read
        assert!(is_permitted(&msg, &claims_with_scope("udex:test:**")).unwrap());
    }

    #[test]
    fn test_wildcard_only_permissions() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        // udex:** matches required udex:test:read
        assert!(is_permitted(&msg, &claims_with_scope("udex:**")).unwrap());
    }

    #[test]
    fn test_mixed_wildcards_in_segments() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        // udex:test:read* matches required udex:test:read
        assert!(is_permitted(
            &msg,
            &claims_with_scope("udex:*:service:** udex:test:read* udex:api:**:read")
        )
        .unwrap());
    }

    #[test]
    fn test_multiple_single_wildcards() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        assert!(!is_permitted(&msg, &claims_with_scope("udex:service*:data*:read*")).unwrap());
    }

    #[test]
    fn test_multiple_double_wildcards() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        assert!(!is_permitted(&msg, &claims_with_scope("udex:service**:data**:read**")).unwrap());
    }

    #[test]
    fn test_alternating_wildcards() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        // udex:test*:read** matches required udex:test:read
        assert!(is_permitted(&msg, &claims_with_scope("udex:test*:read**")).unwrap());
    }

    #[test]
    fn test_wildcard_validation_success() {
        let valid_patterns = vec![
            "udex:test:*",
            "udex:service:**",
            "udex:foo*:bar**",
            "udex:index*:v1**:read",
            "*",
            "**",
        ];
        for pattern in valid_patterns {
            assert!(
                validate_permission(pattern).is_ok(),
                "Pattern '{pattern}' should be valid"
            );
        }
    }

    #[test]
    fn test_wildcard_validation_failure() {
        let invalid_patterns = vec![
            "udex:test:***",
            "udex:service:****",
            "udex:foo:*****",
            "udex:test:bar***:baz",
        ];
        for pattern in invalid_patterns {
            assert!(
                validate_permission(pattern).is_err(),
                "Pattern '{pattern}' should be invalid"
            );
        }
    }

    #[test]
    fn test_complex_wildcard_combinations() {
        let msg = TestSinglePermissionMessage {
            data: String::new(),
        };
        // Required: udex:test:read
        let cases = vec![
            ("udex:test:**", true),
            ("udex:test:*", true),
            ("udex:test:read", true),
            ("udex:other:*", false),
            ("udex:test*", false), // no colon → won't match udex:test:read
            ("udex:*:*", true),
            ("udex:*:read", true),
        ];
        for (pattern, should_match) in cases {
            let result = is_permitted(&msg, &claims_with_scope(pattern)).unwrap();
            assert_eq!(
                result,
                should_match,
                "Pattern '{pattern}' should {}match 'udex:test:read'",
                if should_match { "" } else { "not " }
            );
        }
    }
}
