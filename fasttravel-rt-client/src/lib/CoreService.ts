
import { PubSub, Observable } from "./PubSub"
import { MessageDispatcher, EventEnvelope, Publisher, CoreEventMessage } from "./Events"
import { RealtimeModule, CoreServiceDelegate } from "../../pkg/fasttravel_rt_client_private"


export type CoreService = {
    sendTextMessage(msg: string): void;
    events(): Observable<CoreEventMessage>;
}

export class CoreServiceImpl implements Publisher, CoreService {

    protected pubsub: PubSub<CoreEventMessage>;
    protected delegate: CoreServiceDelegate;

    constructor(protected rtModule: RealtimeModule) {
        const dispatcher = new MessageDispatcher(this);
        this.delegate = rtModule.init_core(dispatcher);
        this.pubsub = new PubSub();
    }

    // Traces the message and logs it in the realtime-server.
    // Useful in testing, similar to ping but with more info.
    public sendTextMessage(msg: string): void {
        this.delegate.send_text_message(msg);
    }

    public events(): Observable<CoreEventMessage> {
        return this.pubsub;
    }

    public publish(env: EventEnvelope): Promise<void> {
        // Add message transformations as needed and publish to subscribers
        return this.pubsub.publish(env);
    }
}
