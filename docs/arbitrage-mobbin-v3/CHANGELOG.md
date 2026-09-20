# Changelog

## 3.0 / 19 September 2026

### Visual direction

Replaced the Atlas black/yellow treatment with the requested Mobbin-derived white/ink system. Light mode is now the first-load default. Added the corresponding neutral dark extension, without claiming it comes from the supplied reference.

### Application shell

Replaced the desktop sidebar with a detached sticky pill header. Retained all six destinations. Added consistent tablet/phone floating navigation, with secondary routes and preferences in More.

### Hierarchy and components

Reworked page headings, open metric columns, session rows, evidence-readiness panels, tables, forms, sign-in, search and evidence inspectors. Added 24px container geometry, pill interactions, soft input fields, natural tracking and local variable-weight font requests. No font files were added.

### Evidence exploration

Added an accessible List / Cards control. It retains query and evidence filters, original record identity and the rejection/unknown-net explanation. The source fixture remains one captured record.

### Behavior retained

Retained local navigation, keyboard search, command confirmation and acknowledgement separation, independent reconciliation, immutable preview-session creation, integer-safe cost research, frozen synthetic export and the locked real-trading route.

### Verification

Extended the previous regression suite for the new navigation, light default, action palette, list/card switch and viewport/theme grid. See `qa/results.json` and `qa/QA-REPORT.md` for the actual run. No production integration or deployment was performed.
