# Private account access and Paper / Real workspaces

This owner-requested change replaces the shipped Demo/Connected chooser. The
production dependency graph no longer imports DemoApp or synthetic demo data.
The existing account session is checked first; otherwise the entry is an email
and password sign-in page. Paper trading and Real trading are the two workspace
choices. Real trading is visibly unavailable and cannot dispatch orders. Selecting
it does not modify a research session. Existing PAPER/OBSERVE/REPLAY records keep
their exact modes and provenance; automatic virtual fills are not invented.

## First-owner activation

The requesting repository owner receives a private one-use activation link. It
lets the owner choose their email and password. There is no public registration,
default password or email-delivery claim. The link is a bearer credential; do not
share it, post it in an issue, or place it in logs. Its fragment is removed from
the browser address immediately and is never saved in browser storage.

`config/owner-bootstrap.json` contains only a SHA-256 verifier of a randomly
generated 256-bit token, a fixed expiry and the existing deployment origin. The
plaintext token is not committed or printed by builds. On the matching origin,
startup inserts the invitation only for the unactivated singleton and only if
none exists. An expired link is not renewed by restarting/redeploying. Activation
atomically installs the account and consumes the link; no later restart can
reopen registration. Another deployment origin does not seed this verifier.
Startup explicitly reports expired/unavailable initial setup rather than
silently accepting an ownerless deployment. An existing valid invitation or
activated owner is not replaced. This initial delivery link expires at its
committed absolute timestamp. It is
not a permanent recovery credential. Redeem it privately and then use normal login.

## Passwords and sessions

Passwords use ring's PBKDF2-HMAC-SHA256 with 600,000 iterations, a fresh random
32-byte salt and 32-byte verifier. ring is already in the locked dependency tree;
this adds a direct dependency without changing its version. KDF work runs outside
the asynchronous request runtime with at most two concurrent computations. Separate bounded attempt buckets cover sign-in, activation, legacy access and
authenticated password changes. Each trusted TCP peer/subject/flow allows ten
attempts per monotonic sixty-second window. Email, invitation verifier and
authenticated session respectively supply flow subjects. Arbitrary forwarding
headers cannot change the trusted peer. Behind a proxy or NAT, users sharing
both the peer and account may share a bucket; no per-end-user-IP guarantee or
distributed multi-instance limiter is claimed. Each flow holds at most 1,024
live buckets, independently, so a public login flood cannot consume the
activation or authenticated password-change bucket table.
Unknown emails perform the same bounded KDF as wrong passwords. This does not
claim FIPS certification or a deployment load test.

New passwords accept 15-128 Unicode characters, at most 512 UTF-8 bytes. Inputs
are never silently truncated. Email identities are normalized lowercase ASCII.
Session cookies retain HttpOnly, SameSite=Strict, HTTPS Secure and eight-hour
expiry; CSRF/Origin checks remain mandatory for authenticated changes. Cookies
survive a browser reload, not a server restart. The existing maximum of eight
in-memory sessions remains. Account storage persists in PostgreSQL.

All requests with an authenticated session compare its credential revision with
the durable account revision. A password reset/change invalidates sessions from
other API processes as well, not only this process's cache. Work already admitted
before revocation may still finish. This is not cancellation of trading workers.
Research data stays attached to the stable `operator` identity, never to a mutable
email or password. No research record is reassigned or deleted.

The old `/v1/auth/login` operator-secret contract remains for existing explicitly
configured automation only before an account is activated. Once activated, that
secret can no longer establish a session and previously issued bootstrap sessions
fail the revision check. It is not an alternate permanent account password.

## Change or recover a password

Use **Change password** in the authenticated web application. The current
password and CSRF token are required; a successful change signs out every session.
No account-changing operation is enabled during an unresolved research mutation.

For a forgotten password, the owner uses a private interactive terminal in the
existing control-api container, not a new project or an exposed admin endpoint:

```sh
control-api --invite-owner OWNER_EMAIL
```

This requires the service's existing database configuration and an interactive
terminal. It creates a fresh random 30-minute link bound to the existing email
and credential revision; issuing it does not reset the password until redeemed.
A new link invalidates the preceding one. The command refuses redirected output,
so the bearer link cannot accidentally enter service/CI logs. Never capture it
in a shared screenshot. No mail provider is provisioned and the UI does not claim
a reset email was sent. `ARB_OPERATOR_SECRET` is not needed for daily login.

## Verification and boundaries

Account tests use isolated real PostgreSQL schemas and the actual HTTP router.
They cover first-claim races, token expiry/replay, matching email, persistence,
legacy bypass refusal, CSRF/origin failures, rate limits, password rotation and
revocation across separate API states. A Python hashlib reference validates the
KDF vector. User-interface tests cover login-first entry, absence of a demo,
private activation, password changes, no browser credential persistence, responsive
themes and zero command requests when switching Paper/Real workspaces. Existing
research data-quality, PENDING/APPLIED, idempotency and export tests remain.

This access change does not supply automatic paper settlement, active hosted
market collection or real-money execution. The disabled Real page stays disabled
even if an inconsistent frontend capability response claims live availability.
No wallet, signing, provider key or funding prompt is introduced.

References: https://docs.rs/ring/0.17.14/ring/pbkdf2/index.html and
https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html.
