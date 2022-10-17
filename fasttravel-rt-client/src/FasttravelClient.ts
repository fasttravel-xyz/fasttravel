
import { RealtimeService } from "./lib/RealtimeService";
import { ServiceUrls } from "./lib/RealtimeService";

export class FasttravelClient {

    readonly realtime: RealtimeService;

    constructor(
        clientPublicKey: string,
        accessToken: string,
        serviceUrls?: ServiceUrls,
    ) {
        this.realtime = new RealtimeService(accessToken, serviceUrls);
    }
}
