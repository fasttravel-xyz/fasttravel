/**
 * 1. Users of the client-sdk subscribe to Events or EventStreams.
 * 2. WASM service-delegates process server-messages and then publish
 *    Events to JS through the MessageDispatcher of the relevant service.
 * 3. The MessageDispatcher publishes the Events to the Publisher which
 *    broadcasts the Events to all the subscribers.
 */

/**
 * Publisher interface that the MessageDispatcher uses to 
 * route messages/events received from WASM-service-delegates
 * to JS-services. The JS-services then dispatch them to
 * all the subscribers.
 */
export type Publisher = {
    publish(env: EventEnvelope): Promise<void>;
}

/**
 * Message dispatcher used by the service delegates to send
 * messages/events to the JS layer/subscribers.
 */
export class MessageDispatcher {

    constructor(protected publisher: Publisher) { }

    public async recv_message(env: EventEnvelope) {
        await this.publisher.publish(env);
    }
}

export class EventEnvelope {
    constructor(public msg: EventMessage) { }
};

export class EventMessage {
    constructor(public type: string) { }
};


/**
 * List of all Service Events and Payloads:
 * * 
 */

export type EventPayload = JSON | Object;
export class CoreEventMessage extends EventMessage {

    // [todo] store an index here, store the string in a global enum.
    static type: string = "CoreEventMessage";

    constructor(public payload: EventPayload) {
        super(CoreEventMessage.type);
    }
}

