# Security policy

## Supported versions

lightMock follows [semantic versioning](https://semver.org) (see [CHANGELOG.md](CHANGELOG.md)). While the version is `0.x`, only the **latest release** receives security fixes; maintenance branches will exist from `1.0.0` on.

| Version | Supported |
|---|---|
| latest release | ✅ |
| any older release | ❌ |

## Reporting a vulnerability

**Please do not open a public issue** for a potential vulnerability (information leak, authentication bypass, injection, path traversal, denial of service, sandbox escape...).

Report it privately through [GitHub Security Advisories](https://github.com/eoth/light-api-mock/security/advisories/new). If that channel is not available to you, write to **etokan.devs@gmail.com**.

Please include, as far as possible:
- a description of the issue and its impact;
- steps to reproduce (lightMock version, relevant configuration such as `AUTH_ENABLED`, `BIND_ADDRESS`, the requests involved);
- a fix or mitigation, if you have one.

### Response times

lightMock is maintained on a best-effort basis: these are targets, not a contractual SLA.

| Step | Target |
|---|---|
| Acknowledgement | 5 business days |
| First assessment (confirmed, not reproducible, more information needed) | 10 business days |
| Fix or mitigation plan | Depends on severity, with priority to authentication, file access and script sandbox issues |

We ask for coordinated disclosure: please give us time to publish a fix before disclosing technical details.

## Scope

In scope:
- the Rust binary (`src/`): matching engine, proxy, templates, Rhai script sandbox, authentication, persistence, REST API, raw TCP mock;
- the web UI (`frontend/src/`);
- the provided `Dockerfile` and Kubernetes manifests (`k8s/`).

Out of scope:
- the services that users choose to mock or proxy (`real_target_url`): lightMock does not control their security;
- Keycloak or Kafka deployments provided by the user;
- vulnerabilities of third-party dependencies with no demonstrated impact on lightMock: please report them upstream. Dependencies are checked continuously in CI (`cargo deny`, `npm audit`, image scan).

## How lightMock is secured

[docs/security.md](docs/security.md) describes the threat model, every outbound network flow, the defaults and how to harden a deployment. [REVIEWING.md](REVIEWING.md) is a guide for a security or code review of the project.
