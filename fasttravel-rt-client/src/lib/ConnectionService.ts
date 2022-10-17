
import { PubSub } from "./PubSub"
import { MessageDispatcher, EventEnvelope, Publisher } from "./Events"
import { RealtimeModule, ConnectionServiceDelegate } from "../../pkg/fasttravel_rt_client_private"

/**
 * This is a private service, not exposed to users of client-sdk.
 */
export type ConnectionService = {}

export class ConnectionServiceImpl implements Publisher, ConnectionService {

    protected pubsub: PubSub<undefined>;
    protected delegate: ConnectionServiceDelegate;

    constructor(protected rtModule: RealtimeModule) {
        const dispatcher = new MessageDispatcher(this);
        this.delegate = rtModule.init_connection(dispatcher);
        this.pubsub = new PubSub();
    }

    /**
     * Add message transformations as needed and publish to
     * the subscribers of the observable.
     * @param env 
     * @returns 
     */
    public publish(env: EventEnvelope): Promise<void> {
        return this.pubsub.publish(env);
    }
}
