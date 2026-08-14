# Architecture

## System model

SonosVolumeBridge keeps one chosen Sonos player and one local output device in synchronization.

```mermaid
flowchart TB
  subgraph Domain
    D[domain crate\npolicies and pure values]
  end
  subgraph Synchronization
    SY[synchronization crate\nstate machine + effects]
  end
  subgraph Integration
    I[integration crate\nports and coordinator]
  end
  subgraph Platform
    P1[platform-audio crate\nWindows/macOS callbacks]
    P2[sonos crate\ndiscovery, SOAP, callbacks]
  end
  subgraph Shell
    S[src-tauri crate\nruntime supervisor + config + tray]
  end
  UI["ui + Tauri frontend"] 

  UI <--> S
  S --> I
  I --> D
  SY --> D
  I --> SY
  P1 --> I
  I --> P1
  P2 --> I
  I --> P2
  S --> P2
  S --> P1

  classDef domain fill:#e8f2ff,stroke:#3d67af,color:#17325d;
  class D,SY,I,P1,P2 domain;
  classDef shell fill:#ffefdf,stroke:#b56a1f,color:#4e2a00;
  class S shell;
  classDef ui fill:#f1f7ff,stroke:#4f46e5,color:#1f2a44;
  class UI ui;
```

## Architectural layers

- `domain`: policy and data model only. No Tauri, OS APIs, or networking.
- `synchronization`: policy machine. It processes normalized local and Sonos events and emits desired side effects.
- `integration`: async coordinator and port abstractions.
  - coalesces local volume updates through bounded channels,
  - deduplicates repeated Sonos command writes,
  - applies optional mute mapping,
  - updates timing metrics.
- `sonos`: local-network client for SSDP, SOAP, GENA subscribe/renew/unsubscribe, and callback listener validation.
- `platform-audio`: OS adapters for local output change events and setting local volume or mute.
- `src-tauri`: composition root.
  - reads validated settings,
  - owns runtime lifecycle and tray status snapshots,
  - hosts command surface for frontend actions.

## Runtime lifecycle

```mermaid
flowchart LR
  A[Configuration load] --> B[RuntimeManager restart]
  B --> C[Stop old generation]
  C --> D[Resolve selected Sonos by UDN]
  D --> E[Attach local audio output]
  E --> F[Read Sonos baseline and local state]
  F --> G[Seed synchronizer on startup]
  G --> H{twoWaySynchronization}
  H -->|true| I[Apply Sonos baseline locally]
  H -->|false| J[Send local baseline to Sonos]
  I --> K[Open callback listener]
  J --> K
  K --> L[SUBSCRIBE event channel]
  L --> M[On Sonos event] --> N[Coordinator -> Synchronizer -> Apply local or clear pending]
  K --> O[on lost callbacks] --> P[Polling fallback + renew]
  P --> K
```

## State and direction behavior

There are three related layers of state:

- Domain synchronization state (`domain::SyncState`): `Connecting`, `Synchronized`,
  `WaitingForSonosConfirmation`, `Degraded`.
- Integration health mode (`integration::Health`): `Healthy`,
  `SubscriptionDegraded`, `PollingFallback`.
- UI/runtime status: `Connecting`, `Synchronized`, `SonosUnavailable`,
  `LocalAudioUnavailable`, `UnsupportedLocalDevice`, and related states.

```mermaid
stateDiagram-v2
  [*] --> Connecting
  Connecting --> Synchronized : SonosConfirmed
  Connecting --> WaitingForSonosConfirmation : LocalChanged
  WaitingForSonosConfirmation --> Synchronized : SonosConfirmed
  Synchronized --> Degraded : Integration deems unhealthy
  Degraded --> Synchronized : recovered updates
  Connecting --> SonosUnavailable : selected Sonos cannot be resolved
  WaitingForSonosConfirmation --> SonosUnavailable : session failure
  Synchronized --> LocalAudioUnavailable : local adapter unavailable
```

## Directional model

Synchronization direction is explicit:

- Two-way mode (default): Sonos confirmation maps back to local output.
- One-way mode: local output remains authoritative; local values are pushed to Sonos.

Both modes still enforce local intent deduplication and Sonos confirmation authority.

## Mapping and policy knobs

- `mapping`: `linear`, `cappedLinear`, or `piecewise`.
- `maximumSonosVolume`: global safety cap before write.
- `synchronizeMute` and `muteSpeakerAtZeroVolume`: mute behavior.
- `followDefaultAudioDevice` vs `fixedAudioDeviceId`: local endpoint selection.
- `fallbackPolling`: event loss handling strategy.

## Data safety boundaries

- No direct filesystem, network, or Sonos protocol action from the frontend.
- Protocol URLs and callback targets are validated before network calls.
- Diagnostics and frontend status are redacted and human friendly.
- Configuration writes are atomic with `.json.tmp` staging and schema validation.
