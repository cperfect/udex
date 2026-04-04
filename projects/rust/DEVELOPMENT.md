# Rust Development Guide

## Guidelines
> The key words “MUST”, “MUST NOT”, “REQUIRED”, “SHALL”, “SHALL NOT”, “SHOULD”, “SHOULD NOT”, “RECOMMENDED”, “MAY”, and “OPTIONAL” in this document are to be interpreted as described in [RFC 2119](https://www.ietf.org/rfc/rfc2119.txt).

1. **Udex MUST the Rust Style Guide**: https://doc.rust-lang.org/style-guide/index.html (`rustfmt` should take care of most of this).
1. **Errors SHOULD follow the NRC Error Design Guidelines**: https://nrc.github.io/error-docs/error-design/index.html
2. **Internal API/Libraries SHOULD follw othe Rust Lang API Design Guidelines**: https://rust-lang.github.io/api-guidelines/ 

### Udex Specific Guidelines

### Errors
1. **thiserror crate SHOULD be used for errors declared by the code**
1. **APIs SHOULD NOT expose errors from 3rd party libraries or services** these should be wrapped or converted to error types exposed by the API.
1. **errors SHOULD be called <SomethingUseful>Error** i.e. explicitly have the word Error at the end of the name