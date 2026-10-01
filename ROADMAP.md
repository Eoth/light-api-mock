# Roadmap

What is planned for Mimicway, in the order it will be done, and why. Finished work moves to [CHANGELOG.md](CHANGELOG.md); this file only holds what is still ahead.

## What Mimicway stays

Every item below is weighed against these promises; an idea that breaks one of them is reshaped or dropped.

- **One self-contained program.** No database, no agent, no cloud account, nothing to install next to it.
- **Quiet by default.** No telemetry, no update check. Every outbound connection is one the user configured, and the [security model](docs/en/security.md) lists them all.
- **Safe on a laptop.** Local-only until told otherwise; a web page cannot drive it.
- **Light for the person using it.** A mock should take a minute to write and zero minutes to maintain: import what exists, suggest what can be guessed, explain what went wrong.
- **Easy to approve.** Every feature keeps the [reviewer guide](REVIEWING.md) true: small dependency set, no `unsafe`, documented flows and permissions.

## How to read an item

Each item has an identifier that never changes, a size (S: a day or less, M: a few days, L: a week or more, to be split before starting), a short **why**, a **what**, and a **done when** that can be checked. Items marked **Decision needed** wait for a choice from the maintainers before any code.

## Order of work

1. [Ready for an international launch](#1-ready-for-an-international-launch): R2, R10, R6, R11, R12, R13, R14, R9.
2. [Trust and operations](#2-trust-and-operations): T1 to T11.
3. [Make it indispensable](#3-make-it-indispensable): U1 to U16, in the listed order.
4. [Engineering backlog](#4-engineering-backlog): taken alongside, when they unblock an item above.

## 1. Ready for an international launch

### R2. Publish signed releases

Size S (what is left)

**Why.** Today every user builds from source, which is exactly the long review companies refused. A signed artifact with its bill of materials can be approved once and reused.

**Progress.** `.github/workflows/release.yml` (checked with actionlint) builds, on a `vX.Y.Z` tag, static Linux binaries (x86_64, arm64), macOS (Intel, Apple Silicon) and Windows binaries with the UI inside, a multi-architecture image on GHCR assembled from those binaries, CycloneDX SBOMs for the crates and the shipped UI packages, checksums, build provenance attestations for everything, and a keyless cosign signature of the image. The release procedure is in CONTRIBUTING.md and the verification commands in SECURITY.md.

**What.** Cut the first release (0.2.0) with it, make the GHCR package public, and fix whatever the first run reveals.

**Done when.** `cosign verify` and `gh attestation verify` succeed on the published image and binaries, and the README quick start runs as written.

### R6. English code comments

Size L, to split by module

**Why.** Reviewers read the code. The server's comments were French session narratives rather than the reason the code is the way it is; the UI's components and tests still are.

**What.** Module by module, in reading order of the [reviewer guide](REVIEWING.md), rewrite comments in English and keep only what explains *why* (an invariant, a pitfall, a specification reference); history stays in Git. Translate test names and test messages at the same time.

**Progress.** Done: the Rust code (`src/`, `tests/`, `build.rs`, with every test message and log line), the scripts, and the whole UI outside its tests: `main.js`, `App.svelte`, every module of `frontend/src/lib/`, every component of `frontend/src/lib/components/` (473 French lines of 528 before) and the two style sheets of `frontend/src/` (9 lines before). CI fails on a French comment in any of them: `scripts/check-french-comments.mjs` reads comments only (interface strings, fixtures and example data are never reported) and lists the covered paths, which grow with each translated folder. Checking each comment against the code found 14 wrong claims (a wrong file, access rules stated wrongly, a lossless round trip that lost data, styles called shared that are not), corrected rather than translated, and 11 defects, fixed in 9 commits, each with a test that failed first (see the changelog).

**Left**, counted in French comment lines by that script, all in the tests that R11 rewrites: `frontend/e2e/` 422 of its 560 comment lines (`scenario-runner.spec.js` 187), `frontend/src/tests/` 140 of 198 (136 of 194 in the test files, 4 of 4 in `helpers/`); and the French test titles, which are strings, not comments, so the check does not see them.

**Done when.** No French comment left in `frontend/`, and the CI check covers it.

### R11. Unit tests in English

Size M

**Why.** The Vitest suite runs the components in French (`src/tests/setup.js`) and asserts about 700 French texts, while the end-to-end suite and the code use English. Contributors who do not read French cannot follow these tests, and any wording change in the French catalogue breaks them, which blocks R9.

**What.** Switch `setup.js` to English and rewrite the assertions with the English texts (the catalogue maps each French text to its English key, which makes most replacements mechanical); keep one test file that renders the main components in French, so that the French catalogue stays exercised in context.

**Done when.** `npm test` passes in English, and changing a French translation breaks no test outside the French-specific file and `l10n.test.js`.

### R9. Polish the French catalogues

Size S

**Depends on** R11. · **Why.** Many French messages were written without accents ("reserve", "deja", "regle"), which reads as careless to French users.

**What.** Restore accents and typography (non-breaking space before `:`, `«»` quotes) in `src/locales/fr.json` and `frontend/src/locales/fr.json`; the existing catalogue tests keep placeholders intact. Fix the mistranslations the French screenshots show, such as "Réécriture annuaire" for "Directory URL rewriting" and an example name that differs from the English one ("ex: get-siret" for "e.g. get-customer"). Give both typed confirmation keywords the same rule: the restore confirmation asks for a translated keyword (`RESTAURER`), the reset confirmation for `RESET` in every language (`App.svelte`); a translated keyword must stay easy to type on any keyboard, and the guide names it in each language. Then regenerate the screenshots (`npm run docs:screenshots`): the French images show the catalogue as it is today.

**Done when.** A spell check of both French catalogues passes, and the French screenshots are regenerated from them.

### R12. Readable condition labels in the rule form

Size S

**Why.** The list of a rule's conditions shows each one as `QueryParam(id) Eq(42)`: internal type names, identical in every language, which QA and business users cannot be expected to decode and which the guide's screenshots show in English and French alike. The pseudo-locale test does not catch it, because the label is assembled from data. The rule tester and the traffic suggestions already word conditions in the user's language.

**What.** Build the label in `RuleConditionsEditor.svelte` from the source and operator labels the condition form already translates (`Query parameter`, `HTTP header`, `Equals`, `Exists (any value)`…), through `t()` with placeholders, one message per operator shape, as `ObservationSuggestions.svelte` does; translate the new messages in `frontend/src/locales/fr.json`; regenerate the screenshots that show a condition list.

**Done when.** No condition in the interface shows a source or operator type name, in either language, and a Vitest test covers the label of each source and operator.

### R13. Screens open at their top

Size S

**Why.** The interface is one page without a router: switching screens keeps the scroll position. A service opened from far down the list, or reached by saving a long form, opens part-way down with its title and the navigation bar cut off, and the guide's screenshots of the service page show it.

**What.** Scroll to the top when the main screen changes (list, service page, logs, groups, backups, Kafka, TCP), and bring the list back to where it was when the user returns to it, as the expanded groups already are.

**Done when.** An end-to-end test opens a service from below the fold and finds its page at the top, then goes back and finds the list where it left it.

### R14. A header that fits in every language

Size S

**Why.** At 1280 px the navigation bar wraps onto two lines as soon as it holds more than the basic actions: in English for a signed-in super-admin (user badge, "Log out" and "Reset", see `docs/en/screenshots/authentication-user-badge.png`), in French already for a signed-in user or with the Kafka button (`docs/fr/screenshots/group-members.png`, `kafka-message-log.png`). Every language longer than English makes it worse.

**What.** Make the header hold on one line at common desktop widths whatever the language: shorter labels, icons with an accessible name for the secondary actions, or a menu for the rare ones (import, export, backups), chosen from a mock-up in both languages.

**Done when.** An end-to-end test measures the header at 1280 px in English and French, signed in as a super-admin, and finds a single line in both.

### R10. Public supply-chain score

Size S (what is left)

**Why.** Reviewers increasingly start from the OpenSSF Scorecard: it checks pinned dependencies, token permissions, branch protection, signed releases, CI tests and more, and shows the result as a badge.

**Progress.** The repository is public, `.github/workflows/scorecard.yml` publishes the score weekly and on pushes to `develop`, and the README shows the badge. Actions and base images are pinned by digest, workflow permissions are read-only by default, and releases are signed.

**What.** Read the first published report and fix what it flags. Branch protection with required reviews and status checks on `develop` and `main` is a repository setting for the maintainers.

**Done when.** The score is 8 or more.

## 2. Trust and operations

### T1. Any OpenID Connect provider, browser login with PKCE

Size L, to split (discovery and validation first, then the login flow)

**Why.** Authentication works with Keycloak only, through the password grant: the UI sees users' passwords, and OAuth 2.1 drops that grant. Entra ID, Okta, Auth0, Google or GitLab users cannot log in at all.

**What.** Configure an issuer URL and a client; read its discovery document and keys; log in with the authorization code flow and PKCE (the UI never sees a password); map groups or roles from a configurable claim to super-admins. Keycloak becomes one provider among others, and its current variables keep working.

**Done when.** End-to-end tests log in against a fake provider through the redirect flow; the password form is gone when the new mode is configured.

### T2. API tokens for automation

Size M

**Why.** CI pipelines and test suites need to change mocks without a person's credentials.

**What.** Named tokens with a scope (read, write on given groups, admin) and an expiry, created by admins, stored hashed, shown once, revocable from the UI.

**Done when.** A token can drive the management API within its scope and nowhere else, which the authorization tests check for each endpoint.

### T3. Audit log of configuration changes

Size M

**Why.** "Who changed this mock, and when?" is the first question on a shared instance, and reviewers ask for traceability.

**What.** An append-only log of every change (who, when, what, a diff of the service or group), stored next to the configuration with rotation, visible to admins, exportable as JSON lines.

**Done when.** Every mutating endpoint writes one entry, which a test asserts for each of them.

### T4. Restrict where the proxy may connect

Size S

**Why.** An editor can point a proxy at any host the server reaches: internal services, cloud metadata endpoints. Today only the network can stop it.

**What.** `PROXY_ALLOWED_TARGETS`: a list of hosts, domains or CIDR ranges that proxy targets must match, checked when a service is saved and again when a request is forwarded (after DNS resolution); link-local addresses refused unless listed.

**Done when.** A target outside the list is refused at save time and at forwarding time, with tests for DNS names that resolve to a forbidden address.

### T5. Host allow-list on every interface

Size S

**Why.** The DNS-rebinding guard only applies when Mimicway listens on loopback. Listening on `0.0.0.0` without authentication leaves that door open.

**What.** `ALLOWED_HOSTS`: the host names the management API answers to, required (or derived from `API_BASE_URL`) when listening beyond loopback without authentication.

**Done when.** A request with an unlisted `Host` is refused on any interface, and startup warns when the configuration leaves the API reachable by any name.

### T6. Rate limits

Size S

**Why.** The login endpoint and the management API have no throttling.

**What.** Per-client limits on `/api/auth/login` and on mutating API calls, with `429` and `Retry-After`; mocked services stay unlimited unless a rule asks for it (see U11).

**Done when.** Repeated failed logins are slowed down, as a test shows, without any cost on mocked traffic.

### T7. Metrics and traces

Size M

**Why.** Operators want to know whether the mock answers, how fast, and which rules are hit, in the tools they already use.

**What.** An opt-in Prometheus endpoint (requests by service, rule, mode and status; latency; proxy errors; configuration writes) and OpenTelemetry traces exported to a configured endpoint. Both off by default, both listed as outbound flows when on.

**Done when.** A Grafana dashboard example in `docs/` works against a running instance.

### T8. Packaging for clusters and compose

Size M

**Why.** Kubernetes users expect a Helm chart, and compose users a ready file.

**What.** A Helm chart (published as an OCI artifact with the image) built from the same settings as `k8s/base`, and a `compose.yaml` at the root; security contexts stay as strict as the current manifests.

**Done when.** `helm install` and `docker compose up` both give a working instance, tested in CI with kind.

### T9. Configuration as code

Size M

**Why.** Teams want mocks reviewed and versioned with the application, and the same mocks in every environment.

**What.** A read-only mode that loads services from files in a directory (one file per service or group, YAML or JSON), reloads on change, and refuses changes through the API; a `mimicway validate <dir>` command that checks files in CI; a JSON Schema of the configuration for editor completion.

**Done when.** A mock committed in a repository reaches a running instance without the UI, and an invalid file fails the CI command with the same message the UI would show.

### T10. Leave the deprecated YAML crate

Size S

**Why.** `serde_yaml` is no longer maintained, and `smartstring` (pulled in by the script engine) is flagged unmaintained, which `deny.toml` currently tolerates.

**What.** Move to a maintained YAML implementation, keeping the file format byte-compatible; follow Rhai's releases and drop the `smartstring` exception as soon as a release no longer depends on it.

**Done when.** `cargo deny check` passes with an empty `ignore` list.

### T11. Serve under a path prefix

Size M

**Why.** The UI loads `/assets`, `/runtime-config.json` and `/api` from the root of its host, so Mimicway cannot live under a path such as `https://tools.example.com/mimicway/`, which many platforms impose (one host, one prefix per tool).

**What.** A `BASE_PATH` setting: the server mounts the UI, the API and the mocked services under it, the UI is built with relative asset paths and reads the prefix from `runtime-config.json`, and service URLs shown in the UI include it.

**Done when.** An end-to-end run with `BASE_PATH=/mimicway` behind a prefix-stripping proxy passes, and a Kubernetes overlay shows it.

## 3. Make it indispensable

### U1. Import an API description

Size L, to split by format

**Why.** Writing the first mocks by hand is the main cost. Most teams already have an OpenAPI file, a WSDL, a Postman collection or a HAR capture.

**What.** Import an OpenAPI 3 or Swagger 2 document into a service: one rule per operation and status, response bodies from the document's examples, or generated from its schemas with fake data when there are none. Then WSDL (one rule per operation, matched on `SOAPAction` and the body element, with sample envelopes), then Postman collections, HAR files and pasted `curl` commands.

**Done when.** The Petstore OpenAPI document and a public WSDL import into working mocks, as end-to-end tests check, and nothing is sent anywhere during an import.

### U2. Record and replay

Size M

**Why.** Traffic observation suggests rules but leaves each one to accept by hand. Teams often want a faithful snapshot of a real backend in one go.

**What.** A "record" switch on a proxied service that turns every distinct exchange into a rule (grouped by the same correlation as suggestions), credentials removed, then switches the service to mock mode on demand.

**Done when.** Recording a session against a real backend, then cutting the network, replays the same responses for the same requests.

### U3. Verify what the application called

Size M

**Why.** Automated tests need to assert that the application under test called a dependency, how many times and with what: today they can only read the request log by hand.

**What.** `POST /api/verify` with a request pattern (method, path, conditions like a rule's) and an expected count; `DELETE /api/logs` to start a test from a clean journal; a journal size setting for test runs.

**Done when.** A Java and a JavaScript example test use it to assert calls, in the docs.

### U4. Scenarios selected per request

Size M

**Why.** Testing an error case means editing the mock and editing it back, which breaks parallel tests sharing an instance.

**What.** Named variants of a rule's response, picked by a request header (`X-Mimicway-Scenario: timeout`), a cookie or a query parameter, with a default; the UI shows the variants side by side.

**Done when.** Two test runs against the same instance get different scenarios at the same time.

### U5. Stateful mocks and instant CRUD

Size L, to split

**Why.** Many APIs are "create, then read what was created". Static rules cannot follow, and scripts have no memory.

**What.** First, a per-service state machine: a rule can require a state and move to another one (like an order going from `created` to `paid`). Then a "resource" service type: from a path such as `/orders/{id}`, Mimicway answers POST, GET (one and list), PUT, PATCH and DELETE on an in-memory collection, seeded from examples, reset by API.

**Done when.** A create-then-read test passes against a resource service with no rule written.

### U6. Frozen and shifted time

Size S

**Why.** Responses with dates (`now_iso`, `date_future`…) make snapshots and assertions flaky.

**What.** A clock per service or per request (`X-Mimicway-Time`): fixed at an instant, or shifted by a duration; templates and scripts read it instead of the system clock.

**Done when.** The same request with the same frozen time returns byte-identical responses.

### U7. Callbacks and webhooks

Size M

**Why.** Asynchronous APIs answer "accepted", then call back later (payments, document processing). Mocking them today needs another tool.

**What.** A rule can schedule an HTTP call after its response: target, delay, method, templated body and headers. The callback target is an outbound flow: it obeys the proxy allow-list (T4) and appears in the security model.

**Done when.** An end-to-end test receives the callback with values taken from the original request.

### U8. Request coverage and one-click rules from misses

Size S

**Why.** Users do not know which rules are dead, nor what the application asked that no rule answered.

**What.** Count hits per rule (shown in the rule list, reset with the log), and an "unmatched requests" view where "Create a rule" fills the rule form from the request.

**Done when.** A 404 from Mimicway can become a working rule in two clicks.

### U9. Built-in identity provider mock

Size M

**Why.** Almost every application under test needs tokens. Running a real identity server for tests is heavy, and hand-made JWKS mocks are error-prone.

**What.** A service type that behaves as an OpenID Connect provider: discovery document, key set, token endpoint (client credentials, password, authorization code with PKCE), with configurable claims and lifetimes, signing keys generated per instance.

**Done when.** A sample application configured with Mimicway as its issuer logs in and validates tokens.

### U10. More protocols

Size L, to split by protocol

**Why.** Modern applications also talk GraphQL, WebSocket and server-sent events.

**What.** GraphQL first (match on operation name and variables, answer per operation), then server-sent events (a scripted sequence of events with delays), then WebSocket (message-to-message rules, like the raw TCP mock), and gRPC last if asked for.

**Done when.** Each protocol has a guide page and end-to-end tests.

### U11. Faults beyond status codes

Size S

**Why.** Real failures are rarely a clean `500`: connections drop, bodies trickle, responses are cut.

**What.** Chaos options for a dropped connection, a reset after headers, a body sent slowly (bytes per second), a truncated body, and a per-rule rate limit answering `429`.

**Done when.** Each fault is reproduced by a test client that sees the expected transport error.

### U12. Responses from files

Size S

**Why.** Some mocks must return PDFs, images or archives, which templates cannot hold.

**What.** A body fragment that serves a file uploaded through the UI and stored in `DATA_PATH/files`, with its content type; included in exports.

**Done when.** A rule returns an uploaded PDF byte for byte.

### U13. Partial export and mock packs

Size S

**Why.** Import and export only cover the whole configuration, so sharing one service between teams means editing JSON by hand.

**What.** Export a service or a group; import it into another instance with a preview of conflicts. A small gallery of generic, ready-to-import packs in `examples/` (an OAuth2 provider, a payment flow, a CI toolchain).

**Done when.** A group exported from one instance imports into another with its rules intact.

### U14. Contract drift detection

Size M

**Why.** Mocks lie silently when the real API changes.

**What.** For a service with a target, compare on demand the mocked responses with the real backend's for the same captured requests, and report differences in status, fields and types.

**Done when.** Changing a field in the real backend shows up as a drift on its rule.

### U15. Fake data in every locale

Size M

**Why.** Fake data is French today (names, addresses, phone numbers, IBAN, SIREN), which looks odd to most users.

**What.** A locale per service (default from the UI language): names, streets, cities, postcodes, phone numbers and IBANs in that locale's formats; the French-specific kinds stay available.

**Done when.** A service in `en-US` and one in `de-DE` produce plausible local data, and each locale has tests for its formats.

### U16. More interface languages

Size S per language

**Why.** English and French cover a small part of the people who could use Mimicway.

**What.** Spanish, German, Portuguese, Italian and Japanese catalogues for the UI and the server messages, contributed and reviewed by native speakers; the existing tests already check each catalogue.

**Done when.** Each language passes the catalogue tests and a native speaker's review.

## 4. Engineering backlog

### E1. One design system for the UI styles

Size M

**Why.** Button, form and layout styles are repeated in many components, so a visual change touches every file.

**What.** Move them into shared tokens and classes in `app.css`, components keep only what is specific to them; the look stays identical.

**Measured (2026-10-01).** The tokens file defines 53 tokens of the former design system; `app.css` is the only file that reads them, 35 times, always with a fallback equal to the token's value, and 30 tokens are never read (nor is its `.custom-dt` selector): removing the file would change no rendered color or size. Whether the UI keeps that design system as its base, and so this file, is the maintainers' choice to make before the work starts. Components still define their own `.section`, `.btn-icon`, `.btn-xs` and warning boxes (`RuleResponseSection`, `RuleActionSelector`, `RuleConditionsEditor`, `RuleWarnings`, `ServiceForm`, the builders), and `ConditionForm` its own `.form-field`.

**Done when.** No component redefines `.btn` or `.form-field`, and screenshots are unchanged.

### E2. Response builder leftovers

Size M

**Why.** The by-example level cannot rename, add or remove fields, and there is no live preview next to the builder.

**What.** Make the by-example level fully editable, then add a side-by-side preview that highlights the field being edited.

**Done when.** A pasted sample can be reshaped without switching to the detailed level.

### E8. Room for long values in the detailed XML builder

Size S

**Why.** In the detailed XML builder, a node holds its tag, type, source, value and transformation on one line, and the value field gets what is left: an XPath such as `Envelope/Body/recherche/Siret` shows cut after `Envelope/Body/recherche/S` in English and after `Envelope/Body/re` in French (`response-xml-xpath-source.png`), so the user cannot read back what they typed.

**What.** Give the value field a minimum width that fits a usual XPath, letting the line wrap before the value instead of shrinking it, and check the JSON builder's rows the same way.

**Done when.** The XPath of `response-xml-xpath-source.png` shows whole in both languages, the image's subject includes the value field, and the screenshot guard fails on an input whose value overflows it.

### E9. The WSDL mode in the service form

Size S. **Decision needed** (what a REST service does with `?wsdl`).

**Why.** The guide says a SOAP service either relays WSDL requests to the real backend (`Proxy`/`Auto`) or answers them with its rules (`Mock`), but the service form offers no such choice: `Mock` can only be set through the API or the configuration file (saving the form now keeps it instead of resetting it to `auto`). And a REST service, which the guide describes as having "no SOAP-specific handling", still sends `?wsdl` requests to the backend past its rules (`auto`, the default), which on a purely mocked service ends in a 502. `Auto` and `Proxy` behave the same.

**What.** Offer the WSDL mode in the form for a SOAP service; decide what a REST service does with `?wsdl` (its rules, like any request, or the current relay) and make the guide say it in both languages; give `Auto` a meaning of its own or drop one of the two values.

**Done when.** A SOAP service's WSDL mode can be set and read back in the form, the guide describes what REST and SOAP services do with `?wsdl`, and end-to-end tests cover each mode.

### E3. Kafka parity

Size M

**Why.** Kafka rules cannot run scripts, and a single global topic is listened to.

**What.** Run the three script slots for messages, and allow a topic per service.

**Done when.** A Kafka rule can compute its reply with a script, tested with the simulator.

### E4. Raw TCP mock, next step

Size M

**Why.** The TCP mock answers fixed bytes only.

**What.** Templated responses that reuse bytes of the request (an identifier echoed back, a length recomputed), and a capture log like the HTTP one.

**Done when.** A request-response protocol with a correlation identifier can be mocked.

### E5. Observation keys by route pattern

Size S

**Why.** Observation groups exchanges by literal path, so `/orders/1` and `/orders/2` never pool their samples.

**What.** Group by the service's route pattern and suggest path-parameter conditions; add a fuzz target for the suggestion code next to its property tests.

**Done when.** Calls to `/orders/{id}` with different ids produce one suggestion with conditions on `id`.

### E6. Shared test application builder

Size S

**Why.** Several test modules build their own router and server with slight variations.

**What.** Use `server::test_support` everywhere and delete the copies.

**Done when.** One function builds every test application.

### E7. Request context for the proxy path

Size S

**Why.** `do_proxy` and `do_proxy_observed` (`src/server/intercept.rs`) take more arguments than clippy accepts, silenced with `#[allow(clippy::too_many_arguments)]`; so do a test helper of the template engine and the constructors of observation and Kafka log entries.

**What.** Group the request-scoped values in one struct, and give the entry constructors a parameter struct.

**Done when.** No `too_many_arguments` allowance is left in `src/`.
