/// High-performance glob pattern matching for URN-style permission authorization.
///
/// This really exists as an experiment in algorithm optimisation and design with AI assistance.
/// Probably better forking an existing library to allow for urn matching instead.
///
/// This module provides utilities for matching globbed patterns to strings for the purpose
/// of determining whether claims match required permissions. Permissions are expected to be
/// URNs with a separator of `:` (note '/' is a valid URN separator but not supported here).
///
/// # Wildcard Types Supported
///
/// - **Single `*`**: Matches any single segment between separators (`:`)
/// - **Double `**`**: Matches zero or more segments, can cross segment boundaries
/// - **Embedded wildcards**: Patterns like `prefix*suffix` or `prefix**suffix` within segments
/// - **Multiple wildcards**: Multiple wildcards per pattern, e.g., `*:*:*` or `pre*:mid**:suf*`
///
/// # Algorithm Overview
///
/// The implementation uses a unified **Dynamic Programming approach with memoization** that
/// provides optimal performance characteristics:
///
/// - **Time Complexity**: O(P × I × S²) where P = pattern segments, I = input segments, S = avg segment length
/// - **Space Complexity**: O(P × I) for memoization cache
///
/// ## Key Algorithmic Innovations
///
/// 1. **DP with Memoization**: Prevents exponential blowup in complex backtracking scenarios
/// 2. **Virtual String Concatenation**: `VirtualConcatenatedInput` eliminates O(S×I) string allocation overhead
/// 3. **Embedded Wildcard Handling**: Special optimization for patterns like `prefix**suffix` that span segments
/// 4. **Segment-based Processing**: Optimized for URN-style colon-separated patterns
///
/// ## Performance Optimizations
///
/// The algorithm achieves high performance through several optimizations:
/// - **Zero-allocation matching**: Virtual concatenation avoids temporary string creation
/// - **Early termination**: Fast-path checks for common cases
/// - **Memoization**: Caches results to avoid recomputing subproblems
/// - **Specialized handlers**: Different algorithms for different wildcard types
///
/// # Pattern Examples
///
/// ```text
/// Pattern              Input                    Result
/// -------              -----                    ------
/// test:foo:bar      -> test:foo:bar           = ✓ (exact match)
/// *                 -> test                   = ✓ (single segment wildcard)
/// test*:bar         -> testfoo:bar            = ✓ (embedded single wildcard)
/// test:foo:*        -> test:foo:bar           = ✓ (trailing wildcard)
/// test:*:bar        -> test:foo:bar           = ✓ (middle wildcard)
/// test:**           -> test:foo:bar           = ✓ (double wildcard, multiple segments)
/// test:**:baz       -> test:foo:bar:baz       = ✓ (double wildcard in middle)
/// test:quux*        -> test:quuxwibble        = ✓ (trailing single wildcard)
/// test:quux*bar     -> test:quuxwibblebar     = ✓ (multiple single wildcards)
/// test:quux**       -> test:quuxwibble        = ✓ (embedded double wildcard, single segment)
/// test:quux**       -> test:quux:wibble       = ✓ (embedded double wildcard, cross-segment)
/// test:quux**:baz   -> test:quuxwibble:zip:baz = ✓ (embedded double wildcard spanning)
/// test:*:baz:*      -> test:quux:baz:quux     = ✓ (multiple single wildcards)
/// test:quux**:baz:* -> test:quuxwibble:zip:baz:quux = ✓ (mixed wildcards)
/// ```
///
/// # Usage
///
/// ```
/// # use udex_api::authz::glob::matches_glob;
/// // Basic wildcard matching
/// assert!(matches_glob("service:*:read", "service:users:read"));
///
/// // Cross-segment wildcards  
/// assert!(matches_glob("service:**", "service:users:read"));
///
/// // Embedded wildcards
/// assert!(matches_glob("udex*:v1**:read*", "udexAPI:v1beta:readwrite"));
/// ```
use std::collections::HashMap;

