# Privacy Policy for Sonos Volume Bridge

**Effective date:** August 6, 2026

Sonos Volume Bridge is an independent, community-developed desktop application
published by Miguel.MS. This policy explains what information the application
accesses and how that information is handled.

## Summary

Sonos Volume Bridge does not require an account, include advertising or
analytics, or send telemetry to the publisher. It communicates directly with
Sonos speakers on the user's local network and stores its settings and
diagnostic logs locally on the user's computer.

## Information the application accesses

To provide volume synchronization, Sonos Volume Bridge accesses:

- information exposed by compatible Sonos devices on the local network, such
  as device identifiers, names, local network addresses, volume, and mute state;
- the selected Windows or macOS audio output identifier, volume, and mute state;
  and
- application preferences, including the selected devices, synchronization
  options, maximum volume, volume mapping, start-at-login choice, and diagnostic
  log level.

This information is used only to discover compatible devices, show their
status, remember the user's choices, and synchronize volume and mute state.

## Local network communication

The application discovers and communicates with compatible Sonos devices over
the user's local network using local discovery, control, and event-notification
protocols. These communications are between the computer running the
application and devices on that local network. The application does not send
this information to the publisher or to an analytics or advertising service.

## Local storage and diagnostics

Application preferences are stored in a configuration file on the user's
computer. Diagnostic log files are also stored locally and may contain technical
error information and local device or network identifiers needed to diagnose a
connection problem.

The application does not automatically upload configuration or log files. A
user may choose to share diagnostic information when requesting support, for
example by attaching it to a GitHub issue. Information shared in that way is
handled by the service through which the user submits it and is subject to that
service's privacy terms.

Users can reset application preferences from within the application. They can
remove locally stored application data and logs by uninstalling the application
and deleting any remaining application data, subject to the operating system's
normal file-management behavior.

## Personal information and third parties

Sonos Volume Bridge does not ask for or intentionally collect names, email
addresses, precise location, contacts, financial information, authentication
credentials, audio content, or other personal content. It does not sell user
information or disclose application data to advertisers or data brokers.

The application is not affiliated with, sponsored by, endorsed by, or supported
by Sonos, Inc. Sonos devices and software are governed by Sonos's own terms and
privacy practices.

## Security

The application limits device discovery and control to local-network addresses
and validates received data. Users should install releases only from the
Microsoft Store or the project's official GitHub repository and keep their
operating system and Sonos devices updated.

## Children's privacy

The application is a general-purpose utility and is not directed to children.
It does not knowingly collect personal information from children.

## Changes to this policy

This policy may be updated when the application's data practices change. The
effective date above will be updated when a material revision is published.

## Contact and support

Questions about this policy or requests concerning application data can be
submitted through the project's public issue tracker:

<https://github.com/MiguelTVMS/sonos-volume-bridge/issues>
