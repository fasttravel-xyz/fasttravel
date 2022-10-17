import { FasttravelClient } from "./FasttravelClient"
import { RealtimeService, ServiceUrls, SessionOptions } from "./lib/RealtimeService"
import { CoreService } from "./lib/CoreService"
import { CoreEventMessage } from "./lib/Events"


const createClient = (
    clientPublicKey: string,
    accessToken: string,
    serviceUrls?: ServiceUrls,
): FasttravelClient => {
    return new FasttravelClient(clientPublicKey, accessToken, serviceUrls)
}

export type {
    CoreService
}

export {
    createClient,
    FasttravelClient,
    RealtimeService,
    ServiceUrls,
    SessionOptions,
    CoreEventMessage
}
