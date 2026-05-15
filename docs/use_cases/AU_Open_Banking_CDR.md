# Consumer Data Right (CDR) Open Banking with ID Permanence
> This document exists for the purpose of illustrating a use case for Udex and is not meant to be authoritative.

The Open Banking Consumer Data Right (CDR) in Australia enforces a highly structured, secure, and permission-based architecture for data sharing. This document details the technical mechanics of CDR dataflows, the implementation of **Id Permanence**, and the sequence of interactions between ecosystem participants according to the mandated [Consumer Data Standards (CDS)](https://consumerdatastandardsaustralia.github.io/standards/).

---

## 1. Architectural Components & Core Concepts

### Core Participants
*   **Consumer:** The end-user (individual or business) who owns the data and initiates the request.
*   **Accredited Data Recipient (ADR):** The fintech, lender, or service provider accredited by the **ACCC** to request and ingest data.
*   **Data Holder (DH):** The financial institution (e.g., bank, credit union) holding the consumer's original data.
*   **CDR Register:** The central authority managed by the [ACCC](https://www.accc.gov.au/) that maintains the whitelist of all accredited software products, ADRs, and DHs.

### ID Permanence (Persistent Identifiers)
To protect consumer privacy and prevent cross-Data Holder profiling, the regulatory standards mandate [**Id Permanence**](https://consumerdatastandardsaustralia.github.io/standards/#id-permanence) for resource identifiers (such as Account IDs). Regulatory implementation details can be referenced in the [Official CDS Guidance on ID Permanence](https://cdr-support.zendesk.com/hc/en-us/articles/6140375237647-Guidance-on-ID-permanence).
*   **Customer-Specific Masking:** When a Data Holder exposes an account ID via an API, that ID must be unique to the specific ADR-Consumer combination.
*   **Consistency:** The ID must remain identical for subsequent requests by the same ADR for the same consumer.
*   **Isolation:** If the same consumer shares the exact same bank account with a *different* ADR, the Data Holder must generate a completely different ID.
*   **Irreversibility:** The masked ID must be a cryptographically secure, non-reversible token so that the ADR cannot deduce the original internal bank account number.

> The implication of this is that Data Holders will need to be generate and lookup a lot of new Ids (Keys) for their customers data reliably and scalably - sounds like a mission for Udex.

---

## 2. Phase 1: Consent and Dynamic Client Registration

Before data can flow, the ADR must discover the Data Holder's endpoints via the CDR Register and ensure their software is registered with the Data Holder using OIDC protocols specified on the [Consumer Data Standards GitHub Portal](https://consumerdatastandardsaustralia.github.io/standards/).

```mermaid
sequenceDiagram
    autonumber
    participant ADR as Accredited Data Recipient (ADR)
    participant Reg as CDR Register (ACCC)
    participant DH as Data Holder (DH)

    rect rgb(240, 245, 255)
        note over ADR, Reg: Dynamic Client Registration
        ADR->>Reg: Request Data Holder Brand Metadata
        Reg-->>ADR: Return OIDC Discovery Document & JWKS URL
        ADR->>DH: Post Software Statement Assertion (SSA) to Registration Endpoint
        DH->>Reg: Verify SSA and ADR Accreditation Status
        Reg-->>DH: Validated Status Confirmation
        DH-->>ADR: Return Client ID & Registration Metadata
    end
```

---

## 3. Phase 2: Hybrid Flow Authentication and Authorization

The CDR uses a profile of **OIDC / OAuth 2.0 (FAPI 1.0 Advanced)**. The consumer must be securely redirected to the Data Holder to authenticate without sharing their bank credentials with the ADR, satisfying consumer workflow steps outlined by the [Official CDR Framework Guide](https://www.cdr.gov.au/how-it-works).

> The Data Holder could use Udex here if they don't want to put internal keys for the Consumer in the Tokens sent to the ADR, but we will focus on the Id Permanence scenario below.

```mermaid
sequenceDiagram
    autonumber
    participant Consumer as Consumer
    participant ADR as Accredited Data Recipient (ADR)
    participant DH as Data Holder (DH)

    Consumer->>ADR: 1. Initiate Data Sharing & Define Scope
    ADR->>ADR: Generate Consent Request Record
    ADR->>DH: 2. Pushed Authorization Request (PAR) with Signed Request Object
    DH-->>ADR: 3. Return Authorization Request URI
    ADR->>Consumer: 4. Redirect Consumer to DH Auth URI via Browser
    
    rect rgb(245, 245, 245)
        note over Consumer, DH: Out-of-Band Authentication & Authorization
        DH->>Consumer: 5. Prompt for Customer Identifier (e.g., Login ID)
        Consumer-->>DH: 6. Provide Identifier
        DH->>Consumer: 7. Send One-Time Password (OTP) via SMS/App
        Consumer-->>DH: 8. Input OTP
        DH->>Consumer: 9. Display Account Selection Screen
        Consumer-->>DH: 10. Select Accounts & Confirm Authorization
    end

    DH-->>Consumer: 11. Redirect back to ADR with Auth Code & ID Token
    Consumer->>ADR: 12. Deliver Auth Code & ID Token
    ADR->>DH: 13. Token Exchange (Exchange Auth Code for Access & Refresh Token)
    DH-->>ADR: 14. Return Tokens (Scoped to authorized accounts)
```
---

## 4. Phase 3: Resource Data Retrieval with ID Permanence

Once the ADR holds a valid access token, it requests financial resources. This is where the Data Holder enforces **Id Permanence** mappings to insulate back-end databases.

> This is the generic version: the implementation of this with Udex is shown next

```mermaid
sequenceDiagram
    autonumber
    participant ADR as Accredited Data Recipient (ADR)
    participant DH_API as Data Holder API Gateway
    participant DH_DB as Data Holder Core Systems

    ADR->>DH_API: 1. GET /banking/accounts (with Access Token)
    DH_API->>DH_API: 2. Validate Token & Identify ADR + Consumer Pair
    DH_API->>DH_DB: 3. Fetch Internal Account Records (e.g., Acc No: 123456)
    DH_DB-->>DH_API: 4. Return Raw Financial Data
    
    rect rgb(255, 240, 245)
        note over DH_API: Apply Id Permanence<br>This introduces a persistent identity translation layer where Data Holders must reliably map stable,<br>ADR-specific external resource IDs back to internal banking identifiers across<br>the full consent and system lifecycle without exposing core account references.<br>(We will focus on Accounts here but Transaction are also subject to ID Permanence)
        DH_API->>DH_API: 5. Lookup/Generate Permanent Id for this ADR-Consumer combo<br/>(e.g., 123456 -> 'XyZ_987')
    end

    DH_API-->>ADR: 6. Return Account List payload using Permanent ID ('XyZ_987')
    
    ADR->>DH_API: 7. GET /banking/accounts/XyZ_987/transactions
    DH_API->>DH_API: 8. Reverse map 'XyZ_987' back to Internal Account 123456
    DH_API->>DH_DB: 9. Query Transactions for Account 123456
    DH_DB-->>DH_API: 10. Return Internal Transaction Data
    DH_API-->>ADR: 11. Return Transactions payload masked under ID 'XyZ_987'
```

## 4A. Phase 3: Resource Data Retrieval with ID Permanence implemented with Udex
> The above implemented using Udex to provide the permanent Ids for the Consumer that will be held by the ADR - the keys are different for each Consumer ADR combination for the same financial data entities. The Data Holder API acts as the Udex Indexer and the Data Holder Core Systems act as the Udex Context Holder.

```mermaid
sequenceDiagram
    autonumber
    participant ADR as Accredited Data Recipient (ADR)<br><<Udex:Key Holder>>
    participant DH_API as Data Holder API Gateway<br><<Udex:Indexer>>
    participant UDX as Udex<br><<Udex:Server>>
    participant DH_DB as Data Holder Core Systems<br><<Udex:Context Holder>>

    ADR->>DH_API: 1. GET /banking/accounts (with Access Token)
    DH_API->>DH_API: 2. Validate Token & Identify ADR + Consumer Pair
    DH_API->>DH_DB: 3. Fetch Internal Account Records (e.g., Acc No: 123456)
    DH_DB-->>DH_API: 4. Return Raw Financial Data
    
    rect rgb(255, 240, 245)
        note over DH_API, UDX: Apply Id Permanence<br>In practice, most Data Holders do not dynamically generate opaque IDs on every request.<br>Instead, they maintain a persistent mapping. We will use a Udex Index for this.<br>(We will focus on Accounts here but Transaction are also subject to ID Permanence)
        DH_API->>DH_API: Generate a Udex Context for each Resource in Financial Data for this ADR-Consumer combo (e.g. Customer Id + Account 123456 + ADR Id)
        DH_API->>UDX: Lookup/Generate Udex Keys for each Context/Resource - these will be the Permanent Ids for this ADR-Consumer combo
    end

    note over DH_API, ADR: ADR doesn't get to see Customer Id + Account 123456,<br>it holds the Permanent Id(s) 
    DH_API-->>ADR: 6. Return Account List payload using <permanent_id>(s)

    
    ADR->>DH_API: 7. GET /banking/accounts/<permanent_id>/transactions
    rect rgb(255, 240, 245)
        note over DH_API, UDX: Reverse map <permanent_id> back to Internal Account 123456
        DH_API->>UDX: 8. Look up Context based on <permanent_id> (e.g.) Internal Account 123456
        UDX-->>DH_API: 8a. Return Context for Key (e.g. Customer Id + Account 123456 + ADR Id)
        DH_API->>DH_DB: 9. Query Transactions for Account 123456
        DH_DB-->>DH_API: 10. Return Internal Transaction Data
    end
    DH_API-->>ADR: 11. Return Transactions payload masked <permanent_id>
```

---
## 5. Not Shown: Revocation & Renewal Data flows, internal processes such as account migration etc.
However it should be noted that permanent ids must perist through renewal.

## 6. Security and Compliance Controls

*   **Mutual TLS (mTLS):** All network traffic between ADRs, Data Holders, and the Register requires mTLS using X.509 certificates issued by the CDR Certificate Authority.
*   **Request Object Signing:** Authorization requests must be signed JSON Web Tokens (JWTs) using keys declared in the participant's JWKS.
*   **Consent Lifecycle Management:** Consents can last a maximum of 12 months. ADRs must provide a "Consent Dashboard" allowing users to revoke access at any point, triggering an immediate token revocation cascade back to the Data Holder. Privacy rules and de-identification obligations are strictly regulated under the [OAIC Consumer Data Right Guidelines](https://www.oaic.gov.au/consumer-data-right/consumer-data-right-legislation,-regulation-and-definitions/consumer-data-right-legislation).
