# Security Policy

## Supported Versions

Only the latest release of `reval` receives security updates.

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

---

## Reporting a Vulnerability

If you discover a security vulnerability or credential exposure issue in `reval`:

1. **Do not create a public GitHub issue.**
2. Send an email with details, reproduction steps, and potential impact to **security@techtideai.io** or open a confidential Security Advisory on GitHub.
3. You will receive an acknowledgment within 24 hours.
4. A fix will be developed and released promptly with an attribution credit (if desired).

---

## Redaction Guarantee

`reval` includes automated high-entropy regex secret scrubbers (`src/ingest/redact.rs`). When exporting trace corpora or running trace error analysis, known API keys (`gho_*`, `ghp_*`, `sk-ant-*`, `sk-proj-*`, bearer tokens, private keys) are automatically scrubbed and replaced with `[REDACTED_API_KEY]`.