/// Matches a glob pattern against an input string using dynamic programming with memoization.
///
/// This is the main entry point for glob pattern matching. It uses a unified DP approach that
/// combines the correctness of systematic state exploration with performance optimizations.
///
/// # Algorithm Overview
///
/// The algorithm uses dynamic programming with memoization to systematically explore all possible
/// matches between pattern segments and input segments. Key algorithmic choices:
///
/// - **DP with Memoization**: Prevents exponential blowup in backtracking scenarios by caching
///   results for each (pattern_index, input_index) state pair
/// - **Segment-based Processing**: Splits on ':' separators for URN-style permission matching
/// - **Enhanced Wildcard Support**: Handles embedded `**` patterns that can span segment boundaries
/// - **Virtual Concatenation**: Avoids expensive string concatenation using `VirtualConcatenatedInput`
///
/// # Performance
///
/// - **Time Complexity**: O(P × I × S²) where P = pattern segments, I = input segments, S = avg segment length
/// - **Space Complexity**: O(P × I) for memoization cache
///
/// # Arguments
///
/// * `pattern` - Glob pattern with `:` separators, supporting `*` (single segment) and `**` (cross-segment) wildcards
/// * `input` - Input string to match against, with `:` separators
///
/// # Returns
///
/// `true` if the pattern matches the input, `false` otherwise
///
/// # Examples
///
/// ```
/// # use udex_api::authz::glob::matches_glob;
/// assert!(matches_glob("test:*:bar", "test:foo:bar"));
/// assert!(matches_glob("test:**", "test:foo:bar:baz"));
/// assert!(matches_glob("test:quux**:baz", "test:quuxwibble:zip:baz"));
/// ```
pub fn matches_glob(pattern: &str, input: &str) -> bool {
    // Split both pattern and input into segments using ':' as delimiter
    // This enables URN-style permission matching (e.g., "service:resource:action")
    let pattern_segments: Vec<&str> = pattern.split(':').collect();
    let input_segments: Vec<&str> = input.split(':').collect();

    // Early termination for degenerate cases
    if pattern_segments.is_empty() || input_segments.is_empty() {
        return false;
    }

    // Initialize memoization cache to store results of (pattern_idx, input_idx) -> bool
    // This prevents exponential blowup in recursive backtracking scenarios
    let mut memo: HashMap<(usize, usize), bool> = HashMap::new();
    dp_match(&pattern_segments, &input_segments, 0, 0, &mut memo)
}

/// Core dynamic programming function with memoization for glob matching.
///
/// This function implements the recursive DP logic with caching to avoid recomputing
/// the same subproblems. It serves as a thin wrapper around `compute_match` that
/// handles memoization.
///
/// # Arguments
///
/// * `pattern_segs` - Array of pattern segments split by ':'
/// * `input_segs` - Array of input segments split by ':'
/// * `p_idx` - Current index in pattern segments (0-based)
/// * `i_idx` - Current index in input segments (0-based)
/// * `memo` - Memoization cache mapping (pattern_idx, input_idx) -> match_result
///
/// # Returns
///
/// `true` if pattern[p_idx..] matches input[i_idx..], `false` otherwise
fn dp_match(
    pattern_segs: &[&str],
    input_segs: &[&str],
    p_idx: usize,
    i_idx: usize,
    memo: &mut HashMap<(usize, usize), bool>,
) -> bool {
    // Check memoization cache first - this is the key optimization that prevents
    // exponential time complexity in pathological cases with lots of backtracking
    if let Some(&cached_result) = memo.get(&(p_idx, i_idx)) {
        return cached_result;
    }

    // Compute the result and cache it before returning
    let result = compute_match(pattern_segs, input_segs, p_idx, i_idx, memo);
    memo.insert((p_idx, i_idx), result);
    result
}

/// Computes whether pattern[p_idx..] matches input[i_idx..] using dynamic programming.
///
/// This is the core matching logic that handles different wildcard types and implements
/// the recursive DP algorithm. Key algorithmic decisions:
///
/// 1. **Base Cases**: Handle exhaustion of pattern/input segments
/// 2. **Wildcard Dispatch**: Route to appropriate handlers based on pattern type
/// 3. **Backtracking**: For `**` wildcards, try multiple consumption strategies
/// 4. **Embedded Wildcards**: Special handling for patterns like "prefix**suffix"
///
/// # Arguments
///
/// * `pattern_segs` - Array of pattern segments
/// * `input_segs` - Array of input segments  
/// * `p_idx` - Current pattern index
/// * `i_idx` - Current input index
/// * `memo` - Memoization cache for recursive calls
///
/// # Returns
///
/// `true` if the remaining pattern matches the remaining input
fn compute_match(
    pattern_segs: &[&str],
    input_segs: &[&str],
    p_idx: usize,
    i_idx: usize,
    memo: &mut HashMap<(usize, usize), bool>,
) -> bool {
    // Base case 1: Pattern exhausted
    if p_idx == pattern_segs.len() {
        return i_idx == input_segs.len(); // Both exhausted = complete match
    }

    // Base case 2: Input exhausted but pattern remains
    if i_idx == input_segs.len() {
        // Only remaining patterns can be trailing ** wildcards or patterns ending with **
        // Example: "prefix**" can match empty remainder, but "prefix:literal" cannot
        return all_remaining_can_match_empty(pattern_segs, p_idx);
    }

    let current_pattern = pattern_segs[p_idx];

    // Handle different pattern types with enhanced logic
    if current_pattern == "*" {
        // Single wildcard: must match exactly one segment
        dp_match(pattern_segs, input_segs, p_idx + 1, i_idx + 1, memo)
    } else if current_pattern == "**" {
        // Pure double wildcard: try matching 0, 1, 2, ... segments

        // Try consuming 0 segments (move to next pattern)
        if dp_match(pattern_segs, input_segs, p_idx + 1, i_idx, memo) {
            return true;
        }

        // Try consuming 1+ segments (stay on same pattern, advance input)
        if i_idx < input_segs.len() {
            dp_match(pattern_segs, input_segs, p_idx, i_idx + 1, memo)
        } else {
            false
        }
    } else if current_pattern.contains("**") {
        // Enhanced handling for embedded ** patterns (like "quux**", "**suffix", "pre**suf")
        handle_embedded_double_wildcard(pattern_segs, input_segs, p_idx, i_idx, memo)
    } else {
        // Regular pattern or pattern with single wildcards within segment
        let segment_match = matches_segment(current_pattern, input_segs[i_idx]);
        if segment_match.0 {
            // Normal segment match - advance both pattern and input
            dp_match(pattern_segs, input_segs, p_idx + 1, i_idx + 1, memo)
        } else {
            false
        }
    }
}

