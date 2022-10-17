**fasttravel-rt (backend)**


This package provides the implementation of the realtime server and the related services. This server is intended to be run inside a single container (pod) and provides static configurations for resource allocation. For dynamic scaling needs multiple instances of this server could be run using any orchestration tool of choice, but this package doesn't provide any tools for that as it is outside the scope of this package. 


**WARNING:** This is a living document and will get updated as we keep implementing more features towards the 0.1.0-alpha.0 release. Some concepts here are not implemented yet, but are mentioned to guide the development.


The fasttravel-rt package consists of the below sub-packages:

* [fasttravel_rt_services](./fasttravel_rt_services/):
    - provides the realtime-services (model, presence, activity, etc.)
* [fasttravel_rt_server](./fasttravel_rt_server/):
    - provides the collaboration-space that hosts a model-root and clients.
    - depends on the fasttravel_rt_services to manage the collaboration space and provide activities to clients.
    - provides the realtime-server and client connection management.
    - hosts multiple collaboration-spaces.


**NOTE:** fasttravel-rt provides only the realtime components. An application will
depend on additional services like authentication and authorization-provider, etc.
We recommend using managed services from cloud-providers for additional
services, though it is upto you how you run these additional services. 
For easily getting started the repository provides some local mocker 
services (session_lambda, auth_provider, db_provider, etc.) which emulate 
the additional service endpoints. You can use your own mockers as well.

**Mockers path (for reference impl and api-calls):** [./fasttravel_rt_server/src/bin/mockers/](./fasttravel_rt_server/src/bin/mockers/)

Below is an example deployment of an application showing where fasttravel
realtime components fit in and what additional services you may need.

figure: [todo]


**DEFINITIONS AND TERMINOLOGIES:**

* **COSPACE**: A collaboration space is a realtime-space, room, or shared-world that hosts multiple clients/participants and provides activities that the present participants could perform on resources under a single model-root.
* **MODEL-ROOT**: A model-root is the root of your resource tree that could be hosted by only one cospace at any instant. This will usually map to the concept of workspaces or documents or 3d-assembly-root or game-world.
* **REALTIME MODEL**: A set of realtime-elements/blocks/objects with persistent representations which could be used to model the app data and state.
* **ACTIVITY**: Actions performed by participants in a collaboration space belong to an activity. At any instant a participant can participate only in a single activity. We may provide activity-stack in future.


**Provides the below endpoints to initiate realtime sessions:**

```rs

// * endpoint: Used by the session-lambda to request the realtime-server
//             to host a model-root by spawning a collaboration space
//             (cospace), when the session-lambda receives the
//             POST /session/join/ request from an authorized client-sdk.
// 
//         POST /realtime/host/
// 
// * endpoint: Used by an authorized client-sdk to query the status of a
//             collaboration space requested in the /realtime/host/ call.
// 
//         GET /realtime/status/:cospace
// 
// * endpoint: The websocket endpoint to start a websocket connection and
//             finally join a session inside a collaboration space.
// 
//         GET UPGRADE WEBSOCKET /realtime/connect/:cospace

```

**LIMITATIONS:**

* The fasttravel_rt package provides pre-defined services fasttravel_rt_services and pre-defined realtime-message protocol buffers fasttravel-rt-proto. To be able to integrate custom services and messages, we have to implement a provider layer. This is not a priority, as pre-defined services are enough for current needs.
* The sub-packages (fasttravel_rt_services and fasttravel_rt_server) have dependencies and can only be used together.
* fasttravel-rt model-roots have a `"domain::namespace::workspace"` scope:

    - domain maps to a tenant.
    - namespace maps to organizations or sub-domains (i.e. the tenants own domain sub division)
    - workspace maps to document or model-root.

  Currently, the realtime server assumes a single tenant setup (i.e all users, models, and assets belong to a single tenant.) So if we want to support multiple tenants (through a managed fasttravel-rt platform) having their own users, models, and assets, the fasttravel-rt suite is not ready yet. Though, we could run a single fasttravel-rt instance for every tenant, but this won't have the desired resource setup or control. A proper multi-tenant approach will require a tenant management console, which is not a priority right now.

