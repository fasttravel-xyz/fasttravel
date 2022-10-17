
**fasttravel-rt**

A collection of services, message-protocol, and client-sdk to build realtime multiplayer applications. Targeted for mini, micro and demo-research applications.


>*The current version of this repository is 0.0.1-dev.0 and is undergoing development for the first alpha release 0.1.0-alpha.0, which means that both the public interfaces and internal module structures may change significantly.*


A realtime collaboration space (shared-game-world, room, etc.) hosts participants who have access to shared state and resources. The space provides actvities to the present participants to interact with the shared states and resources. The participants' presence-mode decides availability and authorization for different activities.

States in a shared world could be categorized into three major categories:
* **Data-Prioritized**
    - State could be authoritative or distributed.
    - Authoritative: Central authority receives modification requests and broadcasts updates after applying transactions.
    - CRDTs: Every participant is allowed to make local changes, but eventual consistency is required.
    - Data is prioritized. Participant edits or modifications could be dropped to maintain data integrity and to avoid or solve conflicts.
* **Participant-Prioritized**
    - Network-variables or Shared-WorldStates. Variables have different priorities based on which it is decided how frequently they get updated.
    - Participants are prioritized. System is allowed to make changes to rate of flow, predict future states, and modify data as long as the outcome is fair to all participants.
* **Experience-Prioritized**
    - Cosmetic states to elevate the experience of the participants.
    - Past states irrelevant to new participants, relevant in realtime, but no delivery guarantee.
    - A special effect in a game, a reaction emoji overlay over a live video stream, etc.
    - Experience is prioritized, but not at the cost of data or participant-fairness. 


The **fasttravel-rt** repository provides the below packages:
* **[fasttravel-rt-proto]**
    * Provides the protocol buffer definitions of the realtime-messages.
    * Provides the helpers used by both the server and the client-sdk.
* **[fasttravel-rt]**
    * Provides the realtime server/backend implementation.
    * Provides the necessary services e.g. connection, core, presence, model, etc. (implementation not complete yet)
    * Provides the mockers for local development and testing.
* **[fasttravel-rt-client]**
    * Provides the client-sdk that applications could use to communicate with the realtime server.
    * Provides demo applications for local development and testing.
* **fasttravel-rt-crdt** (future)
    * Depending on the features/flexibility requirement, we may use an existing library.
* **fasttravel-rt-edge** (future)
    * fasttravel-rt requires a server (and necessary cloud functions/resources) deployment, as it has future goals of being able to provide managed realtime-services to multiple tenants. But, sometimes an application just needs a presence-channel or a simple cursor/pointer-activity those could be provided through simpler methods than deploying the relatime-server or building dependency on the rt-client-sdk.


[fasttravel-rt-proto]: ./fasttravel-rt-proto/README.md
[fasttravel-rt]: ./fasttravel-rt/README.md
[fasttravel-rt-client]: ./fasttravel-rt-client/README.md


**Contributing**

Currently, this repository is not open for contributions or feature requests.



**License**

Copyright (c) 2021-Present Cosmoplankton

Licensed under Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or [Apache-2.0][link-web])

[link-web]: http://www.apache.org/licenses/LICENSE-2.0