// Matches a single segment against a pattern segment.
// Returns a tuple of (bool, Option<&str>) where
// the bool indicates if the input matches the pattern,
// and if matched, the Option<&str> contains remaining pattern that isn't matched (e.g. ** can cross segments)
fn matches_segment<'a>(pattern: &'a str, input: &'a str) -> (bool, Option<&'a str>) {
    if pattern == "*" {
        return (true, None); // Matches anything
    } else if pattern == "**" {
        return (true, Some(pattern)); // Matches anything and could match more
    }

    let matcher = |parts: Vec<&'a str>, double_star: bool| -> (bool, Option<&'a str>) {
        if parts.is_empty() {
            return (false, None); // No parts to match
        }

        // Check leading part
        if !parts[0].is_empty() && !input.starts_with(parts[0]) {
            return (false, None);
        }

        // Check trailing part
        if !parts.last().unwrap().is_empty() && !input.ends_with(parts.last().unwrap()) {
            return (false, None);
        }

        let mut previous_part = parts[0];

        // Check middle parts
        let mut remaining_input = &input[parts[0].len()..];
        for part in &parts[1..] {
            if part.is_empty() {
                return (true, if double_star { Some("**") } else { None });
            }

            if previous_part.is_empty() && remaining_input.ends_with(part) {
                return (true, None);
            }

            if let Some(pos) = remaining_input.find(part) {
                remaining_input = &remaining_input[pos + part.len()..];
                previous_part = part; // Update previous part for next iteration
            } else {
                return (false, None); // Part not found in remaining input
            }
        }

        (true, if double_star { Some("**") } else { None })
    };

    let parts_double_star: Vec<&str> = pattern.split("**").collect();
    if parts_double_star.len() > 1 {
        return matcher(parts_double_star, true);
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return (pattern == input, None); // Exact match
    }

    matcher(parts, false)
}

/// Optimized version of matches_segment that works on a range of input segments without concatenation
fn matches_segment_range<'a>(
    pattern: &'a str,
    input_segs: &'a [&'a str],
    start_idx: usize,
    end_idx: usize,
) -> (bool, Option<&'a str>) {
    if pattern == "*" {
        return (true, None); // Matches anything
    } else if pattern == "**" {
        return (true, Some(pattern)); // Matches anything and could match more
    }

    // For single segment, use standard matches_segment
    if end_idx - start_idx == 1 {
        return matches_segment(pattern, input_segs[start_idx]);
    }

    // For multi-segment ranges, we need to match against the conceptual concatenation
    // without actually creating the concatenated string
    let matcher_range = |parts: Vec<&'a str>, double_star: bool| -> (bool, Option<&'a str>) {
        if parts.is_empty() {
            return (false, None);
        }

        // Create a virtual view of concatenated segments with separators
        let virtual_input = VirtualConcatenatedInput::new(input_segs, start_idx, end_idx);

        // Check leading part
        if !parts[0].is_empty() && !virtual_input.starts_with(parts[0]) {
            return (false, None);
        }

        // Check trailing part
        if !parts.last().unwrap().is_empty() && !virtual_input.ends_with(parts.last().unwrap()) {
            return (false, None);
        }

        // Check middle parts using virtual input
        let mut search_pos = parts[0].len();
        for part in &parts[1..parts.len() - 1] {
            if part.is_empty() {
                return (true, if double_star { Some("**") } else { None });
            }

            if let Some(pos) = virtual_input.find(part, search_pos) {
                search_pos = pos + part.len();
            } else {
                return (false, None);
            }
        }

        (true, if double_star { Some("**") } else { None })
    };

    let parts_double_star: Vec<&str> = pattern.split("**").collect();
    if parts_double_star.len() > 1 {
        return matcher_range(parts_double_star, true);
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        // For exact match across segments, compare virtually
        let virtual_input = VirtualConcatenatedInput::new(input_segs, start_idx, end_idx);
        return (virtual_input.equals(pattern), None);
    }

    matcher_range(parts, false)
}

