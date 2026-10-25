# Security Policy

OmegaDesk gives remote control over computers, so we take security reports seriously.

## Reporting a vulnerability

**Please do not open a public issue, discussion or pull request for security problems.**

Report vulnerabilities privately through GitHub: go to the [Security tab](https://github.com/OmegaSoftDLL/omegadesk/security) and click **[Report a vulnerability](https://github.com/OmegaSoftDLL/omegadesk/security/advisories/new)**. Only the maintainer can see the report.

Please include:

- the affected component (agent, client, transport, a specific crate) and version or commit;
- steps to reproduce, or a proof of concept;
- the impact you expect, for example unauthenticated screen access, input injection or code execution.

## What to expect

- We acknowledge the report within **3 business days**.
- We send an initial assessment within **10 business days**.
- We fix the issue and agree on a coordinated disclosure date with you. By default, we publish a GitHub Security Advisory when the fix is released, and we credit you unless you prefer to stay anonymous.

## Supported versions

OmegaDesk is in early development and has no stable release yet. Security fixes go to the `main` branch. Once releases start, this section will list the versions that receive fixes.

## Scope

In scope: everything in this repository, including the agent, the client, the protocol, the transport and the build and release workflows.

Out of scope: vulnerabilities in third-party dependencies that are already public (report them upstream; Dependabot tracks them here), and attacks that require prior administrator access to the machine running the agent.
