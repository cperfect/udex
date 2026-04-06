use crate::authz::{claims::Claims, Error};
use prost::Message;
use super::glob::matches_glob;
use regex::Regex;
use lazy_static::lazy_static;


/// Trait to define the permissions required for a message
pub trait Permissable<M> where M: Message { 
    fn required_permissions(&self) -> Vec<String>;
}
lazy_static! {
    /// valid perms are segments of (lowercase a-z, 0-9, hyphen, single astrisk, double astrisk) separated by a colon: e.g. foo:bar*:**:1-3-adb
    static ref PERMISSION_REGEX: Regex = Regex::new(r"^([a-z0-9-]+\*{0,2}|\*{1,2})(:(([a-z0-9-]+\*{0,2})|\*{1,2}))*$").unwrap();
}

fn validate_permission(permission: &str) -> Result<(), Error> {
    if permission.is_empty() {
        return Err(Error::InvalidPermissionError("Permission cannot be empty".to_string()));
    }
    // Add more validation rules as needed
    if !PERMISSION_REGEX.is_match(permission) {
        return Err(Error::InvalidPermissionError(format!("Invalid permission format: {}", permission)));
    }
    Ok(())
}

pub fn is_permitted<M>(message: &M, claims: &Claims) -> Result<bool, Error> where M: Message + Permissable<M> {
    let required_permissions = message.required_permissions();
    
    // Get user permissions from claims
    let user_permissions: Vec<&str> = if let Some(permissions_value) = claims.get_extras().get("permissions") {
        if let Some(permissions_array) = permissions_value.as_array() {
            let mut validated_permissions = Vec::new();
            for p in permissions_array.iter() {
                if let Some(s) = p.as_str() {
                    // Validate permission and return error if invalid
                    validate_permission(s)?;
                    validated_permissions.push(s);
                }
                // Non-string permissions are silently ignored (existing behavior for mixed arrays)
            }
            validated_permissions
        } else {
            tracing::warn!("Permissions in claims are not an array, treating as empty");
            Vec::new()
        }
    } else {
        tracing::warn!("No permissions found in claims, treating as empty");
        Vec::new()
    };
    
    // Check that all required permissions are present in user permissions
    for required_permission in required_permissions {
        if !user_permissions.iter().any(|p| matches_glob(p, &required_permission)) {
            return Ok(false);
        }
    }
    
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    // Test message structs that implement prost::Message for testing
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

    // Implement Permissable for test messages
    impl Permissable<TestSinglePermissionMessage> for TestSinglePermissionMessage {
        fn required_permissions(&self) -> Vec<String> {
            vec!["test:read".to_string()]
        }
    }

    impl Permissable<TestMultiplePermissionsMessage> for TestMultiplePermissionsMessage {
        fn required_permissions(&self) -> Vec<String> {
            vec!["test:read".to_string(), "test:write".to_string(), "test:admin".to_string()]
        }
    }

    impl Permissable<TestNoPermissionsMessage> for TestNoPermissionsMessage {
        fn required_permissions(&self) -> Vec<String> {
            vec![]
        }
    }

    // Helper function to create claims with specific permissions
    fn create_claims_with_permissions(permissions: Vec<&str>) -> Claims {
        let mut claims = Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1700000000, // exp
            1600000000, // iat
        );

        let permissions_json = json!(permissions);
        let mut extras = HashMap::new();
        extras.insert("permissions".to_string(), permissions_json);
        claims.add_extras(extras);

        claims
    }

    // Helper function to create claims without permissions
    fn create_claims_without_permissions() -> Claims {
        Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1700000000, // exp
            1600000000, // iat
        )
    }

    // Helper function to create claims with empty permissions array
    fn create_claims_with_empty_permissions() -> Claims {
        let mut claims = Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1700000000, // exp
            1600000000, // iat
        );

        let permissions_json = json!([]);
        let mut extras = HashMap::new();
        extras.insert("permissions".to_string(), permissions_json);
        claims.add_extras(extras);

        claims
    }

    // Helper function to create claims with malformed permissions (not an array)
    fn create_claims_with_malformed_permissions() -> Claims {
        let mut claims = Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1700000000, // exp
            1600000000, // iat
        );

        let permissions_json = json!("not-an-array");
        let mut extras = HashMap::new();
        extras.insert("permissions".to_string(), permissions_json);
        claims.add_extras(extras);

        claims
    }

    #[test]
    fn test_single_permission_granted() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_with_permissions(vec!["test:read"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_single_permission_denied() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_with_permissions(vec!["test:write"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_single_permission_with_multiple_user_permissions() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_with_permissions(vec!["test:read", "test:write", "test:admin"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_multiple_permissions_all_granted() {
        let message = TestMultiplePermissionsMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_with_permissions(vec!["test:read", "test:write", "test:admin"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_multiple_permissions_partially_granted() {
        let message = TestMultiplePermissionsMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_with_permissions(vec!["test:read", "test:write"]); // missing test:admin

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_multiple_permissions_none_granted() {
        let message = TestMultiplePermissionsMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_with_permissions(vec!["other:permission"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_multiple_permissions_with_extra_user_permissions() {
        let message = TestMultiplePermissionsMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_with_permissions(vec![
            "test:read",
            "test:write", 
            "test:admin",
            "extra:permission",
            "another:permission"
        ]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_no_permissions_in_claims() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_without_permissions();

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_empty_permissions_array() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_with_empty_permissions();

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_malformed_permissions_in_claims() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_with_malformed_permissions();

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_no_required_permissions() {
        let message = TestNoPermissionsMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_without_permissions();

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should succeed when no permissions are required
    }

    #[test]
    fn test_no_required_permissions_with_user_permissions() {
        let message = TestNoPermissionsMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_with_permissions(vec!["test:read", "test:write"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should succeed when no permissions are required
    }

    #[test]
    fn test_permissions_with_mixed_array_types() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        let mut claims = Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1700000000,
            1600000000,
        );

        // Create permissions array with mixed types (string and number)
        let permissions_json = json!(["test:read", 123, "test:write"]);
        let mut extras = HashMap::new();
        extras.insert("permissions".to_string(), permissions_json);
        claims.add_extras(extras);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should work as "test:read" is present and is a string
    }

    #[test]
    fn test_permissions_case_sensitivity() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_with_permissions(vec!["TEST:READ"]); // Different case - invalid format

        let result = is_permitted(&message, &claims);
        assert!(result.is_err()); // Should fail due to invalid uppercase format
        assert_eq!(result.unwrap_err().to_string(), "Invalid permission: Invalid permission format: TEST:READ");
    }

    #[test]
    fn test_permissions_exact_match() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        let claims = create_claims_with_permissions(vec!["test:read:extra"]); // Similar but not exact

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should fail as it requires exact match
    }

    #[test]
    fn test_permissions_not_array_denies_access() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        
        // Create claims with permissions field as a string instead of array
        let mut claims = Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1700000000,
            1600000000,
        );

        let permissions_json = json!("test:read"); // String instead of array
        let mut extras = HashMap::new();
        extras.insert("permissions".to_string(), permissions_json);
        claims.add_extras(extras);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should fail because permissions is not an array
    }

    #[test]
    fn test_permissions_object_denies_access() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        
        // Create claims with permissions field as an object instead of array
        let mut claims = Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1700000000,
            1600000000,
        );

        let permissions_json = json!({"test:read": true}); // Object instead of array
        let mut extras = HashMap::new();
        extras.insert("permissions".to_string(), permissions_json);
        claims.add_extras(extras);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should fail because permissions is not an array
    }

    #[test]
    fn test_permissions_number_denies_access() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        
        // Create claims with permissions field as a number instead of array
        let mut claims = Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1700000000,
            1600000000,
        );

        let permissions_json = json!(123); // Number instead of array
        let mut extras = HashMap::new();
        extras.insert("permissions".to_string(), permissions_json);
        claims.add_extras(extras);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should fail because permissions is not an array
    }

    #[test]
    fn test_permissions_boolean_denies_access() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        
        // Create claims with permissions field as a boolean instead of array
        let mut claims = Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1700000000,
            1600000000,
        );

        let permissions_json = json!(true); // Boolean instead of array
        let mut extras = HashMap::new();
        extras.insert("permissions".to_string(), permissions_json);
        claims.add_extras(extras);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should fail because permissions is not an array
    }

    #[test]
    fn test_permissions_null_denies_access() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        
        // Create claims with permissions field as null instead of array
        let mut claims = Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1700000000,
            1600000000,
        );

        let permissions_json = json!(null); // Null instead of array
        let mut extras = HashMap::new();
        extras.insert("permissions".to_string(), permissions_json);
        claims.add_extras(extras);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should fail because permissions is not an array
    }

    #[test]
    fn test_invalid_permissions_bad_chars() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        
        // Create claims with an invalid permission format
        let claims = create_claims_with_permissions(vec!["valid:permission", "invalid:permission:CAPS_bad?"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_err());
        //check we get the expected Error::InvalidPermissionError
        assert_eq!(result.unwrap_err().to_string(), "Invalid permission: Invalid permission format: invalid:permission:CAPS_bad?");
    }

    #[test]
    fn test_invalid_permission_three_wildcards() {
        let message = TestSinglePermissionMessage {
            data: "test".to_string(),
        };
        
        // Create claims with a permission that has three wildcards
        let claims = create_claims_with_permissions(vec!["test:read:write:***"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_err());
        //check we get the expected Error::InvalidPermissionError
        assert_eq!(result.unwrap_err().to_string(), "Invalid permission: Invalid permission format: test:read:write:***");
    }

    #[test]
    fn test_regex_patterns() {
        // Test that our regex matches the expected patterns
        assert!(PERMISSION_REGEX.is_match("test:read"));
        assert!(PERMISSION_REGEX.is_match("test:*"));
        assert!(PERMISSION_REGEX.is_match("foo:bar*"));
        assert!(PERMISSION_REGEX.is_match("foo:**"));
        assert!(PERMISSION_REGEX.is_match("udex:index:v1:read"));
        
        // Test that invalid patterns are rejected
        assert!(!PERMISSION_REGEX.is_match("invalid:CAPS"));
        assert!(!PERMISSION_REGEX.is_match("test:***"));
        assert!(!PERMISSION_REGEX.is_match("test:bad?"));
    }

    #[test]
    fn test_single_wildcard_permissions() {
        let message = TestSinglePermissionMessage {
            data: "test:foo:bar".to_string(),
        };
        
        // Create claims with single wildcard permissions
        let claims = create_claims_with_permissions(vec!["test:*"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should succeed as glob matches "test:foo:bar"
    }

    #[test]
    fn test_double_wildcard_permissions() {
        let message = TestSinglePermissionMessage {
            data: "test:foo:bar".to_string(),
        };
        
        // Create claims with double wildcard permissions
        let claims = create_claims_with_permissions(vec!["test:**"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should succeed as double wildcard matches "test:foo:bar"
    }

    #[test]
    fn test_wildcard_only_permissions() {
        let message = TestSinglePermissionMessage {
            data: "test:foo:bar".to_string(),
        };
        
        // Create claims with wildcard-only permissions
        let claims = create_claims_with_permissions(vec!["*", "**"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should succeed as wildcard matches anything
    }

    #[test]
    fn test_mixed_wildcards_in_segments() {
        let message = TestSinglePermissionMessage {
            data: "test:foo:bar".to_string(),
        };
        
        // Create claims with mixed wildcard patterns - test:read* should match test:read
        let claims = create_claims_with_permissions(vec!["udex:*:service:**", "test:read*", "api:**:read"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should succeed as "test:read*" matches "test:read"
    }

    #[test]
    fn test_multiple_single_wildcards() {
        let message = TestSinglePermissionMessage {
            data: "test:foo:bar".to_string(),
        };
        
        // Create claims with multiple single wildcards in different segments
        let claims = create_claims_with_permissions(vec!["service*:data*:read*"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        // This should not match as the pattern doesn't match "test:read"
        assert!(!result.unwrap());
    }

    #[test]
    fn test_multiple_double_wildcards() {
        let message = TestSinglePermissionMessage {
            data: "test:foo:bar".to_string(),
        };
        
        // Create claims with multiple double wildcards
        let claims = create_claims_with_permissions(vec!["service**:data**:read**"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        // This should not match as the pattern doesn't match "test:read"
        assert!(!result.unwrap());
    }

    #[test]
    fn test_alternating_wildcards() {
        let message = TestSinglePermissionMessage {
            data: "test:foo:bar".to_string(),
        };
        
        // Create claims with alternating single and double wildcards that match test:read
        let claims = create_claims_with_permissions(vec!["test*:read**"]);

        let result = is_permitted(&message, &claims);
        assert!(result.is_ok());
        assert!(result.unwrap()); // Should succeed as "test*:read**" matches "test:read"
    }

    #[test]
    fn test_wildcard_validation_success() {
        // Test that valid wildcard patterns pass validation
        let valid_patterns = vec![
            "test:*",
            "service:**", 
            "foo*:bar**",
            "*",
            "**",
            "udex:index*:v1**:read",
            "a*:b**:c*:d**"
        ];
        
        for pattern in valid_patterns {
            let result = validate_permission(pattern);
            assert!(result.is_ok(), "Pattern '{}' should be valid", pattern);
        }
    }

    #[test]
    fn test_wildcard_validation_failure() {
        // Test that invalid wildcard patterns fail validation
        let invalid_patterns = vec![
            "test:***",           // Three wildcards not allowed
            "service:****",       // Four wildcards not allowed  
            "foo:*****",         // Five wildcards not allowed
            "test:bar***:baz",   // Three wildcards in middle segment
        ];
        
        for pattern in invalid_patterns {
            let result = validate_permission(pattern);
            assert!(result.is_err(), "Pattern '{}' should be invalid", pattern);
        }
    }

    #[test]
    fn test_complex_wildcard_combinations() {
        let message = TestSinglePermissionMessage {
            data: "test:foo:bar".to_string(),
        };
        
        // Test various complex but valid wildcard combinations
        // Note: The TestSinglePermissionMessage requires "test:read" permission
        let test_cases = vec![
            ("test:**", true),           // Double wildcard should match
            ("test:*", true),            // Single wildcard after colon should match
            ("test:read", true),         // Exact match should work
            ("other:*", false),          // Different prefix shouldn't match
            ("test*", false),            // Single wildcard on segment doesn't match across colons
            ("*:*", true),               // Wildcard pattern that matches test:read
            ("*:read", true),            // Wildcard first segment, exact second
        ];
        
        for (permission_pattern, should_match) in test_cases {
            let claims = create_claims_with_permissions(vec![permission_pattern]);
            let result = is_permitted(&message, &claims);
            
            assert!(result.is_ok(), "Permission validation should succeed for pattern '{}'", permission_pattern);
            assert_eq!(result.unwrap(), should_match, 
                "Pattern '{}' should {} match 'test:read'", 
                permission_pattern, 
                if should_match { "" } else { "not" }
            );
        }
    }
}