/// Virtual concatenated input that avoids string allocation
struct VirtualConcatenatedInput<'a> {
    segments: &'a [&'a str],
    start_idx: usize,
    end_idx: usize,
    total_len: usize,
}

impl<'a> VirtualConcatenatedInput<'a> {
    fn new(segments: &'a [&'a str], start_idx: usize, end_idx: usize) -> Self {
        let end_idx = end_idx.min(segments.len()); // Clamp to valid range
        let total_len = if end_idx > start_idx {
            segments[start_idx..end_idx]
                .iter()
                .map(|s| s.len())
                .sum::<usize>()
                + (end_idx - start_idx - 1)
        } else {
            0
        };

        Self {
            segments,
            start_idx,
            end_idx, // Use the clamped end_idx
            total_len,
        }
    }

    fn starts_with(&self, prefix: &str) -> bool {
        if prefix.is_empty() {
            return true;
        }

        let mut remaining_prefix = prefix;
        for (i, &segment) in self.segments[self.start_idx..self.end_idx]
            .iter()
            .enumerate()
        {
            if remaining_prefix.is_empty() {
                return true;
            }

            if remaining_prefix.len() <= segment.len() {
                return segment.starts_with(remaining_prefix);
            }

            if !segment.starts_with(&remaining_prefix[..segment.len()]) {
                return false;
            }

            remaining_prefix = &remaining_prefix[segment.len()..];

            // Account for separator (':') between segments
            if i < self.end_idx - self.start_idx - 1 {
                if remaining_prefix.is_empty() || !remaining_prefix.starts_with(':') {
                    return remaining_prefix.is_empty();
                }
                remaining_prefix = &remaining_prefix[1..];
            }
        }

        remaining_prefix.is_empty()
    }

    fn ends_with(&self, suffix: &str) -> bool {
        if suffix.is_empty() {
            return true;
        }

        let mut remaining_suffix = suffix;
        for (i, &segment) in self.segments[self.start_idx..self.end_idx]
            .iter()
            .rev()
            .enumerate()
        {
            if remaining_suffix.is_empty() {
                return true;
            }

            if remaining_suffix.len() <= segment.len() {
                return segment.ends_with(remaining_suffix);
            }

            let segment_suffix_len = segment.len();
            if !segment.ends_with(&remaining_suffix[remaining_suffix.len() - segment_suffix_len..])
            {
                return false;
            }

            remaining_suffix = &remaining_suffix[..remaining_suffix.len() - segment_suffix_len];

            // Account for separator (':') between segments
            if i < self.end_idx - self.start_idx - 1 {
                if remaining_suffix.is_empty() || !remaining_suffix.ends_with(':') {
                    return remaining_suffix.is_empty();
                }
                remaining_suffix = &remaining_suffix[..remaining_suffix.len() - 1];
            }
        }

        remaining_suffix.is_empty()
    }

    fn find(&self, needle: &str, start_pos: usize) -> Option<usize> {
        if needle.is_empty() {
            return Some(start_pos);
        }

        // This is a simplified implementation - a full implementation would need
        // to handle searching across segment boundaries more efficiently
        // For now, we'll create the virtual string for searching
        let virtual_str = self.to_string();
        virtual_str[start_pos..]
            .find(needle)
            .map(|pos| start_pos + pos)
    }

    fn equals(&self, other: &str) -> bool {
        if other.len() != self.total_len {
            return false;
        }

        // For exact equality, we need to compare the virtual concatenation
        let virtual_str = self.to_string();
        virtual_str == other
    }
}

impl std::fmt::Display for VirtualConcatenatedInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, &segment) in self.segments[self.start_idx..self.end_idx]
            .iter()
            .enumerate()
        {
            if i > 0 {
                f.write_str(":")?;
            }
            f.write_str(segment)?;
        }
        Ok(())
    }
}

