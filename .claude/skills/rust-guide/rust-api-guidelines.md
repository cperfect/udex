# Rust API Guidelines — Checklist

Source: https://rust-lang.github.io/api-guidelines/checklist.html

> Guidelines are recommendations, not mandates. Apply judgement. For Udex internal crates, prioritise interoperability, type safety, and dependability sections.

---

## 1. Naming
- **C-CASE**: Casing conforms to RFC 430 (`UpperCamelCase` types, `snake_case` functions/fields, `SCREAMING_SNAKE_CASE` constants)
- **C-CONV**: Ad-hoc conversions follow `as_`/`to_`/`into_` conventions
- **C-GETTER**: Getter names follow Rust convention (no `get_` prefix)
- **C-ITER**: Iterator-producing collection methods named `iter`, `iter_mut`, `into_iter`
- **C-ITER-TY**: Iterator type names match the method that produces them
- **C-FEATURE**: Feature names are free of placeholder words
- **C-WORD-ORDER**: Names use consistent word order

## 2. Interoperability
- **C-COMMON-TRAITS**: Types eagerly implement common traits: `Copy`, `Clone`, `Eq`, `PartialEq`, `Ord`, `PartialOrd`, `Hash`, `Debug`, `Display`, `Default`
- **C-CONV-TRAITS**: Conversions use `From`, `AsRef`, `AsMut`
- **C-COLLECT**: Collections implement `FromIterator` and `Extend`
- **C-SERDE**: Data structures implement `Serialize`/`Deserialize` where appropriate
- **C-SEND-SYNC**: Types are `Send` and `Sync` where possible
- **C-GOOD-ERR**: Error types are meaningful and well-behaved
- **C-NUM-FMT**: Binary number types provide `Hex`, `Octal`, `Binary` formatting
- **C-RW-VALUE**: Generic reader/writer functions take `R: Read` and `W: Write` by value

## 3. Macros
- **C-EVOCATIVE**: Input syntax is evocative of the output
- **C-MACRO-ATTR**: Macros compose well with attributes
- **C-ANYWHERE**: Item macros work anywhere items are allowed
- **C-MACRO-VIS**: Item macros support visibility specifiers
- **C-MACRO-TY**: Type fragments are flexible

## 4. Documentation
- **C-CRATE-DOC**: Crate-level docs are thorough and include examples
- **C-EXAMPLE**: All public items have a rustdoc example
- **C-QUESTION-MARK**: Examples use `?`, not `try!` or `unwrap`
- **C-FAILURE**: Function docs include error, panic, and safety considerations
- **C-LINK**: Prose contains hyperlinks to relevant items
- **C-METADATA**: `Cargo.toml` includes authors, description, license, homepage, documentation, repository, keywords, categories
- **C-RELNOTES**: Release notes document all significant changes
- **C-HIDDEN**: Rustdoc does not show unhelpful implementation details

## 5. Predictability
- **C-SMART-PTR**: Smart pointers do not add inherent methods
- **C-CONV-SPECIFIC**: Conversions live on the most specific type involved
- **C-METHOD**: Functions with a clear receiver are methods
- **C-NO-OUT**: Functions do not take out-parameters
- **C-OVERLOAD**: Operator overloads are unsurprising
- **C-DEREF**: Only smart pointers implement `Deref`/`DerefMut`
- **C-CTOR**: Constructors are static inherent methods

## 6. Flexibility
- **C-INTERMEDIATE**: Functions expose intermediate results to avoid duplicate work
- **C-CALLER-CONTROL**: Caller decides where to copy and place data
- **C-GENERIC**: Functions minimise assumptions via generics
- **C-OBJECT**: Traits are object-safe if they may be useful as trait objects

## 7. Type Safety
- **C-NEWTYPE**: Newtypes provide static distinctions
- **C-CUSTOM-TYPE**: Arguments convey meaning through types, not `bool` or `Option`
- **C-BITFLAG**: Sets of flags use `bitflags`, not enums
- **C-BUILDER**: Builders enable construction of complex values

## 8. Dependability
- **C-VALIDATE**: Functions validate their arguments
- **C-DTOR-FAIL**: Destructors never fail
- **C-DTOR-BLOCK**: Destructors that may block have alternatives

## 9. Debuggability
- **C-DEBUG**: All public types implement `Debug`
- **C-DEBUG-NONEMPTY**: `Debug` representation is never empty

## 10. Future Proofing
- **C-SEALED**: Sealed traits protect against downstream implementations
- **C-STRUCT-PRIVATE**: Structs have private fields
- **C-NEWTYPE-HIDE**: Newtypes encapsulate implementation details
- **C-STRUCT-BOUNDS**: Data structures do not duplicate derived trait bounds

## 11. Necessities
- **C-STABLE**: Public dependencies of a stable crate are stable
- **C-PERMISSIVE**: Crate and its dependencies have a permissive license
