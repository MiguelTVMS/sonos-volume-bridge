# Security Policy

## Supported Versions

Security fixes are provided for the latest stable release of Sonos Volume
Bridge.

| Release | Security support |
| --- | --- |
| Latest stable release | Supported |
| Earlier releases | Not supported |
| Development builds and source snapshots | Best effort |

Users should upgrade to the latest stable release before reporting a
vulnerability that might already have been fixed.

## Reporting a Vulnerability

Please report suspected security vulnerabilities privately through
[GitHub Security Advisories](https://github.com/MiguelTVMS/sonos-volume-bridge/security/advisories/new).

Do not disclose the vulnerability through a public issue, discussion, pull
request, or other public channel.

If private vulnerability reporting is unavailable, contact the repository
owner through a private channel listed on their GitHub profile. Do not include
vulnerability details in an initial public request for contact.

A useful report should include:

- A concise description of the vulnerability and its potential impact.
- The affected Sonos Volume Bridge version.
- Any prerequisites or configuration needed to reproduce it.
- Clear reproduction steps or a minimal proof of concept.
- The expected and observed behavior.
- Any known mitigations or workarounds.
- Whether you would like to be credited in a published advisory.

Remove secrets, personal information, device names, hostnames, network
addresses, account identifiers, and other sensitive data from the report.
Provide additional diagnostics only when requested through the private
reporting channel.

For ordinary bugs, feature requests, or support questions, use the repository's
public issue tracker instead.

## Response Process

The project aims to:

- Acknowledge a report within three business days.
- Provide an initial assessment within seven business days.
- Keep the reporter informed while a confirmed vulnerability is investigated
  and corrected.
- Coordinate the release and public disclosure of a fix with the reporter.

These are response targets rather than guarantees. Resolution time depends on
the severity and complexity of the vulnerability.

If a report is not considered a security vulnerability, the maintainer will
explain why and may suggest reporting it as an ordinary issue.

## Scope

Examples of security issues that are in scope include:

- Unauthorized control of a Sonos device caused by Sonos Volume Bridge.
- Arbitrary code execution or privilege escalation.
- Unsafe parsing of data received from devices or the local network.
- Exposure of sensitive information.
- Installer, update, or release-integrity vulnerabilities.
- Security boundary failures caused by the application's interaction with
  Windows or Sonos devices.

The following are generally outside the project's security scope:

- Vulnerabilities in Sonos products, services, or Windows that are not caused
  or worsened by Sonos Volume Bridge.
- General bugs, compatibility problems, and feature requests.
- Availability problems caused only by local network conditions.
- Reports without a practical security impact.

## Security Research Guidelines

When investigating a potential vulnerability:

- Test only systems and devices that you own or have permission to test.
- Avoid accessing, modifying, or retaining other people's data.
- Do not degrade services, disrupt devices, or perform denial-of-service tests.
- Stop testing and report the issue if sensitive data is encountered.
- Allow a reasonable opportunity to investigate and release a fix before
  public disclosure.

## Disclosure and Credit

Confirmed vulnerabilities will be handled through coordinated disclosure.
When appropriate, the project may publish a GitHub Security Advisory and
request a CVE.

Reporters will be credited with their permission. The project does not
currently offer a bug bounty or financial compensation for vulnerability
reports.