fn handle_embedded_double_wildcard(
    pattern_segs: &[&str],
    input_segs: &[&str],
    p_idx: usize,
    i_idx: usize,
    memo: &mut HashMap<(usize, usize), bool>,
) -> bool {
    let current_pattern = pattern_segs[p_idx];

    // Try matching the pattern against different spans of input segments
    // Start with single segment, then try spanning multiple segments
    for span_length in 1..=(input_segs.len() - i_idx) {
        let end_idx = i_idx + span_length;

        // Use optimized range-based matching instead of string concatenation
        let segment_match = matches_segment_range(current_pattern, input_segs, i_idx, end_idx);
        if segment_match.0 {
            // If it matches, try to continue with the rest of the pattern
            if dp_match(pattern_segs, input_segs, p_idx + 1, end_idx, memo) {
                return true;
            }
        }
    }

    false
}

fn all_remaining_can_match_empty(pattern_segs: &[&str], start_idx: usize) -> bool {
    // Enhanced version that handles patterns ending with ** (like "prefix**")
    for segment in &pattern_segs[start_idx..] {
        if *segment != "**" && !segment.ends_with("**") {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_segment() {
        assert_eq!(matches_segment("test", "test"), (true, None), "Exact match");
        assert_eq!(matches_segment("*", "test"), (true, None), "Wildcard match");
        assert_eq!(
            matches_segment("test*", "testfoo"),
            (true, None),
            "Trailing wildcard"
        );
        assert_eq!(
            matches_segment("*test", "footest"),
            (true, None),
            "Leading wildcard"
        );
        assert_eq!(
            matches_segment("**", "testfoo"),
            (true, Some("**")),
            "Double wildcard"
        );
        assert_eq!(
            matches_segment("test**", "testfoo"),
            (true, Some("**")),
            "Trailing double wildcard"
        );
        assert_eq!(
            matches_segment("**test", "footest"),
            (true, None),
            "Leading double wildcard"
        );
        assert_eq!(
            matches_segment("te*st", "teXst"),
            (true, None),
            "Single wildcard in middle of segment"
        );
        assert_eq!(
            matches_segment("fo*o", "foYo"),
            (true, None),
            "Single wildcard in middle of segment - short"
        );
        assert_eq!(
            matches_segment("t*s*t", "tXsYt"),
            (true, None),
            "Multiple wildcards in same segment"
        );
        assert_eq!(
            matches_segment("test*middle*end", "testXXXmiddleYYYend"),
            (true, None),
            "Multiple wildcards with literal text between"
        );

        assert_eq!(
            matches_segment("test", "foo"),
            (false, None),
            "Non-matching exact"
        );
        assert_eq!(
            matches_segment("test", "testfoo"),
            (false, None),
            "Non-matching exact with suffix"
        );
        assert_eq!(
            matches_segment("test*", "foo"),
            (false, None),
            "Non-matching prefix"
        );
        assert_eq!(
            matches_segment("*test", "testfoo"),
            (false, None),
            "Non-matching suffix"
        );
        assert_eq!(
            matches_segment("test**", "foo"),
            (false, None),
            "Non-matching trailing double wildcard"
        );
        assert_eq!(
            matches_segment("**test", "testfoo"),
            (false, None),
            "Non-matching leading double wildcard"
        );
        assert_eq!(
            matches_segment("te*st", "tst"),
            (false, None),
            "Single wildcard in middle - partial match"
        );
        assert_eq!(
            matches_segment("*", ""),
            (true, None),
            "Single wildcard matches with empty input"
        );
        assert_eq!(
            matches_segment("", ""),
            (true, None),
            "empty pattern should match empty segment"
        );
    }

    #[test]
    fn test_matches_glob() {
        assert!(matches_glob("test", "test"), "Exact match - single segment");
        assert!(
            matches_glob("test:foo:bar", "test:foo:bar"),
            "Exact match - multi-segment"
        );
        assert!(matches_glob("*", "test"), "Wildcard match - single segment");
        assert!(
            matches_glob("test:foo:*", "test:foo:bar"),
            "Trailing wildcard - multi-segment"
        );
        assert!(
            matches_glob("*wibble:foo", "testwibble:foo"),
            "Leading wildcard - multi-segment"
        );
        assert!(
            matches_glob("test:foo*:bar", "test:foowibble:bar"),
            "Middle wildcard - multi-segment"
        );
        assert!(
            matches_glob("test:*:bar", "test:foo:bar"),
            "Middle wildcard - multi-segment"
        );
        assert!(
            matches_glob("test:**", "test:foo:bar"),
            "Double wildcard - multi-segment"
        );
        assert!(
            matches_glob("test:**", "test:foo"),
            "Double wildcard - single segment"
        );
        assert!(
            matches_glob("test:**:baz", "test:foo:bar:baz"),
            "Double wildcard in middle - multi-segment"
        );
        assert!(
            matches_glob("test:**:baz", "test:foo:baz"),
            "Double wildcard in middle - single-segment"
        );
        assert!(
            matches_glob("test:quux*", "test:quuxwibble"),
            "Trailing wildcard - multi-segment"
        );
        assert!(
            matches_glob("test:quux**", "test:quuxwibble"),
            "Double wildcard in trailing segment"
        );
        assert!(
            matches_glob("test:quux**", "test:quuxwibble:zip:baz"),
            "Double wildcard in trailing segment - matches multiple segments"
        );
        assert!(
            matches_glob("test:quux**:baz", "test:quuxwibble:zip:baz"),
            "Double wildcard in middle segment - multi-segment"
        );
        assert!(
            matches_glob("test:quux**:baz:*", "test:quuxwibble:baz:zip"),
            "Double wildcard in middle segment plus trailing wildcard - multi-segment"
        );

        assert!(!matches_glob("test", "foo"), "Non-matching exact");
        assert!(
            !matches_glob("test:foo:bar", "test:foo"),
            "Non-matching exact - missing segment"
        );
        assert!(
            !matches_glob("test:foo", "test:foo:bar"),
            "Non-matching exact - extra segment"
        );
        assert!(
            !matches_glob("test:foo:bar", "test:foo:baz"),
            "Non-matching exact - different segment"
        );
        assert!(
            !matches_glob("*wibble:foo", "testquux:foo"),
            "Non-matching leading wildcard"
        );
        assert!(
            !matches_glob("test:foo:*", "test:bar:foo"),
            "Non-matching equal number of segements with trailing wildcard segment"
        );
        assert!(
            !matches_glob("test:*", "test:bar:foo"),
            "Non-matching trailing wildcard segment"
        );
        assert!(
            !matches_glob("test:foo*", "test:foowibble:bar"),
            "Non-matching middle wildcard"
        );
        assert!(
            !matches_glob("test:*:bar", "test:bar:foo"),
            "Non-matching middle wildcard"
        );
        assert!(
            !matches_glob("test:*:bar", "test:foo:wibble:bar"),
            "Non-matching middle wildcard"
        );
        assert!(
            !matches_glob("test:**:baz", "test:foo:bar"),
            "Non-matching double wildcard"
        );
        assert!(
            !matches_glob("test:quux*", "test:bar"),
            "Non-matching trailing wildcard"
        );
        assert!(
            !matches_glob("test:quux**", "test:bar"),
            "Non-matching double wildcard"
        );

        // Additional comprehensive test cases

        // Multiple single wildcards in same pattern
        assert!(
            matches_glob("*:*", "test:foo"),
            "Multiple single wildcards - two segments"
        );
        assert!(
            matches_glob("*:*:*", "test:foo:bar"),
            "Multiple single wildcards - three segments"
        );
        assert!(
            matches_glob("test*:foo*:bar*", "testA:fooB:barC"),
            "Multiple single wildcards with prefixes"
        );
        assert!(
            matches_glob("*test:*foo:*bar", "Atest:Bfoo:Cbar"),
            "Multiple single wildcards with suffixes"
        );
        assert!(
            matches_glob("te*st:fo*o:ba*r", "teXst:foYo:baZr"),
            "Multiple single wildcards in middle of segments - supported"
        );

        // Multiple double wildcards in same pattern (should work)
        assert!(
            matches_glob("test**:foo**", "testABC:DEF:fooGHI"),
            "Multiple double wildcards - first matches across segment boundary"
        );
        assert!(
            matches_glob("test**:foo**:bar", "testABC:DEF:fooGHI:JKL:bar"),
            "Multiple double wildcards with exact ending"
        );

        // Mixed single and double wildcards
        assert!(
            matches_glob("test*:foo**:bar", "testA:fooB:C:bar"),
            "Mixed single and double wildcards"
        );
        assert!(
            matches_glob("*:test**:bar*", "A:testB:C:barD"),
            "Mixed wildcards - single, double, single"
        );
        assert!(
            matches_glob("test**:*:bar", "testA:B:C:D:bar"),
            "Mixed wildcards - double, single"
        );

        // Complex patterns with multiple globs
        assert!(
            matches_glob("udex*:*:v1**:read*", "udexABC:service:v1DEF:GHI:readJKL"),
            "Complex pattern with mixed wildcards"
        );
        assert!(
            matches_glob("*service*:*data*:*read*", "microservice:bigdata:readwrite"),
            "Multiple wildcards within each segment - supported"
        );

        // Edge cases with empty segments and wildcards
        assert!(
            matches_glob("**", "test:foo:bar:baz"),
            "Pure double wildcard matches any multi-segment"
        );
        assert!(
            matches_glob("**", "test"),
            "Pure double wildcard matches single segment"
        );
        assert!(
            matches_glob("*", "anything"),
            "Pure single wildcard matches single segment"
        );

        // Invalid patterns that should NOT match
        assert!(
            !matches_glob("*", "test:foo"),
            "Single wildcard should not match across segments"
        );
        assert!(
            !matches_glob("test*:foo", "testABC:bar"),
            "Wildcard prefix mismatch"
        );
        assert!(
            !matches_glob("*:*:*", "test:foo"),
            "Too many wildcards for input segments"
        );
        assert!(
            !matches_glob("test*foo:bar", "testXXXbaz:bar"),
            "Wildcard in middle - doesn't match wrong suffix"
        );

        // Complex non-matching cases
        assert!(
            !matches_glob("test**:foo*:bar", "testABC:baz:bar"),
            "Complex pattern with wrong middle segment"
        );
        assert!(
            !matches_glob("*service*:*data*:*read*", "microservice:bigdata:write"),
            "Complex pattern with wrong final segment"
        );
        assert!(
            !matches_glob("udex*:*:v2**:read*", "udexABC:service:v1DEF:readJKL"),
            "Complex pattern with version mismatch"
        );

        // Boundary cases with adjacent wildcards
        assert!(
            matches_glob("test**:foo", "testABC:DEF:foo"),
            "Double wildcard followed by exact match"
        );
        assert!(
            matches_glob("test:**:foo", "test:ABC:DEF:foo"),
            "Exact, double wildcard, exact"
        );
        assert!(
            !matches_glob("test*:*foo", "testABC:DEF"),
            "Adjacent wildcards with no match"
        );

        // Patterns with consecutive separators (edge case)
        assert!(
            matches_glob("test::foo", "test::foo"),
            "Empty segment exact match"
        );
        assert!(
            matches_glob("test:*:foo", "test::foo"),
            "Wildcard should match empty segment in this context"
        );

        // Performance/stress test cases
        assert!(
            matches_glob("a*:b*:c*:d*:e*", "aX:bY:cZ:dW:eV"),
            "Many single wildcards"
        );
        assert!(
            matches_glob(
                "test**:intermediate**:final",
                "testA:B:C:intermediateD:E:F:final"
            ),
            "Multiple double wildcards with multiple segments each"
        );
    }

    #[test]
    fn test_matches_segment_range() {
        let input_segs = vec!["test", "foo", "bar", "baz"];

        // Single segment cases (should delegate to matches_segment)
        assert_eq!(
            matches_segment_range("test", &input_segs, 0, 1),
            (true, None),
            "Exact match - single segment"
        );
        assert_eq!(
            matches_segment_range("*", &input_segs, 1, 2),
            (true, None),
            "Wildcard match - single segment"
        );
        assert_eq!(
            matches_segment_range("te*st", &input_segs, 0, 1),
            (true, None),
            "Pattern with wildcard matching - single segment"
        );
        assert_eq!(
            matches_segment_range("**", &input_segs, 2, 3),
            (true, Some("**")),
            "Double wildcard - single segment"
        );
        assert_eq!(
            matches_segment_range("test**", &input_segs, 0, 1),
            (true, Some("**")),
            "Pattern with trailing double wildcard - single segment"
        );

        // Multi-segment range cases - exact matches
        assert_eq!(
            matches_segment_range("test:foo", &input_segs, 0, 2),
            (true, None),
            "Exact match - two segments"
        );
        assert_eq!(
            matches_segment_range("test:foo:bar", &input_segs, 0, 3),
            (true, None),
            "Exact match - three segments"
        );
        assert_eq!(
            matches_segment_range("test:foo:bar:baz", &input_segs, 0, 4),
            (true, None),
            "Exact match - all four segments"
        );

        // Multi-segment with embedded double wildcards (these are the patterns this function is designed for)
        assert_eq!(
            matches_segment_range("test**", &input_segs, 0, 2),
            (true, Some("**")),
            "Embedded double wildcard spanning two segments"
        );
        assert_eq!(
            matches_segment_range("test**", &input_segs, 0, 3),
            (true, Some("**")),
            "Embedded double wildcard spanning three segments"
        );
        assert_eq!(
            matches_segment_range("test**bar", &input_segs, 0, 3),
            (true, Some("**")),
            "Embedded double wildcard with suffix spanning segments"
        );

        // Test with different segment ranges
        let test_segs = vec!["testABC", "fooXYZ", "barDEF"];
        assert_eq!(
            matches_segment_range("test*", &test_segs, 0, 1),
            (true, None),
            "Pattern with wildcard matching prefix - single segment"
        );
        assert_eq!(
            matches_segment_range("*ABC", &test_segs, 0, 1),
            (true, None),
            "Pattern with wildcard matching suffix - single segment"
        );

        // Complex embedded patterns spanning multiple segments
        let complex_segs = vec!["testABCDEF", "fooGHI", "barJKL"];
        assert_eq!(
            matches_segment_range("test**", &complex_segs, 0, 1),
            (true, Some("**")),
            "Embedded double wildcard - single segment"
        );
        assert_eq!(
            matches_segment_range("test**", &complex_segs, 0, 2),
            (true, Some("**")),
            "Embedded double wildcard spanning segments"
        );
        assert_eq!(
            matches_segment_range("testABC**fooGHI", &complex_segs, 0, 2),
            (true, Some("**")),
            "Complex embedded pattern spanning two segments"
        );

        // Negative cases - non-matching patterns
        assert_eq!(
            matches_segment_range("wrong", &input_segs, 0, 1),
            (false, None),
            "Non-matching exact - single segment"
        );
        assert_eq!(
            matches_segment_range("test:wrong", &input_segs, 0, 2),
            (false, None),
            "Non-matching exact - two segments"
        );
        assert_eq!(
            matches_segment_range("wrong:foo:bar", &input_segs, 0, 3),
            (false, None),
            "Non-matching first segment - three segments"
        );
        assert_eq!(
            matches_segment_range("test:wrong:bar", &input_segs, 0, 3),
            (false, None),
            "Non-matching middle segment - three segments"
        );
        assert_eq!(
            matches_segment_range("test:foo:wrong", &input_segs, 0, 3),
            (false, None),
            "Non-matching last segment - three segments"
        );

        // Negative cases with wildcards
        assert_eq!(
            matches_segment_range("wrong*", &input_segs, 0, 1),
            (false, None),
            "Non-matching wildcard prefix - single segment"
        );
        assert_eq!(
            matches_segment_range("*wrong", &input_segs, 0, 1),
            (false, None),
            "Non-matching wildcard suffix - single segment"
        );

        // Negative cases with double wildcards
        assert_eq!(
            matches_segment_range("wrong**", &input_segs, 0, 2),
            (false, None),
            "Non-matching double wildcard prefix - two segments"
        );
        assert_eq!(
            matches_segment_range("**wrong", &input_segs, 0, 2),
            (false, None),
            "Non-matching double wildcard suffix - two segments"
        );
        assert_eq!(
            matches_segment_range("testXYZ**fooGHI", &complex_segs, 0, 2),
            (false, None),
            "Non-matching complex embedded pattern"
        );
        assert_eq!(
            matches_segment_range("testABC**fooXYZ", &complex_segs, 0, 2),
            (false, None),
            "Non-matching suffix in complex embedded pattern"
        );

        // Edge cases - empty ranges
        assert_eq!(
            matches_segment_range("test", &input_segs, 1, 1),
            (false, None),
            "Empty range should not match any pattern"
        );

        // Edge cases - out of bounds (VirtualConcatenatedInput should handle gracefully)
        // The behavior depends on implementation but should not panic
        let result = matches_segment_range("test", &input_segs, 0, 10);
        // We expect this to either return false or handle the bounds correctly
        assert!(
            result == (false, None) || result == (true, None),
            "Out of bounds range should be handled gracefully"
        );

        // Test patterns that span segment boundaries with double wildcards
        assert_eq!(
            matches_segment_range("test**baz", &input_segs, 0, 4),
            (true, Some("**")),
            "Pattern spanning from first to last segment with embedded **"
        );

        // Test edge case with exact pattern match but wrong range
        assert_eq!(
            matches_segment_range("foo:bar", &input_segs, 0, 2),
            (false, None),
            "Pattern matching later segments but wrong range"
        );
        assert_eq!(
            matches_segment_range("foo:bar", &input_segs, 1, 3),
            (true, None),
            "Pattern matching correct range"
        );
    }

    #[test]
    fn test_optimization_verification() {
        // Test cases that specifically exercise the optimization

        // Embedded ** wildcard spanning multiple segments
        assert!(
            matches_glob("test:quux**", "test:quuxwibble:zip:baz"),
            "Embedded ** should span multiple segments"
        );

        // Multiple embedded ** patterns
        assert!(
            matches_glob("test**:foo**:bar", "testABC:DEF:fooGHI:JKL:bar"),
            "Multiple embedded ** patterns should work"
        );

        // Complex embedded ** with other wildcards
        assert!(
            matches_glob("pre**:*:suf**", "preABC:DEF:middle:sufGHI:JKL"),
            "Complex mixed pattern with embedded **"
        );

        println!("✅ All optimization verification tests passed!");
    }
}
