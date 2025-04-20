# Rust Style Guide - Directive Summary
Based on [https://doc.rust-lang.org/style-guide/index.html]

## General Formatting Principles

- Use spaces, not tabs for indentation
- Use 4 spaces per indentation level (all indentation must be multiple of 4)
- Set maximum line width to 100 characters
- Prefer block indent over visual indent for smaller diffs and less rightward drift
- Use trailing commas in comma-separated lists when followed by a newline
- Separate items and statements by zero or one blank lines only
- Use version sorting when sorting is required (handles numeric sequences properly)

## Comments

- Prefer line comments (`//`) over block comments (`/* */`)
- Put a single space after the opening sigil in line comments
- Put single space after opening and before closing sigil in single-line block comments
- Use newlines after opening and before closing sigils in multi-line block comments
- Put comments on their own line when possible
- Use single space before comments that follow code
- Start comments with capital letter and end with period for complete sentences
- Limit comment-only lines to 80 characters or line width maximum, whichever is smaller

## Doc Comments

- Prefer line doc comments (`///`) over block doc comments (`/** */`)
- Use outer doc comments (`///`) over inner doc comments (`//!`) except for module/crate-level docs
- Put doc comments before attributes

## Attributes

- Put each attribute on its own line, indented to item level
- For inner attributes (`#!`), indent to inside of item level
- Format attributes with argument lists like functions
- Put single space before and after `=` in attributes with equal signs
- Use only single `derive` attribute (combine multiple derives into one)
- Preserve order of derived names when combining derives

## Blocks

- Use newline after opening `{` and before closing `}` unless single-line
- Keep keywords before blocks on same line as opening brace with single space
- Indent block contents
- Write empty blocks as `{}`
- Use single-line blocks only for expressions (not statements), single expressions with no statements/comments
- Put spaces after opening and before closing braces in single-line blocks
- Put block attributes on own line before block

## Let Statements

- Put space after `:` and on both sides of `=` when present
- Don't put space before semicolon
- Format on single line when possible
- If multi-line needed, split after `=` first, then after `:` if still too long
- Block-indent expressions that span multiple lines
- For let-else statements: format on single line only if entire statement is short with single-line else block
- Break before `}` in let-else statements, never between `else` and `{`

## Expressions

### Function Calls
- Don't put space between function name and opening parenthesis
- Don't put space between argument and following comma
- Put space between argument and preceding comma
- For single-line calls: no spaces around parentheses or trailing comma
- For multi-line calls: put each argument on block-indented line with trailing comma
- Never break nullary function calls (`func()`) across lines

### Method Calls and Chains
- Don't put spaces around `.`
- Format chains on one line if small, otherwise break before `.` and after `?`
- Block-indent subsequent lines in chains
- Combine first and second elements if length plus indentation allows
- Put multi-line elements on their own lines

### Struct Literals
- Format small struct literals on single line without trailing comma
- For multi-line: put each field on block-indented line with trailing comma
- Put space after colon in field:value pairs
- Put space before opening brace, spaces around braces in single-line form
- Functional update syntax (`..expr`) never has trailing comma, no space after `..`

### Arrays and Tuples
- Use single-line form when possible
- Don't put spaces around brackets/parentheses at edges
- Separate elements with comma followed by space
- For multi-line: block-indent elements with trailing comma
- For array repeat syntax: put space after semicolon only

### Control Flow
- Don't include extraneous parentheses for `if` and `while` expressions
- Put keyword, clauses, and opening brace on single line when possible
- Use single space before and after `else`
- Break after `=` in let expressions and before `in` in for expressions
- Put opening brace on own line when control line is broken
- Single-line if-else allowed only in expression context when small

### Match Expressions
- Don't line-break in discriminant expression
- Break after opening and before closing brace
- Block-indent match arms once
- Use trailing comma on match arms only when not using blocks
- Never start match arm pattern with `|`
- Avoid splitting left-hand side of match arms
- Use blocks for multi-statement, commented, or non-fitting right-hand sides
- Break before `if` in guards when splitting patterns

### Binary Operations
- Include spaces around binary operators (including `=` and assignment ops)
- Use parentheses liberally for clarity
- For line-breaking: break after assignment operators, before other operators
- Put each sub-expression on its own line when breaking
- Block-indent subsequent lines

### Unary Operations
- Don't include space between unary operator and operand
- Exception: must have space after `&mut`
- Avoid line-breaking between unary operator and operand

### Closures
- Don't put extra spaces before first `|` (unless prefixed by keyword)
- Put space between second `|` and expression
- Use function definition syntax between `|`s, elide types when possible
- Omit `{}` when possible; add for return types, statements, comments, or multi-line control flow
- Follow block rules when using braces

### Ranges
- Don't put spaces in ranges (`0..10`, `x..=y`, `..x.len()`, `foo..`)
- When breaking within range, break before range operator and block-indent
- Use parentheses around compound expressions in ranges for precedence clarity

## Patterns

- Format patterns like their corresponding expressions
- Apply same rules as match expressions for pattern formatting
- Use consistent indentation and spacing rules

## Macros

- Format parseable macros like corresponding constructs (e.g., function calls)
- For statement position macros: use parentheses or square brackets, terminate with semicolon
- Don't put spaces around name, `!`, delimiters, or semicolon
- For format string macros: format args before format string on single line if small, format string on own line, args after on single line if they fit

## Sorting Rules

- Use version sorting that handles numeric sequences properly
- Process strings as chunks of non-digits or digits
- Compare numeric chunks by value, non-numeric chunks lexicographically
- Underscore sorts immediately after letters
- Sort non-lowercase before lowercase characters unless specified otherwise
- Handle leading zeros by preferring string with more leading zeros when chunks are numerically equal

## Small Items

- Tools decide definition of "small" based on character count or complexity
- Apply different small item rules for single-line vs multi-line formatting
- Use heuristics like simple names only vs complex sub-expressions

## Udex Specific Guidelines

### Errors
- thiserror crate should be used for errors declared by the code
- APIs should not expose errors from 3rd party libraries or services - these should be wrapped or converted to error types exposed by the API.
- errors should be called <SomethingUseful>Error - i.e. explicitly have the word Error at the end of the name